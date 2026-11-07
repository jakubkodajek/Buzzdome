//! Reading questions aloud offline (spec SN-10). The app keeps one neural
//! voice model (`model`), downloaded once; the voice process (`server`)
//! renders each question's words into a WAV clip with it (`engine`), and
//! the app keeps the clips by what they say (`clip_name`), so a show plays
//! them without waiting and without a network.

pub mod engine;
pub mod model;
pub mod server;
pub mod wav;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use quiz_core::QuizLanguage;
use ts_rs::TS;

pub use model::{MODEL, MODEL_FILES};

/// Longest text a clip is rendered for, in characters; more is cut.
pub const MAX_TEXT_CHARS: usize = 2000;

/// Raised when a change makes the same words sound different, so that
/// clips kept from before are rendered again.
const RENDER_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    #[error("download failed: {0}")]
    Download(String),
    /// The download or the archive is not the model it should be.
    #[error("the voice model is damaged")]
    Damaged,
    #[error("disk: {0}")]
    Disk(String),
    /// The one receiving it stopped the download (`model::receive`).
    #[error("the download was stopped")]
    Stopped,
    #[error("the voice model could not be loaded")]
    Load,
    #[error("the words could not be rendered")]
    Render,
}

impl VoiceError {
    pub(crate) fn download(e: std::io::Error) -> Self {
        VoiceError::Download(e.to_string())
    }

    pub(crate) fn disk(e: std::io::Error) -> Self {
        VoiceError::Disk(e.to_string())
    }
}

/// What to say, and how: one clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "VoiceLine")]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub text: String,
    pub language: QuizLanguage,
    /// The model's voice, from 0.
    pub voice: u8,
}

impl Line {
    /// The words as the engine takes them: on one line, no control
    /// characters, at most `MAX_TEXT_CHARS`.
    pub fn words(&self) -> String {
        let cleaned: String = self.text.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_TEXT_CHARS).collect();
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// The line as the app takes it from a window: its words only, and a
    /// voice the model has, so that one clip is kept for what sounds the same.
    pub fn checked(self) -> Line {
        Line { text: self.words(), voice: self.voice.min(quiz_core::MAX_VOICE), ..self }
    }

    /// Whether there is anything to say.
    pub fn is_empty(&self) -> bool {
        self.words().is_empty()
    }

    /// The file name a clip of this line is kept under: the same words,
    /// language, voice and model always give the same name, and anything
    /// else another.
    pub fn clip_name(&self) -> String {
        let mut hasher = Sha256::new();
        let key = serde_json::json!([RENDER_VERSION, MODEL.id, self.language, self.voice, self.words()]);
        hasher.update(key.to_string().as_bytes());
        format!("{}.wav", model::hex(&hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> Line {
        Line { text: text.into(), language: QuizLanguage::Cs, voice: 0 }
    }

    #[test]
    fn a_checked_line_has_its_words_and_a_voice_the_model_has() {
        let checked = Line { text: " Kdo\n je  tu? ".into(), language: QuizLanguage::Cs, voice: 200 }.checked();
        assert_eq!((checked.text.as_str(), checked.voice), ("Kdo je tu?", quiz_core::MAX_VOICE));
        assert_eq!(checked.clip_name(), Line { voice: quiz_core::MAX_VOICE, ..checked.clone() }.clip_name());
    }

    #[test]
    fn words_are_on_one_line_without_control_characters() {
        assert_eq!(line("  Kolik\n nohou\0má\tpavouk? ").words(), "Kolik nohou má pavouk?");
        assert!(line(" \n ").is_empty());
        assert_eq!(line(&"a".repeat(MAX_TEXT_CHARS + 10)).words().chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn a_clip_is_named_by_what_it_says_and_how() {
        let name = line("Kolik nohou má pavouk?").clip_name();
        assert_eq!(name, line(" Kolik  nohou má\npavouk? ").clip_name());
        assert_ne!(name, line("Kolik nohou má moucha?").clip_name());
        assert_ne!(name, Line { voice: 1, ..line("Kolik nohou má pavouk?") }.clip_name());
        assert_ne!(name, Line { language: QuizLanguage::En, ..line("Kolik nohou má pavouk?") }.clip_name());
        assert!(name.ends_with(".wav") && name.len() == 68);
    }
}
