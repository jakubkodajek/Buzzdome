//! Writes down what is said in a recording, offline, with the speech
//! recognition model (`model::LISTENER`, Whisper through sherpa-onnx), so
//! that the AI panel's model, which reads no sound, gets a recording's
//! words as text. Used only in the listening process, which the app starts
//! for one recording (`app/src-tauri/src/listen.rs`).

use crate::model::{LISTENER, LISTENER_FILES};
use crate::VoiceError;
use quiz_core::QuizLanguage;
use sherpa_onnx::{OfflineModelConfig, OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig};
use std::ops::Range;
use std::path::Path;

/// Samples a second the model hears (Whisper's own rate).
pub const RATE: usize = 16_000;
/// The longest recording written down: longer ones are cut, so that one
/// takes a few minutes at most on a laptop's CPU.
pub const MAX_SECONDS: usize = 15 * 60;
/// Whisper hears at most 30 s at once (Radford et al., Whisper paper,
/// https://arxiv.org/abs/2212.04356): a recording is heard in pieces of at
/// most this much.
const MAX_PIECE_SECONDS: usize = 28;
/// A piece ends at the quietest moment of its last seconds, so that a word
/// is seldom cut in two.
const LOOK_BACK_SECONDS: usize = 6;
/// The stretch whose loudness is compared, 50 ms.
const FRAME: usize = RATE / 20;
/// CPU threads the model uses.
const MAX_THREADS: usize = 4;

/// The loaded model.
pub struct Listener {
    recognizer: OfflineRecognizer,
}

impl Listener {
    /// Loads the model installed under `root` (`ModelSource::installed`),
    /// to hear `language`.
    pub fn load(root: &Path, language: QuizLanguage) -> Result<Listener, VoiceError> {
        let dir = LISTENER.dir(root);
        let file = |name: &str| Some(dir.join(name).to_string_lossy().into_owned());
        let [encoder, decoder, tokens] = LISTENER_FILES.map(file);
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1).clamp(1, MAX_THREADS));
        let config = OfflineRecognizerConfig {
            model_config: OfflineModelConfig {
                whisper: OfflineWhisperModelConfig { encoder, decoder, language: Some(language.code().into()), task: Some("transcribe".into()), tail_paddings: -1, ..Default::default() },
                tokens,
                num_threads: threads as i32,
                provider: Some("cpu".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        OfflineRecognizer::create(&config).map(|recognizer| Listener { recognizer }).ok_or(VoiceError::Load)
    }

    /// What is said in `samples` (`RATE` a second, -1 to 1), piece by
    /// piece (`pieces`); `heard` is told how many seconds are done.
    pub fn transcribe(&self, samples: &[f32], mut heard: impl FnMut(usize)) -> Result<String, VoiceError> {
        let mut words: Vec<String> = Vec::new();
        for piece in pieces(samples) {
            let end = piece.end;
            let stream = self.recognizer.create_stream();
            stream.accept_waveform(RATE as i32, &samples[piece]);
            self.recognizer.decode(&stream);
            let text = stream.get_result().ok_or(VoiceError::Render)?.text;
            let text = text.trim();
            if !text.is_empty() {
                words.push(text.to_string());
            }
            heard(end / RATE);
        }
        Ok(words.join(" "))
    }
}

/// `samples` at `rate` a second as the model hears them, `RATE` a second.
pub fn resampled(samples: &[f32], rate: u32) -> Vec<f32> {
    match sherpa_onnx::LinearResampler::create(rate as i32, RATE as i32) {
        Some(resampler) if rate as usize != RATE => resampler.resample(samples, true),
        _ => samples.to_vec(),
    }
}

/// Where `samples` are cut into pieces Whisper hears whole: at most
/// `MAX_PIECE_SECONDS` each, each ending at the quietest `FRAME` of its
/// last `LOOK_BACK_SECONDS`, at most `MAX_SECONDS` in all. Silence alone is
/// no piece.
pub fn pieces(samples: &[f32]) -> Vec<Range<usize>> {
    let samples = &samples[..samples.len().min(MAX_SECONDS * RATE)];
    let (most, back) = (MAX_PIECE_SECONDS * RATE, LOOK_BACK_SECONDS * RATE);
    let loudness = |at: usize| samples[at..(at + FRAME).min(samples.len())].iter().map(|s| s * s).sum::<f32>();
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        let end = if samples.len() - start <= most {
            samples.len()
        } else {
            let last = start + most;
            (last - back..last).step_by(FRAME).min_by(|a, b| loudness(*a).total_cmp(&loudness(*b))).map_or(last, |quiet| quiet + FRAME / 2)
        };
        if samples[start..end].iter().any(|s| s.abs() > SILENCE) {
            pieces.push(start..end);
        }
        start = end;
    }
    pieces
}

/// Louder than this is sound (about -54 dBFS) [no source - common convention for a silence gate].
const SILENCE: f32 = 0.002;

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: usize) -> Vec<f32> {
        (0..seconds * RATE).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect()
    }

    #[test]
    fn a_short_recording_is_one_piece_and_silence_none() {
        let one = pieces(&tone(10));
        assert_eq!((one.len(), one[0].clone()), (1, 0..10 * RATE));
        assert!(pieces(&vec![0.0; 40 * RATE]).is_empty());
        assert!(pieces(&[]).is_empty());
    }

    #[test]
    fn a_long_recording_is_cut_where_it_is_quiet() {
        let mut samples = tone(60);
        // A pause at 25 s, within the last seconds of the first piece.
        let pause = 25 * RATE..25 * RATE + RATE / 2;
        samples[pause.clone()].fill(0.0);
        let cut = pieces(&samples);
        assert!(cut.len() >= 3, "{cut:?}");
        assert!(pause.contains(&cut[0].end), "cut at {} s", cut[0].end as f32 / RATE as f32);
        // The pieces follow each other, none longer than Whisper hears.
        for pair in cut.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        assert!(cut.iter().all(|piece| piece.len() <= MAX_PIECE_SECONDS * RATE));
        assert_eq!(cut.last().unwrap().end, samples.len());
    }

    #[test]
    fn at_most_the_longest_recording_is_heard() {
        let cut = pieces(&tone(MAX_SECONDS + 60));
        assert_eq!(cut.last().unwrap().end, MAX_SECONDS * RATE);
    }
}
