//! Makes the built-in fixture library (`data/fixture-library.json`) from a
//! checkout of the Open Fixture Library:
//!
//! ```text
//! git clone --depth 1 https://github.com/OpenLightingProject/open-fixture-library.git ofl
//! cargo run -p lights --example ofl-import -- ofl
//! ```
//!
//! Each definition goes through `lights::library::ofl::convert`; the
//! library is sorted by manufacturer and model, one model per line, and
//! records the commit it was made from and the source's licence, which is
//! also kept next to it (`data/OPEN-FIXTURE-LIBRARY-LICENSE`).

use lights::library::ofl::{self, Converted};
use lights::library::LibrarySource;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let checkout = PathBuf::from(std::env::args().nth(1).expect("usage: ofl-import <open-fixture-library checkout>"));
    let fixtures_dir = checkout.join("fixtures");
    let manufacturers: Value = read(&fixtures_dir.join("manufacturers.json"));
    let mut fixtures = Vec::new();
    let (mut redirects, mut lasers, mut without_modes) = (0, 0, 0);
    let mut makers: Vec<PathBuf> = dirs(&fixtures_dir);
    makers.sort();
    for maker in makers {
        let key = maker.file_name().unwrap().to_string_lossy().into_owned();
        let name = manufacturers.get(&key).and_then(|m| m.get("name")).and_then(Value::as_str).unwrap_or(&key).to_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&maker).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
        files.sort();
        for file in files {
            let model = file.file_stem().unwrap().to_string_lossy();
            let text = std::fs::read_to_string(&file).unwrap();
            match ofl::convert(&name, &format!("{key}/{model}"), &text).unwrap_or_else(|e| panic!("{}: {e}", file.display())) {
                Converted::Fixture(fixture) => fixtures.push(fixture),
                Converted::Redirect => redirects += 1,
                Converted::Laser => lasers += 1,
                Converted::NoModes => without_modes += 1,
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
        licence: std::fs::read_to_string(checkout.join("LICENSE")).expect("the checkout's LICENSE"),
    };
    let modes: usize = fixtures.iter().map(|f| f.modes.len()).sum();
    let to_check = fixtures.iter().filter(|f| f.modes.iter().any(|m| !m.check.is_empty())).count();
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    std::fs::write(data.join("OPEN-FIXTURE-LIBRARY-LICENSE"), &source.licence).unwrap();
    // The same JSON as one `Library` value, a model per line, so a new
    // import's diff shows which models changed.
    let lines: Vec<String> = fixtures.iter().map(|fixture| serde_json::to_string(fixture).unwrap()).collect();
    let json = format!("{{\"source\":{},\"fixtures\":[\n{}\n]}}\n", serde_json::to_string(&source).unwrap(), lines.join(",\n"));
    let out = data.join("fixture-library.json");
    std::fs::write(&out, json).unwrap();
    println!("{} models ({to_check} with channels to check), {modes} modes -> {}", fixtures.len(), out.display());
    println!("left out: {redirects} redirects, {lasers} lasers, {without_modes} without modes");
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn dirs(path: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(path).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.is_dir()).collect()
}
