//! Fastest-finger presses in the order the buzzers' own timers give.
//!
//! A receiver that reports with each press how long after its light last
//! changed the buzzer was pressed (`Press::timer`) shows who pressed first
//! once their light turned ready, however the radio delivered the presses.
//! Presses made at almost the same moment can arrive a little apart and in
//! any order, so in a fastest-finger round the first one waits a short
//! window for the others; then all of them go on, the lowest timer first.
//! They all count from when the first one arrived: the earliest moment the
//! app knew of a buzz.

use crate::Press;

#[derive(Debug)]
pub struct TimerOrder {
    window_ms: u64,
    /// Presses waiting, in arrival order, and when the first one arrived.
    held: Option<(u64, Vec<Press>)>,
}

impl TimerOrder {
    pub fn new(window_ms: u64) -> TimerOrder {
        TimerOrder { window_ms, held: None }
    }

    /// The presses of one report, received at `at_ms`: those to pass on now,
    /// each with the time it counts from. While `hold`, in a fastest-finger
    /// round, a press with a timer waits for the others of its moment
    /// instead; a press without one goes on at once, as nothing orders it.
    pub fn take(&mut self, presses: Vec<Press>, at_ms: u64, hold: bool) -> Vec<(Press, u64)> {
        let mut out = if hold { self.release(at_ms) } else { self.flush() };
        for press in presses {
            if hold && press.timer.is_some() {
                self.held.get_or_insert_with(|| (at_ms, Vec::new())).1.push(press);
            } else {
                out.push((press, at_ms));
            }
        }
        out
    }

    /// When the presses waiting are due, if any wait.
    pub fn due_ms(&self) -> Option<u64> {
        self.held.as_ref().map(|(since, _)| since + self.window_ms)
    }

    /// The presses whose wait is over at `now_ms`, lowest timer first.
    pub fn release(&mut self, now_ms: u64) -> Vec<(Press, u64)> {
        match self.due_ms() {
            Some(due) if now_ms >= due => self.flush(),
            _ => Vec::new(),
        }
    }

    /// Every press waiting, lowest timer first, without waiting longer: when
    /// the round ends or the receiver goes. Equal timers keep the order the
    /// presses arrived in.
    pub fn flush(&mut self) -> Vec<(Press, u64)> {
        let Some((since, mut presses)) = self.held.take() else { return Vec::new() };
        presses.sort_by_key(|p| p.timer);
        presses.into_iter().map(|p| (p, since)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{BuzzerId, Button};

    fn timed(buzzer: BuzzerId, timer: u32) -> Press {
        Press::timed(buzzer, Button::Big, timer)
    }

    #[test]
    fn a_round_goes_by_the_timers_after_the_window() {
        let mut order = TimerOrder::new(100);
        assert!(order.take(vec![timed(1, 900)], 1_000, true).is_empty());
        assert_eq!(order.due_ms(), Some(1_100));
        assert!(order.take(vec![timed(2, 400), timed(3, 900)], 1_040, true).is_empty());
        assert!(order.release(1_099).is_empty());
        // Buzzer 2 pressed first after its light, though its press came later.
        assert_eq!(order.release(1_100), [(timed(2, 400), 1_000), (timed(1, 900), 1_000), (timed(3, 900), 1_000)]);
        assert_eq!(order.due_ms(), None);
    }

    #[test]
    fn a_press_after_the_window_begins_the_next() {
        let mut order = TimerOrder::new(100);
        order.take(vec![timed(1, 300)], 1_000, true);
        // Due before this report was read: the first window goes on first.
        assert_eq!(order.take(vec![timed(2, 100)], 1_150, true), [(timed(1, 300), 1_000)]);
        assert_eq!(order.flush(), [(timed(2, 100), 1_150)]);
    }

    #[test]
    fn presses_go_on_at_once_outside_a_round_or_without_a_timer() {
        let mut order = TimerOrder::new(100);
        assert_eq!(order.take(vec![timed(1, 300)], 1_000, false), [(timed(1, 300), 1_000)]);
        let untimed = Press::new(4, Button::A);
        assert_eq!(order.take(vec![untimed, timed(2, 50)], 1_010, true), [(untimed, 1_010)]);
        // The round ends: what waits goes on with the next report.
        assert_eq!(order.take(vec![], 1_020, false), [(timed(2, 50), 1_010)]);
        assert!(order.flush().is_empty());
    }
}
