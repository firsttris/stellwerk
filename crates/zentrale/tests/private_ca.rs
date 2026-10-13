//! A Zentrale behind HTTPS with a private CA, as with Caddy's `tls internal`: no system trusts
//! the CA, yet a device pairs with the fingerprint from the pairing command alone.

mod common;

use std::sync::Arc;

use common::{new_pairing_code, start_agent, start_zentrale, wait_for};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use stellwerk_agent::ca;
use stellwerk_zentrale::ca::from_pem;
use tokio::net::{TcpListener, TcpStream};

/// A private CA and a certificate for `localhost` it signed.
struct Pki {
    ca_pem: String,
    chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

fn pki() -> Pki {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Test private CA");
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);

    let key = rcgen::KeyPair::generate().unwrap();
    let cert = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&key, &issuer)
        .unwrap();
    Pki {
        ca_pem: ca_cert.pem(),
        chain: vec![cert.der().clone()],
        key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    }
}

/// Terminates TLS like a reverse proxy and passes the bytes on to the Zentrale; returns the
/// HTTPS address.
async fn tls_proxy(pki: &Pki, upstream: std::net::SocketAddr) -> String {
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(pki.chain.clone(), pki.key.clone_key())
        .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((client, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(client).await else {
                    return;
                };
                if let Ok(mut zentrale) = TcpStream::connect(upstream).await {
                    let _ = tokio::io::copy_bidirectional(&mut tls, &mut zentrale).await;
                }
            });
        }
    });
    format!("https://localhost:{port}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_pairs_through_a_private_ca_with_the_fingerprint_from_the_command() {
    let tmp = tempfile::tempdir().unwrap();
    let pki = pki();
    let zentrale = start_zentrale(tmp.path()).await;
    let ca_info = from_pem(pki.ca_pem.clone()).unwrap();
    zentrale.app.set_ca(ca_info.clone());
    let server = tls_proxy(&pki, zentrale.addr).await;

    // Without the CA the connection is refused, as curl refused it.
    let url = stellwerk_agent::config::ws_url(&server).unwrap();
    let plain = tokio_tungstenite::connect_async(url.as_str()).await;
    assert!(plain.is_err(), "the system does not trust a private CA");

    // A wrong fingerprint is refused before anything is stored.
    let other = from_pem(pki_ca_only()).unwrap();
    let wrong = ca::fetch(&server, &other.sha256).await.unwrap_err();
    assert!(format!("{wrong:#}").contains("fingerprint"), "{wrong:#}");

    // The fingerprint from the command lets the device in.
    let fetched = ca::fetch(&server, &ca_info.sha256).await.unwrap();
    let dir = tmp.path().join("laptop");
    let code = new_pairing_code(&zentrale).await;
    stellwerk_agent::join(&dir, &server, Some(&code), Some(fetched)).unwrap();
    let agent = start_agent(dir).await;
    let store = zentrale.app.store().clone();
    let device = wait_for("the device to pair over HTTPS", || async {
        store.config().await.ok()?.devices.into_iter().next()
    })
    .await;
    wait_for("the device to be online", || async {
        zentrale
            .app
            .online_devices()
            .contains_key(&device.id)
            .then_some(())
    })
    .await;

    agent.stop().await;
    zentrale.stop().await.unwrap();
}

/// Another CA's certificate, for a fingerprint that does not match.
fn pki_ca_only() -> String {
    pki().ca_pem
}
