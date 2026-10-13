//! The install scripts behind the pairing dialog's one command: `/install.sh` (Linux, macOS)
//! and `/install.ps1` (Windows) download the agent of this Zentrale's version from the GitHub
//! release, check it against the release's SHA256SUMS, pair it and start it. They hold no
//! secret (server and code are arguments), so they are served without login.

use axum::http::header;
use axum::response::{IntoResponse, Response};

const SH: &str = include_str!("install/install.sh");
const PS1: &str = include_str!("install/install.ps1");

/// The agent release that matches this Zentrale: older and newer agents may not speak its
/// protocol.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A script template with the version filled in.
pub fn script(template: &str, version: &str) -> String {
    template.replace("{{VERSION}}", version)
}

pub async fn sh() -> Response {
    respond(SH)
}

pub async fn ps1() -> Response {
    respond(PS1)
}

fn respond(template: &str) -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        script(template, VERSION),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_scripts_name_the_zentrales_version_and_nothing_else() {
        for template in [SH, PS1] {
            let filled = script(template, "1.2.3");
            assert!(!filled.contains("{{"), "a placeholder is left");
            assert!(filled.contains("1.2.3"));
            assert!(filled.contains("releases/download/v$version"));
        }
    }

    #[test]
    fn the_scripts_stay_ascii_for_windows_powershell() {
        // Windows PowerShell 5.1 reads a script without a BOM in the legacy code page.
        assert!(SH.is_ascii() && PS1.is_ascii());
    }

    #[test]
    fn the_shell_script_parses() {
        let status = std::process::Command::new("sh")
            .args(["-n", "-c", &script(SH, "1.2.3")])
            .status();
        if let Ok(status) = status {
            assert!(status.success());
        }
    }
}
