//! Renders with the real voice model when `VOICE_MODEL_ROOT` names a folder
//! it is installed under (`voice::model::installed`); skipped otherwise, as
//! the model is a 129 MB download.

use voice::engine::Engine;
use voice::Line;

#[test]
fn the_model_says_a_czech_question() {
    let Some(root) = std::env::var_os("VOICE_MODEL_ROOT") else {
        eprintln!("VOICE_MODEL_ROOT not set: skipped");
        return;
    };
    let root = std::path::PathBuf::from(root);
    assert!(voice::model::installed(&root), "no model under {}", root.display());
    let engine = Engine::load(&root).unwrap();
    let line = Line { text: "Kolik nohou má pavouk? A: Šest. B: Osm.".into(), language: quiz_core::QuizLanguage::Cs, voice: 3 };
    let clip = engine.render(&line).unwrap();
    let seconds = clip.samples.len() as f32 / clip.rate as f32;
    assert!((1.5..15.0).contains(&seconds), "{seconds} s");
}

/// Checks and unpacks the real download when `VOICE_ARCHIVE` names it.
#[test]
fn the_real_download_unpacks() {
    let Some(archive) = std::env::var_os("VOICE_ARCHIVE") else {
        eprintln!("VOICE_ARCHIVE not set: skipped");
        return;
    };
    let file = std::fs::File::open(&archive).unwrap();
    let sha = voice::model::receive(file, std::io::sink(), |_| {}).unwrap();
    let root = std::env::temp_dir().join(format!("voice-real-unpack-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    voice::model::unpack(std::path::Path::new(&archive), &sha, &root).unwrap();
    assert!(voice::model::installed(&root));
    std::fs::remove_dir_all(&root).unwrap();
}
