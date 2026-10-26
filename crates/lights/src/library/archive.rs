//! A whole Open Fixture Library made into our library: a checkout's
//! `fixtures` folder (`examples/ofl-import.rs`), or the zip of every
//! fixture its website gives (<https://open-fixture-library.org/download.ofl>,
//! made by its "ofl" export plugin, `plugins/ofl/export.js` there: each
//! definition as `<manufacturer>/<model>.json`, and `manufacturers.json`),
//! which the app downloads to bring its library up to date (spec LT-9).

use super::ofl::{self, Converted};
use super::{Library, LibraryFixture, LibrarySource};
use serde_json::Value;
use std::io::{Cursor, Read};

/// The major version of the Open Fixture Library's format that `ofl`
/// follows (its `schemas/fixture.json` was 12.6.0 at the built-in
/// library's commit). A download in another is refused: its format may
/// have changed in ways the conversion does not know.
pub const SCHEMA_MAJOR: &str = "12";
/// Most bytes one definition in a zip may unpack to.
const MAX_DEFINITION_BYTES: u64 = 4 << 20;
/// Most bytes a whole zip may unpack to.
const MAX_ARCHIVE_BYTES: u64 = 256 << 20;

/// What became of the definitions besides the models taken.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImportCounts {
    pub redirects: usize,
    pub lasers: usize,
    pub without_modes: usize,
    /// Definitions that are not JSON.
    pub unreadable: usize,
}

/// Definitions converted one by one into a library.
pub struct Import {
    manufacturers: Value,
    fixtures: Vec<LibraryFixture>,
    pub counts: ImportCounts,
}

impl Import {
    /// An import with the manufacturers' names from `manufacturers`
    /// (`manufacturers.json`).
    pub fn new(manufacturers: &str) -> serde_json::Result<Import> {
        Ok(Import { manufacturers: serde_json::from_str(manufacturers)?, fixtures: Vec::new(), counts: ImportCounts::default() })
    }

    /// Adds `text`, the definition of model `model` by manufacturer `maker`
    /// (their keys).
    pub fn add(&mut self, maker: &str, model: &str, text: &str) {
        let name = self.manufacturers.get(maker).and_then(|m| m.get("name")).and_then(Value::as_str).unwrap_or(maker).to_owned();
        match ofl::convert(&name, &format!("{maker}/{model}"), text) {
            Ok(Converted::Fixture(fixture)) => self.fixtures.push(fixture),
            Ok(Converted::Redirect) => self.counts.redirects += 1,
            Ok(Converted::Laser) => self.counts.lasers += 1,
            Ok(Converted::NoModes) => self.counts.without_modes += 1,
            Err(_) => self.counts.unreadable += 1,
        }
    }

    /// The library, sorted by manufacturer and model.
    pub fn finish(mut self, source: LibrarySource) -> Library {
        self.fixtures.sort_by_cached_key(|f| (f.manufacturer.to_lowercase(), f.name.to_lowercase(), f.key.clone()));
        Library { source, fixtures: self.fixtures }
    }
}

/// Why a zip of the Open Fixture Library was not made into a library.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ArchiveError {
    #[error("not a zip of fixture definitions: {0}")]
    NotRead(String),
    /// A definition says it is in another major version of the format.
    #[error("the fixture definitions are in format {version}")]
    FormatChanged { version: String },
}

/// The library in `zip` (bytes of the Open Fixture Library's download),
/// credited to `source`.
pub fn from_zip(zip: &[u8], source: LibrarySource) -> Result<(Library, ImportCounts), ArchiveError> {
    let not_read = |e: &dyn std::fmt::Display| ArchiveError::NotRead(e.to_string());
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| not_read(&e))?;
    let mut unpacked = 0;
    let mut read = |archive: &mut zip::ZipArchive<Cursor<&[u8]>>, index: usize| -> Result<(String, String), ArchiveError> {
        let entry = archive.by_index(index).map_err(|e| not_read(&e))?;
        let name = entry.name().map_err(|e| not_read(&e))?.into_owned();
        let mut text = String::new();
        entry.take(MAX_DEFINITION_BYTES).read_to_string(&mut text).map_err(|e| not_read(&format!("{name}: {e}")))?;
        unpacked += text.len() as u64;
        if unpacked > MAX_ARCHIVE_BYTES {
            return Err(not_read(&"it unpacks to too much"));
        }
        Ok((name, text))
    };
    let manufacturers = archive.index_for_name("manufacturers.json").ok_or_else(|| not_read(&"no manufacturers.json"))?;
    let (_, manufacturers) = read(&mut archive, manufacturers)?;
    let mut import = Import::new(&manufacturers).map_err(|e| not_read(&format!("manufacturers.json: {e}")))?;
    for index in 0..archive.len() {
        let (name, text) = read(&mut archive, index)?;
        let Some((maker, model)) = name.strip_suffix(".json").and_then(|path| path.split_once('/')) else { continue };
        if model.contains('/') {
            continue;
        }
        if let Some(version) = schema_version(&text).filter(|version| version.split('.').next() != Some(SCHEMA_MAJOR)) {
            return Err(ArchiveError::FormatChanged { version });
        }
        import.add(maker, model, &text);
    }
    let counts = import.counts;
    Ok((import.finish(source), counts))
}

/// The format version a definition names in its `$schema` link
/// (".../schema-12.6.0/schemas/fixture.json"); none when it names none.
fn schema_version(text: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Schema {
        #[serde(rename = "$schema")]
        schema: Option<String>,
    }
    let schema = serde_json::from_str::<Schema>(text).ok()?.schema?;
    let (_, after) = schema.split_once("/schema-")?;
    Some(after.split('/').next()?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            out.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            out.write_all(text.as_bytes()).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    fn source() -> LibrarySource {
        LibrarySource { name: "Test".into(), copyright: String::new(), url: String::new(), commit: String::new(), date_ms: 1, licence: String::new() }
    }

    const SCHEMA: &str = "https://raw.githubusercontent.com/OpenLightingProject/open-fixture-library/schema-12.6.0/schemas/fixture.json";

    fn definition(name: &str, categories: &str) -> String {
        format!(
            r#"{{ "$schema": "{SCHEMA}", "name": "{name}", "categories": [{categories}],
                "availableChannels": {{ "Dimmer": {{ "capability": {{ "type": "Intensity" }} }} }},
                "modes": [{{ "name": "1-channel", "channels": ["Dimmer"] }}] }}"#
        )
    }

    #[test]
    fn a_download_becomes_a_sorted_library_without_lasers() {
        let manufacturers = r#"{ "$schema": "x", "acme": { "name": "Acme" }, "zeta": { "name": "Zeta" } }"#;
        let bytes = zip(&[
            ("zeta/par.json", &definition("Par", r#""Color Changer""#)),
            ("acme/wash.json", &definition("Wash", r#""Color Changer""#)),
            ("acme/beam.json", &definition("Beam", r#""Laser""#)),
            ("acme/broken.json", "{ not json"),
            ("manufacturers.json", manufacturers),
        ]);
        let (library, counts) = from_zip(&bytes, source()).unwrap();
        let names: Vec<_> = library.fixtures.iter().map(|f| format!("{} {}", f.manufacturer, f.name)).collect();
        assert_eq!(names, ["Acme Wash", "Zeta Par"]);
        assert_eq!(counts, ImportCounts { lasers: 1, unreadable: 1, ..ImportCounts::default() });
    }

    #[test]
    fn a_download_in_another_major_version_of_the_format_is_refused() {
        let newer = definition("Par", "").replace("schema-12.6.0", "schema-13.0.0");
        let bytes = zip(&[("manufacturers.json", "{}"), ("acme/par.json", &newer)]);
        assert_eq!(from_zip(&bytes, source()).map(|_| ()), Err(ArchiveError::FormatChanged { version: "13.0.0".into() }));
        assert!(matches!(from_zip(b"not a zip", source()), Err(ArchiveError::NotRead(_))));
        assert!(matches!(from_zip(&zip(&[("acme/par.json", &definition("Par", ""))]), source()), Err(ArchiveError::NotRead(_))));
    }
}
