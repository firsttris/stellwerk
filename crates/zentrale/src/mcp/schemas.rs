//! The JSON Schemas of the tools' arguments, as `tools/list` shows them.

use serde_json::{Value, json};

pub fn no_input() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn device_property() -> Value {
    json!({ "type": "string", "description": "The device's ID or name." })
}

pub fn browse_input() -> Value {
    json!({
        "type": "object",
        "properties": {
            "device": device_property(),
            "path": {
                "type": "string",
                "description": "An absolute path; empty or left out for the home directory.",
            },
        },
        "required": ["device"],
        "additionalProperties": false,
    })
}

pub fn search_input() -> Value {
    json!({
        "type": "object",
        "properties": {
            "device": device_property(),
            "query": { "type": "string", "description": "A name, e.g. \"game of thrones\"." },
            "path": {
                "type": "string",
                "description": "Where to start, absolute; the home directory when left out.",
            },
        },
        "required": ["device", "query"],
        "additionalProperties": false,
    })
}

pub fn create_folder_input() -> Value {
    json!({
        "type": "object",
        "properties": {
            "label": { "type": "string", "description": "The folder's name, e.g. \"Game of Thrones\"." },
        },
        "required": ["label"],
        "additionalProperties": false,
    })
}

pub fn draft_input() -> Value {
    json!({
        "type": "object",
        "properties": {
            "changes": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "folder": { "type": "string", "description": "The folder's ID or name." },
                        "device": device_property(),
                        "mode": {
                            "type": "string",
                            "enum": ["sendReceive", "sendOnly", "receiveOnly", "off"],
                            "description": "sendReceive: both directions; sendOnly: the \
                                            device's changes go out, others' do not come in; \
                                            receiveOnly: the reverse; off: stop syncing it there.",
                        },
                        "path": {
                            "type": "string",
                            "description": "Where the folder is on the device. Left out: the \
                                            current path, or ~/Stellwerk/<folder> for a new one.",
                        },
                    },
                    "required": ["folder", "device", "mode"],
                    "additionalProperties": false,
                },
            },
        },
        "required": ["changes"],
        "additionalProperties": false,
    })
}
