//! Finds a Zentrale in the LAN over mDNS, for `stellwerk-agent join` without a server.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent};
use stellwerk_protocol::messages::MDNS_SERVICE;

/// A Zentrale that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its instance name, e.g. "Stellwerk on server".
    pub name: String,
    pub url: String,
}

/// Listens for `timeout` and returns every Zentrale that announced itself, by name.
pub async fn discover(timeout: Duration) -> anyhow::Result<Vec<Found>> {
    tokio::task::spawn_blocking(move || browse(timeout)).await?
}

fn browse(timeout: Duration) -> anyhow::Result<Vec<Found>> {
    let daemon = ServiceDaemon::new()?;
    let events = daemon.browse(MDNS_SERVICE)?;
    let deadline = Instant::now() + timeout;
    let mut found = BTreeMap::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        let Ok(event) = events.recv_timeout(left) else {
            break;
        };
        if let ServiceEvent::ServiceResolved(service) = event {
            let addresses: Vec<IpAddr> = service.addresses.iter().map(|a| a.to_ip_addr()).collect();
            let txt_url = service.txt_properties.get_property_val_str("url");
            if let Some(url) = url_of(txt_url, &addresses, service.port) {
                let name = instance_name(&service.fullname);
                found.insert(name.clone(), Found { name, url });
            }
        }
    }
    let _ = daemon.shutdown();
    Ok(found.into_values().collect())
}

/// Where to reach an announced Zentrale: the `url` it announces, else plain HTTP on its first
/// usable address, IPv4 before IPv6.
pub fn url_of(txt_url: Option<&str>, addresses: &[IpAddr], port: u16) -> Option<String> {
    if let Some(url) = txt_url.filter(|url| !url.is_empty()) {
        return Some(url.trim_end_matches('/').to_string());
    }
    let usable = |ip: &&IpAddr| !ip.is_loopback() && !ip.is_unspecified();
    let address = addresses
        .iter()
        .filter(usable)
        .find(|ip| ip.is_ipv4())
        .or_else(|| addresses.iter().find(|ip| usable(ip) && !is_link_local(ip)))?;
    Some(match address {
        IpAddr::V4(ip) => format!("http://{ip}:{port}"),
        IpAddr::V6(ip) => format!("http://[{ip}]:{port}"),
    })
}

/// IPv6 link-local addresses need a zone, which a URL cannot carry portably.
fn is_link_local(ip: &IpAddr) -> bool {
    matches!(ip, IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80)
}

/// "Stellwerk on server._stellwerk._tcp.local." → "Stellwerk on server".
fn instance_name(fullname: &str) -> String {
    fullname
        .strip_suffix(MDNS_SERVICE)
        .map(|name| name.trim_end_matches('.'))
        .unwrap_or(fullname)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ips(list: &[&str]) -> Vec<IpAddr> {
        list.iter().map(|ip| ip.parse().unwrap()).collect()
    }

    #[test]
    fn an_announced_url_wins() {
        assert_eq!(
            url_of(
                Some("https://sync.example.org/"),
                &ips(&["192.168.1.5"]),
                8080
            ),
            Some("https://sync.example.org".into())
        );
    }

    #[test]
    fn otherwise_the_first_ipv4_address() {
        assert_eq!(
            url_of(None, &ips(&["fd00::5", "127.0.0.1", "192.168.1.5"]), 8080),
            Some("http://192.168.1.5:8080".into())
        );
        assert_eq!(
            url_of(Some(""), &ips(&["fe80::1", "fd00::5"]), 8080),
            Some("http://[fd00::5]:8080".into())
        );
        assert_eq!(url_of(None, &ips(&["127.0.0.1", "fe80::1"]), 8080), None);
    }

    #[test]
    fn the_instance_name_drops_the_service() {
        assert_eq!(
            instance_name("Stellwerk on server._stellwerk._tcp.local."),
            "Stellwerk on server"
        );
    }
}
