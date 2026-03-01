# AIVI Test Quality Audit Report

**Total tests audited: 439**
**Passed: 390 (88.8%)**
**Failed: 49 (11.2%)**

---

## Summary by Area

| Area | Total | Pass | Fail | Fail % |
|:-----|------:|-----:|-----:|-------:|
| typechecker | 55 | 45 | 10 | 18.2% |
| other (misc test files) | 74 | 64 | 10 | 13.5% |
| formatter | 37 | 28 | 9 | 24.3% |
| cranelift_backend | 45 | 40 | 5 | 11.1% |
| resolver | 9 | 6 | 3 | 33.3% |
| parser/surface | 48 | 45 | 3 | 6.3% |
| pipeline | 41 | 39 | 2 | 4.9% |
| mcp | 5 | 3 | 2 | 40.0% |
| lsp | 67 | 65 | 2 | 3.0% |
| runtime | 41 | 40 | 1 | 2.4% |
| lexer | 8 | 7 | 1 | 12.5% |
| cli | 3 | 2 | 1 | 33.3% |
| core | 6 | 6 | 0 | 0.0% |

## Failure Categories

| Category | Count |
|:---------|------:|
| Smoke/shallow test (only checks error codes or `is_empty()`) | 18 |
| Unclear/weak assertions | 16 |
| Missing assertions entirely | 4 |
| Debug-only (prints, no assertions) | 3 |
| Possible bug or contradictory name | 2 |
| Ignored/broken (`#[ignore]`) | 2 |
| Tautological (input == expected output) | 1 |
| Not a real test (helper function) | 1 |
| Missing file dependency (silently skips) | 1 |
| Duplicate test | 1 |

---

## All Failed Tests (49)

### Parser / Surface Tests
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 57 | crates/aivi/src/surface/tests.rs:1417 | `rejects_multiple_modules_per_file` | Smoke test: only checks error code without validating location or context. |
| 58 | crates/aivi/src/surface/tests.rs:1430 | `rejects_result_or_success_arms` | Smoke test: only checks E1530 without context validation. |
| 59 | crates/aivi/src/surface/tests.rs:1445 | `rejects_test_without_argument` | Smoke test: only checks E1511 without decorator behavior validation. |

### Lexer
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 103 | crates/aivi/src/lexer.rs:836 | `lex_rejects_non_ascii_identifier_start_as_unexpected_character` | Smoke: single assertion on error code, no recovery state check. |

### Formatter
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 66 | crates/aivi/src/formatter.rs:144 | `format_preserves_default_brace_style_kr` | Smoke: only checks substring presence without full output verification. |
| 201 | crates/aivi/tests/fmt_test.rs:70 | `test_fmt_remove_extra_whitespace` | Trivial single assertion, no corner cases. |
| 202 | crates/aivi/tests/fmt_test.rs:77 | `test_fmt_binary_minus_has_spaces` | Input and output identical: tautological. |
| 205 | crates/aivi/tests/fmt_test.rs:107 | `test_fmt_keeps_space_before_list_literal_arg` | Single shallow assertion. |
| 207 | crates/aivi/tests/fmt_test.rs:121 | `test_fmt_merges_hanging_opener_after_then_and_else` | Only substring checks. |
| 212 | crates/aivi/tests/fmt_test.rs:236 | `test_fmt_allman_brace_style_is_configurable` | Only substring check. |
| 213 | crates/aivi/tests/fmt_test.rs:249 | `test_fmt_match_subject_moves_onto_arrow_line` | Only substring check. |
| 214 | crates/aivi/tests/fmt_test.rs:261 | `test_fmt_drops_leading_commas_in_multiline_records` | Only negative assertions. |
| 433 | crates/aivi_core/src/formatter/mod.rs:589 | `bench_format_large_file` | Depends on missing /tmp file, silently skips. |

### Resolver
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 76 | crates/aivi/src/resolver/debug_and_unused.rs:503 | `module_aliasing_rewrites_and_resolves_wildcard_imports` | Smoke: only checks `diags.is_empty()` without validating resolution. |
| 77 | crates/aivi/src/resolver/debug_and_unused.rs:533 | `module_aliasing_handles_call_and_index_syntax` | Smoke: no assertion on actual resolution behavior. |
| 78 | crates/aivi/src/resolver/debug_and_unused.rs:568 | `gtk4_native_record_is_resolved_as_builtin` | Smoke: no assertion gtk4 resolved as builtin. |

### Cranelift Backend
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 110 | crates/aivi/src/cranelift_backend/use_analysis.rs:535 | `unused_var_zero_count` | Smoke: only defensive check for nonexistent var. |
| 120 | crates/aivi/src/cranelift_backend/inline.rs:1476 | `substitute_simple` | **Possible bug**: asserts replacement with 99 but checks for "42". |
| 124 | crates/aivi/src/cranelift_backend/inline.rs:1562 | `inline_depth_limit` | No assertion: only verifies it doesn't hang, not behavior. |
| 129 | crates/aivi/src/cranelift_backend/runtime_helpers.rs:2352 | `rt_register_machines_from_data_in_symbol_table` | Surface-level symbol existence check only. |
| 304 | crates/aivi/tests/cranelift_jit.rs:225 | `cranelift_jit_generate_with_filter` | Duplicate of test 303, tests same thing. |

### Typechecker
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 254 | crates/aivi/tests/typecheck_core.rs:276 | `typecheck_error_unknown_numeric_delta_literal` | Generic `check_err` without specific error verification. |
| 259 | crates/aivi/tests/typecheck_core.rs:347 | `typecheck_record_literal_missing_required_field_is_error` | **Contradictory**: name says `is_error` but uses `check_ok`. |
| 260 | crates/aivi/tests/typecheck_core.rs:360 | `typecheck_imported_type_alias_checks_record_fields` | Name suggests error but uses `check_ok_with_embedded`. |
| 261 | crates/aivi/tests/typecheck_core.rs:375 | `typecheck_branded_type_is_nominal` | Missing assertions, unclear error expectation. |
| 264 | crates/aivi/tests/typecheck_core.rs:434 | `typecheck_error_effect_final` | Cryptic test, doesn't document why `final 1` should error. |
| 267 | crates/aivi/tests/typecheck_core.rs:494 | `typecheck_effect_block_let_rejects_effect_expr` | Unclear why assignment of effect expr is wrong. |
| 276 | crates/aivi/tests/typecheck_core.rs:651 | `typecheck_row_op_errors` | No assertion: test runs but doesn't verify. |
| 287 | crates/aivi/tests/typecheck_core.rs:878 | `typecheck_error_custom_type_no_cross_operator` | No assertion, unclear error expectation. |

### Pipeline
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 174 | crates/aivi/tests/typecheck_astar_regression.rs:78 | `typecheck_astar_no_ambiguous_vec2_minus` | Integration only, no explicit assertions. |
| 175 | crates/aivi/tests/typecheck_astar_regression.rs:85 | `typecheck_dijkstra_effectful_target` | Integration only, no explicit assertions. |
| 233 | crates/aivi/tests/pipeline_qa_phases.rs:91 | `p3_multiple_errors_aggregated` | Only asserts `>= 1` error, not specific count. |

### Runtime
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 348 | crates/aivi/tests/runner.rs:118 | `run_files_parallel` | Not a test function, helper code with `#[test]`. |

### MCP
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 85 | crates/aivi/src/mcp/schema.rs:568 | `bundled_specs_manifest_lists_resources` | Smoke: only checks non-empty collections, no structure validation. |
| 86 | crates/aivi/src/mcp/schema.rs:582 | `resources_read_returns_markdown_text` | Smoke: only MIME type and `contains` check. |

### CLI
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 92 | crates/aivi/src/main/cli.rs:950 | `version_text_contains_cli_and_language_versions` | Smoke: only string contains check, no format validation. |

### LSP
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 355 | crates/aivi_lsp/src/repro_lsp.rs:29 | `hover_on_operator_works` | Marked `#[ignore]`, intentionally panics. |
| 366 | crates/aivi_lsp/src/tests/fixtures.rs:124 | `examples_open_without_lsp_errors` | Marked `#[ignore]`, assertion fails. |

### Other (misc test files)
| ID | File | Test | Verdict |
|---:|:-----|:-----|:--------|
| 139 | crates/aivi/tests/examples_build.rs:4 | `examples_build` | Smoke: just runs build, no output validation. |
| 142 | crates/aivi/tests/debug_parse.rs:4 | `debug_file_content` | Debug-only: no assertions, just prints. |
| 144 | crates/aivi/tests/debug_lex.rs:3 | `debug_tokenization` | Debug-only: print statements, no assertions. |
| 147 | crates/aivi/tests/debug_pipeline.rs:3 | `debug_full_pipeline` | Debug-only: print, no assertions. |
| 156 | crates/aivi/tests/pm_publish_preflight.rs:13 | `publish_preflight_accepts_consistent_manifests` | Smoke: just asserts success. |
| 160 | crates/aivi/tests/classes_and_syntax.rs:5 | `class_inheritance_uses_type_and_combinator` | Smoke: no structure verification. |
| 161 | crates/aivi/tests/classes_and_syntax.rs:35 | `instance_inherited_methods_can_delegate_to_super_instance` | Smoke: only checks absence of errors. |
| 163 | crates/aivi/tests/ir_dump_json.rs:63 | `kernel_dump_ir_dump_minimal_is_valid_json` | Surface: only JSON validity, no content check. |
| 164 | crates/aivi/tests/ir_dump_json.rs:83 | `rust_ir_dump_ir_dump_minimal_is_valid_json` | Surface: only JSON validity, no content check. |
| 171 | crates/aivi/tests/pipe_test.rs:1 | `test_pipe_preserved_without_alignment` | Smoke: only string presence check. |
| 172 | crates/aivi/tests/pipe_test.rs:11 | `test_pipe_preserved_with_alignment` | Smoke: only string presence check. |

---

## Priority Recommendations

### 🔴 High Priority (possible bugs or contradictions)
1. **ID 120** (`substitute_simple`): Asserts replacement value 99 but checks for string "42" — possible logic bug.
2. **ID 259** (`typecheck_record_literal_missing_required_field_is_error`): Name says error expected but uses `check_ok` helper.
3. **ID 260** (`typecheck_imported_type_alias_checks_record_fields`): Same contradiction pattern.

### 🟡 Medium Priority (dead/useless tests)
4. **ID 142, 144, 147** (`debug_parse`, `debug_lex`, `debug_pipeline`): Debug-only, no assertions. Should be removed or given assertions.
5. **ID 348** (`run_files_parallel`): Helper function incorrectly marked as `#[test]`.
6. **ID 304** (`cranelift_jit_generate_with_filter`): Duplicate of test 303.
7. **ID 276, 287** (`typecheck_row_op_errors`, `typecheck_error_custom_type_no_cross_operator`): No assertions at all.
8. **ID 433** (`bench_format_large_file`): Silently skips when `/tmp` file missing.

### 🟢 Low Priority (could improve but functional)
- 18 smoke/shallow tests that check error codes without context validation.
- 8 formatter tests with shallow substring assertions.
- 2 `#[ignore]` tests (IDs 355, 366) need decision: fix or remove.
