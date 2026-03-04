pub mod mcp {
    pub mod manifest;
    pub mod schema;
}

pub use mcp::manifest::{
    bundled_specs_manifest, bundled_specs_manifest_with_ui, McpManifest, McpPolicy, McpResource,
    McpTool,
};
pub use mcp::schema::{serve_mcp_stdio, serve_mcp_stdio_with_policy};
