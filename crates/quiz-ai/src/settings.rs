//! What the quizmaster sets for AI assistants in the app (File > AI
//! assistants): the folder they work in and the quizmaster's own
//! instructions for every quiz. The app keeps them in its data folder,
//! which is never a root (`roots`), so an assistant cannot change them; the
//! server reads the folder when it starts and the instructions each time an
//! assistant reads the format, so a change to them needs no restart.

use crate::roots::{self, RootError, Roots};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use ts_rs::TS;

/// The file in the app's data folder that keeps the settings.
pub const SETTINGS_FILE: &str = "ai.json";
/// Longest instructions the quizmaster may give, in characters.
pub const MAX_INSTRUCTIONS_CHARS: usize = 4000;
/// The folder in the quizmaster's documents that assistants work in unless
/// they choose another.
const QUIZ_FOLDER: &str = "Buzzdome";

/// A language of the app, which the default instructions are written in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Cs,
    #[default]
    En,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    /// The folder assistants work in, when the quizmaster chose one; else
    /// `QUIZ_FOLDER` in their documents.
    pub folder: Option<PathBuf>,
    /// The quizmaster's own instructions; none for the default ones.
    pub instructions: Option<String>,
    /// The quizmaster's language when they last connected an app or changed
    /// the instructions: the default instructions are given in it, as the
    /// app showed them.
    pub language: Language,
}

impl AiSettings {
    /// The settings kept in `file`; the defaults when there is none or it
    /// cannot be read, which the app reports when it starts.
    pub fn read(file: &Path) -> AiSettings {
        fs::read_to_string(file).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    /// The instructions assistants follow.
    pub fn instructions(&self) -> &str {
        self.instructions.as_deref().unwrap_or(default_instructions(self.language))
    }

    /// Keeps `instructions` as the quizmaster's own, cut to
    /// `MAX_INSTRUCTIONS_CHARS`, or none when they are empty or the default
    /// ones in `language`, so that the default ones keep following the app.
    pub fn set_instructions(&mut self, instructions: &str, language: Language) {
        let instructions: String = instructions.trim().chars().take(MAX_INSTRUCTIONS_CHARS).collect();
        self.language = language;
        self.instructions = (!instructions.is_empty() && instructions != default_instructions(language).trim()).then_some(instructions);
    }

    /// The folder assistants work in, if the system names a documents folder
    /// for the default one.
    pub fn folder(&self) -> Option<PathBuf> {
        self.folder.clone().or_else(|| dirs::document_dir().map(|documents| documents.join(QUIZ_FOLDER)))
    }

    /// The folder assistants work in as their root (`roots::allow`), never
    /// in `app_data`. The default folder is made when it is not there yet; a
    /// chosen one is not, as it may be on a drive that is not plugged in.
    pub fn root(&self, app_data: &[PathBuf]) -> Result<PathBuf, RootError> {
        let folder = self.folder().ok_or(RootError::NoDocumentsFolder)?;
        if self.folder.is_none() {
            fs::create_dir_all(&folder).map_err(|source| RootError::Unusable { path: folder.clone(), source })?;
        }
        roots::allow(&folder, app_data)
    }

    /// The roots of a server started now, and why there are none.
    pub fn roots(&self, app_data: &[PathBuf]) -> (Roots, Option<RootError>) {
        match self.root(app_data) {
            Ok(root) => (Roots::new(vec![root]), None),
            Err(error) => (Roots::default(), Some(error)),
        }
    }
}

/// The instructions assistants follow until the quizmaster writes their own:
/// enough for a whole quiz from a topic alone.
pub fn default_instructions(language: Language) -> &'static str {
    match language {
        Language::Cs => include_str!("instructions/cs.md"),
        Language::En => include_str!("instructions/en.md"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::tests::scratch;

    #[test]
    fn the_default_instructions_are_the_same_rules_in_each_language() {
        let rules = |language| default_instructions(language).lines().filter(|line| line.starts_with("- ")).count();
        assert_eq!(rules(Language::Cs), rules(Language::En));
        for language in [Language::Cs, Language::En] {
            let instructions = default_instructions(language);
            assert!(instructions.chars().count() < MAX_INSTRUCTIONS_CHARS / 2, "{language:?}");
            for slide in ["billboard", "endOfRound", "final", "everyone", "fastestFinger", "explanation", ".quiz"] {
                assert!(instructions.contains(slide), "{language:?} names {slide}");
            }
        }
    }

    #[test]
    fn the_quizmasters_instructions_replace_the_default_ones_until_cleared() {
        let mut settings = AiSettings::default();
        assert_eq!(settings.instructions(), default_instructions(Language::En));
        settings.set_instructions(default_instructions(Language::Cs), Language::Cs);
        assert_eq!(settings.instructions, None, "the default ones are not kept as the quizmaster's");
        assert_eq!(settings.instructions(), default_instructions(Language::Cs));
        settings.set_instructions("  Two rounds of ten questions.\n", Language::Cs);
        assert_eq!(settings.instructions(), "Two rounds of ten questions.");
        settings.set_instructions(&"x".repeat(MAX_INSTRUCTIONS_CHARS + 10), Language::En);
        assert_eq!(settings.instructions().chars().count(), MAX_INSTRUCTIONS_CHARS);
        settings.set_instructions(" ", Language::En);
        assert_eq!(settings.instructions(), default_instructions(Language::En));
    }

    #[test]
    fn settings_are_read_from_the_file_or_are_the_defaults() {
        let dir = scratch("ai-settings");
        let file = dir.join(SETTINGS_FILE);
        assert_eq!(AiSettings::read(&file), AiSettings::default());
        fs::write(&file, "{ broken").unwrap();
        assert_eq!(AiSettings::read(&file), AiSettings::default());
        fs::write(&file, r#"{ "folder": "/quizzes", "instructions": "Kids' quizzes.", "language": "cs", "later": 1 }"#).unwrap();
        let settings = AiSettings::read(&file);
        assert_eq!(settings.folder, Some(PathBuf::from("/quizzes")));
        assert_eq!((settings.instructions(), settings.language), ("Kids' quizzes.", Language::Cs));
    }

    #[test]
    fn the_root_is_the_chosen_folder_but_never_the_apps_data() {
        let dir = scratch("ai-settings-root");
        let chosen = dir.join("quizzes");
        let settings = AiSettings { folder: Some(chosen.clone()), ..AiSettings::default() };
        assert!(matches!(settings.root(&[]), Err(RootError::Unusable { .. })), "a chosen folder is not made");
        fs::create_dir_all(&chosen).unwrap();
        assert_eq!(settings.roots(&[]).0, Roots::new(vec![fs::canonicalize(&chosen).unwrap()]));
        let (roots, refused) = settings.roots(std::slice::from_ref(&dir));
        assert_eq!(roots, Roots::default());
        assert!(matches!(refused, Some(RootError::AppData { .. })));
    }
}
