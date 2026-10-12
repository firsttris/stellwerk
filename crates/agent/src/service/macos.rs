//! launchd agent in `~/Library/LaunchAgents`, loaded into the user's GUI domain.

use std::path::{Path, PathBuf};

use super::definitions::{LAUNCHD_LABEL, launchd_plist};
use super::{Service, command};

fn plist_path() -> anyhow::Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home directory"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

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
    let _ = command(
        "launchctl",
        &["bootout", &format!("{domain}/{LAUNCHD_LABEL}")],
    );
    std::fs::write(&path, launchd_plist(service))?;
    command(
        "launchctl",
        &["bootstrap", &domain, &path.to_string_lossy()],
    )?;
    Ok(format!("Installed {} and started it.", path.display()))
}

/// launchd stops the agent with SIGTERM, which it handles like Ctrl-C.
pub fn uninstall(_data_dir: &Path) -> anyhow::Result<()> {
    let domain = domain()?;
    let _ = command(
        "launchctl",
        &["bootout", &format!("{domain}/{LAUNCHD_LABEL}")],
    );
    let path = plist_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}
