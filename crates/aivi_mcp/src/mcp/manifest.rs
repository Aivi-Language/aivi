use std::collections::BTreeMap;
use std::path::Path;

use include_dir::{include_dir, Dir};
use serde::Serialize;

use aivi_driver::AiviError;

#[derive(Debug, Clone, Serialize, Default)]
pub struct McpManifest {
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub module: String,
    pub binding: String,
    pub input_schema: serde_json::Value,
    pub effectful: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpResource {
    pub name: String,
    pub module: String,
    pub binding: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct McpPolicy {
    pub allow_effectful_tools: bool,
}

static SPECS_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../specs");

pub fn specs_uri(binding: &str) -> String {
    format!("aivi://specs/{binding}")
}

fn normalize_spec_path(path: &Path) -> String {
    // Keep URIs stable across platforms.
    path.to_string_lossy().replace('\\', "/")
}

fn spec_mime_type(binding: &str) -> &'static str {
    if binding.ends_with(".md") {
        "text/markdown"
    } else if binding.ends_with(".aivi") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

fn spec_binding_from_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("aivi://specs/")
        .filter(|rest| !rest.is_empty())
}

pub fn read_bundled_spec(uri: &str) -> Result<(String, String), AiviError> {
    let binding = spec_binding_from_uri(uri)
        .ok_or_else(|| AiviError::InvalidCommand("invalid MCP resource uri".to_string()))?;
    let file = SPECS_DIR
        .get_file(binding)
        .ok_or_else(|| AiviError::InvalidCommand("unknown MCP resource uri".to_string()))?;
    let bytes = file.contents();
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AiviError::InvalidCommand("spec file is not valid UTF-8".to_string()))?
        .to_string();
    Ok((spec_mime_type(binding).to_string(), text))
}

pub fn bundled_specs_manifest() -> McpManifest {
    let mut resources_by_binding: BTreeMap<String, McpResource> = BTreeMap::new();

    fn visit_dir(dir: &Dir<'static>, resources_by_binding: &mut BTreeMap<String, McpResource>) {
        for file in dir.files() {
            let binding = normalize_spec_path(file.path());
            if !(binding.ends_with(".md") || binding.ends_with(".aivi")) {
                continue;
            }
            resources_by_binding
                .entry(binding.clone())
                .or_insert_with(|| McpResource {
                    name: format!("specs/{binding}"),
                    module: "specs".to_string(),
                    binding,
                });
        }
        for subdir in dir.dirs() {
            visit_dir(subdir, resources_by_binding);
        }
    }

    visit_dir(&SPECS_DIR, &mut resources_by_binding);

    McpManifest {
        tools: vec![
            McpTool {
                name: "aivi.parse".to_string(),
                description: "Parse AIVI source files and return syntax diagnostics.".to_string(),
                module: "aivi".to_string(),
                binding: "parse".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target"],
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Path/target glob accepted by AIVI driver commands."
                        }
                    }
                }),
                effectful: false,
            },
            McpTool {
                name: "aivi.check".to_string(),
                description: "Type-check AIVI source files and return diagnostics.".to_string(),
                module: "aivi".to_string(),
                binding: "check".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target"],
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Path/target glob accepted by AIVI driver commands."
                        },
                        "checkStdlib": {
                            "type": "boolean",
                            "description": "Include embedded stdlib diagnostics.",
                            "default": false
                        }
                    }
                }),
                effectful: false,
            },
            McpTool {
                name: "aivi.fmt".to_string(),
                description: "Format an AIVI source file and return the formatted output."
                    .to_string(),
                module: "aivi".to_string(),
                binding: "fmt".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target"],
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Single file target to format and return."
                        }
                    }
                }),
                effectful: false,
            },
            McpTool {
                name: "aivi.fmt.write".to_string(),
                description: "Format AIVI source files in place.".to_string(),
                module: "aivi".to_string(),
                binding: "fmt.write".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["target"],
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Path/target glob to format in place."
                        }
                    }
                }),
                effectful: true,
            },
        ],
        resources: resources_by_binding.into_values().collect(),
    }
}

pub fn bundled_specs_manifest_with_ui() -> McpManifest {
    let mut manifest = bundled_specs_manifest();
    manifest.tools.extend([
        McpTool {
            name: "aivi.gtk.discover".to_string(),
            description: "Discover running AIVI GTK application sessions.".to_string(),
            module: "gtk".to_string(),
            binding: "discover".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {}
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.attach".to_string(),
            description: "Attach to a running AIVI GTK application session.".to_string(),
            module: "gtk".to_string(),
            binding: "attach".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["socketPath", "token"],
                "properties": {
                    "socketPath": { "type": "string" },
                    "token": { "type": "string" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.launch".to_string(),
            description: "Launch an AIVI GTK application.".to_string(),
            module: "gtk".to_string(),
            binding: "launch".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["target"],
                "properties": {
                    "target": { "type": "string" },
                    "release": { "type": "boolean", "default": false }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.hello".to_string(),
            description: "Ping an attached GTK session to verify connectivity.".to_string(),
            module: "gtk".to_string(),
            binding: "hello".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.listWidgets".to_string(),
            description: "List inspectable widgets in an attached GTK session with dimensions and capabilities.".to_string(),
            module: "gtk".to_string(),
            binding: "listWidgets".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.listSignals".to_string(),
            description: "List reactive signals in an attached GTK session with revisions, dependencies, and watcher counts.".to_string(),
            module: "gtk".to_string(),
            binding: "listSignals".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.inspectWidget".to_string(),
            description: "Inspect one widget in an attached GTK session, including props, dimensions, and children.".to_string(),
            module: "gtk".to_string(),
            binding: "inspectWidget".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.inspectSignal".to_string(),
            description: "Inspect one reactive signal in an attached GTK session, including value summary, watchers, and dependencies.".to_string(),
            module: "gtk".to_string(),
            binding: "inspectSignal".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "signalId"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "signalId": { "type": "integer" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.dumpTree".to_string(),
            description: "Dump the live widget tree of an attached GTK session, including props and dimensions.".to_string(),
            module: "gtk".to_string(),
            binding: "dumpTree".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "rootId": { "type": "integer" }
                }
            }),
            effectful: false,
        },
        McpTool {
            name: "aivi.gtk.click".to_string(),
            description: "Simulate a click on a widget in an attached GTK session.".to_string(),
            module: "gtk".to_string(),
            binding: "click".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.type".to_string(),
            description: "Simulate typing text into a widget in an attached GTK session."
                .to_string(),
            module: "gtk".to_string(),
            binding: "type".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "text"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" },
                    "text": { "type": "string" }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.focus".to_string(),
            description: "Move keyboard focus onto a specific widget in an attached GTK session.".to_string(),
            module: "gtk".to_string(),
            binding: "focus".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.moveFocus".to_string(),
            description: "Move focus within the current GTK focus chain, including Tab/Shift-Tab style traversal.".to_string(),
            module: "gtk".to_string(),
            binding: "moveFocus".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "direction"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string", "description": "Optional widget or window anchor used to choose the focus container." },
                    "id": { "type": "integer", "description": "Optional widget or window anchor used to choose the focus container." },
                    "direction": {
                        "type": "string",
                        "enum": ["next", "previous", "up", "down", "left", "right", "tab", "shift-tab"],
                        "description": "Focus navigation direction. `next`/`tab` moves forward; `previous`/`shift-tab` moves backward."
                    }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.select".to_string(),
            description: "Select or set a value on a widget in an attached GTK session (for example a stack page, toggle state, dropdown item, or range value).".to_string(),
            module: "gtk".to_string(),
            binding: "select".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "value"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" },
                    "value": { "type": "string" }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.scroll".to_string(),
            description: "Scroll a GTK scrolled window in an attached session.".to_string(),
            module: "gtk".to_string(),
            binding: "scroll".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "direction"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" },
                    "direction": {
                        "type": "string",
                        "enum": ["up", "down", "left", "right"]
                    },
                    "amount": {
                        "type": "number",
                        "description": "Optional scroll delta in GTK adjustment units. Defaults to 40."
                    }
                }
            }),
            effectful: true,
        },
        McpTool {
            name: "aivi.gtk.keyPress".to_string(),
            description: "Inject a key press into an attached GTK session, defaulting to the current focused widget or the sole window when possible.".to_string(),
            module: "gtk".to_string(),
            binding: "keyPress".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["sessionId", "key"],
                "properties": {
                    "sessionId": { "type": "string" },
                    "name": { "type": "string" },
                    "id": { "type": "integer" },
                    "key": { "type": "string" },
                    "detail": { "type": "string", "description": "Optional extra detail or keycode text exposed as the fourth GtkKeyPressed field." }
                }
            }),
            effectful: true,
        },
    ]);
    manifest
}
