//! The part of Syncthing's configuration that Stellwerk manages, as `GET /rest/config` returns
//! it and `PUT /rest/config/{devices,folders}/{id}` takes it.
//!
//! Field names follow lib/config/deviceconfiguration.go and lib/config/folderconfiguration.go.
//! Unknown fields are ignored when reading; on `PUT` Syncthing starts from its defaults
//! (lib/api/confighandler.go, `adjustFolder`/`adjustDevice`), so the fields left out here get
//! Syncthing's default values.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use stellwerk_protocol::{FolderMode, Versioning};

/// `devices[]` in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceConfig {
    #[serde(rename = "deviceID")]
    pub device_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub addresses: Vec<String>,
}

/// `folders[].devices[]`: one device a folder is shared with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderDevice {
    #[serde(rename = "deviceID")]
    pub device_id: String,
}

/// `folders[].type`, see lib/config/foldertype.go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FolderType {
    SendReceive,
    SendOnly,
    ReceiveOnly,
    ReceiveEncrypted,
}

impl From<FolderMode> for FolderType {
    fn from(mode: FolderMode) -> Self {
        match mode {
            FolderMode::SendReceive => Self::SendReceive,
            FolderMode::SendOnly => Self::SendOnly,
            FolderMode::ReceiveOnly => Self::ReceiveOnly,
        }
    }
}

/// `folders[].versioning`, see lib/config/versioningconfiguration.go. An empty type means
/// no versioning.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersioningConfig {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub params: BTreeMap<String, String>,
}

impl From<Versioning> for VersioningConfig {
    fn from(versioning: Versioning) -> Self {
        match versioning {
            Versioning::None => Self::default(),
            // lib/versioner/staggered.go reads `maxAge` in seconds
            Versioning::Staggered { max_age_days } => Self {
                kind: "staggered".into(),
                params: BTreeMap::from([(
                    "maxAge".into(),
                    (u64::from(max_age_days) * 24 * 60 * 60).to_string(),
                )]),
            },
        }
    }
}

/// `folders[]` in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderConfig {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub path: String,
    #[serde(rename = "type")]
    pub folder_type: FolderType,
    #[serde(default)]
    pub devices: Vec<FolderDevice>,
    #[serde(default)]
    pub versioning: VersioningConfig,
}

/// The managed part of `GET /rest/config`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub devices: Vec<DeviceConfig>,
    #[serde(default)]
    pub folders: Vec<FolderConfig>,
}

/// What a running Syncthing has: its own device ID and its configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actual {
    pub my_id: String,
    pub config: Config,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_rest_config_shape_and_ignores_the_rest() {
        let json = r#"{
            "version": 52,
            "devices": [{"deviceID": "AAA", "name": "a", "addresses": ["dynamic"], "paused": false}],
            "folders": [{"id": "f", "label": "F", "path": "/f", "type": "receiveonly",
                         "devices": [{"deviceID": "AAA", "introducedBy": ""}], "rescanIntervalS": 3600}],
            "options": {}
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.devices[0].addresses, ["dynamic"]);
        assert_eq!(config.folders[0].folder_type, FolderType::ReceiveOnly);
        assert_eq!(config.folders[0].devices[0].device_id, "AAA");
        assert_eq!(config.folders[0].versioning, VersioningConfig::default());
    }

    #[test]
    fn staggered_versioning_keeps_max_age_in_seconds() {
        let config = VersioningConfig::from(Versioning::Staggered { max_age_days: 30 });
        assert_eq!(config.kind, "staggered");
        assert_eq!(config.params["maxAge"], "2592000");
    }
}
