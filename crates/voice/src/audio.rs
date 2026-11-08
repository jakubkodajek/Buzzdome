//! Puts a clip together from its parts, each rendered on its own: the
//! silence the model leaves around each part is cut (a third of a second
//! before and more than half a second after a short one, in our
//! measurement), each part is brought to the same loudness, and the
//! pauses the line asks for go between them.

/// A sample this far below a part's loudest one counts as silence.
const SILENCE: f32 = 0.02;
/// How much of the silence is kept before and after the sound, so that a
/// soft start or end is not cut.
const MARGIN_MS: u32 = 40;
/// The loudness each part is brought to (its root mean square over the
/// sound, about -20 dBFS), and the loudest a sample may then be.
const TARGET_RMS: f32 = 0.1;
const MAX_PEAK: f32 = 0.95;

/// `samples` without the silence before and after the sound in them,
/// `MARGIN_MS` of it kept; nothing when they are all silence.
pub fn trimmed(samples: &[f32], rate: u32) -> &[f32] {
    let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
    if peak == 0.0 {
        return &[];
    }
    let loud = |s: &f32| s.abs() > peak * SILENCE;
    let (Some(first), Some(last)) = (samples.iter().position(loud), samples.iter().rposition(loud)) else { return &[] };
    let margin = (rate * MARGIN_MS / 1000) as usize;
    &samples[first.saturating_sub(margin)..(last + 1 + margin).min(samples.len())]
}

/// `samples` brought to `TARGET_RMS`, as far as `MAX_PEAK` allows.
pub fn levelled(samples: &[f32]) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |peak, s| peak.max(s.abs()));
    let sound: Vec<f32> = samples.iter().copied().filter(|s| s.abs() > peak * SILENCE).collect();
    if sound.is_empty() {
        return samples.to_vec();
    }
    let rms = (sound.iter().map(|s| s * s).sum::<f32>() / sound.len() as f32).sqrt();
    let gain = (TARGET_RMS / rms).min(MAX_PEAK / peak);
    samples.iter().map(|s| s * gain).collect()
}

/// One clip of `parts`, each trimmed and levelled, with each one's pause
/// in milliseconds after it.
pub fn joined(parts: &[(Vec<f32>, u32)], rate: u32) -> Vec<f32> {
    let mut clip = Vec::new();
    for (samples, pause_ms) in parts {
        clip.extend(levelled(trimmed(samples, rate)));
        clip.extend(std::iter::repeat_n(0.0, (rate as u64 * u64::from(*pause_ms) / 1000) as usize));
    }
    clip
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 1000;

    fn tone(silent_before: usize, sound: usize, silent_after: usize, level: f32) -> Vec<f32> {
        let mut samples = vec![0.0; silent_before];
        samples.extend((0..sound).map(|i| if i % 2 == 0 { level } else { -level }));
        samples.extend(vec![0.0; silent_after]);
        samples
    }

    #[test]
    fn the_silence_around_a_part_is_cut_but_for_a_margin() {
        let samples = tone(300, 100, 600, 0.5);
        assert_eq!(trimmed(&samples, RATE).len(), 100 + 2 * MARGIN_MS as usize);
        assert!(trimmed(&[0.0; 50], RATE).is_empty());
        assert_eq!(trimmed(&tone(0, 10, 0, 0.5), RATE).len(), 10);
    }

    #[test]
    fn parts_are_brought_to_one_loudness_without_clipping() {
        let soft = levelled(&tone(0, 100, 0, 0.01));
        let loud = levelled(&tone(0, 100, 0, 0.8));
        assert!((soft[0] - TARGET_RMS).abs() < 1e-4 && (loud[0] - TARGET_RMS).abs() < 1e-4);
        let spiky: Vec<f32> = [vec![0.9], vec![0.01; 99]].concat();
        assert!(levelled(&spiky).iter().all(|s| s.abs() <= MAX_PEAK + 1e-6));
    }

    #[test]
    fn parts_are_joined_with_their_pauses() {
        let clip = joined(&[(tone(300, 100, 600, 0.5), 200), (tone(10, 50, 10, 0.5), 0)], RATE);
        assert_eq!(clip.len(), (100 + 80) + 200 + (50 + 20));
    }
}
