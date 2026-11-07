//! Reading questions aloud offline (spec SN-10). The app keeps one neural
//! voice model (`model`), downloaded once; the voice process (`server`)
//! renders each question's words into a WAV clip with it (`engine`), and
//! the app keeps the clips by what they say (`clip_name`), so a show plays
//! them without waiting and without a network.

pub mod audio;
pub mod engine;
pub mod model;
pub mod server;
pub mod speech;
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
const RENDER_VERSION: u32 = 2;

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

/// What to say, and how: one clip, a question and its answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export, rename = "VoiceLine")]
#[serde(rename_all = "camelCase")]
pub struct Line {
    /// The question's words (`quiz_core::Question::read_as`, or its text).
    pub question: String,
    /// The answers read after it, each after its letter by its place: an
    /// empty one is passed over and keeps its letter.
    #[serde(default)]
    pub answers: Vec<String>,
    pub language: QuizLanguage,
    /// The model's voice, from 0.
    pub voice: u8,
    /// How fast it reads, in percent of its own pace
    /// (`quiz_core::QuizSettings::voice_speed`).
    #[serde(default = "default_speed")]
    pub speed: u8,
}

fn default_speed() -> u8 {
    quiz_core::DEFAULT_VOICE_SPEED
}

/// One piece of a clip, rendered on its own, and the silence after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub words: String,
    pub pause_ms: u32,
}

/// The silence after the question when answers follow it, after an
/// answer's letter, and after an answer [no source - common convention for
/// read-out lists; tuned by ear].
const PAUSE_AFTER_QUESTION_MS: u32 = 600;
const PAUSE_AFTER_LETTER_MS: u32 = 150;
const PAUSE_AFTER_ANSWER_MS: u32 = 450;

/// `text` on one line, with no control characters, at most `MAX_TEXT_CHARS`.
fn cleaned(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).take(MAX_TEXT_CHARS).collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `words` ending as a sentence ends, so the voice falls at the end.
fn sentence(mut words: String) -> String {
    if !words.ends_with(['.', '!', '?', '…', ':', ';']) {
        words.push('.');
    }
    words
}

impl Line {
    /// The line as the app takes it from a window: its texts cleaned, at
    /// most `quiz_core::MAX_ANSWERS` answers, and a voice and speed the
    /// model has, so that one clip is kept for what sounds the same.
    pub fn checked(self) -> Line {
        let mut answers: Vec<String> = self.answers.iter().take(quiz_core::MAX_ANSWERS).map(|answer| cleaned(answer)).collect();
        while answers.last().is_some_and(String::is_empty) {
            answers.pop();
        }
        Line {
            question: cleaned(&self.question),
            answers,
            voice: self.voice.min(quiz_core::MAX_VOICE),
            speed: self.speed.clamp(quiz_core::MIN_VOICE_SPEED, quiz_core::MAX_VOICE_SPEED),
            ..self
        }
    }

    /// What is said, piece by piece, as it is said (`speech::spoken`): the
    /// question, then each answer's letter and the answer.
    pub fn parts(&self) -> Vec<Part> {
        let spoken = |text: &str| speech::spoken(&cleaned(text), self.language);
        let mut parts = Vec::new();
        let question = spoken(&self.question);
        if !question.is_empty() {
            parts.push(Part { words: sentence(question), pause_ms: PAUSE_AFTER_QUESTION_MS });
        }
        for (i, answer) in self.answers.iter().enumerate() {
            let answer = spoken(answer);
            let letter = speech::letter(i, self.language);
            if answer.is_empty() || letter.is_empty() {
                continue;
            }
            parts.push(Part { words: sentence(letter.into()), pause_ms: PAUSE_AFTER_LETTER_MS });
            parts.push(Part { words: sentence(answer), pause_ms: PAUSE_AFTER_ANSWER_MS });
        }
        if let Some(last) = parts.last_mut() {
            last.pause_ms = 0;
        }
        parts
    }

    /// Everything said, in one text: for the system's voice, which reads
    /// when there is no clip, and to tell how long a clip takes.
    pub fn words(&self) -> String {
        self.parts().into_iter().map(|part| part.words).collect::<Vec<_>>().join(" ")
    }

    /// Whether there is anything to say.
    pub fn is_empty(&self) -> bool {
        self.parts().is_empty()
    }

    /// The file name a clip of this line is kept under: the same words,
    /// language, voice, speed and model always give the same name, and
    /// anything else another.
    pub fn clip_name(&self) -> String {
        let mut hasher = Sha256::new();
        let parts: Vec<(String, u32)> = self.parts().into_iter().map(|part| (part.words, part.pause_ms)).collect();
        let key = serde_json::json!([RENDER_VERSION, MODEL.id, self.language, self.voice, self.speed, parts]);
        hasher.update(key.to_string().as_bytes());
        format!("{}.wav", model::hex(&hasher.finalize()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(question: &str, answers: &[&str]) -> Line {
        Line { question: question.into(), answers: answers.iter().map(|a| (*a).into()).collect(), language: QuizLanguage::Cs, voice: 0, speed: 100 }
    }

    fn words(parts: &[Part]) -> Vec<&str> {
        parts.iter().map(|part| part.words.as_str()).collect()
    }

    #[test]
    fn a_checked_line_has_its_words_and_a_voice_and_speed_the_model_has() {
        let checked = Line { voice: 200, speed: 7, ..line(" Kdo\n je  tu? ", &["Já", "", " "]) }.checked();
        assert_eq!((checked.question.as_str(), checked.answers.as_slice(), checked.voice), ("Kdo je tu?", ["Já".to_string()].as_slice(), quiz_core::MAX_VOICE));
        assert_eq!(checked.speed, quiz_core::MIN_VOICE_SPEED);
        assert_eq!(checked.clip_name(), Line { voice: quiz_core::MAX_VOICE, ..checked.clone() }.clip_name());
    }

    #[test]
    fn a_line_is_said_in_parts_each_letter_alone_and_with_pauses_between() {
        let parts = line("Kolik nohou má pavouk", &["Šest", "", "Osm!"]).parts();
        assert_eq!(words(&parts), ["Kolik nohou má pavouk.", "Á.", "Šest.", "Cé.", "Osm!"]);
        let pauses: Vec<u32> = parts.iter().map(|part| part.pause_ms).collect();
        assert_eq!(pauses, [PAUSE_AFTER_QUESTION_MS, PAUSE_AFTER_LETTER_MS, PAUSE_AFTER_ANSWER_MS, PAUSE_AFTER_LETTER_MS, 0]);
        assert_eq!(words(&line("Kdo?", &[]).parts()), ["Kdo?"]);
        assert_eq!(words(&Line { language: QuizLanguage::En, ..line("Who", &["Me"]) }.parts()), ["Who.", "A.", "Me."]);
        assert!(line(" \n ", &["", " "]).is_empty());
    }

    #[test]
    fn a_line_is_said_as_spoken_words() {
        assert_eq!(line("Kdy skončila 2. světová válka?", &["1945", "1918"]).words(), "Kdy skončila druhá světová válka? Á. tisíc devět set čtyřicet pět. Bé. tisíc devět set osmnáct.");
    }

    #[test]
    fn words_are_on_one_line_without_control_characters() {
        assert_eq!(line("  Kolik\n nohou\0má\tpavouk? ", &[]).words(), "Kolik nohou má pavouk?");
        assert_eq!(cleaned(&"a".repeat(MAX_TEXT_CHARS + 10)).chars().count(), MAX_TEXT_CHARS);
    }

    #[test]
    fn a_clip_is_named_by_what_it_says_and_how() {
        let name = line("Kolik nohou má pavouk?", &["Osm"]).clip_name();
        assert_eq!(name, line(" Kolik  nohou má\npavouk? ", &["Osm "]).clip_name());
        assert_ne!(name, line("Kolik nohou má moucha?", &["Osm"]).clip_name());
        assert_ne!(name, line("Kolik nohou má pavouk?", &[]).clip_name());
        assert_ne!(name, Line { voice: 1, ..line("Kolik nohou má pavouk?", &["Osm"]) }.clip_name());
        assert_ne!(name, Line { speed: 90, ..line("Kolik nohou má pavouk?", &["Osm"]) }.clip_name());
        assert_ne!(name, Line { language: QuizLanguage::En, ..line("Kolik nohou má pavouk?", &["Osm"]) }.clip_name());
        assert!(name.ends_with(".wav") && name.len() == 68);
    }
}
