//! The show's results so far (spec SF-11), for the Director's Results tab,
//! its print and the results export: every player with what each round
//! gave them, and every question asked with what it gave whom.

use crate::game::{BuzzerId, Game, Mark};
use crate::model::{Quiz, SlideContent};
use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ShowResults {
    /// The questions asked, in the order the show asked them (`Game::asked`).
    pub questions: Vec<AskedQuestion>,
    /// How many rounds the quiz's questions are in (`slide_rounds`).
    pub rounds: usize,
    /// Every player, in the ranking's order (`Game::standings`).
    pub players: Vec<PlayerResult>,
}

/// A question as the show asked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AskedQuestion {
    pub slide: usize,
    /// The question's round, from 0 (`slide_rounds`).
    pub round: usize,
    /// What it gave the players it scored, by buzzer: those who answered or
    /// buzzed and were judged, and those it penalised for no answer.
    pub marks: Vec<(BuzzerId, Mark)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerResult {
    pub buzzer: BuzzerId,
    pub name: String,
    /// The player's place, none when switched off (`Game::ranking`).
    pub rank: Option<usize>,
    pub score: i32,
    pub total_answer_ms: u64,
    /// What the questions asked gave the player, by round: with `adjusted`,
    /// their score.
    pub rounds: Vec<i32>,
    /// What the quizmaster added to the score by hand (`Player::adjusted`).
    pub adjusted: i32,
}

/// The round of each of the quiz's slides, from 0: an end-of-round slide
/// closes its round, so the slides after it are in the next one.
fn slide_rounds(quiz: &Quiz) -> Vec<usize> {
    let mut round = 0;
    let mut rounds = Vec::with_capacity(quiz.slides.len());
    for slide in &quiz.slides {
        rounds.push(round);
        if matches!(slide.content, SlideContent::EndOfRound { .. }) {
            round += 1;
        }
    }
    rounds
}

impl Game {
    pub fn show_results(&self) -> ShowResults {
        let slide_rounds = slide_rounds(self.quiz());
        let round_of = |slide: usize| slide_rounds.get(slide).copied().unwrap_or_default();
        // As many rounds as hold a question: an end-of-round slide after the last one opens none.
        let last_question = self.quiz().slides.iter().rposition(|slide| matches!(slide.content, SlideContent::Question(_)));
        let rounds = last_question.map_or(1, |slide| round_of(slide) + 1);
        let questions: Vec<AskedQuestion> = self
            .asked()
            .map(|asked| AskedQuestion {
                slide: asked.slide,
                round: round_of(asked.slide),
                marks: asked.marks.iter().map(|(&buzzer, &mark)| (buzzer, mark)).collect(),
            })
            .collect();
        let players = self
            .standings()
            .into_iter()
            .map(|(rank, player)| {
                let mut by_round = vec![0_i32; rounds];
                for asked in self.asked() {
                    let points = asked.marks.get(&player.buzzer).map_or(0, |mark| mark.points);
                    if let Some(sum) = by_round.get_mut(round_of(asked.slide)) {
                        *sum = sum.saturating_add(points);
                    }
                }
                PlayerResult {
                    buzzer: player.buzzer,
                    name: player.name.clone(),
                    rank,
                    score: player.score,
                    total_answer_ms: player.total_answer_ms,
                    rounds: by_round,
                    adjusted: player.adjusted,
                }
            })
            .collect();
        ShowResults { questions, rounds, players }
    }
}
