//! The admin's password (argon2id) and random tokens for sessions and CSRF.

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngExt;
use sha2::{Digest, Sha256};

/// Passwords shorter than this are refused.
pub const MIN_PASSWORD_LEN: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    #[error("the password needs at least {MIN_PASSWORD_LEN} characters")]
    TooShort,
    #[error("could not hash the password: {0}")]
    Hash(String),
}

/// A PHC string (`$argon2id$v=19$…`) with a random salt.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(PasswordError::TooShort);
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|err| PasswordError::Hash(err.to_string()))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

/// A random token for a cookie or header, URL-safe base64.
pub fn new_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// What is stored for a session token: its SHA-256, so a leaked database opens no session.
pub fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Constant-time comparison for the CSRF token.
pub fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |diff, (x, y)| diff | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hashed_password_verifies_and_others_do_not() {
        let hash = hash_password("correct horse").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("correct horse", &hash));
        assert!(!verify_password("wrong horse!", &hash));
        assert!(!verify_password("correct horse", "not a hash"));
        assert_eq!(hash_password("short"), Err(PasswordError::TooShort));
    }

    #[test]
    fn tokens_are_random_and_compared_exactly() {
        let token = new_token();
        assert_ne!(token, new_token());
        assert_eq!(token_hash(&token), token_hash(&token));
        assert!(same_token(&token, &token.clone()));
        assert!(!same_token(&token, &new_token()));
        assert!(!same_token("a", "ab"));
    }
}
