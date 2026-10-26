//! The skill package for chat assistants that run code in the cloud (Agent
//! Skills, a `SKILL.md` with the files it uses): what a quiz is, the format
//! guide and schema, and a script that checks a quiz folder and packs it,
//! since the assistant cannot run this app. It is uploaded as a ZIP with
//! the skill's folder at its root, as Claude takes it; Gemini and ChatGPT
//! take the same folder.

use quiz_core::{AnswerMode, MediaKind};
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

/// `SKILL.md` with the core's limits written in, so it never says other
/// numbers or file types than the app takes.
fn skill_md() -> String {
    let extensions = |kind: MediaKind| {
        let names: Vec<String> = kind.extensions().iter().map(|extension| format!(".{extension}")).collect();
        let (last, rest) = names.split_last().expect("every kind has extensions");
        if rest.is_empty() { last.clone() } else { format!("{} or {last}", rest.join(", ")) }
    };
    let everyone = AnswerMode::Everyone.answer_limits();
    [
        ("{{EVERYONE_ANSWERS}}", format!("{} to {}", everyone.fewest, everyone.most)),
        ("{{FASTEST_FINGER_ANSWERS}}", AnswerMode::FastestFinger.answer_limits().most.to_string()),
        ("{{PICTURES}}", extensions(MediaKind::Image)),
        ("{{SOUNDS}}", extensions(MediaKind::Audio)),
        ("{{VIDEOS}}", extensions(MediaKind::Video)),
    ]
    .into_iter()
    .fold(SKILL_MD.to_string(), |text, (placeholder, value)| text.replace(placeholder, &value))
}

/// The package's files, by their path in it.
pub fn files() -> Vec<(String, String)> {
    let schema = serde_json::to_string_pretty(&crate::quiz_schema()).expect("a JSON value serializes");
    [
        ("SKILL.md", skill_md()),
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
        let skill = skill_md();
        let front = skill.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).unwrap().0;
        let field = |name: &str| front.lines().find_map(|line| line.strip_prefix(&format!("{name}: "))).unwrap();
        assert_eq!(field("name"), NAME);
        assert!(field("description").chars().count() <= 200);
    }

    #[test]
    fn the_skill_gives_the_cores_limits() {
        let skill = skill_md();
        assert!(!skill.contains("{{"), "a placeholder is left");
        assert!(skill.contains("has 2 to 4 answers") && skill.contains("up to 6 answers"));
        assert!(skill.contains("Videos: .mp4 or .webm"));
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
    fn the_script_packs_the_media_of_every_media_field() {
        let dir = crate::workbench::tests::scratch("skill-script-media");
        let skill = unpacked_skill(&dir);
        let quiz = dir.join("quiz");
        std::fs::create_dir_all(quiz.join("media")).unwrap();
        let json = serde_json::json!({
            "schemaVersion": quiz_core::SCHEMA_VERSION,
            "title": "Every medium",
            "categories": [{ "id": "c1", "name": "Films", "background": { "file": "media/category.webm" } }],
            "slides": [
                { "id": "b1", "type": "billboard", "title": "Round", "background": { "file": "media/background.png" },
                  "sound": { "file": "media/intro.mp3" } },
                { "id": "q1", "type": "question", "mode": "everyone", "text": "Which?", "answers": ["A", "B"],
                  "correct": [{ "answer": 0 }], "picture": "media/picture.jpg" },
                { "id": "q2", "type": "question", "mode": "everyone", "text": "Which?", "answers": ["A", "B"],
                  "correct": [{ "answer": 1 }], "video": { "file": "media/clip.mp4" } },
            ]
        });
        let read = quiz_core::Quiz::read_json(&json.to_string()).unwrap();
        assert!(read.dropped.is_empty(), "{:?}", read.dropped);
        let media: Vec<String> = read.quiz.media_files().into_iter().map(str::to_owned).collect();
        assert_eq!(media.len(), 5, "the quiz uses every media field");
        for file in &media {
            std::fs::write(quiz.join(file), b"x").unwrap();
        }
        std::fs::write(quiz.join("quiz.json"), json.to_string()).unwrap();
        let output = dir.join("media.quiz");
        let run = make_quiz(&skill, &quiz, &output);
        assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
        let mut names = entry_names(&mut zip::ZipArchive::new(std::fs::File::open(&output).unwrap()).unwrap());
        names.retain(|name| name != quiz_file::QUIZ_JSON);
        assert_eq!(names, media);
    }

    #[test]
    fn the_script_packs_nothing_from_outside_the_media_folder() {
        let dir = crate::workbench::tests::scratch("skill-script-names");
        let skill = unpacked_skill(&dir);
        let quiz = dir.join("quiz");
        std::fs::create_dir_all(quiz.join("media/sub")).unwrap();
        std::fs::write(dir.join("secret.png"), b"x").unwrap();
        std::fs::write(quiz.join("media/sub/a.png"), b"x").unwrap();
        for picture in ["media/../../secret.png", "media/sub/a.png", "media\\..\\..\\secret.png", "media/CON.png", "media/a.png."] {
            std::fs::write(quiz.join("quiz.json"), crate::workbench::tests::quiz_with_picture(picture)).unwrap();
            let output = dir.join("bad.quiz");
            let run = make_quiz(&skill, &quiz, &output);
            assert_eq!(run.status.code(), Some(1), "{picture}");
            let said = String::from_utf8_lossy(&run.stderr);
            assert!(said.contains("media/<file name>"), "{picture}: {said}");
            assert!(!output.exists());
        }
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
