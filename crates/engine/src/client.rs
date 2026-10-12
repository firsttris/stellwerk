//! A small client for Syncthing's REST API (<https://docs.syncthing.net/dev/rest.html>).

use std::collections::BTreeMap;

use reqwest::{Method, RequestBuilder, Url};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::monitor::{CompletionReply, Event, FolderSummary};
use crate::syncthing_config::{Actual, Config, DeviceConfig, FolderConfig};

/// Where a Syncthing listens and the key that opens it.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: Url,
    api_key: String,
}

/// Without the API key, so that no log shows it.
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.base.as_str())
            .finish_non_exhaustive()
    }
}

/// One archived version of a file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    /// When it was archived, RFC 3339; passed back as is to restore it.
    pub version_time: String,
    pub mod_time: String,
    pub size: u64,
}

#[derive(Deserialize)]
struct Ignores {
    #[serde(default)]
    ignore: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SystemStatus {
    #[serde(rename = "myID")]
    my_id: String,
}

impl Client {
    /// `base` is the GUI address, e.g. `http://127.0.0.1:41384`.
    pub fn new(base: Url, api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            base,
            api_key: api_key.into(),
        }
    }

    /// `GET /rest/system/ping`
    pub async fn ping(&self) -> Result<()> {
        self.send(Method::GET, &["rest", "system", "ping"], |r| r)
            .await
            .map(drop)
    }

    /// The device ID from `GET /rest/system/status`.
    pub async fn my_id(&self) -> Result<String> {
        let status: SystemStatus = self.get_json(&["rest", "system", "status"]).await?;
        Ok(status.my_id)
    }

    /// The managed part of `GET /rest/config`.
    pub async fn config(&self) -> Result<Config> {
        self.get_json(&["rest", "config"]).await
    }

    /// The whole of `GET /rest/config`, untyped.
    pub async fn config_raw(&self) -> Result<serde_json::Value> {
        self.get_json(&["rest", "config"]).await
    }

    /// Own device ID and configuration, the input of [`crate::plan`].
    pub async fn actual(&self) -> Result<Actual> {
        let config = self.config().await?;
        let mut ignores = BTreeMap::new();
        for folder in &config.folders {
            ignores.insert(folder.id.clone(), self.ignores(&folder.id).await?);
        }
        Ok(Actual {
            my_id: self.my_id().await?,
            config,
            ignores,
        })
    }

    /// `GET /rest/db/ignores?folder={id}`: the lines of `.stignore`, empty without one.
    pub async fn ignores(&self, folder: &str) -> Result<Vec<String>> {
        let mut url = self.url(&["rest", "db", "ignores"]);
        url.query_pairs_mut().append_pair("folder", folder);
        let response = self.send_to(Method::GET, url, |r| r).await?;
        let ignores: Ignores = response.json().await?;
        Ok(ignores.ignore.unwrap_or_default())
    }

    /// `POST /rest/db/ignores?folder={id}`: replaces the folder's `.stignore`.
    pub async fn set_ignores(&self, folder: &str, lines: &[String]) -> Result<()> {
        let mut url = self.url(&["rest", "db", "ignores"]);
        url.query_pairs_mut().append_pair("folder", folder);
        let body = serde_json::json!({ "ignore": lines });
        self.send_to(Method::POST, url, |r| r.json(&body))
            .await
            .map(drop)
    }

    /// `PUT /rest/config/devices/{id}`
    pub async fn put_device(&self, device: &DeviceConfig) -> Result<()> {
        let path = ["rest", "config", "devices", &device.device_id];
        self.send(Method::PUT, &path, |r| r.json(device))
            .await
            .map(drop)
    }

    /// `DELETE /rest/config/devices/{id}`
    pub async fn delete_device(&self, id: &str) -> Result<()> {
        let path = ["rest", "config", "devices", id];
        self.send(Method::DELETE, &path, |r| r).await.map(drop)
    }

    /// `PUT /rest/config/folders/{id}`
    pub async fn put_folder(&self, folder: &FolderConfig) -> Result<()> {
        let path = ["rest", "config", "folders", &folder.id];
        self.send(Method::PUT, &path, |r| r.json(folder))
            .await
            .map(drop)
    }

    /// `DELETE /rest/config/folders/{id}`
    pub async fn delete_folder(&self, id: &str) -> Result<()> {
        let path = ["rest", "config", "folders", id];
        self.send(Method::DELETE, &path, |r| r).await.map(drop)
    }

    /// `POST /rest/db/scan?folder={id}`: rescan a folder now instead of waiting for the watcher.
    pub async fn scan(&self, folder: &str) -> Result<()> {
        let mut url = self.url(&["rest", "db", "scan"]);
        url.query_pairs_mut().append_pair("folder", folder);
        self.send_to(Method::POST, url, |r| r).await.map(drop)
    }

    /// `GET /rest/db/status?folder={id}`
    pub async fn folder_summary(&self, folder: &str) -> Result<FolderSummary> {
        let mut url = self.url(&["rest", "db", "status"]);
        url.query_pairs_mut().append_pair("folder", folder);
        Ok(self.send_to(Method::GET, url, |r| r).await?.json().await?)
    }

    /// `GET /rest/db/completion?folder={id}&device={id}`: how much of the folder the other
    /// device has, from its index, also while it is offline.
    pub async fn completion(&self, folder: &str, device: &str) -> Result<CompletionReply> {
        let mut url = self.url(&["rest", "db", "completion"]);
        url.query_pairs_mut()
            .append_pair("folder", folder)
            .append_pair("device", device);
        Ok(self.send_to(Method::GET, url, |r| r).await?.json().await?)
    }

    /// `GET /rest/events?since=…&events=…&timeout=…`: waits up to `timeout_secs` for events
    /// after `since`.
    pub async fn events(&self, since: u64, kinds: &str, timeout_secs: u32) -> Result<Vec<Event>> {
        let mut url = self.url(&["rest", "events"]);
        url.query_pairs_mut()
            .append_pair("since", &since.to_string())
            .append_pair("events", kinds)
            .append_pair("timeout", &timeout_secs.to_string());
        Ok(self.send_to(Method::GET, url, |r| r).await?.json().await?)
    }

    /// `GET /rest/folder/versions?folder={id}`: archived versions by file path
    /// (lib/versioner/versioner.go, `FileVersion`).
    pub async fn versions(&self, folder: &str) -> Result<BTreeMap<String, Vec<FileVersion>>> {
        let mut url = self.url(&["rest", "folder", "versions"]);
        url.query_pairs_mut().append_pair("folder", folder);
        Ok(self.send_to(Method::GET, url, |r| r).await?.json().await?)
    }

    /// `POST /rest/folder/versions?folder={id}`: puts the version back; Syncthing archives the
    /// current file first. `version_time` as the listing gave it. Returns errors by path.
    pub async fn restore_version(
        &self,
        folder: &str,
        path: &str,
        version_time: &str,
    ) -> Result<BTreeMap<String, String>> {
        let mut url = self.url(&["rest", "folder", "versions"]);
        url.query_pairs_mut().append_pair("folder", folder);
        let body = serde_json::json!({ path: version_time });
        Ok(self
            .send_to(Method::POST, url, |r| r.json(&body))
            .await?
            .json()
            .await?)
    }

    /// `POST /rest/system/shutdown`
    pub async fn shutdown(&self) -> Result<()> {
        self.send(Method::POST, &["rest", "system", "shutdown"], |r| r)
            .await
            .map(drop)
    }

    async fn get_json<T: DeserializeOwned>(&self, path: &[&str]) -> Result<T> {
        let response = self.send(Method::GET, path, |r| r).await?;
        Ok(response.json().await?)
    }

    async fn send(
        &self,
        method: Method,
        path: &[&str],
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
    ) -> Result<reqwest::Response> {
        self.send_to(method, self.url(path), build).await
    }

    async fn send_to(
        &self,
        method: Method,
        url: Url,
        build: impl FnOnce(RequestBuilder) -> RequestBuilder,
    ) -> Result<reqwest::Response> {
        let request = self
            .http
            .request(method.clone(), url.clone())
            .header("X-API-Key", &self.api_key);
        let response = build(request).send().await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        Err(Error::Api {
            method: method_name(&method),
            path: url.path().to_string(),
            status,
            body: response.text().await.unwrap_or_default(),
        })
    }

    /// Joins the segments onto the base, percent-encoding each (folder IDs are free text).
    fn url(&self, segments: &[&str]) -> Url {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .expect("an http base URL has path segments")
            .pop_if_empty()
            .extend(segments);
        url
    }
}

fn method_name(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::PUT => "PUT",
        Method::POST => "POST",
        Method::DELETE => "DELETE",
        _ => "request",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_ids_are_one_encoded_path_segment() {
        let client = Client::new("http://127.0.0.1:41384".parse().unwrap(), "key");
        let url = client.url(&["rest", "config", "folders", "a b/c"]);
        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:41384/rest/config/folders/a%20b%2Fc"
        );
    }

    #[test]
    fn debug_output_leaves_the_key_out() {
        let client = Client::new("http://127.0.0.1:41384".parse().unwrap(), "s3cr3t");
        let shown = format!("{client:?}");
        assert!(shown.contains("127.0.0.1:41384") && !shown.contains("s3cr3t"));
    }
}
