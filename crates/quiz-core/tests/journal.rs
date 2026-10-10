//! The journal contract: every command a show journals reads back as it was
//! written, and lines written by earlier versions still read.

use quiz_core::{Button, Command, Stamped};
use std::collections::BTreeSet;
use ts_rs::{Config, TS};

/// One journal line per command variant, exactly as the app writes it, with
/// the command it holds. Saved journals hold these lines, so a line is never
/// changed once shipped: a renamed field or tag would break every journal.
/// A new variant adds its line here.
fn lines() -> Vec<(&'static str, Command)> {
    vec![
        (r#"{"atMs":1,"command":{"type":"press","buzzer":7,"button":"big"}}"#, Command::Press { buzzer: 7, button: Button::Big }),
        (r#"{"atMs":2,"command":{"type":"next"}}"#, Command::Next),
        (r#"{"atMs":3,"command":{"type":"goTo","slide":3}}"#, Command::GoTo { slide: 3 }),
        (r#"{"atMs":4,"command":{"type":"goToFinal"}}"#, Command::GoToFinal),
        (r#"{"atMs":5,"command":{"type":"pauseClock"}}"#, Command::PauseClock),
        (r#"{"atMs":6,"command":{"type":"resumeClock"}}"#, Command::ResumeClock),
        (r#"{"atMs":7,"command":{"type":"closeQuestion"}}"#, Command::CloseQuestion),
        (r#"{"atMs":8,"command":{"type":"restartQuestion"}}"#, Command::RestartQuestion),
        (r#"{"atMs":9,"command":{"type":"judgeRight"}}"#, Command::JudgeRight),
        (r#"{"atMs":10,"command":{"type":"judgeWrong"}}"#, Command::JudgeWrong),
        (
            r#"{"atMs":11,"command":{"type":"addPlayer","buzzer":2,"name":"Žluťoučký kůň \"A\""}}"#,
            Command::AddPlayer { buzzer: 2, name: "Žluťoučký kůň \"A\"".into() },
        ),
        (r#"{"atMs":12,"command":{"type":"renamePlayer","buzzer":2,"name":"Hawks"}}"#, Command::RenamePlayer { buzzer: 2, name: "Hawks".into() }),
        (r#"{"atMs":13,"command":{"type":"setTeams","names":["Hawks","Šneci"]}}"#, Command::SetTeams { names: vec!["Hawks".into(), "Šneci".into()] }),
        (r#"{"atMs":14,"command":{"type":"pairTeam","buzzer":3,"name":"Šneci"}}"#, Command::PairTeam { buzzer: 3, name: "Šneci".into() }),
        (r#"{"atMs":15,"command":{"type":"setPlayerEnabled","buzzer":3,"enabled":false}}"#, Command::SetPlayerEnabled { buzzer: 3, enabled: false }),
        (r#"{"atMs":16,"command":{"type":"adjustScore","buzzer":3,"delta":-25}}"#, Command::AdjustScore { buzzer: 3, delta: -25 }),
        (r#"{"atMs":17,"command":{"type":"showLeaderboard","visible":true}}"#, Command::ShowLeaderboard { visible: true }),
        (r#"{"atMs":18,"command":{"type":"playVideo"}}"#, Command::PlayVideo),
    ]
}

/// The `type` tag of every variant, read from the type the bindings are
/// generated from, so a variant added to `Command` is known here at once.
fn variant_tags() -> BTreeSet<String> {
    let declared = <Command as TS>::inline(&Config::new());
    declared.split(r#""type": ""#).skip(1).map(|rest| rest.split('"').next().unwrap().to_owned()).collect()
}

#[test]
fn every_variant_has_its_journal_line() {
    let tags: Vec<String> = lines()
        .iter()
        .map(|(line, _)| serde_json::from_str::<serde_json::Value>(line).unwrap()["command"]["type"].as_str().unwrap().to_owned())
        .collect();
    let distinct: BTreeSet<String> = tags.iter().cloned().collect();
    assert_eq!(tags.len(), distinct.len(), "one line per variant: {tags:?}");
    assert_eq!(distinct, variant_tags());
}

#[test]
fn every_journal_line_reads_as_its_command_and_is_written_as_it_was() {
    for (at_ms, (line, command)) in (1..).zip(lines()) {
        let stamped = Stamped { at_ms, command };
        assert_eq!(serde_json::from_str::<Stamped>(line).unwrap(), stamped, "{line}");
        assert_eq!(serde_json::to_string(&stamped).unwrap(), line);
    }
}
