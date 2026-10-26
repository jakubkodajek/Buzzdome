//! Quiz file model (schema version 5). This is the contract with quiz authors:
//! change it only with a schema version bump and a migration.

use crate::category::{category_problems, Category};
use crate::media::{
    check_reference, Background, MediaIssue, MediaKind, Playback, SoundClip, SoundStart, VideoClip, BACKGROUND_KINDS, MAX_GAIN_DB,
    MAX_INTRO_PAUSE_MS, MAX_REPLAY_DELAY_MS, MAX_VIDEO_SPEED, MIN_GAIN_DB, MIN_VIDEO_SPEED,
};
use crate::settings::QuizSettings;
use crate::style::SlideStyle;
use crate::text::file_key;
use crate::theme::is_theme_id;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use ts_rs::TS;

/// 1: first version. 2: adds explanation slides. 3: backgrounds become
/// objects with video options; sounds get a part, a loudness gain and a
/// replay part; later within 3, fastest finger questions may have 6
/// answers instead of 4 (builds before refuse more). 4: adds final slides;
/// later within 4, video options and slide styles (builds before drop them
/// when they save). 5: a question may have several correct answers, each
/// with its share of the points (`correct` becomes a list); later within 5,
/// categories (builds before drop them when they save), and later still the
/// quiz's settings (builds before play as with the defaults and drop them
/// when they save). Older files read unchanged (`migrate`).
pub const SCHEMA_VERSION: u32 = 5;

/// Most answers a question can show (A-F, as many as the reference app's
/// keypads have). A fastest finger question is judged by the quizmaster, so
/// it may use them all: with one to four the first press of A to D is the
/// answer, otherwise the big button buzzes and the answer is said aloud
/// (`Button::buzzes_in`).
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
/// The share of a question's points a fully correct answer gives, in percent.
pub const FULL_SHARE: u8 = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Quiz {
    #[schemars(range(min = 1, max = SCHEMA_VERSION))]
    pub schema_version: u32,
    pub title: String,
    /// Id of the look the screens use (spec section 5.2); the app's default
    /// when missing or not installed.
    #[serde(default, deserialize_with = "theme_id_or_none")]
    #[schemars(with = "Option<String>")]
    pub theme: Option<String>,
    #[serde(default)]
    pub slides: Vec<Slide>,
    /// Named looks the slides can take on (spec QZ-10).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<Category>,
    /// How the show runs (spec QZ-11). Left out when every setting is its default.
    #[serde(default, skip_serializing_if = "QuizSettings::is_default", with = "crate::style::lenient_or_default")]
    #[schemars(with = "QuizSettings")]
    #[ts(as = "Option<QuizSettings>", optional)]
    pub settings: QuizSettings,
}

/// A `theme` that is not a theme id (see `is_theme_id`), a text or not,
/// can name no installed theme, so it reads as none: the default look it
/// would show anyway (`ReadQuiz::dropped` says so). That also keeps an
/// unbounded value out of the quiz.
fn theme_id_or_none<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.as_ref().and_then(serde_json::Value::as_str).filter(|id| is_theme_id(id)).map(str::to_owned))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    /// Shown only to the quizmaster in the Director.
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    #[schemars(pattern(crate::media::MEDIA_PATTERN))]
    pub picture: Option<String>,
    /// Shown in place of a picture; a slide has one or the other.
    #[serde(default)]
    pub video: Option<VideoClip>,
    #[serde(default)]
    pub sound: Option<SoundClip>,
    /// Picture or video behind the slide; the theme's otherwise.
    #[serde(default, with = "crate::media::background_any_version")]
    #[schemars(with = "Option<Background>")]
    #[ts(as = "Option<Background>")]
    pub background: Option<Background>,
    /// Colours, gradients and fonts over the theme's.
    #[serde(default, skip_serializing_if = "SlideStyle::is_empty", with = "crate::style::lenient_or_default")]
    #[schemars(with = "SlideStyle")]
    #[ts(as = "SlideStyle")]
    pub style: SlideStyle,
    /// Id of the category whose look the slide last took on (`Quiz::categories`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub category: Option<String>,
    #[serde(flatten)]
    pub content: SlideContent,
}

impl Slide {
    /// A slide without notes or media.
    pub fn new(id: impl Into<String>, content: SlideContent) -> Slide {
        Slide {
            id: id.into(),
            notes: String::new(),
            picture: None,
            video: None,
            sound: None,
            background: None,
            style: SlideStyle::default(),
            category: None,
            content,
        }
    }

    /// Drops what the slide's type cannot play (`SlideType::plays`) and
    /// tells `dropped` the problem each drop fixed. Files saved before
    /// `validate` refused these still have them (the editor's `convertSlide`
    /// kept them when a slide changed type, and drops them the same way
    /// now). A sound that waited for the clock plays when the slide opens,
    /// and a fastest-finger answer with a part of the points gives all of them.
    fn drop_unplayable(&mut self, dropped: &mut impl FnMut(Problem)) {
        if let SlideContent::Question(q) = &mut self.content {
            if q.part_shares_on_fastest_finger() {
                q.correct.iter_mut().for_each(|c| c.share = FULL_SHARE);
                dropped(Problem::ShareNotOnEveryone);
            }
        }
        let plays = self.content.slide_type().plays();
        if !plays.picture_and_video && (self.picture.is_some() || self.video.is_some()) {
            self.picture = None;
            self.video = None;
            dropped(Problem::MediaOnRanking);
        }
        if plays.clock_and_reveal {
            return;
        }
        if let Some(sound) = &mut self.sound {
            if sound.start == SoundStart::WithClock {
                sound.start = SoundStart::OnSlide;
                dropped(Problem::WithClockNotOnQuestion);
            }
            if sound.replay.take().is_some() {
                dropped(Problem::ReplayNotOnQuestion { media: MediaKind::Audio });
            }
        }
        if let Some(video) = &mut self.video {
            if video.effect.take().is_some() {
                dropped(Problem::EffectNotOnQuestion);
            }
            if video.replay.take().is_some() {
                dropped(Problem::ReplayNotOnQuestion { media: MediaKind::Video });
            }
        }
    }

    /// The media files this slide uses, with the kinds each may be.
    pub fn media(&self) -> Vec<(&str, &'static [MediaKind])> {
        const IMAGE: &[MediaKind] = &[MediaKind::Image];
        const AUDIO: &[MediaKind] = &[MediaKind::Audio];
        const VIDEO: &[MediaKind] = &[MediaKind::Video];
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
            media.push((background.file.as_str(), BACKGROUND_KINDS));
        }
        media
    }

    /// Points the media references in `renamed` (old reference to new one)
    /// to their new names.
    pub fn rename_media(&mut self, renamed: &BTreeMap<String, String>) {
        for file in self.media_mut() {
            if let Some(new) = renamed.get(file.as_str()) {
                file.clone_from(new);
            }
        }
    }

    /// Takes `file` off the slide wherever it is used, with its options.
    pub fn remove_media(&mut self, file: &str) {
        if self.picture.as_deref() == Some(file) {
            self.picture = None;
        }
        if self.video.as_ref().is_some_and(|video| video.file == file) {
            self.video = None;
        }
        if self.sound.as_ref().is_some_and(|sound| sound.file == file) {
            self.sound = None;
        }
        if self.background.as_ref().is_some_and(|background| background.file == file) {
            self.background = None;
        }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
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
    /// The final ranking: a drum roll, then third, second and first place,
    /// then everyone. As a slide it has a sound (the winners' music) and a
    /// background; a quiz without one shows the ranking after its last slide.
    #[serde(rename_all = "camelCase")]
    Final {
        #[serde(default)]
        title: String,
    },
}

impl SlideContent {
    pub fn slide_type(&self) -> SlideType {
        match self {
            SlideContent::Billboard { .. } => SlideType::Billboard,
            SlideContent::Question(_) => SlideType::Question,
            SlideContent::Explanation { .. } => SlideType::Explanation,
            SlideContent::EndOfRound { .. } => SlideType::EndOfRound,
            SlideContent::Final { .. } => SlideType::Final,
        }
    }
}

/// A slide's type, named as the quiz file's `type` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideType {
    Billboard,
    Question,
    Explanation,
    EndOfRound,
    Final,
}

/// What a slide of a type can play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Plays {
    /// A picture or a video.
    pub picture_and_video: bool,
    /// A clock, for a sound to start with and a video effect to follow,
    /// and a reveal, for a sound or video to replay a part after.
    pub clock_and_reveal: bool,
}

impl SlideType {
    pub const ALL: [SlideType; 5] = [SlideType::Billboard, SlideType::Question, SlideType::Explanation, SlideType::EndOfRound, SlideType::Final];

    /// The one rule of what a slide of this type can play (spec QZ-2), which
    /// `Quiz::validate`, opening a file, the spreadsheet import and the
    /// editor (`SLIDE_PLAYS` in Constants.ts) follow: a ranking (end of
    /// round, final) shows the ranking where others have a picture or
    /// video, and only a question has a clock and reveals an answer.
    pub fn plays(self) -> Plays {
        Plays { picture_and_video: !matches!(self, SlideType::EndOfRound | SlideType::Final), clock_and_reveal: self == SlideType::Question }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum AnswerMode {
    /// Every player answers with A-D before the clock runs out.
    Everyone,
    /// The first player to buzz answers; the quizmaster judges.
    FastestFinger,
}

/// How many answers a question of one mode can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AnswerLimits {
    pub fewest: usize,
    pub most: usize,
}

impl AnswerMode {
    pub const ALL: [AnswerMode; 2] = [AnswerMode::Everyone, AnswerMode::FastestFinger];

    /// Everyone answers with a button, so 2 to 4; a fastest finger question
    /// is judged by the quizmaster, so none to all six letters
    /// (`MAX_ANSWERS`).
    pub fn answer_limits(self) -> AnswerLimits {
        match self {
            AnswerMode::Everyone => AnswerLimits { fewest: MIN_EVERYONE_ANSWERS, most: MAX_EVERYONE_ANSWERS },
            AnswerMode::FastestFinger => AnswerLimits { fewest: 0, most: MAX_ANSWERS },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub text: String,
    pub mode: AnswerMode,
    /// Empty for a fastest-finger question answered verbally.
    #[serde(default)]
    pub answers: Vec<String>,
    /// The right answers, each once. Required for everyone-answers
    /// questions; a fastest finger question shows them at the reveal.
    #[serde(default)]
    pub correct: Vec<CorrectAnswer>,
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

/// A right answer of a question and the share of the question's points it
/// gives, so a nearly right answer can give fewer points. Shares below 100
/// are for everyone-answers questions: on a fastest finger question the
/// quizmaster judges what was said, not which answer it was, so a right
/// one gives all of the points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CorrectAnswer {
    /// Index into `answers`, from 0.
    pub answer: usize,
    /// Percent of the question's points, 1 to 100.
    #[serde(default = "full_share")]
    pub share: u8,
}

impl CorrectAnswer {
    /// `answer`, giving all of the points.
    pub fn full(answer: usize) -> CorrectAnswer {
        CorrectAnswer { answer, share: FULL_SHARE }
    }
}

fn full_share() -> u8 {
    FULL_SHARE
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

    /// The share of the points `answer` gives; none for a wrong answer. A
    /// fastest-finger question gives all of them, whatever a session saved
    /// before `validate` refused part shares there says.
    pub fn share_of(&self, answer: usize) -> Option<u8> {
        let share = self.correct.iter().find(|c| c.answer == answer)?.share;
        Some(if self.mode == AnswerMode::FastestFinger { FULL_SHARE } else { share })
    }

    /// A fastest-finger right answer gives a part of the points (`Problem::ShareNotOnEveryone`).
    fn part_shares_on_fastest_finger(&self) -> bool {
        self.mode == AnswerMode::FastestFinger && self.correct.iter().any(|c| (1..FULL_SHARE).contains(&c.share))
    }

    pub fn is_correct(&self, answer: usize) -> bool {
        self.share_of(answer).is_some()
    }

    /// What `answer` (none: a button that is no answer) given after
    /// `elapsed_ms` on the clock scores: its share of the points, or the
    /// penalty for a wrong one.
    pub fn points_for(&self, answer: Option<usize>, elapsed_ms: u64) -> i32 {
        let Some(share) = answer.and_then(|a| self.share_of(a)) else {
            return -self.penalty;
        };
        let (points, share) = (i64::from(self.points_at(elapsed_ms)), i64::from(share));
        // Round to the nearest point, symmetric for negative points.
        let scaled = (points * share * 2 + i64::from(FULL_SHARE) * points.signum()) / (i64::from(FULL_SHARE) * 2);
        // A share of at most 100 % keeps it within the points, so within i32.
        scaled as i32
    }

    /// Adds the problems of this question to `found`.
    fn problems(&self, found: &mut impl FnMut(Problem)) {
        let AnswerLimits { fewest, most } = self.mode.answer_limits();
        if self.answers.len() > most {
            found(Problem::TooManyAnswers { max: most });
        }
        if self.answers.len() < fewest {
            found(Problem::TooFewAnswers { min: fewest });
        }
        if self.mode == AnswerMode::Everyone && self.correct.is_empty() {
            found(Problem::NoCorrectAnswer);
        }
        if self.correct.iter().any(|c| c.answer >= self.answers.len()) {
            found(Problem::CorrectOutOfRange);
        }
        let mut seen = HashSet::new();
        if !self.correct.iter().all(|c| seen.insert(c.answer)) {
            found(Problem::CorrectTwice);
        }
        if self.correct.iter().any(|c| !(1..=FULL_SHARE).contains(&c.share)) {
            found(Problem::ShareOutOfRange { max: FULL_SHARE });
        }
        if self.part_shares_on_fastest_finger() {
            found(Problem::ShareNotOnEveryone);
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

/// A problem that keeps a quiz from being saved, and most from being
/// played (`Problem::keeps_from_playing`), or that reading a file fixed by
/// leaving something out (`ReadQuiz::dropped`). The UI words it in the
/// quizmaster's language; `Display` is for logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS, thiserror::Error)]
#[ts(export)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Problem {
    #[error("duplicate slide id")]
    DuplicateSlideId,
    #[error("media file {file}: {issue}")]
    Media { file: String, issue: MediaIssue },
    #[error("a slide shows a picture or a video, not both")]
    PictureAndVideo,
    #[error("{media:?} volume is a percentage, 0 to 100")]
    Volume { media: MediaKind },
    #[error("a part of the {media:?} ends before it starts")]
    PartEmpty { media: MediaKind },
    /// `max` in seconds.
    #[error("the replay part must start at most {max} s after the reveal")]
    ReplayDelayTooLong { max: u32 },
    #[error("loudness gain must be between {min} and {max} dB")]
    SoundGainOutOfRange { min: i32, max: i32 },
    #[error("video speed must be between {min} and {max} %")]
    VideoSpeedOutOfRange { min: u16, max: u16 },
    /// `max` in seconds.
    #[error("the video must start at most {max} s after the slide")]
    IntroPauseTooLong { max: u32 },
    #[error("only a question reveals an answer to replay a part of its {media:?} after")]
    ReplayNotOnQuestion { media: MediaKind },
    #[error("only a question has a clock for its sound to play with")]
    WithClockNotOnQuestion,
    #[error("only a question has an answer time for a video effect to fade over")]
    EffectNotOnQuestion,
    #[error("an end of round or final slide shows the ranking, not a picture or a video")]
    MediaOnRanking,
    /// Only in `ReadQuiz::dropped`: a slide's style is always valid once read.
    /// `path` names the value as the file has it, such as `style.headline.color`.
    #[error("the style value {path} cannot be read")]
    StyleUnread { path: String },
    /// Only in `ReadQuiz::dropped`, as `StyleUnread` for a category's style.
    #[error("the style value {path} of category {name} cannot be read")]
    CategoryStyleUnread { name: String, path: String },
    /// Only in `ReadQuiz::dropped`: the quiz's `theme` is no theme id, so
    /// the quiz reads with the default look.
    #[error("the quiz's theme cannot be read")]
    ThemeUnread,
    /// On a slide whose category the quiz does not have; in
    /// `ReadQuiz::dropped`, the slide read without it.
    #[error("the slide's category is not in the quiz")]
    UnknownCategory,
    #[error("two categories have the same id")]
    DuplicateCategoryId { name: String },
    #[error("the id of category {name} is empty or longer than {max} characters")]
    CategoryIdInvalid { name: String, max: usize },
    #[error("a category has no name")]
    CategoryUnnamed,
    #[error("a category name has more than {max} characters")]
    CategoryNameTooLong { max: usize },
    #[error("two categories are called {name}")]
    CategoryNameTaken { name: String },
    #[error("media file {file} of category {name}: {issue}")]
    CategoryMedia { name: String, file: String, issue: MediaIssue },
    /// Two media files whose names differ only in case or in how their
    /// accents are encoded (`file_key`): one file on macOS, and on Windows
    /// when only case differs, so one would take the other's place.
    #[error("media file {file} has the name of {other} to macOS or Windows")]
    MediaNameClash { file: String, other: String },
    #[error("more than {max} answers")]
    TooManyAnswers { max: usize },
    #[error("an everyone-answers question needs at least {min} answers")]
    TooFewAnswers { min: usize },
    #[error("an everyone-answers question needs a correct answer")]
    NoCorrectAnswer,
    #[error("a correct answer points past the answer list")]
    CorrectOutOfRange,
    #[error("an answer is listed as correct twice")]
    CorrectTwice,
    #[error("a correct answer's share of the points must be between 1 and {max} %")]
    ShareOutOfRange { max: u8 },
    /// A share below 100 % on a fastest finger question, whose right answer
    /// the quizmaster judges without knowing which one it was.
    #[error("only an everyone-answers question can give a share of the points")]
    ShareNotOnEveryone,
    #[error("points must be between -{max} and {max}")]
    PointsOutOfRange { max: i32 },
    #[error("penalty must be between 0 and {max}")]
    PenaltyOutOfRange { max: i32 },
    /// `max` in seconds.
    #[error("time limit must be at most {max} s")]
    TimeLimitTooLong { max: u32 },
}

impl Problem {
    /// Whether a show refuses the quiz for it. A show reads only the
    /// slides: a category's look and background were copied onto its
    /// slides when it was applied, so a category's problems only keep the
    /// quiz from being saved.
    pub fn keeps_from_playing(&self) -> bool {
        !matches!(
            self,
            Problem::CategoryStyleUnread { .. }
                | Problem::UnknownCategory
                | Problem::DuplicateCategoryId { .. }
                | Problem::CategoryIdInvalid { .. }
                | Problem::CategoryUnnamed
                | Problem::CategoryNameTooLong { .. }
                | Problem::CategoryNameTaken { .. }
                | Problem::CategoryMedia { .. }
        )
    }
}

/// A problem and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// A quiz as read from its JSON.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadQuiz {
    pub quiz: Quiz,
    /// What the file has that the quiz cannot keep, each as the problem it
    /// would be: a style value that does not read, or what a slide's type
    /// cannot play. The quiz reads without it, so the next save leaves it
    /// out; the editor tells the author.
    pub dropped: Vec<ValidationError>,
}

/// Why a quiz's JSON cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QuizJsonError {
    #[error("quiz file uses schema version {found}, this app supports up to {supported}")]
    NewerSchema { found: u32, supported: u32 },
    #[error("{0}")]
    Syntax(String),
}

/// Brings the JSON of an older schema `version` to the current one, so the
/// quiz reads unchanged: up to version 4 a question's `correct` is one
/// index or null.
fn migrate(quiz: &mut serde_json::Value, version: u64) {
    use serde_json::{json, Value};
    if version >= 5 {
        return;
    }
    let questions = quiz.get_mut("slides").and_then(Value::as_array_mut).into_iter().flatten().filter(|s| s["type"] == "question");
    for slide in questions {
        if let Some(fields) = slide.as_object_mut() {
            let correct = match fields.get("correct") {
                Some(Value::Number(index)) => json!([{ "answer": index, "share": FULL_SHARE }]),
                _ => json!([]),
            };
            fields.insert("correct".into(), correct);
        }
    }
}

impl Quiz {
    /// A quiz of the current schema version in the default look.
    pub fn new(title: impl Into<String>, slides: Vec<Slide>) -> Quiz {
        Quiz { schema_version: SCHEMA_VERSION, title: title.into(), theme: None, slides, categories: Vec::new(), settings: QuizSettings::default() }
    }

    /// Reads a quiz, leaving out what it cannot keep (see `ReadQuiz::dropped`).
    pub fn from_json(json: &str) -> Result<Quiz, QuizJsonError> {
        Ok(Quiz::read_json(json)?.quiz)
    }

    /// Reads a quiz and says what reading left out.
    pub fn read_json(json: &str) -> Result<ReadQuiz, QuizJsonError> {
        use serde_json::Value;
        let syntax = |e: serde_json::Error| QuizJsonError::Syntax(e.to_string());
        let mut value: Value = serde_json::from_str(json).map_err(syntax)?;
        let version = value.get("schemaVersion").and_then(Value::as_u64).unwrap_or(0);
        if version > u64::from(SCHEMA_VERSION) {
            let found = u32::try_from(version).unwrap_or(u32::MAX);
            return Err(QuizJsonError::NewerSchema { found, supported: SCHEMA_VERSION });
        }
        migrate(&mut value, version);
        let mut quiz = Quiz::deserialize(&value).map_err(syntax)?;
        // Every older version reads as the current one, so it is saved as such.
        quiz.schema_version = SCHEMA_VERSION;
        // The slides read in the file's order, one for each.
        let written = value.get("slides").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        let mut dropped = Vec::new();
        if quiz.theme.is_none() && value.get("theme").is_some_and(|theme| !theme.is_null()) {
            dropped.push(ValidationError { slide_id: None, problem: Problem::ThemeUnread });
        }
        let written_categories = value.get("categories").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        for (category, written) in quiz.categories.iter().zip(written_categories) {
            for path in category.style.left_out(written.get("style").unwrap_or(&Value::Null)) {
                dropped.push(ValidationError { slide_id: None, problem: Problem::CategoryStyleUnread { name: category.name.clone(), path } });
            }
        }
        let categories: HashSet<&str> = quiz.categories.iter().map(|category| category.id.as_str()).collect();
        for (slide, written) in quiz.slides.iter_mut().zip(written) {
            let id = slide.id.clone();
            let mut found = |problem: Problem| dropped.push(ValidationError { slide_id: Some(id.clone()), problem });
            for path in slide.style.left_out(written.get("style").unwrap_or(&Value::Null)) {
                found(Problem::StyleUnread { path });
            }
            slide.drop_unplayable(&mut found);
            // A category that is gone gives the slide nothing to apply again; the look stays.
            if slide.category.as_deref().is_some_and(|category| !categories.contains(category)) {
                slide.category = None;
                found(Problem::UnknownCategory);
            }
        }
        Ok(ReadQuiz { quiz, dropped })
    }

    /// Index of the slide that shows the final ranking: the last final slide.
    /// None when the quiz has no final slide.
    pub fn final_slide(&self) -> Option<usize> {
        self.slides.iter().rposition(|slide| matches!(slide.content, SlideContent::Final { .. }))
    }

    /// Every media file the quiz uses, its categories' too, each once.
    pub fn media_files(&self) -> BTreeSet<&str> {
        let slides = self.slides.iter().flat_map(|s| s.media()).map(|(file, _)| file);
        slides.chain(self.categories.iter().filter_map(Category::media)).collect()
    }

    /// Points the media references in `renamed` (old reference to new one)
    /// to their new names, in the slides and the categories.
    pub fn rename_media(&mut self, renamed: &BTreeMap<String, String>) {
        for slide in &mut self.slides {
            slide.rename_media(renamed);
        }
        for file in self.categories.iter_mut().filter_map(Category::media_mut) {
            if let Some(new) = renamed.get(file.as_str()) {
                file.clone_from(new);
            }
        }
    }

    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();
        category_problems(&self.categories, &mut |problem| errors.push(ValidationError { slide_id: None, problem }));
        let categories: HashSet<&str> = self.categories.iter().map(|category| category.id.as_str()).collect();
        let mut ids = HashSet::new();
        for slide in &self.slides {
            let mut found = |problem: Problem| errors.push(ValidationError { slide_id: Some(slide.id.clone()), problem });
            if !ids.insert(slide.id.as_str()) {
                found(Problem::DuplicateSlideId);
            }
            if slide.category.as_deref().is_some_and(|category| !categories.contains(category)) {
                found(Problem::UnknownCategory);
            }
            for (file, kinds) in slide.media() {
                if let Err(issue) = check_reference(file, kinds) {
                    found(Problem::Media { file: file.to_string(), issue });
                }
            }
            if slide.picture.is_some() && slide.video.is_some() {
                found(Problem::PictureAndVideo);
            }
            let plays = slide.content.slide_type().plays();
            if !plays.picture_and_video && (slide.picture.is_some() || slide.video.is_some()) {
                found(Problem::MediaOnRanking);
            }
            let question = plays.clock_and_reveal;
            if let Some(sound) = &slide.sound {
                playback_problems(sound, question, &mut found);
                if !(MIN_GAIN_DB as f32..=MAX_GAIN_DB as f32).contains(&sound.gain_db) {
                    found(Problem::SoundGainOutOfRange { min: MIN_GAIN_DB, max: MAX_GAIN_DB });
                }
                if sound.start == SoundStart::WithClock && !question {
                    found(Problem::WithClockNotOnQuestion);
                }
            }
            if let Some(video) = &slide.video {
                playback_problems(video, question, &mut found);
                if video.effect.is_some() && !question {
                    found(Problem::EffectNotOnQuestion);
                }
                if !(MIN_VIDEO_SPEED..=MAX_VIDEO_SPEED).contains(&video.speed) {
                    found(Problem::VideoSpeedOutOfRange { min: MIN_VIDEO_SPEED, max: MAX_VIDEO_SPEED });
                }
                if video.intro_pause_ms > MAX_INTRO_PAUSE_MS {
                    found(Problem::IntroPauseTooLong { max: MAX_INTRO_PAUSE_MS / 1000 });
                }
            }
            if let SlideContent::Question(q) = &slide.content {
                q.problems(&mut found);
            }
        }
        let mut by_key: HashMap<String, &str> = HashMap::new();
        for file in self.media_files() {
            match by_key.entry(file_key(file)) {
                Entry::Occupied(other) => {
                    let problem = Problem::MediaNameClash { file: file.to_string(), other: other.get().to_string() };
                    errors.push(ValidationError { slide_id: None, problem });
                }
                Entry::Vacant(free) => {
                    free.insert(file);
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

/// The rules a sound and a video share: a volume in percent, parts that hold
/// something, and a replay part that starts in time, on a `question` only.
fn playback_problems(clip: &impl Playback, question: bool, found: &mut impl FnMut(Problem)) {
    let media = clip.kind();
    if clip.volume() > 100 {
        found(Problem::Volume { media });
    }
    if clip.parts().any(|part| part.is_empty()) {
        found(Problem::PartEmpty { media });
    }
    if let Some(replay) = clip.replay() {
        if !question {
            found(Problem::ReplayNotOnQuestion { media });
        }
        if replay.delay_ms > MAX_REPLAY_DELAY_MS {
            found(Problem::ReplayDelayTooLong { max: MAX_REPLAY_DELAY_MS / 1000 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Button;
    use crate::media::{Replay, Span};
    use serde_json::json;

    fn question(points: i32, points_end: Option<i32>, time_limit_s: u32) -> Question {
        Question {
            text: "?".into(),
            mode: AnswerMode::Everyone,
            answers: vec!["a".into(), "b".into()],
            correct: vec![CorrectAnswer::full(0)],
            time_limit_s,
            points,
            points_end,
            penalty: 0,
        }
    }

    #[test]
    fn a_correct_answer_scores_its_share_of_the_points_and_a_wrong_one_the_penalty() {
        let mut q = question(100, Some(50), 10);
        q.answers.push("c".into());
        q.correct = vec![CorrectAnswer::full(0), CorrectAnswer { answer: 2, share: 25 }];
        q.penalty = 30;
        assert_eq!((q.points_for(Some(0), 0), q.points_for(Some(2), 0), q.points_for(Some(2), 10_000)), (100, 25, 13));
        assert_eq!((q.points_for(Some(1), 0), q.points_for(None, 0)), (-30, -30));
        q.points = -7;
        q.points_end = None;
        assert_eq!(q.points_for(Some(2), 0), -2, "rounded away from zero like positive points");
        assert!(q.is_correct(2) && !q.is_correct(1));
        // A fastest-finger answer gives all the points, whatever its share says.
        q.mode = AnswerMode::FastestFinger;
        assert_eq!((q.share_of(2), q.points_for(Some(2), 0)), (Some(FULL_SHARE), -7));
    }

    #[test]
    fn a_fastest_finger_part_share_saved_before_it_was_refused_reads_as_all_the_points() {
        let json = r#"{"schemaVersion": 5, "title": "x", "slides": [
            {"id": "a", "type": "question", "mode": "fastestFinger", "text": "?", "answers": ["a", "b"], "correct": [{"answer": 0}, {"answer": 1, "share": 50}]},
            {"id": "b", "type": "question", "mode": "everyone", "text": "?", "answers": ["a", "b"], "correct": [{"answer": 1, "share": 50}]}
        ]}"#;
        let ReadQuiz { quiz, dropped } = Quiz::read_json(json).unwrap();
        assert_eq!(dropped, [ValidationError { slide_id: Some("a".into()), problem: Problem::ShareNotOnEveryone }]);
        let correct = |i: usize| match &quiz.slides[i].content {
            SlideContent::Question(q) => q.correct.clone(),
            _ => unreachable!(),
        };
        assert_eq!((correct(0), correct(1)), (vec![CorrectAnswer::full(0), CorrectAnswer::full(1)], vec![CorrectAnswer { answer: 1, share: 50 }]));
        assert_eq!(quiz.validate(), Ok(()));
    }

    #[test]
    fn version_4_questions_read_with_their_one_correct_answer() {
        let json = r#"{"schemaVersion": 4, "title": "x", "slides": [
            {"id": "a", "type": "question", "mode": "everyone", "text": "?", "answers": ["a", "b"], "correct": 1},
            {"id": "b", "type": "question", "mode": "fastestFinger", "text": "?", "correct": null},
            {"id": "c", "type": "question", "mode": "fastestFinger", "text": "?"},
            {"id": "d", "type": "billboard", "title": "", "correct": 3}
        ]}"#;
        let quiz = Quiz::from_json(json).unwrap();
        let correct = |i: usize| match &quiz.slides[i].content {
            SlideContent::Question(q) => q.correct.clone(),
            _ => unreachable!(),
        };
        assert_eq!((correct(0), correct(1), correct(2)), (vec![CorrectAnswer::full(1)], vec![], vec![]));
        assert_eq!(quiz.schema_version, SCHEMA_VERSION);
        // A version 5 share left out is the whole of the points.
        let json = r#"{"schemaVersion": 5, "title": "x", "slides": [
            {"id": "a", "type": "question", "mode": "everyone", "text": "?", "answers": ["a", "b"], "correct": [{"answer": 0}, {"answer": 1, "share": 40}]}
        ]}"#;
        let SlideContent::Question(q) = &Quiz::from_json(json).unwrap().slides[0].content else { unreachable!() };
        assert_eq!(q.correct, [CorrectAnswer::full(0), CorrectAnswer { answer: 1, share: 40 }]);
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
    fn a_final_slide_reads_without_a_title_and_marks_where_the_ranking_is() {
        let json = r#"{"schemaVersion": 4, "title": "x", "slides": [
            {"id": "a", "type": "billboard", "title": ""},
            {"id": "f", "type": "final"}
        ]}"#;
        let quiz = Quiz::from_json(json).unwrap();
        assert_eq!(quiz.slides[1].content, SlideContent::Final { title: String::new() });
        assert_eq!(quiz.final_slide(), Some(1));
        assert!(quiz.validate().is_ok());
        assert_eq!(Quiz::new("y", vec![]).final_slide(), None);
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
    fn a_theme_that_is_not_a_theme_id_reads_as_none_and_says_so() {
        let long = format!("cz.{}", "a".repeat(crate::theme::MAX_THEME_ID_LEN));
        let unread = vec![ValidationError { slide_id: None, problem: Problem::ThemeUnread }];
        for theme in [json!("Autumn"), json!("../themes"), json!(long), json!(7), json!({ "id": "cz.example.x" })] {
            let read = Quiz::read_json(&json!({ "schemaVersion": 3, "title": "x", "theme": theme }).to_string()).unwrap();
            assert_eq!((read.quiz.theme, read.dropped), (None, unread.clone()), "{theme}");
        }
        for fine in [json!({ "schemaVersion": 3, "title": "x", "theme": null }), json!({ "schemaVersion": 3, "title": "x" })] {
            assert_eq!(Quiz::read_json(&fine.to_string()).unwrap().dropped, []);
        }
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
        let cases: [(Edit, Problem); 13] = [
            (|q| q.answers = vec!["a".into(); MAX_EVERYONE_ANSWERS + 1], Problem::TooManyAnswers { max: MAX_EVERYONE_ANSWERS }),
            (|q| q.answers.truncate(1), Problem::TooFewAnswers { min: MIN_EVERYONE_ANSWERS }),
            (|q| q.correct.clear(), Problem::NoCorrectAnswer),
            (|q| q.correct = vec![CorrectAnswer::full(2)], Problem::CorrectOutOfRange),
            (|q| q.correct.push(CorrectAnswer { answer: 0, share: 50 }), Problem::CorrectTwice),
            (|q| q.correct[0].share = 0, Problem::ShareOutOfRange { max: FULL_SHARE }),
            (|q| q.correct[0].share = FULL_SHARE + 1, Problem::ShareOutOfRange { max: FULL_SHARE }),
            (|q| (q.mode, q.correct[0].share) = (AnswerMode::FastestFinger, 50), Problem::ShareNotOnEveryone),
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
    fn media_names_one_file_on_windows_and_macos_are_a_problem() {
        let billboard = |id: &str, picture: &str| {
            let mut slide = Slide::new(id, SlideContent::Billboard { title: "x".into(), text: String::new() });
            slide.picture = Some(picture.into());
            slide
        };
        let decomposed = "media/Ku\u{30a}n\u{30c}.png";
        let slides = vec![billboard("a", "media/kůň.png"), billboard("b", decomposed), billboard("c", "media/flag.png")];
        let clash = Problem::MediaNameClash { file: "media/kůň.png".into(), other: decomposed.into() };
        assert_eq!(problems_of(slides), [clash]);
        // One name used twice is one file.
        assert_eq!(problems_of(vec![billboard("a", "media/flag.png"), billboard("b", "media/flag.png")]), []);
        assert!(Problem::MediaNameClash { file: String::new(), other: String::new() }.keeps_from_playing());
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
        slide.video = Some(VideoClip::new("media/clip.mp4"));
        assert_eq!(problems_of(vec![slide]), [Problem::PictureAndVideo]);

        // A question, which may use every option.
        let quiz_question = || Slide::new("q", SlideContent::Question(question(10, None, 20)));
        let with_sound = |edit: fn(&mut SoundClip)| {
            let mut slide = quiz_question();
            let mut sound = sound("media/a.mp3");
            edit(&mut sound);
            slide.sound = Some(sound);
            problems_of(vec![slide])
        };
        let audio = MediaKind::Audio;
        assert_eq!(with_sound(|s| s.volume = 101), [Problem::Volume { media: audio }]);
        assert_eq!(with_sound(|s| s.part = Span { from_ms: 500, to_ms: Some(500) }), [Problem::PartEmpty { media: audio }]);
        let empty_replay = |s: &mut SoundClip| s.replay = Some(Replay { part: Span { from_ms: 9_000, to_ms: Some(1_000) }, delay_ms: 0 });
        assert_eq!(with_sound(empty_replay), [Problem::PartEmpty { media: audio }]);
        let late_replay = |s: &mut SoundClip| s.replay = Some(Replay { part: Span::default(), delay_ms: MAX_REPLAY_DELAY_MS + 1 });
        assert_eq!(with_sound(late_replay), [Problem::ReplayDelayTooLong { max: MAX_REPLAY_DELAY_MS / 1000 }]);
        let gain = Problem::SoundGainOutOfRange { min: MIN_GAIN_DB, max: MAX_GAIN_DB };
        assert_eq!(with_sound(|s| s.gain_db = MAX_GAIN_DB as f32 + 0.1), std::slice::from_ref(&gain));
        assert_eq!(with_sound(|s| s.gain_db = MIN_GAIN_DB as f32 - 0.1), std::slice::from_ref(&gain));
        // The limits themselves, and a part that runs to the end of the file, are fine.
        assert_eq!(with_sound(|s| (s.volume, s.gain_db) = (100, MAX_GAIN_DB as f32)), []);
        assert_eq!(with_sound(|s| (s.gain_db, s.part) = (MIN_GAIN_DB as f32, Span { from_ms: 60_000, to_ms: None })), []);

        let with_video = |edit: fn(&mut VideoClip)| {
            let mut slide = quiz_question();
            let mut video = VideoClip::new("media/clip.mp4");
            edit(&mut video);
            slide.video = Some(video);
            problems_of(vec![slide])
        };
        let video = MediaKind::Video;
        assert_eq!(with_video(|v| v.volume = 101), [Problem::Volume { media: video }]);
        assert_eq!(with_video(|v| v.part = Span { from_ms: 9_000, to_ms: Some(3_000) }), [Problem::PartEmpty { media: video }]);
        let speed = Problem::VideoSpeedOutOfRange { min: MIN_VIDEO_SPEED, max: MAX_VIDEO_SPEED };
        assert_eq!(with_video(|v| v.speed = MIN_VIDEO_SPEED - 1), std::slice::from_ref(&speed));
        assert_eq!(with_video(|v| v.speed = MAX_VIDEO_SPEED + 1), std::slice::from_ref(&speed));
        assert_eq!(with_video(|v| v.intro_pause_ms = MAX_INTRO_PAUSE_MS + 1), [Problem::IntroPauseTooLong { max: MAX_INTRO_PAUSE_MS / 1000 }]);
        let late_replay = |v: &mut VideoClip| v.replay = Some(Replay { part: Span::default(), delay_ms: MAX_REPLAY_DELAY_MS + 1 });
        assert_eq!(with_video(late_replay), [Problem::ReplayDelayTooLong { max: MAX_REPLAY_DELAY_MS / 1000 }]);
        // The limits themselves are fine.
        let at_limits = |v: &mut VideoClip| (v.volume, v.speed, v.intro_pause_ms) = (100, MAX_VIDEO_SPEED, MAX_INTRO_PAUSE_MS);
        assert_eq!(with_video(at_limits), []);
        assert_eq!(with_video(|v| v.speed = MIN_VIDEO_SPEED), []);
    }

    #[test]
    fn options_that_need_a_question_are_problems_on_other_slides() {
        let replay = Some(Replay { part: Span::default(), delay_ms: 0 });
        let mut sound = sound("media/a.mp3");
        (sound.start, sound.replay) = (SoundStart::WithClock, replay);
        let video = VideoClip { effect: Some(crate::media::VideoEffect::Blur), replay, ..VideoClip::new("media/clip.mp4") };
        let with_options = |content: SlideContent| {
            let mut slide = Slide::new("s", content);
            (slide.sound, slide.video) = (Some(sound.clone()), Some(video.clone()));
            problems_of(vec![slide])
        };
        assert_eq!(with_options(SlideContent::Question(question(10, None, 20))), []);
        let elsewhere = [
            Problem::ReplayNotOnQuestion { media: MediaKind::Audio },
            Problem::WithClockNotOnQuestion,
            Problem::ReplayNotOnQuestion { media: MediaKind::Video },
            Problem::EffectNotOnQuestion,
        ];
        assert_eq!(with_options(SlideContent::Billboard { title: "x".into(), text: String::new() }), elsewhere);
        assert_eq!(with_options(SlideContent::Explanation { text: String::new() }), elsewhere);
    }

    #[test]
    fn a_ranking_shows_no_picture_or_video_but_plays_its_sound() {
        let with_media = |content: SlideContent, picture: bool| {
            let mut slide = Slide::new("s", content);
            if picture {
                slide.picture = Some("media/a.png".into());
            } else {
                slide.video = Some(VideoClip::new("media/clip.mp4"));
            }
            slide.sound = Some(sound("media/a.mp3"));
            problems_of(vec![slide])
        };
        let final_slide = || SlideContent::Final { title: String::new() };
        assert_eq!(with_media(final_slide(), true), [Problem::MediaOnRanking]);
        assert_eq!(with_media(final_slide(), false), [Problem::MediaOnRanking]);
        assert_eq!(with_media(SlideContent::EndOfRound { title: String::new() }, true), [Problem::MediaOnRanking]);
        assert_eq!(with_media(SlideContent::Explanation { text: String::new() }, true), []);
    }

    #[test]
    fn slide_types_are_named_as_the_file_names_them() {
        let contents = [
            SlideContent::Billboard { title: String::new(), text: String::new() },
            SlideContent::Question(question(10, None, 20)),
            SlideContent::Explanation { text: String::new() },
            SlideContent::EndOfRound { title: String::new() },
            SlideContent::Final { title: String::new() },
        ];
        let types: Vec<SlideType> = contents.iter().map(SlideContent::slide_type).collect();
        assert_eq!(types, SlideType::ALL);
        for content in &contents {
            assert_eq!(serde_json::to_value(content).unwrap()["type"], serde_json::to_value(content.slide_type()).unwrap());
        }
    }

    #[test]
    fn a_video_reads_with_the_options_it_had_before_they_were_added() {
        let old: VideoClip = serde_json::from_str(r#"{"file": "media/clip.mp4", "autoplay": false, "muted": true, "repeat": true}"#).unwrap();
        let expected = VideoClip { autoplay: false, muted: true, repeat: true, ..VideoClip::new("media/clip.mp4") };
        assert_eq!(old, expected);
        assert_eq!((old.intro_pause_ms, old.volume, old.speed, old.part, old.effect, old.replay), (0, 100, 100, Span::default(), None, None));
        let json = r#"{"file": "media/clip.mp4", "introPauseMs": 2500, "volume": 40, "speed": 50, "part": {"fromMs": 5000, "toMs": 20000},
                       "effect": "mosaic", "replay": {"part": {"fromMs": 15000}, "delayMs": 500}}"#;
        let full: VideoClip = serde_json::from_str(json).unwrap();
        assert_eq!((full.intro_pause_ms, full.volume, full.speed, full.effect), (2_500, 40, 50, Some(crate::media::VideoEffect::Mosaic)));
        assert_eq!(full.part, Span { from_ms: 5_000, to_ms: Some(20_000) });
        assert_eq!(full.replay, Some(Replay { part: Span { from_ms: 15_000, to_ms: None }, delay_ms: 500 }));
        assert_eq!(serde_json::from_str::<VideoClip>(&serde_json::to_string(&full).unwrap()).unwrap(), full);
    }

    /// A sound with the options a quiz file gets when it names only the file.
    fn sound(file: &str) -> SoundClip {
        serde_json::from_value(serde_json::json!({ "file": file })).unwrap()
    }

    #[test]
    fn removing_a_media_file_takes_it_off_wherever_the_slide_uses_it() {
        let mut slide = Slide::new("s", SlideContent::Billboard { title: "t".into(), text: String::new() });
        slide.picture = Some("media/a.png".into());
        slide.sound = Some(sound("media/b.mp3"));
        slide.background = Some(Background::new("media/a.png"));
        slide.remove_media("media/a.png");
        assert_eq!((&slide.picture, &slide.background), (&None, &None));
        slide.remove_media("media/other.mp3");
        assert_eq!(slide.sound, Some(sound("media/b.mp3")));
        slide.remove_media("media/b.mp3");
        assert_eq!(slide.sound, None);
    }

    #[test]
    fn a_new_sound_has_the_options_of_a_file_that_names_only_it() {
        assert_eq!(SoundClip::new("media/a.mp3"), sound("media/a.mp3"));
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
        assert_eq!(MAX_EVERYONE_ANSWERS, crate::game::ANSWER_BUTTONS.len());
        let buttons = [Button::A, Button::B, Button::C, Button::D, Button::Big];
        assert!((0..MAX_EVERYONE_ANSWERS).all(|i| buttons.iter().any(|b| b.answer_index() == Some(i))));
        assert_eq!(Button::Big.answer_index(), None);
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
    fn says_which_style_values_a_hand_edited_file_read_without() {
        let json = r##"{"schemaVersion": 5, "title": "x", "slides": [
            {"id": "a", "type": "billboard", "title": "", "style": {"fill": {"from": "#000000", "to": "#ffffff", "kind": "vertical"}}},
            {"id": "b", "type": "billboard", "title": "", "style": {"headline": {"color": "red", "bold": true}}}
        ]}"##;
        let ReadQuiz { quiz, dropped } = Quiz::read_json(json).unwrap();
        assert_eq!(quiz.slides[1].style.headline.bold, Some(true));
        let path = "style.headline.color".to_owned();
        assert_eq!(dropped, [ValidationError { slide_id: Some("b".into()), problem: Problem::StyleUnread { path } }]);
        // What reading left out is not in the file it saves, so that reads whole.
        assert_eq!(Quiz::read_json(&serde_json::to_string(&quiz).unwrap()).unwrap().dropped, []);
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
    fn reads_what_a_slide_type_cannot_play_from_older_files_without_it() {
        // As our editor saved a question turned into another slide before `validate` refused these.
        let ReadQuiz { quiz, dropped } = Quiz::read_json(
            r#"{"schemaVersion": 4, "title": "x", "slides": [
                {"id": "f", "type": "final", "picture": "media/a.png", "sound": {"file": "media/win.mp3", "start": "withClock", "replay": {}}},
                {"id": "v", "type": "final", "video": {"file": "media/a.mp4"}},
                {"id": "e", "type": "endOfRound", "title": "", "picture": "media/a.png"},
                {"id": "b", "type": "billboard", "title": "", "video": {"file": "media/a.mp4", "effect": "mosaic", "replay": {}},
                 "sound": {"file": "media/a.mp3", "start": "onDemand", "replay": {}}},
                {"id": "q", "type": "question", "mode": "fastestFinger", "text": "?", "video": {"file": "media/a.mp4", "effect": "blur", "replay": {}},
                 "sound": {"file": "media/a.mp3", "start": "withClock", "replay": {}}}
            ]}"#,
        )
        .unwrap();
        assert_eq!(quiz.validate(), Ok(()));
        // Each drop is the problem it fixed, so the editor can tell the author.
        let on = |id: &str, problem| ValidationError { slide_id: Some(id.into()), problem };
        let (audio, video) = (MediaKind::Audio, MediaKind::Video);
        assert_eq!(
            dropped,
            [
                on("f", Problem::MediaOnRanking),
                on("f", Problem::WithClockNotOnQuestion),
                on("f", Problem::ReplayNotOnQuestion { media: audio }),
                on("v", Problem::MediaOnRanking),
                on("e", Problem::MediaOnRanking),
                on("b", Problem::ReplayNotOnQuestion { media: audio }),
                on("b", Problem::EffectNotOnQuestion),
                on("b", Problem::ReplayNotOnQuestion { media: video }),
            ]
        );
        let [final_slide, final_video, end_of_round, billboard, question] = &quiz.slides[..] else { panic!("five slides") };
        assert_eq!((&final_slide.picture, &final_video.video, &end_of_round.picture), (&None, &None, &None));
        let sound = final_slide.sound.as_ref().unwrap();
        assert_eq!((sound.start, sound.replay), (SoundStart::OnSlide, None));
        let (video, sound) = (billboard.video.as_ref().unwrap(), billboard.sound.as_ref().unwrap());
        assert_eq!((video.effect, video.replay, sound.start, sound.replay), (None, None, SoundStart::OnDemand, None));
        // A question keeps all of it.
        let (video, sound) = (question.video.as_ref().unwrap(), question.sound.as_ref().unwrap());
        assert!(video.effect.is_some() && video.replay.is_some() && sound.replay.is_some());
        assert_eq!(sound.start, SoundStart::WithClock);
    }

    #[test]
    fn a_slide_keeps_its_style_and_writes_none_without_one() {
        let quiz = Quiz::from_json(
            r##"{"schemaVersion": 4, "title": "x", "slides": [
                {"id": "a", "type": "billboard", "title": "", "style": {"headline": {"color": "#FFCC00", "size": 150}, "text": {}}},
                {"id": "b", "type": "billboard", "title": "", "style": "red"}
            ]}"##,
        )
        .unwrap();
        let written = serde_json::to_value(&quiz).unwrap();
        assert_eq!(written["slides"][0]["style"], serde_json::json!({"headline": {"color": "#FFCC00", "size": 150}}));
        // An unreadable style is the theme's, and a slide with none writes none.
        assert!(quiz.slides[1].style.is_empty());
        assert!(written["slides"][1].get("style").is_none());
        assert_eq!(Quiz::from_json(&written.to_string()).unwrap(), quiz);
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

    #[test]
    fn keeps_categories_and_the_slides_that_took_one_on() {
        let json = r##"{"schemaVersion": 5, "title": "x",
            "categories": [{"id": "c1", "name": "Science", "background": {"file": "media/lab.mp4", "sync": true},
                            "style": {"fill": {"from": "#000000", "to": "#102030", "kind": "radial"}, "answers": {"bold": false}}}],
            "slides": [{"id": "a", "type": "billboard", "title": "", "category": "c1"}, {"id": "b", "type": "billboard", "title": ""}]}"##;
        let ReadQuiz { quiz, dropped } = Quiz::read_json(json).unwrap();
        assert_eq!(dropped, []);
        assert_eq!(quiz.validate(), Ok(()));
        let science = &quiz.categories[0];
        assert_eq!((science.name.as_str(), science.background.as_ref().map(|b| (b.file.as_str(), b.repeat, b.sync))), ("Science", Some(("media/lab.mp4", true, true))));
        assert_eq!(science.style.answers.bold, Some(false));
        assert_eq!((quiz.slides[0].category.as_deref(), quiz.slides[1].category.as_deref()), (Some("c1"), None));
        // A slide without a category and a quiz without categories write none.
        let written = serde_json::to_value(&quiz).unwrap();
        assert!(written["slides"][1].get("category").is_none());
        assert!(serde_json::to_value(Quiz::new("y", vec![])).unwrap().get("categories").is_none());
        assert_eq!(Quiz::from_json(&written.to_string()).unwrap(), quiz);
    }

    #[test]
    fn says_what_reading_left_out_of_categories_and_slides_that_name_one_gone() {
        let json = r##"{"schemaVersion": 5, "title": "x",
            "categories": [{"id": "c1", "name": "Music", "style": {"headline": {"color": "blue", "bold": true}}}],
            "slides": [{"id": "a", "type": "billboard", "title": "", "category": "gone"}]}"##;
        let ReadQuiz { quiz, dropped } = Quiz::read_json(json).unwrap();
        let unread = Problem::CategoryStyleUnread { name: "Music".into(), path: "style.headline.color".into() };
        assert_eq!(
            dropped,
            [ValidationError { slide_id: None, problem: unread }, ValidationError { slide_id: Some("a".into()), problem: Problem::UnknownCategory }]
        );
        assert_eq!(quiz.slides[0].category, None);
        assert_eq!(Quiz::read_json(&serde_json::to_string(&quiz).unwrap()).unwrap().dropped, []);
    }

    #[test]
    fn checks_categories_and_the_categories_slides_name() {
        let mut quiz = Quiz::new("x", vec![Slide::new("a", SlideContent::Billboard { title: String::new(), text: String::new() })]);
        quiz.categories = vec![Category::new("c1", "Sport"), Category::new("c2", "sport")];
        quiz.categories[0].background = Some(Background::new("media/stage.txt"));
        quiz.slides[0].category = Some("c3".into());
        let issue = MediaIssue::WrongType { expected: vec![MediaKind::Image, MediaKind::Video] };
        assert_eq!(
            quiz.validate().unwrap_err(),
            [
                ValidationError { slide_id: None, problem: Problem::CategoryMedia { name: "Sport".into(), file: "media/stage.txt".into(), issue } },
                ValidationError { slide_id: None, problem: Problem::CategoryNameTaken { name: "sport".into() } },
                ValidationError { slide_id: Some("a".into()), problem: Problem::UnknownCategory },
            ]
        );
    }

    #[test]
    fn a_categorys_background_is_among_the_quizs_media_and_renamed_with_them() {
        let mut quiz = Quiz::new("x", vec![]);
        quiz.categories = vec![Category::new("c1", "Sport"), Category::new("c2", "Film")];
        quiz.categories[0].background = Some(Background::new("media/a?.mp4"));
        quiz.rename_media(&[("media/a?.mp4".to_string(), "media/a_.mp4".to_string())].into_iter().collect());
        assert_eq!(quiz.media_files().into_iter().collect::<Vec<_>>(), ["media/a_.mp4"]);
    }
}
