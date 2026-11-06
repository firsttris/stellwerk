//! Updating the agent when the UI asks: the release of the Zentrale's version replaces this
//! binary, and the service manager starts the new one.
//!
//! The Zentrale only names a version. The file comes from the project's GitHub release over
//! HTTPS, the same place and checksums as `installer/install.sh`, so a compromised Zentrale can
//! ask for a genuine newer release but cannot hand a device a program of its own.

use std::path::{Path, PathBuf};

use anyhow::Context;
use sha2::{Digest, Sha256};

const RELEASES: &str = "https://github.com/firsttris/stellwerk/releases/download";

/// Whether this agent can update itself. Windows locks the running files and installs two of
/// them; there the installer stays the way.
pub const SUPPORTED: bool = cfg!(unix);

/// `x.y.z` as numbers; anything else is not a version this agent installs, which also keeps
/// what the Zentrale sends out of the download address.
fn parse(version: &str) -> Option<[u64; 3]> {
    let mut parts = version.split('.').map(|part| {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse().ok())
            .flatten()
    });
    let version = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(version)
}

/// Only forward: an older release may have flaws fixed since.
fn check_newer(wanted: &str, current: &str) -> anyhow::Result<()> {
    let parsed = parse(wanted).with_context(|| format!("not a version: {wanted:?}"))?;
    let installed = parse(current).with_context(|| format!("not a version: {current:?}"))?;
    anyhow::ensure!(parsed > installed, "{wanted} is not newer than {current}");
    Ok(())
}

/// The release file for this platform, named as in `installer/install.sh`.
fn asset(os: &str, arch: &str) -> anyhow::Result<String> {
    anyhow::ensure!(matches!(os, "linux" | "macos"), "no self-update on {os}");
    anyhow::ensure!(
        matches!(arch, "x86_64" | "aarch64"),
        "no release for {arch}"
    );
    Ok(format!("stellwerk-agent-{os}-{arch}"))
}

/// The checksum of `name` in a `SHA256SUMS` file.
fn checksum<'a>(sums: &'a str, name: &str) -> anyhow::Result<&'a str> {
    sums.lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .find(|(_, file)| file.trim_start().trim_start_matches('*') == name)
        .map(|(sum, _)| sum)
        .with_context(|| format!("{name} is not in SHA256SUMS"))
}

fn verify(binary: &[u8], expected: &str) -> anyhow::Result<()> {
    let actual = Sha256::digest(binary)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    anyhow::ensure!(
        actual.eq_ignore_ascii_case(expected),
        "the checksum of the download does not match"
    );
    Ok(())
}

/// The running binary. After an earlier update replaced the file, Linux reports the path with
/// " (deleted)" appended; the file there now is the one to replace.
fn current_exe() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(
        match exe.to_str().and_then(|s| s.strip_suffix(" (deleted)")) {
            Some(path) => PathBuf::from(path),
            None => exe,
        },
    )
}

/// Puts `binary` in place of `exe` if it runs and reports `version`; anything else changes
/// nothing. The rename is atomic, and the running process keeps its old file until it exits.
async fn replace(exe: &Path, binary: &[u8], version: &str) -> anyhow::Result<()> {
    let new = exe.with_extension("new");
    tokio::fs::write(&new, binary).await?;
    let checked = check(&new, version).await;
    if checked.is_err() {
        let _ = tokio::fs::remove_file(&new).await;
    }
    checked?;
    tokio::fs::rename(&new, exe).await?;
    Ok(())
}

async fn check(new: &Path, version: &str) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(new, std::fs::Permissions::from_mode(0o755)).await?;
    }
    let output = tokio::process::Command::new(new)
        .arg("--version")
        .output()
        .await
        .context("the download does not run")?;
    let text = String::from_utf8_lossy(&output.stdout);
    anyhow::ensure!(
        output.status.success() && text.trim() == format!("stellwerk-agent {version}"),
        "the download is not agent {version}: {}",
        text.trim()
    );
    Ok(())
}

async fn fetch(client: &reqwest::Client, url: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .with_context(|| format!("downloading {url}"))?
        .bytes()
        .await
        .with_context(|| format!("downloading {url}"))?;
    Ok(bytes.to_vec())
}

/// Downloads `version` from `releases`, checks it and replaces `exe`.
async fn install(releases: &str, exe: &Path, version: &str, asset: &str) -> anyhow::Result<()> {
    let base = format!("{}/v{version}", releases.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let sums = fetch(&client, &format!("{base}/SHA256SUMS")).await?;
    let sums = String::from_utf8(sums).context("SHA256SUMS is not text")?;
    let expected = checksum(&sums, asset)?;
    let binary = fetch(&client, &format!("{base}/{asset}")).await?;
    verify(&binary, expected)?;
    replace(exe, &binary, version).await
}

/// Replaces this agent with `version`; the caller then exits so that the service restarts it.
pub async fn to(version: &str) -> anyhow::Result<()> {
    anyhow::ensure!(SUPPORTED, "no self-update on this platform");
    check_newer(version, env!("CARGO_PKG_VERSION"))?;
    let asset = asset(std::env::consts::OS, std::env::consts::ARCH)?;
    install(RELEASES, &current_exe()?, version, &asset).await
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
