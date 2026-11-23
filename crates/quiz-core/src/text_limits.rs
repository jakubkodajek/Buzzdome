//! How long each text of a quiz can be (spec QZ-13). A text is drawn
//! smaller until it fits its place on the projector, down to a floor
//! (`app/src/host/fit-text.ts`); these lengths keep every text readable at
//! that floor, with the most answers and a picture on the slide. Texts
//! only the quizmaster sees, or the voice reads, are limited so a pasted
//! document does not end up in a field.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Longest quiz title, in characters (the welcome screen's heading when it has none).
pub const MAX_QUIZ_TITLE_CHARS: usize = 80;
/// Longest question text.
pub const MAX_QUESTION_CHARS: usize = 250;
/// Longest answer.
pub const MAX_ANSWER_CHARS: usize = 80;
/// Longest way to read a question aloud.
pub const MAX_READ_AS_CHARS: usize = 500;
/// Longest title of an information, end of round or final slide.
pub const MAX_TITLE_CHARS: usize = 80;
/// Longest text of an information or explanation slide.
pub const MAX_SLIDE_TEXT_CHARS: usize = 400;
/// Longest notes of a slide, shown only in the Director.
pub const MAX_NOTES_CHARS: usize = 2_000;
/// Longest heading of the welcome screen.
pub const MAX_WELCOME_HEADING_CHARS: usize = 80;
/// Longest footer of the welcome screen.
pub const MAX_WELCOME_FOOTER_CHARS: usize = 120;

/// A text of a quiz with a longest length (`TextField::max_chars`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS, JsonSchema)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum TextField {
    QuizTitle,
    Question,
    Answer,
    ReadAs,
    /// The title of an information, end of round or final slide.
    Title,
    /// The text of an information or explanation slide.
    Text,
    Notes,
    WelcomeHeading,
    WelcomeFooter,
}

impl TextField {
    pub const ALL: [TextField; 9] = [
        TextField::QuizTitle,
        TextField::Question,
        TextField::Answer,
        TextField::ReadAs,
        TextField::Title,
        TextField::Text,
        TextField::Notes,
        TextField::WelcomeHeading,
        TextField::WelcomeFooter,
    ];

    /// The most characters the text can have.
    pub fn max_chars(self) -> usize {
        match self {
            TextField::QuizTitle => MAX_QUIZ_TITLE_CHARS,
            TextField::Question => MAX_QUESTION_CHARS,
            TextField::Answer => MAX_ANSWER_CHARS,
            TextField::ReadAs => MAX_READ_AS_CHARS,
            TextField::Title => MAX_TITLE_CHARS,
            TextField::Text => MAX_SLIDE_TEXT_CHARS,
            TextField::Notes => MAX_NOTES_CHARS,
            TextField::WelcomeHeading => MAX_WELCOME_HEADING_CHARS,
            TextField::WelcomeFooter => MAX_WELCOME_FOOTER_CHARS,
        }
    }

    /// The field as the quiz file names it, for logs and AI assistants.
    pub fn json_name(self) -> &'static str {
        match self {
            TextField::QuizTitle => "title",
            TextField::Question => "text",
            TextField::Answer => "answers",
            TextField::ReadAs => "readAs",
            TextField::Title => "title",
            TextField::Text => "text",
            TextField::Notes => "notes",
            TextField::WelcomeHeading => "welcome.heading",
            TextField::WelcomeFooter => "welcome.footer",
        }
    }

    /// Whether `text` is longer than the field takes, counted in characters
    /// as the editor counts them.
    pub fn too_long(self, text: &str) -> bool {
        text.chars().count() > self.max_chars()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_may_be_as_long_as_its_field_takes_in_characters_not_bytes() {
        let longest = "ř".repeat(MAX_ANSWER_CHARS);
        assert!(!TextField::Answer.too_long(&longest), "Czech letters count once");
        assert!(TextField::Answer.too_long(&format!("{longest}a")));
        assert!(TextField::ALL.iter().all(|field| field.max_chars() > 0));
    }
}
