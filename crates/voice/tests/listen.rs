//! Writes down speech with the real speech recognition model when
//! `LISTENER_MODEL_ROOT` names a folder it is installed under, and
//! `VOICE_MODEL_ROOT` one the voice model is under: the voice says a
//! Czech question and the listener must hear its words. Skipped otherwise,
//! as the models are a 564 MB and a 129 MB download.

use voice::engine::Engine;
use voice::listen::{Listener, RATE};
use voice::Line;

#[test]
fn the_listener_hears_what_the_voice_says() {
    let (Some(heard_root), Some(voice_root)) = (std::env::var_os("LISTENER_MODEL_ROOT"), std::env::var_os("VOICE_MODEL_ROOT")) else {
        eprintln!("LISTENER_MODEL_ROOT or VOICE_MODEL_ROOT not set: skipped");
        return;
    };
    let (heard_root, voice_root) = (std::path::PathBuf::from(heard_root), std::path::PathBuf::from(voice_root));
    assert!(voice::LISTENER.installed(&heard_root), "no listener under {}", heard_root.display());
    let line = Line {
        question: "Která řeka protéká Prahou? Je to Vltava, nejdelší řeka v Česku.".into(),
        answers: vec![],
        language: quiz_core::QuizLanguage::Cs,
        voice: 0,
        speed: quiz_core::DEFAULT_VOICE_SPEED,
    };
    let clip = Engine::load(&voice_root).unwrap().render(&line).unwrap();
    let samples = voice::listen::resampled(&clip.samples, clip.rate);
    let started = std::time::Instant::now();
    let listener = Listener::load(&heard_root, quiz_core::QuizLanguage::Cs).unwrap();
    let loaded = started.elapsed();
    let text = listener.transcribe(&samples, |_| {}).unwrap();
    println!("loaded in {loaded:?}, {:.1} s heard in {:?}: {text}", samples.len() as f32 / RATE as f32, started.elapsed() - loaded);
    let text = text.to_lowercase();
    assert!(text.contains("vltav") && text.contains("prah"), "{text}");
}
