//! The fonts installed on this computer, for the editor's font list (the
//! `font` of a slide's text style).

use quiz_core::FontName;
use std::sync::OnceLock;

/// The family names of the installed fonts, read once per run: sorted, each
/// once, and only names a quiz can store (`FontName`). Empty when none can
/// be read; the editor then offers common fonts.
#[tauri::command]
pub async fn installed_fonts() -> Vec<String> {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    if let Some(fonts) = FONTS.get() {
        return fonts.clone();
    }
    // Reading every font file takes a moment, so not on the async runtime.
    let fonts = tauri::async_runtime::spawn_blocking(read_installed).await.unwrap_or_default();
    FONTS.get_or_init(|| fonts).clone()
}

fn read_installed() -> Vec<String> {
    let mut fonts = fontdb::Database::new();
    fonts.load_system_fonts();
    family_names(fonts.faces().filter_map(|face| face.families.first().map(|(name, _)| name.clone())))
}

/// `names` sorted regardless of case, each once (CSS matches a family name
/// regardless of case, so the first spelling stands for all), without
/// those a quiz cannot store.
fn family_names(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut names: Vec<String> = names.filter(|name| FontName::try_from(name.clone()).is_ok()).collect();
    names.sort_by_cached_key(|name| name.to_lowercase());
    names.dedup_by_key(|name| name.to_lowercase());
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_each_family_once_in_order_and_only_storable_names() {
        let faces = ["Noto Sans", "arial", "Noto Sans", "Bad \"Font\"", "Arial Black", "Noto Sans", "Arial", "NOTO SANS"];
        assert_eq!(family_names(faces.into_iter().map(String::from)), ["arial", "Arial Black", "Noto Sans"]);
    }
}
