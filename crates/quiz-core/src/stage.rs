//! The show's moment as stage lights and outside programs follow it: a
//! coarser view of the game than its phase, one name per look the room
//! can take (spec LT-4).

use crate::game::{Game, Phase, QuestionStep, FINAL_LAST_STEP};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How long before time up a running question counts as its last seconds.
pub const LAST_SECONDS_MS: u64 = 5_000;

/// The step of the final ranking from which the winner is on screen.
const WINNER_STEP: u8 = FINAL_LAST_STEP - 1;

/// The show's moment for the stage lights (`Game::stage`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// No show runs. The app's, never `Game::stage`.
    NoShow,
    /// Before the first slide, while buzzers register.
    Lobby,
    Billboard,
    Explanation,
    EndOfRound,
    /// A question is on screen and its clock has not started.
    QuestionReady,
    /// A question's clock runs or stands paused, more than
    /// `LAST_SECONDS_MS` before time up (or with no time limit).
    Answering,
    /// A running question's last `LAST_SECONDS_MS`.
    LastSeconds,
    /// A player buzzed in and is being judged.
    Buzzed,
    /// The answers are in; the right answer is not shown yet.
    Closed,
    /// The right answer is on screen.
    Revealed,
    /// The leaderboard is on screen.
    Leaderboard,
    /// The final ranking builds up, before the winner is shown.
    FinalBuildUp,
    /// The final ranking shows the winner.
    Winner,
}

impl Stage {
    pub const ALL: [Stage; 14] = [
        Stage::NoShow,
        Stage::Lobby,
        Stage::Billboard,
        Stage::Explanation,
        Stage::EndOfRound,
        Stage::QuestionReady,
        Stage::Answering,
        Stage::LastSeconds,
        Stage::Buzzed,
        Stage::Closed,
        Stage::Revealed,
        Stage::Leaderboard,
        Stage::FinalBuildUp,
        Stage::Winner,
    ];
}

impl Game {
    /// The show's moment at `now`, for the stage lights. The leaderboard
    /// wins over the slide under it, as it covers the screen.
    pub fn stage(&self, now: u64) -> Stage {
        if self.leaderboard_visible() {
            return Stage::Leaderboard;
        }
        match self.phase() {
            Phase::Lobby => Stage::Lobby,
            Phase::Billboard => Stage::Billboard,
            Phase::Explanation => Stage::Explanation,
            Phase::EndOfRound => Stage::EndOfRound,
            Phase::Question(QuestionStep::Ready) => Stage::QuestionReady,
            Phase::Question(QuestionStep::Running { clock }) => match clock.remaining_ms(now) {
                Some(left) if left <= LAST_SECONDS_MS => Stage::LastSeconds,
                _ => Stage::Answering,
            },
            Phase::Question(QuestionStep::Buzzed { .. }) => Stage::Buzzed,
            Phase::Question(QuestionStep::Closed) => Stage::Closed,
            Phase::Question(QuestionStep::Revealed { .. }) => Stage::Revealed,
            Phase::Final { step } if *step >= WINNER_STEP => Stage::Winner,
            Phase::Final { .. } => Stage::FinalBuildUp,
        }
    }
}
