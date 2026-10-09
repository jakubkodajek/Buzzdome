//! Quiz file model (schema version 1). This is the contract with quiz authors:
//! change it only with a schema version bump and a migration.

use crate::media::{check_reference, MediaIssue, MediaKind, SoundClip, VideoClip};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};
use ts_rs::TS;

/// 1: first version. 2: adds explanation slides; version 1 files read unchanged.
pub const SCHEMA_VERSION: u32 = 2;

/// Most answers a question can have: one per answer button of a buzzer (A-D).
pub const MAX_ANSWERS: usize = 4;
/// Fewest answers an everyone-answers question can have.
pub const MIN_EVERYONE_ANSWERS: usize = 2;
/// Seconds to answer when a quiz file leaves `timeLimitS` out.
pub const DEFAULT_TIME_LIMIT_S: u32 = 20;
/// Points when a quiz file leaves `points` out.
pub const DEFAULT_POINTS: i32 = 10;
/// Largest number of points (or penalty) a question can give, either way, so
/// scores cannot overflow.
pub const MAX_POINTS: i32 = 100_000;
/// Longest time limit of a question, in seconds.
pub const MAX_TIME_LIMIT_S: u32 = 3_600;

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
    #[serde(default)]
    pub picture: Option<String>,
    /// Shown in place of a picture; a slide has one or the other.
    #[serde(default)]
    pub video: Option<VideoClip>,
    #[serde(default)]
    pub sound: Option<SoundClip>,
    /// Picture or looping video behind the slide; the theme's otherwise.
    #[serde(default)]
    pub background: Option<String>,
    #[serde(flatten)]
    pub content: SlideContent,
}

impl Slide {
    /// A slide without notes or media.
    pub fn new(id: impl Into<String>, content: SlideContent) -> Slide {
        Slide { id: id.into(), notes: String::new(), picture: None, video: None, sound: None, background: None, content }
    }

    /// The media files this slide uses, with the kinds each may be.
    pub fn media(&self) -> Vec<(&str, &'static [MediaKind])> {
        const IMAGE: &[MediaKind] = &[MediaKind::Image];
        const AUDIO: &[MediaKind] = &[MediaKind::Audio];
        const VIDEO: &[MediaKind] = &[MediaKind::Video];
        const BACKGROUND: &[MediaKind] = &[MediaKind::Image, MediaKind::Video];
        let mut media = Vec::new();
        if let Some(file) = &self.picture {
            media.push((file.as_str(), IMAGE));
        }
        if let Some(video) = &self.video {
            media.push((video.file.as_str(), VIDEO));
        }
        if let Some(sound) = &self.sound {
            media.push((sound.file.as_str(), AUDIO));
        }
        if let Some(file) = &self.background {
            media.push((file.as_str(), BACKGROUND));
        }
        media
    }
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
    },
    Question(Question),
    /// Explains the answer of the question before it in the same round. The
    /// screen shows that question and its correct answer above `text`; with
    /// no question before it, only `text`.
    #[serde(rename_all = "camelCase")]
    Explanation {
        #[serde(default)]
        text: String,
    },
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
}

fn default_time_limit() -> u32 {
    DEFAULT_TIME_LIMIT_S
}

fn default_points() -> i32 {
    DEFAULT_POINTS
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
        // Wide enough that no field values can overflow.
        let (elapsed, limit) = (i128::from(elapsed_ms.min(limit)), i128::from(limit));
        let start = i128::from(self.points);
        let span = i128::from(end) - start;
        // Round to the nearest point, symmetric for rising and falling scales.
        let delta = (span * elapsed * 2 + limit * span.signum()) / (limit * 2);
        // Between `points` and `points_end`, so within i32.
        (start + delta) as i32
    }

    /// Adds the problems of this question to `found`.
    fn problems(&self, found: &mut impl FnMut(Problem)) {
        if self.answers.len() > MAX_ANSWERS {
            found(Problem::TooManyAnswers { max: MAX_ANSWERS });
        }
        if self.mode == AnswerMode::Everyone {
            if self.answers.len() < MIN_EVERYONE_ANSWERS {
                found(Problem::TooFewAnswers { min: MIN_EVERYONE_ANSWERS });
            }
            if self.correct.is_none() {
                found(Problem::NoCorrectAnswer);
            }
        }
        if self.correct.is_some_and(|c| c >= self.answers.len()) {
            found(Problem::CorrectOutOfRange);
        }
        let points = -MAX_POINTS..=MAX_POINTS;
        if !points.contains(&self.points) || self.points_end.is_some_and(|end| !points.contains(&end)) {
            found(Problem::PointsOutOfRange { max: MAX_POINTS });
        }
        if !(0..=MAX_POINTS).contains(&self.penalty) {
            found(Problem::PenaltyOutOfRange { max: MAX_POINTS });
        }
        if self.time_limit_s > MAX_TIME_LIMIT_S {
            found(Problem::TimeLimitTooLong { max: MAX_TIME_LIMIT_S });
        }
    }
}

/// A problem that keeps a quiz from being played. The UI words it in the
/// quizmaster's language; `Display` is for logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Problem {
    #[error("duplicate slide id")]
    DuplicateSlideId,
    #[error("media file {file}: {issue}")]
    Media { file: String, issue: MediaIssue },
    #[error("a slide shows a picture or a video, not both")]
    PictureAndVideo,
    #[error("sound volume is a percentage, 0 to 100")]
    SoundVolume,
    #[error("more than {max} answers")]
    TooManyAnswers { max: usize },
    #[error("an everyone-answers question needs at least {min} answers")]
    TooFewAnswers { min: usize },
    #[error("an everyone-answers question needs a correct answer")]
    NoCorrectAnswer,
    #[error("the correct answer points past the answer list")]
    CorrectOutOfRange,
    #[error("points must be between -{max} and {max}")]
    PointsOutOfRange { max: i32 },
    #[error("penalty must be between 0 and {max}")]
    PenaltyOutOfRange { max: i32 },
    /// `max` in seconds.
    #[error("time limit must be at most {max} s")]
    TimeLimitTooLong { max: u32 },
}

/// A problem and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ValidationError {
    /// The slide it is on; none for problems of the whole quiz.
    pub slide_id: Option<String>,
    pub problem: Problem,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.slide_id {
            Some(id) => write!(f, "slide {id}: {}", self.problem),
            None => write!(f, "{}", self.problem),
        }
    }
}

/// Why a quiz's JSON cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuizJsonError {
    #[error("quiz file uses schema version {found}, this app supports up to {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("{0}")]
    Syntax(String),
}

impl Quiz {
    pub fn from_json(json: &str) -> Result<Quiz, QuizJsonError> {
        let mut quiz: Quiz = serde_json::from_str(json).map_err(|e| QuizJsonError::Syntax(e.to_string()))?;
        if quiz.schema_version > SCHEMA_VERSION {
            return Err(QuizJsonError::NewerSchema { found: quiz.schema_version, supported: SCHEMA_VERSION });
        }
        // Every older version reads as the current one, so it is saved as such.
        quiz.schema_version = SCHEMA_VERSION;
        Ok(quiz)
    }

    /// Every media file the quiz uses, each once.
    pub fn media_files(&self) -> BTreeSet<&str> {
        self.slides.iter().flat_map(|s| s.media()).map(|(file, _)| file).collect()
    }

    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        let mut ids = HashSet::new();
        for slide in &self.slides {
            let mut found = |problem: Problem| errors.push(ValidationError { slide_id: Some(slide.id.clone()), problem });
            if !ids.insert(slide.id.as_str()) {
                found(Problem::DuplicateSlideId);
            }
            for (file, kinds) in slide.media() {
                if let Err(issue) = check_reference(file, kinds) {
                    found(Problem::Media { file: file.to_string(), issue });
                }
            }
            if slide.picture.is_some() && slide.video.is_some() {
                found(Problem::PictureAndVideo);
            }
            if slide.sound.as_ref().is_some_and(|s| s.volume > 100) {
                found(Problem::SoundVolume);
            }
            if let SlideContent::Question(q) = &slide.content {
                q.problems(&mut found);
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
    use crate::game::Button;

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
    fn points_never_overflow_whatever_the_fields_say() {
        let q = question(i32::MAX, Some(i32::MIN), u32::MAX);
        assert_eq!(q.points_at(0), i32::MAX);
        assert_eq!(q.points_at(u64::MAX), i32::MIN);
        assert!((i32::MIN..=i32::MAX).contains(&q.points_at(u64::from(u32::MAX) * 500)));
    }

    #[test]
    fn rejects_a_newer_schema_and_broken_json() {
        assert_eq!(
            Quiz::from_json(r#"{"schemaVersion": 99, "title": "x"}"#),
            Err(QuizJsonError::NewerSchema { found: 99, supported: SCHEMA_VERSION })
        );
        assert!(matches!(Quiz::from_json("{"), Err(QuizJsonError::Syntax(_))));
    }

    /// The problems `validate` finds in these slides.
    fn problems_of(slides: Vec<Slide>) -> Vec<Problem> {
        let quiz = Quiz { schema_version: SCHEMA_VERSION, title: "x".into(), slides };
        quiz.validate().err().unwrap_or_default().into_iter().map(|e| e.problem).collect()
    }

    /// A change to a valid question.
    type Edit = fn(&mut Question);

    fn question_slide(edit: Edit) -> Slide {
        let mut q = question(10, None, 10);
        edit(&mut q);
        Slide::new("a", SlideContent::Question(q))
    }

    #[test]
    fn each_question_rule_names_its_problem() {
        let cases: [(Edit, Problem); 9] = [
            (|q| q.answers = vec!["a".into(); MAX_ANSWERS + 1], Problem::TooManyAnswers { max: MAX_ANSWERS }),
            (|q| q.answers.truncate(1), Problem::TooFewAnswers { min: MIN_EVERYONE_ANSWERS }),
            (|q| q.correct = None, Problem::NoCorrectAnswer),
            (|q| q.correct = Some(2), Problem::CorrectOutOfRange),
            (|q| q.points = MAX_POINTS + 1, Problem::PointsOutOfRange { max: MAX_POINTS }),
            (|q| q.points_end = Some(-MAX_POINTS - 1), Problem::PointsOutOfRange { max: MAX_POINTS }),
            (|q| q.penalty = -1, Problem::PenaltyOutOfRange { max: MAX_POINTS }),
            (|q| q.penalty = MAX_POINTS + 1, Problem::PenaltyOutOfRange { max: MAX_POINTS }),
            (|q| q.time_limit_s = MAX_TIME_LIMIT_S + 1, Problem::TimeLimitTooLong { max: MAX_TIME_LIMIT_S }),
        ];
        for (edit, problem) in cases {
            assert_eq!(problems_of(vec![question_slide(edit)]), std::slice::from_ref(&problem), "{problem}");
        }
        // The limits themselves are fine.
        let at_limits = question_slide(|q| {
            (q.points, q.points_end, q.penalty, q.time_limit_s) = (MAX_POINTS, Some(-MAX_POINTS), MAX_POINTS, MAX_TIME_LIMIT_S)
        });
        assert_eq!(problems_of(vec![at_limits]), []);
    }

    #[test]
    fn each_slide_rule_names_its_problem() {
        let intro = || Slide::new("intro", SlideContent::Billboard { title: "x".into(), text: String::new() });
        assert_eq!(problems_of(vec![intro(), intro()]), [Problem::DuplicateSlideId]);

        let mut slide = intro();
        slide.picture = Some("../secret.jpg".into());
        let issue = MediaIssue::NotInMediaFolder;
        assert_eq!(problems_of(vec![slide]), [Problem::Media { file: "../secret.jpg".into(), issue }]);

        let mut slide = intro();
        slide.picture = Some("media/a.png".into());
        slide.video = Some(VideoClip { file: "media/clip.mp4".into(), autoplay: true, muted: false, repeat: false });
        assert_eq!(problems_of(vec![slide]), [Problem::PictureAndVideo]);

        let mut slide = intro();
        slide.sound = Some(SoundClip { file: "media/a.mp3".into(), start: Default::default(), repeat: false, volume: 101 });
        assert_eq!(problems_of(vec![slide]), [Problem::SoundVolume]);
    }

    #[test]
    fn a_question_has_at_most_one_answer_per_button() {
        // Every answer must be one a buzzer button can choose.
        let buttons = [Button::A, Button::B, Button::C, Button::D, Button::Big];
        assert!((0..MAX_ANSWERS).all(|i| buttons.iter().any(|b| b.answer_index() == Some(i))));
    }

    #[test]
    fn reads_an_older_version_as_the_current_one() {
        let quiz = Quiz::from_json(r#"{"schemaVersion": 1, "title": "Old"}"#).unwrap();
        assert_eq!(quiz.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn reads_slide_media_and_checks_it() {
        let json = r#"{
            "schemaVersion": 1,
            "title": "Media",
            "slides": [
                {"id": "q1", "type": "question", "mode": "fastestFinger", "text": "Which song?",
                 "picture": "media/cover.jpg", "background": "media/stage.mp4",
                 "sound": {"file": "media/song.mp3", "start": "withClock"}}
            ]
        }"#;
        let quiz = Quiz::from_json(json).unwrap();
        let slide = &quiz.slides[0];
        let sound = slide.sound.as_ref().unwrap();
        assert_eq!((sound.start, sound.repeat, sound.volume), (crate::media::SoundStart::WithClock, false, 100));
        assert_eq!(quiz.media_files().into_iter().collect::<Vec<_>>(), ["media/cover.jpg", "media/song.mp3", "media/stage.mp4"]);
        assert!(quiz.validate().is_ok());

        let mut broken = quiz.clone();
        broken.slides[0].background = Some("media/stage.txt".into());
        let issue = MediaIssue::WrongType { expected: vec![MediaKind::Image, MediaKind::Video] };
        let problem = Problem::Media { file: "media/stage.txt".into(), issue };
        assert_eq!(broken.validate().unwrap_err(), [ValidationError { slide_id: Some("q1".into()), problem }]);
    }
}
