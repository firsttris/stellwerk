//! The agent's Ed25519 key: created once, it identifies the device to the Zentrale.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signer, SigningKey};
use rand::Rng;
use stellwerk_protocol::messages::{Hello, hello_payload};

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

/// The hello with its signature over the Zentrale's nonce and all its other fields.
pub fn sign_hello(key: &SigningKey, nonce: &str, hello: Hello) -> Hello {
    let signature = BASE64.encode(key.sign(&hello_payload(nonce, &hello)).to_bytes());
    Hello { signature, ..hello }
}

#[cfg(test)]
mod tests {
    use stellwerk_protocol::DeviceId;

    use super::*;

    fn hello(key: &SigningKey) -> Hello {
        Hello {
            public_key: public_key(key),
            signature: String::new(),
            enrollment_code: None,
            name: "laptop".into(),
            os: "linux".into(),
            agent_version: "0.1.1".into(),
            syncthing_version: "v2.1.6".into(),
            syncthing_id: DeviceId::new("AAAA"),
        }
    }

    #[test]
    fn a_stored_secret_gives_the_same_key() {
        let secret = new_secret();
        let a = signing_key(&secret).unwrap();
        let b = signing_key(&secret).unwrap();
        assert_eq!(public_key(&a), public_key(&b));
        assert_eq!(
            sign_hello(&a, "n", hello(&a)),
            sign_hello(&b, "n", hello(&b))
        );
        assert_ne!(secret, new_secret());
        assert!(signing_key("short").is_err());
    }

    #[test]
    fn the_signature_covers_nonce_and_hello() {
        let key = signing_key(&new_secret()).unwrap();
        let signed = sign_hello(&key, "n", hello(&key));
        assert_ne!(signed, sign_hello(&key, "m", hello(&key)));
        let other_id = Hello {
            syncthing_id: DeviceId::new("BBBB"),
            ..hello(&key)
        };
        assert_ne!(sign_hello(&key, "n", other_id).signature, signed.signature);
    }
}
