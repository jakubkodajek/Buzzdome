//! Crash drills: random shows (`show_sim`) played through the app's own
//! journal, killed at random moments, sometimes in the middle of writing a
//! line, and resumed as the Director's Resume does (`resume_show`). After
//! every resume the show must be the one the windows last saw, with a
//! question that was in play restarted. `SOAK_SHOWS` sets how many shows,
//! `SOAK_FIRST` the first seed, `SOAK_SEED` replays one that failed.

use super::*;
use show_sim::{random_quiz, Rng, Show, MAX_BUZZERS};
use std::io::Write;

/// What a crash leaves at the end of the journal.
#[derive(Debug, Clone, Copy)]
enum Tail {
    /// The last command was on disk before the app stopped.
    Clean,
    /// The app stopped while writing a command: part of a line, no newline.
    Torn,
    /// The disk kept the newline but not the bytes before it.
    Zeros,
    /// A whole line that is not JSON at all.
    Garbage,
}

fn journal_path(root: &Path) -> PathBuf {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root).unwrap().map(|entry| entry.unwrap().path()).filter(|path| path.is_dir()).collect();
    dirs.sort_by_key(|dir| dir.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse::<u64>().ok()));
    dirs.last().expect("no session folder").join("journal.jsonl")
}

fn leave_tail(root: &Path, tail: Tail, next: &Stamped) {
    let mut file = fs::OpenOptions::new().append(true).open(journal_path(root)).unwrap();
    let line = serde_json::to_string(next).unwrap();
    match tail {
        Tail::Clean => {}
        Tail::Torn => file.write_all(&line.as_bytes()[..line.len() / 2]).unwrap(),
        Tail::Zeros => file.write_all(&[0u8; 12]).and_then(|()| file.write_all(b"\n")).unwrap(),
        Tail::Garbage => file.write_all(b"{\"atMs\": 12, \"comm\n").unwrap(),
    }
}

fn new_show(root: &Path, quiz: &Quiz, started_ms: u64) -> Inner {
    let meta = SessionMeta {
        quiz_title: quiz.title.clone(),
        source_path: None,
        started_ms,
        media_root: None,
        rules_version: quiz_core::RULES_VERSION,
    };
    let session = Session::create(root, quiz, &meta).unwrap();
    Inner { game: Some(Game::new(quiz.clone())), session: Some(session), ..Inner::new() }
}

/// Plays one show from `seed` through the journal under `root`, crashing
/// now and then, and checks each resume.
fn drill(seed: u64, root: &Path) {
    let _ = fs::remove_dir_all(root);
    let mut rng = Rng::new(seed);
    let quiz = random_quiz(&mut rng);
    let buzzers = rng.between(1, u64::from(MAX_BUZZERS)) as u16;
    let mut sim = Show::new(quiz.clone(), buzzers);
    let mut app = new_show(root, &quiz, sim.now);
    for _ in 0..250 {
        sim.random_wait(&mut rng);
        let command = sim.random_command(&mut rng);
        let taken = apply_journaled(&mut app, command.clone(), sim.now);
        let expected = sim.apply(command);
        assert_eq!(taken.is_ok(), expected.result.is_ok(), "the app and the core disagree on {:?}: {taken:?}", expected.stamped);
        if let Some(due) = sim.game.due_command(sim.now) {
            apply_journaled(&mut app, due.clone(), sim.now).unwrap();
            sim.apply(due);
        }
        assert_eq!(app.game.as_ref(), Some(&sim.game), "the app's show is not the core's");

        if rng.chance(4) {
            let tail = *rng.pick(&[Tail::Clean, Tail::Torn, Tail::Zeros, Tail::Garbage]);
            leave_tail(root, tail, &Stamped { at_ms: sim.now + 1, command: Command::Next });
            drop(app);
            sim.crash_and_resume();
            let resumed = resume_show(root, sim.now, &[]).unwrap_or_else(|e| panic!("resume after a {tail:?} crash failed: {e:?}"));
            assert_eq!(resumed.game, sim.game, "after a {tail:?} crash the show resumed differently");
            assert_eq!(resumed.warnings, []);
            app = Inner { game: Some(resumed.game), session: Some(resumed.session), ..Inner::new() };
        } else if rng.chance(1) {
            // The quizmaster ends the show and starts another; only the latest resumes.
            let quiz = random_quiz(&mut rng);
            sim = Show::new(quiz.clone(), buzzers);
            sim.now = 1_000 + seed;
            app = new_show(root, &quiz, sim.now);
        }
    }
    // What is on disk replays into the show the windows last saw.
    let on_disk = Session::resume_latest(root).unwrap().unwrap();
    assert_eq!(Game::replay(on_disk.quiz, &on_disk.journal, quiz_core::RULES_VERSION), sim.game);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn shows_resume_after_crashes_exactly_as_they_were() {
    let env = |name: &str| std::env::var(name).ok().and_then(|value| value.parse::<u64>().ok());
    let root = std::env::temp_dir().join(format!("crash-drill-{}", std::process::id()));
    let seeds = match env("SOAK_SEED") {
        Some(seed) => seed..seed + 1,
        None => {
            let first = env("SOAK_FIRST").unwrap_or(0);
            first..first + env("SOAK_SHOWS").unwrap_or(40)
        }
    };
    for seed in seeds {
        if let Err(panic) = std::panic::catch_unwind(|| drill(seed, &root)) {
            eprintln!("crash drill failed; replay it with SOAK_SEED={seed}");
            std::panic::resume_unwind(panic);
        }
    }
}
