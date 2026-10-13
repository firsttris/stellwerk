//! A Zentrale behind a private CA: the agent fetches the CA once at `join`, keeps it only when
//! its fingerprint is the one in the pairing command, and trusts it from then on next to the
//! system's roots (`stellwerk_protocol::ca`).

use std::sync::Arc;

use anyhow::Context;
use stellwerk_protocol::ca::{fingerprint, first_certificate, normalize};
use tokio_tungstenite::Connector;

/// Where the Zentrale serves its CA (`crates/zentrale/src/server.rs`).
pub const CA_PATH: &str = "/ca.crt";

/// Fetches the Zentrale's CA without trusting the connection yet, and returns its PEM when its
/// fingerprint is `expected`. A connection that is not to the Zentrale cannot pass: it would
/// need the CA's certificate itself, and that is what the fingerprint pins.
pub async fn fetch(server: &str, expected: &str) -> anyhow::Result<String> {
    let expected =
        normalize(expected).with_context(|| format!("not a SHA-256 fingerprint: {expected}"))?;
    let url = format!("{}{CA_PATH}", server.trim_end_matches('/'));
    let pem = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()?
        .get(&url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .with_context(|| format!("fetching the Zentrale's CA from {url}"))?
        .text()
        .await?;
    check(&pem, &expected)?;
    Ok(pem)
}

/// The PEM's first certificate has the fingerprint `expected` (normalized).
fn check(pem: &str, expected: &str) -> anyhow::Result<()> {
    let der = first_certificate(pem).context("the Zentrale sent no certificate as its CA")?;
    let actual = fingerprint(&der);
    anyhow::ensure!(
        actual == expected,
        "the Zentrale's CA has the fingerprint {actual}, not {expected} from the pairing command; \
         someone may be in between, or the command is for another Zentrale"
    );
    Ok(())
}

/// The TLS setup for the connection to the Zentrale: the system's roots, plus the stored CA
/// when there is one; `None` keeps tokio-tungstenite's default (the system's roots).
pub fn connector(ca: Option<&str>) -> anyhow::Result<Option<Connector>> {
    let Some(pem) = ca else { return Ok(None) };
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    let der = first_certificate(pem).context("the stored CA is not a PEM certificate")?;
    roots
        .add(der.into())
        .context("the stored CA is not a usable certificate")?;
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(Some(Connector::Rustls(Arc::new(config))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ca() -> (String, String) {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let cert = params.self_signed(&key).unwrap();
        (cert.pem(), fingerprint(cert.der()))
    }

    #[test]
    fn only_the_ca_with_the_expected_fingerprint_passes() {
        let (pem, sha) = test_ca();
        assert!(check(&pem, &sha).is_ok());
        let (_, other) = test_ca();
        let err = check(&pem, &other).unwrap_err().to_string();
        assert!(err.contains("not"), "{err}");
        assert!(check("<html>", &sha).is_err());
    }

    #[test]
    fn a_stored_ca_gives_a_connector_and_none_keeps_the_default() {
        let (pem, _) = test_ca();
        assert!(connector(None).unwrap().is_none());
        assert!(connector(Some(&pem)).unwrap().is_some());
        assert!(connector(Some("garbage")).is_err());
    }
}
