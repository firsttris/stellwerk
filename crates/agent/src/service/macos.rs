//! launchd agent in `~/Library/LaunchAgents`, loaded into the user's GUI domain.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::definitions::{LAUNCHD_LABEL, launchd_plist};
use super::{Service, command};

fn plist_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

/// How long `bootout` may take: launchd stops the agent with SIGTERM, and the agent first shuts
/// its Syncthing down.
const UNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// `gui/<uid>`, the domain of the logged-in user.
fn domain() -> anyhow::Result<String> {
    let uid = command("id", &["-u"])?;
    Ok(format!("gui/{}", uid.trim()))
}

pub fn install(service: &Service) -> anyhow::Result<String> {
    let path = plist_path()?;
    std::fs::create_dir_all(path.parent().expect("the plist path has a parent"))?;
    std::fs::create_dir_all(&service.data_dir)?;
    let domain = domain()?;
    // A loaded older definition would keep running; replace it.
    unload(&domain);
    std::fs::write(&path, launchd_plist(service))?;
    command(
        "launchctl",
        &["bootstrap", &domain, &path.to_string_lossy()],
    )?;
    Ok(format!("Installed {} and started it.", path.display()))
}

/// launchd stops the agent with SIGTERM, which it handles like Ctrl-C.
pub fn uninstall(_data_dir: &Path) -> anyhow::Result<()> {
    unload(&domain()?);
    let path = plist_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Unloads the agent if it is loaded and waits until launchd has let go of it: `bootout` can
/// return before the job is gone, and a `bootstrap` in that window fails with
/// "Bootstrap failed: 5: Input/output error". An agent that was not loaded passes at once.
fn unload(domain: &str) {
    let target = format!("{domain}/{LAUNCHD_LABEL}");
    let _ = command("launchctl", &["bootout", &target]);
    let deadline = Instant::now() + UNLOAD_TIMEOUT;
    while command("launchctl", &["print", &target]).is_ok() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
}
