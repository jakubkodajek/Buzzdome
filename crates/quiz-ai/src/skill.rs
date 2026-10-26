//! The skill package for chat assistants that run code in the cloud (Agent
//! Skills, a `SKILL.md` with the files it uses): what a quiz is, the format
//! guide and schema, and a script that checks a quiz folder and packs it,
//! since the assistant cannot run this app. It is uploaded as a ZIP with
//! the skill's folder at its root, as Claude takes it; Gemini and ChatGPT
//! take the same folder.

use quiz_file::{FileAction, QuizFileError};
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// The skill's name, also its folder in the package.
pub const NAME: &str = "buzzdome-quiz";
/// File extension of the package.
pub const EXTENSION: &str = "zip";

const SKILL_MD: &str = include_str!("skill/SKILL.md");
const MAKE_QUIZ_PY: &str = include_str!("skill/make_quiz.py");

/// The package's files, by their path in it.
pub fn files() -> Vec<(String, String)> {
    let schema = serde_json::to_string_pretty(&crate::quiz_schema()).expect("a JSON value serializes");
    [
        ("SKILL.md", SKILL_MD.to_string()),
        ("reference/quiz-format.md", crate::FORMAT_GUIDE.to_string()),
        ("reference/quiz.schema.json", schema),
        ("scripts/make_quiz.py", MAKE_QUIZ_PY.to_string()),
    ]
    .into_iter()
    .map(|(path, contents)| (format!("{NAME}/{path}"), contents))
    .collect()
}

/// Writes the package to `dest`, replacing it only once it is complete.
pub fn write(dest: &Path) -> Result<(), QuizFileError> {
    let failed = |e: zip::result::ZipError| QuizFileError::Io { action: FileAction::Write, path: dest.to_path_buf(), source: std::io::Error::other(e) };
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (path, contents) in files() {
        zip.start_file(path, SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(failed)?;
        zip.write_all(contents.as_bytes()).map_err(|e| failed(e.into()))?;
    }
    let bytes = zip.finish().map_err(failed)?.into_inner();
    quiz_file::write_atomic(dest, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn the_package_holds_the_skill_folder_with_its_files() {
        let dir = crate::workbench::tests::scratch("skill");
        let dest = dir.join("buzzdome-quiz.zip");
        write(&dest).unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let names = entry_names(&mut zip);
        assert_eq!(
            names,
            ["buzzdome-quiz/SKILL.md", "buzzdome-quiz/reference/quiz-format.md", "buzzdome-quiz/reference/quiz.schema.json", "buzzdome-quiz/scripts/make_quiz.py"]
        );
        let mut schema = String::new();
        zip.by_name("buzzdome-quiz/reference/quiz.schema.json").unwrap().read_to_string(&mut schema).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&schema).unwrap(), crate::quiz_schema());
    }

    /// Claude takes a skill's name up to 64 characters and its description
    /// up to 200 ("How to create custom skills", Claude Help Center).
    #[test]
    fn the_skill_says_its_name_and_when_to_use_it_within_the_limits() {
        let front = SKILL_MD.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).unwrap().0;
        let field = |name: &str| front.lines().find_map(|line| line.strip_prefix(&format!("{name}: "))).unwrap();
        assert_eq!(field("name"), NAME);
        assert!(field("description").chars().count() <= 200);
    }

    fn entry_names(zip: &mut zip::ZipArchive<std::fs::File>) -> Vec<String> {
        let mut names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned()).collect();
        names.sort();
        names
    }

    /// Runs the skill's script, as the assistant would, on a quiz folder.
    fn make_quiz(skill: &Path, quiz: &Path, output: &Path) -> std::process::Output {
        let python = if cfg!(windows) { "python" } else { "python3" };
        std::process::Command::new(python)
            .arg(skill.join("scripts/make_quiz.py"))
            .arg(quiz)
            .arg("-o")
            .arg(output)
            .output()
            .expect("Python 3 runs the skill's script, as in the assistants' sandboxes")
    }

    fn unpacked_skill(dir: &Path) -> std::path::PathBuf {
        for (path, contents) in files() {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, contents).unwrap();
        }
        dir.join(NAME)
    }

    #[test]
    fn the_script_packs_a_quiz_the_app_opens() {
        let dir = crate::workbench::tests::scratch("skill-script");
        let skill = unpacked_skill(&dir);
        let quiz = dir.join("quiz");
        std::fs::create_dir_all(quiz.join("media")).unwrap();
        std::fs::write(quiz.join("media/brno.jpg"), b"jpeg").unwrap();
        std::fs::write(quiz.join("media/unused.png"), b"png").unwrap();
        std::fs::write(quiz.join("quiz.json"), crate::workbench::tests::quiz_with_picture("media/brno.jpg")).unwrap();
        let output = dir.join("Capitals.quiz");
        let run = make_quiz(&skill, &quiz, &output);
        assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        let check = crate::workbench::check(&output).unwrap();
        assert!(check.is_ready(), "{check:?}");
        let names = entry_names(&mut zip::ZipArchive::new(std::fs::File::open(&output).unwrap()).unwrap());
        assert_eq!(names, ["media/brno.jpg", "quiz.json"]);
    }

    #[test]
    fn the_script_says_what_is_wrong_and_writes_nothing() {
        let dir = crate::workbench::tests::scratch("skill-script-wrong");
        let skill = unpacked_skill(&dir);
        let quiz = dir.join("quiz");
        std::fs::create_dir_all(&quiz).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&crate::workbench::tests::quiz_with_picture("media/brno.jpg")).unwrap();
        json["slides"][1]["correct"][0]["answer"] = "A".into();
        json["slides"][2]["type"] = "scores".into();
        json["slides"][0].as_object_mut().unwrap().remove("title");
        std::fs::write(quiz.join("quiz.json"), json.to_string()).unwrap();
        let output = dir.join("broken.quiz");
        let run = make_quiz(&skill, &quiz, &output);
        assert_eq!(run.status.code(), Some(1));
        let said = String::from_utf8_lossy(&run.stderr);
        for problem in [
            "quiz.slides[0]: missing title",
            "quiz.slides[1].correct[0].answer: expected integer, found string",
            "quiz.slides[2].type: must be one of",
            "media/brno.jpg: the file is missing",
        ] {
            assert!(said.contains(problem), "{problem:?} not in {said}");
        }
        assert!(!output.exists());
    }
}
