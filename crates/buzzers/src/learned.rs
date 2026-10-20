//! "Learned" protocol: maps raw input reports to presses using signatures
//! recorded while the quizmaster presses each button. It needs no vendor
//! documentation, so it works for any receiver that sends a distinct report per
//! press. It cannot drive lights: that needs the real protocol.
//!
//! A signature keeps only the bytes that stayed the same over all recorded
//! presses (`mask[i] == true`). Bytes that changed, such as a running press-time
//! counter, are ignored when matching.

use crate::hid::DeviceInfo;
use crate::protocol::{Protocol, ProtocolKind};
use crate::Press;
use quiz_core::{BuzzerId, Button};
use serde::{Deserialize, Serialize};

/// Fewest recorded presses that make a signature (three is better).
pub const MIN_SAMPLES: usize = 2;

/// Why recorded presses cannot be saved as a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LearnError {
    #[error("press the button at least {MIN_SAMPLES} times")]
    TooFewPresses,
    #[error("the recorded reports have different lengths")]
    Inconsistent,
    #[error("these presses look the same as buzzer {buzzer} button {button:?}")]
    Conflict { buzzer: BuzzerId, button: Button },
}

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

    pub fn from_samples(buzzer: BuzzerId, button: Button, samples: &[Vec<u8>]) -> Result<Signature, LearnError> {
        let first = samples.first().ok_or(LearnError::TooFewPresses)?;
        if samples.iter().any(|s| s.len() != first.len()) {
            return Err(LearnError::Inconsistent);
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
    fn kind(&self) -> ProtocolKind {
        ProtocolKind::Learned { buttons: self.mapping.signatures.len() }
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

    fn learnable(&self) -> bool {
        true
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

    /// Enough presses are recorded to save the button (see `save_into`).
    pub fn can_save(&self) -> bool {
        self.sample_count() >= MIN_SAMPLES
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

    pub fn finish(&self, existing: &[Signature]) -> Result<LearnOutcome, LearnError> {
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

    /// Saves the recorded presses as the signature of this button of the
    /// receiver `device`, unless they would be mistaken for another button.
    pub fn save_into(&self, mappings: &mut Vec<LearnedMapping>, device: &DeviceInfo) -> Result<(), LearnError> {
        if !self.can_save() {
            return Err(LearnError::TooFewPresses);
        }
        let index = mappings.iter().position(|m| m.for_device(device));
        let outcome = self.finish(index.map_or(&[][..], |i| &mappings[i].signatures))?;
        if let Some(c) = outcome.conflicts.first() {
            return Err(LearnError::Conflict { buzzer: c.buzzer, button: c.button });
        }
        let index = index.unwrap_or_else(|| {
            mappings.push(LearnedMapping { vendor_id: device.vendor_id, product_id: device.product_id, signatures: vec![] });
            mappings.len() - 1
        });
        mappings[index].upsert(outcome.signature);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::for_device;

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
        assert_eq!(Signature::from_samples(1, Button::A, &[vec![1, 2], vec![1, 2, 3]]), Err(LearnError::Inconsistent));
        assert_eq!(Signature::from_samples(1, Button::A, &[]), Err(LearnError::TooFewPresses));
    }

    fn device(vendor_id: u16) -> DeviceInfo {
        DeviceInfo {
            path: "p".into(),
            vendor_id,
            product_id: 2,
            manufacturer: None,
            product: None,
            serial_number: None,
            usage_page: 0,
            usage: 0,
            interface_number: 0,
        }
    }

    #[test]
    fn saves_learned_buttons_per_receiver_model_and_refuses_lookalikes() {
        let mut mappings = Vec::new();
        let mut r = Recorder::new(1, Button::A);
        r.feed(&[1, 1, 0x10], 0);
        assert_eq!(r.save_into(&mut mappings, &device(1)), Err(LearnError::TooFewPresses));
        assert!(mappings.is_empty(), "nothing is saved for a refused button");
        r.feed(&[1, 1, 0x11], 1000);
        r.save_into(&mut mappings, &device(1)).unwrap();
        // Another model gets a mapping of its own.
        r.save_into(&mut mappings, &device(9)).unwrap();
        assert_eq!(mappings.len(), 2);
        assert_eq!(for_device(&mappings, &device(1)).kind(), ProtocolKind::Learned { buttons: 1 });
        assert_eq!(for_device(&[], &device(1)).kind(), ProtocolKind::RawOnly);
        // Buttons can be taught again, or for the first time.
        assert!(for_device(&mappings, &device(1)).learnable());
        assert!(for_device(&[], &device(1)).learnable());

        let mut same = Recorder::new(2, Button::B);
        same.feed(&[1, 1, 0x12], 0);
        same.feed(&[1, 1, 0x13], 1000);
        assert_eq!(same.save_into(&mut mappings, &device(1)), Err(LearnError::Conflict { buzzer: 1, button: Button::A }));
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
