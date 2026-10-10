//! The app's icon colour, chosen in Settings: the windows' icon (their title
//! bar and taskbar button on Windows and Linux) in one of the logo's
//! colours, orange by default. Installed shortcuts, the program file and
//! the macOS Dock keep the orange icon the installer gave them.

use crate::app_file::AppFile;
use crate::error::CmdResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use ts_rs::TS;

/// Where the chosen colour is kept, in the app's data folder.
pub const ICON_FILE: &str = "app-icon.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum IconColor {
    #[default]
    Orange,
    Blue,
    Green,
    Purple,
    Red,
    Yellow,
}

impl IconColor {
    #[cfg(test)]
    const ALL: [IconColor; 6] = [IconColor::Orange, IconColor::Blue, IconColor::Green, IconColor::Purple, IconColor::Red, IconColor::Yellow];

    /// The logo in this colour, 128 px square.
    pub fn image(self) -> tauri::Result<Image<'static>> {
        Image::from_bytes(match self {
            IconColor::Orange => include_bytes!("../icons/colors/buzzdome-orange.png"),
            IconColor::Blue => include_bytes!("../icons/colors/buzzdome-blue.png"),
            IconColor::Green => include_bytes!("../icons/colors/buzzdome-green.png"),
            IconColor::Purple => include_bytes!("../icons/colors/buzzdome-purple.png"),
            IconColor::Red => include_bytes!("../icons/colors/buzzdome-red.png"),
            IconColor::Yellow => include_bytes!("../icons/colors/buzzdome-yellow.png"),
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IconFile {
    #[serde(default)]
    color: IconColor,
}

pub struct AppIcon {
    file: AppFile,
    color: Mutex<IconColor>,
}

impl AppIcon {
    /// The colour kept at `path`. A file that cannot be read (an unknown
    /// colour, say) is kept aside and the icon is orange.
    pub fn load(path: PathBuf) -> AppIcon {
        let (file, saved) = AppFile::load::<IconFile>(path);
        if let Some(lost) = file.lost() {
            eprintln!("app icon not read: {} (kept as {:?})", lost.detail, lost.kept_as);
        }
        AppIcon { file, color: Mutex::new(saved.color) }
    }

    pub fn color(&self) -> IconColor {
        *self.color.lock().unwrap()
    }
}

/// Gives `window` the chosen icon; a window that cannot take it keeps the one it has.
pub fn apply(app: &AppHandle, window: &WebviewWindow) {
    let color = app.state::<AppIcon>().color();
    if let Err(e) = color.image().and_then(|image| window.set_icon(image)) {
        eprintln!("app icon not set on {}: {e}", window.label());
    }
}

#[tauri::command]
pub fn app_icon(state: State<AppIcon>) -> IconColor {
    state.color()
}

/// Gives every window the icon in `color` and keeps it for the next start.
#[tauri::command]
pub fn set_app_icon(app: AppHandle, state: State<AppIcon>, color: IconColor) -> CmdResult<()> {
    let image = color.image()?;
    *state.color.lock().unwrap() = color;
    for window in app.webview_windows().values() {
        window.set_icon(image.clone())?;
    }
    let _ = app.emit("app-icon", color);
    state.file.store(&IconFile { color })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_has_its_icon() {
        for color in IconColor::ALL {
            let image = color.image().unwrap();
            assert_eq!((image.width(), image.height()), (128, 128), "{color:?}");
        }
    }

    #[test]
    fn the_icon_is_orange_until_another_colour_is_kept() {
        let dir = std::env::temp_dir().join(format!("app-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(ICON_FILE);
        assert_eq!(AppIcon::load(path.clone()).color(), IconColor::Orange);
        AppIcon::load(path.clone()).file.store(&IconFile { color: IconColor::Purple }).unwrap();
        assert_eq!(AppIcon::load(path.clone()).color(), IconColor::Purple);
        // A colour this build does not know leaves the icon orange.
        std::fs::write(&path, r#"{"color": "pink"}"#).unwrap();
        assert_eq!(AppIcon::load(path).color(), IconColor::Orange);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
