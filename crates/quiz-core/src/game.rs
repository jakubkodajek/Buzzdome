//! The live show: a deterministic state machine driven by timestamped commands.
//!
//! Every accepted command is journaled by the app, so replaying the journal on
//! the same quiz rebuilds the exact state after a crash. Time never comes from a
//! clock inside this module: each command carries its own `at_ms`.

use crate::model::{AnswerMode, Question, Quiz, Slide, SlideContent};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub type BuzzerId = u16;

/// The version of the rules a journal is replayed with. Bump it whenever
/// the same commands give another show: a resumed show begun under another
/// version is rebuilt with today's rules, but for what `Game::replay` keeps
/// of older ones, and the Director says its scores may differ. A command
/// that was refused before and is taken now needs no bump, as a refused
/// command is never journaled.
///
/// 1: refused presses that were journaled count (N3), and renaming a player
/// takes the name from another buzzer (R17). 2: a question in play restarts
/// with `RestartQuestion`, which takes its score changes back, and refuses
/// `GoTo` and `GoToFinal` (A3).
pub const RULES_VERSION: u32 = 2;

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

impl Button {
    pub fn answer_index(self) -> Option<usize> {
        match self {
            Button::A => Some(0),
            Button::B => Some(1),
            Button::C => Some(2),
            Button::D => Some(3),
            Button::Big => None,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
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

/// Side effects for the app to carry out after a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Effect {
    Lights { lights: Vec<(BuzzerId, Light)> },
    Sound { cue: Cue },
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
    PauseClock,
    ResumeClock,
    /// End the running question now (time up, or the quizmaster ends it).
    CloseQuestion,
    /// Start the question in play again, as after a crash
    /// (`Game::recovery_command`): its score changes are taken back, and it
    /// waits for `Next` to start its clock.
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
    /// and repeats (ignoring case) are left out.
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
    pub pause_clock: bool,
    pub resume_clock: bool,
    pub close_question: bool,
    pub play_video: bool,
    /// `GoTo` and `GoToFinal`: not while a question is in play.
    pub go_to: bool,
    /// `RestartQuestion`: while a question is in play, the way to leave it
    /// without scoring it, and once it closed, until its answer is shown,
    /// the way to take its scores back and ask it again.
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
    /// Fastest finger: a player buzzed; the clock is paused until judged.
    #[serde(rename_all = "camelCase")]
    Buzzed { buzzer: BuzzerId, clock: Clock },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub buzzer: BuzzerId,
    /// Empty until the quizmaster names the player; the UI shows the buzzer number.
    pub name: String,
    pub score: i32,
    pub enabled: bool,
    /// Sum of answering times; breaks ties in the ranking.
    pub total_answer_ms: u64,
}

impl Player {
    /// Scores never wrap around, whatever the quizmaster adds.
    fn add_score(&mut self, delta: i32) {
        self.score = self.score.saturating_add(delta);
    }
}

/// A name as typed, without surrounding spaces and at most `MAX_NAME_CHARS` long.
fn clean_name(name: &str) -> String {
    name.trim().chars().take(MAX_NAME_CHARS).collect::<String>().trim_end().to_string()
}

/// The team names to prepare from `names` as given: trimmed and cut to
/// `MAX_NAME_CHARS`, without blanks or repeats (`same_name`), in their order.
/// More than `MAX_TEAMS` is refused as soon as it is seen, so a wrong file of
/// thousands of lines costs no more than reading `MAX_TEAMS` of them.
pub fn team_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>, GameError> {
    let mut seen = HashSet::new();
    let mut teams = Vec::new();
    for name in names.into_iter().map(clean_name) {
        if name.is_empty() || !seen.insert(name.to_lowercase()) {
            continue;
        }
        if teams.len() == MAX_TEAMS {
            return Err(GameError::TooManyTeams);
        }
        teams.push(name);
    }
    Ok(teams)
}

/// Two names are the same team when they differ only in case; an empty
/// name is nobody's.
pub fn same_name(a: &str, b: &str) -> bool {
    !a.is_empty() && a.to_lowercase() == b.to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub button: Button,
    pub elapsed_ms: u64,
}

/// What the current question gave a player, which `RestartQuestion` takes back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Gain {
    points: i32,
    answer_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    quiz: Quiz,
    slide: usize,
    phase: Phase,
    players: BTreeMap<BuzzerId, Player>,
    /// Everyone-answers: the first answer of each player in the current question.
    responses: BTreeMap<BuzzerId, Response>,
    /// Fastest finger: players who answered wrong in the current question.
    locked_out: BTreeSet<BuzzerId>,
    /// What the current question gave each player (`credit`).
    gains: BTreeMap<BuzzerId, Gain>,
    leaderboard_visible: bool,
    /// Team names prepared for the show; a team is paired with the buzzer
    /// whose player has its name (`view::TeamView`).
    teams: Vec<String>,
    /// How many times the quizmaster started the current slide's video
    /// (`Command::PlayVideo`); the projector starts it whenever this grows.
    video_plays: u32,
}

impl Game {
    pub fn new(quiz: Quiz) -> Game {
        Game {
            quiz,
            slide: 0,
            phase: Phase::Lobby,
            players: BTreeMap::new(),
            responses: BTreeMap::new(),
            locked_out: BTreeSet::new(),
            gains: BTreeMap::new(),
            leaderboard_visible: false,
            teams: Vec::new(),
            video_plays: 0,
        }
    }

    /// Rebuilds a game from a journal begun under `rules_version`. Commands
    /// that fail are skipped, exactly as they were rejected when first applied.
    pub fn replay<'a>(quiz: Quiz, journal: impl IntoIterator<Item = &'a Stamped>, rules_version: u32) -> Game {
        let mut game = Game::new(quiz);
        for stamped in journal {
            let _ = game.apply_under(stamped, rules_version);
        }
        game
    }

    /// `apply`, for a journal begun under `rules_version`. Before version 2
    /// a jump left a question in play, and the app restarted one after a
    /// crash with a `GoTo` to it: replayed with today's refusal, every later
    /// `Next` would land a step off (G11).
    fn apply_under(&mut self, stamped: &Stamped, rules_version: u32) -> Result<Vec<Effect>, GameError> {
        match stamped.command {
            Command::GoTo { slide } if rules_version < 2 && slide < self.quiz.slides.len() => Ok(self.enter_slide(slide)),
            Command::GoToFinal if rules_version < 2 => Ok(self.enter_slide(self.final_index())),
            _ => self.apply(stamped),
        }
    }

    /// After a crash the clock of a running question is meaningless, so the
    /// question starts again (as in the original, §9.1). Returns the command
    /// to apply and journal, if one is needed.
    pub fn recovery_command(&self) -> Option<Command> {
        self.question_in_play().then_some(Command::RestartQuestion)
    }

    /// A question's clock runs (or stands paused), or a buzz waits for the
    /// quizmaster's judgement: leaving the slide would throw the question's
    /// score changes away with it.
    fn question_in_play(&self) -> bool {
        matches!(self.phase, Phase::Question(QuestionStep::Running { .. } | QuestionStep::Buzzed { .. }))
    }

    /// The question can start again: it is in play, or it closed and its
    /// answer is not shown yet. So a restart the quizmaster asked for still
    /// works when time ran out while they confirmed it (J20).
    fn question_restartable(&self) -> bool {
        self.question_in_play() || self.phase == Phase::Question(QuestionStep::Closed)
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

    /// The points the current question gave `buzzer`'s player, for feedback on screen.
    pub fn delta(&self, buzzer: BuzzerId) -> Option<i32> {
        self.gains.get(&buzzer).map(|gain| gain.points)
    }

    pub fn leaderboard_visible(&self) -> bool {
        self.leaderboard_visible
    }

    /// Players by score (high first), then total answering time, then buzzer.
    pub fn ranking(&self) -> Vec<&Player> {
        let mut players: Vec<&Player> = self.players.values().filter(|p| p.enabled).collect();
        players.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then(a.total_answer_ms.cmp(&b.total_answer_ms))
                .then(a.buzzer.cmp(&b.buzzer))
        });
        players
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
            pause_clock: running.is_some_and(Clock::is_running),
            resume_clock: running.is_some_and(|clock| !clock.is_running()),
            close_question: running.is_some(),
            play_video: self.shows_video(),
            go_to: !self.question_in_play(),
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
    /// (`PRESS_GRACE_MS` after the clock runs out).
    pub fn due_command(&self, now: u64) -> Option<Command> {
        match &self.phase {
            Phase::Question(QuestionStep::Running { clock }) if clock.expired(now.saturating_sub(PRESS_GRACE_MS)) => {
                Some(Command::CloseQuestion)
            }
            _ => None,
        }
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

    fn lights_all(&self, light: Light) -> Effect {
        Effect::Lights { lights: self.enabled_buzzers().map(|b| (b, light)).collect() }
    }

    /// Lights while a fastest-finger question waits for a buzz.
    fn lights_waiting(&self) -> Effect {
        let lights = self
            .enabled_buzzers()
            .map(|b| (b, if self.locked_out.contains(&b) { Light::Locked } else { Light::Ready }))
            .collect();
        Effect::Lights { lights }
    }

    pub fn apply(&mut self, stamped: &Stamped) -> Result<Vec<Effect>, GameError> {
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
                    let mut effects = self.close_question();
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
                self.register(*buzzer, name.clone());
                Ok(vec![])
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
            Command::SetPlayerEnabled { buzzer, enabled } => {
                self.player_mut(*buzzer)?.enabled = *enabled;
                let light = if *enabled { Light::Off } else { Light::Locked };
                Ok(vec![Effect::Lights { lights: vec![(*buzzer, light)] }])
            }
            Command::AdjustScore { buzzer, delta } => {
                self.player_mut(*buzzer)?.add_score(*delta);
                Ok(vec![])
            }
            Command::ShowLeaderboard { visible } => {
                self.leaderboard_visible = *visible;
                Ok(vec![])
            }
            Command::PlayVideo => {
                if !self.shows_video() {
                    return Err(GameError::NotNow);
                }
                self.video_plays = self.video_plays.saturating_add(1);
                Ok(vec![])
            }
        }
    }

    fn player_mut(&mut self, buzzer: BuzzerId) -> Result<&mut Player, GameError> {
        self.players.get_mut(&buzzer).ok_or(GameError::UnknownBuzzer(buzzer))
    }

    fn register(&mut self, buzzer: BuzzerId, name: String) {
        self.players.entry(buzzer).or_insert(Player {
            buzzer,
            name: clean_name(&name),
            score: 0,
            enabled: true,
            total_answer_ms: 0,
        });
    }

    fn press(&mut self, buzzer: BuzzerId, button: Button, now: u64) -> Vec<Effect> {
        // Any press from an unknown buzzer registers it, also late arrivals.
        if !self.players.contains_key(&buzzer) {
            self.register(buzzer, String::new());
            if self.phase == Phase::Lobby {
                return vec![Effect::Lights { lights: vec![(buzzer, Light::Ready)] }];
            }
        }
        match self.press_play(buzzer, button, now) {
            None => vec![],
            Some(PressPlay::Answer { elapsed_ms }) => self.record_answer(buzzer, button, elapsed_ms),
            // The first press applied wins. Presses come in the receiver's order and the
            // PC clock never re-ranks them (BZ-5), not even against a keyboard press.
            Some(PressPlay::Buzz { mut clock }) => {
                clock.pause(now);
                self.phase = Phase::Question(QuestionStep::Buzzed { buzzer, clock });
                let lights = self
                    .enabled_buzzers()
                    .map(|b| (b, if b == buzzer { Light::First } else { Light::Locked }))
                    .collect();
                vec![Effect::Lights { lights }, Effect::Sound { cue: Cue::Buzz }]
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
                // The first answer counts; changing it is not allowed.
                (valid && !self.responses.contains_key(&buzzer)).then(|| PressPlay::Answer { elapsed_ms: clock.elapsed_ms(now) })
            }
            AnswerMode::FastestFinger => (!self.locked_out.contains(&buzzer)).then_some(PressPlay::Buzz { clock }),
        }
    }

    /// Whether applying `stamped` would leave the show as it is: a press that
    /// does not play, such as a second answer or one after time up. The app
    /// neither journals nor shows such a press (A8), so a burst of them
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

    fn record_answer(&mut self, buzzer: BuzzerId, button: Button, elapsed_ms: u64) -> Vec<Effect> {
        self.responses.insert(buzzer, Response { button, elapsed_ms });
        let mut effects = vec![Effect::Lights { lights: vec![(buzzer, Light::Locked)] }];
        if self.enabled_buzzers().all(|b| self.responses.contains_key(&b)) {
            effects.extend(self.close_question());
        }
        effects
    }

    fn close_question(&mut self) -> Vec<Effect> {
        let Some(q) = self.question().cloned() else {
            return vec![];
        };
        self.phase = Phase::Question(QuestionStep::Closed);
        if q.mode != AnswerMode::Everyone {
            return vec![self.lights_all(Light::Off)];
        }
        for (buzzer, response) in self.responses.clone() {
            if self.players.get(&buzzer).is_some_and(|player| player.enabled) {
                self.credit(buzzer, q.points_for(response.button.answer_index(), response.elapsed_ms), response.elapsed_ms);
            }
        }
        // The lights show the result only at the reveal; until then they go dark.
        vec![self.lights_all(Light::Off)]
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
        let Phase::Question(QuestionStep::Buzzed { buzzer, clock }) = self.phase else {
            return Err(GameError::NotNow);
        };
        let q = self.question().cloned().ok_or(GameError::NotNow)?;
        let elapsed = clock.elapsed_ms(now);
        if right {
            self.credit(buzzer, q.points_at(elapsed), elapsed);
            self.phase = Phase::Question(QuestionStep::Revealed { winner: Some(buzzer) });
            return Ok(vec![
                Effect::Lights { lights: vec![(buzzer, Light::Correct)] },
                Effect::Sound { cue: Cue::Correct },
            ]);
        }
        self.credit(buzzer, -q.penalty, 0);
        self.locked_out.insert(buzzer);
        let mut effects = vec![
            Effect::Lights { lights: vec![(buzzer, Light::Wrong)] },
            Effect::Sound { cue: Cue::Wrong },
        ];
        let anyone_left = self.enabled_buzzers().any(|b| !self.locked_out.contains(&b));
        if !anyone_left || clock.expired(now) {
            self.phase = Phase::Question(QuestionStep::Closed);
            return Ok(effects);
        }
        let mut clock = clock;
        clock.go_on_after_buzz(now);
        self.phase = Phase::Question(QuestionStep::Running { clock });
        effects.push(self.lights_waiting());
        Ok(effects)
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
                    Ok(vec![self.lights_waiting(), Effect::Sound { cue: Cue::ClockStart }])
                }
                QuestionStep::Running { .. } => Ok(self.close_question()),
                QuestionStep::Buzzed { .. } => Err(GameError::NotNow),
                QuestionStep::Closed => {
                    self.phase = Phase::Question(QuestionStep::Revealed { winner: None });
                    let mut effects = vec![Effect::Sound { cue: Cue::Reveal }];
                    if let Some(lights) = self.result_lights() {
                        effects.insert(0, lights);
                    }
                    Ok(effects)
                }
                QuestionStep::Revealed { .. } => Err(GameError::NotNow),
            },
            Phase::Final { step } => {
                self.phase = Phase::Final { step: step + 1 };
                Ok(vec![])
            }
        }
    }

    /// Everyone-answers: green for right, red for wrong or no answer.
    fn result_lights(&self) -> Option<Effect> {
        let q = self.question()?;
        if q.mode != AnswerMode::Everyone {
            return None;
        }
        let lights = self
            .enabled_buzzers()
            .map(|b| {
                let right = self.responses.get(&b).and_then(|r| r.button.answer_index()).is_some_and(|i| q.is_correct(i));
                (b, if right { Light::Correct } else { Light::Wrong })
            })
            .collect();
        Some(Effect::Lights { lights })
    }

    /// Where `GoToFinal` jumps: the final slide, or past the last slide.
    fn final_index(&self) -> usize {
        self.quiz.final_slide().unwrap_or(self.quiz.slides.len())
    }

    fn jump_to(&mut self, index: usize) -> Result<Vec<Effect>, GameError> {
        if self.question_in_play() {
            return Err(GameError::NotNow);
        }
        Ok(self.enter_slide(index))
    }

    fn enter_slide(&mut self, index: usize) -> Vec<Effect> {
        self.responses.clear();
        self.locked_out.clear();
        self.gains.clear();
        self.leaderboard_visible = false;
        self.video_plays = 0;
        self.slide = index;
        self.phase = match self.quiz.slides.get(index).map(|s| &s.content) {
            None => Phase::Final { step: 0 },
            Some(SlideContent::Billboard { .. }) => Phase::Billboard,
            Some(SlideContent::Explanation { .. }) => Phase::Explanation,
            Some(SlideContent::EndOfRound { .. }) => Phase::EndOfRound,
            Some(SlideContent::Final { .. }) => Phase::Final { step: 0 },
            Some(SlideContent::Question(_)) => Phase::Question(QuestionStep::Ready),
        };
        vec![self.lights_all(Light::Off)]
    }
}
