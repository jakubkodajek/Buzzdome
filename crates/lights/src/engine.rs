//! The lights thread. It owns the desk and the output: the show thread only
//! queues what happened (`Control`) and never waits for a device. A frame
//! goes out at the output's rate whether or not anything changed, as DMX
//! fixtures and nodes expect; an output that fails is opened again a
//! second later, so unplugging an interface mid-show costs only its light.

use crate::desk::Desk;
use crate::dmx::{self, DmxOutput, OutputError, Universe, FRAME_EVERY};
use crate::setup::{LightsSetup, LookId, OutputChoice};
use crate::signals::{self, Signal, SignalOut, Target};
use quiz_core::{Cue, Stage};
use serde::Serialize;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ts_rs::TS;

/// How long a lost output or message target waits before it is opened again.
pub const RETRY_AFTER: Duration = Duration::from_secs(1);

/// What the lights thread is told.
#[derive(Debug, Clone)]
pub enum Control {
    Setup(LightsSetup),
    Stage(Stage),
    Cue(Cue),
    Signal(Signal),
    /// Holds a look whatever the show does, or lets go.
    Hold(Option<LookId>),
    Blackout(bool),
}

/// The output as the Director shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputStatus {
    /// No output chosen.
    Off,
    Connected { name: String },
    /// Opening or sending failed; it is tried again every `RETRY_AFTER`.
    Lost { error: OutputError },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LightsStatus {
    pub output: OutputStatus,
    /// Why the show's events do not reach where the setup sends them, if so.
    pub messages: Option<OutputError>,
    /// The look showing, if any.
    pub look: Option<LookId>,
    pub hold: Option<LookId>,
    pub blackout: bool,
}

/// Opens the DMX output a setup chooses; none for no output.
pub type OpenOutput = Box<dyn Fn(&OutputChoice) -> Result<Option<Box<dyn DmxOutput>>, OutputError> + Send>;
/// Opens one place a setup sends messages.
pub type OpenSignals = Box<dyn Fn(&Target) -> Result<Box<dyn SignalOut>, OutputError> + Send>;

/// Opens outputs: the real ones in the app, fakes in tests.
pub struct Openers {
    pub output: OpenOutput,
    pub signals: OpenSignals,
}

impl Default for Openers {
    fn default() -> Openers {
        Openers { output: Box::new(dmx::open), signals: Box::new(signals::open) }
    }
}

/// What the thread shares with the app.
struct Shared {
    status: LightsStatus,
    universe: Universe,
}

/// The app's handle on the lights thread.
pub struct Lights {
    controls: Sender<Control>,
    shared: Arc<Mutex<Shared>>,
}

impl Lights {
    /// Starts the lights thread with `setup`. `clock` gives the time in
    /// milliseconds, never going back; `notify` hears every change of status.
    pub fn start(
        setup: LightsSetup,
        clock: impl Fn() -> u64 + Send + 'static,
        openers: Openers,
        notify: impl Fn(&LightsStatus) + Send + 'static,
    ) -> Lights {
        let (controls, received) = mpsc::channel();
        let status = LightsStatus { output: OutputStatus::Off, messages: None, look: None, hold: None, blackout: false };
        let shared = Arc::new(Mutex::new(Shared { status, universe: Universe::default() }));
        let thread = Thread {
            messages: setup.messages.targets().into_iter().map(MessageOut::new).collect(),
            desk: Desk::new(setup),
            output: None,
            output_retry: None,
            shared: shared.clone(),
            openers,
            notify: Box::new(notify),
        };
        std::thread::Builder::new()
            .name("lights".into())
            .spawn(move || thread.run(received, clock))
            .expect("spawn lights thread");
        Lights { controls, shared }
    }

    /// Queues `control` for the lights thread. A thread that ended (it
    /// never does while the app runs) takes nothing.
    pub fn send(&self, control: Control) {
        let _ = self.controls.send(control);
    }

    pub fn status(&self) -> LightsStatus {
        self.shared.lock().unwrap().status.clone()
    }

    /// The last frame sent, or that would have been with no output.
    pub fn universe(&self) -> Universe {
        self.shared.lock().unwrap().universe.clone()
    }
}

/// One place messages go, opened when the first message after its retry
/// time comes.
struct MessageOut {
    target: Target,
    out: Option<Box<dyn SignalOut>>,
    retry: Instant,
    error: Option<OutputError>,
}

impl MessageOut {
    fn new(target: Target) -> MessageOut {
        MessageOut { target, out: None, retry: Instant::now(), error: None }
    }

    fn send(&mut self, signal: &Signal, open: &OpenSignals) {
        let now = Instant::now();
        if self.out.is_none() && now >= self.retry {
            match open(&self.target) {
                Ok(out) => (self.out, self.error) = (Some(out), None),
                Err(error) => (self.retry, self.error) = (now + RETRY_AFTER, Some(error)),
            }
        }
        if let Some(Err(error)) = self.out.as_mut().map(|out| out.send(signal)) {
            (self.out, self.retry, self.error) = (None, now + RETRY_AFTER, Some(error));
        }
    }
}

struct Thread {
    desk: Desk,
    output: Option<Box<dyn DmxOutput>>,
    /// When to open the output again; none while it is open or none is chosen.
    output_retry: Option<Instant>,
    messages: Vec<MessageOut>,
    shared: Arc<Mutex<Shared>>,
    openers: Openers,
    notify: Box<dyn Fn(&LightsStatus) + Send>,
}

impl Thread {
    fn run(mut self, controls: Receiver<Control>, clock: impl Fn() -> u64) {
        self.output_retry = Some(Instant::now());
        let mut next_frame = Instant::now();
        loop {
            match controls.recv_timeout(next_frame.saturating_duration_since(Instant::now())) {
                Ok(control) => self.take(control, clock()),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            let now = Instant::now();
            if now < next_frame {
                continue;
            }
            self.frame(now, clock());
            let every = self.output.as_ref().map_or(FRAME_EVERY, |output| output.frame_every());
            // A thread that fell behind starts counting again, rather than sending a burst.
            next_frame = (next_frame + every).max(now);
        }
    }

    fn take(&mut self, control: Control, now: u64) {
        match control {
            Control::Setup(setup) => {
                if setup.output != self.desk.setup().output {
                    // Dropping an output lets go of its interface.
                    self.output = None;
                    self.output_retry = Some(Instant::now());
                }
                if setup.messages != self.desk.setup().messages {
                    // A target that stays keeps its connection.
                    let mut before = std::mem::take(&mut self.messages);
                    self.messages = setup
                        .messages
                        .targets()
                        .into_iter()
                        .map(|target| match before.iter().position(|m| m.target == target) {
                            Some(i) => before.swap_remove(i),
                            None => MessageOut::new(target),
                        })
                        .collect();
                    let error = self.messages.iter().find_map(|m| m.error.clone());
                    self.update(|status| status.messages = error);
                }
                self.desk.set_setup(setup);
            }
            Control::Stage(stage) => self.desk.set_stage(stage),
            Control::Cue(cue) => self.desk.cue(cue, now),
            Control::Signal(signal) => self.signal(&signal),
            Control::Hold(look) => self.desk.hold(look),
            Control::Blackout(on) => self.desk.blackout(on),
        }
    }

    fn signal(&mut self, signal: &Signal) {
        for message in &mut self.messages {
            message.send(signal, &self.openers.signals);
        }
        let error = self.messages.iter().find_map(|m| m.error.clone());
        self.update(|status| status.messages = error);
    }

    fn frame(&mut self, at: Instant, now: u64) {
        if self.output.is_none() && self.output_retry.is_some_and(|retry| at >= retry) {
            match (self.openers.output)(&self.desk.setup().output) {
                Ok(output) => {
                    let status = output.as_ref().map_or(OutputStatus::Off, |o| OutputStatus::Connected { name: o.name() });
                    self.output = output;
                    self.output_retry = None;
                    self.update(|s| s.output = status);
                }
                Err(error) => {
                    self.output_retry = Some(at + RETRY_AFTER);
                    self.update(|s| s.output = OutputStatus::Lost { error });
                }
            }
        }
        // The status and the universe first, so that a frame sent is one the
        // Director can already read.
        let universe = self.desk.frame(now);
        let (look, hold, blackout) = (self.desk.look(), self.desk.held(), self.desk.is_blacked_out());
        self.update(|s| {
            s.look = look;
            s.hold = hold;
            s.blackout = blackout;
        });
        self.shared.lock().unwrap().universe = universe.clone();
        if let Some(Err(error)) = self.output.as_mut().map(|output| output.send(&universe)) {
            self.output = None;
            self.output_retry = Some(at + RETRY_AFTER);
            self.update(|s| s.output = OutputStatus::Lost { error });
        }
    }

    /// Changes the status, and tells the app when it changed.
    fn update(&self, change: impl FnOnce(&mut LightsStatus)) {
        let changed = {
            let mut shared = self.shared.lock().unwrap();
            let before = shared.status.clone();
            change(&mut shared.status);
            (shared.status != before).then(|| shared.status.clone())
        };
        if let Some(status) = changed {
            (self.notify)(&status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{Channel, Fixture, MessageSetup, MidiTarget, OscTarget};
    use std::sync::mpsc::SyncSender;

    /// An output whose frames a test reads, which fails while told to.
    struct Fake {
        frames: SyncSender<u8>,
        fail: Arc<Mutex<bool>>,
    }

    impl DmxOutput for Fake {
        fn name(&self) -> String {
            "fake".into()
        }

        fn frame_every(&self) -> Duration {
            Duration::from_millis(5)
        }

        fn send(&mut self, universe: &Universe) -> Result<(), OutputError> {
            if *self.fail.lock().unwrap() {
                return Err(OutputError::SendFailed { detail: "unplugged".into() });
            }
            let _ = self.frames.try_send(universe.get(1));
            Ok(())
        }
    }

    struct Sent(Arc<Mutex<Vec<Signal>>>);

    impl SignalOut for Sent {
        fn send(&mut self, signal: &Signal) -> Result<(), OutputError> {
            self.0.lock().unwrap().push(*signal);
            Ok(())
        }
    }

    fn setup() -> LightsSetup {
        LightsSetup {
            output: OutputChoice::ArtNet { host: "fake".into(), universe: 0 },
            fixtures: vec![Fixture { id: 1, name: "Dimmer".into(), address: 1, channels: vec![Channel::Dimmer], labels: Vec::new() }],
            messages: MessageSetup { osc: Some(OscTarget { host: "127.0.0.1".into(), port: 9 }), midi: None },
            ..LightsSetup::default()
        }
    }

    /// Waits for a frame whose channel 1 is `value`.
    fn wait_for(frames: &Receiver<u8>, value: u8) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if frames.recv_timeout(Duration::from_millis(100)) == Ok(value) {
                return;
            }
        }
        panic!("no frame with {value}");
    }

    fn wait_until(done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn frames_follow_the_stage_and_a_lost_output_is_opened_again() {
        let (frames_in, frames) = mpsc::sync_channel(1000);
        let fail = Arc::new(Mutex::new(false));
        let opened = Arc::new(Mutex::new(0));
        let sent = Arc::new(Mutex::new(Vec::new()));
        let openers = Openers {
            output: Box::new({
                let (fail, opened) = (fail.clone(), opened.clone());
                move |_| {
                    *opened.lock().unwrap() += 1;
                    Ok(Some(Box::new(Fake { frames: frames_in.clone(), fail: fail.clone() }) as Box<dyn DmxOutput>))
                }
            }),
            signals: Box::new({
                let sent = sent.clone();
                move |_| Ok(Box::new(Sent(sent.clone())) as Box<dyn SignalOut>)
            }),
        };
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let lights = Lights::start(setup(), || 0, openers, {
            let statuses = statuses.clone();
            move |status: &LightsStatus| statuses.lock().unwrap().push(status.output.clone())
        });
        let mut no_fades = setup();
        for look in &mut no_fades.looks {
            look.fade_ms = 0;
        }
        lights.send(Control::Setup(no_fades));
        // The default setup lights a question at 70 %.
        lights.send(Control::Stage(Stage::QuestionReady));
        wait_for(&frames, 179);
        lights.send(Control::Blackout(true));
        wait_for(&frames, 0);
        assert!(lights.status().blackout);
        lights.send(Control::Signal(Signal::Slide { number: 2 }));

        *fail.lock().unwrap() = true;
        wait_until(|| matches!(lights.status().output, OutputStatus::Lost { .. }));
        *fail.lock().unwrap() = false;
        lights.send(Control::Blackout(false));
        wait_for(&frames, 179);
        assert_eq!(*opened.lock().unwrap(), 2);
        assert_eq!(lights.universe().get(1), 179);
        assert_eq!(*sent.lock().unwrap(), [Signal::Slide { number: 2 }]);
        let mut statuses = statuses.lock().unwrap().clone();
        statuses.dedup();
        assert!(matches!(statuses[..], [OutputStatus::Connected { .. }, OutputStatus::Lost { .. }, OutputStatus::Connected { .. }]));
    }

    #[test]
    fn a_message_target_that_fails_leaves_the_other_working() {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let sent = Arc::new(Mutex::new(Vec::new()));
        let openers = Openers {
            output: Box::new(|_| Ok(None)),
            signals: Box::new({
                let (opened, sent) = (opened.clone(), sent.clone());
                move |target| {
                    opened.lock().unwrap().push(target.clone());
                    match target {
                        Target::Osc(_) => Err(OutputError::SendFailed { detail: "no network".into() }),
                        Target::Midi(_) => Ok(Box::new(Sent(sent.clone())) as Box<dyn SignalOut>),
                    }
                }
            }),
        };
        let messages = MessageSetup {
            osc: Some(OscTarget { host: "127.0.0.1".into(), port: 9 }),
            midi: Some(MidiTarget { port: "Loop".into(), channel: 1 }),
        };
        let lights = Lights::start(LightsSetup { messages, ..LightsSetup::default() }, || 0, openers, |_: &LightsStatus| {});
        lights.send(Control::Signal(Signal::ShowStarted));
        lights.send(Control::Signal(Signal::Slide { number: 1 }));
        wait_until(|| sent.lock().unwrap().len() == 2);
        assert!(matches!(lights.status().messages, Some(OutputError::SendFailed { .. })));
        // MIDI was opened once, and OSC is not tried again before its retry time.
        let opened = opened.lock().unwrap();
        assert_eq!(opened.iter().filter(|t| matches!(t, Target::Midi(_))).count(), 1);
        assert_eq!(opened.iter().filter(|t| matches!(t, Target::Osc(_))).count(), 1);
    }
}
