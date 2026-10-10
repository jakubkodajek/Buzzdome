//! Press edge detection.

use crate::Press;
use std::collections::HashSet;

/// Turns a stream of decoded reports into press edges.
///
/// Receivers either send one report per press, or keep re-sending the current
/// button state while a button is held. Either way a press must count once: a
/// press is passed on only if the same (buzzer, button) was not in the previous
/// report, or if more than `repeat_gap_ms` passed since that report.
#[derive(Debug)]
pub struct EdgeFilter {
    previous: HashSet<(u16, u8)>,
    previous_at_ms: Option<u64>,
    repeat_gap_ms: u64,
}

impl EdgeFilter {
    pub fn new(repeat_gap_ms: u64) -> EdgeFilter {
        EdgeFilter { previous: HashSet::new(), previous_at_ms: None, repeat_gap_ms }
    }

    pub fn filter(&mut self, presses: Vec<Press>, at_ms: u64) -> Vec<Press> {
        let fresh = self.previous_at_ms.is_none_or(|t| at_ms.saturating_sub(t) >= self.repeat_gap_ms);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for press in presses {
            let key = (press.buzzer, press.button as u8);
            if !seen.insert(key) {
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

    pub fn reset(&mut self) {
        self.previous.clear();
        self.previous_at_ms = None;
    }
}

impl Default for EdgeFilter {
    fn default() -> Self {
        EdgeFilter::new(150)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::Button;

    fn p(buzzer: u16, button: Button) -> Press {
        Press { buzzer, button }
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
    fn new_buttons_in_a_state_report_pass_in_report_order() {
        let mut f = EdgeFilter::new(150);
        f.filter(vec![p(1, Button::A)], 0);
        assert_eq!(
            f.filter(vec![p(3, Button::Big), p(1, Button::A), p(2, Button::Big)], 10),
            vec![p(3, Button::Big), p(2, Button::Big)]
        );
    }
}
