//! The folder picker in the UI, on this device: the subdirectories of one directory. Only
//! names of directories leave the device, never files or their contents.

use std::path::{Path, PathBuf};

use stellwerk_protocol::messages::{Directory, Entry};

/// The directory at `requested` (absolute, `~/…`, or empty for `home`) and its visible
/// subdirectories. Symbolic links to directories count as directories: that is where a
/// folder would sync to.
pub fn list(requested: &str, home: Option<&Path>) -> Result<Directory, String> {
    let path = resolve(requested, home)?;
    let entries = std::fs::read_dir(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut dirs: Vec<Entry> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (!name.starts_with('.')).then(|| Entry {
                path: text(&entry.path()),
                name,
            })
        })
        .collect();
    dirs.sort_by_key(|entry| entry.name.to_lowercase());
    Ok(Directory {
        path: text(&path),
        parent: path.parent().map(text),
        dirs,
        home: home.map(text),
        roots: roots(),
    })
}

/// The path to list: `requested` with `~` expanded, the home directory when empty.
fn resolve(requested: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    let requested = requested.trim();
    let tilde = requested
        .strip_prefix('~')
        .filter(|rest| rest.is_empty() || rest.starts_with(['/', '\\']));
    let path = match (requested, tilde, home) {
        ("", _, Some(home)) => home.to_path_buf(),
        ("", _, None) => PathBuf::from(roots().swap_remove(0)),
        (_, Some(rest), Some(home)) => home.join(rest.trim_start_matches(['/', '\\'])),
        _ => PathBuf::from(requested),
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(format!("not an absolute path: {requested}"))
    }
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `/`, or every drive Windows has right now.
fn roots() -> Vec<String> {
    if cfg!(windows) {
        let drives: Vec<String> = ('A'..='Z')
            .map(|letter| format!("{letter}:\\"))
            .filter(|drive| Path::new(drive).is_dir())
            .collect();
        if drives.is_empty() {
            vec!["C:\\".into()]
        } else {
            drives
        }
    } else {
        vec!["/".into()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_visible_subdirectories_sorted_by_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["photos", "Documents", ".cache", "music"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        std::fs::write(dir.path().join("notes.txt"), "a file").unwrap();

        let listed = list(&text(dir.path()), None).unwrap();
        let names: Vec<&str> = listed.dirs.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Documents", "music", "photos"]);
        assert_eq!(listed.dirs[0].path, text(&dir.path().join("Documents")));
        assert_eq!(listed.path, text(dir.path()));
        assert_eq!(listed.parent, dir.path().parent().map(text));
        assert!(!listed.roots.is_empty());
    }

    #[test]
    fn an_empty_path_or_a_tilde_is_the_home_directory() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join("Pictures/2026")).unwrap();

        let listed = list("", Some(home.path())).unwrap();
        assert_eq!(listed.path, text(home.path()));
        assert_eq!(listed.home, Some(text(home.path())));
        assert_eq!(listed.dirs[0].name, "Pictures");

        let pictures = list("~/Pictures", Some(home.path())).unwrap();
        assert_eq!(pictures.path, text(&home.path().join("Pictures")));
        assert_eq!(pictures.dirs[0].name, "2026");
    }

    #[test]
    fn a_relative_or_missing_path_is_an_error() {
        let home = tempfile::tempdir().unwrap();
        assert!(list("Pictures", Some(home.path())).is_err());
        assert!(list("~someone/x", Some(home.path())).is_err());
        let missing = home.path().join("gone");
        let err = list(&text(&missing), None).unwrap_err();
        assert!(err.contains("gone"), "{err}");
    }

    #[test]
    fn the_root_has_no_parent() {
        let root = roots().swap_remove(0);
        assert_eq!(list(&root, None).unwrap().parent, None);
    }
}
