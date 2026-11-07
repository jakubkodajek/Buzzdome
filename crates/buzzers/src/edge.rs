//! Press edge detection.

use crate::{Press, RemoteKey};
use std::collections::{HashMap, HashSet};

/// How long after a press that carries its buzzer's timer the receiver may
/// send it again. The Enjoy 303S did, 180 and 541 ms later, twice in about
/// 90 presses in the hardware test of 2026-10-25; a buzzer's timer moves on
/// every millisecond, so a press with the same timer is that press again.
const TIMED_REPEAT_MS: u64 = 2_000;

/// How soon after a key of the quizmaster's remote the same key counts as
/// that key sent again. The receiver sent presses again up to 541 ms later
/// (`TIMED_REPEAT_MS`), and a remote's message carries nothing that tells
/// a key sent again from one pressed again, so a key pressed twice this
/// quickly counts once: a second step would go by unseen.
const REMOTE_REPEAT_MS: u64 = 700;

/// Turns a stream of decoded reports into press edges.
///
/// Receivers either send one report per press, or keep re-sending the current
/// button state while a button is held. Either way a press must count once: a
/// press is passed on only if the same (buzzer, button) was not in the previous
/// report, or if more than `repeat_gap_ms` passed since that report. A press
/// that carries its buzzer's timer is also dropped when it repeats one of
/// that buzzer's timed presses of the last `TIMED_REPEAT_MS`, timer and all:
/// the receiver sent it again.
#[derive(Debug)]
pub struct EdgeFilter {
    previous: HashSet<(u16, u8)>,
    previous_at_ms: Option<u64>,
    repeat_gap_ms: u64,
    /// Each buzzer's presses that carried a timer within the last
    /// `TIMED_REPEAT_MS`, counted or stale, and when they came. A list, not
    /// the last one: a stale press, then a real one, then the stale one sent
    /// again must still know the stale one.
    recent_timed: HashMap<u16, Vec<(Press, u64)>>,
    /// The remote's last key that counted, and when it came.
    last_remote: Option<(RemoteKey, u64)>,
}

impl EdgeFilter {
    pub fn new(repeat_gap_ms: u64) -> EdgeFilter {
        EdgeFilter { previous: HashSet::new(), previous_at_ms: None, repeat_gap_ms, recent_timed: HashMap::new(), last_remote: None }
    }

    pub fn filter(&mut self, presses: Vec<Press>, at_ms: u64) -> Vec<Press> {
        self.filter_stale(presses, at_ms, |_| false)
    }

    /// Like `filter`, but a press for which `stale` holds never counts. It
    /// is still remembered by its timer, in the receiver's order, so that
    /// the receiver sending it again later is known as that; it is not
    /// noted as in this report, so the same buzzer's real press still
    /// counts.
    pub fn filter_stale(&mut self, presses: Vec<Press>, at_ms: u64, stale: impl Fn(&Press) -> bool) -> Vec<Press> {
        let fresh = self.previous_at_ms.is_none_or(|t| at_ms.saturating_sub(t) >= self.repeat_gap_ms);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for press in presses {
            let key = (press.buzzer, press.button as u8);
            // A press sent again is not noted as in this report: it would
            // hide the buzzer's next press within the repeat gap.
            if seen.contains(&key) || self.sent_again(press, at_ms) || stale(&press) {
                continue;
            }
            seen.insert(key);
            if fresh || !self.previous.contains(&key) {
                out.push(press);
            }
        }
        self.previous = seen;
        self.previous_at_ms = Some(at_ms);
        out
    }

    /// Whether `press` is one of its buzzer's recent timed presses sent
    /// again; remembers it otherwise.
    fn sent_again(&mut self, press: Press, at_ms: u64) -> bool {
        if press.timer.is_none() {
            return false;
        }
        let recent = self.recent_timed.entry(press.buzzer).or_default();
        recent.retain(|&(_, then)| at_ms.saturating_sub(then) <= TIMED_REPEAT_MS);
        let again = recent.iter().any(|&(earlier, _)| earlier == press);
        if !again {
            recent.push((press, at_ms));
        }
        again
    }

    /// Whether the remote's `key`, which came at `at_ms`, counts: not when
    /// it repeats the last key that counted within `REMOTE_REPEAT_MS`.
    pub fn remote(&mut self, key: RemoteKey, at_ms: u64) -> bool {
        let again = self.last_remote.is_some_and(|(last, last_ms)| last == key && at_ms.saturating_sub(last_ms) < REMOTE_REPEAT_MS);
        if !again {
            self.last_remote = Some((key, at_ms));
        }
        !again
    }

    /// Forgets the report before, so the next press counts even if its
    /// button was in it. A press or key sent again is still known as one.
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
    fn a_stale_press_sent_again_after_a_real_one_stays_dropped() {
        let mut f = EdgeFilter::new(150);
        let stale = Press::timed(2, Button::A, 150);
        let real = Press::timed(2, Button::B, 40);
        let is_stale = |press: &Press| press.timer == Some(150);
        // S then R in one report: S is dropped, R counts.
        assert_eq!(f.filter_stale(vec![stale, real], 0, is_stale), vec![real]);
        // S sent again later, when the round has run longer than its timer.
        assert_eq!(f.filter_stale(vec![stale], 400, |_| false), vec![]);
        // And R sent again too.
        assert_eq!(f.filter_stale(vec![real], 600, |_| false), vec![]);
        // S, R and S' in separate reports.
        let mut f = EdgeFilter::new(150);
        assert_eq!(f.filter_stale(vec![stale], 0, is_stale), vec![]);
        assert_eq!(f.filter_stale(vec![real], 20, is_stale), vec![real]);
        assert_eq!(f.filter_stale(vec![stale], 541, |_| false), vec![]);
    }

    #[test]
    fn a_key_of_the_remote_sent_again_counts_once() {
        let mut f = EdgeFilter::new(150);
        assert!(f.remote(RemoteKey::Forward, 1_000));
        f.reset();
        assert!(!f.remote(RemoteKey::Forward, 1_541), "sent again, also after a reset");
        // Another key at once counts, and so does the first after it.
        assert!(f.remote(RemoteKey::Tick, 1_560));
        assert!(f.remote(RemoteKey::Forward, 1_600));
        // Counted from the key that counted, not from one dropped.
        assert!(!f.remote(RemoteKey::Forward, 1_600 + REMOTE_REPEAT_MS - 1));
        assert!(f.remote(RemoteKey::Forward, 1_600 + REMOTE_REPEAT_MS));
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
