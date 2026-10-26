//! File pickers the backend opens itself. A file the app reads or
//! writes for the quizmaster is one picked here, never a path a window
//! names, so a window cannot have a file written anywhere; and picking adds
//! nothing to what the windows may load, as the dialog plugin's own
//! commands would (they add the file to the asset scope).

use crate::error::CmdResult;
use crate::off_ui;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tauri::Window;
use tauri_plugin_dialog::{DialogExt, FileDialogBuilder, FilePath};
use ts_rs::TS;

/// A picker's wording, in the quizmaster's language: the UI words it.
#[derive(Debug, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Picker {
    /// Names the files offered, such as "Quiz files".
    pub filter: String,
    /// The name a save picker offers, without the extension it adds, worded
    /// freely (a quiz title): the backend turns it into a file name that
    /// every supported system can store.
    #[ts(optional)]
    pub file_name: Option<String>,
}

/// Picks a file to read among those with one of `extensions`; none when the
/// quizmaster cancels.
pub async fn pick_file(window: Window, picker: Picker, extensions: Vec<&'static str>) -> CmdResult<Option<PathBuf>> {
    off_ui(move || Ok(dialog(&window, &picker, &extensions).blocking_pick_file().and_then(local))).await
}

/// Picks files to read among those with one of `extensions`.
pub async fn pick_files(window: Window, picker: Picker, extensions: Vec<&'static str>) -> CmdResult<Vec<PathBuf>> {
    off_ui(move || Ok(dialog(&window, &picker, &extensions).blocking_pick_files().unwrap_or_default().into_iter().filter_map(local).collect())).await
}

/// Picks where to write a file with `extension`, offering the folder and
/// name of `start` if given, else the picker's name. A name typed without
/// the extension gets it.
pub async fn pick_save(window: Window, picker: Picker, extension: &'static str, start: Option<PathBuf>) -> CmdResult<Option<PathBuf>> {
    off_ui(move || {
        let mut dialog = dialog(&window, &picker, &[extension]);
        let folder = start.as_deref().and_then(Path::parent).filter(|folder| !folder.as_os_str().is_empty());
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        let stem = start.as_deref().and_then(Path::file_stem).map(PathBuf::from).or(picker.file_name.as_deref().and_then(plain_name));
        if let Some(stem) = stem {
            dialog = dialog.set_file_name(with_extension(stem, extension).to_string_lossy());
        }
        Ok(dialog.blocking_save_file().and_then(local).map(|path| with_extension(path, extension)))
    })
    .await
}

/// Picks a folder, starting in `start` if given; none when the quizmaster cancels.
pub async fn pick_folder(window: Window, start: Option<PathBuf>) -> CmdResult<Option<PathBuf>> {
    off_ui(move || {
        let mut dialog = window.dialog().file().set_parent(&window);
        if let Some(start) = start {
            dialog = dialog.set_directory(start);
        }
        Ok(dialog.blocking_pick_folder().and_then(local))
    })
    .await
}

/// A picker over the window that asked, offering files with `extensions`.
fn dialog(window: &Window, picker: &Picker, extensions: &[&str]) -> FileDialogBuilder<tauri::Wry> {
    // Upper case too: some systems match file names case-sensitively.
    let upper: Vec<String> = extensions.iter().map(|extension| extension.to_uppercase()).collect();
    let offered: Vec<&str> = extensions.iter().copied().chain(upper.iter().map(String::as_str)).collect();
    window.dialog().file().set_parent(window).add_filter(&picker.filter, &offered)
}

/// The path of a picked file; none for a URL, which only mobile pickers give.
fn local(picked: FilePath) -> Option<PathBuf> {
    picked.into_path().ok()
}

/// The longest name a save picker offers, in characters: a long quiz title
/// stays well under the 255 that file systems allow, extension included.
const MAX_OFFERED_CHARS: usize = 100;

/// `offered`, a name the page words, as a file name every supported system
/// can store (`quiz_core::portable_name`): a character some system refuses,
/// a path separator among them, becomes `_`, and a Windows device name is
/// prefixed. Cut to `MAX_OFFERED_CHARS` and without leading or trailing dots
/// and spaces, so it is never hidden, `.` or `..`. None when nothing of a
/// name is left.
fn plain_name(offered: &str) -> Option<PathBuf> {
    let capped: String = offered.chars().take(MAX_OFFERED_CHARS).collect();
    let name = capped.trim_matches(|c: char| c == '.' || c.is_whitespace());
    (!name.is_empty()).then(|| PathBuf::from(quiz_core::portable_name(name)))
}

/// `path`, ending in `.extension` (in any case) as a save picker may not.
fn with_extension(path: PathBuf, extension: &str) -> PathBuf {
    if Path::new(&path).extension().is_some_and(|found| found.eq_ignore_ascii_case(extension)) {
        return path;
    }
    let mut name = OsString::from(path);
    name.push(".");
    name.push(extension);
    name.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_file_gets_its_extension_once() {
        assert_eq!(with_extension("/q/Round 1".into(), "quiz"), PathBuf::from("/q/Round 1.quiz"));
        assert_eq!(with_extension("/q/Round 1.quiz".into(), "quiz"), PathBuf::from("/q/Round 1.quiz"));
        assert_eq!(with_extension("/q/ROUND.QUIZ".into(), "quiz"), PathBuf::from("/q/ROUND.QUIZ"));
        // A quiz is never saved over a plain JSON quiz.
        assert_eq!(with_extension("/q/round.json".into(), "quiz"), PathBuf::from("/q/round.json.quiz"));
        assert_eq!(with_extension("v1.2".into(), "csv"), PathBuf::from("v1.2.csv"));
    }

    #[test]
    fn a_save_picker_offers_a_name_every_system_stores() {
        let plain = |offered: &str| plain_name(offered).map(|name| name.to_string_lossy().into_owned());
        assert_eq!(plain("Round 1").as_deref(), Some("Round 1"));
        assert_eq!(plain("Rock/Pop: 80s?").as_deref(), Some("Rock_Pop_ 80s_"));
        assert_eq!(plain("../../etc/passwd").as_deref(), Some("_.._etc_passwd"));
        assert_eq!(plain("C:\\Windows\\win.ini").as_deref(), Some("C__Windows_win.ini"));
        assert_eq!(plain("\\\\server\\share").as_deref(), Some("__server_share"));
        assert_eq!(plain("a*b\"c<d>e|f").as_deref(), Some("a_b_c_d_e_f"));
        assert_eq!(plain(" Pub\u{0}\nquiz ").as_deref(), Some("Pub__quiz"));
        assert_eq!(plain(".hidden.").as_deref(), Some("hidden"));
        assert_eq!(plain("CON").as_deref(), Some("_CON"));
        assert_eq!(plain("Kvíz č. 1").as_deref(), Some("Kvíz č. 1"));
        let long = "ř".repeat(MAX_OFFERED_CHARS + 50);
        assert_eq!(plain(&long).map(|name| name.chars().count()), Some(MAX_OFFERED_CHARS));
        // A cut that ends in a space or dot loses it.
        assert_eq!(plain(&format!("{} x", "a".repeat(MAX_OFFERED_CHARS - 1))), Some("a".repeat(MAX_OFFERED_CHARS - 1)));
        for nothing in ["", " ", "..", ".", " . ", "\t"] {
            assert_eq!(plain(nothing), None, "{nothing:?}");
        }
    }
}
