//! Task Scheduler task that starts the windowless agent at logon (`schtasks`).

use std::path::Path;
use std::time::Duration;

use super::definitions::{BACKGROUND_EXE, TASK_NAME, task_xml};
use super::{Service, command, request_stop};

/// How long uninstalling waits for the agent to stop Syncthing.
const STOP_TIMEOUT: Duration = Duration::from_secs(30);

/// `DOMAIN\name` of the current user.
fn user() -> anyhow::Result<String> {
    let name = std::env::var("USERNAME")?;
    Ok(match std::env::var("USERDOMAIN") {
        Ok(domain) => format!("{domain}\\{name}"),
        Err(_) => name,
    })
}

pub fn install(service: &Service) -> anyhow::Result<String> {
    let background = service.exe.with_file_name(BACKGROUND_EXE);
    anyhow::ensure!(
        background.exists(),
        "{} is missing next to {}",
        BACKGROUND_EXE,
        service.exe.display()
    );
    std::fs::create_dir_all(&service.data_dir)?;
    // Task Scheduler reads task XML as UTF-16 with a byte order mark.
    let xml = task_xml(&background, &service.data_dir, &user()?);
    let utf16: Vec<u8> = std::iter::once(0xFEFF_u16)
        .chain(xml.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect();
    let file = service.data_dir.join("task.xml");
    std::fs::write(&file, utf16)?;
    let created = command(
        "schtasks",
        &[
            "/Create",
            "/F",
            "/TN",
            TASK_NAME,
            "/XML",
            &file.to_string_lossy(),
        ],
    );
    let _ = std::fs::remove_file(&file);
    created?;
    command("schtasks", &["/Run", "/TN", TASK_NAME])?;
    Ok(format!("Created the task {TASK_NAME} and started it."))
}

/// Ending the task would kill the agent without a signal and leave Syncthing running, so the
/// agent is asked to stop through the stop file first.
pub fn uninstall(data_dir: &Path) -> anyhow::Result<()> {
    request_stop(data_dir, STOP_TIMEOUT)?;
    // The agent removes the stop file first, then stops Syncthing and exits; wait for that,
    // so its files are free to be deleted.
    let deadline = std::time::Instant::now() + STOP_TIMEOUT;
    while agent_running() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(500));
    }
    let _ = command("schtasks", &["/End", "/TN", TASK_NAME]);
    let _ = command("schtasks", &["/Delete", "/F", "/TN", TASK_NAME]);
    Ok(())
}

/// Whether the background agent still runs; by process name, as task states are localized.
fn agent_running() -> bool {
    let filter = format!("IMAGENAME eq {BACKGROUND_EXE}");
    command("tasklist", &["/FI", &filter, "/NH"])
        .is_ok_and(|list| list.to_lowercase().contains(BACKGROUND_EXE))
}
