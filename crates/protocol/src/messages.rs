//! Messages on the agent's WebSocket to the Zentrale, JSON with a `type` field.
//!
//! 1. Zentrale → agent: [`ServerMessage::Challenge`] with a fresh nonce.
//! 2. Agent → Zentrale: [`AgentMessage::Hello`], signed over [`challenge_payload`] with the
//!    agent's Ed25519 key; on first contact with the one-time enrollment code.
//! 3. Zentrale → agent: [`ServerMessage::Welcome`], then the full [`DesiredState`] now and
//!    after every change. A rejected agent gets [`ServerMessage::Rejected`] and the socket closes.
//!    An unknown agent without a code may instead get [`ServerMessage::Waiting`]: it stays
//!    connected until it is approved in the UI (then step 3 follows) or rejected.
//! 4. Agent → Zentrale: [`AgentMessage::Applied`] after each desired state, and
//!    [`AgentMessage::Status`] whenever its folders change, at most about once per second.

use serde::{Deserialize, Serialize};

use crate::status::FolderStatus;
use crate::{DesiredState, DeviceId};

/// Path of the agent's WebSocket on the Zentrale.
pub const AGENT_WS_PATH: &str = "/api/agent/ws";

/// The mDNS service type the Zentrale announces in the LAN. Its TXT record may carry `url`,
/// the address devices should use (e.g. behind a TLS proxy); without it they use the
/// announced address and port over plain HTTP.
pub const MDNS_SERVICE: &str = "_stellwerk._tcp.local.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Challenge {
        nonce: String,
    },
    Welcome {
        device: String,
    },
    DesiredState(DesiredState),
    Rejected {
        reason: String,
    },
    /// The agent is unknown and came without a code: it waits for approval in the UI, where it
    /// shows up with this [`fingerprint`].
    Waiting {
        fingerprint: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentMessage {
    Hello(Hello),
    Applied(Applied),
    /// The state of every folder; sent on change, at most about once per second.
    Status(Status),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub folders: Vec<FolderStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    /// Ed25519 public key, base64 (standard alphabet, padded).
    pub public_key: String,
    /// Signature of [`challenge_payload`] for the received nonce, base64.
    pub signature: String,
    /// Only on first contact: the one-time code from "Pair device".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_code: Option<String>,
    /// The host name, suggested as the device name.
    pub name: String,
    /// Operating system as `std::env::consts::OS` names it.
    pub os: String,
    pub agent_version: String,
    pub syncthing_version: String,
    pub syncthing_id: DeviceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Applied {
    pub revision: u64,
    /// What could not be applied; empty when everything went through.
    #[serde(default)]
    pub errors: Vec<ApplyError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyError {
    /// The folder the failed change was about, if it was about one.
    #[serde(default)]
    pub folder: Option<String>,
    pub message: String,
}

/// A short, readable fingerprint of an agent's public key (as sent in [`Hello`]): the first
/// 8 bytes of its SHA-256 in hex, e.g. `3F2A-91BC-04DE-77A1`. Agent and UI show it, so the
/// admin can check that the waiting device is the one in front of them.
pub fn fingerprint(public_key: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(public_key.as_bytes());
    digest[..8]
        .chunks(2)
        .map(|pair| format!("{:02X}{:02X}", pair[0], pair[1]))
        .collect::<Vec<_>>()
        .join("-")
}

/// The bytes the agent signs: a fixed prefix keeps the key from signing anything else.
pub fn challenge_payload(nonce: &str) -> Vec<u8> {
    format!("stellwerk-agent-auth:{nonce}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_carry_their_type() {
        let json = serde_json::to_string(&ServerMessage::Challenge { nonce: "n".into() }).unwrap();
        assert_eq!(json, r#"{"type":"challenge","nonce":"n"}"#);

        let state = ServerMessage::DesiredState(DesiredState::default());
        let json = serde_json::to_string(&state).unwrap();
        assert!(
            json.starts_with(r#"{"type":"desiredState","revision":0"#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<ServerMessage>(&json).unwrap(), state);

        let applied = AgentMessage::Applied(Applied {
            revision: 3,
            errors: vec![],
        });
        let json = serde_json::to_string(&applied).unwrap();
        assert_eq!(json, r#"{"type":"applied","revision":3,"errors":[]}"#);
    }

    #[test]
    fn a_hello_with_fields_unknown_here_still_reads() {
        // Agents up to v0.1.1 sent `applied_revision`, which nothing read.
        let json = r#"{"type":"hello","public_key":"k","signature":"s","name":"laptop",
            "os":"linux","agent_version":"0.1.1","syncthing_version":"v2",
            "syncthing_id":"ID","applied_revision":7}"#;
        let AgentMessage::Hello(hello) = serde_json::from_str(json).unwrap() else {
            panic!("not a hello");
        };
        assert_eq!(hello.name, "laptop");
        assert_eq!(hello.enrollment_code, None);
    }

    #[test]
    fn fingerprints_are_short_and_stable() {
        let a = fingerprint("key-a");
        assert_eq!(a.len(), 19);
        assert_eq!(a.split('-').count(), 4);
        assert!(
            a.chars()
                .all(|c| c == '-' || c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
        );
        assert_eq!(a, fingerprint("key-a"));
        assert_ne!(a, fingerprint("key-b"));
    }

    #[test]
    fn waiting_carries_the_fingerprint() {
        let json = serde_json::to_string(&ServerMessage::Waiting {
            fingerprint: "AB".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"type":"waiting","fingerprint":"AB"}"#);
    }

    #[test]
    fn the_payload_is_prefixed() {
        assert_eq!(challenge_payload("abc"), b"stellwerk-agent-auth:abc");
    }
}
