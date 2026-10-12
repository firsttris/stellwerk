//! The service definitions per platform, as text.

use std::path::Path;

use super::Service;

pub const SYSTEMD_UNIT: &str = "stellwerk-agent.service";
pub const LAUNCHD_LABEL: &str = "io.github.firsttris.stellwerk.agent";
pub const TASK_NAME: &str = "Stellwerk\\Agent";
/// The windowless agent on Windows; a console program started at logon would open a window.
pub const BACKGROUND_EXE: &str = "stellwerk-agent-background.exe";

/// A systemd user unit (systemd.service(5)); `%` is systemd's specifier character.
pub fn systemd_unit(service: &Service) -> String {
    let quote = |path: &Path| format!("\"{}\"", path.display().to_string().replace('%', "%%"));
    format!(
        "[Unit]\n\
         Description=Stellwerk agent: keeps Syncthing at the state the Zentrale sets\n\
         Wants=network-online.target\n\
         After=network-online.target\n\
         \n\
         [Service]\n\
         ExecStart={} --data-dir {} run\n\
         Restart=on-failure\n\
         RestartSec=10\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        quote(&service.exe),
        quote(&service.data_dir),
    )
}

/// A launchd agent (launchd.plist(5)), logging into the data directory.
pub fn launchd_plist(service: &Service) -> String {
    let path = |path: &Path| xml_escape(&path.display().to_string());
    let log = service.data_dir.join("agent.log");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--data-dir</string>
    <string>{}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{}</string>
  <key>StandardErrorPath</key>
  <string>{}</string>
</dict>
</plist>
"#,
        path(&service.exe),
        path(&service.data_dir),
        path(&log),
        path(&log),
    )
}

/// A Task Scheduler task (schema 1.2) that starts the windowless agent at the user's logon,
/// as that user, without stored password or elevation, and restarts it when it fails.
/// `exe` is the background binary, `user` is `DOMAIN\name`.
pub fn task_xml(exe: &Path, data_dir: &Path, user: &str) -> String {
    let exe = xml_escape(&exe.display().to_string());
    let data_dir = xml_escape(&data_dir.display().to_string());
    let user = xml_escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Stellwerk agent: keeps Syncthing at the state the Zentrale sets</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>999</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--data-dir "{data_dir}"</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> Service {
        Service {
            exe: "/home/ann/.local/bin/stellwerk-agent".into(),
            data_dir: "/home/ann/.local/share/stellwerk agent 100%".into(),
        }
    }

    #[test]
    fn the_unit_runs_the_agent_with_its_data_dir() {
        let unit = systemd_unit(&service());
        assert!(unit.contains(
            "ExecStart=\"/home/ann/.local/bin/stellwerk-agent\" --data-dir \"/home/ann/.local/share/stellwerk agent 100%%\" run\n"
        ));
        assert!(unit.contains("WantedBy=default.target"));
    }

    #[test]
    fn the_plist_lists_every_argument() {
        let plist = launchd_plist(&service());
        assert!(plist.contains(
            "<string>/home/ann/.local/bin/stellwerk-agent</string>\n    <string>--data-dir</string>\n    <string>/home/ann/.local/share/stellwerk agent 100%</string>\n    <string>run</string>"
        ));
        let log = service().data_dir.join("agent.log");
        assert!(plist.contains(&format!("<string>{}</string>", log.display())));
    }

    #[test]
    fn the_task_starts_at_logon_as_the_user() {
        let xml = task_xml(
            Path::new(
                r"C:\Users\Ann & Bob\AppData\Local\Programs\Stellwerk\stellwerk-agent-background.exe",
            ),
            Path::new(r"C:\Users\Ann & Bob\AppData\Local\stellwerk-agent"),
            r"PC\Ann & Bob",
        );
        assert!(xml.contains(r"<UserId>PC\Ann &amp; Bob</UserId>"));
        assert!(xml.contains(r#"<Arguments>--data-dir "C:\Users\Ann &amp; Bob\AppData\Local\stellwerk-agent"</Arguments>"#));
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
    }
}
