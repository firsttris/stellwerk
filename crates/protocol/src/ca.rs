//! The certificate authority a Zentrale behind a private CA (e.g. Caddy's `tls internal`) hands
//! to new devices. The pairing command carries the CA's fingerprint; the agent fetches the CA
//! from the Zentrale without trusting it yet and keeps it only when the fingerprints match, the
//! way a k3s join token pins its server's CA.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};

const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const END: &str = "-----END CERTIFICATE-----";

/// The DER of the first certificate in a PEM file; `None` when there is none or it is not
/// valid base64.
pub fn first_certificate(pem: &str) -> Option<Vec<u8>> {
    let start = pem.find(BEGIN)? + BEGIN.len();
    let body = &pem[start..];
    let body = &body[..body.find(END)?];
    let base64: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    STANDARD.decode(base64).ok()
}

/// The SHA-256 of a certificate's DER in lowercase hex: what
/// `openssl x509 -noout -fingerprint -sha256` prints, without the colons.
pub fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A fingerprint as a person may paste it, with colons or in capitals, in the form
/// [`fingerprint`] gives; `None` when it is not 32 bytes of hex.
pub fn normalize(fingerprint: &str) -> Option<String> {
    let hex: String = fingerprint
        .chars()
        .filter(|c| *c != ':')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())).then_some(hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A self-signed test CA; its fingerprint from `openssl x509 -fingerprint -sha256`.
    const PEM: &str = "-----BEGIN CERTIFICATE-----
MIIBkDCCATWgAwIBAgIUFRGnspLbsWleoerQLeJQtBnqC0YwCgYIKoZIzj0EAwIw
HDEaMBgGA1UEAwwRU3RlbGx3ZXJrIHRlc3QgQ0EwIBcNMjYxMDEzMTIyNTM4WhgP
MjEyNjA5MTkxMjI1MzhaMBwxGjAYBgNVBAMMEVN0ZWxsd2VyayB0ZXN0IENBMFkw
EwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEmYfBIN/paMdToH+O6n65z7au/TAMlSql
3MpPsC67SJ8CrP56WudmdahbcNQ8E7sGq9WlzBIij1Oik66EVSv866NTMFEwHQYD
VR0OBBYEFG5JJbIkM9KdsqPp5nZApJVXh3fOMB8GA1UdIwQYMBaAFG5JJbIkM9Kd
sqPp5nZApJVXh3fOMA8GA1UdEwEB/wQFMAMBAf8wCgYIKoZIzj0EAwIDSQAwRgIh
AMjcnL9mfuBfKL7v65lKTAxkHIOukcOPBHYGxYiNYfIpAiEA7LiCoEIYBG3Nctmn
mKzwGVJezEIyO6YinnT3+E+hv3c=
-----END CERTIFICATE-----
";
    const OPENSSL: &str = "DA:C9:1E:DE:63:D2:D1:0C:20:AF:7D:A1:A0:28:08:D2:58:B4:C4:27:A9:B1:84:B7:DB:18:32:A9:7E:F7:43:89";

    #[test]
    fn the_fingerprint_is_the_one_openssl_prints() {
        let der = first_certificate(PEM).unwrap();
        assert_eq!(Some(fingerprint(&der)), normalize(OPENSSL));
    }

    #[test]
    fn the_first_of_several_certificates_counts_and_text_around_it_is_ignored() {
        let bundle = format!("subject=CN = test\n{PEM}{PEM}");
        assert_eq!(first_certificate(&bundle), first_certificate(PEM));
        assert_eq!(first_certificate("no certificate"), None);
        assert_eq!(first_certificate(&format!("{BEGIN}\n!!\n{END}")), None);
    }

    #[test]
    fn fingerprints_are_normalized_and_checked() {
        let hex = "dac91ede63d2d10c20af7da1a02808d258b4c427a9b184b7db1832a97ef74389";
        assert_eq!(normalize(OPENSSL).as_deref(), Some(hex));
        assert_eq!(normalize(&hex.to_uppercase()).as_deref(), Some(hex));
        assert_eq!(normalize("dac9"), None);
        assert_eq!(normalize(&"zz".repeat(32)), None);
    }
}
