use super::*;

const SUMS: &str = "aaaa  stellwerk-agent-linux-aarch64\nbbbb *stellwerk-agent-linux-x86_64\n";

#[test]
fn only_a_newer_plain_version_is_installed() {
    assert!(check_newer("0.1.12", "0.1.11").is_ok());
    assert!(check_newer("0.2.0", "0.1.11").is_ok());
    assert!(check_newer("0.1.10", "0.1.11").is_err());
    assert!(check_newer("0.1.11", "0.1.11").is_err());
}

#[test]
fn nothing_but_a_version_reaches_the_download_address() {
    for version in [
        "../../evil/1.2.3",
        "1.2",
        "1.2.3.4",
        "1.2.3-rc.1",
        "",
        "1..3",
        "+1.2.3",
    ] {
        assert!(check_newer(version, "0.1.11").is_err(), "{version}");
    }
}

#[test]
fn assets_are_named_as_in_the_release() {
    assert_eq!(
        asset("linux", "x86_64").unwrap(),
        "stellwerk-agent-linux-x86_64"
    );
    assert_eq!(
        asset("macos", "aarch64").unwrap(),
        "stellwerk-agent-macos-aarch64"
    );
    assert!(asset("windows", "x86_64").is_err());
    assert!(asset("linux", "riscv64").is_err());
}

#[test]
fn finds_the_checksum_of_exactly_this_file() {
    assert_eq!(
        checksum(SUMS, "stellwerk-agent-linux-x86_64").unwrap(),
        "bbbb"
    );
    assert_eq!(
        checksum(SUMS, "stellwerk-agent-linux-aarch64").unwrap(),
        "aaaa"
    );
    assert!(checksum(SUMS, "stellwerk-agent").is_err());
}

#[test]
fn a_changed_download_does_not_pass() {
    let sum = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    assert!(verify(b"hello", sum).is_ok());
    assert!(verify(b"hello!", sum).is_err());
}

#[cfg(unix)]
fn script(dir: &Path, name: &str, body: &str) -> Vec<u8> {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::read(path).unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn a_binary_that_runs_as_the_wanted_agent_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("stellwerk-agent");
    std::fs::write(&exe, "old").unwrap();
    let new = script(dir.path(), "new", "echo stellwerk-agent 0.2.0");
    replace(&exe, &new, "0.2.0").await.unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), new);
    assert!(!exe.with_extension("new").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn anything_else_leaves_the_old_binary_alone() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("stellwerk-agent");
    std::fs::write(&exe, "old").unwrap();
    for body in [
        "echo stellwerk-agent 0.1.0",
        "echo something else",
        "exit 1",
    ] {
        let new = script(dir.path(), "new-file", body);
        assert!(replace(&exe, &new, "0.2.0").await.is_err(), "{body}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"old");
        assert!(!exe.with_extension("new").exists());
    }
}

/// Serves one release: `files` by path, 404 for the rest.
async fn release(files: Vec<(String, Vec<u8>)>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut request = [0; 1024];
            let read = socket.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..read]).into_owned();
            let path = request.split_whitespace().nth(1).unwrap_or_default();
            let file = files.iter().find(|(name, _)| name == path);
            let (status, body) =
                file.map_or(("404 Not Found", &[][..]), |(_, body)| ("200 OK", body));
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        }
    });
    format!("http://{address}")
}

#[cfg(unix)]
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(unix)]
#[tokio::test]
async fn a_release_is_downloaded_checked_and_installed() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("stellwerk-agent");
    std::fs::write(&exe, "old").unwrap();
    let name = "stellwerk-agent-linux-x86_64";
    let binary = script(dir.path(), "new", "echo stellwerk-agent 0.2.0");
    let sums = format!("{}  {name}\n", sha256(&binary));
    let base = release(vec![
        ("/v0.2.0/SHA256SUMS".into(), sums.into_bytes()),
        (format!("/v0.2.0/{name}"), binary.clone()),
    ])
    .await;
    install(&base, &exe, "0.2.0", name).await.unwrap();
    assert_eq!(std::fs::read(&exe).unwrap(), binary);
}

#[cfg(unix)]
#[tokio::test]
async fn a_download_that_does_not_match_its_checksum_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("stellwerk-agent");
    std::fs::write(&exe, "old").unwrap();
    let name = "stellwerk-agent-linux-x86_64";
    let genuine = script(dir.path(), "genuine", "echo stellwerk-agent 0.2.0");
    let tampered = script(
        dir.path(),
        "tampered",
        "echo stellwerk-agent 0.2.0 # changed",
    );
    let sums = format!("{}  {name}\n", sha256(&genuine));
    let base = release(vec![
        ("/v0.2.0/SHA256SUMS".into(), sums.into_bytes()),
        (format!("/v0.2.0/{name}"), tampered),
    ])
    .await;
    let error = install(&base, &exe, "0.2.0", name).await.unwrap_err();
    assert!(format!("{error:#}").contains("checksum"), "{error:#}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"old");
}

#[tokio::test]
async fn a_missing_release_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("stellwerk-agent");
    std::fs::write(&exe, "old").unwrap();
    let base = release(vec![]).await;
    assert!(install(&base, &exe, "0.2.0", "x").await.is_err());
    assert_eq!(std::fs::read(&exe).unwrap(), b"old");
}
