//! Quiz file model (schema version 1). This is the contract with quiz authors:
//! change it only with a schema version bump and a migration.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::HashSet;

pub const SCHEMA_VERSION: u32 = 1;

/// Buzzers have four answer buttons (A-D); keypads in the original have six.
pub const MAX_ANSWERS: usize = 6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Quiz {
    pub schema_version: u32,
    pub title: String,
    #[serde(default)]
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    /// Shown only to the quizmaster in the Director.
    #[serde(default)]
    pub notes: String,
    #[serde(flatten)]
    pub content: SlideContent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SlideContent {
    /// Information slide without interaction: round intros, announcements.
    #[serde(rename_all = "camelCase")]
    Billboard {
        title: String,
        #[serde(default)]
        text: String,
        #[serde(default)]
        picture: Option<String>,
    },
    Question(Question),
    /// Closes a round and shows the leaderboard.
    #[serde(rename_all = "camelCase")]
    EndOfRound { title: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AnswerMode {
    /// Every player answers with A-D before the clock runs out.
    Everyone,
    /// The first player to buzz answers; the quizmaster judges.
    FastestFinger,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub text: String,
    pub mode: AnswerMode,
    /// Empty for a fastest-finger question answered verbally.
    #[serde(default)]
    pub answers: Vec<String>,
    /// Index into `answers`. Required for everyone-answers questions.
    #[serde(default)]
    pub correct: Option<usize>,
    /// Seconds to answer; 0 means untimed (the quizmaster ends the question).
    #[serde(default = "default_time_limit")]
    pub time_limit_s: u32,
    /// Points for a correct answer at the start of the clock.
    #[serde(default = "default_points")]
    pub points: i32,
    /// Points for a correct answer when the clock runs out. Points fall linearly
    /// from `points` to this value. `None` keeps them constant.
    #[serde(default)]
    pub points_end: Option<i32>,
    /// Points taken for a wrong answer (a positive number).
    #[serde(default)]
    pub penalty: i32,
    #[serde(default)]
    pub picture: Option<String>,
}

fn default_time_limit() -> u32 {
    20
}

fn default_points() -> i32 {
    10
}

impl Question {
    pub fn time_limit_ms(&self) -> Option<u64> {
        (self.time_limit_s > 0).then(|| u64::from(self.time_limit_s) * 1000)
    }

    /// Points for a correct answer given after `elapsed_ms` on the clock.
    pub fn points_at(&self, elapsed_ms: u64) -> i32 {
        let (Some(end), Some(limit)) = (self.points_end, self.time_limit_ms()) else {
            return self.points;
        };
        let elapsed = elapsed_ms.min(limit) as i64;
        let start = i64::from(self.points);
        let span = i64::from(end) - start;
        // Round to the nearest point, symmetric for rising and falling scales.
        let delta = (span * elapsed * 2 + limit as i64 * span.signum()) / (limit as i64 * 2);
        (start + delta) as i32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub slide_id: Option<String>,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.slide_id {
            Some(id) => write!(f, "slide {id}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl Quiz {
    pub fn from_json(json: &str) -> Result<Quiz, String> {
        let quiz: Quiz = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if quiz.schema_version > SCHEMA_VERSION {
            return Err(format!(
                "quiz file uses schema version {}, this app supports up to {SCHEMA_VERSION}",
                quiz.schema_version
            ));
        }
        Ok(quiz)
    }

    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        let mut ids = HashSet::new();
        for slide in &self.slides {
            let err = |message: String| ValidationError {
                slide_id: Some(slide.id.clone()),
                message,
            };
            if !ids.insert(slide.id.as_str()) {
                errors.push(err("duplicate slide id".into()));
            }
            if let SlideContent::Question(q) = &slide.content {
                if q.answers.len() > MAX_ANSWERS {
                    errors.push(err(format!("more than {MAX_ANSWERS} answers")));
                }
                if q.mode == AnswerMode::Everyone {
                    if q.answers.len() < 2 {
                        errors.push(err("an everyone-answers question needs at least 2 answers".into()));
                    }
                    if q.correct.is_none() {
                        errors.push(err("an everyone-answers question needs a correct answer".into()));
                    }
                }
                if let Some(c) = q.correct {
                    if c >= q.answers.len() {
                        errors.push(err("the correct answer points past the answer list".into()));
                    }
                }
                if q.penalty < 0 {
                    errors.push(err("penalty must not be negative".into()));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(points: i32, points_end: Option<i32>, time_limit_s: u32) -> Question {
        Question {
            text: "?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["a".into(), "b".into()],
            correct: Some(0),
            time_limit_s,
            points,
            points_end,
            penalty: 0,
            picture: None,
        }
    }

    #[test]
    fn points_fall_linearly_over_the_clock() {
        let q = question(100, Some(50), 10);
        assert_eq!(q.points_at(0), 100);
        assert_eq!(q.points_at(5_000), 75);
        assert_eq!(q.points_at(10_000), 50);
        assert_eq!(q.points_at(60_000), 50);
    }

    #[test]
    fn points_stay_constant_without_an_end_value_or_clock() {
        assert_eq!(question(10, None, 10).points_at(9_000), 10);
        assert_eq!(question(10, Some(0), 0).points_at(9_000), 10);
    }

    #[test]
    fn parses_and_validates_a_quiz_file() {
        let json = r#"{
            "schemaVersion": 1,
            "title": "Test",
            "slides": [
                {"id": "intro", "type": "billboard", "title": "Kolo 1"},
                {"id": "q1", "type": "question", "mode": "everyone", "text": "Hlavní město?",
                 "answers": ["Praha", "Brno"], "correct": 0, "timeLimitS": 15},
                {"id": "q2", "type": "question", "mode": "fastestFinger", "text": "Kdo?"},
                {"id": "end", "type": "endOfRound", "title": "Konec kola"}
            ]
        }"#;
        let quiz = Quiz::from_json(json).unwrap();
        assert_eq!(quiz.slides.len(), 4);
        assert!(quiz.validate().is_ok());
    }

    #[test]
    fn rejects_a_newer_schema_and_broken_questions() {
        assert!(Quiz::from_json(r#"{"schemaVersion": 99, "title": "x"}"#).is_err());
        let mut quiz = Quiz { schema_version: 1, title: "x".into(), slides: vec![] };
        let mut q = question(10, None, 10);
        q.correct = Some(5);
        quiz.slides.push(Slide { id: "a".into(), notes: String::new(), content: SlideContent::Question(q) });
        assert_eq!(quiz.validate().unwrap_err().len(), 1);
    }
}
