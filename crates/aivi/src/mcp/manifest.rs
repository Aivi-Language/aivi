use std::collections::BTreeMap;
use std::path::Path;

use include_dir::{include_dir, Dir};
use serde::Serialize;

use crate::AiviError;

#[derive(Debug, Clone, Serialize, Default)]
pub struct McpManifest {
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTool {
    pub name: String,
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

pub(crate) fn specs_uri(binding: &str) -> String {
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

pub(crate) fn read_bundled_spec(uri: &str) -> Result<(String, String), AiviError> {
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
