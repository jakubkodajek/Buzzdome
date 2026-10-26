//! What a quiz's show sounds like (spec SN-1 to SN-4): what each of the
//! show's moments plays, from the app's own library, from the quiz's own
//! files, or nothing, and how loud its effects and its music are. The
//! Director plays them; the game says when, by its cues (`Cue`) and by the
//! music its moment takes (`Game::music`).

use crate::game::{Game, Phase, QuestionStep, WINNER_STEP};
use crate::media::{check_reference, MediaIssue, MediaKind};
use crate::settings::count_up_to;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Loudest volume of the effects or the music, in percent of their files.
pub const MAX_SOUND_VOLUME: u8 = 100;
pub const DEFAULT_EFFECTS_VOLUME: u8 = 100;
/// Music plays under the quizmaster's voice, so softer than the effects.
pub const DEFAULT_MUSIC_VOLUME: u8 = 50;
/// Longest id of one of the app's own sounds, in bytes.
pub const MAX_SOUND_ID_LEN: usize = 64;

/// A moment of the show that has a sound of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SoundSlot {
    // Effects, played once when the moment comes.
    /// A question's clock starts.
    ClockStart,
    /// A running question's last seconds (`LAST_SECONDS_MS`), so that it
    /// ends as the time is up; it waits while the clock is paused.
    Countdown,
    /// A player buzzes in.
    Buzz,
    Correct,
    Wrong,
    TimeUp,
    /// A question's right answer is shown.
    Reveal,
    /// The leaderboard opens.
    Leaderboard,
    /// The final ranking shows a place before the winner's.
    Place,
    /// The final ranking shows the winner.
    Winner,
    // Music, played over and over while the show is at its moment.
    /// Before the first slide, while the buzzers register.
    LobbyMusic,
    /// Between questions: on billboards, explanations, the ends of rounds
    /// and the leaderboard, and once a question's answer is shown.
    BetweenMusic,
    /// While a question's clock runs.
    ClockMusic,
    /// While the final ranking builds up to the winner.
    BuildUpMusic,
    /// Once the final ranking shows the winner.
    WinnerMusic,
}

impl SoundSlot {
    pub const ALL: [SoundSlot; 15] = [
        SoundSlot::ClockStart,
        SoundSlot::Countdown,
        SoundSlot::Buzz,
        SoundSlot::Correct,
        SoundSlot::Wrong,
        SoundSlot::TimeUp,
        SoundSlot::Reveal,
        SoundSlot::Leaderboard,
        SoundSlot::Place,
        SoundSlot::Winner,
        SoundSlot::LobbyMusic,
        SoundSlot::BetweenMusic,
        SoundSlot::ClockMusic,
        SoundSlot::BuildUpMusic,
        SoundSlot::WinnerMusic,
    ];

    /// Music plays over and over and fades in and out; an effect plays once.
    pub fn is_music(self) -> bool {
        matches!(self, SoundSlot::LobbyMusic | SoundSlot::BetweenMusic | SoundSlot::ClockMusic | SoundSlot::BuildUpMusic | SoundSlot::WinnerMusic)
    }
}

/// What a moment of the show plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SoundChoice {
    /// Nothing.
    None,
    /// One of the app's own sounds, by its id (`is_sound_id`).
    BuiltIn(String),
    /// A sound file of the quiz, `media/<name>`.
    File(String),
}

/// True when `id` can name one of the app's own sounds: lower-case
/// letters, digits, `-`, `_` and `.`, up to `MAX_SOUND_ID_LEN` bytes. An id
/// the app does not have plays the moment's default, as a theme it does
/// not have shows the default look.
pub fn is_sound_id(id: &str) -> bool {
    let allowed = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_' || b == b'.';
    !id.is_empty() && id.len() <= MAX_SOUND_ID_LEN && id.bytes().all(allowed)
}

/// What a quiz's show sounds like. Every moment left out plays the app's
/// default for it, as do the volumes, so a quiz without it sounds as the
/// app does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct QuizSounds {
    /// What a moment plays, for the moments that do not play the default.
    /// A moment or a choice a build cannot read is left out.
    #[serde(with = "chosen")]
    #[schemars(with = "BTreeMap<SoundSlot, SoundChoice>")]
    #[ts(as = "BTreeMap<SoundSlot, SoundChoice>")]
    pub slots: BTreeMap<SoundSlot, SoundChoice>,
    /// Up to `MAX_SOUND_VOLUME` percent.
    #[serde(with = "effects_volume")]
    #[schemars(with = "u8")]
    #[ts(as = "u8")]
    pub effects_volume: u8,
    /// Up to `MAX_SOUND_VOLUME` percent.
    #[serde(with = "music_volume")]
    #[schemars(with = "u8")]
    #[ts(as = "u8")]
    pub music_volume: u8,
}

impl Default for QuizSounds {
    fn default() -> Self {
        QuizSounds { slots: BTreeMap::new(), effects_volume: DEFAULT_EFFECTS_VOLUME, music_volume: DEFAULT_MUSIC_VOLUME }
    }
}

impl QuizSounds {
    pub fn is_default(&self) -> bool {
        *self == QuizSounds::default()
    }

    /// The quiz's sound files the moments play, with their moment.
    pub fn files(&self) -> impl Iterator<Item = (SoundSlot, &str)> {
        self.slots.iter().filter_map(|(&slot, choice)| match choice {
            SoundChoice::File(file) => Some((slot, file.as_str())),
            SoundChoice::None | SoundChoice::BuiltIn(_) => None,
        })
    }

    /// Points the files in `renamed` (old reference to new one) to their new names.
    pub fn rename_media(&mut self, renamed: &BTreeMap<String, String>) {
        for choice in self.slots.values_mut() {
            if let SoundChoice::File(file) = choice {
                if let Some(new) = renamed.get(file.as_str()) {
                    file.clone_from(new);
                }
            }
        }
    }

    /// Each file that is not a sound in the quiz's media folder, with its moment.
    pub fn problems(&self) -> impl Iterator<Item = (SoundSlot, &str, MediaIssue)> {
        self.files().filter_map(|(slot, file)| check_reference(file, &[MediaKind::Audio]).err().map(|issue| (slot, file, issue)))
    }
}

/// Reads the chosen moments one by one, leaving out a moment a build does
/// not know (a newer one) and a choice it cannot read, so the rest stays.
mod chosen {
    use super::{is_sound_id, SoundChoice, SoundSlot};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(slots: &BTreeMap<SoundSlot, SoundChoice>, serializer: S) -> Result<S::Ok, S::Error> {
        slots.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<SoundSlot, SoundChoice>, D::Error> {
        let Some(serde_json::Value::Object(written)) = Option::<serde_json::Value>::deserialize(deserializer)? else {
            return Ok(BTreeMap::new());
        };
        let read = written.into_iter().filter_map(|(slot, choice)| {
            let slot = serde_json::from_value::<SoundSlot>(serde_json::Value::String(slot)).ok()?;
            let choice = serde_json::from_value::<SoundChoice>(choice).ok()?;
            let readable = match &choice {
                SoundChoice::BuiltIn(id) => is_sound_id(id),
                SoundChoice::None | SoundChoice::File(_) => true,
            };
            readable.then_some((slot, choice))
        });
        Ok(read.collect())
    }
}

count_up_to!(effects_volume, super::DEFAULT_EFFECTS_VOLUME, super::MAX_SOUND_VOLUME);
count_up_to!(music_volume, super::DEFAULT_MUSIC_VOLUME, super::MAX_SOUND_VOLUME);

impl Game {
    /// The music the show's moment takes, if any. It stops while the clock
    /// is paused, while a buzz is judged and while a question's answers are
    /// in but not shown, so that those are heard in silence.
    pub fn music(&self) -> Option<SoundSlot> {
        if self.leaderboard_visible() {
            return Some(SoundSlot::BetweenMusic);
        }
        match self.phase() {
            Phase::Lobby => Some(SoundSlot::LobbyMusic),
            Phase::Billboard | Phase::Explanation | Phase::EndOfRound => Some(SoundSlot::BetweenMusic),
            Phase::Question(QuestionStep::Running { clock }) if clock.is_running() => Some(SoundSlot::ClockMusic),
            Phase::Question(QuestionStep::Revealed { .. }) => Some(SoundSlot::BetweenMusic),
            Phase::Question(_) => None,
            Phase::Final { step } if *step >= WINNER_STEP => Some(SoundSlot::WinnerMusic),
            Phase::Final { .. } => Some(SoundSlot::BuildUpMusic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Cue;

    fn read(json: &str) -> QuizSounds {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn sounds_left_out_play_the_defaults() {
        assert_eq!(read("{}"), QuizSounds::default());
        assert!(QuizSounds::default().is_default());
        assert_eq!(serde_json::to_string(&QuizSounds::default()).unwrap(), r#"{"slots":{},"effectsVolume":100,"musicVolume":50}"#);
    }

    #[test]
    fn each_choice_reads_back_the_same() {
        let sounds = QuizSounds {
            slots: BTreeMap::from([
                (SoundSlot::Buzz, SoundChoice::BuiltIn("buzz.horn".into())),
                (SoundSlot::Wrong, SoundChoice::None),
                (SoundSlot::LobbyMusic, SoundChoice::File("media/intro.mp3".into())),
            ]),
            effects_volume: 80,
            music_volume: 30,
        };
        let json = serde_json::to_string(&sounds).unwrap();
        assert_eq!(json, r#"{"slots":{"buzz":{"builtIn":"buzz.horn"},"wrong":"none","lobbyMusic":{"file":"media/intro.mp3"}},"effectsVolume":80,"musicVolume":30}"#);
        assert_eq!(read(&json), sounds);
        assert_eq!(sounds.files().collect::<Vec<_>>(), [(SoundSlot::LobbyMusic, "media/intro.mp3")]);
    }

    #[test]
    fn what_a_build_cannot_read_is_left_out_and_the_rest_stays() {
        let sounds = read(
            r#"{ "slots": { "buzz": "none", "applause": "none", "correct": { "builtIn": "Ding Dong!" }, "wrong": { "beep": 1 },
                 "timeUp": { "builtIn": "time-up.gong" } }, "effectsVolume": "loud", "musicVolume": 250 }"#,
        );
        let kept = BTreeMap::from([(SoundSlot::Buzz, SoundChoice::None), (SoundSlot::TimeUp, SoundChoice::BuiltIn("time-up.gong".into()))]);
        assert_eq!(sounds, QuizSounds { slots: kept, effects_volume: DEFAULT_EFFECTS_VOLUME, music_volume: MAX_SOUND_VOLUME });
        assert_eq!(read(r#"{ "slots": [1, 2] }"#), QuizSounds::default());
        assert!(!is_sound_id(&"a".repeat(MAX_SOUND_ID_LEN + 1)) && is_sound_id(&"a".repeat(MAX_SOUND_ID_LEN)));
    }

    #[test]
    fn a_file_that_is_not_a_sound_in_the_media_folder_is_a_problem() {
        let mut sounds = QuizSounds::default();
        sounds.slots.insert(SoundSlot::Correct, SoundChoice::File("media/yes.wav".into()));
        sounds.slots.insert(SoundSlot::Wrong, SoundChoice::File("media/no.png".into()));
        sounds.slots.insert(SoundSlot::ClockMusic, SoundChoice::File("../music.mp3".into()));
        let problems: Vec<_> = sounds.problems().map(|(slot, file, _)| (slot, file)).collect();
        assert_eq!(problems, [(SoundSlot::Wrong, "media/no.png"), (SoundSlot::ClockMusic, "../music.mp3")]);
        sounds.rename_media(&BTreeMap::from([("media/yes.wav".to_string(), "media/yes_.wav".to_string())]));
        assert_eq!(sounds.slots[&SoundSlot::Correct], SoundChoice::File("media/yes_.wav".into()));
    }

    #[test]
    fn a_quiz_keeps_its_sounds_and_their_files_are_its_media() {
        use crate::model::{Problem, Quiz, ValidationError};
        let mut quiz = Quiz::new("Q", vec![]);
        assert!(!serde_json::to_string(&quiz).unwrap().contains("sounds"));
        quiz.sounds.slots.insert(SoundSlot::LobbyMusic, SoundChoice::File("media/intro.mp3".into()));
        quiz.sounds.slots.insert(SoundSlot::Buzz, SoundChoice::File("media/buzz.png".into()));
        assert_eq!(Quiz::from_json(&serde_json::to_string(&quiz).unwrap()).unwrap().sounds, quiz.sounds);
        assert_eq!(quiz.media_files().into_iter().collect::<Vec<_>>(), ["media/buzz.png", "media/intro.mp3"]);
        let problems = quiz.validate().unwrap_err();
        assert!(matches!(&problems[..], [ValidationError { slide_id: None, problem: Problem::SoundMedia { slot: SoundSlot::Buzz, .. } }]));
        let unread = r#"{ "schemaVersion": 5, "title": "Q", "theme": null, "slides": [], "sounds": "loud" }"#;
        assert_eq!(Quiz::from_json(unread).unwrap().sounds, QuizSounds::default());
    }

    #[test]
    fn each_cue_plays_the_effect_slot_of_its_name() {
        // The Director plays a cue's sound from the slot named as the cue is.
        for cue in Cue::ALL {
            let slot: SoundSlot = serde_json::from_value(serde_json::to_value(cue).unwrap()).unwrap();
            assert!(!slot.is_music(), "{slot:?}");
        }
        let effects = SoundSlot::ALL.iter().filter(|slot| !slot.is_music()).count();
        assert_eq!(effects, Cue::ALL.len() + 1, "the countdown is the one effect the clock plays, not a cue");
    }
}

