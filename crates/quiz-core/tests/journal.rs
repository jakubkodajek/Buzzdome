//! The journal contract: every command a show journals reads back as it was
//! written, and lines written by earlier versions still read (A2).

use quiz_core::{Button, Command, Stamped};
use std::collections::BTreeSet;

/// Which variant `command` is. The match has no catch-all, so a new variant
/// does not compile until it is numbered here and given a sample below.
fn variant(command: &Command) -> usize {
    match command {
        Command::Press { .. } => 0,
        Command::Next => 1,
        Command::GoTo { .. } => 2,
        Command::GoToFinal => 3,
        Command::PauseClock => 4,
        Command::ResumeClock => 5,
        Command::CloseQuestion => 6,
        Command::JudgeRight => 7,
        Command::JudgeWrong => 8,
        Command::AddPlayer { .. } => 9,
        Command::RenamePlayer { .. } => 10,
        Command::SetTeams { .. } => 11,
        Command::PairTeam { .. } => 12,
        Command::SetPlayerEnabled { .. } => 13,
        Command::AdjustScore { .. } => 14,
        Command::ShowLeaderboard { .. } => 15,
        Command::PlayVideo => 16,
        Command::RestartQuestion => 17,
    }
}

const VARIANTS: usize = 18;

fn samples() -> Vec<Command> {
    vec![
        Command::Press { buzzer: 7, button: Button::Big },
        Command::Next,
        Command::GoTo { slide: 3 },
        Command::GoToFinal,
        Command::PauseClock,
        Command::ResumeClock,
        Command::CloseQuestion,
        Command::JudgeRight,
        Command::JudgeWrong,
        Command::AddPlayer { buzzer: 2, name: "Žluťoučký kůň \"A\"".into() },
        Command::RenamePlayer { buzzer: 2, name: "Hawks".into() },
        Command::SetTeams { names: vec!["Hawks".into(), "Šneci".into()] },
        Command::PairTeam { buzzer: 3, name: "Šneci".into() },
        Command::SetPlayerEnabled { buzzer: 3, enabled: false },
        Command::AdjustScore { buzzer: 3, delta: -25 },
        Command::ShowLeaderboard { visible: true },
        Command::PlayVideo,
        Command::RestartQuestion,
    ]
}

#[test]
fn every_command_reads_back_from_the_journal_as_it_was_written() {
    let samples = samples();
    assert_eq!(samples.iter().map(variant).collect::<BTreeSet<_>>(), (0..VARIANTS).collect(), "one sample of every variant");
    for (at_ms, command) in samples.into_iter().enumerate() {
        let stamped = Stamped { at_ms: at_ms as u64 * 1_000, command };
        let line = serde_json::to_string(&stamped).unwrap();
        assert!(!line.contains('\n'), "a journal line is one line: {line}");
        assert_eq!(serde_json::from_str::<Stamped>(&line).unwrap(), stamped, "{line}");
    }
}

#[test]
fn lines_journaled_by_earlier_versions_still_read() {
    let lines = [
        (r#"{"atMs":5,"command":{"type":"next"}}"#, Command::Next),
        (r#"{"atMs":6,"command":{"type":"press","buzzer":1,"button":"a"}}"#, Command::Press { buzzer: 1, button: Button::A }),
        (r#"{"atMs":7,"command":{"type":"goTo","slide":2}}"#, Command::GoTo { slide: 2 }),
        (r#"{"atMs":8,"command":{"type":"setPlayerEnabled","buzzer":4,"enabled":true}}"#, Command::SetPlayerEnabled { buzzer: 4, enabled: true }),
    ];
    for (line, command) in lines {
        let at_ms = serde_json::from_str::<serde_json::Value>(line).unwrap()["atMs"].as_u64().unwrap();
        assert_eq!(serde_json::from_str::<Stamped>(line).unwrap(), Stamped { at_ms, command }, "{line}");
    }
}
