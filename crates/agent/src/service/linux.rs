//! systemd user service (`systemctl --user`).

use std::path::{Path, PathBuf};

use super::definitions::{SYSTEMD_UNIT, systemd_unit};
use super::{Service, command};

fn unit_path() -> anyhow::Result<PathBuf> {
    let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
    Ok(config.join("systemd/user").join(SYSTEMD_UNIT))
}

pub fn install(service: &Service) -> anyhow::Result<String> {
    let path = unit_path()?;
    std::fs::create_dir_all(path.parent().expect("the unit path has a parent"))?;
    std::fs::write(&path, systemd_unit(service))?;
    command("systemctl", &["--user", "daemon-reload"])?;
    command("systemctl", &["--user", "enable", SYSTEMD_UNIT])?;
    // `restart` also starts a stopped unit; `enable --now` would keep an old binary running.
    command("systemctl", &["--user", "restart", SYSTEMD_UNIT])?;
    Ok(format!(
        "Installed {} and (re)started it.\n\
         To keep it running while you are logged out: loginctl enable-linger $USER",
        path.display()
    ))
}

/// systemd stops the agent with SIGTERM, which it handles like Ctrl-C.
pub fn uninstall(_data_dir: &Path) -> anyhow::Result<()> {
    let _ = command("systemctl", &["--user", "disable", "--now", SYSTEMD_UNIT]);
    let path = unit_path()?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    command("systemctl", &["--user", "daemon-reload"])?;
    Ok(())
}
