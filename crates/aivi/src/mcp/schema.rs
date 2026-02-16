use std::io::{BufRead, Write};

fn jsonrpc_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn jsonrpc_result(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn handle_request(
    manifest: &McpManifest,
    policy: McpPolicy,
    message: &serde_json::Value,
) -> Option<serde_json::Value> {
    let method = message.get("method")?.as_str()?;
    let id = message.get("id")?.clone();

    let response = match method {
        "initialize" => jsonrpc_result(
            id,
            serde_json::json!({
                "serverInfo": { "name": "aivi", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": {
                    "tools": {},
                    "resources": {}
                }
            }),
        ),
        "tools/list" => jsonrpc_result(
            id,
            serde_json::json!({
                "tools": manifest.tools.iter().filter(|tool| policy.allow_effectful_tools || !tool.effectful).map(|tool| {
                    serde_json::json!({
                        "name": tool.name,
                        "description": null,
                        "inputSchema": tool.input_schema
                    })
                }).collect::<Vec<_>>()
            }),
        ),
        "resources/list" => jsonrpc_result(
            id,
            serde_json::json!({
                "resources": manifest.resources.iter().map(|res| {
                    serde_json::json!({
                        "name": res.name,
                        "description": null,
                        "uri": specs_uri(&res.binding)
                    })
                }).collect::<Vec<_>>()
            }),
        ),
        "resources/read" => {
            let uri = message
                .get("params")
                .and_then(|params| params.get("uri"))
                .and_then(|uri| uri.as_str());
            let Some(uri) = uri else {
                return Some(jsonrpc_error(id, -32602, "missing params.uri"));
            };

            match read_bundled_spec(uri) {
                Ok((mime_type, text)) => jsonrpc_result(
                    id,
                    serde_json::json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": mime_type,
                            "text": text
                        }]
                    }),
                ),
                Err(AiviError::InvalidCommand(_)) => jsonrpc_error(id, -32602, "invalid uri"),
                Err(_) => jsonrpc_error(id, -32603, "internal error"),
            }
        }
        _ => jsonrpc_error(id, -32601, "method not found"),
    };

    Some(response)
}

fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<serde_json::Value>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            if let Ok(len) = rest.trim().parse::<usize>() {
                content_length = Some(len);
            }
        }
    }
    let Some(len) = content_length else {
        return Ok(None);
    };
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    let message: serde_json::Value = serde_json::from_slice(&buf)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    Ok(Some(message))
}

fn write_message(mut out: impl Write, message: &serde_json::Value) -> std::io::Result<()> {
    let json = serde_json::to_vec(message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    write!(out, "Content-Length: {}\r\n\r\n", json.len())?;
    out.write_all(&json)?;
    out.flush()
}

pub fn serve_mcp_stdio(manifest: &McpManifest) -> Result<(), AiviError> {
    serve_mcp_stdio_with_policy(manifest, McpPolicy::default())
}

pub fn serve_mcp_stdio_with_policy(
    manifest: &McpManifest,
    policy: McpPolicy,
) -> Result<(), AiviError> {
    let stdin = std::io::stdin();
    let mut reader = std::io::BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    while let Some(message) = read_message(&mut reader)? {
        if let Some(response) = handle_request(manifest, policy, &message) {
            write_message(&mut out, &response)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_specs_manifest_lists_resources() {
        let manifest = bundled_specs_manifest();
        assert!(!manifest.resources.is_empty(), "expected bundled specs");
        assert!(
            manifest
                .resources
                .iter()
                .any(|res| res.binding == "02_syntax/14_decorators.md"),
            "expected 02_syntax/14_decorators.md to be bundled"
        );
    }

    #[test]
    fn resources_read_returns_markdown_text() {
        let uri = "aivi://specs/02_syntax/14_decorators.md";
        let (mime_type, text) = read_bundled_spec(uri).expect("read bundled spec");
        assert_eq!(mime_type, "text/markdown");
        assert!(text.contains("# Decorators"));
    }
}
