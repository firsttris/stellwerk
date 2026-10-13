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
        parse_conflict_name(".syncthing.config.sync-conflict-20261012-103112-ABCDEFG.ts.tmp"),
        None,
        "still being pulled"
    );
    assert_eq!(
        parse_conflict_name("~syncthing~config.sync-conflict-20261012-103112-ABCDEFG.ts.tmp"),
        None
    );
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

#[cfg(unix)]
#[test]
fn symbolic_links_are_not_followed() {
    use std::os::unix::fs::symlink;
    let (dir, copy) = folder_with_conflict();
    let secret = tempfile::NamedTempFile::new().unwrap();
    let planted = "src/passwd.sync-conflict-20261012-103112-ABCDEFG";
    symlink(secret.path(), dir.path().join(planted)).unwrap();
    std::fs::remove_file(dir.path().join("src/config.ts")).unwrap();
    symlink(secret.path(), dir.path().join("src/config.ts")).unwrap();

    let found = scan_conflicts(dir.path());
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].path.as_str(), found[0].original_size),
        (copy.as_str(), None)
    );
    assert!(resolve(dir.path(), planted, Keep::Copy, "x", 0).is_err());
    assert!(dir.path().join(planted).is_symlink());
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
    let archived = dir
        .path()
        .join(".stversions/src/config.sync-conflict-20261012-103112-ABCDEFG~19700101-000000.ts");
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

    let second = "src/config.sync-conflict-20261012-103113-HIJKLMN.ts";
    std::fs::write(dir.path().join(second), "third").unwrap();
    resolve(dir.path(), second, Keep::Copy, "laptop", 0).unwrap();
    let versions = dir.path().join(".stversions/src");
    let read = |name: &str| std::fs::read_to_string(versions.join(name)).unwrap();
    assert_eq!(read("config~19700101-000000.ts"), "mine", "not overwritten");
    assert_eq!(read("config~19700101-000001.ts"), "theirs");
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
