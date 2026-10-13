//! A Zentrale behind a private CA (e.g. Caddy's `tls internal`): `--ca-cert` names the CA's
//! certificate, the Zentrale serves it at `/ca.crt`, and the pairing dialog puts its
//! fingerprint into the command, so a new device can trust it (`stellwerk_protocol::ca`).

use std::path::Path;

use anyhow::Context;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use stellwerk_protocol::ca::{fingerprint, first_certificate};

use crate::app::App;

/// The CA devices are told to trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ca {
    pub pem: String,
    /// SHA-256 of the certificate, lowercase hex.
    pub sha256: String,
}

/// The CA from a PEM file; only its first certificate counts.
pub fn load(path: &Path) -> anyhow::Result<Ca> {
    let pem = std::fs::read_to_string(path)
        .with_context(|| format!("reading the CA certificate {}", path.display()))?;
    from_pem(pem).with_context(|| format!("{} holds no PEM certificate", path.display()))
}

pub fn from_pem(pem: String) -> Option<Ca> {
    let sha256 = fingerprint(&first_certificate(&pem)?);
    Some(Ca { pem, sha256 })
}

/// `GET /ca.crt`, without login: a device fetches it before it trusts the connection and
/// checks it against the fingerprint from the pairing command.
pub async fn serve(State(app): State<App>) -> Response {
    match app.ca() {
        Some(ca) => (
            [(header::CONTENT_TYPE, "application/x-pem-file")],
            ca.pem.clone(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "no private CA configured").into_response(),
    }
}
