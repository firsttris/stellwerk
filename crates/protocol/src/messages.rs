//! Messages on the agent's WebSocket to the Zentrale, JSON with a `type` field.
//!
//! 1. Zentrale → agent: [`ServerMessage::Challenge`] with a fresh nonce.
//! 2. Agent → Zentrale: [`AgentMessage::Hello`], signed over [`challenge_payload`] with the
//!    agent's Ed25519 key; on first contact with the one-time enrollment code.
//! 3. Zentrale → agent: [`ServerMessage::Welcome`], then the full [`DesiredState`] now and
//!    after every change. A rejected agent gets [`ServerMessage::Rejected`] and the socket closes.
//! 4. Agent → Zentrale: [`AgentMessage::Applied`] after each desired state, and
//!    [`AgentMessage::Status`] whenever its folders change, at most about once per second.

use serde::{Deserialize, Serialize};

use crate::status::FolderStatus;
use crate::{DesiredState, DeviceId};

/// Path of the agent's WebSocket on the Zentrale.
pub const AGENT_WS_PATH: &str = "/api/agent/ws";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Challenge { nonce: String },
    Welcome { device: String },
    DesiredState(DesiredState),
    Rejected { reason: String },
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
    /// The revision the agent applied last, if any.
    #[serde(default)]
    pub applied_revision: Option<u64>,
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
    fn the_payload_is_prefixed() {
        assert_eq!(challenge_payload("abc"), b"stellwerk-agent-auth:abc");
    }
}
