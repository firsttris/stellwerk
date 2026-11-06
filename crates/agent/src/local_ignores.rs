//! Exclusions other programs on this device ask for: files Stellwerk must not sync because
//! another program moves them itself (Spielstand, for game saves it synchronises).
//!
//! A program writes one list into `<data dir>/ignore.d/<name>.list`: one absolute path per
//! line, `#` starts a comment. On every desired state, and when a list changes, each path that
//! lies inside a folder becomes a rooted pattern for exactly that path, added to the folder's
//! patterns from the Zentrale. The lists stay on the device: other devices get the same
//! patterns from their own lists, and the Zentrale's patterns are untouched.

use std::path::{Component, Path, PathBuf};

use stellwerk_engine::ignore::{anchored_pattern, exclusion_position};
use stellwerk_protocol::DesiredState;

pub const DIR: &str = "ignore.d";

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR)
}

/// The text of every `*.list` file, sorted by name: what is compared to see a change. Read
/// lossily: a line in another encoding matches nothing, the lines next to it still count.
pub fn read_lists(dir: &Path) -> String {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return String::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "list"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|f| std::fs::read(f).ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The absolute paths in the lists' text; comments, blank and relative lines are skipped. A
/// path is taken as written: its spaces belong to the name.
pub fn paths(lists: &str) -> Vec<PathBuf> {
    lists
        .lines()
        .filter(|l| {
            let line = l.trim();
            !line.is_empty() && !line.starts_with('#')
        })
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .collect()
}

/// A folder path from the Zentrale with `~` expanded, as Syncthing does.
fn folder_root(path: &str, home: Option<&Path>) -> Option<PathBuf> {
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            Some(home?.join(rest.trim_start_matches('/')))
        }
        Some(_) => None,
        None => Some(PathBuf::from(path)),
    }
}

/// `path` with `..` resolved by its text.
fn without_dots(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}

/// `path` inside `root`, `/`-separated as Syncthing patterns are on every platform.
fn relative_in(path: &Path, root: &Path) -> Option<String> {
    let path = without_dots(path);
    let root = without_dots(root);
    let parts = path
        .strip_prefix(&root)
        .ok()?
        .components()
        .map(|c| c.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()?;
    Some(parts.join("/"))
}

/// `path` with the links on the way to it followed, as far as it exists.
pub fn real(path: &Path) -> PathBuf {
    path.ancestors()
        .find_map(|dir| {
            let rest = path.strip_prefix(dir).ok()?;
            Some(std::fs::canonicalize(dir).ok()?.join(rest))
        })
        .unwrap_or_else(|| path.to_path_buf())
}

/// A listed path with `real` applied to its directory only: a link in a folder is excluded as
/// the link, which is what Syncthing sees there.
fn real_listed(path: &Path, real: &impl Fn(&Path) -> PathBuf) -> PathBuf {
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => real(dir).join(name),
        _ => path.to_path_buf(),
    }
}

/// `state` with the excluded paths added to the folders they lie in, by their text.
pub fn merge(state: &DesiredState, excluded: &[PathBuf], home: Option<&Path>) -> DesiredState {
    merge_with(state, excluded, home, Path::to_path_buf)
}

/// [`merge`] that also compares paths as `real` resolves them, so a path that reaches a folder
/// through a link lies in it (on Fedora Atomic systems `/home` is a link to `/var/home`).
pub fn merge_with(
    state: &DesiredState,
    excluded: &[PathBuf],
    home: Option<&Path>,
    real: impl Fn(&Path) -> PathBuf,
) -> DesiredState {
    let mut merged = state.clone();
    for folder in &mut merged.folders {
        let Some(root) = folder_root(&folder.path, home) else {
            continue;
        };
        let real_root = real(&root);
        for path in excluded {
            let relative = relative_in(&real_listed(path, &real), &real_root)
                .or_else(|| relative_in(path, &root));
            let Some(pattern) = relative.and_then(|r| anchored_pattern(&r)) else {
                continue;
            };
            let at = exclusion_position(&folder.ignore_patterns);
            if !folder.ignore_patterns[..at].contains(&pattern) {
                folder.ignore_patterns.insert(at, pattern);
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellwerk_engine::ignore::ESCAPE;
    use stellwerk_protocol::{DeviceId, Folder, FolderMode, Versioning};

    fn folder(id: &str, path: &str) -> Folder {
        Folder {
            id: id.into(),
            label: id.into(),
            path: path.into(),
            mode: FolderMode::SendReceive,
            shared_with: vec![DeviceId::new("HUB")],
            versioning: Versioning::None,
            ignore_patterns: vec!["*.tmp".into()],
        }
    }

    #[test]
    fn listed_paths_become_rooted_patterns_of_the_folder_they_are_in() {
        // Real absolute paths of this platform: on Windows `C:\…`, with `\` between the parts.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let photos = home.join("photos");
        let state = DesiredState {
            revision: 3,
            peers: vec![],
            folders: vec![
                folder("games", "~/Games"),
                folder("photos", photos.to_str().unwrap()),
            ],
        };
        let save = home
            .join("Games")
            .join("Hades")
            .join("prefix")
            .join("Save [1].sav");
        let excluded = paths(&format!(
            "# Written by Spielstand\n{save}\n\nrelative/ignored\n{elsewhere}\n{save}\n",
            save = save.display(),
            elsewhere = home.join("elsewhere").join("x.sav").display(),
        ));
        assert_eq!(excluded.len(), 3, "the relative line is skipped");
        let merged = merge(&state, &excluded, Some(home));
        let e = ESCAPE;
        assert_eq!(
            merged.folders[0].ignore_patterns,
            vec![
                "*.tmp".to_string(),
                format!("/Hades/prefix/Save {e}[1{e}].sav")
            ]
        );
        assert_eq!(merged.folders[1].ignore_patterns, vec!["*.tmp".to_string()]);
        assert_eq!(merged.revision, 3);
        assert_eq!(
            merge(&state, &[], None),
            state,
            "nothing listed, nothing changed"
        );
    }

    #[test]
    fn a_home_folder_needs_a_home() {
        assert_eq!(folder_root("~/Games", None), None);
        assert_eq!(
            folder_root("~other/Games", Some(Path::new("/home/t"))),
            None
        );
        assert_eq!(
            folder_root("~", Some(Path::new("/home/t"))),
            Some(PathBuf::from("/home/t"))
        );
    }

    fn games(root: &Path, ignore_patterns: &[&str]) -> DesiredState {
        let mut games = folder("games", root.to_str().unwrap());
        games.ignore_patterns = ignore_patterns.iter().map(ToString::to_string).collect();
        DesiredState {
            revision: 1,
            peers: vec![],
            folders: vec![games],
        }
    }

    /// Syncthing stops at the first pattern that matches (lib/ignore/ignore.go, `Match`).
    #[test]
    fn an_exclusion_comes_before_the_patterns_that_keep_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Games");
        let state = games(&root, &["*.tmp", "(?i)!*.sav", "*"]);
        let merged = merge(&state, &[root.join("x.sav")], None);
        assert_eq!(
            merged.folders[0].ignore_patterns,
            vec!["*.tmp", "/x.sav", "(?i)!*.sav", "*"]
        );
    }

    /// Syncthing trims pattern lines, so such a name cannot be matched; trimming it here would
    /// exclude the file without the space instead.
    #[test]
    fn a_path_ending_in_a_space_leaves_its_neighbour_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Games");
        let lists = format!("{}\n", root.join("save.dat ").display());
        let merged = merge(&games(&root, &["*.tmp"]), &paths(&lists), None);
        assert_eq!(merged.folders[0].ignore_patterns, vec!["*.tmp".to_string()]);
    }

    #[test]
    fn a_line_that_is_not_utf8_costs_only_that_line() {
        let tmp = tempfile::tempdir().unwrap();
        let good = tmp.path().join("good.sav");
        let mut list = format!("{}\n", good.display()).into_bytes();
        list.extend_from_slice(b"/caf\xe9/old-encoding.sav\n");
        std::fs::write(tmp.path().join("other.list"), list).unwrap();
        assert!(paths(&read_lists(tmp.path())).contains(&good));
    }

    #[test]
    fn dots_in_a_listed_path_are_resolved_before_it_is_placed() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Games");
        let inside = root.join("sub").join("..").join("x.sav");
        let outside = root.join("..").join("other").join("y.sav");
        let merged = merge(&games(&root, &[]), &[inside, outside], None);
        assert_eq!(merged.folders[0].ignore_patterns, vec!["/x.sav"]);
    }

    #[test]
    fn every_list_in_the_folder_is_read_in_name_order() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(read_lists(&tmp.path().join("missing")), "");
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        std::fs::write(tmp.path().join("b.list"), format!("{}\n", b.display())).unwrap();
        std::fs::write(tmp.path().join("a.list"), format!("{}\n", a.display())).unwrap();
        std::fs::write(
            tmp.path().join("notes.txt"),
            format!("{}\n", tmp.path().display()),
        )
        .unwrap();
        let text = read_lists(tmp.path());
        assert_eq!(paths(&text), vec![a, b]);
    }
}
