//! The built-in themes (spec 5.2): theme packages in `app/src-tauri/themes/<id>/`,
//! shipped with the app as resources. Their `theme.json` files are also
//! compiled in, so the app always has its complete default theme; their
//! pictures and fonts are served to the windows from the resource folder.

use quiz_core::{resolve_themes, Theme, ThemeError, ThemeManifest, ThemeSource};
use tauri::State;

/// The folder of the theme packages in the app's resources (`bundle.resources`).
pub const RESOURCE_FOLDER: &str = "themes";

/// A built-in theme: its id, which is also its folder, and its `theme.json`.
macro_rules! builtin {
    ($id:literal) => {
        ($id, include_str!(concat!("../themes/", $id, "/theme.json")))
    };
}

/// The built-in themes in gallery order, the default first.
const BUILTIN: [(&str, &str); 5] = [
    builtin!("builtin.classic"),
    builtin!("builtin.studio"),
    builtin!("builtin.night"),
    builtin!("builtin.showtime"),
    builtin!("builtin.neon"),
];

/// The themes the windows can draw with, read once at start.
pub struct Themes(pub Vec<Theme>);

/// The built-in themes with every value filled in. `folder_of` gives a
/// theme's package folder from its id, ending with a path separator.
pub fn builtin_themes(folder_of: impl Fn(&str) -> String) -> Result<Vec<Theme>, ThemeError> {
    let sources = BUILTIN
        .iter()
        .map(|(id, json)| Ok(ThemeSource { manifest: ThemeManifest::from_json(json)?.0, root: folder_of(id) }))
        .collect::<Result<Vec<_>, ThemeError>>()?;
    resolve_themes(&sources)
}

/// Every installed theme, the default first (`DEFAULT_THEME_ID`).
#[tauri::command]
pub fn list_themes(themes: State<Themes>) -> Vec<Theme> {
    themes.0.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quiz_core::{ThemeFile, DEFAULT_THEME_ID};
    use std::path::Path;

    const PACKAGES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/themes");

    fn on_disk(id: &str) -> String {
        format!("{PACKAGES}/{id}/")
    }

    #[test]
    fn built_in_themes_are_valid_packages_in_their_own_folders() {
        for (id, json) in BUILTIN {
            let (manifest, unknown) = ThemeManifest::from_json(json).unwrap();
            assert_eq!(manifest.id, id);
            assert!(unknown.is_empty(), "{id}: {unknown:?}");
        }
        let themes = builtin_themes(on_disk).unwrap();
        assert_eq!(themes[0].id, DEFAULT_THEME_ID);
        assert_eq!(themes.len(), BUILTIN.len());
    }

    #[test]
    fn every_file_a_built_in_theme_uses_is_there_with_its_font_licence() {
        for theme in builtin_themes(on_disk).unwrap() {
            let b = &theme.backgrounds;
            let backgrounds = [&b.default, &b.welcome, &b.question, &b.answer, &b.leaderboard, &b.round_end, &b.r#final];
            let fonts = [&theme.fonts.heading, &theme.fonts.body];
            for ThemeFile { root, path } in backgrounds.into_iter().chain(fonts) {
                assert!(Path::new(root).join(path).is_file(), "{}: {root}{path}", theme.id);
            }
            // Fonts are under the SIL Open Font License, which goes with them.
            for ThemeFile { root, path } in fonts {
                let licence = Path::new(root).join(path).with_file_name("OFL.txt");
                assert!(licence.is_file(), "{}: {}", theme.id, licence.display());
            }
        }
    }

    /// The browser preview and the UI tests use the built-in themes as
    /// `list_themes` returns them, with the files served by Vite.
    #[test]
    fn export_built_in_themes_for_the_preview() {
        let Ok(dir) = std::env::var("TS_RS_EXPORT_DIR") else { return };
        let themes = builtin_themes(|id| format!("/src-tauri/{RESOURCE_FOLDER}/{id}/")).unwrap();
        let json = serde_json::to_string_pretty(&themes).unwrap() + "\n";
        std::fs::write(Path::new(&dir).join("BuiltinThemes.json"), json).unwrap();
    }
}
