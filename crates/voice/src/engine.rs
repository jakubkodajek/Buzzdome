//! Renders words into speech with the voice model, through sherpa-onnx
//! (ONNX Runtime, on the CPU). Used only in the voice process.

use crate::model::{model_dir, MODEL_FILES};
use crate::{audio, Line, VoiceError};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig, OfflineTtsSupertonicModelConfig};
use std::collections::HashMap;
use std::path::Path;

/// Denoising steps per part: more sound cleaner and take longer. Twelve is
/// the model's own default for its best quality (Supertone's supertonic
/// Python package, `total_steps`); sherpa-onnx's example takes eight. In
/// our measurement a 9 s question renders in about 3 s either way.
const STEPS: i32 = 12;
/// CPU threads the engine uses: the show's own threads (projector, video,
/// lights) keep the rest of an event laptop's.
const MAX_THREADS: usize = 2;

/// A clip: samples from -1 to 1, `rate` per second.
pub struct Clip {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// The loaded model.
pub struct Engine {
    tts: OfflineTts,
}

impl Engine {
    /// Loads the model installed under `root` (`model::installed`).
    pub fn load(root: &Path) -> Result<Engine, VoiceError> {
        let dir = model_dir(root);
        let file = |name: &str| Some(dir.join(name).to_string_lossy().into_owned());
        let [duration_predictor, text_encoder, vector_estimator, vocoder, tts_json, unicode_indexer, voice_style] = MODEL_FILES.map(file);
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1).clamp(1, MAX_THREADS));
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                supertonic: OfflineTtsSupertonicModelConfig { duration_predictor, text_encoder, vector_estimator, vocoder, tts_json, unicode_indexer, voice_style },
                num_threads: threads as i32,
                provider: Some("cpu".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        OfflineTts::create(&config).map(|tts| Engine { tts }).ok_or(VoiceError::Load)
    }

    /// Says `line`: each of its parts on its own (`Line::parts`), put
    /// together with the pauses between them (`audio::joined`).
    pub fn render(&self, line: &Line) -> Result<Clip, VoiceError> {
        let parts = line.parts();
        if parts.is_empty() {
            return Err(VoiceError::Render);
        }
        let voices = self.tts.num_speakers().max(1);
        let config = GenerationConfig {
            sid: i32::from(line.voice).min(voices - 1),
            num_steps: STEPS,
            speed: f32::from(line.speed) / 100.0,
            extra: Some(HashMap::from([("lang".to_string(), serde_json::to_value(line.language).map_err(|_| VoiceError::Render)?)])),
            ..Default::default()
        };
        let mut rate = 0;
        let mut rendered = Vec::with_capacity(parts.len());
        for part in &parts {
            let audio = self.tts.generate_with_config(&part.words, &config, None::<fn(&[f32], f32) -> bool>).ok_or(VoiceError::Render)?;
            rate = u32::try_from(audio.sample_rate()).map_err(|_| VoiceError::Render)?;
            rendered.push((audio.samples().to_vec(), part.pause_ms));
        }
        let samples = audio::joined(&rendered, rate);
        if samples.is_empty() {
            return Err(VoiceError::Render);
        }
        Ok(Clip { samples, rate })
    }
}
