//! Themes: the look of the stage (spec 5.2). A theme package's `theme.json`
//! lists only what differs from its parent; `resolve_themes` fills in the
//! rest from the parents, so the UI gets every value of every theme.

use crate::media::{extension, is_portable_path, MediaKind};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use ts_rs::TS;

/// The `format` of `theme.json` this app reads (TH-1).
pub const THEME_FORMAT: u32 = 1;
/// The theme a quiz without one uses. Every theme ends at it, so it sets
/// every value (TH-2).
pub const DEFAULT_THEME_ID: &str = "builtin.classic";
/// The ids of the app's own themes start with this; no other theme may take one.
pub const BUILTIN_PREFIX: &str = "builtin.";
/// Longest theme id, in bytes.
pub const MAX_THEME_ID_LEN: usize = 100;
/// Longest theme name or author, in characters.
const MAX_LABEL_CHARS: usize = 100;
/// The value that takes a key from the parent theme (TH-2).
const INHERIT: &str = "default";
/// Font files a theme can bring.
pub const FONT_EXTENSIONS: &[&str] = &["woff2", "woff", "ttf", "otf"];

/// Defines the keys of a section of `theme.json` once: an enum of the keys,
/// and the section with every value filled in, one field per key.
macro_rules! theme_section {
    (
        $(#[$section_doc:meta])* $section:ident<$value:ty>,
        $(#[$key_doc:meta])* $key:ident {
            $($(#[$field_doc:meta])* $variant:ident $field:ident $name:literal,)+
        }
    ) => {
        $(#[$key_doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, TS)]
        #[ts(export)]
        pub enum $key {
            $(#[serde(rename = $name)] $variant,)+
        }

        impl $key {
            pub const ALL: &'static [$key] = &[$($key::$variant,)+];

            /// The key as `theme.json` writes it.
            pub fn name(self) -> &'static str {
                match self {
                    $($key::$variant => $name,)+
                }
            }

            fn parse(name: &str) -> Option<$key> {
                $key::ALL.iter().copied().find(|key| key.name() == name)
            }
        }

        $(#[$section_doc])*
        #[derive(Debug, Clone, PartialEq, Serialize, TS)]
        #[ts(export)]
        pub struct $section {
            $($(#[$field_doc])* #[serde(rename = $name)] pub $field: $value,)+
        }

        impl $section {
            /// The section from a value per key; the first key without one when any is missing.
            fn try_from_fn(mut value: impl FnMut($key) -> Option<$value>) -> Result<$section, $key> {
                Ok($section { $($field: value($key::$variant).ok_or($key::$variant)?,)+ })
            }
        }
    };
}

theme_section! {
    /// Colours as `#rrggbb` or `#rrggbbaa` (TH-4).
    ThemeColors<String>,
    ColorKey {
        /// Text on the background.
        Text text "text",
        /// Shadow under text on the background, to keep it readable.
        Shadow shadow "shadow",
        /// Glow around slide titles.
        Glow glow "glow",
        /// Boxes placed on the background.
        Panel panel "panel",
        /// The bars across the top and bottom of a screen.
        Bar bar "bar",
        /// The bright line along a bar's edge.
        BarEdge bar_edge "barEdge",
        /// Leaderboard rows.
        Tile tile "tile",
        /// Scores and counters.
        Accent accent "accent",
        /// Answers A-D, the buzzer's answer buttons.
        AnswerA answer_a "answerA",
        AnswerB answer_b "answerB",
        AnswerC answer_c "answerC",
        AnswerD answer_d "answerD",
        /// Answers E and F, read out in fastest finger questions.
        AnswerE answer_e "answerE",
        AnswerF answer_f "answerF",
        Correct correct "correct",
        Wrong wrong "wrong",
    }
}

theme_section! {
    /// The two fonts of the stage.
    ThemeFonts<ThemeFile>,
    FontKey {
        /// Titles, names and big numbers.
        Heading heading "heading",
        /// Questions, answers and other text.
        Body body "body",
    }
}

theme_section! {
    /// A background for every screen (TH-5).
    ThemeBackgrounds<ThemeFile>,
    /// The screens a theme can give its own background (TH-5).
    ScreenKey {
        /// Every screen without its own; billboards.
        Default default "default",
        /// Team registration before the first slide.
        Welcome welcome "welcome",
        /// A question being asked.
        Question question "question",
        /// A question once its answer is shown, and explanation slides.
        Answer answer "answer",
        /// The leaderboard shown over a slide.
        Leaderboard leaderboard "leaderboard",
        /// End-of-round slides.
        RoundEnd round_end "roundEnd",
        /// Final results.
        Final r#final "final",
    }
}

theme_section! {
    /// Light colours of RGB buzzers.
    BuzzerColors<String>,
    BuzzerColorKey {
        /// The buzzer may be pressed.
        Ready ready "ready",
        /// The buzzer pressed first.
        First first "first",
    }
}

impl ScreenKey {
    /// The screen whose background this one takes when the theme has none
    /// for it (TH-5); none for `Default`, where every path ends.
    pub fn fallback(self) -> Option<ScreenKey> {
        match self {
            ScreenKey::Default => None,
            ScreenKey::Welcome | ScreenKey::Question | ScreenKey::Leaderboard | ScreenKey::Final => Some(ScreenKey::Default),
            ScreenKey::Answer => Some(ScreenKey::Question),
            ScreenKey::RoundEnd => Some(ScreenKey::Leaderboard),
        }
    }

    /// This screen and the screens it falls back to, ending with `Default`.
    fn fallback_path(self) -> impl Iterator<Item = ScreenKey> {
        std::iter::successors(Some(self), |screen| screen.fallback())
    }
}

/// Text in each language of the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LocalizedText {
    pub cs: String,
    pub en: String,
}

/// A file of a theme package.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct ThemeFile {
    /// The package's folder, ending with a path separator.
    pub root: String,
    /// The file's path inside the package, with `/` between folders (TH-3).
    pub path: String,
}

/// A theme with every value filled in, ready to draw with.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub id: String,
    pub name: LocalizedText,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
    pub backgrounds: ThemeBackgrounds,
    pub buzzer_colors: BuzzerColors,
}

/// A checked `theme.json` (spec 5.2). Keys set to `"default"` are left out,
/// so a value missing here comes from the parent (TH-2).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeManifest {
    pub id: String,
    /// Major, minor and patch: a higher version of an installed theme updates it (TH-1).
    pub version: [u32; 3],
    pub name: LocalizedText,
    pub author: Option<String>,
    /// The theme to take missing values from; the default theme when none.
    pub parent: Option<String>,
    pub colors: BTreeMap<ColorKey, String>,
    /// Font file paths.
    pub fonts: BTreeMap<FontKey, String>,
    /// Picture or video file paths.
    pub backgrounds: BTreeMap<ScreenKey, String>,
    pub buzzer_colors: BTreeMap<BuzzerColorKey, String>,
}

/// A key the app does not know, ignored so that newer themes still load (TH-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKey {
    /// The section of `theme.json` it is in, such as `backgrounds`.
    pub section: &'static str,
    pub key: String,
}

/// Why a theme cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ThemeError {
    #[error("theme.json is not valid: {0}")]
    Json(String),
    #[error("theme format {0} is not known (this app reads format {THEME_FORMAT})")]
    UnknownFormat(u32),
    #[error("{field}: {value:?} is not valid")]
    BadValue { field: String, value: String },
    #[error("theme {0} is listed twice")]
    DuplicateId(String),
    #[error("theme id {0} is kept for the app's own themes")]
    ReservedId(String),
    #[error("the default theme {DEFAULT_THEME_ID} is missing")]
    NoDefault,
    #[error("the default theme sets no {0}")]
    Incomplete(String),
}

/// The fields of `theme.json` as written, before they are checked.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawManifest {
    id: String,
    version: String,
    name: LocalizedText,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    fonts: BTreeMap<String, String>,
    #[serde(default)]
    backgrounds: BTreeMap<String, String>,
    #[serde(default)]
    buzzer_colors: BTreeMap<String, String>,
}

impl ThemeManifest {
    /// Reads and checks `theme.json`: a known format (TH-1), an id in reverse
    /// domain style, colours as `#rrggbb` or `#rrggbbaa` (TH-4), and file
    /// paths that stay inside the package and have a usable type (TH-3).
    /// Keys the app does not know are returned, not refused (TH-5).
    pub fn from_json(json: &str) -> Result<(ThemeManifest, Vec<UnknownKey>), ThemeError> {
        #[derive(Deserialize)]
        struct Format {
            format: u32,
        }
        let json_error = |e: serde_json::Error| ThemeError::Json(e.to_string());
        // The format comes first: a newer format may not have the fields below.
        let Format { format } = serde_json::from_str(json).map_err(json_error)?;
        if format != THEME_FORMAT {
            return Err(ThemeError::UnknownFormat(format));
        }
        let raw: RawManifest = serde_json::from_str(json).map_err(json_error)?;
        let mut unknown = Vec::new();
        let manifest = ThemeManifest {
            version: parse_version(&raw.version).ok_or_else(|| bad("version", &raw.version))?,
            name: LocalizedText { cs: label("name.cs", raw.name.cs)?, en: label("name.en", raw.name.en)? },
            author: raw.author.map(|author| label("author", author)).transpose()?,
            parent: raw.parent.map(|parent| theme_id("parent", parent)).transpose()?,
            colors: section("colors", raw.colors, ColorKey::parse, is_hex_color, &mut unknown)?,
            fonts: section("fonts", raw.fonts, FontKey::parse, |path| check_file(path, &[FONT_EXTENSIONS]), &mut unknown)?,
            backgrounds: section(
                "backgrounds",
                raw.backgrounds,
                ScreenKey::parse,
                |path| check_file(path, &[MediaKind::Image.extensions(), MediaKind::Video.extensions()]),
                &mut unknown,
            )?,
            buzzer_colors: section("buzzerColors", raw.buzzer_colors, BuzzerColorKey::parse, is_hex_color, &mut unknown)?,
            id: theme_id("id", raw.id)?,
        };
        Ok((manifest, unknown))
    }
}

fn bad(field: &str, value: &str) -> ThemeError {
    ThemeError::BadValue { field: field.into(), value: value.into() }
}

/// True for an id in reverse domain style, such as `cz.example.autumn`: at
/// least two parts of lower-case letters, digits, `-` and `_`, joined by
/// dots. Such an id is also a portable folder name (TH-6).
pub fn is_theme_id(id: &str) -> bool {
    let part = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    id.len() <= MAX_THEME_ID_LEN && id.split('.').count() >= 2 && id.split('.').all(part)
}

fn theme_id(field: &str, id: String) -> Result<String, ThemeError> {
    if is_theme_id(&id) {
        Ok(id)
    } else {
        Err(bad(field, &id))
    }
}

/// `major.minor.patch`.
fn parse_version(version: &str) -> Option<[u32; 3]> {
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    let parsed = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(parsed)
}

/// A name or author: not blank, and short enough for the gallery.
fn label(field: &str, text: String) -> Result<String, ThemeError> {
    if text.trim().is_empty() || text.chars().count() > MAX_LABEL_CHARS {
        return Err(bad(field, &text));
    }
    Ok(text)
}

fn is_hex_color(color: &str) -> bool {
    color.strip_prefix('#').is_some_and(|hex| matches!(hex.len(), 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// A file path inside the package (TH-3) with one of the extensions in `types`.
fn check_file(path: &str, types: &[&[&str]]) -> bool {
    let extension = extension(path);
    is_portable_path(path) && types.iter().any(|extensions| extensions.contains(&extension.as_str()))
}

/// The keys of one section that the app knows, with their values checked.
/// `"default"` leaves a key out (TH-2); unknown keys go to `unknown`.
fn section<K: Ord>(
    name: &'static str,
    raw: BTreeMap<String, String>,
    parse: fn(&str) -> Option<K>,
    valid: impl Fn(&str) -> bool,
    unknown: &mut Vec<UnknownKey>,
) -> Result<BTreeMap<K, String>, ThemeError> {
    let mut known = BTreeMap::new();
    for (key, value) in raw {
        let Some(parsed) = parse(&key) else {
            unknown.push(UnknownKey { section: name, key });
            continue;
        };
        if value == INHERIT {
            continue;
        }
        if !valid(&value) {
            return Err(bad(&format!("{name}.{key}"), &value));
        }
        known.insert(parsed, value);
    }
    Ok(known)
}

/// A theme read from its package.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSource {
    pub manifest: ThemeManifest,
    /// The package's folder, ending with a path separator.
    pub root: String,
}

/// The themes the app can draw with, and those it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedThemes {
    pub themes: Vec<Theme>,
    pub skipped: Vec<SkippedTheme>,
}

/// A theme package left out, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct SkippedTheme {
    /// The package's folder.
    pub root: String,
    pub problem: ThemeError,
}

/// Every theme, the app's own (`builtin`) and then the others, in their
/// order, with all values filled in (TH-2): a key a theme leaves out comes
/// from its parent, recursively, ending at the default theme
/// (`DEFAULT_THEME_ID`), which must be among `builtin` and set every value.
/// A parent that is missing or already on the way (a loop) also ends at the
/// default theme, so a theme always draws. A screen without a background
/// takes the theme's own background of the screen it falls back to before
/// any parent's (TH-5), so a theme that sets only `default` is not mixed
/// with its parent's pictures.
///
/// The app's own themes ship with it, so any fault in them is an error.
/// Each other theme stands on its own: one with an id under
/// `BUILTIN_PREFIX`, or the id of a theme before it, is left out, and the
/// rest are still there.
pub fn resolve_themes(builtin: &[ThemeSource], others: &[ThemeSource]) -> Result<ResolvedThemes, ThemeError> {
    let mut by_id = BTreeMap::new();
    for source in builtin {
        if by_id.insert(source.manifest.id.as_str(), source).is_some() {
            return Err(ThemeError::DuplicateId(source.manifest.id.clone()));
        }
    }
    let root = *by_id.get(DEFAULT_THEME_ID).ok_or(ThemeError::NoDefault)?;
    let mut themes = builtin.iter().map(|source| resolve(source, &inheritance(source, root, &by_id))).collect::<Result<Vec<_>, _>>()?;
    let mut skipped = Vec::new();
    let mut usable = Vec::new();
    for source in others {
        let id = source.manifest.id.as_str();
        if id.starts_with(BUILTIN_PREFIX) {
            skipped.push(SkippedTheme { root: source.root.clone(), problem: ThemeError::ReservedId(id.to_string()) });
        } else if by_id.insert(id, source).is_some() {
            skipped.push(SkippedTheme { root: source.root.clone(), problem: ThemeError::DuplicateId(id.to_string()) });
        } else {
            usable.push(source);
        }
    }
    for source in usable {
        match resolve(source, &inheritance(source, root, &by_id)) {
            Ok(theme) => themes.push(theme),
            Err(problem) => skipped.push(SkippedTheme { root: source.root.clone(), problem }),
        }
    }
    Ok(ResolvedThemes { themes, skipped })
}

/// `source`, its parents, and the default theme last.
fn inheritance<'a>(source: &'a ThemeSource, root: &'a ThemeSource, by_id: &BTreeMap<&str, &'a ThemeSource>) -> Vec<&'a ThemeSource> {
    let mut chain = vec![source];
    let mut seen = HashSet::from([source.manifest.id.as_str()]);
    let mut next = source;
    while let Some(parent) = next.manifest.parent.as_deref().and_then(|id| by_id.get(id)) {
        if !seen.insert(parent.manifest.id.as_str()) {
            break;
        }
        chain.push(parent);
        next = parent;
    }
    if !seen.contains(root.manifest.id.as_str()) {
        chain.push(root);
    }
    chain
}

/// The theme `own` with each value from the first theme in `chain` (`own`,
/// its parents, the default theme) that has it.
fn resolve(own: &ThemeSource, chain: &[&ThemeSource]) -> Result<Theme, ThemeError> {
    let incomplete = |key: &str| ThemeError::Incomplete(key.to_string());
    let file = |source: &ThemeSource, path: &String| ThemeFile { root: source.root.clone(), path: path.clone() };
    Ok(Theme {
        id: own.manifest.id.clone(),
        name: own.manifest.name.clone(),
        colors: ThemeColors::try_from_fn(|key| chain.iter().find_map(|theme| theme.manifest.colors.get(&key).cloned()))
            .map_err(|key| incomplete(&format!("colors.{}", key.name())))?,
        fonts: ThemeFonts::try_from_fn(|key| chain.iter().find_map(|theme| theme.manifest.fonts.get(&key).map(|path| file(theme, path))))
            .map_err(|key| incomplete(&format!("fonts.{}", key.name())))?,
        backgrounds: ThemeBackgrounds::try_from_fn(|screen| {
            chain.iter().find_map(|theme| screen.fallback_path().find_map(|key| theme.manifest.backgrounds.get(&key).map(|path| file(theme, path))))
        })
        .map_err(|key| incomplete(&format!("backgrounds.{}", key.name())))?,
        buzzer_colors: BuzzerColors::try_from_fn(|key| chain.iter().find_map(|theme| theme.manifest.buzzer_colors.get(&key).cloned()))
            .map_err(|key| incomplete(&format!("buzzerColors.{}", key.name())))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A complete default theme.
    fn root_json() -> serde_json::Value {
        let colors: serde_json::Map<String, serde_json::Value> = ColorKey::ALL.iter().map(|key| (key.name().into(), "#102030".into())).collect();
        serde_json::json!({
            "format": 1,
            "id": DEFAULT_THEME_ID,
            "version": "1.0.0",
            "name": { "cs": "Klasický", "en": "Classic" },
            "colors": colors,
            "fonts": { "heading": "fonts/Heading.woff2", "body": "fonts/Body.woff2" },
            "backgrounds": { "default": "backgrounds/main.svg", "leaderboard": "backgrounds/scores.png" },
            "buzzerColors": { "ready": "#00ff00", "first": "#0000ff" }
        })
    }

    fn manifest(json: serde_json::Value) -> ThemeManifest {
        ThemeManifest::from_json(&json.to_string()).unwrap().0
    }

    fn source(json: serde_json::Value) -> ThemeSource {
        let manifest = manifest(json);
        let root = format!("/themes/{}/", manifest.id);
        ThemeSource { manifest, root }
    }

    fn child(id: &str, parent: Option<&str>, extra: serde_json::Value) -> ThemeSource {
        let mut json = serde_json::json!({ "format": 1, "id": id, "version": "1.0.0", "name": { "cs": id, "en": id } });
        if let Some(parent) = parent {
            json["parent"] = parent.into();
        }
        json.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        source(json)
    }

    fn resolved(sources: &[ThemeSource], id: &str) -> Theme {
        resolve_themes(sources, &[]).unwrap().themes.into_iter().find(|theme| theme.id == id).unwrap()
    }

    #[test]
    fn reads_the_example_from_the_specification() {
        let json = r##"{
            "format": 1, "id": "cz.example.autumn", "version": "1.0.0",
            "name": { "cs": "Podzim", "en": "Autumn" }, "author": "Name <email>", "parent": "builtin.classic",
            "backgrounds": { "default": "backgrounds/main.jpg", "welcome": "backgrounds/welcome.mp4" },
            "colors": { "text": "#ffffff", "panel": "#101830cc", "wrong": "default" },
            "fonts": { "heading": "fonts/Heading.woff2" },
            "buzzerColors": { "ready": "#00ff00" }
        }"##;
        let (theme, unknown) = ThemeManifest::from_json(json).unwrap();
        assert!(unknown.is_empty());
        assert_eq!(theme.version, [1, 0, 0]);
        assert_eq!(theme.parent.as_deref(), Some(DEFAULT_THEME_ID));
        assert_eq!(theme.colors.get(&ColorKey::Panel).map(String::as_str), Some("#101830cc"));
        // "default" takes the value from the parent.
        assert!(!theme.colors.contains_key(&ColorKey::Wrong));
        assert_eq!(theme.backgrounds.get(&ScreenKey::Welcome).map(String::as_str), Some("backgrounds/welcome.mp4"));
    }

    #[test]
    fn refuses_a_format_it_does_not_know() {
        let json = r#"{ "format": 2, "id": "cz.example.next", "whatever": [] }"#;
        assert_eq!(ThemeManifest::from_json(json), Err(ThemeError::UnknownFormat(2)));
    }

    #[test]
    fn ignores_keys_it_does_not_know_and_says_which() {
        let mut json = root_json();
        json["backgrounds"]["lobby"] = "backgrounds/lobby.png".into();
        json["colors"]["sparkle"] = "#ffffff".into();
        json["sounds"] = serde_json::json!({ "correct": "sounds/yes.ogg" });
        let (theme, unknown) = ThemeManifest::from_json(&json.to_string()).unwrap();
        assert_eq!(
            unknown,
            [UnknownKey { section: "colors", key: "sparkle".into() }, UnknownKey { section: "backgrounds", key: "lobby".into() }]
        );
        assert_eq!(theme.backgrounds.len(), 2);
    }

    #[test]
    fn refuses_values_that_break_the_rules() {
        let cases: [(&str, serde_json::Value); 12] = [
            ("/id", "Autumn".into()),
            ("/id", "single".into()),
            ("/id", "cz..double".into()),
            ("/version", "1.0".into()),
            ("/name/cs", "  ".into()),
            ("/colors/text", "white".into()),
            ("/colors/text", "#fff".into()),
            ("/fonts/body", "../fonts/Body.woff2".into()),
            ("/fonts/body", "fonts/Body.png".into()),
            ("/backgrounds/default", "/etc/main.png".into()),
            ("/backgrounds/default", "backgrounds\\main.png".into()),
            ("/backgrounds/default", "C:/main.png".into()),
        ];
        for (pointer, value) in cases {
            let mut json = root_json();
            *json.pointer_mut(pointer).unwrap() = value.clone();
            assert!(
                matches!(ThemeManifest::from_json(&json.to_string()), Err(ThemeError::BadValue { .. })),
                "{pointer} = {value} was accepted"
            );
        }
    }

    #[test]
    fn a_theme_id_is_a_reverse_domain_name_and_a_portable_folder_name() {
        assert!(is_theme_id("builtin.classic"));
        assert!(is_theme_id("cz.example.autumn-2_dark"));
        assert!(!is_theme_id("cz.example."));
        assert!(!is_theme_id("cz/example"));
        assert!(!is_theme_id(&format!("cz.{}", "a".repeat(MAX_THEME_ID_LEN))));
    }

    #[test]
    fn fills_in_what_a_theme_leaves_out_from_its_parents() {
        let sources = [
            source(root_json()),
            child("test.night", Some(DEFAULT_THEME_ID), serde_json::json!({ "colors": { "bar": "#222222", "tile": "#333333" } })),
            child(
                "test.neon",
                Some("test.night"),
                serde_json::json!({ "colors": { "tile": "#444444" }, "fonts": { "heading": "fonts/Exo.woff2" } }),
            ),
        ];
        let neon = resolved(&sources, "test.neon");
        assert_eq!(neon.colors.tile, "#444444");
        assert_eq!(neon.colors.bar, "#222222");
        assert_eq!(neon.colors.text, "#102030");
        assert_eq!(neon.fonts.heading, ThemeFile { root: "/themes/test.neon/".into(), path: "fonts/Exo.woff2".into() });
        // A file keeps the folder of the theme it comes from.
        assert_eq!(neon.fonts.body, ThemeFile { root: "/themes/builtin.classic/".into(), path: "fonts/Body.woff2".into() });
        assert_eq!(neon.buzzer_colors.first, "#0000ff");
    }

    #[test]
    fn a_missing_parent_or_a_loop_ends_at_the_default_theme() {
        let sources = [
            source(root_json()),
            child("test.a", Some("test.b"), serde_json::json!({ "colors": { "text": "#aaaaaa" } })),
            child("test.b", Some("test.a"), serde_json::json!({ "colors": { "bar": "#bbbbbb" } })),
            child("test.orphan", Some("cz.example.gone"), serde_json::json!({})),
            child("test.no-parent", None, serde_json::json!({})),
        ];
        let a = resolved(&sources, "test.a");
        assert_eq!((a.colors.text.as_str(), a.colors.bar.as_str(), a.colors.wrong.as_str()), ("#aaaaaa", "#bbbbbb", "#102030"));
        let classic = resolved(&sources, DEFAULT_THEME_ID);
        assert_eq!(resolved(&sources, "test.orphan").colors, classic.colors);
        assert_eq!(resolved(&sources, "test.no-parent").backgrounds, classic.backgrounds);
    }

    #[test]
    fn a_screen_takes_the_themes_own_background_before_a_parents() {
        let sources = [
            source(root_json()),
            child("test.own", Some(DEFAULT_THEME_ID), serde_json::json!({ "backgrounds": { "default": "own.png", "answer": "answer.webm" } })),
            child("test.final", Some(DEFAULT_THEME_ID), serde_json::json!({ "backgrounds": { "final": "final.png" } })),
        ];
        let own = resolved(&sources, "test.own").backgrounds;
        // The parent has its own leaderboard picture, but this theme's default comes first.
        assert_eq!(own.leaderboard.path, "own.png");
        assert_eq!(own.round_end.path, "own.png");
        assert_eq!(own.answer.path, "answer.webm");
        assert_eq!(own.question.path, "own.png");
        let partial = resolved(&sources, "test.final").backgrounds;
        assert_eq!(partial.r#final.path, "final.png");
        assert_eq!(partial.default.path, "backgrounds/main.svg");
        assert_eq!(partial.round_end.path, "backgrounds/scores.png");
    }

    #[test]
    fn the_default_theme_must_be_there_and_complete() {
        assert_eq!(resolve_themes(&[child("test.a", None, serde_json::json!({}))], &[]), Err(ThemeError::NoDefault));
        assert_eq!(resolve_themes(&[], &[source(root_json())]), Err(ThemeError::NoDefault));
        let mut json = root_json();
        json["colors"].as_object_mut().unwrap().remove("answerF");
        assert_eq!(resolve_themes(&[source(json)], &[]), Err(ThemeError::Incomplete("colors.answerF".into())));
        let twice = [source(root_json()), source(root_json())];
        assert_eq!(resolve_themes(&twice, &[]), Err(ThemeError::DuplicateId(DEFAULT_THEME_ID.into())));
    }

    #[test]
    fn a_theme_that_takes_the_id_of_the_app_or_of_an_earlier_theme_is_left_out_alone() {
        let builtin = [source(root_json()), child("builtin.night", Some(DEFAULT_THEME_ID), serde_json::json!({}))];
        let mut copy = child("test.autumn", None, serde_json::json!({}));
        copy.root = "/elsewhere/test.autumn/".into();
        let others = [
            child(DEFAULT_THEME_ID, None, serde_json::json!({})),
            child("test.autumn", None, serde_json::json!({})),
            copy,
            child("builtin.new", None, serde_json::json!({})),
            child("test.winter", Some("test.autumn"), serde_json::json!({})),
        ];
        let resolved = resolve_themes(&builtin, &others).unwrap();
        let ids: Vec<&str> = resolved.themes.iter().map(|theme| theme.id.as_str()).collect();
        assert_eq!(ids, [DEFAULT_THEME_ID, "builtin.night", "test.autumn", "test.winter"]);
        let skipped = |root: &str, problem: ThemeError| SkippedTheme { root: root.into(), problem };
        assert_eq!(
            resolved.skipped,
            [
                skipped("/themes/builtin.classic/", ThemeError::ReservedId(DEFAULT_THEME_ID.into())),
                skipped("/elsewhere/test.autumn/", ThemeError::DuplicateId("test.autumn".into())),
                skipped("/themes/builtin.new/", ThemeError::ReservedId("builtin.new".into())),
            ]
        );
        // The default theme is still the app's own.
        assert_eq!(resolved.themes[0].name, builtin[0].manifest.name);
    }

    #[test]
    fn every_screen_falls_back_to_default() {
        for screen in ScreenKey::ALL {
            assert_eq!(screen.fallback_path().last(), Some(ScreenKey::Default), "{screen:?}");
        }
    }
}
