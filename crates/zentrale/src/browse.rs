//! The UI's folder picker: a directory on a device, asked of its agent over the open socket.

use std::collections::BTreeMap;
use std::time::Duration;

use stellwerk_protocol::messages::Directory;
use tokio::sync::{mpsc, oneshot};

/// How long an agent has to answer. One that never does is older than the picker (v0.1.5 and
/// before skip the request).
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BrowseError {
    #[error("the device is offline")]
    Offline,
    #[error("the device did not answer; update its agent to browse its folders")]
    NoAnswer,
    #[error("{0}")]
    Failed(String),
}

/// One request from the UI on its way to the connection that serves the device.
#[derive(Debug)]
pub struct Job {
    pub path: String,
    pub reply: oneshot::Sender<Result<Directory, String>>,
}

pub type Jobs = mpsc::Sender<Job>;

/// Asks the connection behind `jobs` for a directory and waits up to `timeout` for the
/// agent's answer.
pub async fn ask(
    jobs: Option<Jobs>,
    path: String,
    timeout: Duration,
) -> Result<Directory, BrowseError> {
    let jobs = jobs.ok_or(BrowseError::Offline)?;
    let (reply, answer) = oneshot::channel();
    jobs.send(Job { path, reply })
        .await
        .map_err(|_| BrowseError::Offline)?;
    match tokio::time::timeout(timeout, answer).await {
        Ok(Ok(result)) => result.map_err(BrowseError::Failed),
        Ok(Err(_)) => Err(BrowseError::Offline),
        Err(_) => Err(BrowseError::NoAnswer),
    }
}

/// The requests a connection has sent to its agent and not yet heard back on, by ID.
#[derive(Debug, Default)]
pub struct Pending {
    next: u64,
    waiting: BTreeMap<u64, oneshot::Sender<Result<Directory, String>>>,
}

impl Pending {
    /// Files the reply and returns the ID to send with the request. Replies whose asker gave
    /// up are dropped here, so an agent that never answers does not pile them up.
    pub fn add(&mut self, reply: oneshot::Sender<Result<Directory, String>>) -> u64 {
        self.waiting.retain(|_, waiting| !waiting.is_closed());
        self.next += 1;
        self.waiting.insert(self.next, reply);
        self.next
    }

    /// Hands the agent's answer to whoever asked; unknown IDs are ignored.
    pub fn answer(&mut self, id: u64, result: Result<Directory, String>) {
        if let Some(reply) = self.waiting.remove(&id) {
            let _ = reply.send(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUICK: Duration = Duration::from_millis(50);

    fn directory(path: &str) -> Directory {
        Directory {
            path: path.into(),
            parent: None,
            dirs: vec![],
            home: None,
            roots: vec!["/".into()],
        }
    }

    #[tokio::test]
    async fn the_answer_reaches_the_asker_by_id() {
        let (jobs, mut received) = mpsc::channel(1);
        let asked = tokio::spawn(ask(Some(jobs), "/home".into(), ANSWER_TIMEOUT));
        let mut pending = Pending::default();
        let first = received.recv().await.unwrap();
        assert_eq!(first.path, "/home");
        let id = pending.add(first.reply);
        let (other, _) = oneshot::channel();
        let other_id = pending.add(other);
        assert_ne!(id, other_id);

        pending.answer(999, Ok(directory("/elsewhere")));
        pending.answer(id, Ok(directory("/home")));
        assert_eq!(asked.await.unwrap(), Ok(directory("/home")));
    }

    #[tokio::test]
    async fn offline_failed_and_silent_agents_are_told_apart() {
        assert_eq!(
            ask(None, String::new(), QUICK).await,
            Err(BrowseError::Offline)
        );

        let (jobs, mut received) = mpsc::channel(1);
        let asked = tokio::spawn(ask(Some(jobs.clone()), String::new(), ANSWER_TIMEOUT));
        let job = received.recv().await.unwrap();
        job.reply.send(Err("permission denied".into())).unwrap();
        assert_eq!(
            asked.await.unwrap(),
            Err(BrowseError::Failed("permission denied".into()))
        );

        let asked = tokio::spawn(ask(Some(jobs), String::new(), QUICK));
        let _unanswered = received.recv().await.unwrap();
        assert_eq!(asked.await.unwrap(), Err(BrowseError::NoAnswer));
    }

    #[test]
    fn replies_nobody_waits_for_are_dropped() {
        let mut pending = Pending::default();
        let (gone, answer) = oneshot::channel();
        pending.add(gone);
        drop(answer);
        let (kept, _answer) = oneshot::channel();
        pending.add(kept);
        assert_eq!(pending.waiting.len(), 1);
    }
}
