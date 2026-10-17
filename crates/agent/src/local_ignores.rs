//! Exclusions other programs on this device ask for: files Stellwerk must not sync because
//! another program moves them itself (Spielstand, for game saves it synchronises).
//!
//! A program writes one list into `<data dir>/ignore.d/<name>.list`: one absolute path per
//! line, `#` starts a comment. On every desired state, and when a list changes, each path that
//! lies inside a folder becomes a rooted pattern for exactly that path, added to the folder's
//! patterns from the Zentrale. The lists stay on the device: other devices get the same
//! patterns from their own lists, and the Zentrale's patterns are untouched.

use std::path::{Path, PathBuf};

use stellwerk_engine::ignore::anchored_pattern;
use stellwerk_protocol::DesiredState;

pub const DIR: &str = "ignore.d";

pub fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join(DIR)
}

/// The text of every `*.list` file, sorted by name: what is compared to see a change.
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
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The absolute paths in the lists' text; comments, blank and relative lines are skipped.
pub fn paths(lists: &str) -> Vec<PathBuf> {
    lists
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
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

/// `state` with the excluded paths added to the folders they lie in.
pub fn merge(state: &DesiredState, excluded: &[PathBuf], home: Option<&Path>) -> DesiredState {
    let mut merged = state.clone();
    for folder in &mut merged.folders {
        let Some(root) = folder_root(&folder.path, home) else {
            continue;
        };
        for path in excluded {
            let Some(pattern) = path
                .strip_prefix(&root)
                .ok()
                .and_then(Path::to_str)
                .and_then(anchored_pattern)
            else {
                continue;
            };
            if !folder.ignore_patterns.contains(&pattern) {
                folder.ignore_patterns.push(pattern);
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let state = DesiredState {
            revision: 3,
            peers: vec![],
            folders: vec![folder("games", "~/Games"), folder("photos", "/srv/photos")],
        };
        let excluded = paths(
            "# Written by Spielstand\n\
             /home/t/Games/Hades/prefix/drive_c/users/steamuser/Save [1].sav\n\
             \n\
             relative/ignored\n\
             /elsewhere/x.sav\n\
             /home/t/Games/Hades/prefix/drive_c/users/steamuser/Save [1].sav\n",
        );
        let merged = merge(&state, &excluded, Some(Path::new("/home/t")));
        assert_eq!(
            merged.folders[0].ignore_patterns,
            vec![
                "*.tmp".to_string(),
                r"/Hades/prefix/drive_c/users/steamuser/Save \[1\].sav".to_string()
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

    #[test]
    fn every_list_in_the_folder_is_read_in_name_order() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(read_lists(&tmp.path().join("missing")), "");
        std::fs::write(tmp.path().join("b.list"), "/b\n").unwrap();
        std::fs::write(tmp.path().join("a.list"), "/a\n").unwrap();
        std::fs::write(tmp.path().join("notes.txt"), "/not-a-list\n").unwrap();
        let text = read_lists(tmp.path());
        assert_eq!(paths(&text), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }
}
