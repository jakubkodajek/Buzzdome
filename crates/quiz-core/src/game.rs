//! The live show: a deterministic state machine driven by timestamped commands.
//!
//! Every accepted command is journaled by the app, so replaying the journal on
//! the same quiz rebuilds the exact state after a crash. Time never comes from a
//! clock inside this module: each command carries its own `at_ms`.

use crate::model::{AnswerMode, Question, Quiz, Slide, SlideContent};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use std::collections::{BTreeMap, BTreeSet};

pub type BuzzerId = u16;

/// Steps of the final score reveal: drum roll, 3rd, 2nd, 1st, full ranking.
pub const FINAL_LAST_STEP: u8 = 4;

/// How long after time up a question stays open, so that presses made in
/// time but still on their way from the receiver are counted. A press counts
/// by when it was received (its `at_ms`), not by when it is applied.
pub const PRESS_GRACE_MS: u64 = 250;

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
    /// Jump to a slide; a question there starts from the beginning.
    #[serde(rename_all = "camelCase")]
    GoTo { slide: usize },
    /// Skip the remaining slides and show the final scores.
    GoToFinal,
    PauseClock,
    ResumeClock,
    /// End the running question now (time up, or the quizmaster ends it).
    CloseQuestion,
    JudgeRight,
    JudgeWrong,
    #[serde(rename_all = "camelCase")]
    AddPlayer { buzzer: BuzzerId, name: String },
    #[serde(rename_all = "camelCase")]
    RenamePlayer { buzzer: BuzzerId, name: String },
    #[serde(rename_all = "camelCase")]
    SetPlayerEnabled { buzzer: BuzzerId, enabled: bool },
    #[serde(rename_all = "camelCase")]
    AdjustScore { buzzer: BuzzerId, delta: i32 },
    #[serde(rename_all = "camelCase")]
    ShowLeaderboard { visible: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stamped {
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
}

/// A countdown that can pause. `limit_ms == None` means untimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub limit_ms: Option<u64>,
    pub elapsed_before_ms: u64,
    pub running_since_ms: Option<u64>,
}

impl Clock {
    fn start(limit_ms: Option<u64>, now: u64) -> Clock {
        Clock { limit_ms, elapsed_before_ms: 0, running_since_ms: Some(now) }
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

    /// A press received at `now` was made while this clock ran (or stood
    /// paused) and before time up. Presses can be applied a moment after they
    /// were received, so one received before the clock last started (for
    /// example while another team's buzz held it) does not count.
    fn takes_press_at(&self, now: u64) -> bool {
        !self.expired(now) && self.running_since_ms.is_none_or(|since| now >= since)
    }
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
    /// The correct answer is on screen.
    Revealed,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub button: Button,
    pub elapsed_ms: u64,
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
    /// Score changes from the current question, for feedback on screen.
    deltas: BTreeMap<BuzzerId, i32>,
    leaderboard_visible: bool,
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
            deltas: BTreeMap::new(),
            leaderboard_visible: false,
        }
    }

    /// Rebuilds a game from a journal. Commands that fail are skipped, exactly
    /// as they were rejected when first applied.
    pub fn replay<'a>(quiz: Quiz, journal: impl IntoIterator<Item = &'a Stamped>) -> Game {
        let mut game = Game::new(quiz);
        for stamped in journal {
            let _ = game.apply(stamped);
        }
        game
    }

    /// After a crash the clock of a running question is meaningless, so the
    /// question starts again (as in the original, §9.1). Returns the command
    /// to apply and journal, if one is needed.
    pub fn recovery_command(&self) -> Option<Command> {
        match &self.phase {
            Phase::Question(QuestionStep::Running { .. } | QuestionStep::Buzzed { .. }) => {
                Some(Command::GoTo { slide: self.slide })
            }
            _ => None,
        }
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

    pub fn current_slide(&self) -> Option<&Slide> {
        match self.phase {
            Phase::Lobby | Phase::Final { .. } => None,
            _ => self.quiz.slides.get(self.slide),
        }
    }

    pub fn players(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
    }

    pub fn responses(&self) -> &BTreeMap<BuzzerId, Response> {
        &self.responses
    }

    pub fn locked_out(&self) -> &BTreeSet<BuzzerId> {
        &self.locked_out
    }

    pub fn deltas(&self) -> &BTreeMap<BuzzerId, i32> {
        &self.deltas
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
                Ok(self.enter_slide(*slide))
            }
            Command::GoToFinal => Ok(self.enter_slide(self.quiz.slides.len())),
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
            Command::JudgeRight => self.judge(true, now),
            Command::JudgeWrong => self.judge(false, now),
            Command::AddPlayer { buzzer, name } => {
                if self.players.contains_key(buzzer) {
                    return Err(GameError::DuplicateBuzzer(*buzzer));
                }
                self.register(*buzzer, name.clone());
                Ok(vec![])
            }
            Command::RenamePlayer { buzzer, name } => {
                self.player_mut(*buzzer)?.name = name.trim().to_string();
                Ok(vec![])
            }
            Command::SetPlayerEnabled { buzzer, enabled } => {
                self.player_mut(*buzzer)?.enabled = *enabled;
                let light = if *enabled { Light::Off } else { Light::Locked };
                Ok(vec![Effect::Lights { lights: vec![(*buzzer, light)] }])
            }
            Command::AdjustScore { buzzer, delta } => {
                self.player_mut(*buzzer)?.score += delta;
                Ok(vec![])
            }
            Command::ShowLeaderboard { visible } => {
                self.leaderboard_visible = *visible;
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
            name: name.trim().to_string(),
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
        if !self.players[&buzzer].enabled {
            return vec![];
        }
        let Some(mode) = self.question().map(|q| q.mode) else {
            return vec![];
        };
        let Phase::Question(QuestionStep::Running { clock }) = self.phase else {
            return vec![];
        };
        if !clock.takes_press_at(now) {
            return vec![];
        }
        match mode {
            AnswerMode::Everyone => self.record_answer(buzzer, button, clock.elapsed_ms(now)),
            AnswerMode::FastestFinger => {
                if self.locked_out.contains(&buzzer) {
                    return vec![];
                }
                let mut clock = clock;
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

    fn record_answer(&mut self, buzzer: BuzzerId, button: Button, elapsed_ms: u64) -> Vec<Effect> {
        let answer_count = self.question().map_or(0, |q| q.answers.len());
        let valid = button.answer_index().is_some_and(|i| i < answer_count);
        // The first answer counts; changing it is not allowed.
        if !valid || self.responses.contains_key(&buzzer) {
            return vec![];
        }
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
            let Some(player) = self.players.get_mut(&buzzer) else { continue };
            if !player.enabled {
                continue;
            }
            let is_correct = response.button.answer_index() == q.correct;
            let delta = if is_correct { q.points_at(response.elapsed_ms) } else { -q.penalty };
            player.score += delta;
            player.total_answer_ms += response.elapsed_ms;
            self.deltas.insert(buzzer, delta);
        }
        // The lights show the result only at the reveal; until then they go dark.
        vec![self.lights_all(Light::Off)]
    }

    fn judge(&mut self, right: bool, now: u64) -> Result<Vec<Effect>, GameError> {
        let Phase::Question(QuestionStep::Buzzed { buzzer, clock }) = self.phase else {
            return Err(GameError::NotNow);
        };
        let q = self.question().cloned().ok_or(GameError::NotNow)?;
        let elapsed = clock.elapsed_ms(now);
        let player = self.player_mut(buzzer)?;
        if right {
            let delta = q.points_at(elapsed);
            player.score += delta;
            player.total_answer_ms += elapsed;
            *self.deltas.entry(buzzer).or_default() += delta;
            self.phase = Phase::Question(QuestionStep::Revealed);
            return Ok(vec![
                Effect::Lights { lights: vec![(buzzer, Light::Correct)] },
                Effect::Sound { cue: Cue::Correct },
            ]);
        }
        player.score -= q.penalty;
        *self.deltas.entry(buzzer).or_default() -= q.penalty;
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
        clock.resume(now);
        self.phase = Phase::Question(QuestionStep::Running { clock });
        effects.push(self.lights_waiting());
        Ok(effects)
    }

    fn next(&mut self, now: u64) -> Result<Vec<Effect>, GameError> {
        match self.phase.clone() {
            Phase::Lobby => Ok(self.enter_slide(0)),
            Phase::Billboard | Phase::Explanation | Phase::EndOfRound => Ok(self.enter_slide(self.slide + 1)),
            Phase::Question(step) => match step {
                QuestionStep::Ready => {
                    let limit = self.question().and_then(Question::time_limit_ms);
                    self.phase = Phase::Question(QuestionStep::Running { clock: Clock::start(limit, now) });
                    Ok(vec![self.lights_waiting(), Effect::Sound { cue: Cue::ClockStart }])
                }
                QuestionStep::Running { .. } => Ok(self.close_question()),
                QuestionStep::Buzzed { .. } => Err(GameError::NotNow),
                QuestionStep::Closed => {
                    self.phase = Phase::Question(QuestionStep::Revealed);
                    let mut effects = vec![Effect::Sound { cue: Cue::Reveal }];
                    if let Some(lights) = self.result_lights() {
                        effects.insert(0, lights);
                    }
                    Ok(effects)
                }
                QuestionStep::Revealed => Ok(self.enter_slide(self.slide + 1)),
            },
            Phase::Final { step } if step < FINAL_LAST_STEP => {
                self.phase = Phase::Final { step: step + 1 };
                Ok(vec![])
            }
            Phase::Final { .. } => Err(GameError::NotNow),
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
                let right = self.responses.get(&b).is_some_and(|r| r.button.answer_index() == q.correct);
                (b, if right { Light::Correct } else { Light::Wrong })
            })
            .collect();
        Some(Effect::Lights { lights })
    }

    fn enter_slide(&mut self, index: usize) -> Vec<Effect> {
        self.responses.clear();
        self.locked_out.clear();
        self.deltas.clear();
        self.leaderboard_visible = false;
        self.slide = index;
        self.phase = match self.quiz.slides.get(index).map(|s| &s.content) {
            None => Phase::Final { step: 0 },
            Some(SlideContent::Billboard { .. }) => Phase::Billboard,
            Some(SlideContent::Explanation { .. }) => Phase::Explanation,
            Some(SlideContent::EndOfRound { .. }) => Phase::EndOfRound,
            Some(SlideContent::Question(_)) => Phase::Question(QuestionStep::Ready),
        };
        vec![self.lights_all(Light::Off)]
    }
}
