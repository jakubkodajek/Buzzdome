//! The journal contract: every command a show journals reads back as it was
//! written, and lines written by earlier versions still read.

use quiz_core::{Button, Command, Stamped};
use std::collections::BTreeSet;
use ts_rs::{Config, TS};

/// Journal lines exactly as the app writes them, with the time and command
/// each holds: one per command variant, and one per button a press can
/// name. Saved journals hold these lines, so a line is never changed once
/// shipped: a renamed field, tag or value would break every journal. A new
/// variant or value adds its line here.
fn lines() -> Vec<(&'static str, u64, Command)> {
    vec![
        (r#"{"atMs":1,"command":{"type":"press","buzzer":7,"button":"big"}}"#, 1, Command::Press { buzzer: 7, button: Button::Big }),
        (r#"{"atMs":2,"command":{"type":"next"}}"#, 2, Command::Next),
        (r#"{"atMs":3,"command":{"type":"goTo","slide":3}}"#, 3, Command::GoTo { slide: 3 }),
        (r#"{"atMs":4,"command":{"type":"goToFinal"}}"#, 4, Command::GoToFinal),
        (r#"{"atMs":5,"command":{"type":"pauseClock"}}"#, 5, Command::PauseClock),
        (r#"{"atMs":6,"command":{"type":"resumeClock"}}"#, 6, Command::ResumeClock),
        (r#"{"atMs":7,"command":{"type":"closeQuestion"}}"#, 7, Command::CloseQuestion),
        (r#"{"atMs":8,"command":{"type":"restartQuestion"}}"#, 8, Command::RestartQuestion),
        (r#"{"atMs":9,"command":{"type":"judgeRight"}}"#, 9, Command::JudgeRight),
        (r#"{"atMs":10,"command":{"type":"judgeWrong"}}"#, 10, Command::JudgeWrong),
        (
            r#"{"atMs":11,"command":{"type":"addPlayer","buzzer":2,"name":"Žluťoučký kůň \"A\""}}"#,
            11,
            Command::AddPlayer { buzzer: 2, name: "Žluťoučký kůň \"A\"".into() },
        ),
        (r#"{"atMs":12,"command":{"type":"renamePlayer","buzzer":2,"name":"Hawks"}}"#, 12, Command::RenamePlayer { buzzer: 2, name: "Hawks".into() }),
        (r#"{"atMs":13,"command":{"type":"setTeams","names":["Hawks","Šneci"]}}"#, 13, Command::SetTeams { names: vec!["Hawks".into(), "Šneci".into()] }),
        (r#"{"atMs":14,"command":{"type":"pairTeam","buzzer":3,"name":"Šneci"}}"#, 14, Command::PairTeam { buzzer: 3, name: "Šneci".into() }),
        (r#"{"atMs":15,"command":{"type":"setPlayerEnabled","buzzer":3,"enabled":false}}"#, 15, Command::SetPlayerEnabled { buzzer: 3, enabled: false }),
        (r#"{"atMs":16,"command":{"type":"adjustScore","buzzer":3,"delta":-25}}"#, 16, Command::AdjustScore { buzzer: 3, delta: -25 }),
        (r#"{"atMs":17,"command":{"type":"showLeaderboard","visible":true}}"#, 17, Command::ShowLeaderboard { visible: true }),
        (r#"{"atMs":18,"command":{"type":"playVideo"}}"#, 18, Command::PlayVideo),
        (r#"{"atMs":19,"command":{"type":"press","buzzer":1,"button":"a"}}"#, 19, Command::Press { buzzer: 1, button: Button::A }),
        (r#"{"atMs":20,"command":{"type":"press","buzzer":1,"button":"b"}}"#, 20, Command::Press { buzzer: 1, button: Button::B }),
        (r#"{"atMs":21,"command":{"type":"press","buzzer":1,"button":"c"}}"#, 21, Command::Press { buzzer: 1, button: Button::C }),
        (r#"{"atMs":22,"command":{"type":"press","buzzer":1,"button":"d"}}"#, 22, Command::Press { buzzer: 1, button: Button::D }),
        (r#"{"atMs":23,"command":{"type":"setJoker","buzzer":2,"on":true}}"#, 23, Command::SetJoker { buzzer: 2, on: true }),
    ]
}

/// The `type` tag of every variant, read from the type the bindings are
/// generated from, so a variant added to `Command` is known here at once.
fn variant_tags() -> BTreeSet<String> {
    let declared = <Command as TS>::inline(&Config::new());
    declared.split(r#""type": ""#).skip(1).map(|rest| rest.split('"').next().unwrap().to_owned()).collect()
}

/// The values of `Button`, read from the type the bindings are generated
/// from, so a button added is known here at once.
fn button_values() -> BTreeSet<String> {
    let declared = <Button as TS>::inline(&Config::new());
    declared.split('"').skip(1).step_by(2).map(str::to_owned).collect()
}

#[test]
fn every_variant_and_button_has_its_journal_line() {
    let commands: Vec<serde_json::Value> = lines().iter().map(|(line, ..)| serde_json::from_str::<serde_json::Value>(line).unwrap()["command"].clone()).collect();
    let tags: BTreeSet<String> = commands.iter().map(|command| command["type"].as_str().unwrap().to_owned()).collect();
    assert_eq!(tags, variant_tags());
    let buttons: BTreeSet<String> = commands.iter().filter_map(|command| Some(command["button"].as_str()?.to_owned())).collect();
    assert_eq!(buttons, button_values());
    assert_eq!(buttons.len(), 5, "{buttons:?}");
}

#[test]
fn every_journal_line_reads_as_its_command_and_is_written_as_it_was() {
    for (line, at_ms, command) in lines() {
        let stamped = Stamped { at_ms, command };
        assert_eq!(serde_json::from_str::<Stamped>(line).unwrap(), stamped, "{line}");
        assert_eq!(serde_json::to_string(&stamped).unwrap(), line);
    }
}
