//! Messages on the agent's WebSocket to the Zentrale, JSON with a `type` field.
//!
//! 1. Zentrale → agent: [`ServerMessage::Challenge`] with a fresh nonce.
//! 2. Agent → Zentrale: [`AgentMessage::Hello`], signed over [`hello_payload`] (the nonce and
//!    every other field of the hello) with the agent's Ed25519 key; on first contact with the
//!    one-time enrollment code.
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
    /// Signature of [`hello_payload`] for the received nonce and this hello, base64.
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

/// What the agent signs: a fixed prefix that keeps the key from signing anything else (its
/// version keeps a signature over the nonce alone, as agents up to v0.1.1 made it, from
/// passing), the connection's nonce, and every field of the hello but the signature itself.
/// Over plain HTTP this keeps an on-path attacker from relaying a valid signature with its own
/// Syncthing ID or with the code moved into a hello of its own. Each field is length-prefixed
/// and a missing code is `~`, so no two hellos give the same bytes.
pub fn hello_payload(nonce: &str, hello: &Hello) -> Vec<u8> {
    let fields = [
        Some(nonce),
        Some(hello.public_key.as_str()),
        hello.enrollment_code.as_deref(),
        Some(hello.name.as_str()),
        Some(hello.os.as_str()),
        Some(hello.agent_version.as_str()),
        Some(hello.syncthing_version.as_str()),
        Some(hello.syncthing_id.as_str()),
    ];
    let mut payload = b"stellwerk-agent-auth-v2:".to_vec();
    for field in fields {
        match field {
            Some(value) => payload.extend(format!("{}:{value}", value.len()).bytes()),
            None => payload.push(b'~'),
        }
    }
    payload
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

    fn hello() -> Hello {
        Hello {
            public_key: "key".into(),
            signature: String::new(),
            enrollment_code: Some("K7Q2-MX9P-41TZ".into()),
            name: "laptop".into(),
            os: "linux".into(),
            agent_version: "0.1.1".into(),
            syncthing_version: "v2.1.6".into(),
            syncthing_id: DeviceId::new("AAAA"),
        }
    }

    #[test]
    fn the_payload_is_prefixed_and_covers_the_hello() {
        assert_eq!(
            hello_payload("n", &hello()),
            b"stellwerk-agent-auth-v2:1:n3:key14:K7Q2-MX9P-41TZ6:laptop5:linux5:0.1.16:v2.1.64:AAAA"
        );
        let signed = Hello {
            signature: "sig".into(),
            ..hello()
        };
        assert_eq!(hello_payload("n", &signed), hello_payload("n", &hello()));
    }

    #[test]
    fn every_field_changes_the_payload() {
        let base = hello_payload("n", &hello());
        let changed = [
            Hello {
                syncthing_id: DeviceId::new("BBBB"),
                ..hello()
            },
            Hello {
                enrollment_code: Some("K7Q2-MX9P-41TA".into()),
                ..hello()
            },
            Hello {
                enrollment_code: None,
                ..hello()
            },
            Hello {
                public_key: "other".into(),
                ..hello()
            },
            Hello {
                name: "phone".into(),
                ..hello()
            },
            Hello {
                os: "macos".into(),
                ..hello()
            },
        ];
        for hello in &changed {
            assert_ne!(hello_payload("n", hello), base, "{hello:?}");
        }
        assert_ne!(hello_payload("m", &hello()), base);
    }

    #[test]
    fn fields_cannot_shift_into_each_other() {
        let a = Hello {
            name: "ab".into(),
            os: "c".into(),
            ..hello()
        };
        let b = Hello {
            name: "a".into(),
            os: "bc".into(),
            ..hello()
        };
        assert_ne!(hello_payload("n", &a), hello_payload("n", &b));
        let none = Hello {
            enrollment_code: None,
            ..hello()
        };
        let empty = Hello {
            enrollment_code: Some(String::new()),
            ..hello()
        };
        assert_ne!(hello_payload("n", &none), hello_payload("n", &empty));
    }
}
