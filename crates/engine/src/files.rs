//! Conflict copies and versions inside a folder on disk: finding, naming, resolving.
//!
//! Syncthing names a conflict copy `name.sync-conflict-YYYYMMDD-HHMMSS-<short ID>.ext`
//! (lib/model/folder_sendrecv.go, `conflictName`) and an archived version
//! `.stversions/<dir>/name~YYYYMMDD-HHMMSS.ext` (lib/versioner/util.go, `TagFilename`).

use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result, io_at};

mod tag;
pub use tag::{format_tag, parse_tag, tag_filename};

const CONFLICT_MARK: &str = ".sync-conflict-";
pub const VERSIONS_DIR: &str = ".stversions";
/// Syncthing's own marker and temporary files, never conflicts.
const SKIP: [&str; 2] = [VERSIONS_DIR, ".stfolder"];

/// A conflict copy found in a folder; paths relative to the folder, with `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    pub original: String,
    /// When Syncthing made the copy, seconds since the epoch (UTC).
    pub at: i64,
    /// Short ID of the device whose version the copy holds.
    pub modified_by: String,
    pub size: u64,
    pub modified: i64,
    /// `None` when the original is gone.
    pub original_size: Option<u64>,
    pub original_modified: Option<i64>,
}

/// What to keep when resolving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// The file under the original name; the copy goes to the versions.
    Original,
    /// The copy, under the original name; the old original goes to the versions.
    Copy,
    /// Both: the copy is renamed to `name.<label>.ext` next to the original.
    Both,
}

/// The parts of a conflict copy's name: (original name, time, short ID).
pub fn parse_conflict_name(name: &str) -> Option<(String, i64, String)> {
    let start = name.find(CONFLICT_MARK)?;
    let (stem, rest) = (&name[..start], &name[start + CONFLICT_MARK.len()..]);
    let (date, rest) = rest.split_at_checked(8)?;
    let rest = rest.strip_prefix('-')?;
    let (time, rest) = rest.split_at_checked(6)?;
    let rest = rest.strip_prefix('-')?;
    let (by, ext) = match rest.find('.') {
        Some(dot) => (&rest[..dot], &rest[dot..]),
        None => (rest, ""),
    };
    if by.is_empty() || !by.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some((
        format!("{stem}{ext}"),
        parse_tag(&format!("{date}-{time}"))?,
        by.to_string(),
    ))
}

/// The original of a conflict copy at `copy` (a path with `/`), in the same directory:
/// (original path, time, short ID).
pub fn conflict_original(copy: &str) -> Option<(String, i64, String)> {
    let (dir, name) = split_path(copy);
    let (original, at, by) = parse_conflict_name(name)?;
    Some((format!("{dir}{original}"), at, by))
}

/// `dir/name` → (`dir/`, `name`).
fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(slash) => (&path[..=slash], &path[slash + 1..]),
        None => ("", path),
    }
}

/// `name.ext` → (`name`, `.ext`); a leading dot is not an extension.
fn split_ext(file: &str) -> (&str, &str) {
    match file.rfind('.') {
        Some(dot) if dot > 0 => (&file[..dot], &file[dot..]),
        _ => (file, ""),
    }
}

/// A relative path from a client, checked: no `..`, no root, no Syncthing internals.
pub fn safe_relative(path: &str) -> Result<PathBuf> {
    let relative = Path::new(path);
    let clean = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
        && !path.is_empty()
        && !SKIP
            .iter()
            .any(|skip| path == *skip || path.starts_with(&format!("{skip}/")));
    if clean {
        Ok(relative.to_path_buf())
    } else {
        Err(Error::Archive(format!(
            "not a path inside the folder: {path}"
        )))
    }
}

/// When the file was last modified, seconds since the epoch; 0 when unknown.
pub fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified().map_or(0, secs)
}

/// Seconds since the epoch; 0 before it.
pub fn secs(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Every conflict copy in the folder, sorted by path.
pub fn scan_conflicts(root: &Path) -> Vec<Conflict> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(root.join(&dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = dir.join(&name);
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                if !(dir.as_os_str().is_empty() && SKIP.contains(&name.as_str())) {
                    pending.push(relative);
                }
                continue;
            }
            let Some((original, at, modified_by)) = parse_conflict_name(&name) else {
                continue;
            };
            let original = dir.join(original);
            let original_meta = std::fs::metadata(root.join(&original)).ok();
            found.push(Conflict {
                path: slashes(&relative),
                original: slashes(&original),
                at,
                modified_by,
                size: meta.len(),
                modified: modified_secs(&meta),
                original_size: original_meta.as_ref().map(std::fs::Metadata::len),
                original_modified: original_meta.as_ref().map(modified_secs),
            });
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

fn slashes(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Moves a file into `.stversions` under Syncthing's name for a version made now.
pub fn archive(root: &Path, path: &str, now: i64) -> Result<String> {
    let source = root.join(safe_relative(path)?);
    let tagged = tag_filename(path, &format_tag(now));
    let target = root.join(VERSIONS_DIR).join(&tagged);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(io_at(parent))?;
    }
    std::fs::rename(&source, &target).map_err(io_at(&source))?;
    Ok(format!("{VERSIONS_DIR}/{tagged}"))
}

/// Resolves a conflict copy in place; Syncthing then sends the result to every device.
/// `label` names the copy for [`Keep::Both`] (usually the device it came from).
pub fn resolve(root: &Path, copy: &str, keep: Keep, label: &str, now: i64) -> Result<()> {
    let (original, _, _) = conflict_original(copy)
        .ok_or_else(|| Error::Archive(format!("not a conflict copy: {copy}")))?;
    let copy_path = root.join(safe_relative(copy)?);
    if !copy_path.is_file() {
        return Err(Error::Archive(format!("conflict copy is gone: {copy}")));
    }
    match keep {
        Keep::Original => archive(root, copy, now).map(drop),
        Keep::Copy => {
            if root.join(&original).exists() {
                archive(root, &original, now)?;
            }
            let target = root.join(safe_relative(&original)?);
            std::fs::rename(&copy_path, &target).map_err(io_at(&copy_path))
        }
        Keep::Both => {
            let target = root.join(safe_relative(&free_name(root, &original, label))?);
            std::fs::rename(&copy_path, &target).map_err(io_at(&copy_path))
        }
    }
}

/// `dir/name.label.ext`, or with a counter when that exists too.
fn free_name(root: &Path, original: &str, label: &str) -> String {
    let label: String = label
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let (dir, file) = split_path(original);
    let (stem, ext) = split_ext(file);
    (1..)
        .map(|n| match n {
            1 => format!("{dir}{stem}.{label}{ext}"),
            n => format!("{dir}{stem}.{label}-{n}{ext}"),
        })
        .find(|candidate| !root.join(candidate).exists())
        .expect("some name is free")
}

/// Seconds since the epoch, now.
pub fn now() -> i64 {
    secs(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_names_are_parsed_like_syncthing_writes_them() {
        let (original, at, by) =
            parse_conflict_name("config.sync-conflict-20261012-103112-ABCDEFG.ts").unwrap();
        assert_eq!(original, "config.ts");
        assert_eq!(format_tag(at), "20261012-103112");
        assert_eq!(by, "ABCDEFG");
        let (original, _, _) =
            parse_conflict_name("Makefile.sync-conflict-20261012-103112-ABCDEFG").unwrap();
        assert_eq!(original, "Makefile");
        let (original, _, _) =
            parse_conflict_name("archive.tar.sync-conflict-20261012-103112-ABCDEFG.gz").unwrap();
        assert_eq!(original, "archive.tar.gz");
        assert_eq!(parse_conflict_name("config.ts"), None);
        assert_eq!(
            parse_conflict_name("x.sync-conflict-2026101-103112-A.ts"),
            None
        );
    }

    #[test]
    fn the_original_of_a_copy_is_in_the_same_directory() {
        let (original, _, by) =
            conflict_original("a/b/config.sync-conflict-20261012-103112-ABCDEFG.ts").unwrap();
        assert_eq!(
            (original.as_str(), by.as_str()),
            ("a/b/config.ts", "ABCDEFG")
        );
        let (original, _, _) =
            conflict_original("config.sync-conflict-20261012-103112-ABCDEFG.ts").unwrap();
        assert_eq!(original, "config.ts");
        assert_eq!(conflict_original("a/config.ts"), None);
    }

    #[test]
    fn client_paths_stay_inside_the_folder() {
        assert!(safe_relative("a/b.txt").is_ok());
        assert!(safe_relative("../etc/passwd").is_err());
        assert!(safe_relative("/etc/passwd").is_err());
        assert!(safe_relative("a/../../b").is_err());
        assert!(safe_relative(".stversions/x~1.txt").is_err());
        assert!(safe_relative("").is_err());
    }

    fn folder_with_conflict() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/config.ts"), "mine").unwrap();
        let copy = "src/config.sync-conflict-20261012-103112-ABCDEFG.ts".to_string();
        std::fs::write(dir.path().join(&copy), "theirs").unwrap();
        std::fs::create_dir_all(dir.path().join(".stversions")).unwrap();
        std::fs::write(
            dir.path()
                .join(".stversions/old.sync-conflict-20261012-103112-ABCDEFG.ts"),
            "",
        )
        .unwrap();
        (dir, copy)
    }

    #[test]
    fn conflicts_are_found_but_not_inside_stversions() {
        let (dir, copy) = folder_with_conflict();
        let found = scan_conflicts(dir.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].path, copy);
        assert_eq!(found[0].original, "src/config.ts");
        assert_eq!((found[0].size, found[0].original_size), (6, Some(4)));
    }

    #[test]
    fn keeping_the_original_archives_the_copy() {
        let (dir, copy) = folder_with_conflict();
        resolve(dir.path(), &copy, Keep::Original, "laptop", 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/config.ts")).unwrap(),
            "mine"
        );
        assert!(!dir.path().join(&copy).exists());
        let archived = dir.path().join(
            ".stversions/src/config.sync-conflict-20261012-103112-ABCDEFG~19700101-000000.ts",
        );
        assert_eq!(std::fs::read_to_string(archived).unwrap(), "theirs");
        assert!(scan_conflicts(dir.path()).is_empty());
    }

    #[test]
    fn keeping_the_copy_archives_the_original() {
        let (dir, copy) = folder_with_conflict();
        resolve(dir.path(), &copy, Keep::Copy, "laptop", 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/config.ts")).unwrap(),
            "theirs"
        );
        let archived = dir.path().join(".stversions/src/config~19700101-000000.ts");
        assert_eq!(std::fs::read_to_string(archived).unwrap(), "mine");
    }

    #[test]
    fn keeping_both_names_the_copy_after_its_device() {
        let (dir, copy) = folder_with_conflict();
        resolve(dir.path(), &copy, Keep::Both, "büro pc", 0).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("src/config.ts")).unwrap(),
            "mine"
        );
        let renamed = dir.path().join("src/config.büro-pc.ts");
        assert_eq!(std::fs::read_to_string(renamed).unwrap(), "theirs");
        assert!(
            resolve(dir.path(), &copy, Keep::Both, "x", 0).is_err(),
            "already resolved"
        );
    }
}
