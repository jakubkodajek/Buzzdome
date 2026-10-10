//! What the windows render: a read-only snapshot of the game at one moment.

use crate::game::{same_name, BuzzerId, Button, Controls, Game, Phase, QuestionStep};
use crate::model::{AnswerMode, Slide, SlideContent};
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GameView {
    pub quiz_title: String,
    pub slide_index: usize,
    pub slide_count: usize,
    pub phase: Phase,
    pub slide: Option<Slide>,
    /// Title of the next slide, for the quizmaster.
    pub next_slide_title: Option<String>,
    pub clock: Option<ClockView>,
    pub players: Vec<PlayerView>,
    /// The team names prepared for the show, in order, with their buzzers.
    pub teams: Vec<TeamView>,
    pub enabled_count: usize,
    /// Enabled players who answered the current question.
    pub answered_count: usize,
    /// Answers per button (A, B, ...), only once the question is closed.
    pub answer_counts: Option<Vec<usize>>,
    /// Everyone-answers: how the enabled players did, once the question is closed.
    pub results: Option<AnswerResults>,
    pub buzzed: Option<BuzzerId>,
    pub leaderboard_visible: bool,
    /// The quizmaster commands the current step takes.
    pub controls: Controls,
    /// See `Game::on_hold`.
    pub on_hold: bool,
}

/// Enabled players by outcome of an everyone-answers question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AnswerResults {
    pub right: usize,
    pub wrong: usize,
    /// No answer.
    pub none: usize,
}

/// A prepared team name and the buzzer paired with it: the lowest-numbered
/// player who has the name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TeamView {
    pub name: String,
    pub buzzer: Option<BuzzerId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClockView {
    pub limit_ms: Option<u64>,
    pub remaining_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub running: bool,
    /// Points a correct answer earns at this moment (they can fall over time).
    pub points: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerView {
    pub buzzer: BuzzerId,
    pub name: String,
    pub score: i32,
    pub enabled: bool,
    pub total_answer_ms: u64,
    /// 1-based position in the ranking; `None` for disabled players.
    pub rank: Option<usize>,
    pub answered: bool,
    /// The player's answer in the current question. For the Director only;
    /// the projector must not show it before the question closes.
    pub answer: Option<Button>,
    /// Whether `answer` is right; for the Director only, like `answer`.
    pub answer_correct: Option<bool>,
    pub answer_ms: Option<u64>,
    pub locked_out: bool,
    /// Score change from the current question.
    pub delta: Option<i32>,
}

pub fn slide_title(slide: &Slide) -> String {
    match &slide.content {
        SlideContent::Billboard { title, .. } | SlideContent::EndOfRound { title } => title.clone(),
        SlideContent::Question(q) => q.text.clone(),
        SlideContent::Explanation { text } => text.clone(),
    }
}

impl Game {
    /// The question clock at `now`, with the points a correct answer earns then.
    /// The app sends it on every tick while the clock runs, so no window has to
    /// compute time or points itself.
    pub fn clock_view(&self, now: u64) -> Option<ClockView> {
        let clock = self.clock()?;
        let elapsed_ms = clock.elapsed_ms(now);
        Some(ClockView {
            limit_ms: clock.limit_ms,
            remaining_ms: clock.remaining_ms(now),
            elapsed_ms,
            running: clock.is_running(),
            points: self.question().map_or(0, |q| q.points_at(elapsed_ms)),
        })
    }

    pub fn view(&self, now: u64) -> GameView {
        let ranking = self.ranking();
        let rank_of = |buzzer: BuzzerId| ranking.iter().position(|p| p.buzzer == buzzer).map(|i| i + 1);
        let correct = self.question().and_then(|q| q.correct);
        let mut players: Vec<PlayerView> = self
            .players()
            .map(|p| PlayerView {
                buzzer: p.buzzer,
                name: p.name.clone(),
                score: p.score,
                enabled: p.enabled,
                total_answer_ms: p.total_answer_ms,
                rank: rank_of(p.buzzer),
                answered: self.responses().contains_key(&p.buzzer),
                answer: self.responses().get(&p.buzzer).map(|r| r.button),
                answer_correct: self
                    .responses()
                    .get(&p.buzzer)
                    .map(|r| r.button.answer_index().is_some_and(|i| Some(i) == correct)),
                answer_ms: self.responses().get(&p.buzzer).map(|r| r.elapsed_ms),
                locked_out: self.locked_out().contains(&p.buzzer),
                delta: self.deltas().get(&p.buzzer).copied(),
            })
            .collect();
        players.sort_by_key(|p| (p.rank.unwrap_or(usize::MAX), p.buzzer));

        let closed = matches!(
            self.phase(),
            Phase::Question(QuestionStep::Closed | QuestionStep::Revealed { .. })
        );
        let enabled: Vec<&PlayerView> = players.iter().filter(|p| p.enabled).collect();
        let results = match self.question() {
            Some(q) if closed && q.mode == AnswerMode::Everyone => {
                let right = enabled.iter().filter(|p| p.answer_correct == Some(true)).count();
                let answered = enabled.iter().filter(|p| p.answered).count();
                Some(AnswerResults { right, wrong: answered - right, none: enabled.len() - answered })
            }
            _ => None,
        };
        let answer_counts = match self.current_slide().map(|s| &s.content) {
            Some(SlideContent::Question(q)) if closed && !q.answers.is_empty() => {
                let mut counts = vec![0; q.answers.len()];
                for response in self.responses().values() {
                    if let Some(i) = response.button.answer_index().filter(|i| *i < counts.len()) {
                        counts[i] += 1;
                    }
                }
                Some(counts)
            }
            _ => None,
        };

        let next_index = match self.phase() {
            Phase::Lobby => 0,
            _ => self.slide_index() + 1,
        };

        GameView {
            quiz_title: self.quiz().title.clone(),
            slide_index: self.slide_index(),
            slide_count: self.quiz().slides.len(),
            phase: self.phase().clone(),
            slide: self.current_slide().cloned(),
            next_slide_title: self.quiz().slides.get(next_index).map(slide_title),
            clock: self.clock_view(now),
            enabled_count: enabled.len(),
            answered_count: enabled.iter().filter(|p| p.answered).count(),
            answer_counts,
            results,
            buzzed: match self.phase() {
                Phase::Question(QuestionStep::Buzzed { buzzer, .. }) => Some(*buzzer),
                _ => None,
            },
            leaderboard_visible: self.leaderboard_visible(),
            controls: self.controls(),
            on_hold: self.on_hold(),
            teams: self
                .teams()
                .iter()
                .map(|name| TeamView {
                    name: name.clone(),
                    buzzer: self.players().find(|p| same_name(&p.name, name)).map(|p| p.buzzer),
                })
                .collect(),
            players,
        }
    }
}
