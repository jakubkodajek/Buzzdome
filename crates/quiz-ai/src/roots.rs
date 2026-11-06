//! The folders an assistant may work in: the MCP tools read and write only
//! inside them (AR1), as the reference filesystem server touches only the
//! folders on its command line. The quizmaster picks the folder in the app
//! (File > AI assistants), which keeps it in the AI settings (`settings`),
//! where the server reads it when it starts. The app's own data (shows,
//! drafts, settings) is never among them.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// What the assistant does when it may work in no folder.
const NO_FOLDER: &str = "ask the quizmaster to check the folder in Buzzdome (File > AI assistants), then to open this app again";

#[derive(Debug, thiserror::Error)]
pub enum RootError {
    /// The system names no documents folder, where the default folder is.
    #[error("the system names no documents folder")]
    NoDocumentsFolder,
    #[error("{} cannot be used as a folder to work in: {source}", path.display())]
    Unusable { path: PathBuf, source: io::Error },
    #[error("{} is not a folder", .0.display())]
    NotAFolder(PathBuf),
    #[error("{} holds or is inside Buzzdome's own data ({})", path.display(), data.display())]
    AppData { path: PathBuf, data: PathBuf },
}

/// The folders where the app keeps its data for this user: the data folder
/// (settings, shows, the media library), the local one (unpacked quizzes,
/// editor drafts) and on macOS the log folder, as Tauri names them
/// (`app_data_dir`, `app_local_data_dir`, `app_log_dir`) for `identifier`.
pub fn app_data_dirs(identifier: &str) -> Vec<PathBuf> {
    let env = |name| std::env::var_os(name).map(PathBuf::from);
    let home = env("HOME");
    let bases: Vec<Option<PathBuf>> = if cfg!(windows) {
        vec![env("APPDATA"), env("LOCALAPPDATA")]
    } else if cfg!(target_os = "macos") {
        vec![home.as_ref().map(|home| home.join("Library/Application Support")), home.as_ref().map(|home| home.join("Library/Logs"))]
    } else {
        vec![env("XDG_DATA_HOME").filter(|dir| dir.is_absolute()).or_else(|| home.map(|home| home.join(".local/share")))]
    };
    bases.into_iter().flatten().map(|base| base.join(identifier)).collect()
}

/// `folder`, an existing folder, as the root it gives: its real path, once
/// links are followed (`canonicalize`, which on Windows starts with `\\?\`).
/// Refused when it holds or is inside one of `app_data`.
pub fn allow(folder: &Path, app_data: &[PathBuf]) -> Result<PathBuf, RootError> {
    let root = fs::canonicalize(folder).map_err(|source| RootError::Unusable { path: folder.to_path_buf(), source })?;
    if !root.is_dir() {
        return Err(RootError::NotAFolder(folder.to_path_buf()));
    }
    for data in app_data {
        let data = real(data).unwrap_or_else(|_| data.clone());
        if root.starts_with(&data) || data.starts_with(&root) {
            return Err(RootError::AppData { path: folder.to_path_buf(), data });
        }
    }
    Ok(root)
}

/// The folders an assistant may work in, as `allow` gives them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots(Vec<PathBuf>);

impl Roots {
    /// Roots that `allow` gave.
    pub fn new(roots: Vec<PathBuf>) -> Roots {
        Roots(roots)
    }

    /// The roots as the quizmaster writes them.
    pub fn folders(&self) -> Vec<PathBuf> {
        self.0.iter().cloned().map(plain).collect()
    }

    /// The file or folder `path` names, when it is inside a root once links
    /// are followed; what to tell the assistant otherwise. The path must be
    /// absolute (the server's working folder is the assistant's, which it
    /// does not know) and without `..`. A path that is not there yet is
    /// judged by the nearest folder above it that is.
    ///
    /// A network or device path (`quiz_file::network_or_device`) is refused
    /// before any file is looked at, unless it is on the share of a root
    /// (`share_of`): on Windows looking it up would contact the server and
    /// sign in to it with the user's credentials, whoever told the assistant
    /// the path.
    pub fn resolve(&self, given: &Path) -> Result<PathBuf, String> {
        let path = given.display();
        if quiz_file::network_or_device(&given.as_os_str().to_string_lossy()) {
            let share = share_of(given);
            if share.is_none() || !self.0.iter().any(|root| share_of(root) == share) {
                return Err(format!("{path} is a network or device path: use a path on this computer"));
            }
        }
        if !given.is_absolute() {
            return Err(format!("{path} is not an absolute path"));
        }
        if given.components().any(|part| part == Component::ParentDir) {
            return Err(format!("{path}: use a path without \"..\""));
        }
        let real = real(given).map_err(|e| format!("{path} cannot be used: {e}"))?;
        if self.0.iter().any(|root| real.starts_with(root)) {
            return Ok(plain(real));
        }
        Err(match self.0.as_slice() {
            [] => format!("Buzzdome lets you work in no folder: {NO_FOLDER}"),
            _ => format!("{path} is outside the folders Buzzdome lets you work in: {}", self.describe()),
        })
    }

    /// The roots in words, for the assistant.
    pub fn describe(&self) -> String {
        if self.0.is_empty() {
            return format!("none: {NO_FOLDER}");
        }
        self.folders().iter().map(|root| root.display().to_string()).collect::<Vec<_>>().join(", ")
    }
}

/// The `server\share` a network path names, in lower case (Windows
/// compares them so): from `\\?\UNC\server\share`, as a root's real path
/// reads, or `\\server\share` and `//server/share`, as written. None for a
/// local or device path (`\\?\C:\`, `\\.\pipe`, `\??\`).
fn share_of(path: &Path) -> Option<String> {
    let text = path.as_os_str().to_string_lossy();
    let separators = |c: char| c == '\\' || c == '/';
    let rest = match text.get(..8) {
        Some(head) if head.eq_ignore_ascii_case(r"\\?\UNC\") => &text[8..],
        _ if quiz_file::network_or_device(&text) && !text[1..].starts_with(['?', '.']) && !text[2..].starts_with(['?', '.']) => &text[2..],
        _ => return None,
    };
    let mut parts = rest.split(separators);
    match (parts.next(), parts.next()) {
        (Some(server), Some(share)) if !server.is_empty() && !share.is_empty() => Some(format!("{server}\\{share}").to_lowercase()),
        _ => None,
    }
}

/// `path` with links followed: the real path of the nearest part of it that
/// is there, with the names after it that are not there yet.
fn real(path: &Path) -> io::Result<PathBuf> {
    let mut missing = Vec::new();
    let mut there = path;
    loop {
        match fs::canonicalize(there) {
            Ok(mut real) => {
                real.extend(missing.iter().rev());
                return Ok(real);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let (Some(parent), Some(name)) = (there.parent(), there.file_name()) else { return Err(e) };
                missing.push(name);
                there = parent;
            }
            Err(e) => return Err(e),
        }
    }
}

/// `path` without the `\\?\` that `canonicalize` puts before a drive letter
/// on Windows, as the assistant and the quizmaster write it.
fn plain(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::Prefix;
        let mut parts = path.components();
        let drive = match parts.next() {
            Some(Component::Prefix(prefix)) => match prefix.kind() {
                Prefix::VerbatimDisk(drive) => Some(drive),
                _ => None,
            },
            _ => None,
        };
        if let Some(drive) = drive {
            let mut simple = PathBuf::from(format!("{}:\\", drive as char));
            simple.extend(parts.filter(|part| !matches!(part, Component::RootDir)));
            return simple;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::tests::scratch;

    #[test]
    fn only_paths_inside_a_root_are_given() {
        let dir = scratch("roots");
        let (quizzes, private) = (dir.join("quizzes"), dir.join("private"));
        fs::create_dir_all(quizzes.join("pub")).unwrap();
        fs::create_dir_all(&private).unwrap();
        let roots = Roots::new(vec![allow(&quizzes, &[]).unwrap()]);
        let real_quizzes = fs::canonicalize(&quizzes).unwrap();
        assert_eq!(roots.resolve(&quizzes).unwrap(), plain(real_quizzes.clone()));
        assert_eq!(roots.resolve(&quizzes.join("pub/new/quiz.json")).unwrap(), plain(real_quizzes.join("pub").join("new").join("quiz.json")));
        assert!(roots.resolve(&private).unwrap_err().contains("outside the folders"));
        assert!(roots.resolve(&dir.join("quizzes-2/x")).unwrap_err().contains("outside the folders"));
        assert!(roots.resolve(&quizzes.join("../private")).unwrap_err().contains(".."));
        assert!(roots.resolve(&quizzes.join("pub/../../private")).unwrap_err().contains(".."));
        assert!(roots.resolve(Path::new("quizzes/x")).unwrap_err().contains("not an absolute path"));
        assert!(Roots::default().resolve(&quizzes).unwrap_err().contains("no folder"));
    }

    #[test]
    fn a_network_or_device_path_is_refused_before_it_is_looked_at() {
        let dir = scratch("roots-network");
        fs::create_dir_all(&dir).unwrap();
        let roots = Roots::new(vec![allow(&dir, &[]).unwrap()]);
        for given in [r"\\host\share\x.png", "//host/share/x.png", r"\\?\UNC\host\share\x.png", r"\\.\pipe\x", r"\??\C:\x.png"] {
            let refused = roots.resolve(Path::new(given)).unwrap_err();
            assert!(refused.contains("network or device path"), "{given}: {refused}");
        }
        let host = Some(r"host\share".to_string());
        assert_eq!(share_of(Path::new(r"\\?\UNC\Host\Share\quizzes")), host);
        assert_eq!(share_of(Path::new(r"\\host\share")), host);
        assert_eq!(share_of(Path::new("//host/share/x.png")), host);
        for local in [r"\\?\C:\Users\me\Quizzes", r"\\.\pipe\x", r"\??\C:\x.png", r"\\host", r"C:\x"] {
            assert_eq!(share_of(Path::new(local)), None, "{local}");
        }
        assert_eq!(share_of(&dir), None);
        // With a root on a share, a path on another share is still refused by its text.
        let on_share = Roots(vec![PathBuf::from(r"\\?\UNC\host\share\quizzes")]);
        for given in [r"\\other\share\x.png", r"\\host\other\x.png", r"\\.\pipe\x"] {
            let refused = on_share.resolve(Path::new(given)).unwrap_err();
            assert!(refused.contains("network or device path"), "{given}: {refused}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_a_root_is_refused() {
        let dir = scratch("roots-link");
        let (quizzes, private) = (dir.join("quizzes"), dir.join("private"));
        fs::create_dir_all(&quizzes).unwrap();
        fs::create_dir_all(&private).unwrap();
        fs::write(private.join("photo.png"), b"x").unwrap();
        std::os::unix::fs::symlink(&private, quizzes.join("out")).unwrap();
        let roots = Roots::new(vec![allow(&quizzes, &[]).unwrap()]);
        assert!(roots.resolve(&quizzes.join("out/photo.png")).is_err());
        assert!(roots.resolve(&quizzes.join("out/new-quiz")).is_err());
    }

    #[test]
    fn the_apps_own_data_is_never_a_root() {
        let dir = scratch("roots-data");
        let data = dir.join("data").join("cz.kodajek.quizshow");
        fs::create_dir_all(data.join("sessions")).unwrap();
        let app_data = [data.clone()];
        assert!(matches!(allow(&data.join("sessions"), &app_data), Err(RootError::AppData { .. })));
        assert!(matches!(allow(&dir, &app_data), Err(RootError::AppData { .. })));
        assert!(matches!(allow(&dir.join("missing"), &app_data), Err(RootError::Unusable { .. })));
        fs::write(dir.join("file"), b"x").unwrap();
        assert!(matches!(allow(&dir.join("file"), &app_data), Err(RootError::NotAFolder(_))));
        // Data that is not there yet is kept out as well.
        assert!(matches!(allow(&dir, &[dir.join("later").join("cz.kodajek.quizshow")]), Err(RootError::AppData { .. })));
    }
}
