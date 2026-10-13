//! `installer/install.sh`, the script behind the pairing command, run for real: it downloads
//! this build of the agent from a release directory, checks it, pairs with `--ca-sha256` and
//! installs the service (systemctl and launchctl are stand-ins that only log).
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use stellwerk_protocol::ca::{fingerprint, first_certificate};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const ASSETS: [&str; 4] = [
    "stellwerk-agent-linux-x86_64",
    "stellwerk-agent-linux-aarch64",
    "stellwerk-agent-macos-x86_64",
    "stellwerk-agent-macos-aarch64",
];

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../installer/install.sh")
}

/// A release directory as GitHub serves it: the agent under every asset name and SHA256SUMS.
fn release(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let agent = std::fs::read(env!("CARGO_BIN_EXE_stellwerk-agent")).unwrap();
    let sum: String = Sha256::digest(&agent)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut sums = String::new();
    for asset in ASSETS {
        std::fs::write(dir.join(asset), &agent).unwrap();
        sums.push_str(&format!("{sum}  {asset}\n"));
    }
    std::fs::write(dir.join("SHA256SUMS"), sums).unwrap();
}

/// systemctl and launchctl that only log; `launchctl print` says the job is gone.
fn stubs(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    for (name, body) in [
        (
            "systemctl",
            "echo \"systemctl $*\" >> \"$HOME/service.log\"",
        ),
        (
            "launchctl",
            "echo \"launchctl $*\" >> \"$HOME/service.log\"\n[ \"$1\" != print ]",
        ),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// An HTTP server that answers every request with the CA, as the Zentrale's /ca.crt does.
async fn ca_server(pem: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut request = [0u8; 2048];
            let _ = socket.read(&mut request).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{pem}",
                pem.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn test_ca() -> String {
    let key = rcgen::KeyPair::generate().unwrap();
    rcgen::CertificateParams::new(Vec::<String>::new())
        .unwrap()
        .self_signed(&key)
        .unwrap()
        .pem()
}

/// Runs the script as the dialog's command does, with only HOME and PATH from outside.
async fn run_script(tmp: &Path, args: &[&str]) -> std::process::Output {
    let home = tmp.join("home");
    std::fs::create_dir_all(&home).unwrap();
    tokio::process::Command::new("sh")
        .arg(script())
        .args(args)
        .env_clear()
        .env("HOME", &home)
        .env(
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                tmp.join("stubs").display()
            ),
        )
        .env(
            "STELLWERK_RELEASES",
            format!("file://{}", tmp.join("release").display()),
        )
        .output()
        .await
        .unwrap()
}

fn data_dir(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/stellwerk-agent")
    } else {
        home.join(".local/share/stellwerk-agent")
    }
}

#[tokio::test]
async fn the_install_command_downloads_checks_pairs_and_starts_the_agent() {
    let tmp = tempfile::tempdir().unwrap();
    release(&tmp.path().join("release"));
    stubs(&tmp.path().join("stubs"));
    let pem = test_ca();
    let sha = fingerprint(&first_certificate(&pem).unwrap());
    let server = ca_server(pem.clone()).await;

    let output = run_script(
        tmp.path(),
        &[&server, "K7Q2-MX9P-41TZ", "--ca-sha256", &sha],
    )
    .await;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let home = tmp.path().join("home");
    let agent = home.join(".local/bin/stellwerk-agent");
    assert_ne!(
        std::fs::metadata(&agent).unwrap().permissions().mode() & 0o111,
        0
    );
    let config = stellwerk_agent::config::load(&data_dir(&home)).unwrap();
    assert_eq!(config.server, server);
    assert_eq!(config.enrollment_code.as_deref(), Some("K7Q2-MX9P-41TZ"));
    assert_eq!(config.ca.as_deref(), Some(pem.as_str()));
    let log = std::fs::read_to_string(home.join("service.log")).unwrap();
    let started = if cfg!(target_os = "macos") {
        "launchctl bootstrap"
    } else {
        "systemctl --user restart"
    };
    assert!(log.contains(started), "{log}");
}

#[tokio::test]
async fn a_wrong_ca_fingerprint_or_a_changed_download_stops_the_install() {
    let tmp = tempfile::tempdir().unwrap();
    release(&tmp.path().join("release"));
    stubs(&tmp.path().join("stubs"));
    let server = ca_server(test_ca()).await;
    let home = tmp.path().join("home");

    let wrong = fingerprint(b"another CA");
    let output = run_script(tmp.path(), &[&server, "CODE", "--ca-sha256", &wrong]).await;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fingerprint"), "{stderr}");
    assert!(!data_dir(&home).join("agent.json").exists());

    for asset in ASSETS {
        std::fs::write(tmp.path().join("release").join(asset), b"tampered").unwrap();
    }
    let output = run_script(tmp.path(), &[&server, "CODE"]).await;
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("checksum"), "{stderr}");
}
