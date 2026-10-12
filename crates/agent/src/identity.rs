//! The agent's Ed25519 key: created once, it identifies the device to the Zentrale.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signer, SigningKey};
use rand::Rng;
use stellwerk_protocol::messages::challenge_payload;

/// A fresh secret key, base64.
pub fn new_secret() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    BASE64.encode(bytes)
}

/// The key from its stored base64 form.
pub fn signing_key(secret: &str) -> anyhow::Result<SigningKey> {
    let bytes: [u8; 32] = BASE64
        .decode(secret)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("the stored key is not 32 bytes"))?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn public_key(key: &SigningKey) -> String {
    BASE64.encode(key.verifying_key().as_bytes())
}

/// Signs the Zentrale's challenge, base64.
pub fn sign_challenge(key: &SigningKey, nonce: &str) -> String {
    BASE64.encode(key.sign(&challenge_payload(nonce)).to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_secret_gives_the_same_key() {
        let secret = new_secret();
        let a = signing_key(&secret).unwrap();
        let b = signing_key(&secret).unwrap();
        assert_eq!(public_key(&a), public_key(&b));
        assert_eq!(sign_challenge(&a, "n"), sign_challenge(&b, "n"));
        assert_ne!(secret, new_secret());
        assert!(signing_key("short").is_err());
    }
}
