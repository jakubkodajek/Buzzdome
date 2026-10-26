//! The fixture library: the channel layouts of known fixture models, so a
//! fixture is added by picking its model and mode instead of typing its
//! channels from the manual (spec LT-9).
//!
//! The library is generated from the Open Fixture Library
//! (<https://open-fixture-library.org>, MIT licence, see
//! `data/OPEN-FIXTURE-LIBRARY-LICENSE`) by `examples/ofl-import.rs`, which
//! turns each mode into our own channels ([`ofl`]), and built into the app.
//! The app can bring it up to date from the Open Fixture Library's download
//! ([`archive`]), through the same conversion. Our format is stable; the
//! Open Fixture Library's own may change at any time, as its documentation
//! warns, so a download in another major version of it is refused.

pub mod archive;
pub mod ofl;

use crate::setup::Channel;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use ts_rs::TS;

/// Where the library comes from, as the Director credits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    /// The source's name and licence.
    pub name: String,
    pub copyright: String,
    pub url: String,
    /// The source's commit the library was made from; empty for a download.
    pub commit: String,
    /// When the source's data is from (its commit, or the download), in
    /// ms since the Unix epoch.
    #[ts(type = "number")]
    pub date_ms: u64,
    /// The source's licence, in full (MIT: it goes with every copy).
    pub licence: String,
}

/// The library file (`data/fixture-library.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub source: LibrarySource,
    pub fixtures: Vec<LibraryFixture>,
}

/// A fixture model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFixture {
    pub manufacturer: String,
    pub name: String,
    /// The model's key at the source, "manufacturer/model".
    pub key: String,
    pub modes: Vec<LibraryMode>,
}

/// One of a model's DMX modes (personalities): its channels in order, and
/// what each is called in the manual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMode {
    pub name: String,
    pub channels: Vec<Channel>,
    pub labels: Vec<String>,
    /// The labels of channels to check on the fixture before a show: by
    /// the source, every value of theirs strobes, runs a program or lights
    /// the fixture, also the one they rest at.
    pub check: Vec<String>,
}

/// The fixture models that match a search, and how many did in all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMatches {
    pub fixtures: Vec<LibraryFixture>,
    pub total: usize,
}

/// Most models a search returns.
pub const MAX_MATCHES: usize = 40;

/// The library built into the app, read the first time it is asked for.
pub fn built_in() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();
    LIBRARY.get_or_init(|| serde_json::from_str(include_str!("../data/fixture-library.json")).expect("the built-in fixture library reads"))
}

impl Library {
    /// The library as JSON, one model per line, so a new import's diff
    /// shows which models changed.
    pub fn to_json(&self) -> String {
        let source = serde_json::to_string(&self.source).expect("a library source is JSON");
        let models: Vec<String> = self.fixtures.iter().map(|fixture| serde_json::to_string(fixture).expect("a fixture model is JSON")).collect();
        format!("{{\"source\":{source},\"fixtures\":[\n{}\n]}}\n", models.join(",\n"))
    }

    /// How many modes its models have in all.
    pub fn modes(&self) -> usize {
        self.fixtures.iter().map(|fixture| fixture.modes.len()).sum()
    }
}

/// The models whose manufacturer, name or key hold every word of `query`,
/// ignoring case; the first `MAX_MATCHES` in the library's order.
pub fn search(library: &Library, query: &str) -> LibraryMatches {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let matching = library.fixtures.iter().filter(|fixture| {
        let text = format!("{} {} {}", fixture.manufacturer, fixture.name, fixture.key).to_lowercase();
        words.iter().all(|word| text.contains(word.as_str()))
    });
    let (mut fixtures, mut total) = (Vec::new(), 0);
    for fixture in matching {
        if fixtures.len() < MAX_MATCHES {
            fixtures.push(fixture.clone());
        }
        total += 1;
    }
    LibraryMatches { fixtures, total }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::{Fixture, LightsSetup, MAX_FIXTURE_CHANNELS};

    #[test]
    fn every_mode_in_the_built_in_library_makes_a_fixture_that_passes_its_checks() {
        let library = built_in();
        assert!(library.fixtures.len() > 500, "{} models", library.fixtures.len());
        for model in &library.fixtures {
            assert!(!model.modes.is_empty(), "{}", model.key);
            for mode in &model.modes {
                assert_eq!(mode.channels.len(), mode.labels.len(), "{} {}", model.key, mode.name);
                assert!(mode.check.iter().all(|label| mode.labels.contains(label)), "{} {}", model.key, mode.name);
                assert!(mode.channels.len() <= MAX_FIXTURE_CHANNELS, "{} {}", model.key, mode.name);
                let fixture = Fixture { id: 1, name: model.name.clone(), address: 1, channels: mode.channels.clone(), labels: mode.labels.clone() };
                let setup = LightsSetup { fixtures: vec![fixture], ..LightsSetup::default() };
                assert_eq!(setup.check(), Ok(()), "{} {}", model.key, mode.name);
            }
        }
    }

    #[test]
    fn a_library_reads_back_from_its_json() {
        let library = built_in();
        let json = library.to_json();
        assert_eq!(json.lines().count(), library.fixtures.len() + 2);
        assert_eq!(&serde_json::from_str::<Library>(&json).unwrap(), library);
        assert_eq!(json, include_str!("../data/fixture-library.json"));
    }

    #[test]
    fn the_library_carries_its_sources_licence_in_full() {
        assert_eq!(built_in().source.licence, include_str!("../data/OPEN-FIXTURE-LIBRARY-LICENSE"));
        assert!(built_in().source.licence.contains("Permission is hereby granted"));
    }

    #[test]
    fn a_search_needs_every_word_and_ignores_case() {
        let library = built_in();
        let found = search(library, "stairville LED");
        assert!(found.total > 0);
        assert!(found.fixtures.iter().all(|f| f.manufacturer == "Stairville" && f.name.to_lowercase().contains("led")));
        assert_eq!(search(library, "stairville nothing-like-this").total, 0);
        assert_eq!(search(library, "").fixtures.len(), MAX_MATCHES);
    }
}
