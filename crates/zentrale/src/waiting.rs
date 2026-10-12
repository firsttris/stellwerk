//! Agents that came without a code and wait for approval in the UI (LAN pairing).
//!
//! Kept in memory only while they stay connected: a waiting agent that disconnects is
//! forgotten and shows up again when it reconnects. Keys rejected in the UI are turned away at
//! once until the Zentrale restarts, so a rejected agent does not keep coming back.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use stellwerk_protocol::messages::fingerprint;
use tokio::sync::oneshot;

use crate::store::NewDevice;

/// More waiting agents than this are turned away; the list is for the few devices of one
/// household, not for whoever can reach the port.
pub const MAX_WAITING: usize = 20;

/// The answer from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Reject,
}

/// A waiting agent as the UI shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitingView {
    pub fingerprint: String,
    pub name: String,
    pub os: String,
    pub syncthing_id: String,
    pub since: i64,
}

#[derive(Debug)]
struct Entry {
    device: NewDevice,
    since: i64,
    ticket: u64,
    decide: oneshot::Sender<Decision>,
}

/// Why an agent may not wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    Rejected,
    Full,
}

impl Refused {
    pub fn reason(self) -> &'static str {
        match self {
            Refused::Rejected => "this device was rejected in the UI",
            Refused::Full => "too many devices are waiting for approval",
        }
    }
}

/// A place in the list: the fingerprint to show, the ticket to leave with, and where the
/// decision arrives.
#[derive(Debug)]
pub struct Place {
    pub fingerprint: String,
    pub ticket: u64,
    pub decision: oneshot::Receiver<Decision>,
}

/// The waiting agents by fingerprint.
#[derive(Debug, Default)]
pub struct Waiting {
    entries: BTreeMap<String, Entry>,
    rejected: BTreeSet<String>,
    tickets: u64,
}

impl Waiting {
    /// Adds an agent. The same key connecting again replaces its older place, whose session
    /// then ends.
    pub fn join(&mut self, device: NewDevice, since: i64) -> Result<Place, Refused> {
        let fingerprint = fingerprint(&device.public_key);
        if self.rejected.contains(&fingerprint) {
            return Err(Refused::Rejected);
        }
        if !self.entries.contains_key(&fingerprint) && self.entries.len() >= MAX_WAITING {
            return Err(Refused::Full);
        }
        self.tickets += 1;
        let (decide, decision) = oneshot::channel();
        let entry = Entry {
            device,
            since,
            ticket: self.tickets,
            decide,
        };
        self.entries.insert(fingerprint.clone(), entry);
        Ok(Place {
            fingerprint,
            ticket: self.tickets,
            decision,
        })
    }

    /// Removes a place when its agent disconnects, unless a newer connection took it over.
    /// Returns whether something was removed.
    pub fn leave(&mut self, fingerprint: &str, ticket: u64) -> bool {
        let current = self
            .entries
            .get(fingerprint)
            .is_some_and(|entry| entry.ticket == ticket);
        if current {
            self.entries.remove(fingerprint);
        }
        current
    }

    /// Takes an agent out of the list to decide about it: what it reported, and the channel
    /// that tells its session. A rejected key is remembered.
    pub fn decide(
        &mut self,
        fingerprint: &str,
        decision: Decision,
    ) -> Option<(NewDevice, oneshot::Sender<Decision>)> {
        let entry = self.entries.remove(fingerprint)?;
        if decision == Decision::Reject {
            self.rejected.insert(fingerprint.to_string());
        }
        Some((entry.device, entry.decide))
    }

    pub fn views(&self) -> Vec<WaitingView> {
        self.entries
            .iter()
            .map(|(fingerprint, entry)| WaitingView {
                fingerprint: fingerprint.clone(),
                name: entry.device.name.clone(),
                os: entry.device.os.clone(),
                syncthing_id: entry.device.syncthing_id.clone(),
                since: entry.since,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DeviceKind;

    fn device(key: &str) -> NewDevice {
        NewDevice {
            name: format!("pc-{key}"),
            kind: DeviceKind::Desktop,
            os: "linux".into(),
            public_key: key.into(),
            syncthing_id: "ST".into(),
            agent_version: "0.1.0".into(),
            syncthing_version: "v2".into(),
        }
    }

    #[test]
    fn an_agent_waits_until_it_leaves() {
        let mut waiting = Waiting::default();
        let place = waiting.join(device("a"), 10).unwrap();
        assert_eq!(place.fingerprint, fingerprint("a"));
        assert_eq!(waiting.views().len(), 1);
        assert_eq!(waiting.views()[0].name, "pc-a");
        assert!(waiting.leave(&place.fingerprint, place.ticket));
        assert!(waiting.views().is_empty());
    }

    #[test]
    fn a_reconnect_replaces_the_older_place() {
        let mut waiting = Waiting::default();
        let old = waiting.join(device("a"), 10).unwrap();
        let new = waiting.join(device("a"), 20).unwrap();
        assert!(!waiting.leave(&old.fingerprint, old.ticket));
        assert_eq!(waiting.views()[0].since, 20);
        assert!(waiting.leave(&new.fingerprint, new.ticket));
    }

    #[test]
    fn a_decision_reaches_the_session() {
        let mut waiting = Waiting::default();
        let mut place = waiting.join(device("a"), 10).unwrap();
        let (reported, decide) = waiting
            .decide(&place.fingerprint, Decision::Approve)
            .unwrap();
        assert_eq!(reported.public_key, "a");
        decide.send(Decision::Approve).unwrap();
        assert_eq!(place.decision.try_recv(), Ok(Decision::Approve));
        assert!(waiting.views().is_empty());
        assert!(
            waiting
                .decide(&place.fingerprint, Decision::Approve)
                .is_none()
        );
    }

    #[test]
    fn a_rejected_key_is_turned_away() {
        let mut waiting = Waiting::default();
        let place = waiting.join(device("a"), 10).unwrap();
        waiting
            .decide(&place.fingerprint, Decision::Reject)
            .unwrap();
        assert_eq!(
            waiting.join(device("a"), 20).unwrap_err(),
            Refused::Rejected
        );
        assert!(waiting.join(device("b"), 20).is_ok());
    }

    #[test]
    fn the_list_is_capped() {
        let mut waiting = Waiting::default();
        for i in 0..MAX_WAITING {
            waiting.join(device(&i.to_string()), 0).unwrap();
        }
        assert_eq!(waiting.join(device("new"), 0).unwrap_err(), Refused::Full);
        // A known agent reconnecting keeps its place.
        assert!(waiting.join(device("0"), 1).is_ok());
    }
}
