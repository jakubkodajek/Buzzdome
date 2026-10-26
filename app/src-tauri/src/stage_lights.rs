//! The stage lights and the show's messages to outside programs (`lights`
//! crate, spec section 12): the venue's setup kept in `lights.json`, and
//! what the show tells the lights thread. The show never waits for the
//! lights: it only queues what happened, and nothing of the lights is
//! journaled, as they follow the show's state (`Game::stage`).

use crate::app_file::{AppFile, SettingsLost};
use crate::app_log;
use crate::error::{AppError, CmdResult};
use crate::{now_ms, off_ui};
use lights::dmx::FoundOutput;
use lights::setup::SetupError;
use lights::{Control, Lights, LightsSetup, LightsStatus, LookId, ShowMoment, Universe};
use quiz_core::{Cue, Effect, Game, Phase, QuestionStep};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

/// Where the setup is kept, in the app's data folder.
pub const LIGHTS_FILE: &str = "lights.json";
/// Tells the Director the lights' status (`LightsStatus`).
const STATUS_EVENT: &str = "lights-status";

pub struct StageLights {
    lights: Lights,
    file: AppFile,
    /// The setup as kept, held while a change is kept (`lights_store`).
    setup: Mutex<LightsSetup>,
    /// Why a kept setup was not used: it did not pass its checks.
    refused: Option<SetupError>,
    /// What the lights were last told of the show. A leaf lock: taken
    /// under the show state, and nothing is taken under it.
    told: Mutex<ShowMoment>,
}

impl StageLights {
    /// Starts the lights thread with the setup kept at `path`. One that
    /// cannot be read is kept aside, and one that does not pass its checks
    /// (edited by hand) is not used; the lights start with the default.
    pub fn start(app: &AppHandle, path: PathBuf) -> StageLights {
        let (file, kept) = AppFile::load::<LightsSetup>(path);
        if let Some(lost) = file.lost() {
            app_log::note(&format!("lights setup not read: {} (kept as {:?})", lost.detail, lost.kept_as));
        }
        let refused = kept.check().err();
        let setup = if refused.is_some() { LightsSetup::default() } else { kept };
        let notify = {
            let app = app.clone();
            move |status: &LightsStatus| {
                let _ = app.emit(STATUS_EVENT, status);
            }
        };
        let lights = Lights::start(setup.clone(), now_ms, lights::engine::Openers::default(), notify);
        StageLights { lights, file, setup: Mutex::new(setup), refused, told: Mutex::new(ShowMoment::NO_SHOW) }
    }

    /// Tells the lights where the show is now: its stage, slide and buzz.
    fn follow(&self, game: Option<&Game>, now: u64) {
        let moment = game.map_or(ShowMoment::NO_SHOW, |game| moment(game, now));
        let mut told = self.told.lock().unwrap();
        if moment == *told {
            return;
        }
        if moment.stage != told.stage {
            self.lights.send(Control::Stage(moment.stage));
        }
        for signal in moment.signals_since(&told) {
            self.lights.send(Control::Signal(signal));
        }
        *told = moment;
    }

    fn cue(&self, cue: Cue) {
        self.lights.send(Control::Cue(cue));
        self.lights.send(Control::Signal(lights::Signal::Cue { cue }));
    }
}

/// The show's moment as the lights follow it.
fn moment(game: &Game, now: u64) -> ShowMoment {
    let slide = (game.phase() != &Phase::Lobby).then(|| u32::try_from(game.slide_index() + 1).unwrap_or(u32::MAX));
    let buzzer = match game.phase() {
        Phase::Question(QuestionStep::Buzzed { buzzer, .. }) => Some(*buzzer),
        _ => None,
    };
    ShowMoment { stage: game.stage(now), slide, buzzer }
}

/// Tells the lights a show change's cues, then where the show is now. Runs
/// with every change and every tick of the show thread, so a stage that
/// time changes (the last seconds) is followed too.
pub fn follow(app: &AppHandle, game: Option<&Game>, effects: &[Effect], now: u64) {
    let Some(lights) = app.try_state::<StageLights>() else { return };
    for effect in effects {
        if let Effect::Sound { cue } = effect {
            lights.cue(*cue);
        }
    }
    lights.follow(game, now);
}

/// The lights as the Director's Lights tab shows them.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LightsInfo {
    pub setup: LightsSetup,
    pub status: LightsStatus,
    /// Why the kept setup could not be read, if so.
    pub settings_error: Option<SettingsLost>,
    /// Why the kept setup was not used, if so.
    pub refused: Option<SetupError>,
}

/// The interfaces and MIDI outputs on this computer.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LightsDevices {
    pub outputs: Vec<FoundOutput>,
    pub midi_ports: Vec<String>,
}

#[tauri::command]
pub fn lights_info(state: State<StageLights>) -> LightsInfo {
    LightsInfo {
        setup: state.setup.lock().unwrap().clone(),
        status: state.lights.status(),
        settings_error: state.file.lost().cloned(),
        refused: state.refused.clone(),
    }
}

/// Checks `setup`, keeps it, then gives it to the lights.
#[tauri::command]
pub fn lights_store(state: State<StageLights>, setup: LightsSetup) -> CmdResult<()> {
    setup.check().map_err(|refused| AppError::LightsRefused { refused })?;
    let mut kept = state.setup.lock().unwrap();
    state.file.store(&setup)?;
    *kept = setup.clone();
    state.lights.send(Control::Setup(setup));
    Ok(())
}

/// Lists the interfaces and MIDI outputs; asking the system can take a while.
#[tauri::command]
pub async fn lights_devices() -> CmdResult<LightsDevices> {
    off_ui(|| Ok(LightsDevices { outputs: lights::dmx::find(), midi_ports: lights::signals::midi_ports() })).await
}

/// Holds `look` whatever the show does, or lets go with none.
#[tauri::command]
pub fn lights_hold(state: State<StageLights>, look: Option<LookId>) {
    state.lights.send(Control::Hold(look));
}

#[tauri::command]
pub fn lights_blackout(state: State<StageLights>, on: bool) {
    state.lights.send(Control::Blackout(on));
}

/// The channels the lights show now, for the Director's preview.
#[tauri::command]
pub fn lights_universe(state: State<StageLights>) -> Vec<u8> {
    let universe: Universe = state.lights.universe();
    universe.slots().to_vec()
}

/// Where the setup is kept.
pub fn lights_path(app: &AppHandle) -> CmdResult<PathBuf> {
    crate::app_data(app, LIGHTS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{Button, Command, Quiz, Stage, Stamped};

    #[test]
    fn the_lights_follow_the_slide_the_stage_and_the_buzz() {
        let quiz = Quiz::from_json(crate::DEMO_QUIZ).unwrap();
        let mut game = Game::new(quiz);
        let run = |game: &mut Game, command| game.apply(&Stamped { at_ms: 0, command }).unwrap();
        run(&mut game, Command::Press { buzzer: 3, button: Button::Big });
        assert_eq!(moment(&game, 0), ShowMoment { stage: Stage::Lobby, slide: None, buzzer: None });
        run(&mut game, Command::Next);
        assert_eq!(moment(&game, 0).slide, Some(1));
        // The demo quiz's slide 8 is a fastest-finger question answered aloud.
        run(&mut game, Command::GoTo { slide: 7 });
        run(&mut game, Command::Next);
        run(&mut game, Command::Press { buzzer: 3, button: Button::Big });
        assert_eq!(moment(&game, 0), ShowMoment { stage: Stage::Buzzed, slide: Some(8), buzzer: Some(3) });
    }
}
