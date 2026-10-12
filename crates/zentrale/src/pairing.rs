//! One-time codes for pairing a device, and checking an agent's signed hello.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signature, VerifyingKey};
use rand::Rng;
use sha2::{Digest, Sha256};
use stellwerk_protocol::messages::{Hello, hello_payload};

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

/// Checks that the hello was signed by the key it names, for this connection's nonce and with
/// exactly the fields it carries ([`hello_payload`]).
pub fn verify_hello(hello: &Hello, nonce: &str) -> Result<(), SignatureError> {
    let key: [u8; 32] = BASE64
        .decode(&hello.public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(SignatureError::BadKey)?;
    let key = VerifyingKey::from_bytes(&key).map_err(|_| SignatureError::BadKey)?;
    let signature: [u8; 64] = BASE64
        .decode(&hello.signature)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(SignatureError::BadSignature)?;
    key.verify_strict(
        &hello_payload(nonce, hello),
        &Signature::from_bytes(&signature),
    )
    .map_err(|_| SignatureError::BadSignature)
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
    use stellwerk_protocol::DeviceId;

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

    fn signed_hello(key: &SigningKey, nonce: &str) -> Hello {
        let hello = Hello {
            public_key: BASE64.encode(key.verifying_key().as_bytes()),
            signature: String::new(),
            enrollment_code: Some("K7Q2-MX9P-41TZ".into()),
            name: "laptop".into(),
            os: "linux".into(),
            agent_version: "0.1.1".into(),
            syncthing_version: "v2.1.6".into(),
            syncthing_id: DeviceId::new("AAAA"),
        };
        let signature = BASE64.encode(key.sign(&hello_payload(nonce, &hello)).to_bytes());
        Hello { signature, ..hello }
    }

    #[test]
    fn only_the_right_key_and_nonce_verify() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let hello = signed_hello(&key, "nonce");

        assert_eq!(verify_hello(&hello, "nonce"), Ok(()));
        assert_eq!(
            verify_hello(&hello, "other"),
            Err(SignatureError::BadSignature)
        );
        let other = Hello {
            public_key: BASE64.encode(SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes()),
            ..hello.clone()
        };
        assert_eq!(
            verify_hello(&other, "nonce"),
            Err(SignatureError::BadSignature)
        );
        let garbled = Hello {
            public_key: "not base64".into(),
            ..hello
        };
        assert_eq!(verify_hello(&garbled, "nonce"), Err(SignatureError::BadKey));
    }

    #[test]
    fn a_changed_syncthing_id_or_code_breaks_the_signature() {
        let hello = signed_hello(&SigningKey::from_bytes(&[7; 32]), "nonce");
        let relayed = Hello {
            syncthing_id: DeviceId::new("ATTACKER"),
            ..hello.clone()
        };
        assert_eq!(
            verify_hello(&relayed, "nonce"),
            Err(SignatureError::BadSignature)
        );
        let other_code = Hello {
            enrollment_code: Some("AAAA-BBBB-CCCC".into()),
            ..hello.clone()
        };
        assert_eq!(
            verify_hello(&other_code, "nonce"),
            Err(SignatureError::BadSignature)
        );
        let renamed = Hello {
            name: "phone".into(),
            ..hello
        };
        assert_eq!(
            verify_hello(&renamed, "nonce"),
            Err(SignatureError::BadSignature)
        );
    }

    #[test]
    fn a_signature_over_the_nonce_alone_is_refused() {
        // How agents up to v0.1.1 signed: `stellwerk-agent-auth:<nonce>`.
        let key = SigningKey::from_bytes(&[7; 32]);
        let old = key.sign(b"stellwerk-agent-auth:nonce");
        let hello = Hello {
            signature: BASE64.encode(old.to_bytes()),
            ..signed_hello(&key, "nonce")
        };
        assert_eq!(
            verify_hello(&hello, "nonce"),
            Err(SignatureError::BadSignature)
        );
    }
}
