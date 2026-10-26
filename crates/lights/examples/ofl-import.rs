//! Makes the built-in fixture library (`data/fixture-library.json`) from a
//! checkout of the Open Fixture Library:
//!
//! ```text
//! git clone --depth 1 https://github.com/OpenLightingProject/open-fixture-library.git ofl
//! cargo run -p lights --example ofl-import -- ofl
//! ```
//!
//! Each definition goes through `lights::library::ofl::convert`; the
//! library is sorted by manufacturer and model, and records the commit it
//! was made from. The source's licence is kept next to it
//! (`data/OPEN-FIXTURE-LIBRARY-LICENSE`).

use lights::library::{ofl, Library, LibrarySource};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let checkout = PathBuf::from(std::env::args().nth(1).expect("usage: ofl-import <open-fixture-library checkout>"));
    let fixtures_dir = checkout.join("fixtures");
    let manufacturers: Value = read(&fixtures_dir.join("manufacturers.json"));
    let mut fixtures = Vec::new();
    let mut left_out = 0;
    let mut makers: Vec<PathBuf> = dirs(&fixtures_dir);
    makers.sort();
    for maker in makers {
        let key = maker.file_name().unwrap().to_string_lossy().into_owned();
        let name = manufacturers.get(&key).and_then(|m| m.get("name")).and_then(Value::as_str).unwrap_or(&key).to_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&maker).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
        files.sort();
        for file in files {
            let model = file.file_stem().unwrap().to_string_lossy();
            match ofl::convert(&name, &format!("{key}/{model}"), &read(&file)) {
                Some(fixture) => fixtures.push(fixture),
                None => left_out += 1,
            }
        }
    }
    fixtures.sort_by_cached_key(|f| (f.manufacturer.to_lowercase(), f.name.to_lowercase()));
    let commit = Command::new("git").arg("-C").arg(&checkout).args(["rev-parse", "HEAD"]).output().expect("git");
    let source = LibrarySource {
        name: "Open Fixture Library (MIT)".into(),
        copyright: "Copyright (c) 2017 Florian & Felix Edelmann".into(),
        url: "https://open-fixture-library.org".into(),
        commit: String::from_utf8_lossy(&commit.stdout).trim().to_owned(),
    };
    let modes: usize = fixtures.iter().map(|f| f.modes.len()).sum();
    let count = fixtures.len();
    let library = Library { source, fixtures };
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/fixture-library.json");
    std::fs::write(&out, serde_json::to_string(&library).unwrap() + "\n").unwrap();
    println!("{count} models, {modes} modes, {left_out} redirects or without modes left out -> {}", out.display());
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dirs(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.is_dir()).collect()
}
