//! The tools an assistant may call: what each one is, and what it does with the Zentrale.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::schemas::{browse_input, create_folder_input, draft_input, no_input, search_input};
use super::views::{self, DraftChange};
use crate::api::ApiError;
use crate::app::App;
use crate::browse::BrowseError;
use crate::live::view;
use crate::store::{Scope, ShareChange, StoreError, now};

/// What the server tells the assistant once, on `initialize`.
pub const INSTRUCTIONS: &str = "Stellwerk steers Syncthing on the user's devices from one \
server. Every folder is held in full on the server, which passes it on to the devices. To sync \
something the user names (\"sync Game of Thrones to my laptop\"): find the devices with \
list_devices; find the directory with search_device on the device that has the files (use \
browse_device to look around); create a folder with create_folder unless list_folders already \
has it; then build a draft where the device with the files sends (sendOnly, or sendReceive if \
changes should flow back) at the path you found, and the target receives (receiveOnly) or \
takes part (sendReceive), at a path the user chose or the default ~/Stellwerk/<folder>. Always \
call preview_shares and show its summary to the user, including the size, before apply_shares. \
Applying changes only configuration, never deletes files.";

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input: fn() -> Value,
    /// Needs the scope "manage".
    manages: bool,
    destructive: bool,
}

const TOOLS: [Tool; 7] = [
    Tool {
        name: "list_devices",
        title: "List devices",
        description: "The paired devices: ID, name, operating system, whether it is online \
                      or blocked, and when it was last seen.",
        input: no_input,
        manages: false,
        destructive: false,
    },
    Tool {
        name: "list_folders",
        title: "List folders",
        description: "The synced folders: size on the server (null until known), ignore \
                      patterns, and per device the mode, path, sync state and what it still \
                      lacks.",
        input: no_input,
        manages: false,
        destructive: false,
    },
    Tool {
        name: "browse_device",
        title: "Browse a device",
        description: "The subdirectories of one directory on an online device, its home \
                      directory and the file system roots (drives on Windows). Only \
                      directory names, never files.",
        input: browse_input,
        manages: false,
        destructive: false,
    },
    Tool {
        name: "search_device",
        title: "Search a device",
        description: "Directories on an online device whose name holds every word of the \
                      query as word prefixes (\"game of thrones\" finds Game.of.Thrones.S01), \
                      breadth first from the home directory or a given path, at most 8 levels \
                      deep and 50 matches. To search a disk outside the home directory, pass \
                      its path, e.g. /mnt/media or D:\\.",
        input: search_input,
        manages: false,
        destructive: false,
    },
    Tool {
        name: "preview_shares",
        title: "Preview shares",
        description: "Checks a draft of shares without applying it and describes what would \
                      happen, with the folder's size. Show this to the user before \
                      apply_shares.",
        input: draft_input,
        manages: false,
        destructive: false,
    },
    Tool {
        name: "create_folder",
        title: "Create a folder",
        description: "Creates a folder on the server, shared with no device yet; add devices \
                      with apply_shares.",
        input: create_folder_input,
        manages: true,
        destructive: false,
    },
    Tool {
        name: "apply_shares",
        title: "Apply shares",
        description: "Applies a draft of shares as one new revision: which device holds which \
                      folder, how and where. \"off\" stops syncing a folder on a device; its \
                      files stay. Call preview_shares first and get the user's consent.",
        input: draft_input,
        manages: true,
        destructive: true,
    },
];

/// The tools a token of this scope may call, as `tools/list` returns them.
pub fn list(scope: Scope) -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .filter(|tool| allowed(tool, scope))
        .map(|tool| {
            json!({
                "name": tool.name,
                "title": tool.title,
                "description": tool.description,
                "inputSchema": (tool.input)(),
                "annotations": {
                    "title": tool.title,
                    "readOnlyHint": !tool.manages,
                    "destructiveHint": tool.destructive,
                    "idempotentHint": tool.name != "create_folder",
                    "openWorldHint": false,
                },
            })
        })
        .collect();
    json!({ "tools": tools })
}

fn allowed(tool: &Tool, scope: Scope) -> bool {
    !tool.manages || scope == Scope::Manage
}

/// Why a call did not work: an unknown tool is a protocol error, anything else a result the
/// assistant reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    Unknown(String),
    Failed(String),
}

impl From<StoreError> for CallError {
    fn from(err: StoreError) -> Self {
        Self::Failed(ApiError::from(err).message)
    }
}

impl From<BrowseError> for CallError {
    fn from(err: BrowseError) -> Self {
        Self::Failed(err.to_string())
    }
}

type CallResult = Result<Value, CallError>;

/// Runs a tool for a caller of this scope.
pub async fn call(app: &App, scope: Scope, name: &str, args: Value) -> CallResult {
    let tool = TOOLS
        .iter()
        .find(|tool| tool.name == name)
        .ok_or_else(|| CallError::Unknown(name.to_string()))?;
    if !allowed(tool, scope) {
        return Err(CallError::Failed("this API token may only read".into()));
    }
    match name {
        "list_devices" => list_devices(app).await,
        "list_folders" => list_folders(app).await,
        "browse_device" => browse_device(app, arguments(args)?).await,
        "search_device" => search_device(app, arguments(args)?).await,
        "preview_shares" => preview_shares(app, arguments(args)?).await,
        "create_folder" => create_folder(app, arguments(args)?).await,
        "apply_shares" => apply_shares(app, arguments(args)?).await,
        other => Err(CallError::Unknown(other.to_string())),
    }
}

fn arguments<T: DeserializeOwned>(args: Value) -> Result<T, CallError> {
    let args = if args.is_null() { json!({}) } else { args };
    serde_json::from_value(args).map_err(|err| CallError::Failed(format!("arguments: {err}")))
}

fn to_value(value: impl serde::Serialize) -> CallResult {
    serde_json::to_value(value).map_err(|err| CallError::Failed(err.to_string()))
}

async fn list_devices(app: &App) -> CallResult {
    let config = app.store().config().await?;
    to_value(views::devices(&config, &app.online_devices()))
}

async fn list_folders(app: &App) -> CallResult {
    let config = app.store().config().await?;
    let live = view(&config, &app.live(), &app.conflicts());
    to_value(views::folders(&config, &live))
}

#[derive(Deserialize)]
struct OnDevice {
    device: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
}

async fn browse_device(app: &App, args: OnDevice) -> CallResult {
    let config = app.store().config().await?;
    let device = views::find_device(&config, &args.device).map_err(CallError::Failed)?;
    to_value(app.browse(&device.id, args.path).await?)
}

async fn search_device(app: &App, args: OnDevice) -> CallResult {
    if args.query.trim().is_empty() {
        return Err(CallError::Failed("the search needs a name".into()));
    }
    let config = app.store().config().await?;
    let device = views::find_device(&config, &args.device).map_err(CallError::Failed)?;
    to_value(app.search(&device.id, args.path, args.query).await?)
}

#[derive(Deserialize)]
struct NewFolder {
    label: String,
}

async fn create_folder(app: &App, args: NewFolder) -> CallResult {
    let folder = app.store().create_folder(&args.label, now()).await?;
    app.changed().await?;
    to_value(folder)
}

#[derive(Deserialize)]
struct Draft {
    changes: Vec<DraftChange>,
}

/// The planned changes and their description, or why the draft cannot be applied.
async fn planned(
    app: &App,
    draft: &Draft,
) -> Result<(Vec<views::PlannedChange>, Value), CallError> {
    let config = app.store().config().await?;
    let planned =
        views::plan(&config, &app.online_devices(), &draft.changes).map_err(CallError::Failed)?;
    let live = view(&config, &app.live(), &app.conflicts());
    let summary = views::describe(&planned, &live, &config);
    let described = json!({ "summary": summary, "changes": planned });
    Ok((planned, described))
}

async fn preview_shares(app: &App, draft: Draft) -> CallResult {
    Ok(planned(app, &draft).await?.1)
}

async fn apply_shares(app: &App, draft: Draft) -> CallResult {
    let (planned, mut described) = planned(app, &draft).await?;
    let changes: Vec<ShareChange> = planned
        .into_iter()
        .filter(views::PlannedChange::changes_something)
        .map(|planned| planned.change)
        .collect();
    let revision = if changes.is_empty() {
        app.store().revision().await?
    } else {
        let revision = app.store().apply_shares(&changes).await?;
        app.changed().await?;
        revision
    };
    described["revision"] = json!(revision);
    described["applied"] = json!(changes.len());
    Ok(described)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(scope: Scope) -> Vec<String> {
        list(scope)["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_read_token_sees_only_tools_that_read() {
        let read = names(Scope::Read);
        assert!(read.contains(&"search_device".to_string()));
        assert!(read.contains(&"preview_shares".to_string()));
        assert!(!read.contains(&"apply_shares".to_string()));
        assert!(!read.contains(&"create_folder".to_string()));
        assert_eq!(names(Scope::Manage).len(), TOOLS.len());
    }

    #[test]
    fn every_tool_says_what_it_takes_and_whether_it_changes_anything() {
        for tool in list(Scope::Manage)["tools"].as_array().unwrap() {
            assert_eq!(tool["inputSchema"]["type"], "object", "{}", tool["name"]);
            let annotations = &tool["annotations"];
            let changes = tool["name"] == "create_folder" || tool["name"] == "apply_shares";
            assert_eq!(annotations["readOnlyHint"], !changes, "{}", tool["name"]);
        }
        let apply = &list(Scope::Manage)["tools"][6];
        assert_eq!(apply["name"], "apply_shares");
        assert_eq!(apply["annotations"]["destructiveHint"], true);
    }

    #[test]
    fn arguments_are_checked() {
        let draft: Result<Draft, _> = arguments(json!({ "changes": [
            { "folder": "Serien", "device": "laptop", "mode": "receiveOnly" }
        ]}));
        assert_eq!(draft.unwrap().changes[0].device, "laptop");
        let wrong: Result<Draft, _> = arguments(json!({ "changes": [
            { "folder": "Serien", "device": "laptop", "mode": "upload" }
        ]}));
        assert!(matches!(wrong, Err(CallError::Failed(m)) if m.starts_with("arguments:")));
        let none: Result<OnDevice, _> = arguments(Value::Null);
        assert!(none.is_err());
    }
}
