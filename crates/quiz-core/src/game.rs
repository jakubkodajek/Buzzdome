//! The live show: a deterministic state machine driven by timestamped commands.
//!
//! Every accepted command is journaled by the app, so replaying the journal on
//! the same quiz rebuilds the exact state after a crash. Time never comes from a
//! clock inside this module: each command carries its own `at_ms`.

use crate::model::{AnswerMode, Question, Quiz, Slide, SlideContent};
use crate::settings::AutoStep;
use crate::text::{composed, name_key};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub type BuzzerId = u16;

/// The version of the rules a journal is replayed with. Bump it whenever
/// the same commands give another show: a resumed show begun under another
/// version runs to its end with today's rules, but for what would put its
/// replay a step off, which it keeps from its own version (`Game::replay`),
/// and the Director says its scores may differ. A command that was refused
/// before and is taken now needs no bump, as a refused command is never
/// journaled.
///
/// 1: refused presses that were journaled count, and renaming a player
/// takes the name from another buzzer. 2: a question in play restarts
/// with `RestartQuestion`, which takes its score changes back, and refuses
/// `GoTo` and `GoToFinal`. 3: only the time of right answers breaks
/// ties, switching a player on or off in a running question closes it when
/// nobody is left to answer, and names are the same however their accents
/// are encoded. 4: a closed question no longer restarts, as the projector
/// shows its results. 5: a fastest-finger question takes buzzes from the
/// answer buttons where every answer has one, else from the big button
/// (`Button::buzzes_in`), and a player switched on while one is under way
/// joins at the next question. 6: a question that ended earlier in the
/// show comes back with its answer shown when the show goes to it again
/// (`Game::closed_questions`), so it is never asked and scored twice.
pub const RULES_VERSION: u32 = 6;

/// The oldest `RULES_VERSION` whose shows replay today as they were played:
/// every later change applies only to shows begun under it
/// (`Game::rules_version`). A change that cannot be kept to newer shows
/// raises this to its version.
pub const REPLAYS_AS_PLAYED_FROM: u32 = 3;

/// Steps of the final score reveal: drum roll, 3rd, 2nd, 1st, full ranking.
pub const FINAL_LAST_STEP: u8 = 4;

/// How long after time up a question stays open, so that presses made in
/// time but still on their way from the receiver are counted. A press counts
/// by when it was received (its `at_ms`), not by when it is applied.
pub const PRESS_GRACE_MS: u64 = 250;

/// The most team names a show can have prepared.
pub const MAX_TEAMS: usize = 200;

/// Player and team names are cut to this many characters, so they fit the
/// projector's lists.
pub const MAX_NAME_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    A,
    B,
    C,
    D,
    /// The big fastest-finger button.
    Big,
}

/// A buzzer's answer buttons, in the order of the answers they give.
pub const ANSWER_BUTTONS: [Button; 4] = [Button::A, Button::B, Button::C, Button::D];

impl Button {
    /// Which answer the button gives, from 0; none for the big button.
    pub fn answer_index(self) -> Option<usize> {
        ANSWER_BUTTONS.iter().position(|&b| b == self)
    }

    /// Whether this button buzzes in on the fastest-finger `question`.
    /// Where every answer has its button (A to D) those buttons do, so the
    /// first press is also the player's answer; with no answers, or with E
    /// and F, which are read out, the big button does.
    pub fn buzzes_in(self, question: &Question) -> bool {
        let by_button = (1..=ANSWER_BUTTONS.len()).contains(&question.answers.len());
        match self.answer_index() {
            Some(i) => by_button && i < question.answers.len(),
            None => !by_button,
        }
    }
}

/// What the buzzer lights should show. Drivers map these to their hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Light {
    Off,
    Ready,
    Locked,
    First,
    Correct,
    Wrong,
}

impl Light {
    pub const ALL: [Light; 6] = [Light::Off, Light::Ready, Light::Locked, Light::First, Light::Correct, Light::Wrong];
}

/// Stops compiling when a light is added: add it to `Light::ALL` too, then here.
const _: fn(Light) = |light| match light {
    Light::Off | Light::Ready | Light::Locked | Light::First | Light::Correct | Light::Wrong => {}
};

/// What the buzzers take in the current step, as a whole: for receivers
/// that take presses only while told to, and to order fastest-finger
/// presses by the buzzers' own timers. It follows from the same state as
/// the lights (`Game::buzzer_mode`), so the two never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuzzerMode {
    /// Presses play no part: between questions, before a question starts
    /// and once it closed.
    Off,
    /// The lobby: a press registers its buzzer.
    Register,
    /// An everyone-answers question runs: each player answers A to D.
    Answers,
    /// A fastest-finger question runs: the first buzz of a player who has
    /// not answered yet wins (`Button::buzzes_in`).
    FastestFinger,
    /// A buzz is being judged.
    Buzzed,
    /// The question's results show: right or wrong.
    Results,
}

/// A moment of the show that plays a sound, flashes the stage lights and
/// is sent to outside programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Cue {
    ClockStart,
    Buzz,
    Correct,
    Wrong,
    TimeUp,
    Reveal,
}

impl Cue {
    pub const ALL: [Cue; 6] = [Cue::ClockStart, Cue::Buzz, Cue::Correct, Cue::Wrong, Cue::TimeUp, Cue::Reveal];
}

/// Side effects for the app to carry out after a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Effect {
    Lights { lights: Vec<(BuzzerId, Light)> },
    Sound { cue: Cue },
}

impl Effect {
    /// Every light `effects` set, a buzzer's later light winning: to light
    /// the buzzers once for one change. Lit effect by effect, a buzzer
    /// judged wrong would show the round's light in between.
    pub fn lights(effects: &[Effect]) -> Vec<(BuzzerId, Light)> {
        let mut lights = BTreeMap::new();
        for effect in effects {
            if let Effect::Lights { lights: set } = effect {
                lights.extend(set.iter().copied());
            }
        }
        lights.into_iter().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// A buzzer button was pressed. Drivers deliver presses in hardware order.
    #[serde(rename_all = "camelCase")]
    Press { buzzer: BuzzerId, button: Button },
    /// Advance the show one step (Space in the original).
    Next,
    /// Jump to a slide; a question there starts from the beginning. Refused
    /// while a question is in play (`Controls::go_to`).
    #[serde(rename_all = "camelCase")]
    GoTo { slide: usize },
    /// Skip the remaining slides and show the final scores; refused as `GoTo` is.
    GoToFinal,
    /// Pause the question clock, or hold the step the quiz's settings take
    /// by themselves once everyone answered (`Game::pending_step`).
    PauseClock,
    ResumeClock,
    /// End the running question now (time up, or the quizmaster ends it).
    CloseQuestion,
    /// Start the question in play again, as after a crash
    /// (`Game::recovery_command`): its score changes are taken back, and it
    /// waits for `Next` to start its clock. Refused once it closed.
    RestartQuestion,
    JudgeRight,
    JudgeWrong,
    #[serde(rename_all = "camelCase")]
    AddPlayer { buzzer: BuzzerId, name: String },
    /// Names a buzzer's player; as with `PairTeam`, a player who had that
    /// name loses it, so a name is never on two buzzers.
    #[serde(rename_all = "camelCase")]
    RenamePlayer { buzzer: BuzzerId, name: String },
    /// Replaces the team names prepared for the show, in order. Empty names
    /// and repeats (`same_name`) are left out.
    SetTeams { names: Vec<String> },
    /// Gives a buzzer a team name; a player who had that name loses it.
    #[serde(rename_all = "camelCase")]
    PairTeam { buzzer: BuzzerId, name: String },
    #[serde(rename_all = "camelCase")]
    SetPlayerEnabled { buzzer: BuzzerId, enabled: bool },
    #[serde(rename_all = "camelCase")]
    AdjustScore { buzzer: BuzzerId, delta: i32 },
    #[serde(rename_all = "camelCase")]
    ShowLeaderboard { visible: bool },
    /// Start the current slide's video from the start of its part, or start
    /// it again. A video that does not start by itself waits for this.
    PlayVideo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamped {
    /// When the command was given or the press received, in milliseconds
    /// since the Unix epoch on the app's show clock (`buzzers::clock`): it
    /// reads the wall clock once and counts on from a monotonic clock, so
    /// within a run it never goes back, whatever the system clock does.
    /// Across a restart it may: a resumed show restarts a question that was
    /// in play (`Game::recovery_command`), so no clock spans two runs.
    pub at_ms: u64,
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GameError {
    #[error("this command does not apply in the current step")]
    NotNow,
    #[error("unknown buzzer {0}")]
    UnknownBuzzer(BuzzerId),
    #[error("no slide {0}")]
    NoSuchSlide(usize),
    #[error("buzzer {0} already exists")]
    DuplicateBuzzer(BuzzerId),
    #[error("more than {MAX_TEAMS} team names")]
    TooManyTeams,
}

/// Which step-dependent quizmaster commands the current step takes
/// (see `Game::controls`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Controls {
    pub next: bool,
    /// `JudgeRight` and `JudgeWrong`.
    pub judge: bool,
    /// `PauseClock`: also while the show waits to take a step by itself.
    pub pause_clock: bool,
    pub resume_clock: bool,
    pub close_question: bool,
    pub play_video: bool,
    /// `GoTo` and `GoToFinal`: not while a question is in play.
    pub go_to: bool,
    /// `RestartQuestion`: while a question is in play, the way to leave it
    /// without scoring it and ask it again. Not once it closed, as its
    /// results are on screen; a show begun before rules version 4 also
    /// restarts it closed, until its answer is shown.
    pub restart_question: bool,
}

/// A countdown that can pause. `limit_ms == None` means untimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub limit_ms: Option<u64>,
    pub elapsed_before_ms: u64,
    pub running_since_ms: Option<u64>,
    /// Presses received from this time on count: when the question started,
    /// or when it went on after a player's buzz. The quizmaster's pause does
    /// not move it.
    pub presses_from_ms: u64,
}

impl Clock {
    fn start(limit_ms: Option<u64>, now: u64) -> Clock {
        Clock { limit_ms, elapsed_before_ms: 0, running_since_ms: Some(now), presses_from_ms: now }
    }

    pub fn elapsed_ms(&self, now: u64) -> u64 {
        let running = self.running_since_ms.map_or(0, |since| now.saturating_sub(since));
        let elapsed = self.elapsed_before_ms + running;
        self.limit_ms.map_or(elapsed, |limit| elapsed.min(limit))
    }

    pub fn remaining_ms(&self, now: u64) -> Option<u64> {
        self.limit_ms.map(|limit| limit - self.elapsed_ms(now))
    }

    pub fn is_running(&self) -> bool {
        self.running_since_ms.is_some()
    }

    fn pause(&mut self, now: u64) {
        if self.running_since_ms.is_some() {
            self.elapsed_before_ms = self.elapsed_ms(now);
            self.running_since_ms = None;
        }
    }

    fn resume(&mut self, now: u64) {
        if self.running_since_ms.is_none() {
            self.running_since_ms = Some(now);
        }
    }

    fn expired(&self, now: u64) -> bool {
        self.remaining_ms(now) == Some(0)
    }

    /// The clock goes on after a player's buzz was judged wrong; presses
    /// received while that player answered do not count.
    fn go_on_after_buzz(&mut self, now: u64) {
        self.resume(now);
        self.presses_from_ms = now;
    }

    /// A press received at `now` was made while this clock ran or stood
    /// paused, and before time up. Presses can be applied a moment after they
    /// were received, so this goes by `now` alone: one received while the
    /// quizmaster paused counts even when applied after the resume, and one
    /// received before the question started or while another team's buzz
    /// held the clock does not.
    fn takes_press_at(&self, now: u64) -> bool {
        !self.expired(now) && now >= self.presses_from_ms
    }
}

/// What a press does in the running question (`Game::press_play`).
enum PressPlay {
    /// An answer in an everyone-answers question, given `elapsed_ms` into it.
    Answer { elapsed_ms: u64 },
    /// The buzz in a fastest-finger question; `clock` is the question's.
    Buzz { clock: Clock },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "step", rename_all = "camelCase")]
pub enum QuestionStep {
    /// Question on screen, clock not started.
    Ready,
    Running { clock: Clock },
    /// Fastest finger: a player buzzed with `button`, which is their answer
    /// on a question whose answers each have a button; the clock is paused
    /// until judged.
    #[serde(rename_all = "camelCase")]
    Buzzed { buzzer: BuzzerId, button: Button, clock: Clock },
    /// Answers are in; points are applied.
    Closed,
    /// The correct answer is on screen. In fastest finger, `winner` is the
    /// buzzer judged right, if any.
    Revealed { winner: Option<BuzzerId> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum Phase {
    /// Before the first slide: buzzers register by pressing any button.
    Lobby,
    Billboard,
    Explanation,
    EndOfRound,
    Question(QuestionStep),
    Final { step: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub buzzer: BuzzerId,
    /// Empty until the quizmaster names the player; the UI shows the buzzer number.
    pub name: String,
    pub score: i32,
    pub enabled: bool,
    /// How long the player took for their right answers, together; breaks
    /// ties in the ranking (`Game::ranking`).
    pub total_answer_ms: u64,
}

impl Player {
    /// Scores never wrap around, whatever the quizmaster adds.
    fn add_score(&mut self, delta: i32) {
        self.score = self.score.saturating_add(delta);
    }

    /// What places the player, best first: the higher score, then the less
    /// answering time.
    fn standing(&self) -> (Reverse<i32>, u64) {
        (Reverse(self.score), self.total_answer_ms)
    }
}

/// A name as typed, without surrounding spaces, composed (`text::composed`)
/// and at most `MAX_NAME_CHARS` long.
fn clean_name(name: &str) -> String {
    composed(name.trim()).chars().take(MAX_NAME_CHARS).collect::<String>().trim_end().to_string()
}

/// The team names to prepare from `names` as given: trimmed and cut to
/// `MAX_NAME_CHARS`, without blanks or repeats (`same_name`), in their order.
/// More than `MAX_TEAMS` is refused as soon as it is seen, so a wrong file of
/// thousands of lines costs no more than reading `MAX_TEAMS` of them.
pub fn team_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>, GameError> {
    let mut seen = HashSet::new();
    let mut teams = Vec::new();
    for name in names.into_iter().map(clean_name) {
        if name.is_empty() || !seen.insert(name_key(&name)) {
            continue;
        }
        if teams.len() == MAX_TEAMS {
            return Err(GameError::TooManyTeams);
        }
        teams.push(name);
    }
    Ok(teams)
}

/// Two names are the same team when they differ only in case or in how
/// their accents are encoded (`text::name_key`); an empty name is nobody's.
pub fn same_name(a: &str, b: &str) -> bool {
    !a.is_empty() && name_key(a) == name_key(b)
}

/// Two phases of a slide are the same step as the quizmaster sees it: a
/// clock paused or resumed is, another buzzer buzzed in or another place of
/// the ranking shown is not.
fn same_step(a: &Phase, b: &Phase) -> bool {
    match (a, b) {
        (Phase::Question(QuestionStep::Buzzed { buzzer: x, .. }), Phase::Question(QuestionStep::Buzzed { buzzer: y, .. })) => x == y,
        (Phase::Question(x), Phase::Question(y)) => std::mem::discriminant(x) == std::mem::discriminant(y),
        _ => a == b,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Response {
    pub button: Button,
    pub elapsed_ms: u64,
}

impl Response {
    /// Whether the answer is one of `question`'s right ones.
    pub fn is_right(&self, question: &Question) -> bool {
        self.button.answer_index().is_some_and(|i| question.is_correct(i))
    }
}

/// What the current question gave a player, which `RestartQuestion` takes back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Gain {
    points: i32,
    answer_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    quiz: Quiz,
    slide: usize,
    phase: Phase,
    players: BTreeMap<BuzzerId, Player>,
    /// Everyone-answers: the first answer of each player in the current question.
    responses: BTreeMap<BuzzerId, Response>,
    /// Everyone-answers: the players the closed question counts, those
    /// switched on when it closed. Its scores, results, answer counts and
    /// lights go by them alone, whoever is switched on or off later.
    counted: BTreeSet<BuzzerId>,
    /// Fastest finger: players who answered wrong in the current question.
    locked_out: BTreeSet<BuzzerId>,
    /// Fastest finger: players switched on or registered while the current
    /// question was under way (rules version 5). The others' buzzer timers
    /// count from when they lit ready, by which a buzz is ordered (BZ-5):
    /// theirs would count from later, so they play from the next question.
    joins_next: BTreeSet<BuzzerId>,
    /// What the current question gave each player (`credit`).
    gains: BTreeMap<BuzzerId, Gain>,
    /// The slides whose question ended in this show (`end_question`). Going to one again
    /// shows its answer (rules version 6): its scores stand, and asking it
    /// again would score it twice.
    closed_questions: BTreeSet<usize>,
    /// When the step the quiz's settings take by itself (`pending_step`)
    /// began to count down: when the current question closed for having
    /// nobody left to answer, or when its answer was shown. None once the
    /// quizmaster holds it.
    auto_step_from: Option<u64>,
    leaderboard_visible: bool,
    /// Team names prepared for the show; a team is paired with the buzzer
    /// whose player has its name (`view::TeamView`).
    teams: Vec<String>,
    /// How many times the quizmaster started the current slide's video
    /// (`Command::PlayVideo`); the projector starts it whenever this grows.
    video_plays: u32,
    /// How many steps the show has taken (`same_step`). A window sends the
    /// one it shows with a command meant for it, which is dropped once the
    /// show has taken another: one step per press, also when the show took
    /// one by itself or another window did just before.
    step_no: u32,
    /// The `RULES_VERSION` the show runs under: today's for a new show, its
    /// journal's for a replayed one (`replay`). A show resumed after an
    /// update goes on under the rules it began with, so its whole journal,
    /// from before the update and after, replays into the same show.
    rules_version: u32,
}

impl Game {
    pub fn new(quiz: Quiz) -> Game {
        Game {
            quiz,
            slide: 0,
            phase: Phase::Lobby,
            players: BTreeMap::new(),
            responses: BTreeMap::new(),
            counted: BTreeSet::new(),
            locked_out: BTreeSet::new(),
            joins_next: BTreeSet::new(),
            gains: BTreeMap::new(),
            closed_questions: BTreeSet::new(),
            auto_step_from: None,
            leaderboard_visible: false,
            teams: Vec::new(),
            video_plays: 0,
            step_no: 0,
            rules_version: RULES_VERSION,
        }
    }

    /// Rebuilds a game from a journal begun under `rules_version`, and keeps
    /// running it under that version. Commands that fail are skipped,
    /// exactly as they were rejected when first applied.
    pub fn replay<'a>(quiz: Quiz, journal: impl IntoIterator<Item = &'a Stamped>, rules_version: u32) -> Game {
        let mut game = Game { rules_version, ..Game::new(quiz) };
        for stamped in journal {
            let _ = game.apply(stamped);
        }
        game
    }

    /// After a crash the clock of a running question is meaningless, so the
    /// question starts again (as in the original, §9.1). A step the show
    /// was to take by itself is held, so it waits for the quizmaster, who
    /// first sees where the show stands (`pending_step`). Returns the
    /// command to apply and journal, if one is needed.
    pub fn recovery_command(&self) -> Option<Command> {
        if self.pending_step().is_some() {
            return Some(Command::PauseClock);
        }
        self.question_in_play().then_some(Command::RestartQuestion)
    }

    /// A question's clock runs (or stands paused), or a buzz waits for the
    /// quizmaster's judgement: leaving the slide would throw the question's
    /// score changes away with it.
    fn question_in_play(&self) -> bool {
        matches!(self.phase, Phase::Question(QuestionStep::Running { .. } | QuestionStep::Buzzed { .. }))
    }

    /// The question can start again: it is in play. Once it closed the
    /// projector shows its results, which would give the answer away.
    /// Before rules version 4 it also restarted closed, until the reveal.
    fn question_restartable(&self) -> bool {
        let closed = self.phase == Phase::Question(QuestionStep::Closed);
        self.question_in_play() || (self.rules_version < 4 && closed)
    }

    pub fn quiz(&self) -> &Quiz {
        &self.quiz
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn slide_index(&self) -> usize {
        self.slide
    }

    /// The slide on screen; none in the lobby and in the final ranking that
    /// follows the last slide of a quiz without a final slide.
    pub fn current_slide(&self) -> Option<&Slide> {
        match self.phase {
            Phase::Lobby => None,
            _ => self.quiz.slides.get(self.slide),
        }
    }

    /// The slide `Next` goes to once the current one is done: the one after
    /// it, or after the last slide the final ranking, unless the quiz has a
    /// final slide of its own (it then ends with its last slide). None when
    /// the show is over.
    pub fn following(&self) -> Option<usize> {
        let next = match self.phase {
            Phase::Lobby => 0,
            _ => self.slide + 1,
        };
        let count = self.quiz.slides.len();
        (next < count || (next == count && self.quiz.final_slide().is_none())).then_some(next)
    }

    /// The slide the quizmaster's Back goes to (`GoTo`): the one before
    /// the slide on screen, the last one from the final ranking after it.
    /// None on the first slide, before it and while a question is in play.
    pub fn previous(&self) -> Option<usize> {
        if self.phase == Phase::Lobby || !self.can_jump() {
            return None;
        }
        self.slide.checked_sub(1)
    }

    /// The next `Next` leaves the current slide (rather than moving on within it).
    fn next_leaves_slide(&self) -> bool {
        match &self.phase {
            Phase::Lobby | Phase::Billboard | Phase::Explanation | Phase::EndOfRound => true,
            Phase::Question(step) => matches!(step, QuestionStep::Revealed { .. }),
            Phase::Final { step } => *step >= FINAL_LAST_STEP,
        }
    }

    pub fn players(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
    }

    pub fn teams(&self) -> &[String] {
        &self.teams
    }

    pub fn responses(&self) -> &BTreeMap<BuzzerId, Response> {
        &self.responses
    }

    pub fn locked_out(&self) -> &BTreeSet<BuzzerId> {
        &self.locked_out
    }

    /// See `joins_next`.
    pub fn joins_next(&self) -> &BTreeSet<BuzzerId> {
        &self.joins_next
    }

    /// Whether the player has no buzz left in the current fastest-finger
    /// question: they answered it wrong, play from the next one, or its one
    /// buzz was judged wrong (`QuizSettings::one_buzz`).
    fn sits_out(&self, buzzer: BuzzerId) -> bool {
        let buzz_used = self.quiz.settings.one_buzz && !self.locked_out.is_empty();
        buzz_used || self.locked_out.contains(&buzzer) || self.joins_next.contains(&buzzer)
    }

    /// Whether a player who joins now plays from the next question
    /// (`joins_next`).
    fn joins_at_next_question(&self) -> bool {
        let fastest_finger = self.question().is_some_and(|q| q.mode == AnswerMode::FastestFinger);
        self.rules_version >= 5 && fastest_finger && self.question_in_play()
    }

    /// See `counted`: empty until an everyone-answers question closes.
    pub fn counted(&self) -> &BTreeSet<BuzzerId> {
        &self.counted
    }

    /// The points the current question gave `buzzer`'s player, for feedback on screen.
    pub fn delta(&self, buzzer: BuzzerId) -> Option<i32> {
        self.gains.get(&buzzer).map(|gain| gain.points)
    }

    pub fn leaderboard_visible(&self) -> bool {
        self.leaderboard_visible
    }

    /// The players switched on, each with their place: by score (high
    /// first), then by the time their right answers took (less first).
    /// Players equal in both share a place, and as many places as share it
    /// are skipped after it (1, 1, 3); they are listed by buzzer.
    pub fn ranking(&self) -> Vec<(usize, &Player)> {
        let mut players: Vec<&Player> = self.players.values().filter(|p| p.enabled).collect();
        players.sort_by_key(|p| (p.standing(), p.buzzer));
        let mut ranking: Vec<(usize, &Player)> = Vec::with_capacity(players.len());
        for (i, player) in players.into_iter().enumerate() {
            let place = match ranking.last() {
                Some(&(place, above)) if above.standing() == player.standing() => place,
                _ => i + 1,
            };
            ranking.push((place, player));
        }
        ranking
    }

    pub fn clock(&self) -> Option<&Clock> {
        match &self.phase {
            Phase::Question(QuestionStep::Running { clock } | QuestionStep::Buzzed { clock, .. }) => Some(clock),
            _ => None,
        }
    }

    /// The quizmaster commands the current step takes; `apply` refuses the
    /// others with `NotNow`. Buttons and keys offer only these.
    pub fn controls(&self) -> Controls {
        let step = match &self.phase {
            Phase::Question(step) => Some(step),
            _ => None,
        };
        let running = match step {
            Some(QuestionStep::Running { clock }) => Some(clock),
            _ => None,
        };
        let buzzed = matches!(step, Some(QuestionStep::Buzzed { .. }));
        let show_over = self.next_leaves_slide() && self.following().is_none();
        Controls {
            next: !buzzed && !show_over,
            judge: buzzed,
            pause_clock: running.is_some_and(Clock::is_running) || self.pending_step().is_some(),
            resume_clock: running.is_some_and(|clock| !clock.is_running()),
            close_question: running.is_some(),
            play_video: self.shows_video(),
            go_to: self.can_jump(),
            restart_question: self.question_restartable(),
        }
    }

    /// True when the screen shows the current slide's video: a slide with a
    /// video whose screen has room for it (a ranking has none).
    fn shows_video(&self) -> bool {
        let screen = matches!(self.phase, Phase::Billboard | Phase::Explanation | Phase::Question(_));
        screen && self.current_slide().is_some_and(|slide| slide.video.is_some())
    }

    /// See `video_plays`.
    pub fn video_plays(&self) -> u32 {
        self.video_plays
    }

    /// See `step_no`.
    pub fn step_no(&self) -> u32 {
        self.step_no
    }

    /// True while the show holds still: a player is answering, the question
    /// clock is paused, or the question is over. A slide's video and a synced
    /// background video (BG-4) pause then.
    pub fn on_hold(&self) -> bool {
        match &self.phase {
            Phase::Question(QuestionStep::Running { clock }) => !clock.is_running(),
            Phase::Question(QuestionStep::Buzzed { .. } | QuestionStep::Closed | QuestionStep::Revealed { .. }) => true,
            _ => false,
        }
    }

    /// A command the app must issue on its own at `now`, such as time up
    /// (`PRESS_GRACE_MS` after the clock runs out), or the step the quiz's
    /// settings take by themselves (`pending_step`).
    pub fn due_command(&self, now: u64) -> Option<Command> {
        match &self.phase {
            Phase::Question(QuestionStep::Running { clock }) if clock.expired(now.saturating_sub(PRESS_GRACE_MS)) => {
                Some(Command::CloseQuestion)
            }
            _ => {
                let (_, command, at) = self.pending_step()?;
                (now >= at).then_some(command)
            }
        }
    }

    /// The quizmaster took over: the step the show was to take by itself
    /// (`pending_step`) waits for their Next.
    fn hold_pending_step(&mut self) {
        self.auto_step_from = None;
    }

    /// The step the show takes by itself, and when: once a question closed
    /// for having nobody left to answer (`QuizSettings::all_answered`), or
    /// once its answer was shown (`QuizSettings::next_after_reveal`), the
    /// settings' wait after `auto_step_from`. The quizmaster can take a
    /// step first, or hold it (`PauseClock`). Gives the step, the command
    /// that takes it and its time.
    pub(crate) fn pending_step(&self) -> Option<(AutoStep, Command, u64)> {
        let settings = self.quiz.settings;
        let (step, wait_ms) = match self.phase {
            Phase::Question(QuestionStep::Closed) => (settings.all_answered.auto_step()?, settings.all_answered_delay_ms()),
            Phase::Question(QuestionStep::Revealed { .. }) if settings.next_after_reveal => {
                (AutoStep::NextSlide, settings.next_after_reveal_delay_ms())
            }
            _ => return None,
        };
        let at = self.auto_step_from?.saturating_add(wait_ms);
        let command = match step {
            AutoStep::Reveal => Command::Next,
            // As Next goes on from the slide, past the last to the final ranking.
            AutoStep::NextSlide => match self.following()? {
                slide if slide < self.quiz.slides.len() => Command::GoTo { slide },
                _ => Command::GoToFinal,
            },
        };
        Some((step, command, at))
    }

    pub(crate) fn question(&self) -> Option<&Question> {
        match &self.quiz.slides.get(self.slide)?.content {
            SlideContent::Question(q) => Some(q),
            _ => None,
        }
    }

    fn enabled_buzzers(&self) -> impl Iterator<Item = BuzzerId> + '_ {
        self.players.values().filter(|p| p.enabled).map(|p| p.buzzer)
    }

    /// The light a switched-on buzzer shows in the current step: ready while
    /// its player can press (in the lobby to register), locked once they
    /// answered or answered wrong or another player buzzed, first for the
    /// buzz being judged, and from the reveal the result: right or wrong
    /// for the players an everyone-answers question counted, right for the
    /// fastest finger judged right. Dark otherwise, also once a question
    /// closed, until its reveal. A switched-off buzzer shows locked. A
    /// buzz judged wrong shows wrong until the step after it (`judge`).
    fn light_of(&self, buzzer: BuzzerId) -> Light {
        match &self.phase {
            Phase::Lobby => Light::Ready,
            Phase::Question(QuestionStep::Running { .. }) => {
                let done = self.responses.contains_key(&buzzer) || self.sits_out(buzzer);
                if done { Light::Locked } else { Light::Ready }
            }
            Phase::Question(QuestionStep::Buzzed { buzzer: first, .. }) => {
                if *first == buzzer { Light::First } else { Light::Locked }
            }
            Phase::Question(QuestionStep::Revealed { winner: Some(winner) }) => {
                if *winner == buzzer { Light::Correct } else { Light::Locked }
            }
            Phase::Question(QuestionStep::Revealed { winner: None }) if self.counted.contains(&buzzer) => {
                let right = self.question().zip(self.responses.get(&buzzer)).is_some_and(|(q, r)| r.is_right(q));
                if right { Light::Correct } else { Light::Wrong }
            }
            _ => Light::Off,
        }
    }

    /// What the buzzers take in the current step, as a whole: the steps of
    /// `light_of`, in which a buzzer can be ready, lit for the buzz being
    /// judged or show a result.
    pub fn buzzer_mode(&self) -> BuzzerMode {
        match &self.phase {
            Phase::Lobby => BuzzerMode::Register,
            Phase::Question(QuestionStep::Running { .. }) => match self.question().map(|q| q.mode) {
                Some(AnswerMode::Everyone) => BuzzerMode::Answers,
                Some(AnswerMode::FastestFinger) => BuzzerMode::FastestFinger,
                None => BuzzerMode::Off,
            },
            Phase::Question(QuestionStep::Buzzed { .. }) => BuzzerMode::Buzzed,
            Phase::Question(QuestionStep::Revealed { winner }) if winner.is_some() || !self.counted.is_empty() => BuzzerMode::Results,
            _ => BuzzerMode::Off,
        }
    }

    /// Every player's light in the current step, switched-off ones locked:
    /// what the buzzers should show when a show begins or resumes.
    pub fn lights(&self) -> Vec<(BuzzerId, Light)> {
        self.players.values().map(|p| (p.buzzer, self.player_light(p.buzzer, p.enabled))).collect()
    }

    fn player_light(&self, buzzer: BuzzerId, enabled: bool) -> Light {
        if enabled { self.light_of(buzzer) } else { Light::Locked }
    }

    /// Every switched-on buzzer's light in the current step (`light_of`).
    fn lights_now(&self) -> Effect {
        Effect::Lights { lights: self.enabled_buzzers().map(|b| (b, self.light_of(b))).collect() }
    }

    /// Whether the running question has nobody left to wait for: every
    /// player switched on answered it, or in fastest finger answered it
    /// wrong.
    fn nobody_left_to_answer(&self) -> bool {
        self.enabled_buzzers().all(|b| self.responses.contains_key(&b) || self.sits_out(b))
    }

    /// Whether the running question closes now for having nobody left to
    /// answer it: not while a timed one's clock is to run out
    /// (`QuizSettings::run_clock_out`).
    fn closes_as_answered(&self) -> bool {
        let runs_out = self.quiz.settings.run_clock_out && self.clock().is_some_and(|clock| clock.limit_ms.is_some());
        !runs_out && self.nobody_left_to_answer()
    }

    pub fn apply(&mut self, stamped: &Stamped) -> Result<Vec<Effect>, GameError> {
        let (slide, phase) = (self.slide, self.phase.clone());
        let effects = self.apply_command(stamped)?;
        if self.slide != slide || !same_step(&self.phase, &phase) {
            self.step_no = self.step_no.wrapping_add(1);
        }
        Ok(effects)
    }

    fn apply_command(&mut self, stamped: &Stamped) -> Result<Vec<Effect>, GameError> {
        let now = stamped.at_ms;
        match &stamped.command {
            Command::Press { buzzer, button } => Ok(self.press(*buzzer, *button, now)),
            Command::Next => self.next(now),
            Command::GoTo { slide } => {
                if *slide >= self.quiz.slides.len() {
                    return Err(GameError::NoSuchSlide(*slide));
                }
                self.jump_to(*slide)
            }
            Command::GoToFinal => self.jump_to(self.final_index()),
            Command::PauseClock if self.pending_step().is_some() => {
                self.hold_pending_step();
                Ok(vec![])
            }
            Command::PauseClock => match &mut self.phase {
                Phase::Question(QuestionStep::Running { clock }) if clock.is_running() => {
                    clock.pause(now);
                    Ok(vec![])
                }
                _ => Err(GameError::NotNow),
            },
            Command::ResumeClock => match &mut self.phase {
                Phase::Question(QuestionStep::Running { clock }) if !clock.is_running() => {
                    clock.resume(now);
                    Ok(vec![])
                }
                _ => Err(GameError::NotNow),
            },
            Command::CloseQuestion => match &self.phase {
                Phase::Question(QuestionStep::Running { clock }) => {
                    let timed_out = clock.expired(now);
                    // A clock left to run out with nobody left to answer (`closes_as_answered`).
                    let answered = timed_out && self.quiz.settings.run_clock_out && self.nobody_left_to_answer();
                    let mut effects = if answered { self.close_answered(now) } else { self.close_question() };
                    if timed_out {
                        effects.push(Effect::Sound { cue: Cue::TimeUp });
                    }
                    Ok(effects)
                }
                _ => Err(GameError::NotNow),
            },
            Command::RestartQuestion => {
                if !self.question_restartable() {
                    return Err(GameError::NotNow);
                }
                for (buzzer, gain) in std::mem::take(&mut self.gains) {
                    if let Some(player) = self.players.get_mut(&buzzer) {
                        player.add_score(gain.points.saturating_neg());
                        player.total_answer_ms = player.total_answer_ms.saturating_sub(gain.answer_ms);
                    }
                }
                Ok(self.enter_slide(self.slide))
            }
            Command::JudgeRight => self.judge(true, now),
            Command::JudgeWrong => self.judge(false, now),
            Command::AddPlayer { buzzer, name } => {
                if self.players.contains_key(buzzer) {
                    return Err(GameError::DuplicateBuzzer(*buzzer));
                }
                Ok(vec![self.join(*buzzer, name.clone())])
            }
            Command::SetTeams { names } => {
                self.teams = team_names(names.iter().map(String::as_str))?;
                Ok(vec![])
            }
            Command::RenamePlayer { buzzer, name } | Command::PairTeam { buzzer, name } => {
                let name = clean_name(name);
                self.player_mut(*buzzer)?;
                for player in self.players.values_mut() {
                    if player.buzzer != *buzzer && same_name(&player.name, &name) {
                        player.name.clear();
                    }
                }
                self.player_mut(*buzzer)?.name = name;
                Ok(vec![])
            }
            Command::SetPlayerEnabled { buzzer, enabled } => self.set_enabled(*buzzer, *enabled, now),
            Command::AdjustScore { buzzer, delta } => {
                self.player_mut(*buzzer)?.add_score(*delta);
                Ok(vec![])
            }
            Command::ShowLeaderboard { visible } => {
                self.leaderboard_visible = *visible;
                if *visible {
                    self.hold_pending_step();
                }
                Ok(vec![])
            }
            Command::PlayVideo => {
                if !self.shows_video() {
                    return Err(GameError::NotNow);
                }
                self.video_plays = self.video_plays.saturating_add(1);
                self.hold_pending_step();
                Ok(vec![])
            }
        }
    }

    fn player_mut(&mut self, buzzer: BuzzerId) -> Result<&mut Player, GameError> {
        self.players.get_mut(&buzzer).ok_or(GameError::UnknownBuzzer(buzzer))
    }

    /// Switches a player on or off. A running question that then has nobody
    /// left to answer closes, as it would on the last answer. Switching on
    /// a player who is on changes nothing.
    fn set_enabled(&mut self, buzzer: BuzzerId, enabled: bool, now: u64) -> Result<Vec<Effect>, GameError> {
        let player = self.player_mut(buzzer)?;
        let switched_on = enabled && !player.enabled;
        player.enabled = enabled;
        if switched_on && self.joins_at_next_question() {
            self.joins_next.insert(buzzer);
        }
        let light = self.player_light(buzzer, enabled);
        let mut effects = vec![Effect::Lights { lights: vec![(buzzer, light)] }];
        // Before rules version 3 a switch never closed a question.
        let running = matches!(self.phase, Phase::Question(QuestionStep::Running { .. }));
        if self.rules_version >= 3 && running && self.closes_as_answered() {
            effects.extend(self.close_answered(now));
        }
        Ok(effects)
    }

    /// Registers a new player, switched on, and lights their buzzer. In a
    /// fastest-finger question under way they play from the next question,
    /// as a player switched on then does (`joins_next`).
    fn join(&mut self, buzzer: BuzzerId, name: String) -> Effect {
        self.players.entry(buzzer).or_insert(Player {
            buzzer,
            name: clean_name(&name),
            score: 0,
            enabled: true,
            total_answer_ms: 0,
        });
        if self.joins_at_next_question() {
            self.joins_next.insert(buzzer);
        }
        Effect::Lights { lights: vec![(buzzer, self.light_of(buzzer))] }
    }

    fn press(&mut self, buzzer: BuzzerId, button: Button, now: u64) -> Vec<Effect> {
        // Any press from an unknown buzzer registers it, also late arrivals.
        if !self.players.contains_key(&buzzer) {
            let light = self.join(buzzer, String::new());
            if self.phase == Phase::Lobby || self.joins_next.contains(&buzzer) {
                return vec![light];
            }
        }
        match self.press_play(buzzer, button, now) {
            None => vec![],
            Some(PressPlay::Answer { elapsed_ms }) => self.record_answer(buzzer, button, elapsed_ms, now),
            // The first press applied wins. Presses come in the receiver's order and the
            // PC clock never re-ranks them (BZ-5), not even against a keyboard press.
            Some(PressPlay::Buzz { mut clock }) => {
                clock.pause(now);
                self.phase = Phase::Question(QuestionStep::Buzzed { buzzer, button, clock });
                vec![self.lights_now(), Effect::Sound { cue: Cue::Buzz }]
            }
        }
    }

    /// What a press of a registered buzzer does in the running question, if
    /// anything. Decided without changing the show, so `ignores` and `press`
    /// cannot disagree.
    fn press_play(&self, buzzer: BuzzerId, button: Button, now: u64) -> Option<PressPlay> {
        if !self.players.get(&buzzer)?.enabled {
            return None;
        }
        let question = self.question()?;
        let Phase::Question(QuestionStep::Running { clock }) = self.phase else {
            return None;
        };
        if !clock.takes_press_at(now) {
            return None;
        }
        match question.mode {
            AnswerMode::Everyone => {
                let valid = button.answer_index().is_some_and(|i| i < question.answers.len());
                // The first answer counts, unless the quiz lets players change it.
                let takes = match self.responses.get(&buzzer) {
                    None => true,
                    Some(answer) => self.quiz.settings.change_answer && answer.button != button,
                };
                (valid && takes).then(|| PressPlay::Answer { elapsed_ms: clock.elapsed_ms(now) })
            }
            AnswerMode::FastestFinger => {
                let buzzes = self.rules_version < 5 || button.buzzes_in(question);
                (buzzes && !self.sits_out(buzzer)).then_some(PressPlay::Buzz { clock })
            }
        }
    }

    /// Whether applying `stamped` would leave the show as it is: a press that
    /// does not play, such as a second answer or one after time up. The app
    /// neither journals nor shows such a press, so a burst of them
    /// cannot hold up the show. A press of a new buzzer always counts, as it
    /// registers the buzzer.
    pub fn ignores(&self, stamped: &Stamped) -> bool {
        match stamped.command {
            Command::Press { buzzer, button } => {
                self.players.contains_key(&buzzer) && self.press_play(buzzer, button, stamped.at_ms).is_none()
            }
            _ => false,
        }
    }

    fn record_answer(&mut self, buzzer: BuzzerId, button: Button, elapsed_ms: u64, now: u64) -> Vec<Effect> {
        self.responses.insert(buzzer, Response { button, elapsed_ms });
        let mut effects = vec![Effect::Lights { lights: vec![(buzzer, self.light_of(buzzer))] }];
        if self.closes_as_answered() {
            effects.extend(self.close_answered(now));
        }
        effects
    }

    /// Closes the running question at `now` for having nobody left to
    /// answer it, for the step the quiz's settings then take (`pending_step`).
    fn close_answered(&mut self, now: u64) -> Vec<Effect> {
        self.auto_step_from = Some(now);
        self.close_question()
    }

    /// Closes the running question. An everyone-answers question counts
    /// the players switched on now (`counted`) and scores their answers;
    /// a right answer adds its time to the player's answering time.
    fn close_question(&mut self) -> Vec<Effect> {
        let Some(q) = self.question().cloned() else {
            return vec![];
        };
        self.end_question(QuestionStep::Closed);
        if q.mode == AnswerMode::Everyone {
            self.counted = self.enabled_buzzers().collect();
            for buzzer in self.counted.clone() {
                match self.responses.get(&buzzer).copied() {
                    Some(response) => {
                        let answer_ms = if response.is_right(&q) { response.elapsed_ms } else { 0 };
                        self.credit(buzzer, q.points_for(response.button.answer_index(), response.elapsed_ms), answer_ms);
                    }
                    None if self.quiz.settings.no_answer_penalty => self.credit(buzzer, q.points_for(None, 0), 0),
                    None => {}
                }
            }
        }
        vec![self.lights_now()]
    }

    /// Ends the current question at `step`: closed, or won by a fastest
    /// finger judged right. Either way it counts as asked from now on
    /// (`closed_questions`).
    fn end_question(&mut self, step: QuestionStep) {
        self.phase = Phase::Question(step);
        self.closed_questions.insert(self.slide);
    }

    /// Gives `buzzer`'s player `points` and `answer_ms` of answering time
    /// from the current question, kept in `gains` for `RestartQuestion`.
    fn credit(&mut self, buzzer: BuzzerId, points: i32, answer_ms: u64) {
        let Some(player) = self.players.get_mut(&buzzer) else { return };
        player.add_score(points);
        player.total_answer_ms += answer_ms;
        let gain = self.gains.entry(buzzer).or_default();
        gain.points = gain.points.saturating_add(points);
        gain.answer_ms += answer_ms;
    }

    fn judge(&mut self, right: bool, now: u64) -> Result<Vec<Effect>, GameError> {
        let Phase::Question(QuestionStep::Buzzed { buzzer, clock, .. }) = self.phase else {
            return Err(GameError::NotNow);
        };
        let q = self.question().cloned().ok_or(GameError::NotNow)?;
        let elapsed = clock.elapsed_ms(now);
        if right {
            self.credit(buzzer, q.points_at(elapsed), elapsed);
            self.reveal(Some(buzzer), now);
            return Ok(vec![
                Effect::Lights { lights: vec![(buzzer, self.light_of(buzzer))] },
                Effect::Sound { cue: Cue::Correct },
            ]);
        }
        self.credit(buzzer, -q.penalty, 0);
        self.locked_out.insert(buzzer);
        // The buzz paused the clock before time up (`Clock::takes_press_at`),
        // so time is left for the others.
        let mut clock = clock;
        clock.go_on_after_buzz(now);
        self.phase = Phase::Question(QuestionStep::Running { clock });
        let mut effects = vec![
            Effect::Lights { lights: vec![(buzzer, Light::Wrong)] },
            Effect::Sound { cue: Cue::Wrong },
        ];
        let follow = if self.closes_as_answered() { self.close_answered(now) } else { vec![self.lights_now()] };
        // The others get the lights of the step that follows; the judged
        // buzzer shows wrong until the next step's lights.
        effects.extend(follow.into_iter().map(|effect| match effect {
            Effect::Lights { mut lights } => {
                lights.retain(|&(lit, _)| lit != buzzer);
                Effect::Lights { lights }
            }
            other => other,
        }));
        Ok(effects)
    }

    /// Shows the question's answer at `now`, which ends it if it had not
    /// closed (a fastest finger judged right), for the step the quiz's
    /// settings then take (`pending_step`).
    fn reveal(&mut self, winner: Option<BuzzerId>, now: u64) {
        self.end_question(QuestionStep::Revealed { winner });
        self.auto_step_from = Some(now);
    }

    fn next(&mut self, now: u64) -> Result<Vec<Effect>, GameError> {
        if self.next_leaves_slide() {
            let following = self.following().ok_or(GameError::NotNow)?;
            return Ok(self.enter_slide(following));
        }
        match self.phase.clone() {
            Phase::Lobby | Phase::Billboard | Phase::Explanation | Phase::EndOfRound => Err(GameError::NotNow),
            Phase::Question(step) => match step {
                QuestionStep::Ready => {
                    let limit = self.question().and_then(Question::time_limit_ms);
                    self.phase = Phase::Question(QuestionStep::Running { clock: Clock::start(limit, now) });
                    Ok(vec![self.lights_now(), Effect::Sound { cue: Cue::ClockStart }])
                }
                QuestionStep::Running { .. } => Ok(self.close_question()),
                QuestionStep::Buzzed { .. } => Err(GameError::NotNow),
                QuestionStep::Closed => {
                    self.reveal(None, now);
                    // Dark after a fastest-finger question nobody won, also
                    // the last buzz judged wrong (`judge`).
                    Ok(vec![self.lights_now(), Effect::Sound { cue: Cue::Reveal }])
                }
                QuestionStep::Revealed { .. } => Err(GameError::NotNow),
            },
            Phase::Final { step } => {
                self.phase = Phase::Final { step: step + 1 };
                Ok(vec![])
            }
        }
    }

    /// Where `GoToFinal` jumps: the final slide, or past the last slide.
    fn final_index(&self) -> usize {
        self.quiz.final_slide().unwrap_or(self.quiz.slides.len())
    }

    fn jump_to(&mut self, index: usize) -> Result<Vec<Effect>, GameError> {
        if !self.can_jump() {
            return Err(GameError::NotNow);
        }
        Ok(self.enter_slide(index))
    }

    /// Whether `GoTo` and `GoToFinal` apply: not while a question is in play.
    fn can_jump(&self) -> bool {
        self.jumps_out_of_a_question() || !self.question_in_play()
    }

    /// Whether a jump may leave a question in play, as before rules version
    /// 2: the app then restarted a question after a crash with a `GoTo` to it.
    fn jumps_out_of_a_question(&self) -> bool {
        self.rules_version < 2
    }

    fn enter_slide(&mut self, index: usize) -> Vec<Effect> {
        self.responses.clear();
        self.counted.clear();
        self.locked_out.clear();
        self.joins_next.clear();
        self.gains.clear();
        self.auto_step_from = None;
        self.leaderboard_visible = false;
        self.video_plays = 0;
        self.slide = index;
        self.phase = match self.quiz.slides.get(index).map(|s| &s.content) {
            None => Phase::Final { step: 0 },
            Some(SlideContent::Billboard { .. }) => Phase::Billboard,
            Some(SlideContent::Explanation { .. }) => Phase::Explanation,
            Some(SlideContent::EndOfRound { .. }) => Phase::EndOfRound,
            Some(SlideContent::Final { .. }) => Phase::Final { step: 0 },
            Some(SlideContent::Question(_)) if self.rules_version >= 6 && self.closed_questions.contains(&index) => {
                Phase::Question(QuestionStep::Revealed { winner: None })
            }
            Some(SlideContent::Question(_)) => Phase::Question(QuestionStep::Ready),
        };
        vec![self.lights_now()]
    }
}
