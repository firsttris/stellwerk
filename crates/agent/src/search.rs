//! Finding a directory by name on this device, for scripts and assistants ("where is Game of
//! Thrones?"). Like the folder picker, only names and paths of directories leave the device.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stellwerk_protocol::messages::{Entry, Matches};

use crate::browse::{resolve, text};

/// How far a search may go; whichever limit comes first ends it, marked as truncated.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Levels below the start directory.
    pub depth: usize,
    pub matches: usize,
    pub time: Duration,
}

/// Deep enough for `/mnt/nas/Media/Serien/Game of Thrones`, quick enough for the Zentrale's
/// answer timeout (`crates/zentrale/src/browse.rs`).
pub const LIMITS: Limits = Limits {
    depth: 8,
    matches: 50,
    time: Duration::from_secs(10),
};

/// The words of a name or query: lower case, split at everything but letters and digits, so
/// "Game.of.Thrones" and "game_of_thrones" read alike.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whether `name` holds every word of the query, each as a prefix of one of its words in the
/// same order: "thrones" finds "Game of Thrones S01", "got" does not find "Forgotten".
pub fn matches(name: &str, query: &[String]) -> bool {
    let name = words(name);
    let mut rest = name.iter();
    query
        .iter()
        .all(|wanted| rest.any(|word| word.starts_with(wanted.as_str())))
}

/// The directories below `requested` (absolute, `~/…`, or empty for `home`) whose name matches
/// `query`, breadth first. Hidden directories and symbolic links are skipped, so a link loop
/// cannot keep it busy; below a match it does not look further.
pub fn search(
    requested: &str,
    query: &str,
    home: Option<&Path>,
    limits: Limits,
) -> Result<Matches, String> {
    let query = words(query);
    if query.is_empty() {
        return Err("the search needs a name".into());
    }
    let root = resolve(requested, home)?;
    if !root.is_dir() {
        return Err(format!("{}: not a directory", root.display()));
    }
    let deadline = Instant::now() + limits.time;
    let mut queue = VecDeque::from([(root.clone(), 0)]);
    let mut dirs = Vec::new();
    let mut truncated = false;
    while let Some((dir, depth)) = queue.pop_front() {
        if Instant::now() >= deadline {
            truncated = true;
            break;
        }
        for (name, path) in subdirectories(&dir) {
            if matches(&name, &query) {
                if dirs.len() == limits.matches {
                    truncated = true;
                    queue.clear();
                    break;
                }
                dirs.push(Entry {
                    name,
                    path: text(&path),
                });
            } else if depth + 1 < limits.depth {
                queue.push_back((path, depth + 1));
            }
        }
    }
    Ok(Matches {
        root: text(&root),
        dirs,
        truncated,
    })
}

/// The visible, real subdirectories, sorted by name; an unreadable directory has none.
fn subdirectories(dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (!name.starts_with('.')).then(|| (name, entry.path()))
        })
        .collect();
    found.sort_by_key(|(name, _)| name.to_lowercase());
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(text: &str) -> Vec<String> {
        words(text)
    }

    #[test]
    fn names_match_by_word_prefixes_in_order() {
        let got = query("game of thrones");
        assert!(matches("Game of Thrones", &got));
        assert!(matches("Game.of.Thrones.S01.1080p", &got));
        assert!(matches("game_of_thrones", &got));
        assert!(!matches("Thrones of Game", &got));
        assert!(!matches("Game of Life", &got));
        assert!(matches("Baldur's Gate 3", &query("baldur gate")));
        assert!(!matches("Forgotten", &query("got")));
        assert!(matches("Fotos 2026", &query("2026")));
    }

    fn tree(paths: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for path in paths {
            std::fs::create_dir_all(dir.path().join(path)).unwrap();
        }
        dir
    }

    fn names(found: &Matches) -> Vec<&str> {
        found.dirs.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn finds_directories_shallow_first_and_not_below_a_match() {
        let dir = tree(&[
            "Media/Serien/Game of Thrones/Game of Thrones Extras",
            "Backup/2020/Game.of.Thrones",
            ".hidden/Game of Thrones",
            "Games/Thrones",
        ]);
        std::fs::write(dir.path().join("Game of Thrones.txt"), "a file").unwrap();
        let found = search(&text(dir.path()), "Game of Thrones", None, LIMITS).unwrap();
        // Both three levels down; Backup comes before Media.
        assert_eq!(names(&found), ["Game.of.Thrones", "Game of Thrones"]);
        assert_eq!(
            found.dirs[1].path,
            text(&dir.path().join("Media/Serien/Game of Thrones"))
        );
        assert_eq!(found.root, text(dir.path()));
        assert!(!found.truncated);
    }

    #[test]
    fn limits_end_the_search_and_say_so() {
        let dir = tree(&["a/b/c/Photos", "Photos 1", "Photos 2", "Photos 3"]);
        let few = Limits {
            matches: 2,
            ..LIMITS
        };
        let found = search(&text(dir.path()), "photos", None, few).unwrap();
        assert_eq!(names(&found), ["Photos 1", "Photos 2"]);
        assert!(found.truncated);

        let shallow = Limits { depth: 2, ..LIMITS };
        let found = search(&text(dir.path()), "photos", None, shallow).unwrap();
        assert_eq!(found.dirs.len(), 3);
        assert!(!found.truncated);

        let no_time = Limits {
            time: Duration::ZERO,
            ..LIMITS
        };
        assert!(
            search(&text(dir.path()), "photos", None, no_time)
                .unwrap()
                .truncated
        );
    }

    #[test]
    fn starts_at_home_and_refuses_an_empty_query() {
        let home = tree(&["Games/Stardew Valley"]);
        let found = search("", "stardew", Some(home.path()), LIMITS).unwrap();
        assert_eq!(names(&found), ["Stardew Valley"]);
        assert!(search("", " .. ", Some(home.path()), LIMITS).is_err());
        assert!(search("relative", "x", Some(home.path()), LIMITS).is_err());
        let file = home.path().join("file");
        std::fs::write(&file, "").unwrap();
        assert!(search(&text(&file), "x", None, LIMITS).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_not_followed() {
        let dir = tree(&["real/Music"]);
        std::os::unix::fs::symlink(dir.path(), dir.path().join("real/loop")).unwrap();
        let found = search(&text(dir.path()), "music", None, LIMITS).unwrap();
        assert_eq!(names(&found), ["Music"]);
    }
}
