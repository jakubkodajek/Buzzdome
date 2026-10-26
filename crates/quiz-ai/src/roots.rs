//! The folders an assistant may work in: the MCP tools read and write only
//! inside them (AR1), as the reference filesystem server touches only the
//! folders on its command line. The quizmaster picks them in the app
//! (File > AI assistants), which passes them as `--root <folder>`. The app's
//! own data (shows, drafts, settings) is never among them.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// The argument that names a folder the assistant may work in.
pub const ROOT_ARG: &str = "--root";
/// The app's bundle identifier, which names its data folders. The app reads
/// its own from its configuration; a test there keeps the two the same.
pub const APP_IDENTIFIER: &str = "cz.kodajek.quizshow";

#[derive(Debug, thiserror::Error)]
pub enum RootError {
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

    /// The folders `--root` names among `args`, each allowed (`allow`), and
    /// why the others are not.
    pub fn from_args(args: &[impl AsRef<std::ffi::OsStr>], app_data: &[PathBuf]) -> (Roots, Vec<RootError>) {
        let mut roots = Vec::new();
        let mut refused = Vec::new();
        let mut args = args.iter().map(AsRef::as_ref);
        while let Some(arg) = args.next() {
            if arg != ROOT_ARG {
                continue;
            }
            match args.next().map(|folder| allow(Path::new(folder), app_data)) {
                Some(Ok(root)) => roots.push(root),
                Some(Err(error)) => refused.push(error),
                None => {}
            }
        }
        (Roots(roots), refused)
    }

    /// The roots as the quizmaster writes them.
    pub fn folders(&self) -> Vec<PathBuf> {
        self.0.iter().cloned().map(plain).collect()
    }

    /// The `--root` arguments that give these roots.
    pub fn args(&self) -> Vec<String> {
        self.folders().iter().flat_map(|root| [ROOT_ARG.to_string(), root.display().to_string()]).collect()
    }

    /// The file or folder `path` names, when it is inside a root once links
    /// are followed; what to tell the assistant otherwise. The path must be
    /// absolute (the server's working folder is the assistant's, which it
    /// does not know) and without `..`. A path that is not there yet is
    /// judged by the nearest folder above it that is.
    pub fn resolve(&self, given: &Path) -> Result<PathBuf, String> {
        let path = given.display();
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
            [] => "Buzzdome lets you work in no folder: ask the quizmaster to connect you again in Buzzdome (File > AI assistants)".into(),
            _ => format!("{path} is outside the folders Buzzdome lets you work in: {}", self.describe()),
        })
    }

    /// The roots in words, for the assistant.
    pub fn describe(&self) -> String {
        if self.0.is_empty() {
            return "none: the quizmaster connects you again in Buzzdome (File > AI assistants) to choose one".into();
        }
        self.folders().iter().map(|root| root.display().to_string()).collect::<Vec<_>>().join(", ")
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

    fn text(path: &Path) -> String {
        path.to_str().unwrap().to_string()
    }

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
        let data = dir.join("data").join(APP_IDENTIFIER);
        fs::create_dir_all(data.join("sessions")).unwrap();
        let app_data = [data.clone()];
        assert!(matches!(allow(&data.join("sessions"), &app_data), Err(RootError::AppData { .. })));
        assert!(matches!(allow(&dir, &app_data), Err(RootError::AppData { .. })));
        assert!(matches!(allow(&dir.join("missing"), &app_data), Err(RootError::Unusable { .. })));
        fs::write(dir.join("file"), b"x").unwrap();
        assert!(matches!(allow(&dir.join("file"), &app_data), Err(RootError::NotAFolder(_))));
        // Data that is not there yet is kept out as well.
        assert!(matches!(allow(&dir, &[dir.join("later").join(APP_IDENTIFIER)]), Err(RootError::AppData { .. })));
    }

    #[test]
    fn roots_come_from_the_arguments_and_go_back_to_them() {
        let dir = scratch("roots-args");
        fs::create_dir_all(dir.join("a")).unwrap();
        let args = ["--mcp", ROOT_ARG, &text(&dir.join("a")), ROOT_ARG, &text(&dir.join("gone")), ROOT_ARG];
        let (roots, refused) = Roots::from_args(&args, &[]);
        assert_eq!(roots.folders().len(), 1);
        assert_eq!(refused.len(), 1);
        assert_eq!(roots.args(), [ROOT_ARG.to_string(), roots.folders()[0].display().to_string()]);
    }
}
