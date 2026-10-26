//! A quiz's advanced settings (spec QZ-11): how its show runs, for the whole
//! quiz. Every default keeps the show as it ran before there were settings,
//! and a value a build cannot read reads as its default, so a quiz always
//! opens and plays.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use ts_rs::TS;

/// Seconds the show waits by default before it acts on everyone having answered.
pub const DEFAULT_ALL_ANSWERED_DELAY_S: u8 = 3;
/// Longest wait, in seconds, before the show acts on everyone having answered.
pub const MAX_ALL_ANSWERED_DELAY_S: u8 = 10;
/// Seconds a question's answer stays on screen by default before the show goes on by itself.
pub const DEFAULT_AFTER_REVEAL_DELAY_S: u8 = 5;
/// Longest wait, in seconds, before the show goes on from a question's answer.
pub const MAX_AFTER_REVEAL_DELAY_S: u8 = 60;
/// Most jokers a player can have in a show.
pub const MAX_JOKERS: u8 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct QuizSettings {
    /// What the show does once a question has nobody left to answer.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "AllAnswered")]
    #[ts(as = "AllAnswered")]
    pub all_answered: AllAnswered,
    /// Seconds it waits before it does it, up to `MAX_ALL_ANSWERED_DELAY_S`;
    /// the quizmaster can step in meanwhile.
    #[serde(with = "all_answered_delay")]
    #[schemars(with = "u8")]
    #[ts(as = "u8")]
    pub all_answered_delay_s: u8,
    /// Once a question's answer is shown, the show goes on to the next
    /// slide by itself after `next_after_reveal_delay_s`; the quizmaster
    /// can step in meanwhile.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub next_after_reveal: bool,
    /// Up to `MAX_AFTER_REVEAL_DELAY_S`.
    #[serde(with = "after_reveal_delay")]
    #[schemars(with = "u8")]
    #[ts(as = "u8")]
    pub next_after_reveal_delay_s: u8,
    /// A timed question's clock runs to its end even once nobody is left
    /// to answer; the question then closes as everyone having answered it.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub run_clock_out: bool,
    /// The projector does not mark a question's right answers when they
    /// are shown, so the quizmaster can tell them; the Director still does.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub hide_answer: bool,
    /// In an everyone-answers question a player can change their answer
    /// while it runs: their last answer counts, with its time.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub change_answer: bool,
    /// A fastest-finger question takes one buzz: once it is judged wrong,
    /// nobody else can buzz in.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub one_buzz: bool,
    /// In an everyone-answers question a player who gives no answer loses
    /// the question's penalty, as for a wrong answer.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub no_answer_penalty: bool,
    /// The projector does not show what a fastest-finger buzz won or lost;
    /// the leaderboard shows the scores.
    #[serde(with = "crate::style::lenient_or_default")]
    #[schemars(with = "bool")]
    #[ts(as = "bool")]
    pub hide_buzz_points: bool,
    /// Jokers each player has for the show, up to `MAX_JOKERS`; none by
    /// default. A player's joker doubles what one question gives them,
    /// the penalty too; the quizmaster plays it before the question ends.
    #[serde(with = "jokers")]
    #[schemars(with = "u8")]
    #[ts(as = "u8")]
    pub jokers: u8,
}

impl Default for QuizSettings {
    fn default() -> Self {
        QuizSettings {
            all_answered: AllAnswered::Wait,
            all_answered_delay_s: DEFAULT_ALL_ANSWERED_DELAY_S,
            next_after_reveal: false,
            next_after_reveal_delay_s: DEFAULT_AFTER_REVEAL_DELAY_S,
            run_clock_out: false,
            hide_answer: false,
            change_answer: false,
            one_buzz: false,
            no_answer_penalty: false,
            hide_buzz_points: false,
            jokers: 0,
        }
    }
}

impl QuizSettings {
    pub fn is_default(&self) -> bool {
        *self == QuizSettings::default()
    }

    /// See `all_answered_delay_s`.
    pub fn all_answered_delay_ms(&self) -> u64 {
        seconds_to_ms(self.all_answered_delay_s, MAX_ALL_ANSWERED_DELAY_S)
    }

    /// See `next_after_reveal_delay_s`.
    pub fn next_after_reveal_delay_ms(&self) -> u64 {
        seconds_to_ms(self.next_after_reveal_delay_s, MAX_AFTER_REVEAL_DELAY_S)
    }
}

fn seconds_to_ms(seconds: u8, max: u8) -> u64 {
    u64::from(seconds.min(max)) * 1000
}

/// What the show does once a question has nobody left to answer: every
/// player switched on answered it, or in fastest finger answered it wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS, JsonSchema)]
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

impl AllAnswered {
    /// The step the show takes by itself; none when it waits.
    pub fn auto_step(self) -> Option<AutoStep> {
        match self {
            AllAnswered::Wait => None,
            AllAnswered::Reveal => Some(AutoStep::Reveal),
            AllAnswered::NextSlide => Some(AutoStep::NextSlide),
        }
    }
}

/// A step the show takes by itself: once everyone answered (`AllAnswered`),
/// or once a question's answer was shown (`QuizSettings::next_after_reveal`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AutoStep {
    Reveal,
    NextSlide,
}

/// A whole number, such as a wait in seconds: one larger than `max` reads
/// as `max`, and one that is not a whole number as `default`.
pub(crate) fn read_count_up_to<'de, D: Deserializer<'de>>(deserializer: D, default: u8, max: u8) -> Result<u8, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let count = value.as_ref().and_then(serde_json::Value::as_u64);
    Ok(count.map_or(default, |n| u8::try_from(n).unwrap_or(u8::MAX).min(max)))
}

/// A module for `#[serde(with)]` that reads a number with `read_count_up_to`.
macro_rules! count_up_to {
    ($module:ident, $default:expr, $max:expr) => {
        mod $module {
            use serde::{Deserializer, Serialize, Serializer};

            pub fn serialize<S: Serializer>(count: &u8, serializer: S) -> Result<S::Ok, S::Error> {
                count.serialize(serializer)
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
                $crate::settings::read_count_up_to(deserializer, $default, $max)
            }
        }
    };
}
pub(crate) use count_up_to;

count_up_to!(all_answered_delay, super::DEFAULT_ALL_ANSWERED_DELAY_S, super::MAX_ALL_ANSWERED_DELAY_S);
count_up_to!(after_reveal_delay, super::DEFAULT_AFTER_REVEAL_DELAY_S, super::MAX_AFTER_REVEAL_DELAY_S);
count_up_to!(jokers, 0, super::MAX_JOKERS);

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
        let settings = read(r#"{ "allAnswered": "dance", "allAnsweredDelayS": "soon", "oneBuzz": "yes", "nextAfterRevealDelayS": null, "jokers": 1.5 }"#);
        assert_eq!(settings, QuizSettings::default());
        assert_eq!(read(r#"{ "allAnsweredDelayS": -1 }"#).all_answered_delay_s, DEFAULT_ALL_ANSWERED_DELAY_S);
        assert_eq!(read(r#"{ "allAnswered": "reveal" }"#).all_answered, AllAnswered::Reveal);
        assert!(read(r#"{ "hideAnswer": true, "changeAnswer": 1 }"#).hide_answer);
    }

    #[test]
    fn a_wait_past_the_longest_reads_as_the_longest() {
        for seconds in ["11", "255", "100000"] {
            let settings = read(&format!(r#"{{ "allAnsweredDelayS": {seconds} }}"#));
            assert_eq!(settings.all_answered_delay_s, MAX_ALL_ANSWERED_DELAY_S, "{seconds}");
        }
        assert_eq!(read(r#"{ "allAnsweredDelayS": 0 }"#).all_answered_delay_ms(), 0);
        let settings = read(r#"{ "nextAfterRevealDelayS": 61, "jokers": 99 }"#);
        assert_eq!((settings.next_after_reveal_delay_s, settings.next_after_reveal_delay_ms()), (MAX_AFTER_REVEAL_DELAY_S, 60_000));
        assert_eq!(settings.jokers, MAX_JOKERS);
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
        let settings = QuizSettings {
            all_answered: AllAnswered::NextSlide,
            all_answered_delay_s: 7,
            next_after_reveal: true,
            next_after_reveal_delay_s: 30,
            run_clock_out: true,
            hide_answer: true,
            change_answer: true,
            one_buzz: true,
            no_answer_penalty: true,
            hide_buzz_points: true,
            jokers: 2,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            json,
            r#"{"allAnswered":"nextSlide","allAnsweredDelayS":7,"nextAfterReveal":true,"nextAfterRevealDelayS":30,"runClockOut":true,"hideAnswer":true,"changeAnswer":true,"oneBuzz":true,"noAnswerPenalty":true,"hideBuzzPoints":true,"jokers":2}"#
        );
        assert_eq!(read(&json), settings);
    }
}
