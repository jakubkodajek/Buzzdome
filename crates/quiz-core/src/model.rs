//! Quiz file model (schema version 3). This is the contract with quiz authors:
//! change it only with a schema version bump and a migration.

use crate::media::{check_reference, Background, MediaIssue, MediaKind, SoundClip, VideoClip, MAX_GAIN_DB, MIN_GAIN_DB};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use ts_rs::TS;

/// 1: first version. 2: adds explanation slides. 3: backgrounds become
/// objects with video options; sounds get a part, a loudness gain and a
/// replay part. Older files read unchanged.
pub const SCHEMA_VERSION: u32 = 3;

/// Most answers a question can show (A-F, as many as the reference app's
/// keypads have). A fastest finger question is answered aloud, so it may use
/// them all.
pub const MAX_ANSWERS: usize = 6;
/// Most answers an everyone-answers question can have: one per answer button
/// of a buzzer (A-D).
pub const MAX_EVERYONE_ANSWERS: usize = 4;
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
    /// Id of the look the screens use (spec section 5.2); the app's default
    /// when missing or not installed.
    #[serde(default)]
    pub theme: Option<String>,
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
    /// Picture or video behind the slide; the theme's otherwise.
    #[serde(default, with = "crate::media::background_any_version")]
    #[ts(as = "Option<Background>")]
    pub background: Option<Background>,
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
        if let Some(background) = &self.background {
            media.push((background.file.as_str(), BACKGROUND));
        }
        media
    }

    /// The same media references as `media`, to change in place.
    fn media_mut(&mut self) -> impl Iterator<Item = &mut String> {
        [
            self.picture.as_mut(),
            self.video.as_mut().map(|video| &mut video.file),
            self.sound.as_mut().map(|sound| &mut sound.file),
            self.background.as_mut().map(|background| &mut background.file),
        ]
        .into_iter()
        .flatten()
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
        let max = match self.mode {
            AnswerMode::Everyone => MAX_EVERYONE_ANSWERS,
            AnswerMode::FastestFinger => MAX_ANSWERS,
        };
        if self.answers.len() > max {
            found(Problem::TooManyAnswers { max });
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
    #[error("a part of the sound ends before it starts")]
    SoundPartEmpty,
    #[error("loudness gain must be between {min} and {max} dB")]
    SoundGainOutOfRange { min: i32, max: i32 },
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
    /// A quiz of the current schema version in the default look.
    pub fn new(title: impl Into<String>, slides: Vec<Slide>) -> Quiz {
        Quiz { schema_version: SCHEMA_VERSION, title: title.into(), theme: None, slides }
    }

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

    /// Points the media references in `renamed` (old reference to new one)
    /// to their new names.
    pub fn rename_media(&mut self, renamed: &BTreeMap<String, String>) {
        for file in self.slides.iter_mut().flat_map(Slide::media_mut) {
            if let Some(new) = renamed.get(file.as_str()) {
                file.clone_from(new);
            }
        }
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
            if let Some(sound) = &slide.sound {
                if sound.volume > 100 {
                    found(Problem::SoundVolume);
                }
                if sound.parts().any(|part| part.is_empty()) {
                    found(Problem::SoundPartEmpty);
                }
                if !(MIN_GAIN_DB as f32..=MAX_GAIN_DB as f32).contains(&sound.gain_db) {
                    found(Problem::SoundGainOutOfRange { min: MIN_GAIN_DB, max: MAX_GAIN_DB });
                }
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
    use crate::media::{Replay, Span};

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
        assert_eq!(quiz.theme, None);
        assert!(quiz.validate().is_ok());
    }

    #[test]
    fn keeps_the_theme_id_even_when_it_is_not_installed() {
        let quiz = Quiz::from_json(r#"{"schemaVersion": 2, "title": "x", "theme": "cz.example.gone"}"#).unwrap();
        assert_eq!(quiz.theme.as_deref(), Some("cz.example.gone"));
        assert!(quiz.validate().is_ok());
        assert_eq!(Quiz::from_json(&serde_json::to_string(&quiz).unwrap()).unwrap(), quiz);
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
        let quiz = Quiz::new("x", slides);
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
            (|q| q.answers = vec!["a".into(); MAX_EVERYONE_ANSWERS + 1], Problem::TooManyAnswers { max: MAX_EVERYONE_ANSWERS }),
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

        let with_sound = |edit: fn(&mut SoundClip)| {
            let mut slide = intro();
            let mut sound = sound("media/a.mp3");
            edit(&mut sound);
            slide.sound = Some(sound);
            problems_of(vec![slide])
        };
        assert_eq!(with_sound(|s| s.volume = 101), [Problem::SoundVolume]);
        assert_eq!(with_sound(|s| s.part = Span { from_ms: 500, to_ms: Some(500) }), [Problem::SoundPartEmpty]);
        let empty_replay = |s: &mut SoundClip| s.replay = Some(Replay { part: Span { from_ms: 9_000, to_ms: Some(1_000) }, delay_ms: 0 });
        assert_eq!(with_sound(empty_replay), [Problem::SoundPartEmpty]);
        let gain = Problem::SoundGainOutOfRange { min: MIN_GAIN_DB, max: MAX_GAIN_DB };
        assert_eq!(with_sound(|s| s.gain_db = MAX_GAIN_DB as f32 + 0.1), std::slice::from_ref(&gain));
        assert_eq!(with_sound(|s| s.gain_db = MIN_GAIN_DB as f32 - 0.1), std::slice::from_ref(&gain));
        // The limits themselves, and a part that runs to the end of the file, are fine.
        assert_eq!(with_sound(|s| (s.volume, s.gain_db) = (100, MAX_GAIN_DB as f32)), []);
        assert_eq!(with_sound(|s| (s.gain_db, s.part) = (MIN_GAIN_DB as f32, Span { from_ms: 60_000, to_ms: None })), []);
    }

    /// A sound with the options a quiz file gets when it names only the file.
    fn sound(file: &str) -> SoundClip {
        serde_json::from_value(serde_json::json!({ "file": file })).unwrap()
    }

    #[test]
    fn a_sound_plays_the_whole_file_at_its_own_loudness_unless_told_otherwise() {
        let plain = sound("media/a.mp3");
        assert_eq!((plain.part, plain.gain_db, plain.replay), (Span::default(), 0.0, None));
        let json = r#"{"file": "media/a.mp3", "part": {"fromMs": 15000, "toMs": 45000}, "gainDb": -4.5,
                       "replay": {"part": {"fromMs": 60000}, "delayMs": 1500}}"#;
        let trimmed: SoundClip = serde_json::from_str(json).unwrap();
        assert_eq!(trimmed.part, Span { from_ms: 15_000, to_ms: Some(45_000) });
        assert_eq!(trimmed.gain_db, -4.5);
        assert_eq!(trimmed.replay, Some(Replay { part: Span { from_ms: 60_000, to_ms: None }, delay_ms: 1_500 }));
        assert_eq!(serde_json::from_str::<SoundClip>(&serde_json::to_string(&trimmed).unwrap()).unwrap(), trimmed);
    }

    #[test]
    fn reads_a_background_as_any_version_wrote_it() {
        let slide = |background: &str| {
            let json = format!(r#"{{"schemaVersion": 2, "title": "x", "slides": [{{"id": "a", "type": "endOfRound", "title": "", "background": {background}}}]}}"#);
            Quiz::from_json(&json).unwrap().slides.remove(0).background
        };
        assert_eq!(slide(r#""media/stage.mp4""#), Some(Background::new("media/stage.mp4")));
        assert_eq!(slide("null"), None);
        let synced = slide(r#"{"file": "media/stage.mp4", "muted": false, "sync": true}"#).unwrap();
        assert_eq!(synced, Background { file: "media/stage.mp4".into(), repeat: true, muted: false, sync: true });
        let written = serde_json::to_value(&synced).unwrap();
        assert_eq!(written, serde_json::json!({ "file": "media/stage.mp4", "repeat": true, "muted": false, "sync": true }));
        assert!(Quiz::from_json(r#"{"schemaVersion": 3, "title": "x", "slides": [{"id": "a", "type": "endOfRound", "title": "", "background": 7}]}"#).is_err());
    }

    #[test]
    fn an_everyone_answers_question_has_at_most_one_answer_per_button() {
        // Every answer must be one a buzzer button can choose.
        let buttons = [Button::A, Button::B, Button::C, Button::D, Button::Big];
        assert!((0..MAX_EVERYONE_ANSWERS).all(|i| buttons.iter().any(|b| b.answer_index() == Some(i))));
    }

    #[test]
    fn a_fastest_finger_question_may_show_every_answer_letter() {
        let fastest = |count: usize| {
            let mut q = question(10, None, 10);
            q.mode = AnswerMode::FastestFinger;
            q.answers = vec!["a".into(); count];
            Slide::new("a", SlideContent::Question(q))
        };
        assert_eq!(problems_of(vec![fastest(MAX_ANSWERS)]), vec![]);
        assert_eq!(problems_of(vec![fastest(MAX_ANSWERS + 1)]), vec![Problem::TooManyAnswers { max: MAX_ANSWERS }]);
    }

    #[test]
    fn renames_media_wherever_a_slide_uses_it() {
        let mut quiz = Quiz::from_json(
            r#"{"schemaVersion": 3, "title": "x", "slides": [{"id": "a", "type": "billboard", "title": "",
                "picture": "media/a?.png", "video": {"file": "media/a?.mp4"}, "sound": {"file": "media/a?.mp3"},
                "background": {"file": "media/kept.png"}}]}"#,
        )
        .unwrap();
        // `media_mut` lists the same references as `media`.
        let slide = &mut quiz.slides[0];
        let listed: Vec<String> = slide.media().into_iter().map(|(file, _)| file.to_string()).collect();
        assert_eq!(slide.media_mut().map(|file| file.clone()).collect::<Vec<_>>(), listed);

        let renamed = ["png", "mp4", "mp3"].map(|ext| (format!("media/a?.{ext}"), format!("media/a_.{ext}")));
        quiz.rename_media(&renamed.into_iter().collect());
        assert_eq!(quiz.media_files().into_iter().collect::<Vec<_>>(), ["media/a_.mp3", "media/a_.mp4", "media/a_.png", "media/kept.png"]);
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
        broken.slides[0].background = Some(Background::new("media/stage.txt"));
        let issue = MediaIssue::WrongType { expected: vec![MediaKind::Image, MediaKind::Video] };
        let problem = Problem::Media { file: "media/stage.txt".into(), issue };
        assert_eq!(broken.validate().unwrap_err(), [ValidationError { slide_id: Some("q1".into()), problem }]);
    }
}
