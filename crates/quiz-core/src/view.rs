//! What the windows render: a read-only snapshot of the game at one moment.

use crate::game::{same_name, BuzzerId, Button, Controls, Game, Phase, QuestionStep, Response};
use crate::model::{AnswerMode, Slide};
use crate::settings::AutoStep;
use serde::Serialize;
use std::collections::BTreeMap;
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
    /// The slide `Next` goes to once the current one is done
    /// (`Game::following`), for the quizmaster: past the last slide for the
    /// final ranking, none once the show is over.
    pub next_slide: Option<usize>,
    /// The slide the quizmaster's Back goes to (`Game::previous`), none
    /// where it goes nowhere.
    pub previous_slide: Option<usize>,
    pub clock: Option<ClockView>,
    pub players: Vec<PlayerView>,
    /// The team names prepared for the show, in order, with their buzzers.
    pub teams: Vec<TeamView>,
    pub enabled_count: usize,
    /// Enabled players who answered the current question.
    pub answered_count: usize,
    /// Everyone-answers: how many of the players it counted gave each
    /// answer, in order, once the question is closed; none when it counted
    /// nobody, as when the show came back to it (`Game::previous`).
    pub answer_counts: Option<Vec<usize>>,
    /// The current question's answers in order, each with the share of the
    /// points it scores (`FULL_SHARE` when fully right), none for a wrong
    /// one; empty on another slide. Windows mark the right answers from it.
    pub answer_shares: Vec<Option<u8>>,
    /// Everyone-answers: how the players it counted did, once the question
    /// is closed; none when it counted nobody, as `answer_counts`.
    pub results: Option<AnswerResults>,
    pub buzzed: Option<BuzzerId>,
    pub leaderboard_visible: bool,
    /// The quizmaster commands the current step takes.
    pub controls: Controls,
    /// See `Game::on_hold`.
    pub on_hold: bool,
    /// See `Game::video_plays`.
    pub video_plays: u32,
    /// See `Game::step_no`.
    pub step_no: u32,
    /// The step the show takes by itself once everyone answered
    /// (`Game::pending_step`); none while it waits for the quizmaster.
    pub auto_step: Option<AutoStepView>,
}

/// A step the show takes by itself, for the Director to show coming.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AutoStepView {
    /// What the step does.
    pub action: AutoStep,
    /// How long until it does it.
    pub in_ms: u64,
}

/// The players a closed everyone-answers question counted, by outcome.
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
    /// The player's place in the ranking, from 1, shared with the players
    /// equal to them (`Game::ranking`); `None` for disabled players.
    pub rank: Option<usize>,
    pub answered: bool,
    /// The player's answer in the current question. For the Director only;
    /// the projector must not show it before the question closes.
    pub answer: Option<Button>,
    /// Whether `answer` is right; for the Director only, like `answer`.
    pub answer_correct: Option<bool>,
    pub answer_ms: Option<u64>,
    pub locked_out: bool,
    /// Switched on or registered while the current fastest-finger question
    /// was under way, so plays from the next question (`Game::joins_next`).
    pub joins_next: bool,
    /// Score change from the current question.
    pub delta: Option<i32>,
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

    /// How the players a closed everyone-answers question counted did.
    fn results(&self) -> Option<AnswerResults> {
        let question = self.question().filter(|q| q.mode == AnswerMode::Everyone && self.has_results())?;
        let answers: Vec<&Response> = self.counted().iter().filter_map(|b| self.responses().get(b)).collect();
        let right = answers.iter().filter(|r| r.is_right(question)).count();
        Some(AnswerResults { right, wrong: answers.len() - right, none: self.counted().len() - answers.len() })
    }

    /// How many of the players a closed everyone-answers question counted
    /// gave each of its answers.
    fn answer_counts(&self) -> Option<Vec<usize>> {
        let question = self.question().filter(|q| q.mode == AnswerMode::Everyone && self.has_results())?;
        let mut counts = vec![0; question.answers.len()];
        for response in self.counted().iter().filter_map(|b| self.responses().get(b)) {
            if let Some(count) = response.button.answer_index().and_then(|i| counts.get_mut(i)) {
                *count += 1;
            }
        }
        Some(counts)
    }

    /// The question closed and counted somebody, so it has results to show.
    fn has_results(&self) -> bool {
        let closed = matches!(self.phase(), Phase::Question(QuestionStep::Closed | QuestionStep::Revealed { .. }));
        closed && !self.counted().is_empty()
    }

    pub fn view(&self, now: u64) -> GameView {
        let ranks: BTreeMap<BuzzerId, usize> = self.ranking().into_iter().map(|(place, p)| (p.buzzer, place)).collect();
        let question = self.question();
        let mut players: Vec<PlayerView> = self
            .players()
            .map(|p| PlayerView {
                buzzer: p.buzzer,
                name: p.name.clone(),
                score: p.score,
                enabled: p.enabled,
                total_answer_ms: p.total_answer_ms,
                rank: ranks.get(&p.buzzer).copied(),
                answered: self.responses().contains_key(&p.buzzer),
                answer: self.responses().get(&p.buzzer).map(|r| r.button),
                answer_correct: self.responses().get(&p.buzzer).map(|r| question.is_some_and(|q| r.is_right(q))),
                answer_ms: self.responses().get(&p.buzzer).map(|r| r.elapsed_ms),
                locked_out: self.locked_out().contains(&p.buzzer),
                joins_next: self.joins_next().contains(&p.buzzer),
                delta: self.delta(p.buzzer),
            })
            .collect();
        players.sort_by_key(|p| (p.rank.unwrap_or(usize::MAX), p.buzzer));

        let enabled: Vec<&PlayerView> = players.iter().filter(|p| p.enabled).collect();
        GameView {
            quiz_title: self.quiz().title.clone(),
            slide_index: self.slide_index(),
            slide_count: self.quiz().slides.len(),
            phase: self.phase().clone(),
            slide: self.current_slide().cloned(),
            next_slide: self.following(),
            previous_slide: self.previous(),
            clock: self.clock_view(now),
            enabled_count: enabled.len(),
            answered_count: enabled.iter().filter(|p| p.answered).count(),
            answer_counts: self.answer_counts(),
            answer_shares: question.map(|q| (0..q.answers.len()).map(|i| q.share_of(i)).collect()).unwrap_or_default(),
            results: self.results(),
            buzzed: match self.phase() {
                Phase::Question(QuestionStep::Buzzed { buzzer, .. }) => Some(*buzzer),
                _ => None,
            },
            leaderboard_visible: self.leaderboard_visible(),
            controls: self.controls(),
            on_hold: self.on_hold(),
            video_plays: self.video_plays(),
            step_no: self.step_no(),
            auto_step: self.pending_step().map(|(action, _, at)| AutoStepView { action, in_ms: at.saturating_sub(now) }),
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
