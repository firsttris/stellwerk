//! Questions for a device's agent over its open socket: a directory for the UI's folder
//! picker, or a search for directories by name.

use std::collections::BTreeMap;
use std::time::Duration;

use stellwerk_protocol::messages::{Directory, Matches, ServerMessage};
use tokio::sync::{mpsc, oneshot};

/// How long an agent has to list a directory. One that never answers is older than the picker
/// (v0.1.5 and before skip the request).
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(5);

/// How long an agent has to search: its own limit (`crates/agent/src/search.rs`) plus room for
/// the way there and back.
pub const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BrowseError {
    #[error("the device is offline")]
    Offline,
    #[error("the device did not answer; update its agent")]
    NoAnswer,
    #[error("{0}")]
    Failed(String),
}

/// What is asked of the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Browse { path: String },
    Search { path: String, query: String },
}

impl Request {
    /// The message that carries the request to the agent under `id`.
    pub fn message(self, id: u64) -> ServerMessage {
        match self {
            Self::Browse { path } => ServerMessage::Browse { id, path },
            Self::Search { path, query } => ServerMessage::Search { id, path, query },
        }
    }
}

/// The agent's answer, or why it could not give one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Listing(Directory),
    Matches(Matches),
}

pub type Reply = oneshot::Sender<Result<Answer, String>>;

/// One request on its way to the connection that serves the device.
#[derive(Debug)]
pub struct Job {
    pub request: Request,
    pub reply: Reply,
}

pub type Jobs = mpsc::Sender<Job>;

/// Asks the connection behind `jobs` and waits up to `timeout` for the agent's answer.
pub async fn ask(
    jobs: Option<Jobs>,
    request: Request,
    timeout: Duration,
) -> Result<Answer, BrowseError> {
    let jobs = jobs.ok_or(BrowseError::Offline)?;
    let (reply, answer) = oneshot::channel();
    jobs.send(Job { request, reply })
        .await
        .map_err(|_| BrowseError::Offline)?;
    match tokio::time::timeout(timeout, answer).await {
        Ok(Ok(result)) => result.map_err(BrowseError::Failed),
        Ok(Err(_)) => Err(BrowseError::Offline),
        Err(_) => Err(BrowseError::NoAnswer),
    }
}

/// The directory from a browse answer.
pub fn listing(answer: Answer) -> Result<Directory, BrowseError> {
    match answer {
        Answer::Listing(directory) => Ok(directory),
        Answer::Matches(_) => Err(unexpected()),
    }
}

/// The directories from a search answer.
pub fn matches(answer: Answer) -> Result<Matches, BrowseError> {
    match answer {
        Answer::Matches(matches) => Ok(matches),
        Answer::Listing(_) => Err(unexpected()),
    }
}

fn unexpected() -> BrowseError {
    BrowseError::Failed("the device answered something else".into())
}

/// The requests a connection has sent to its agent and not yet heard back on, by ID.
#[derive(Debug, Default)]
pub struct Pending {
    next: u64,
    waiting: BTreeMap<u64, Reply>,
}

impl Pending {
    /// Files the reply and returns the ID to send with the request. Replies whose asker gave
    /// up are dropped here, so an agent that never answers does not pile them up.
    pub fn add(&mut self, reply: Reply) -> u64 {
        self.waiting.retain(|_, waiting| !waiting.is_closed());
        self.next += 1;
        self.waiting.insert(self.next, reply);
        self.next
    }

    /// Hands the agent's answer to whoever asked; unknown IDs are ignored.
    pub fn answer(&mut self, id: u64, result: Result<Answer, String>) {
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

    fn browse(path: &str) -> Request {
        Request::Browse { path: path.into() }
    }

    #[tokio::test]
    async fn the_answer_reaches_the_asker_by_id() {
        let (jobs, mut received) = mpsc::channel(1);
        let asked = tokio::spawn(ask(Some(jobs), browse("/home"), ANSWER_TIMEOUT));
        let mut pending = Pending::default();
        let first = received.recv().await.unwrap();
        assert_eq!(first.request, browse("/home"));
        let id = pending.add(first.reply);
        let (other, _) = oneshot::channel();
        let other_id = pending.add(other);
        assert_ne!(id, other_id);

        pending.answer(999, Ok(Answer::Listing(directory("/elsewhere"))));
        pending.answer(id, Ok(Answer::Listing(directory("/home"))));
        let answer = asked.await.unwrap().unwrap();
        assert_eq!(listing(answer), Ok(directory("/home")));
    }

    #[test]
    fn an_answer_of_the_wrong_kind_is_an_error() {
        let found = Answer::Matches(Matches {
            root: "/".into(),
            dirs: vec![],
            truncated: false,
        });
        assert!(listing(found.clone()).is_err());
        assert!(matches(found).is_ok());
        assert!(matches(Answer::Listing(directory("/"))).is_err());
    }

    #[test]
    fn requests_become_messages_under_their_id() {
        let search = Request::Search {
            path: String::new(),
            query: "got".into(),
        };
        assert_eq!(
            search.message(4),
            ServerMessage::Search {
                id: 4,
                path: String::new(),
                query: "got".into()
            }
        );
    }

    #[tokio::test]
    async fn offline_failed_and_silent_agents_are_told_apart() {
        assert_eq!(
            ask(None, browse(""), QUICK).await,
            Err(BrowseError::Offline)
        );

        let (jobs, mut received) = mpsc::channel(1);
        let asked = tokio::spawn(ask(Some(jobs.clone()), browse(""), ANSWER_TIMEOUT));
        let job = received.recv().await.unwrap();
        job.reply.send(Err("permission denied".into())).unwrap();
        assert_eq!(
            asked.await.unwrap(),
            Err(BrowseError::Failed("permission denied".into()))
        );

        let asked = tokio::spawn(ask(Some(jobs), browse(""), QUICK));
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
