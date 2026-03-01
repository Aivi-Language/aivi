use std::io::{BufRead, Write};

const MCP_TOOL_PARSE: &str = "aivi.parse";
const MCP_TOOL_CHECK: &str = "aivi.check";
const MCP_TOOL_FMT: &str = "aivi.fmt";
const MCP_TOOL_FMT_WRITE: &str = "aivi.fmt.write";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdioFraming {
    /// The MCP SDK's stdio transport typically uses newline-delimited JSON.
    Ndjson,
    /// LSP-style `Content-Length:` framing (kept for compatibility / tests).
    ContentLength,
}

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
        "initialize" => {
            // MCP clients commonly validate these fields. Keep the response small but standards-ish.
            let protocol_version = message
                .get("params")
                .and_then(|params| params.get("protocolVersion"))
                .and_then(|v| v.as_str())
                .unwrap_or("2024-11-05");
            jsonrpc_result(
                id,
                serde_json::json!({
                    "protocolVersion": protocol_version,
                    "serverInfo": { "name": "aivi", "version": env!("CARGO_PKG_VERSION") },
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "resources": { "listChanged": false }
                    }
                }),
            )
        }
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
        "tools/call" => {
            let params = message.get("params").and_then(|params| params.as_object());
            let Some(params) = params else {
                return Some(jsonrpc_error(id, -32602, "missing params"));
            };
            let Some(name) = params.get("name").and_then(|value| value.as_str()) else {
                return Some(jsonrpc_error(id, -32602, "missing params.name"));
            };
            let Some(tool) = manifest.tools.iter().find(|tool| tool.name == name) else {
                return Some(jsonrpc_error(id, -32602, "unknown tool"));
            };
            if tool.effectful && !policy.allow_effectful_tools {
                return Some(jsonrpc_error(
                    id,
                    -32602,
                    "effectful tool not allowed; restart with --allow-effects",
                ));
            }
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if !arguments.is_object() {
                return Some(jsonrpc_error(
                    id,
                    -32602,
                    "params.arguments must be an object",
                ));
            }

            match execute_tool(name, &arguments) {
                Ok(structured_content) => jsonrpc_result(
                    id,
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": format!("{} executed", name)
                        }],
                        "structuredContent": structured_content,
                        "isError": false
                    }),
                ),
                Err(err) => jsonrpc_result(
                    id,
                    serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": err.to_string()
                        }],
                        "structuredContent": {
                            "ok": false,
                            "tool": name,
                            "error": {
                                "code": aivi_error_code(&err),
                                "message": err.to_string()
                            }
                        },
                        "isError": true
                    }),
                ),
            }
        }
        _ => jsonrpc_error(id, -32601, "method not found"),
    };

    Some(response)
}

fn execute_tool(name: &str, arguments: &serde_json::Value) -> Result<serde_json::Value, AiviError> {
    match name {
        MCP_TOOL_PARSE => execute_parse_tool(arguments),
        MCP_TOOL_CHECK => execute_check_tool(arguments),
        MCP_TOOL_FMT => execute_fmt_tool(arguments),
        MCP_TOOL_FMT_WRITE => execute_fmt_write_tool(arguments),
        _ => Err(AiviError::InvalidCommand(format!("unknown tool {name}"))),
    }
}

fn execute_parse_tool(arguments: &serde_json::Value) -> Result<serde_json::Value, AiviError> {
    let args = parse_tool_args(arguments, &["target"])?;
    let target = get_required_string(args, "target")?;
    let bundle = crate::parse_target(target)?;

    let mut files = Vec::new();
    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    for file in bundle.files {
        let mut file_error_count = 0usize;
        let mut file_warning_count = 0usize;
        let diagnostics = file
            .diagnostics
            .iter()
            .map(|diag| {
                match diag.severity {
                    crate::DiagnosticSeverity::Error => file_error_count += 1,
                    crate::DiagnosticSeverity::Warning => file_warning_count += 1,
                }
                diagnostic_to_json(diag)
            })
            .collect::<Vec<_>>();
        error_count += file_error_count;
        warning_count += file_warning_count;
        files.push(serde_json::json!({
            "path": file.path,
            "byteCount": file.byte_count,
            "lineCount": file.line_count,
            "tokenCount": file.tokens.len(),
            "diagnostics": diagnostics,
            "summary": {
                "errors": file_error_count,
                "warnings": file_warning_count
            }
        }));
    }

    Ok(serde_json::json!({
        "ok": error_count == 0,
        "tool": MCP_TOOL_PARSE,
        "target": target,
        "summary": {
            "files": files.len(),
            "errors": error_count,
            "warnings": warning_count
        },
        "files": files
    }))
}

fn execute_check_tool(arguments: &serde_json::Value) -> Result<serde_json::Value, AiviError> {
    let args = parse_tool_args(arguments, &["target", "checkStdlib"])?;
    let target = get_required_string(args, "target")?;
    let check_stdlib = get_optional_bool(args, "checkStdlib", false)?;

    let pipeline = crate::Pipeline::from_target(target)?;
    let mut diagnostics = pipeline.parse_diagnostics().to_vec();
    diagnostics.extend(crate::check_modules(pipeline.modules()));
    if !crate::file_diagnostics_have_errors(&diagnostics) {
        if check_stdlib {
            diagnostics.extend(crate::check_types_including_stdlib(pipeline.modules()));
        } else {
            diagnostics.extend(crate::check_types(pipeline.modules()));
        }
    }
    if !check_stdlib {
        diagnostics.retain(|diag| !diag.path.starts_with("<embedded:"));
    }

    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let diagnostics = diagnostics
        .iter()
        .map(|diag| {
            match diag.diagnostic.severity {
                crate::DiagnosticSeverity::Error => error_count += 1,
                crate::DiagnosticSeverity::Warning => warning_count += 1,
            }
            file_diagnostic_to_json(diag)
        })
        .collect::<Vec<_>>();

    let diagnostic_count = diagnostics.len();
    Ok(serde_json::json!({
        "ok": error_count == 0,
        "tool": MCP_TOOL_CHECK,
        "target": target,
        "checkStdlib": check_stdlib,
        "summary": {
            "diagnostics": diagnostic_count,
            "errors": error_count,
            "warnings": warning_count
        },
        "diagnostics": diagnostics
    }))
}

fn execute_fmt_tool(arguments: &serde_json::Value) -> Result<serde_json::Value, AiviError> {
    let args = parse_tool_args(arguments, &["target"])?;
    let target = get_required_string(args, "target")?;
    let paths = crate::resolve_target(target)?;
    if paths.len() != 1 {
        return Err(AiviError::InvalidCommand(
            "fmt expects a single file path".to_string(),
        ));
    }
    let path = &paths[0];
    let source = std::fs::read_to_string(path)?;
    let formatted = crate::format_text(&source);

    Ok(serde_json::json!({
        "ok": true,
        "tool": MCP_TOOL_FMT,
        "target": target,
        "path": path.display().to_string(),
        "changed": formatted != source,
        "formatted": formatted
    }))
}

fn execute_fmt_write_tool(arguments: &serde_json::Value) -> Result<serde_json::Value, AiviError> {
    let args = parse_tool_args(arguments, &["target"])?;
    let target = get_required_string(args, "target")?;
    let paths = crate::resolve_target(target)?;

    let mut processed_files = Vec::new();
    let mut changed_files = Vec::new();

    for path in paths {
        if path.extension().and_then(|s| s.to_str()) != Some("aivi") {
            continue;
        }
        let path_text = path.display().to_string();
        let source = std::fs::read_to_string(&path)?;
        let formatted = crate::format_text(&source);
        processed_files.push(path_text.clone());
        if formatted != source {
            std::fs::write(&path, formatted)?;
            changed_files.push(path_text);
        }
    }

    let processed_count = processed_files.len();
    let changed_count = changed_files.len();
    Ok(serde_json::json!({
        "ok": true,
        "tool": MCP_TOOL_FMT_WRITE,
        "target": target,
        "processedFiles": processed_files,
        "changedFiles": changed_files,
        "summary": {
            "processed": processed_count,
            "changed": changed_count
        }
    }))
}

fn parse_tool_args<'a>(
    arguments: &'a serde_json::Value,
    allowed_keys: &[&str],
) -> Result<&'a serde_json::Map<String, serde_json::Value>, AiviError> {
    let Some(args) = arguments.as_object() else {
        return Err(AiviError::InvalidCommand(
            "tool arguments must be an object".to_string(),
        ));
    };
    for key in args.keys() {
        if !allowed_keys.iter().any(|allowed| key == *allowed) {
            return Err(AiviError::InvalidCommand(format!(
                "unknown tool argument {key}"
            )));
        }
    }
    Ok(args)
}

fn get_required_string<'a>(
    args: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<&'a str, AiviError> {
    let Some(value) = args.get(key) else {
        return Err(AiviError::InvalidCommand(format!(
            "missing required argument {key}"
        )));
    };
    value
        .as_str()
        .ok_or_else(|| AiviError::InvalidCommand(format!("argument {key} must be a string")))
}

fn get_optional_bool(
    args: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: bool,
) -> Result<bool, AiviError> {
    match args.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_bool()
            .ok_or_else(|| AiviError::InvalidCommand(format!("argument {key} must be a boolean"))),
    }
}

fn aivi_error_code(err: &AiviError) -> &'static str {
    match err {
        AiviError::Io(_) => "io",
        AiviError::InvalidPath(_) => "invalid_path",
        AiviError::Diagnostics => "diagnostics",
        AiviError::InvalidCommand(_) => "invalid_command",
        AiviError::Codegen(_) => "codegen",
        AiviError::Wasm(_) => "wasm",
        AiviError::Runtime(_) => "runtime",
        AiviError::Config(_) => "config",
        AiviError::Cargo(_) => "cargo",
    }
}

fn diagnostic_to_json(diagnostic: &crate::Diagnostic) -> serde_json::Value {
    let severity = match diagnostic.severity {
        crate::DiagnosticSeverity::Error => "error",
        crate::DiagnosticSeverity::Warning => "warning",
    };
    serde_json::json!({
        "code": diagnostic.code.clone(),
        "severity": severity,
        "message": diagnostic.message.clone(),
        "span": span_to_json(&diagnostic.span),
        "labels": diagnostic.labels.iter().map(|label| serde_json::json!({
            "message": label.message.clone(),
            "span": span_to_json(&label.span)
        })).collect::<Vec<_>>()
    })
}

fn file_diagnostic_to_json(file_diagnostic: &crate::FileDiagnostic) -> serde_json::Value {
    serde_json::json!({
        "path": file_diagnostic.path.clone(),
        "diagnostic": diagnostic_to_json(&file_diagnostic.diagnostic)
    })
}

fn span_to_json(span: &crate::Span) -> serde_json::Value {
    serde_json::json!({
        "start": position_to_json(&span.start),
        "end": position_to_json(&span.end)
    })
}

fn position_to_json(position: &crate::Position) -> serde_json::Value {
    serde_json::json!({
        "line": position.line,
        "column": position.column
    })
}

fn detect_stdio_framing(reader: &mut impl BufRead) -> std::io::Result<Option<StdioFraming>> {
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok(None);
        }
        let mut idx = 0;
        while idx < buf.len() {
            match buf[idx] {
                b' ' | b'\t' | b'\r' | b'\n' => idx += 1,
                b'{' => return Ok(Some(StdioFraming::Ndjson)),
                _ => return Ok(Some(StdioFraming::ContentLength)),
            }
        }
        // Buffer contained only whitespace; consume and keep looking.
        reader.consume(idx);
    }
}

fn read_message_content_length(
    reader: &mut impl BufRead,
) -> std::io::Result<Option<serde_json::Value>> {
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

fn write_message_content_length(
    mut out: impl Write,
    message: &serde_json::Value,
) -> std::io::Result<()> {
    let json = serde_json::to_vec(message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    write!(out, "Content-Length: {}\r\n\r\n", json.len())?;
    out.write_all(&json)?;
    out.flush()
}

fn read_message_ndjson(reader: &mut impl BufRead) -> std::io::Result<Option<serde_json::Value>> {
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let message: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        return Ok(Some(message));
    }
}

fn write_message_ndjson(mut out: impl Write, message: &serde_json::Value) -> std::io::Result<()> {
    serde_json::to_writer(&mut out, message)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    out.write_all(b"\n")?;
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

    // Prefer the standard MCP stdio framing (NDJSON), but keep Content-Length framing for
    // compatibility with earlier experiments/tests.
    let Some(framing) = detect_stdio_framing(&mut reader)? else {
        return Ok(());
    };
    match framing {
        StdioFraming::Ndjson => {
            while let Some(message) = read_message_ndjson(&mut reader)? {
                if let Some(response) = handle_request(manifest, policy, &message) {
                    write_message_ndjson(&mut out, &response)?;
                }
            }
        }
        StdioFraming::ContentLength => {
            while let Some(message) = read_message_content_length(&mut reader)? {
                if let Some(response) = handle_request(manifest, policy, &message) {
                    write_message_content_length(&mut out, &response)?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn bundled_specs_manifest_lists_resources() {
        let manifest = bundled_specs_manifest();
        assert!(!manifest.resources.is_empty(), "expected bundled specs");
        assert!(!manifest.tools.is_empty(), "expected bundled tools");
        assert!(
            manifest
                .resources
                .iter()
                .any(|res| res.binding == "syntax/decorators.md"),
            "expected syntax/decorators.md to be bundled"
        );
        // Verify resource structure: each resource should have a non-empty binding and URI
        for res in &manifest.resources {
            assert!(!res.binding.is_empty(), "resource binding must not be empty");
        }
    }

    #[test]
    fn resources_read_returns_markdown_text() {
        let uri = "aivi://specs/syntax/decorators.md";
        let (mime_type, text) = read_bundled_spec(uri).expect("read bundled spec");
        assert_eq!(mime_type, "text/markdown");
        assert!(text.contains("# Decorators"), "should contain heading");
        assert!(text.len() > 100, "spec content should be non-trivial, got {} bytes", text.len());
    }

    #[test]
    fn tools_list_filters_effectful_tools() {
        let manifest = bundled_specs_manifest();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list"
        });
        let default_response = handle_request(&manifest, McpPolicy::default(), &request)
            .expect("tools/list default response");
        let default_tools = default_response
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(|tools| tools.as_array())
            .expect("tools array");
        let default_names = default_tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
            .collect::<Vec<_>>();
        assert!(default_names.contains(&MCP_TOOL_PARSE));
        assert!(default_names.contains(&MCP_TOOL_CHECK));
        assert!(default_names.contains(&MCP_TOOL_FMT));
        assert!(!default_names.contains(&MCP_TOOL_FMT_WRITE));

        let effectful_response = handle_request(
            &manifest,
            McpPolicy {
                allow_effectful_tools: true,
            },
            &request,
        )
        .expect("tools/list effectful response");
        let effectful_tools = effectful_response
            .get("result")
            .and_then(|result| result.get("tools"))
            .and_then(|tools| tools.as_array())
            .expect("tools array");
        let effectful_names = effectful_tools
            .iter()
            .filter_map(|tool| tool.get("name").and_then(|name| name.as_str()))
            .collect::<Vec<_>>();
        assert!(effectful_names.contains(&MCP_TOOL_FMT_WRITE));
    }

    #[test]
    fn tools_call_fmt_returns_structured_content() {
        let dir = tempdir().expect("create tempdir");
        let source_path = dir.path().join("fmt.aivi");
        std::fs::write(
            &source_path,
            "@no_prelude\nmodule integrationTests.mcp.fmt\n\nx=1\n",
        )
        .expect("write source");

        let manifest = bundled_specs_manifest();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": MCP_TOOL_FMT,
                "arguments": {
                    "target": source_path.display().to_string()
                }
            }
        });

        let response =
            handle_request(&manifest, McpPolicy::default(), &request).expect("tools/call response");
        assert_eq!(
            response
                .get("result")
                .and_then(|result| result.get("isError"))
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(
            response
                .get("result")
                .and_then(|result| result.get("structuredContent"))
                .and_then(|content| content.get("tool"))
                .and_then(|tool| tool.as_str()),
            Some(MCP_TOOL_FMT)
        );
        assert_eq!(
            response
                .get("result")
                .and_then(|result| result.get("structuredContent"))
                .and_then(|content| content.get("changed"))
                .and_then(|changed| changed.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn tools_call_effectful_write_requires_policy() {
        let dir = tempdir().expect("create tempdir");
        let source_path = dir.path().join("write.aivi");
        std::fs::write(
            &source_path,
            "@no_prelude\nmodule integrationTests.mcp.write\n\nx=1\n",
        )
        .expect("write source");

        let manifest = bundled_specs_manifest();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": MCP_TOOL_FMT_WRITE,
                "arguments": {
                    "target": source_path.display().to_string()
                }
            }
        });

        let denied = handle_request(&manifest, McpPolicy::default(), &request)
            .expect("effectful denied response");
        assert_eq!(
            denied
                .get("error")
                .and_then(|error| error.get("code"))
                .and_then(|code| code.as_i64()),
            Some(-32602)
        );

        let allowed = handle_request(
            &manifest,
            McpPolicy {
                allow_effectful_tools: true,
            },
            &request,
        )
        .expect("effectful allowed response");
        assert_eq!(
            allowed
                .get("result")
                .and_then(|result| result.get("isError"))
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        let updated = std::fs::read_to_string(&source_path).expect("read updated source");
        assert!(updated.contains("x = 1"));
    }
}
