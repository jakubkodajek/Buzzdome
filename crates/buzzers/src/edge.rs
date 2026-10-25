//! Press edge detection.

use crate::Press;
use std::collections::{HashMap, HashSet};

/// How long after a press that carries its buzzer's timer the receiver may
/// send it again. The Enjoy 303S did, 180 and 541 ms later, twice in about
/// 90 presses in the hardware test of 2026-10-25; a buzzer's timer moves on
/// every millisecond, so a press with the same timer is that press again.
const TIMED_REPEAT_MS: u64 = 2_000;

/// Turns a stream of decoded reports into press edges.
///
/// Receivers either send one report per press, or keep re-sending the current
/// button state while a button is held. Either way a press must count once: a
/// press is passed on only if the same (buzzer, button) was not in the previous
/// report, or if more than `repeat_gap_ms` passed since that report. A press
/// that carries its buzzer's timer is also dropped when it repeats that
/// buzzer's last one, timer and all, within `TIMED_REPEAT_MS`: the receiver
/// sent it again.
#[derive(Debug)]
pub struct EdgeFilter {
    previous: HashSet<(u16, u8)>,
    previous_at_ms: Option<u64>,
    repeat_gap_ms: u64,
    /// Each buzzer's last press that carried a timer, and when it came.
    last_timed: HashMap<u16, (Press, u64)>,
}

impl EdgeFilter {
    pub fn new(repeat_gap_ms: u64) -> EdgeFilter {
        EdgeFilter { previous: HashSet::new(), previous_at_ms: None, repeat_gap_ms, last_timed: HashMap::new() }
    }

    pub fn filter(&mut self, presses: Vec<Press>, at_ms: u64) -> Vec<Press> {
        let fresh = self.previous_at_ms.is_none_or(|t| at_ms.saturating_sub(t) >= self.repeat_gap_ms);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for press in presses {
            let key = (press.buzzer, press.button as u8);
            if !seen.insert(key) || self.sent_again(press, at_ms) {
                continue;
            }
            if fresh || !self.previous.contains(&key) {
                out.push(press);
            }
        }
        self.previous = seen;
        self.previous_at_ms = Some(at_ms);
        out
    }

    /// Whether `press` is its buzzer's last timed press sent again; notes
    /// it as the last one otherwise.
    fn sent_again(&mut self, press: Press, at_ms: u64) -> bool {
        if press.timer.is_none() {
            return false;
        }
        let again = self.last_timed.get(&press.buzzer).is_some_and(|&(last, last_ms)| last == press && at_ms.saturating_sub(last_ms) <= TIMED_REPEAT_MS);
        if !again {
            self.last_timed.insert(press.buzzer, (press, at_ms));
        }
        again
    }

    /// Forgets the report before, so the next press counts even if its
    /// button was in it. A press sent again is still known as one.
    pub fn reset(&mut self) {
        self.previous.clear();
        self.previous_at_ms = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::Button;

    fn p(buzzer: u16, button: Button) -> Press {
        Press::new(buzzer, button)
    }

    #[test]
    fn a_held_button_counts_once_and_again_after_release() {
        let mut f = EdgeFilter::new(150);
        assert_eq!(f.filter(vec![p(1, Button::A)], 0).len(), 1);
        assert_eq!(f.filter(vec![p(1, Button::A)], 10).len(), 0);
        assert_eq!(f.filter(vec![], 20).len(), 0);
        assert_eq!(f.filter(vec![p(1, Button::A)], 30).len(), 1);
    }

    #[test]
    fn identical_reports_after_a_gap_are_new_presses() {
        let mut f = EdgeFilter::new(150);
        f.filter(vec![p(1, Button::A)], 0);
        assert_eq!(f.filter(vec![p(1, Button::A)], 400).len(), 1);
    }

    #[test]
    fn a_timed_press_sent_again_counts_once() {
        let mut f = EdgeFilter::new(150);
        let press = Press::timed(2, Button::A, 1_842_884);
        assert_eq!(f.filter(vec![press], 0), vec![press]);
        // The receiver sends it again, also after a round began.
        f.reset();
        assert_eq!(f.filter(vec![press], 541), vec![]);
        // The same button again with a later timer is a new press.
        let next = Press::timed(2, Button::A, 1_843_900);
        assert_eq!(f.filter(vec![next], 1_560), vec![next]);
        // Another buzzer's press with that timer is its own.
        assert_eq!(f.filter(vec![Press::timed(1, Button::A, 1_843_900)], 1_570), vec![Press::timed(1, Button::A, 1_843_900)]);
        // Long after, the same timer counts again: the window covers only the receiver's repeats.
        assert_eq!(f.filter(vec![next], 1_560 + TIMED_REPEAT_MS + 1), vec![next]);
    }

    #[test]
    fn new_buttons_in_a_state_report_pass_in_report_order() {
        let mut f = EdgeFilter::new(150);
        f.filter(vec![p(1, Button::A)], 0);
        assert_eq!(
            f.filter(vec![p(3, Button::Big), p(1, Button::A), p(2, Button::Big)], 10),
            vec![p(3, Button::Big), p(2, Button::Big)]
        );
    }
}
