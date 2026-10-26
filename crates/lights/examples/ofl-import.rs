//! Makes the built-in fixture library (`data/fixture-library.json`) from a
//! checkout of the Open Fixture Library:
//!
//! ```text
//! git clone --depth 1 https://github.com/OpenLightingProject/open-fixture-library.git ofl
//! cargo run -p lights --example ofl-import -- ofl
//! ```
//!
//! Each definition goes through `lights::library::archive::Import`, as a
//! download in the app does; the library records the commit it was made
//! from, its date and the source's licence, which is also kept next to it
//! (`data/OPEN-FIXTURE-LIBRARY-LICENSE`).

use lights::library::archive::Import;
use lights::library::LibrarySource;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let checkout = PathBuf::from(std::env::args().nth(1).expect("usage: ofl-import <open-fixture-library checkout>"));
    let fixtures_dir = checkout.join("fixtures");
    let mut import = Import::new(&read(&fixtures_dir.join("manufacturers.json"))).expect("manufacturers.json reads");
    let mut makers: Vec<PathBuf> = std::fs::read_dir(&fixtures_dir).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.is_dir()).collect();
    makers.sort();
    for maker in makers {
        let key = maker.file_name().unwrap().to_string_lossy().into_owned();
        let mut files: Vec<PathBuf> = std::fs::read_dir(&maker).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
        files.sort();
        for file in files {
            import.add(&key, &file.file_stem().unwrap().to_string_lossy(), &read(&file));
        }
    }
    let counts = import.counts;
    assert_eq!(counts.unreadable, 0, "every definition of a checkout reads");
    let git = |args: &[&str]| String::from_utf8_lossy(&Command::new("git").arg("-C").arg(&checkout).args(args).output().expect("git").stdout).trim().to_owned();
    let source = LibrarySource {
        name: "Open Fixture Library (MIT)".into(),
        copyright: "Copyright (c) 2017 Florian & Felix Edelmann".into(),
        url: "https://open-fixture-library.org".into(),
        commit: git(&["rev-parse", "HEAD"]),
        date_ms: git(&["log", "-1", "--format=%ct"]).parse::<u64>().expect("the commit's time") * 1000,
        licence: read(&checkout.join("LICENSE")),
    };
    let library = import.finish(source);
    let to_check = library.fixtures.iter().filter(|f| f.modes.iter().any(|m| !m.check.is_empty())).count();
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
    std::fs::write(data.join("OPEN-FIXTURE-LIBRARY-LICENSE"), &library.source.licence).unwrap();
    let out = data.join("fixture-library.json");
    std::fs::write(&out, library.to_json()).unwrap();
    println!("{} models ({to_check} with channels to check), {} modes -> {}", library.fixtures.len(), library.modes(), out.display());
    println!("left out: {} redirects, {} lasers, {} without modes", counts.redirects, counts.lasers, counts.without_modes);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
