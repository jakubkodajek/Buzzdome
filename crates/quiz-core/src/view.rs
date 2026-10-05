//! What the windows render: a read-only snapshot of the game at one moment.

use crate::game::{BuzzerId, Button, Game, Phase, QuestionStep};
use crate::model::{SlideContent, Slide};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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
    pub enabled_count: usize,
    pub answered_count: usize,
    /// Answers per button (A, B, ...), only once the question is closed.
    pub answer_counts: Option<Vec<usize>>,
    pub buzzed: Option<BuzzerId>,
    pub leaderboard_visible: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockView {
    pub limit_ms: Option<u64>,
    pub remaining_ms: Option<u64>,
    pub elapsed_ms: u64,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize)]
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
    pub answer_ms: Option<u64>,
    pub locked_out: bool,
    /// Score change from the current question.
    pub delta: Option<i32>,
}

pub fn slide_title(slide: &Slide) -> String {
    match &slide.content {
        SlideContent::Billboard { title, .. } | SlideContent::EndOfRound { title } => title.clone(),
        SlideContent::Question(q) => q.text.clone(),
    }
}

impl Game {
    pub fn view(&self, now: u64) -> GameView {
        let ranking = self.ranking();
        let rank_of = |buzzer: BuzzerId| ranking.iter().position(|p| p.buzzer == buzzer).map(|i| i + 1);
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
                answer_ms: self.responses().get(&p.buzzer).map(|r| r.elapsed_ms),
                locked_out: self.locked_out().contains(&p.buzzer),
                delta: self.deltas().get(&p.buzzer).copied(),
            })
            .collect();
        players.sort_by_key(|p| (p.rank.unwrap_or(usize::MAX), p.buzzer));

        let closed = matches!(
            self.phase(),
            Phase::Question(QuestionStep::Closed | QuestionStep::Revealed)
        );
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
            clock: self.clock().map(|c| ClockView {
                limit_ms: c.limit_ms,
                remaining_ms: c.remaining_ms(now),
                elapsed_ms: c.elapsed_ms(now),
                running: c.is_running(),
            }),
            enabled_count: players.iter().filter(|p| p.enabled).count(),
            answered_count: self.responses().len(),
            answer_counts,
            buzzed: match self.phase() {
                Phase::Question(QuestionStep::Buzzed { buzzer, .. }) => Some(*buzzer),
                _ => None,
            },
            leaderboard_visible: self.leaderboard_visible(),
            players,
        }
    }
}
