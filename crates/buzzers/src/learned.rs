//! "Learned" protocol: maps raw input reports to presses using signatures
//! recorded while the quizmaster presses each button. It needs no vendor
//! documentation, so it works for any receiver that sends a distinct report per
//! press. It cannot drive lights: that needs the real protocol.
//!
//! A signature keeps only the bytes that stayed the same over all recorded
//! presses (`mask[i] == true`). Bytes that changed, such as a running press-time
//! counter, are ignored when matching.

use crate::hid::DeviceInfo;
use crate::protocol::Protocol;
use crate::Press;
use quiz_core::{BuzzerId, Button};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub buzzer: BuzzerId,
    pub button: Button,
    pub bytes: Vec<u8>,
    pub mask: Vec<bool>,
}

impl Signature {
    pub fn matches(&self, report: &[u8]) -> bool {
        report.len() == self.bytes.len()
            && self.bytes.iter().zip(&self.mask).zip(report).all(|((b, m), r)| !m || b == r)
    }

    fn specificity(&self) -> usize {
        self.mask.iter().filter(|m| **m).count()
    }

    pub fn from_samples(buzzer: BuzzerId, button: Button, samples: &[Vec<u8>]) -> Result<Signature, String> {
        let first = samples.first().ok_or("no presses recorded")?;
        if samples.iter().any(|s| s.len() != first.len()) {
            return Err("the recorded reports have different lengths".into());
        }
        let mask = (0..first.len()).map(|i| samples.iter().all(|s| s[i] == first[i])).collect();
        Ok(Signature { buzzer, button, bytes: first.clone(), mask })
    }
}

/// Learned signatures for one receiver model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedMapping {
    pub vendor_id: u16,
    pub product_id: u16,
    pub signatures: Vec<Signature>,
}

impl LearnedMapping {
    pub fn for_device(&self, d: &DeviceInfo) -> bool {
        d.vendor_id == self.vendor_id && d.product_id == self.product_id
    }

    /// Adds or replaces the signature for its (buzzer, button).
    pub fn upsert(&mut self, sig: Signature) {
        self.signatures.retain(|s| !(s.buzzer == sig.buzzer && s.button == sig.button));
        self.signatures.push(sig);
    }
}

#[derive(Debug, Clone)]
pub struct LearnedProtocol {
    mapping: LearnedMapping,
}

impl LearnedProtocol {
    pub fn new(mapping: LearnedMapping) -> LearnedProtocol {
        LearnedProtocol { mapping }
    }
}

impl Protocol for LearnedProtocol {
    fn name(&self) -> String {
        format!("learned buttons ({})", self.mapping.signatures.len())
    }

    fn decode(&mut self, report: &[u8]) -> Vec<Press> {
        self.mapping
            .signatures
            .iter()
            .filter(|s| s.matches(report))
            .max_by_key(|s| s.specificity())
            .map(|s| vec![Press { buzzer: s.buzzer, button: s.button }])
            .unwrap_or_default()
    }
}

/// True for reports without button data: every byte after the report ID is zero.
pub fn is_idle_report(report: &[u8]) -> bool {
    report.iter().skip(1).all(|b| *b == 0)
}

/// Records presses of one (buzzer, button) pair.
///
/// One press often produces a burst of reports (press, repeats while held,
/// release). Only the first non-idle report after an idle report or a quiet gap
/// is kept as a sample.
#[derive(Debug, Clone)]
pub struct Recorder {
    pub buzzer: BuzzerId,
    pub button: Button,
    samples: Vec<Vec<u8>>,
    last_at_ms: Option<u64>,
    last_was_idle: bool,
    gap_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnOutcome {
    pub signature: Signature,
    /// Other learned buttons that also match the recorded presses (ambiguous).
    pub conflicts: Vec<Signature>,
}

impl Recorder {
    pub fn new(buzzer: BuzzerId, button: Button) -> Recorder {
        Recorder { buzzer, button, samples: Vec::new(), last_at_ms: None, last_was_idle: true, gap_ms: 150 }
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Feeds a raw report. Returns true when it was kept as a new sample.
    pub fn feed(&mut self, report: &[u8], at_ms: u64) -> bool {
        let idle = is_idle_report(report);
        let starts_burst = self.last_was_idle || self.last_at_ms.is_none_or(|t| at_ms.saturating_sub(t) >= self.gap_ms);
        self.last_at_ms = Some(at_ms);
        self.last_was_idle = idle;
        if !starts_burst || idle {
            return false;
        }
        self.samples.push(report.to_vec());
        true
    }

    pub fn finish(&self, existing: &[Signature]) -> Result<LearnOutcome, String> {
        let signature = Signature::from_samples(self.buzzer, self.button, &self.samples)?;
        let conflicts = existing
            .iter()
            .filter(|s| !(s.buzzer == self.buzzer && s.button == self.button))
            // Ambiguous either way: their signature matches our presses, or ours matches theirs.
            .filter(|s| self.samples.iter().any(|r| s.matches(r)) || signature.matches(&s.bytes))
            .cloned()
            .collect();
        Ok(LearnOutcome { signature, conflicts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(buzzer: BuzzerId, button: Button, bytes: &[u8], mask: &[bool]) -> Signature {
        Signature { buzzer, button, bytes: bytes.to_vec(), mask: mask.to_vec() }
    }

    #[test]
    fn masks_bytes_that_change_between_presses() {
        let s = Signature::from_samples(
            3,
            Button::A,
            &[vec![1, 3, 1, 0x10, 0x20], vec![1, 3, 1, 0x55, 0x21], vec![1, 3, 1, 0x91, 0x22]],
        )
        .unwrap();
        assert_eq!(s.mask, vec![true, true, true, false, false]);
        assert!(s.matches(&[1, 3, 1, 0xff, 0xff]));
        assert!(!s.matches(&[1, 4, 1, 0xff, 0xff]));
        assert!(!s.matches(&[1, 3, 1, 0xff]));
    }

    #[test]
    fn rejects_samples_of_different_lengths() {
        assert!(Signature::from_samples(1, Button::A, &[vec![1, 2], vec![1, 2, 3]]).is_err());
        assert!(Signature::from_samples(1, Button::A, &[]).is_err());
    }

    #[test]
    fn decoding_prefers_the_most_specific_signature() {
        let generic = sig(1, Button::A, &[1, 0, 0], &[true, false, false]);
        let specific = sig(2, Button::B, &[1, 2, 0], &[true, true, false]);
        let mut p = LearnedProtocol::new(LearnedMapping { vendor_id: 1, product_id: 2, signatures: vec![generic, specific] });
        assert_eq!(p.decode(&[1, 2, 9]), vec![Press { buzzer: 2, button: Button::B }]);
        assert_eq!(p.decode(&[1, 7, 9]), vec![Press { buzzer: 1, button: Button::A }]);
        assert!(p.decode(&[2, 2, 9]).is_empty());
    }

    #[test]
    fn recorder_keeps_one_sample_per_press() {
        let mut r = Recorder::new(1, Button::A);
        let press = [1, 1, 1];
        let idle = [1, 0, 0];
        assert!(r.feed(&press, 0));
        assert!(!r.feed(&press, 10), "held-button repeat");
        assert!(!r.feed(&idle, 20), "release");
        assert!(r.feed(&press, 30), "press right after a release");
        assert!(r.feed(&press, 500), "press after a quiet gap");
        assert_eq!(r.sample_count(), 3);
    }

    #[test]
    fn recorder_reports_conflicts_with_other_buttons() {
        let mut r = Recorder::new(2, Button::A);
        r.feed(&[1, 5, 5], 0);
        r.feed(&[1, 5, 5], 1000);
        let other = sig(1, Button::A, &[1, 0, 0], &[true, false, false]);
        assert_eq!(r.finish(std::slice::from_ref(&other)).unwrap().conflicts, vec![other]);
        let unrelated = sig(1, Button::B, &[2, 0, 0], &[true, true, true]);
        assert!(r.finish(&[unrelated]).unwrap().conflicts.is_empty());
    }

    #[test]
    fn mapping_round_trips_through_json() {
        let mut m = LearnedMapping { vendor_id: 0x10c4, product_id: 0xea80, signatures: vec![] };
        m.upsert(sig(1, Button::Big, &[1, 2], &[true, false]));
        m.upsert(sig(1, Button::Big, &[1, 3], &[true, true]));
        assert_eq!(m.signatures.len(), 1);
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<LearnedMapping>(&json).unwrap(), m);
    }
}
