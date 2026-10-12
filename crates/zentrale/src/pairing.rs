//! One-time codes for pairing a device, and checking an agent's signed hello.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signature, VerifyingKey};
use rand::Rng;
use sha2::{Digest, Sha256};
use stellwerk_protocol::messages::challenge_payload;

/// How long a code from "Pair device" can be used.
pub const CODE_LIFETIME_SECS: i64 = 15 * 60;

/// No 0/O, 1/I: codes are read off one screen and typed into another.
const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// A new code such as `K7Q2-MX9P-41TZ`.
pub fn new_code() -> String {
    let mut rng = rand::rng();
    let chars: Vec<char> = (0..12)
        .map(|_| char::from(ALPHABET[rng.random_range(0..ALPHABET.len())]))
        .collect();
    chars
        .chunks(4)
        .map(|group| group.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("-")
}

/// What is stored and compared: the code without case, dashes or spaces, hashed.
pub fn code_hash(code: &str) -> String {
    let normalized: String = code
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    hex_sha256(format!("stellwerk-enroll:{normalized}").as_bytes())
}

/// A random nonce for the challenge, base64.
pub fn new_nonce() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    BASE64.encode(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("public key is not a base64 Ed25519 key")]
    BadKey,
    #[error("signature is not valid for this key and challenge")]
    BadSignature,
}

/// Checks that the hello was signed by the key it names, for this connection's nonce.
pub fn verify_hello(public_key: &str, signature: &str, nonce: &str) -> Result<(), SignatureError> {
    let key: [u8; 32] = BASE64
        .decode(public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(SignatureError::BadKey)?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| SignatureError::BadKey)?;
    let signature: [u8; 64] = BASE64
        .decode(signature)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(SignatureError::BadSignature)?;
    key.verify_strict(
        &challenge_payload(nonce),
        &Signature::from_bytes(&signature),
    )
    .map_err(|_| SignatureError::BadSignature)
}

/// The short fingerprint shown in the UI next to a new device, e.g. `7F3A-91C2`.
pub fn fingerprint(public_key: &str) -> String {
    let hash = hex_sha256(public_key.as_bytes()).to_uppercase();
    format!("{}-{}", &hash[0..4], &hash[4..8])
}

fn hex_sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    #[test]
    fn codes_are_three_groups_and_typed_loosely() {
        let code = new_code();
        assert_eq!(code.len(), 14);
        assert_eq!(code.matches('-').count(), 2);
        assert_ne!(code, new_code());
        assert_eq!(code_hash("k7q2-mx9p-41tz"), code_hash("K7Q2 MX9P 41TZ"));
        assert_ne!(code_hash("K7Q2-MX9P-41TZ"), code_hash("K7Q2-MX9P-41TA"));
    }

    #[test]
    fn only_the_right_key_and_nonce_verify() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let public = BASE64.encode(key.verifying_key().as_bytes());
        let signed = BASE64.encode(key.sign(&challenge_payload("nonce")).to_bytes());

        assert_eq!(verify_hello(&public, &signed, "nonce"), Ok(()));
        assert_eq!(
            verify_hello(&public, &signed, "other"),
            Err(SignatureError::BadSignature)
        );
        let other = BASE64.encode(SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes());
        assert_eq!(
            verify_hello(&other, &signed, "nonce"),
            Err(SignatureError::BadSignature)
        );
        assert_eq!(
            verify_hello("not base64", &signed, "nonce"),
            Err(SignatureError::BadKey)
        );
    }

    #[test]
    fn fingerprints_are_short_and_stable() {
        assert_eq!(fingerprint("KEY"), fingerprint("KEY"));
        assert_eq!(fingerprint("KEY").len(), 9);
    }
}
