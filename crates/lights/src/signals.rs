//! The show's events for outside programs: a lighting program, a DJ
//! program or a sound desk. Each goes out as an OSC message over UDP
//! (OSC 1.0, <https://opensoundcontrol.stanford.edu/spec-1_0.html>) and as
//! a MIDI message, wherever the setup says. The addresses and notes are
//! listed in `docs/design.md` section 7.5.

use crate::dmx::OutputError;
use crate::setup::{MessageSetup, MidiTarget, OscTarget};
use quiz_core::{Cue, Stage};
use serde::Serialize;
use std::net::{SocketAddr, UdpSocket};
use ts_rs::TS;

/// Something that happened in the show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Signal {
    ShowStarted,
    ShowEnded,
    /// The show went on to another stage.
    Stage { stage: Stage },
    Cue { cue: Cue },
    /// Another slide is on screen, numbered from 1.
    Slide { number: u32 },
    /// A player buzzed in.
    Buzz { buzzer: u16 },
}

/// What the lights know of the show at a moment, to tell what changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowMoment {
    pub stage: Stage,
    /// The slide on screen, numbered from 1; none in the lobby.
    pub slide: Option<u32>,
    /// The player buzzed in and being judged.
    pub buzzer: Option<u16>,
}

impl ShowMoment {
    pub const NO_SHOW: ShowMoment = ShowMoment { stage: Stage::NoShow, slide: None, buzzer: None };

    /// The events of going from `before` to `self`, in the order they happen.
    pub fn signals_since(&self, before: &ShowMoment) -> Vec<Signal> {
        let mut signals = Vec::new();
        let (was_on, is_on) = (before.stage != Stage::NoShow, self.stage != Stage::NoShow);
        if is_on && !was_on {
            signals.push(Signal::ShowStarted);
        }
        if let Some(number) = self.slide.filter(|_| self.slide != before.slide) {
            signals.push(Signal::Slide { number });
        }
        if self.stage != before.stage {
            signals.push(Signal::Stage { stage: self.stage });
        }
        if let Some(buzzer) = self.buzzer.filter(|_| self.buzzer != before.buzzer) {
            signals.push(Signal::Buzz { buzzer });
        }
        if was_on && !is_on {
            signals.push(Signal::ShowEnded);
        }
        signals
    }
}

/// The first MIDI note of the stages, in the order of `Stage::ALL`.
pub const FIRST_STAGE_NOTE: u8 = 36;
/// The first MIDI note of the cues, in the order of `Cue::ALL`.
pub const FIRST_CUE_NOTE: u8 = 60;
pub const SHOW_STARTED_NOTE: u8 = 70;
pub const SHOW_ENDED_NOTE: u8 = 71;
pub const BUZZ_NOTE: u8 = 72;

/// A stage's or cue's name in OSC addresses: as in the setup file.
fn name<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => String::new(),
    }
}

/// An OSC argument.
#[derive(Debug, Clone, PartialEq)]
pub enum OscArg {
    Int(i32),
    Float(f32),
}

impl Signal {
    /// Its OSC address and argument. Every event has an address of its own,
    /// so a program can map each to a button without reading arguments; a
    /// trigger's argument is 1.0, as many programs take a press.
    pub fn osc(&self) -> (String, OscArg) {
        match *self {
            Signal::ShowStarted => ("/buzzdome/show/start".into(), OscArg::Float(1.0)),
            Signal::ShowEnded => ("/buzzdome/show/end".into(), OscArg::Float(1.0)),
            Signal::Stage { stage } => (format!("/buzzdome/stage/{}", name(&stage)), OscArg::Float(1.0)),
            Signal::Cue { cue } => (format!("/buzzdome/cue/{}", name(&cue)), OscArg::Float(1.0)),
            Signal::Slide { number } => ("/buzzdome/slide".into(), OscArg::Int(i32::try_from(number).unwrap_or(i32::MAX))),
            Signal::Buzz { buzzer } => ("/buzzdome/buzz".into(), OscArg::Int(i32::from(buzzer))),
        }
    }

    /// Its MIDI messages on `channel` (1 to 16): a note on and off, or a
    /// program change with the slide's number (up to 128).
    pub fn midi(&self, channel: u8) -> Vec<[u8; 3]> {
        let channel = channel.clamp(1, 16) - 1;
        let note = |note: u8| vec![[0x90 | channel, note, 127], [0x80 | channel, note, 0]];
        let index = |i: Option<usize>| i.and_then(|i| u8::try_from(i).ok()).unwrap_or(0);
        match *self {
            Signal::ShowStarted => note(SHOW_STARTED_NOTE),
            Signal::ShowEnded => note(SHOW_ENDED_NOTE),
            Signal::Stage { stage } => note(FIRST_STAGE_NOTE + index(Stage::ALL.iter().position(|&s| s == stage))),
            Signal::Cue { cue } => note(FIRST_CUE_NOTE + index(Cue::ALL.iter().position(|&c| c == cue))),
            Signal::Buzz { .. } => note(BUZZ_NOTE),
            Signal::Slide { number } => match u8::try_from(number) {
                Ok(n @ 1..=128) => vec![[0xC0 | channel, n - 1, 0]],
                _ => Vec::new(),
            },
        }
    }
}

/// `text` as an OSC string: its bytes, a zero, and zeros up to a multiple of 4.
fn osc_string(packet: &mut Vec<u8>, text: &str) {
    packet.extend(text.as_bytes());
    packet.push(0);
    while !packet.len().is_multiple_of(4) {
        packet.push(0);
    }
}

/// An OSC message with one argument.
pub fn osc_message(address: &str, arg: &OscArg) -> Vec<u8> {
    let mut packet = Vec::new();
    osc_string(&mut packet, address);
    match arg {
        OscArg::Int(n) => {
            osc_string(&mut packet, ",i");
            packet.extend(n.to_be_bytes());
        }
        OscArg::Float(x) => {
            osc_string(&mut packet, ",f");
            packet.extend(x.to_be_bytes());
        }
    }
    packet
}

/// Where the show's events go, opened from the setup.
pub trait SignalOut: Send {
    fn send(&mut self, signal: &Signal) -> Result<(), OutputError>;
}

/// OSC over UDP.
pub struct OscOut {
    socket: UdpSocket,
    to: SocketAddr,
}

impl OscOut {
    pub fn open(target: &OscTarget) -> Result<OscOut, OutputError> {
        let (socket, to) = crate::dmx::udp_to(&target.host, target.port)?;
        Ok(OscOut { socket, to })
    }
}

impl SignalOut for OscOut {
    fn send(&mut self, signal: &Signal) -> Result<(), OutputError> {
        let (address, arg) = signal.osc();
        crate::dmx::send_to(&self.socket, &osc_message(&address, &arg), self.to)
    }
}

/// A MIDI output port.
#[cfg(feature = "devices")]
pub struct MidiOut {
    connection: midir::MidiOutputConnection,
    channel: u8,
}

#[cfg(feature = "devices")]
impl MidiOut {
    pub fn open(target: &MidiTarget) -> Result<MidiOut, OutputError> {
        let not_available = |e: &dyn std::fmt::Display| OutputError::NotAvailable { detail: e.to_string() };
        let output = midir::MidiOutput::new("Buzzdome").map_err(|e| not_available(&e))?;
        let port = output
            .ports()
            .into_iter()
            .find(|port| output.port_name(port).is_ok_and(|name| name == target.port))
            .ok_or_else(|| not_available(&target.port))?;
        let connection = output.connect(&port, "Buzzdome").map_err(|e| not_available(&e))?;
        Ok(MidiOut { connection, channel: target.channel })
    }
}

#[cfg(feature = "devices")]
impl SignalOut for MidiOut {
    fn send(&mut self, signal: &Signal) -> Result<(), OutputError> {
        for message in signal.midi(self.channel) {
            self.connection.send(&message).map_err(|e| OutputError::SendFailed { detail: e.to_string() })?;
        }
        Ok(())
    }
}

/// The MIDI outputs of this computer, by name.
pub fn midi_ports() -> Vec<String> {
    #[cfg(feature = "devices")]
    if let Ok(output) = midir::MidiOutput::new("Buzzdome") {
        return output.ports().iter().filter_map(|port| output.port_name(port).ok()).collect();
    }
    Vec::new()
}

/// Opens where `setup` sends events; each output that cannot open is
/// left out and its error given.
pub fn open(setup: &MessageSetup) -> (Vec<Box<dyn SignalOut>>, Option<OutputError>) {
    let mut outs: Vec<Box<dyn SignalOut>> = Vec::new();
    let mut error = None;
    if let Some(osc) = &setup.osc {
        match OscOut::open(osc) {
            Ok(out) => outs.push(Box::new(out)),
            Err(e) => error = Some(e),
        }
    }
    if let Some(midi) = &setup.midi {
        match open_midi(midi) {
            Ok(out) => outs.push(out),
            Err(e) => error = Some(e),
        }
    }
    (outs, error)
}

fn open_midi(target: &MidiTarget) -> Result<Box<dyn SignalOut>, OutputError> {
    #[cfg(feature = "devices")]
    return Ok(Box::new(MidiOut::open(target)?));
    #[cfg(not(feature = "devices"))]
    {
        let _ = target;
        Err(OutputError::NotSupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_moment_tells_what_happened_in_order() {
        let lobby = ShowMoment { stage: Stage::Lobby, slide: None, buzzer: None };
        assert_eq!(lobby.signals_since(&ShowMoment::NO_SHOW), [Signal::ShowStarted, Signal::Stage { stage: Stage::Lobby }]);
        let question = ShowMoment { stage: Stage::QuestionReady, slide: Some(1), buzzer: None };
        assert_eq!(question.signals_since(&lobby), [Signal::Slide { number: 1 }, Signal::Stage { stage: Stage::QuestionReady }]);
        let buzzed = ShowMoment { stage: Stage::Buzzed, slide: Some(1), buzzer: Some(4) };
        assert_eq!(buzzed.signals_since(&question), [Signal::Stage { stage: Stage::Buzzed }, Signal::Buzz { buzzer: 4 }]);
        assert!(buzzed.signals_since(&buzzed).is_empty());
        assert_eq!(ShowMoment::NO_SHOW.signals_since(&buzzed), [Signal::Stage { stage: Stage::NoShow }, Signal::ShowEnded]);
    }

    #[test]
    fn every_event_has_an_osc_address_of_its_own() {
        assert_eq!(Signal::Stage { stage: Stage::LastSeconds }.osc(), ("/buzzdome/stage/lastSeconds".into(), OscArg::Float(1.0)));
        assert_eq!(Signal::Cue { cue: Cue::TimeUp }.osc().0, "/buzzdome/cue/timeUp");
        assert_eq!(Signal::Slide { number: 3 }.osc(), ("/buzzdome/slide".into(), OscArg::Int(3)));
        let mut addresses: Vec<String> = Stage::ALL.iter().map(|&stage| Signal::Stage { stage }.osc().0).collect();
        addresses.extend(Cue::ALL.iter().map(|&cue| Signal::Cue { cue }.osc().0));
        let count = addresses.len();
        addresses.sort();
        addresses.dedup();
        assert_eq!(addresses.len(), count);
    }

    #[test]
    fn an_osc_message_pads_its_strings_to_four_bytes() {
        assert_eq!(osc_message("/ab", &OscArg::Int(258)), b"/ab\0,i\0\0\0\0\x01\x02");
        assert_eq!(osc_message("/abcd", &OscArg::Float(1.0)), b"/abcd\0\0\0,f\0\0\x3f\x80\0\0");
    }

    #[test]
    fn stages_and_cues_play_notes_of_their_own_and_a_slide_its_program() {
        assert_eq!(Signal::Stage { stage: Stage::NoShow }.midi(1), [[0x90, 36, 127], [0x80, 36, 0]]);
        assert_eq!(Signal::Stage { stage: Stage::Winner }.midi(1)[0][1], 36 + 13);
        assert_eq!(Signal::Cue { cue: Cue::Reveal }.midi(16), [[0x9F, 65, 127], [0x8F, 65, 0]]);
        assert!(FIRST_STAGE_NOTE as usize + Stage::ALL.len() <= FIRST_CUE_NOTE as usize);
        assert!(FIRST_CUE_NOTE as usize + Cue::ALL.len() <= SHOW_STARTED_NOTE as usize);
        assert_eq!(Signal::Slide { number: 1 }.midi(2), [[0xC1, 0, 0]]);
        assert!(Signal::Slide { number: 129 }.midi(2).is_empty());
    }

    #[test]
    fn events_reach_an_osc_program() {
        let program = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = program.local_addr().unwrap().port();
        let (mut outs, error) = open(&MessageSetup { osc: Some(OscTarget { host: "127.0.0.1".into(), port }), midi: None });
        assert!(error.is_none());
        outs[0].send(&Signal::Buzz { buzzer: 7 }).unwrap();
        let mut buffer = [0; 64];
        let (len, _) = program.recv_from(&mut buffer).unwrap();
        assert_eq!(&buffer[..len], osc_message("/buzzdome/buzz", &OscArg::Int(7)).as_slice());
    }
}
