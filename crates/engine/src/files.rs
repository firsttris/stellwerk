//! Conflict copies and versions inside a folder on disk: finding, naming, resolving.
//!
//! Syncthing names a conflict copy `name.sync-conflict-YYYYMMDD-HHMMSS-<short ID>.ext`
//! (lib/model/folder_sendrecv.go, `conflictName`) and an archived version
//! `.stversions/<dir>/name~YYYYMMDD-HHMMSS.ext` (lib/versioner/util.go, `TagFilename`).

use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result, io_at};

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

/// `YYYYMMDD-HHMMSS` as seconds since the epoch, read as UTC.
pub fn parse_tag(tag: &str) -> Option<i64> {
    let digits = |s: &str| {
        s.chars()
            .all(|c| c.is_ascii_digit())
            .then(|| s.parse::<i64>().ok())?
    };
    if tag.len() != 15 || tag.as_bytes()[8] != b'-' {
        return None;
    }
    let (year, month, day) = (
        digits(&tag[0..4])?,
        digits(&tag[4..6])?,
        digits(&tag[6..8])?,
    );
    let (hour, minute, second) = (
        digits(&tag[9..11])?,
        digits(&tag[11..13])?,
        digits(&tag[13..15])?,
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Seconds since the epoch as `YYYYMMDD-HHMMSS` (UTC), Syncthing's version tag.
pub fn format_tag(seconds: i64) -> String {
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

// Howard Hinnant's algorithms for the proleptic Gregorian calendar.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `dir/name.ext` → `dir/name~tag.ext`, as Syncthing tags versions.
pub fn tag_filename(path: &str, tag: &str) -> String {
    let (dir, file) = match path.rfind('/') {
        Some(slash) => (&path[..=slash], &path[slash + 1..]),
        None => ("", path),
    };
    let (stem, ext) = split_ext(file);
    format!("{dir}{stem}~{tag}{ext}")
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

fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
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
    let name = copy.rsplit('/').next().unwrap_or(copy);
    let (original_name, _, _) = parse_conflict_name(name)
        .ok_or_else(|| Error::Archive(format!("not a conflict copy: {copy}")))?;
    let dir = copy.strip_suffix(name).unwrap_or("");
    let original = format!("{dir}{original_name}");
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
    let (dir, file) = match original.rfind('/') {
        Some(slash) => (&original[..=slash], &original[slash + 1..]),
        None => ("", original),
    };
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
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
    fn tags_round_trip_and_match_known_dates() {
        assert_eq!(parse_tag("19700101-000000"), Some(0));
        assert_eq!(parse_tag("20000301-000000"), Some(951_868_800));
        for seconds in [0, 951_868_800, 1_791_798_071] {
            assert_eq!(parse_tag(&format_tag(seconds)), Some(seconds));
        }
        assert_eq!(parse_tag("20261312-000000"), None);
        assert_eq!(
            tag_filename("a/b/config.ts", "20261012-103112"),
            "a/b/config~20261012-103112.ts"
        );
        assert_eq!(
            tag_filename(".bashrc", "20261012-103112"),
            ".bashrc~20261012-103112"
        );
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
