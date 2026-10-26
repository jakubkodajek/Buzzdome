//! Quiz writing for AI assistants (docs/ai-assistants.md). An assistant
//! writes a quiz as `quiz.json` with its media in a folder, as the quiz file
//! format describes (docs/quiz-format.md), and this crate gives it what it
//! needs to do that well:
//!
//! - the format's JSON Schema, generated from the quiz model (`quiz_schema`);
//! - a skill package for chat assistants that run code in the cloud
//!   (`skill`), with the format guide, the schema and a packing script;
//! - an MCP server for desktop assistants (`mcp`), whose tools check, pack
//!   and fill quiz folders on this computer (`workbench`), only in the
//!   folders the quizmaster chose (`roots`);
//! - the `buzzdome-quiz` command line over the same functions (src/main.rs).
//!
//! Every check is the app's own: the tools read and write quizzes through
//! `quiz_file`, so what they accept is what the editor and the show accept.

pub mod claude_desktop;
pub mod mcp;
pub mod roots;
pub mod skill;
pub mod workbench;

/// The quiz file format, as the people who write quizzes by hand read it.
pub const FORMAT_GUIDE: &str = include_str!("../../../docs/quiz-format.md");

/// The JSON Schema of `quiz.json`, from the quiz model's types and their
/// doc comments. It describes the shape of a quiz; the rules that relate
/// its fields (a correct answer within the answers, a media file that
/// exists) are checked by `workbench::check` and listed in `FORMAT_GUIDE`.
pub fn quiz_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(quiz_core::Quiz)).expect("a JSON Schema serializes: it is plain JSON values")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_requires_what_a_quiz_cannot_read_without() {
        let schema = quiz_schema();
        assert_eq!(schema["title"], "Quiz");
        assert_eq!(schema["required"], serde_json::json!(["schemaVersion", "title"]));
        assert_eq!(schema["properties"]["schemaVersion"]["maximum"], quiz_core::SCHEMA_VERSION);
        let slide = &schema["$defs"]["Slide"];
        let types: Vec<&str> = slide["oneOf"].as_array().unwrap().iter().map(|v| v["properties"]["type"]["const"].as_str().unwrap()).collect();
        let written: Vec<String> = quiz_core::SlideType::ALL.iter().map(|t| serde_json::to_value(t).unwrap().as_str().unwrap().to_owned()).collect();
        assert_eq!(types, written);
        // Media references say where they point, for assistants that check the schema.
        let media = format!("^{}", quiz_core::MEDIA_PREFIX);
        assert_eq!(schema["$defs"]["Slide"]["properties"]["picture"]["pattern"], media);
        assert_eq!(schema["$defs"]["SoundClip"]["properties"]["file"]["pattern"], media);
    }
}
