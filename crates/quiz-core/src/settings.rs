//! A quiz's advanced settings (spec QZ-11): how its show runs, for the whole
//! quiz. Every default keeps the show as it ran before there were settings,
//! and a value a build cannot read reads as its default, so a quiz always
//! opens and plays.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Seconds the show waits by default before it acts on everyone having answered.
pub const DEFAULT_ALL_ANSWERED_DELAY_S: u8 = 3;
/// Longest wait, in seconds, before the show acts on everyone having answered.
pub const MAX_ALL_ANSWERED_DELAY_S: u8 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct QuizSettings {
    /// What the show does once a question has nobody left to answer.
    #[serde(with = "crate::style::lenient_or_default")]
    #[ts(as = "AllAnswered")]
    pub all_answered: AllAnswered,
    /// Seconds it waits before it does it, up to `MAX_ALL_ANSWERED_DELAY_S`;
    /// the quizmaster can step in meanwhile.
    #[serde(with = "delay_in_range")]
    #[ts(as = "u8")]
    pub all_answered_delay_s: u8,
}

impl Default for QuizSettings {
    fn default() -> Self {
        QuizSettings { all_answered: AllAnswered::Wait, all_answered_delay_s: DEFAULT_ALL_ANSWERED_DELAY_S }
    }
}

impl QuizSettings {
    pub fn is_default(&self) -> bool {
        *self == QuizSettings::default()
    }

    /// See `all_answered_delay_s`.
    pub fn all_answered_delay_ms(&self) -> u64 {
        u64::from(self.all_answered_delay_s.min(MAX_ALL_ANSWERED_DELAY_S)) * 1000
    }
}

/// What the show does once a question has nobody left to answer: every
/// player switched on answered it, or in fastest finger answered it wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AllAnswered {
    /// The closed question waits for the quizmaster's Next.
    #[default]
    Wait,
    /// The right answer is shown, as Next would show it.
    Reveal,
    /// The show goes on to the next slide without showing the right answer.
    NextSlide,
}

/// A wait in whole seconds: one longer than the longest reads as the
/// longest, and one that is not a number of seconds as the default.
mod delay_in_range {
    use super::{DEFAULT_ALL_ANSWERED_DELAY_S, MAX_ALL_ANSWERED_DELAY_S};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(seconds: &u8, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*seconds)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
        let value = Option::<serde_json::Value>::deserialize(deserializer)?;
        let seconds = value.as_ref().and_then(serde_json::Value::as_u64);
        Ok(seconds.map_or(DEFAULT_ALL_ANSWERED_DELAY_S, |s| u8::try_from(s).unwrap_or(u8::MAX).min(MAX_ALL_ANSWERED_DELAY_S)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> QuizSettings {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn settings_left_out_keep_the_show_as_it_was() {
        assert_eq!(read("{}"), QuizSettings::default());
        assert_eq!(QuizSettings::default().all_answered, AllAnswered::Wait);
        assert!(QuizSettings::default().is_default());
    }

    #[test]
    fn a_setting_that_does_not_read_reads_as_its_default() {
        let settings = read(r#"{ "allAnswered": "dance", "allAnsweredDelayS": "soon" }"#);
        assert_eq!(settings, QuizSettings::default());
        assert_eq!(read(r#"{ "allAnsweredDelayS": -1 }"#).all_answered_delay_s, DEFAULT_ALL_ANSWERED_DELAY_S);
        assert_eq!(read(r#"{ "allAnswered": "reveal" }"#).all_answered, AllAnswered::Reveal);
    }

    #[test]
    fn a_wait_past_the_longest_reads_as_the_longest() {
        for seconds in ["11", "255", "100000"] {
            let settings = read(&format!(r#"{{ "allAnsweredDelayS": {seconds} }}"#));
            assert_eq!(settings.all_answered_delay_s, MAX_ALL_ANSWERED_DELAY_S, "{seconds}");
        }
        assert_eq!(read(r#"{ "allAnsweredDelayS": 0 }"#).all_answered_delay_ms(), 0);
    }

    #[test]
    fn a_quiz_keeps_its_settings_and_leaves_default_ones_out() {
        use crate::model::Quiz;
        let mut quiz = Quiz::new("Q", vec![]);
        assert!(!serde_json::to_string(&quiz).unwrap().contains("settings"));
        quiz.settings.all_answered = AllAnswered::Reveal;
        assert_eq!(Quiz::from_json(&serde_json::to_string(&quiz).unwrap()).unwrap().settings, quiz.settings);
        let unread = r#"{ "schemaVersion": 5, "title": "Q", "theme": null, "slides": [], "settings": "fast" }"#;
        assert_eq!(Quiz::from_json(unread).unwrap().settings, QuizSettings::default());
    }

    #[test]
    fn settings_read_back_the_same() {
        let settings = QuizSettings { all_answered: AllAnswered::NextSlide, all_answered_delay_s: 7 };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(json, r#"{"allAnswered":"nextSlide","allAnsweredDelayS":7}"#);
        assert_eq!(read(&json), settings);
    }
}
