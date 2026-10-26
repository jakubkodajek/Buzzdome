//! The lighting desk: which look shows now and the fades between looks.
//! The quizmaster's blackout wins over all, then a look they hold, then a
//! cue's flash, then the look of the show's stage. Time comes in with
//! every call, so a desk plays the same in a test as in a show.

use crate::dmx::Universe;
use crate::look::{dark, draw};
use crate::setup::{LightsSetup, LookId};
use quiz_core::{Cue, Stage};

/// What the lights show, apart from how far a fade is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Dark,
    Look(LookId),
}

#[derive(Debug, Clone)]
struct Fade {
    from: Universe,
    start_ms: u64,
    ms: u32,
}

#[derive(Debug, Clone)]
pub struct Desk {
    setup: LightsSetup,
    stage: Stage,
    /// The look of the stage, or of the last stage that had one.
    stage_look: Option<LookId>,
    /// A cue's look and when it ends.
    flash: Option<(LookId, u64)>,
    hold: Option<LookId>,
    blackout: bool,
    shown: Shown,
    /// When `shown` began, for its effect.
    since_ms: u64,
    fade: Option<Fade>,
    last: Universe,
}

impl Desk {
    /// A desk with no show, dark until its first frame fades to the no-show look.
    pub fn new(setup: LightsSetup) -> Desk {
        let last = dark(&setup);
        let stage_look = setup.stages.get(&Stage::NoShow).copied();
        Desk { setup, stage: Stage::NoShow, stage_look, flash: None, hold: None, blackout: false, shown: Shown::Dark, since_ms: 0, fade: None, last }
    }

    pub fn setup(&self) -> &LightsSetup {
        &self.setup
    }

    /// Takes a new setup. The stage takes its look from it, and a look held
    /// or flashed that is gone ends.
    pub fn set_setup(&mut self, setup: LightsSetup) {
        let exists = |id: &LookId| setup.look(*id).is_some();
        self.stage_look = setup.stages.get(&self.stage).copied().or(self.stage_look.filter(exists));
        self.hold = self.hold.filter(exists);
        self.flash = self.flash.filter(|(id, _)| exists(id));
        self.setup = setup;
        // A look changed in place shows as changed from the next frame.
    }

    pub fn set_stage(&mut self, stage: Stage) {
        if stage != self.stage {
            self.stage = stage;
            if let Some(&look) = self.setup.stages.get(&stage) {
                self.stage_look = Some(look);
            }
        }
    }

    /// Flashes the look of `cue`, if it has one, from `now`.
    pub fn cue(&mut self, cue: Cue, now: u64) {
        if let Some(flash) = self.setup.flashes.get(&cue) {
            self.flash = Some((flash.look, now.saturating_add(u64::from(flash.ms))));
        }
    }

    /// Holds a look whatever the show does, or lets go of it.
    pub fn hold(&mut self, look: Option<LookId>) {
        self.hold = look.filter(|id| self.setup.look(*id).is_some());
    }

    pub fn held(&self) -> Option<LookId> {
        self.hold
    }

    pub fn blackout(&mut self, on: bool) {
        self.blackout = on;
    }

    pub fn is_blacked_out(&self) -> bool {
        self.blackout
    }

    /// The look showing, if any.
    pub fn look(&self) -> Option<LookId> {
        match self.shown {
            Shown::Look(id) => Some(id),
            Shown::Dark => None,
        }
    }

    /// What the lights should show at `now`.
    pub fn frame(&mut self, now: u64) -> Universe {
        if self.flash.is_some_and(|(_, until)| now >= until) {
            self.flash = None;
        }
        let flashing = self.flash.map(|(id, _)| id);
        let wanted = match () {
            _ if self.blackout => Shown::Dark,
            _ if self.hold.is_some() => self.hold.map_or(Shown::Dark, Shown::Look),
            _ if flashing.is_some() => flashing.map_or(Shown::Dark, Shown::Look),
            _ => self.stage_look.map_or(Shown::Dark, Shown::Look),
        };
        if wanted != self.shown {
            // A flash and a blackout come at once; any other look in its own time.
            let instant = self.blackout || (flashing.is_some() && self.hold.is_none());
            let ms = match wanted {
                Shown::Look(id) if !instant => self.setup.look(id).map_or(0, |look| look.fade_ms),
                _ => 0,
            };
            self.fade = Some(Fade { from: self.last.clone(), start_ms: now, ms });
            self.shown = wanted;
            self.since_ms = now;
        }
        let target = match self.shown {
            Shown::Look(id) => match self.setup.look(id) {
                Some(look) => draw(&self.setup, look, now.saturating_sub(self.since_ms), self.setup.master),
                None => dark(&self.setup),
            },
            Shown::Dark => dark(&self.setup),
        };
        let frame = match &self.fade {
            Some(fade) if now < fade.start_ms + u64::from(fade.ms) => {
                fade.from.blend(&target, (now - fade.start_ms) as f32 / fade.ms as f32)
            }
            _ => {
                self.fade = None;
                target
            }
        };
        self.last = frame.clone();
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{Channel, Fixture, Flash, Look, LookEffect};
    use quiz_core::Color;
    use std::collections::BTreeMap;

    fn look(id: LookId, hex: &str, fade_ms: u32) -> Look {
        Look {
            id,
            name: format!("L{id}"),
            colors: vec![Color::try_from(hex.to_owned()).unwrap()],
            intensity: 100,
            effect: LookEffect::Static,
            step_ms: 1_000,
            fade_ms,
            fixtures: Vec::new(),
            strobe: 0,
            channels: Vec::new(),
        }
    }

    /// One dimmer at channel 1; looks 1 (full, 1 s fade in), 2 (dark red,
    /// no fade) and 3 (flash). The lobby takes look 1, a question look 2.
    fn desk() -> Desk {
        let setup = LightsSetup {
            fixtures: vec![Fixture { id: 1, name: "Dimmer".into(), address: 1, channels: vec![Channel::Dimmer, Channel::Red] }],
            looks: vec![look(1, "#FFFFFF", 1_000), look(2, "#800000", 0), look(3, "#00FF00", 500)],
            stages: BTreeMap::from([(Stage::Lobby, 1), (Stage::QuestionReady, 2)]),
            flashes: BTreeMap::from([(Cue::Correct, Flash { look: 3, ms: 1_000 })]),
            ..LightsSetup::default()
        };
        Desk::new(setup)
    }

    fn red(desk: &mut Desk, now: u64) -> u8 {
        desk.frame(now).get(2)
    }

    #[test]
    fn a_stage_fades_into_its_look_and_a_stage_without_one_keeps_the_look_before() {
        let mut desk = desk();
        assert_eq!(red(&mut desk, 0), 0, "no look for no show");
        desk.set_stage(Stage::Lobby);
        assert_eq!(red(&mut desk, 1_000), 0);
        assert_eq!(red(&mut desk, 1_500), 128);
        assert_eq!(red(&mut desk, 2_000), 255);
        desk.set_stage(Stage::Answering);
        assert_eq!((red(&mut desk, 3_000), desk.look()), (255, Some(1)));
        desk.set_stage(Stage::QuestionReady);
        assert_eq!((red(&mut desk, 3_000), desk.look()), (128, Some(2)));
    }

    #[test]
    fn a_flash_comes_at_once_and_fades_back_into_the_stage_look() {
        let mut desk = desk();
        desk.set_stage(Stage::QuestionReady);
        desk.frame(0);
        desk.cue(Cue::Correct, 100);
        assert_eq!((desk.frame(100).get(2), desk.look()), (0, Some(3)));
        assert_eq!(desk.look(), Some(3));
        // Look 2 has no fade, so the stage's look is back at once when the flash ends.
        assert_eq!((red(&mut desk, 1_100), desk.look()), (128, Some(2)));
        // A cue with no flash changes nothing.
        desk.cue(Cue::Buzz, 1_200);
        assert_eq!(desk.look(), Some(2));
    }

    #[test]
    fn a_blackout_wins_over_a_held_look_which_wins_over_the_show() {
        let mut desk = desk();
        desk.set_stage(Stage::QuestionReady);
        desk.hold(Some(1));
        desk.cue(Cue::Correct, 0);
        desk.frame(0);
        assert_eq!(red(&mut desk, 1_000), 255);
        desk.blackout(true);
        assert_eq!((red(&mut desk, 1_000), desk.look()), (0, None));
        desk.blackout(false);
        desk.hold(None);
        assert_eq!(desk.look(), None);
        assert_eq!((red(&mut desk, 1_000), desk.look()), (128, Some(2)));
        // A look that is not there is not held.
        desk.hold(Some(99));
        assert_eq!(desk.held(), None);
    }

    #[test]
    fn a_new_setup_keeps_the_stage_and_drops_what_is_gone() {
        let mut desk = desk();
        desk.set_stage(Stage::Lobby);
        desk.hold(Some(3));
        let mut setup = desk.setup().clone();
        setup.looks.retain(|look| look.id != 3);
        setup.flashes.clear();
        setup.stages.insert(Stage::Lobby, 2);
        desk.set_setup(setup);
        assert_eq!(desk.held(), None);
        assert_eq!((red(&mut desk, 0), desk.look()), (128, Some(2)));
    }
}
