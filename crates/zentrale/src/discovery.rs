//! Announces the Zentrale in the LAN over mDNS, so `stellwerk-agent join` without a server
//! finds it (LAN pairing).

use mdns_sd::{ServiceDaemon, ServiceInfo};
use stellwerk_protocol::messages::MDNS_SERVICE;

/// Announces the web port on every interface until the returned daemon is shut down.
/// `url`, if set, tells devices where to connect instead (e.g. the TLS proxy).
pub fn announce(port: u16, url: Option<&str>) -> anyhow::Result<ServiceDaemon> {
    let host = hostname::get()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "stellwerk".into());
    let mut properties = vec![("version", env!("CARGO_PKG_VERSION"))];
    if let Some(url) = url {
        properties.push(("url", url));
    }
    let info = ServiceInfo::new(
        MDNS_SERVICE,
        &format!("Stellwerk on {host}"),
        &format!("{host}.local."),
        "",
        port,
        &properties[..],
    )?
    .enable_addr_auto();
    let daemon = ServiceDaemon::new()?;
    daemon.register(info)?;
    Ok(daemon)
}
