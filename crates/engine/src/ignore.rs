//! Syncthing ignore patterns that match exactly one path, for exclusions other programs on a
//! device ask for (see `stellwerk_agent::local_ignores`).
//!
//! Syncthing reads a pattern line trimmed (`lib/ignore/ignore.go`, `loadParseIncludeFile`),
//! compiles it with gobwas/glob, and takes a backslash as the escape character, or `|` on
//! Windows, where the backslash separates paths (`defaultEscapeChar`; there it also turns
//! `\` into `/` and `|` back into a backslash before compiling). A leading `/` roots the
//! pattern at the folder.

/// Syncthing's default escape character on this platform.
pub const ESCAPE: char = if cfg!(windows) { '|' } else { '\\' };

/// The pattern for `relative` (a path inside the folder, `/`-separated): rooted, with every
/// character the glob reads specially escaped. `None` for a path Syncthing cannot match
/// exactly because it trims pattern lines (leading or trailing spaces), or for an empty one.
pub fn anchored_pattern(relative: &str) -> Option<String> {
    anchored_pattern_with(relative, ESCAPE)
}

/// [`anchored_pattern`] with the escape character of a given platform.
pub fn anchored_pattern_with(relative: &str, escape: char) -> Option<String> {
    if relative.is_empty() || relative != relative.trim() || relative.contains('\n') {
        return None;
    }
    let mut out = String::with_capacity(relative.len() + 8);
    out.push('/');
    for c in relative.chars() {
        if c == escape || matches!(c, '*' | '?' | '[' | ']' | '{' | '}') {
            out.push(escape);
        }
        out.push(c);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same table as Spielstand's `foreign::anchored_pattern` (Linux), which reads these
    /// patterns back from `.stignore` to see whether its saves are excluded.
    #[test]
    fn patterns_are_rooted_and_escaped() {
        let unix = |relative| anchored_pattern_with(relative, '\\');
        assert_eq!(
            unix("Games/Hades/prefix/drive_c/x.sav").as_deref(),
            Some("/Games/Hades/prefix/drive_c/x.sav")
        );
        assert_eq!(
            unix("Games/Save [1] {a,b} *?.dat").as_deref(),
            Some(r"/Games/Save \[1\] \{a,b\} \*\?.dat")
        );
        assert_eq!(unix(r"a\b").as_deref(), Some(r"/a\\b"));
        assert_eq!(anchored_pattern("trailing "), None);
        assert_eq!(anchored_pattern(""), None);
    }

    #[test]
    fn on_windows_the_pipe_escapes() {
        assert_eq!(
            anchored_pattern_with("Games/Save [1] {a,b}.dat", '|').as_deref(),
            Some("/Games/Save |[1|] |{a,b|}.dat")
        );
        assert_eq!(
            anchored_pattern("x [1]"),
            anchored_pattern_with("x [1]", ESCAPE),
            "this platform's escape character by default"
        );
    }
}
