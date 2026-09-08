use std::{hint::black_box, path::PathBuf, sync::Arc, time::Duration};

use aivi_lsp::{
    diagnostics::collect_lsp_diagnostics,
    documents::{change_document, open_document},
    formatting::format_document,
    references::references,
    semantic_tokens::{semantic_tokens_full, semantic_tokens_full_delta, semantic_tokens_range},
    state::ServerState,
};
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use tower_lsp::lsp_types::{
    PartialResultParams, Position, Range, ReferenceContext, ReferenceParams,
    SemanticTokensDeltaParams, SemanticTokensFullDeltaResult, SemanticTokensParams,
    SemanticTokensRangeParams, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

fn benchmark_document() -> (Arc<ServerState>, Url, aivi_query::SourceFile) {
    let state = Arc::new(ServerState::new());
    let uri = Url::from_file_path(PathBuf::from("/benchmarks/snake.aivi"))
        .expect("benchmark URI should be valid");
    open_document(
        &state,
        &uri,
        1,
        include_str!("../../demos/snake.aivi").to_owned(),
    );
    let file = state.file(&uri).expect("benchmark document should be open");
    (state, uri, file)
}

fn bench_lsp_operations(c: &mut Criterion) {
    let (state, uri, file) = benchmark_document();
    let source = include_str!("../../demos/snake.aivi");
    let token_params = SemanticTokensParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: Default::default(),
    };
    let advance_offset = source
        .find("advance")
        .expect("benchmark source should contain advance");
    let advance_prefix = &source[..advance_offset];
    let advance_line = advance_prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let advance_line_start = advance_prefix.rfind('\n').map_or(0, |index| index + 1);
    let reference_params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: advance_line,
                character: source[advance_line_start..advance_offset]
                    .encode_utf16()
                    .count() as u32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };
    references(reference_params.clone(), Arc::clone(&state))
        .expect("benchmark reference should resolve while warming the workspace index");

    let mut group = c.benchmark_group("lsp");
    group.throughput(Throughput::Bytes(
        include_str!("../../demos/snake.aivi").len() as u64,
    ));

    group.bench_function("diagnostics_cached_document", |b| {
        b.iter(|| {
            black_box(collect_lsp_diagnostics(
                black_box(&state.db),
                black_box(file),
                black_box(&uri),
            ));
        });
    });

    group.bench_function("format_document", |b| {
        b.iter(|| {
            black_box(format_document(black_box(&state.db), black_box(file)));
        });
    });

    group.bench_function("semantic_tokens_full", |b| {
        b.iter(|| {
            black_box(semantic_tokens_full(
                black_box(token_params.clone()),
                Arc::clone(&state),
            ));
        });
    });

    let mut previous_result_id =
        match semantic_tokens_full(token_params.clone(), Arc::clone(&state))
            .expect("benchmark semantic tokens should resolve")
        {
            tower_lsp::lsp_types::SemanticTokensResult::Tokens(tokens) => tokens
                .result_id
                .expect("full semantic tokens should provide a delta result id"),
            tower_lsp::lsp_types::SemanticTokensResult::Partial(_) => {
                panic!("full semantic tokens should not be partial")
            }
        };
    group.bench_function("semantic_tokens_unchanged_delta", |b| {
        b.iter(|| {
            let result = semantic_tokens_full_delta(
                SemanticTokensDeltaParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    previous_result_id: previous_result_id.clone(),
                    work_done_progress_params: WorkDoneProgressParams::default(),
                    partial_result_params: PartialResultParams::default(),
                },
                Arc::clone(&state),
            )
            .expect("benchmark delta should resolve");
            previous_result_id = match &result {
                SemanticTokensFullDeltaResult::Tokens(tokens) => tokens
                    .result_id
                    .clone()
                    .expect("full fallback should provide a result id"),
                SemanticTokensFullDeltaResult::TokensDelta(delta) => delta
                    .result_id
                    .clone()
                    .expect("delta should provide a result id"),
                SemanticTokensFullDeltaResult::PartialTokensDelta { .. } => {
                    panic!("benchmark delta should not be partial")
                }
            };
            black_box(result);
        });
    });

    let range_params = SemanticTokensRangeParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range: Range {
            start: Position {
                line: 190,
                character: 0,
            },
            end: Position {
                line: 240,
                character: 0,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    group.bench_function("semantic_tokens_range_50_lines", |b| {
        b.iter(|| {
            black_box(semantic_tokens_range(
                black_box(range_params.clone()),
                Arc::clone(&state),
            ));
        });
    });

    group.bench_function("references_warm_workspace_index", |b| {
        b.iter(|| {
            black_box(references(
                black_box(reference_params.clone()),
                Arc::clone(&state),
            ));
        });
    });

    group.finish();

    let mut version = 1;
    let mut uppercase = false;
    let mut sync = c.benchmark_group("lsp_incremental_sync");
    sync.throughput(Throughput::Elements(1));
    sync.bench_function("single_utf16_edit_13kb_document", |b| {
        b.iter(|| {
            version += 1;
            uppercase = !uppercase;
            let replacement = if uppercase { "U" } else { "u" };
            black_box(
                change_document(
                    black_box(&state),
                    black_box(&uri),
                    black_box(version),
                    &[TextDocumentContentChangeEvent {
                        range: Some(Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 1,
                            },
                        }),
                        range_length: Some(1),
                        text: replacement.to_owned(),
                    }],
                )
                .expect("benchmark edit should remain valid"),
            );
        });
    });
    sync.finish();
}

criterion_group! {
    name = lsp;
    config = Criterion::default()
        .sample_size(40)
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5));
    targets = bench_lsp_operations
}
criterion_main!(lsp);
