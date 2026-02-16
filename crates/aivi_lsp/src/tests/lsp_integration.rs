mod lsp_integration {
    use std::path::PathBuf;

    use serde_json::json;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
    use tokio::time::{timeout, Duration};
    use tower_lsp::lsp_types::{DiagnosticSeverity, Url};
    use tower_lsp::{LspService, Server};

    use crate::backend::Backend;
    use crate::state::BackendState;

    async fn write_lsp_msg(mut w: impl AsyncWrite + Unpin, value: &serde_json::Value) {
        let body = serde_json::to_vec(value).expect("json encode");
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        w.write_all(header.as_bytes()).await.expect("write header");
        w.write_all(&body).await.expect("write body");
        w.flush().await.expect("flush");
    }

    async fn read_lsp_msg(mut r: impl AsyncRead + Unpin) -> serde_json::Value {
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

    async fn wait_for_publish_diagnostics(
        reader: &mut (impl AsyncRead + Unpin),
        target_uri: &str,
    ) -> Vec<serde_json::Value> {
        loop {
            let msg = read_lsp_msg(&mut *reader).await;
            let Some(method) = msg.get("method").and_then(|m| m.as_str()) else {
                continue;
            };
            if method != "textDocument/publishDiagnostics" {
                continue;
            }
            let Some(params) = msg.get("params") else {
                continue;
            };
            let Some(uri) = params.get("uri").and_then(|u| u.as_str()) else {
                continue;
            };
            if uri != target_uri {
                continue;
            }
            return params
                .get("diagnostics")
                .and_then(|d| d.as_array())
                .cloned()
                .unwrap_or_default();
        }
    }

    #[tokio::test]
    async fn examples_open_without_lsp_error_diagnostics() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("repo root");
        let rel_paths = [
            // Syntax coverage
            "integration-tests/syntax/ir_dump_minimal.aivi",
            "integration-tests/syntax/domains/import_and_suffix_literals.aivi",
            "integration-tests/syntax/sigils/basic.aivi",
            "integration-tests/syntax/sigils/collections_structured.aivi",
            "integration-tests/syntax/effects/attempt_and_match.aivi",
            // Legacy runnable programs / larger modules
            "integration-tests/legacy/hello.aivi",
            "integration-tests/legacy/11_concurrency.aivi",
            "integration-tests/legacy/12_text_regex.aivi",
            // Stdlib-import-only microtests
            "integration-tests/stdlib/aivi/text/length.aivi",
            "integration-tests/stdlib/aivi/duration/domain_Duration/suffix_ms.aivi",
            "integration-tests/stdlib/aivi/number/decimal/n_1dec.aivi",
        ];
        let mut files: Vec<PathBuf> = rel_paths
            .iter()
            .map(|rel| repo_root.join(rel))
            .collect();
        for path in &files {
            assert!(path.exists(), "missing integration test file: {}", path.display());
        }
        files.sort();

        let (service, socket) = LspService::new(|client| Backend {
            client,
            state: std::sync::Arc::new(tokio::sync::Mutex::new(BackendState::default())),
        });
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        let (server_read, server_write) = tokio::io::split(server_io);
        let (mut client_read, mut client_write) = tokio::io::split(client_io);

        let server_task = tokio::spawn(async move {
            Server::new(server_read, server_write, socket)
                .serve(service)
                .await;
        });

        // initialize
        let root_uri = Url::from_file_path(repo_root).expect("root uri").to_string();
        write_lsp_msg(
            &mut client_write,
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

        // Wait for initialize response.
        let _ = timeout(Duration::from_secs(5), async {
            loop {
                let msg = read_lsp_msg(&mut client_read).await;
                if msg.get("id") == Some(&json!(1)) {
                    break msg;
                }
            }
        })
        .await
        .expect("initialize response");

        // initialized notification
        write_lsp_msg(
            &mut client_write,
            &json!({
                "jsonrpc": "2.0",
                "method": "initialized",
                "params": {}
            }),
        )
        .await;

        let mut failures = Vec::new();
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(uri) = Url::from_file_path(&path) else {
                continue;
            };
            let uri_str = uri.to_string();
            write_lsp_msg(
                &mut client_write,
                &json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/didOpen",
                    "params": {
                        "textDocument": {
                            "uri": uri_str,
                            "languageId": "aivi",
                            "version": 1,
                            "text": text
                        }
                    }
                }),
            )
            .await;

            let diags = timeout(
                Duration::from_secs(2),
                wait_for_publish_diagnostics(&mut client_read, uri.as_str()),
            )
            .await
            .expect("publishDiagnostics");

            let mut errors = Vec::new();
            for diag in &diags {
                let severity = diag
                    .get("severity")
                    .and_then(|s| s.as_u64())
                    .and_then(|n| match n {
                        1 => Some(DiagnosticSeverity::ERROR),
                        2 => Some(DiagnosticSeverity::WARNING),
                        3 => Some(DiagnosticSeverity::INFORMATION),
                        4 => Some(DiagnosticSeverity::HINT),
                        _ => None,
                    });
                if severity == Some(DiagnosticSeverity::ERROR) {
                    if let Some(msg) = diag.get("message").and_then(|m| m.as_str()) {
                        errors.push(msg.to_string());
                    } else {
                        errors.push(diag.to_string());
                    }
                }
            }
            if !errors.is_empty() {
                failures.push(format!("{}: {}", path.display(), errors.join(" | ")));
            }
        }

        // Best-effort shutdown.
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

        assert!(
            failures.is_empty(),
            "expected no ERROR diagnostics from aivi-lsp for integration-tests; got:\n{}",
            failures.join("\n")
        );
    }
}
