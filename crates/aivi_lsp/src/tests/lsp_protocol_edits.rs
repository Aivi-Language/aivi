mod lsp_protocol_edits {
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::{json, Value};
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio::time::{timeout, Duration};
    use tower_lsp::lsp_types::{DiagnosticSeverity, Url};
    use tower_lsp::{LspService, Server};

    use crate::backend::Backend;
    use crate::state::BackendState;

    async fn write_lsp_msg(mut w: impl AsyncWrite + Unpin, value: &Value) {
        let body = serde_json::to_vec(value).expect("json encode");
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        w.write_all(header.as_bytes()).await.expect("write header");
        w.write_all(&body).await.expect("write body");
        w.flush().await.expect("flush");
    }

    async fn read_lsp_msg(mut r: impl AsyncRead + Unpin) -> Value {
        let mut header_bytes = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            r.read_exact(&mut buf).await.expect("read header byte");
            header_bytes.push(buf[0]);
            if header_bytes.ends_with(b"\r\n\r\n") {
                break;
            }
            assert!(header_bytes.len() < 16 * 1024, "LSP header too large");
        }
        let header = String::from_utf8_lossy(&header_bytes);
        let mut content_len: Option<usize> = None;
        for line in header.split("\r\n") {
            let Some((k, v)) = line.split_once(':') else {
                continue;
            };
            if k.eq_ignore_ascii_case("content-length") {
                content_len = Some(v.trim().parse::<usize>().expect("content-length"));
            }
        }
        let len = content_len.expect("missing content-length");

        let mut body = vec![0u8; len];
        r.read_exact(&mut body).await.expect("read body");
        serde_json::from_slice(&body).expect("json decode")
    }

    async fn wait_for_response_id(reader: &mut (impl AsyncRead + Unpin), id: i64) -> Value {
        loop {
            let msg = read_lsp_msg(&mut *reader).await;
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
        }
    }

    async fn wait_for_publish_diagnostics(
        reader: &mut (impl AsyncRead + Unpin),
        target_uri: &str,
        version: Option<i64>,
    ) -> Vec<Value> {
        loop {
            let msg = read_lsp_msg(&mut *reader).await;
            let Some(method) = msg.get("method").and_then(Value::as_str) else {
                continue;
            };
            if method != "textDocument/publishDiagnostics" {
                continue;
            }
            let Some(params) = msg.get("params") else {
                continue;
            };
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                continue;
            };
            if uri != target_uri {
                continue;
            }
            if let Some(expected_version) = version {
                let got = params.get("version").and_then(Value::as_i64);
                if got != Some(expected_version) {
                    continue;
                }
            }
            return params
                .get("diagnostics")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
        }
    }

    async fn wait_for_publish_diagnostics_and_log_message(
        reader: &mut (impl AsyncRead + Unpin),
        target_uri: &str,
        version: Option<i64>,
        log_fragment: &str,
    ) -> (Vec<Value>, Value) {
        let mut diagnostics = None;
        let mut log_message = None;
        loop {
            let msg = read_lsp_msg(&mut *reader).await;
            match msg.get("method").and_then(Value::as_str) {
                Some("textDocument/publishDiagnostics") => {
                    let Some(params) = msg.get("params") else {
                        continue;
                    };
                    let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                        continue;
                    };
                    if uri != target_uri {
                        continue;
                    }
                    if let Some(expected_version) = version {
                        let got = params.get("version").and_then(Value::as_i64);
                        if got != Some(expected_version) {
                            continue;
                        }
                    }
                    diagnostics = Some(
                        params
                            .get("diagnostics")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                Some("window/logMessage") => {
                    let Some(message) = msg
                        .get("params")
                        .and_then(|params| params.get("message"))
                        .and_then(Value::as_str)
                    else {
                        continue;
                    };
                    if message.contains(log_fragment) {
                        log_message = Some(msg);
                    }
                }
                _ => {}
            }
            if let (Some(diagnostics), Some(log_message)) =
                (diagnostics.as_ref(), log_message.as_ref())
            {
                return (diagnostics.clone(), log_message.clone());
            }
        }
    }

    async fn wait_for_response_and_log_message(
        reader: &mut (impl AsyncRead + Unpin),
        id: i64,
        log_fragment: &str,
    ) -> (Value, Value) {
        let mut response = None;
        let mut log_message = None;
        loop {
            let msg = read_lsp_msg(&mut *reader).await;
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                response = Some(msg.clone());
            }
            if msg.get("method").and_then(Value::as_str) == Some("window/logMessage") {
                let Some(message) = msg
                    .get("params")
                    .and_then(|params| params.get("message"))
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                if message.contains(log_fragment) {
                    log_message = Some(msg);
                }
            }
            if let (Some(response), Some(log_message)) = (response.as_ref(), log_message.as_ref()) {
                return (response.clone(), log_message.clone());
            }
        }
    }

    fn has_error(diags: &[Value]) -> bool {
        diags.iter().any(|diag| {
            let severity = diag
                .get("severity")
                .and_then(Value::as_u64)
                .and_then(|n| match n {
                    1 => Some(DiagnosticSeverity::ERROR),
                    2 => Some(DiagnosticSeverity::WARNING),
                    3 => Some(DiagnosticSeverity::INFORMATION),
                    4 => Some(DiagnosticSeverity::HINT),
                    _ => None,
                });
            severity == Some(DiagnosticSeverity::ERROR)
        })
    }

    fn position_for(text: &str, needle: &str) -> (u32, u32) {
        let offset = text.find(needle).expect("needle exists");
        let mut line = 0u32;
        let mut col = 0u32;
        for (idx, ch) in text.char_indices() {
            if idx == offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn repo_root() -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root")
            .to_path_buf()
    }

    async fn start_lsp() -> (
        tokio::io::ReadHalf<tokio::io::DuplexStream>,
        tokio::io::WriteHalf<tokio::io::DuplexStream>,
        tokio::task::JoinHandle<()>,
    ) {
        let (service, socket) = LspService::new(|client| Backend {
            client,
            state: Arc::new(tokio::sync::Mutex::new(BackendState::default())),
        });
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (client_read, client_write) = tokio::io::split(client_io);

        let server_task = tokio::spawn(async move {
            Server::new(server_read, server_write, socket)
                .serve(service)
                .await;
        });

        (client_read, client_write, server_task)
    }

    async fn initialize_lsp(
        client_read: &mut (impl AsyncRead + Unpin),
        client_write: &mut (impl AsyncWrite + Unpin),
    ) -> Value {
        let root_uri = Url::from_file_path(repo_root())
            .expect("root uri")
            .to_string();

        write_lsp_msg(
            &mut *client_write,
            &json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "rootUri": root_uri,
                    "capabilities": {}
                }
            }),
        )
        .await;

        let response = timeout(Duration::from_secs(5), wait_for_response_id(client_read, 1))
            .await
            .expect("initialize response");

        write_lsp_msg(
            client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )
        .await;

        response
    }

    async fn shutdown_lsp(
        mut client_write: impl AsyncWrite + Unpin,
        server_task: tokio::task::JoinHandle<()>,
    ) {
        let _ = write_lsp_msg(
            &mut client_write,
            &json!({"jsonrpc":"2.0","id":2,"method":"shutdown","params":{}}),
        )
        .await;
        let _ = write_lsp_msg(
            &mut client_write,
            &json!({"jsonrpc":"2.0","method":"exit","params":{}}),
        )
        .await;
        let _ = timeout(Duration::from_secs(2), server_task).await;
    }

    #[tokio::test]
    async fn initialize_reports_incremental_sync() {
        let (mut client_read, mut client_write, server_task) = start_lsp().await;
        let response = initialize_lsp(&mut client_read, &mut client_write).await;

        let sync = response
            .get("result")
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("textDocumentSync"))
            .and_then(Value::as_i64)
            .unwrap_or_default();
        assert_eq!(sync, 2, "expected TextDocumentSyncKind::INCREMENTAL");
        let prepare_provider = response
            .get("result")
            .and_then(|r| r.get("capabilities"))
            .and_then(|c| c.get("renameProvider"))
            .and_then(|rename| rename.get("prepareProvider"))
            .and_then(Value::as_bool);
        assert_eq!(prepare_provider, Some(true));

        shutdown_lsp(client_write, server_task).await;
    }

    #[tokio::test]
    async fn diagnostics_clear_after_fix() {
        let (mut client_read, mut client_write, server_task) = start_lsp().await;
        initialize_lsp(&mut client_read, &mut client_write).await;

        let uri = Url::parse("file:///lsp/diagnostics.aivi").expect("uri");
        let bad_text = "module lsp.demo\n\nfail = if True then 1 else\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.to_string(),
                        "languageId": "aivi",
                        "version": 1,
                        "text": bad_text
                    }
                }
            }),
        )
        .await;

        let diags = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(1)),
        )
        .await
        .expect("publishDiagnostics");
        assert!(has_error(&diags));

        let fixed_text = "module lsp.demo\n\nfail = if True then 1 else 2\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri.to_string(), "version": 2},
                    "contentChanges": [{"text": fixed_text}]
                }
            }),
        )
        .await;

        let diags = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(2)),
        )
        .await
        .expect("publishDiagnostics");
        assert!(!has_error(&diags));

        shutdown_lsp(client_write, server_task).await;
    }

    #[tokio::test]
    async fn rapid_changes_keep_latest_diagnostics() {
        let (mut client_read, mut client_write, server_task) = start_lsp().await;
        initialize_lsp(&mut client_read, &mut client_write).await;

        let uri = Url::parse("file:///lsp/rapid.aivi").expect("uri");
        let initial = "module lsp.demo\n\nvalue = 1\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.to_string(),
                        "languageId": "aivi",
                        "version": 1,
                        "text": initial
                    }
                }
            }),
        )
        .await;

        let _ = timeout(
            Duration::from_secs(5),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(1)),
        )
        .await
        .expect("publishDiagnostics");

        let broken = "module lsp.demo\n\nvalue = if True then 1 else\n";
        let fixed = "module lsp.demo\n\nvalue = if True then 1 else 2\n";

        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri.to_string(), "version": 2},
                    "contentChanges": [{"text": broken}]
                }
            }),
        )
        .await;
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri.to_string(), "version": 3},
                    "contentChanges": [{"text": fixed}]
                }
            }),
        )
        .await;

        let diags = timeout(
            Duration::from_secs(5),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(3)),
        )
        .await
        .expect("publishDiagnostics");
        assert!(!has_error(&diags));

        shutdown_lsp(client_write, server_task).await;
    }

    #[tokio::test]
    async fn edits_at_document_boundaries() {
        let (mut client_read, mut client_write, server_task) = start_lsp().await;
        initialize_lsp(&mut client_read, &mut client_write).await;

        let uri = Url::parse("file:///lsp/boundary.aivi").expect("uri");
        let initial = "module lsp.demo\n\nvalue = 1\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.to_string(),
                        "languageId": "aivi",
                        "version": 1,
                        "text": initial
                    }
                }
            }),
        )
        .await;

        let _ = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(1)),
        )
        .await
        .expect("publishDiagnostics");

        let prepend = "module lsp.demo\n\nfirst = 0\nvalue = 1\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri.to_string(), "version": 2},
                    "contentChanges": [{"text": prepend}]
                }
            }),
        )
        .await;

        let diags = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(2)),
        )
        .await
        .expect("publishDiagnostics");
        assert!(!has_error(&diags));

        let append = "module lsp.demo\n\nfirst = 0\nvalue = 1\nlast = 2\n";
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didChange",
                "params": {
                    "textDocument": {"uri": uri.to_string(), "version": 3},
                    "contentChanges": [{"text": append}]
                }
            }),
        )
        .await;

        let diags = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics(&mut client_read, uri.as_str(), Some(3)),
        )
        .await
        .expect("publishDiagnostics");
        assert!(!has_error(&diags));

        shutdown_lsp(client_write, server_task).await;
    }

    #[tokio::test]
    async fn hover_definition_completion_round_trip() {
        let (mut client_read, mut client_write, server_task) = start_lsp().await;
        initialize_lsp(&mut client_read, &mut client_write).await;

        let uri = Url::parse("file:///lsp/requests.aivi").expect("uri");
        let text = "module lsp.demo\n\nadd = a b => a + b\nrun = add 1 2\n";

        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": uri.to_string(),
                        "languageId": "aivi",
                        "version": 1,
                        "text": text
                    }
                }
            }),
        )
        .await;

        let (_, diagnostics_log) = timeout(
            Duration::from_secs(2),
            wait_for_publish_diagnostics_and_log_message(
                &mut client_read,
                uri.as_str(),
                Some(1),
                "diagnostics.did_open",
            ),
        )
        .await
        .expect("publishDiagnostics");
        let diagnostics_log_message = diagnostics_log
            .get("params")
            .and_then(|params| params.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(diagnostics_log_message.contains("count="));

        let (line, col) = position_for(text, "add 1 2");
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "id": 10,
                "method": "textDocument/hover",
                "params": {
                    "textDocument": {"uri": uri.to_string()},
                    "position": {"line": line, "character": col}
                }
            }),
        )
        .await;

        let hover = timeout(
            Duration::from_secs(2),
            wait_for_response_id(&mut client_read, 10),
        )
        .await
        .expect("hover response");
        let hover_contents = hover
            .get("result")
            .and_then(|r| r.get("contents"))
            .and_then(|c| c.get("value"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(hover_contents.contains("`add`"));

        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "id": 11,
                "method": "textDocument/definition",
                "params": {
                    "textDocument": {"uri": uri.to_string()},
                    "position": {"line": line, "character": col}
                }
            }),
        )
        .await;

        let definition = timeout(
            Duration::from_secs(2),
            wait_for_response_id(&mut client_read, 11),
        )
        .await
        .expect("definition response");
        let def_uri = definition
            .get("result")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|loc| loc.get("uri"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(def_uri, uri.as_str());

        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "id": 12,
                "method": "textDocument/completion",
                "params": {
                    "textDocument": {"uri": uri.to_string()},
                    "position": {"line": line, "character": col}
                }
            }),
        )
        .await;

        let (completion, completion_log) = timeout(
            Duration::from_secs(2),
            wait_for_response_and_log_message(&mut client_read, 12, "completion duration_ms="),
        )
        .await
        .expect("completion response");
        let items = completion
            .get("result")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        assert!(items.iter().any(|item| {
            item.get("label")
                .and_then(Value::as_str)
                .is_some_and(|label| label == "add")
        }));
        let completion_log_message = completion_log
            .get("params")
            .and_then(|params| params.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(completion_log_message.contains("count="));

        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "id": 13,
                "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": {"uri": uri.to_string()},
                    "position": {"line": line, "character": col}
                }
            }),
        )
        .await;

        let prepare_rename = timeout(
            Duration::from_secs(2),
            wait_for_response_id(&mut client_read, 13),
        )
        .await
        .expect("prepare rename response");
        let range = prepare_rename.get("result").expect("prepare rename result");
        assert_eq!(
            range
                .get("start")
                .and_then(|start| start.get("line"))
                .and_then(Value::as_u64),
            Some(line as u64)
        );
        assert_eq!(
            range
                .get("start")
                .and_then(|start| start.get("character"))
                .and_then(Value::as_u64),
            Some(col as u64)
        );
        assert_eq!(
            range
                .get("end")
                .and_then(|end| end.get("character"))
                .and_then(Value::as_u64),
            Some((col + 3) as u64)
        );

        shutdown_lsp(client_write, server_task).await;
    }
}
