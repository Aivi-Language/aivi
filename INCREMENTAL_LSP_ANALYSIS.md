# AIVI Incremental LSP/Typecheck Analysis

## Overview

The AIVI LSP server implements an incremental, dependency-aware diagnostics pipeline with workspace-level snapshot coherence. The implementation spans:

- **Typecheck infrastructure** (`crates/aivi/src/typecheck/`): checkpoint caching, module export surface summarization, and dependency graphs
- **LSP state management** (`crates/aivi_lsp/src/state.rs`): per-document and workspace state, pending task tracking
- **Workspace indexing** (`crates/aivi_lsp/src/workspace.rs`): disk/open module tracking, lazy disk-index building
- **Document lifecycle** (`crates/aivi_lsp/src/server.rs`): didOpen/didChange/didClose/didChangeWatchedFiles handlers with debounce/cancellation
- **Diagnostic building** (`crates/aivi_lsp/src/diagnostics.rs`): workspace-aware checking with checkpoint reuse
- **Integration tests** (`crates/aivi_lsp/src/tests/lsp_protocol_edits.rs`): incremental recheck and dependent module validation

---

## Current Flow Architecture

### 1. Document Lifecycle & State Management

#### File: `crates/aivi_lsp/src/state.rs`

**Core State Structures:**
```rust
pub struct DocumentState {
    pub text: String,
    pub version: i32,
    pub parse_diags: Vec<aivi::FileDiagnostic>,  // Cache from last update_document call
}

pub struct BackendState {
    pub documents: HashMap<Url, DocumentState>,
    pub open_modules_by_uri: HashMap<Url, Vec<String>>,
    pub open_module_index: HashMap<String, IndexedModule>,
    pub disk_indexes: HashMap<PathBuf, DiskIndex>,
    pub module_export_summaries: HashMap<String, aivi::ModuleExportSurfaceSummary>,
    pub pending_diagnostics: Option<tokio::task::AbortHandle>,  // Per-keystroke cancellation
    pub diagnostics_snapshot: u64,  // Monotonic workspace token for debounce
    pub typecheck_checkpoint: Option<aivi::CheckTypesCheckpoint>,  // Lazy stdlib cache
}
```

**Key observations:**
- `parse_diags` captured once in `update_document` to prevent concurrent stale document corruption
- `module_export_summaries`: quick lookup for changed-module detection (see export surface rules below)
- `diagnostics_snapshot`: monotonically incremented on every keystroke; supersedes older in-flight work
- `pending_diagnostics`: AbortHandle allows mid-diagnostic-run cancellation

#### File: `crates/aivi_lsp/src/workspace.rs` (369 lines)

**Key Functions:**
- `update_document(uri, text, version)` → `Vec<FileDiagnostic>`
  - Parses modules from text in blocking task
  - Removes old module entries from `open_module_index`
  - Computes `ModuleExportSurfaceSummary` for export-change detection
  - Stores `parse_diags` for reuse in semantic checking
  
- `workspace_modules_for_diagnostics(uri)` → `HashMap<String, IndexedModule>`
  - Returns union of: embedded stdlib, disk-indexed modules (from workspace root), open modules
  - Stdlib takes priority over workspace duplicates
  - Used by both diagnostics and navigation

- `workspace_modules_for(uri)` → `HashMap<String, IndexedModule>`
  - Core function that builds workspace view
  - Lazy-loads disk index if not cached: `build_disk_index(root)` runs in blocking task
  
- **Disk Index Management:**
  - `invalidate_disk_index_for_path(path)`: deletes cached index (triggers rebuild on next use)
  - `refresh_disk_index_file(path)`: incremental single-file update (parse + merge into existing index)
  - `remove_from_disk_index(uri)`: removes deleted file's modules from index

### 2. Document Lifecycle Handlers

#### File: `crates/aivi_lsp/src/server.rs` (1269 lines)

**`didOpen` Handler (lines 423–488):**
1. Calls `update_document()` → captures parse diagnostics
2. Fetches workspace modules
3. Launches blocking typecheck task with:
   - Text, workspace modules, `pre_parsed_diags`, `typecheck_checkpoint`
   - Uses checkpoint if available; otherwise initializes stdlib checkpoint
4. Publishes diagnostics immediately (no debounce)
5. Logs telemetry: `[telemetry] diagnostics.did_open duration_ms=X uri=... version=Y count=Z`

**`didChange` Handler (lines 490–681):** [Core incremental logic]

**Phase 1: Incremental Text Apply (lines 494–513)**
```rust
let text = {
    let state = self.state.lock().await;
    let mut text = state.documents.get(&uri).map(|doc| doc.text.clone()).unwrap_or_default();
    for change in params.content_changes {
        if let Some(range) = change.range {
            let start = Self::offset_at(&text, range.start).min(text.len());
            let end = Self::offset_at(&text, range.end).min(text.len());
            text.replace_range(start..end, &change.text);
        } else {
            text = change.text;  // Full replacement fallback
        }
    }
    text
};
```
- Applies LSP incremental edits to current document text
- Handles both incremental (`range` set) and full (`range` None) replacements

**Phase 2: Export-Surface Change Detection (lines 518–524)**
```rust
let previous_summaries = self.document_module_export_summaries(&uri).await;
let parse_diags = self.update_document(uri.clone(), text.clone(), version).await;
let current_summaries = self.document_module_export_summaries(&uri).await;
let changed_modules = Self::changed_module_names(&previous_summaries, &current_summaries);
let export_changed = !changed_modules.is_empty();
```
- Compares module export surface fingerprints before/after
- `changed_module_names()`: modules with different `ModuleExportSurfaceSummary`
- Only export-surface changes (not private body changes) trigger dependent rechecks

**Phase 3: Debounce & Snapshot Token (lines 526–546)**
```rust
let current_snapshot = {
    let mut state = self.state.lock().await;
    if let Some(handle) = state.pending_diagnostics.take() {
        handle.abort();  // Cancel previous in-flight task
    }
    state.diagnostics_snapshot = state.diagnostics_snapshot.wrapping_add(1);
    state.diagnostics_snapshot
};

tokio::time::sleep(Duration::from_millis(150)).await;  // Debounce window

let state = self.state.lock().await;
if state.diagnostics_snapshot != current_snapshot {
    return;  // Superseded by newer keystroke
}
```
- 150ms debounce: waits before launching expensive typecheck
- Snapshot token: monotonic ID that identifies this keystroke's work
- Aborting old handle prevents stale results from publishing

**Phase 4: Dependent Module Recheck (lines 548–670)**
```rust
let dependent_targets = self.open_dependents_for_recheck(&uri, &changed_modules, &workspace).await;
```
- Calls `open_dependents_for_recheck()` (server.rs:100–153)
- Builds reverse dependency graph from all workspace modules
- Floods from `changed_modules` to find all transitively dependent open files
- Returns only open modules (in order) whose export surface changed

For each dependent:
```rust
for (dependent_uri, dependent_version, dependent_text) in dependent_targets {
    let still_current = { state.diagnostics_snapshot == current_snapshot };
    if !still_current { break; }
    // Recheck with same workspace + checkpoint
    let dependent_diagnostics = tokio::task::spawn_blocking(...).await.unwrap_or_default();
    client.publish_diagnostics(dependent_uri, dependent_diagnostics, Some(dependent_version)).await;
}
```
- Can be cancelled mid-recheck if newer keystroke arrives
- Publishes immediately as each dependent finishes

**`didClose` Handler (lines 683–687):**
```rust
pub(super) async fn remove_document(&self, uri: &Url) {
    let mut state = self.state.lock().await;
    state.documents.remove(uri);
    if let Some(existing) = state.open_modules_by_uri.remove(uri) {
        for module_name in existing {
            state.open_module_index.remove(&module_name);
            state.module_export_summaries.remove(&module_name);
        }
    }
}
```
- Removes document from all indexes
- Clears summaries and open modules
- Publishes empty diagnostics to client

**`didChangeWatchedFiles` Handler (lines 689–791):** [File system watching]

- Monitors `.aivi` file creation/modification/deletion outside editor
- Handles `aivi.toml` changes (project boundary)
- For `.aivi` changes: `refresh_disk_index_file()` (incremental) or `invalidate_disk_index_for_path()` (full rebuild on demand)
- Rechecks all open modules affected by file's workspace root
- Uses same diagnostic pipeline as `didChange`

### 3. Module Export Surface Summarization

#### File: `crates/aivi/src/typecheck/check.rs` (lines 142–212)

**`ModuleExportSurfaceSummary`:**
```rust
pub struct ModuleExportSurfaceSummary {
    pub fingerprint: u64,
    pub body_sensitive: bool,
}

pub fn summarize_module_export_surface(module: &Module) -> ModuleExportSurfaceSummary {
    // Hash: module name, exports.len, exports content, uses, annotations
    if body_sensitive {
        // Exported value/domain without explicit sig: hash full body
        format!("{:?}", module.items).hash(&mut hasher);
    } else {
        // Only hash type sigs, type decls, classes, instances, exported domains
        // Skip private defs entirely
    }
}
```

**Key rules (aligns with `/specs/tools/incremental_compilation.md`):**
- `body_sensitive = true`: exported value without type sig, or exported domain with body-only definitions
- When `false`: only type declarations matter; private body changes don't affect downstream
- Fingerprint incorporates: exports, type/class/domain structure, but not private bodies (unless exported)

### 4. Typecheck Checkpointing

#### File: `crates/aivi/src/typecheck/check.rs` (lines 15–140)

**`CheckTypesCheckpoint`:** (3.5 KB in memory)
```rust
pub struct CheckTypesCheckpoint {
    module_exports: HashMap<String, HashMap<String, Vec<Scheme>>>,
    module_domain_exports: HashMap<String, HashMap<String, Vec<String>>>,
    module_class_exports: HashMap<String, HashMap<String, ClassDeclInfo>>,
    module_instance_exports: HashMap<String, Vec<InstanceDeclInfo>>,
}

pub fn check_types_stdlib_checkpoint(stdlib_modules: &[Module]) -> CheckTypesCheckpoint
```
- Built once on first diagnostic run
- Caches type setup for all 63 embedded stdlib modules
- Reused for every user document keystroke
- Skips `setup_module()` for stdlib; runs full type setup only for user modules

**`check_types_with_checkpoint()` usage:**
```rust
let type_diags = if let Some(cp) = typecheck_checkpoint {
    check_types_with_checkpoint(&modules, cp)  // Faster: skips stdlib setup
} else {
    aivi::check_types(&modules)  // Full setup
};
```

### 5. Dependency-Aware Rechecking

#### File: `crates/aivi/src/typecheck/ordering.rs` (155 lines)

**`reverse_module_dependencies(modules: &[Module]) -> HashMap<String, Vec<String>>`**
```rust
for module in modules {
    for use_decl in &module.uses {
        let dep = use_decl.module.name.clone();
        reverse.entry(dep).or_insert_with(Vec::new).push(module.name.name.clone());
    }
}
```
- Maps: `"library"` → `["consumer_a", "consumer_b"]`
- Used to flood from changed modules to all transitive dependents

**`ordered_module_names()` / `ordered_modules()`**
- Topological sort via Kahn's algorithm
- Ensures modules are checked in dependency order

---

## Diagnostic Flow (Checkpoint to Publication)

### Full Workspace Build

```
didChange → update_document() {
  ├─ parse_modules() in blocking task
  ├─ compute ModuleExportSurfaceSummary
  ├─ compare previous vs. current → changed_modules
}
  ↓
workspace_modules_for_diagnostics() {
  ├─ get or lazy-build disk_index for workspace root
  ├─ merge: stdlib (authoritative) + disk_modules + open_modules
}
  ↓
build_diagnostics_with_workspace(text, uri, workspace, checkpoint) {
  ├─ parse_modules(text) [if pre_parsed_diags absent]
  ├─ collect_transitive_modules_for_diagnostics(file_modules, module_map)
  │   └─ BFS from file modules through imports
  ├─ check_modules() [resolver + name checks]
  ├─ check_types_with_checkpoint() [if checkpoint present]
  │   └─ setup_module() only for non-stdlib modules
  │   └─ type-check definition bodies
  ├─ build_strict_diagnostics() [optional strict-mode checks]
}
  ↓
collect open_dependents_for_recheck() {
  ├─ if changed_modules.is_empty() → return []
  ├─ reverse_module_dependencies(all_modules)
  ├─ transitive_dependents = bfs(changed_modules, reverse_deps)
  ├─ ordered_module_names() to sort targets
  ├─ return open module targets (filtered by open_module_index)
}
  ↓
publish_diagnostics(uri, diags, version)
```

**Cancellation Point:**
- After each dependent finishes checking: `state.diagnostics_snapshot == current_snapshot?`
- If no, break loop (newer keystroke arrived)

---

## Current Tests

### Integration Tests: `crates/aivi_lsp/src/tests/lsp_protocol_edits.rs` (924 lines)

**7 async test functions:**

1. **`initialize_reports_incremental_sync`**
   - Verifies server advertises `TextDocumentSyncKind::INCREMENTAL`
   - Basic handshake test

2. **`diagnostics_clear_after_fix`**
   - Opens file with error
   - Fixes it via didChange
   - Verifies diagnostics clear
   - **Gap**: no workspace-level testing (all single-file)

3. **`rapid_changes_keep_latest_diagnostics`** ⭐ Debounce validation
   - Sends 6 rapid edits within 150ms debounce window
   - Verifies only final keystroke's diagnostics publish
   - **Tests**: snapshot token superseding, AbortHandle usage
   - **Limitation**: single file; no dependent rechecking

4. **`edits_at_document_boundaries`**
   - Tests incremental text edits at boundaries (start, end, middle, full replacement)
   - **Gap**: no semantic validation (just parse diagnostics)

5. **`export_changes_recheck_open_dependents`** ⭐ Export-surface change
   - Opens `lib.aivi` exporting `value: Text`
   - Opens `consumer.aivi` importing `value`
   - Changes lib's export to `other: Int` (changes export surface)
   - **Validates**:
     - `export_changed=true` in telemetry
     - `dependents=1` in telemetry
     - Consumer gets rechecked (now has error: `value` no longer available)
   - **Limitation**: no latency timing assertions

6. **`private_body_changes_do_not_recheck_open_dependents`** ⭐ Private-body rule
   - Opens `lib.aivi` with exported `value` and private `helper`
   - Opens `consumer.aivi` importing `value`
   - Changes lib's private `helper` (export surface unchanged)
   - **Validates**:
     - `export_changed=false` in telemetry
     - `dependents=0` in telemetry
     - Consumer diagnostics do NOT update (timeout waits 700ms)
   - **Testing**: correct invalidation granularity

7. **`hover_definition_completion_round_trip`**
   - Tests navigation/completion after edits
   - **Gap**: not invalidation-focused

**Test helpers (not tests themselves):**
- `start_lsp()`, `initialize_lsp()`, `shutdown_lsp()`
- `wait_for_publish_diagnostics()`, `wait_for_publish_diagnostics_and_log_message()`
- Telemetry log parsing

**Notable test infrastructure:**
- Full LSP server over duplex pipes (not mocked)
- Async/await + tokio::time::timeout for race condition safety
- JSON RPC serialization validation

---

## Unit Tests

### `lsp_handler_unit.rs` (154 lines)
- Serialization JSON round-tripping
- Position handling at EOF
- Telemetry message formatting
- **Gap**: No incremental/dependency tests

### `backend_behavior.rs` (1012 lines)
- Parse/semantic diagnostics accuracy (non-incremental)
- Strict mode validation
- Semantic tokens, code actions, hover
- **Gap**: No workspace/dependent testing; single-document context

### `lsp_integration.rs` (237 lines)
- Opens ~10 integration-test files
- Verifies no ERROR diagnostics
- **Gap**: No multifile dependencies tested; purely structural validation

---

## Identified Gaps vs. Spec

### 1. Responsiveness & Latency Assertions ❌

**Missing:**
- No end-to-end timing assertions for:
  - Time from keystroke → first diagnostics
  - Time from keystroke → dependent diagnostics
  - Debounce overhead measurement
  
**Current state:**
- 150ms debounce is hardcoded (no configurable target latency SLA)
- Telemetry logs `duration_ms` but tests don't parse/assert ranges
- No percentile-based latency tests (p50/p95/p99)

**Required tests:**
```rust
#[tokio::test]
async fn diagnostics_publish_within_sla() {
    // Edit keystroke, measure time to first publish
    let start = Instant::now();
    // ... trigger diagnostics ...
    assert!(start.elapsed() < Duration::from_millis(500), "SLA miss");
}

#[tokio::test]
async fn dependent_recheck_bounded_latency() {
    // Export change in lib, measure dependent recheck time
    // Assert O(N) scaling where N = num open modules affected
}
```

### 2. Invalidation Behavior Gaps ❌

**Partially tested:**
- Export-surface change → dependent recheck (tests 5)
- Private-body change → no dependent recheck (test 6)

**Missing:**
- Schema-aware source changes (no schema tests at all)
- Definition-group granularity within modules (always rechecks whole module)
- Cross-file module duplication (first-seen vs. conflict behavior)
- `aivi.toml` boundary changes (partially tested via `didChangeWatchedFiles`)
- Partial recheck cleanup (stale checkpoint reuse after invalidation)

**Required tests:**
```rust
#[tokio::test]
async fn type_sig_vs_body_export_change_detection() {
    // Exported value WITH type sig: body-only changes don't affect export
    // Exported value WITHOUT type sig: body changes affect export
}

#[tokio::test]
async fn definition_group_same_module_recheck() {
    // Change in mutually recursive group dirties group + dependents in same module
    // Change in non-recursive def doesn't recheck others (needs group tracking)
}

#[tokio::test]
async fn workspace_boundary_aivi_toml_invalidation() {
    // Add/remove aivi.toml → should invalidate disk index for that branch
}
```

### 3. Diagnostics Flow Gaps ❌

**Working correctly:**
- Parse diagnostics captured early (before concurrent recheck)
- Workspace-aware module resolution for transitive dependencies
- Checkpoint reuse for stdlib
- Strict-mode diagnostics overlaid correctly

**Missing:**
- **Diagnostic de-duplication**: what if same file has multiple modules and some appear as workspace imports?
- **Incremental diagnostic merging**: when dependent rechecks, old diagnostics for that file cleared atomically?
- **Inconsistent workspace views**: can two concurrent checks see different module sets?
  - Current: debounce + snapshot prevents concurrent checks of same file; different files could have stale views
  
**Required tests:**
```rust
#[tokio::test]
async fn diagnostics_deduplicated_for_workspace_shadow() {
    // File "a.aivi" defines module "a"; workspace root also has "a"
    // Which wins? (answer: stdlib > workspace > open, but no test confirms)
}

#[tokio::test]
async fn concurrent_file_edits_dont_mix_diagnostics() {
    // Edit two independent files simultaneously
    // Each should see consistent module set from single snapshot
}
```

### 4. Module Export Surface Summarization Gaps ❌

**Working:**
- Fingerprinting includes exports, type decls, classes
- `body_sensitive` flag correctly identifies unsafe optimization

**Missing:**
- **No test for`body_sensitive` accuracy**: Can't confirm exported value without sig is truly marked sensitive
- **No test for fingerprint collision resistance**: Do unrelated changes affect hash?
- **No test for instance/class fingerprinting**: Do class/instance changes propagate correctly?

**Required tests:**
```rust
#[tokio::test]
async fn export_surface_body_sensitive_flag_accuracy() {
    // Case 1: export f; f = ... (no sig) → body_sensitive=true
    // Case 2: export f; f: Int -> Int; f = ... → body_sensitive=false
    // Verify only case 1 triggers dependent recheck on body change
}

#[tokio::test]
async fn export_fingerprint_change_detection() {
    // Add new exported value → fingerprint changes
    // Rename type constructor → fingerprint changes
    // Modify private def → fingerprint unchanged
}
```

### 5. Cancellation & Correctness Gaps ⚠️

**Working:**
- Snapshot token prevents stale publishes (debounce-level)
- AbortHandle cancels mid-diagnostic runs

**Edge cases untested:**
- **Cancellation after publish started**: if dependent 1 publishes but dependent 2 is cancelled, can old state leak?
- **Race between abort() and spawn_blocking() completion**: does Tokio guarantee no post-cancel execution?
- **Multiple rapid keystroke super-session**: does wrapping_add(1) on u64 snapshot token ever collide? (No, but no test assumes correctness)

**Required tests:**
```rust
#[tokio::test]
async fn cancelled_dependent_recheck_doesnt_publish() {
    // Rapid keystroke that cancels dependent recheck mid-run
    // Wait > timeout; verify dependent diagnostics stay stale
}

#[tokio::test]
async fn snapshot_token_wraparound_doesnt_collide() {
    // Unlikely but: if token wraps u64, do old tasks correctly reject?
}
```

### 6. Checkpoint Reuse Correctness ⚠️

**Working:**
- Checkpoint built on first keystroke, reused for all subsequent
- Checkpoint is immutable; never mutated

**Untested:**
- **Checkpoint reuse after stdlib change**: if stdlib fingerprint changes, old checkpoint is stale. Currently no detection.
  - Code: `typecheck_checkpoint.get_or_insert(cp)` – once set, never refreshed
  - Risk: rolling back compiler version leaves stale checkpoint
  
**Required test:**
```rust
#[tokio::test]
async fn stdlib_change_invalidates_checkpoint() {
    // Hypothetical: imagine swapping embedded stdlib at runtime
    // Verify checkpoint is rebuilt
    // (Current: no way to trigger this; requires design change)
}
```

### 7. Workspace Index Staleness ⚠️

**Current behavior:**
- Disk index cached per workspace root
- `refresh_disk_index_file()` does incremental updates
- `invalidate_disk_index_for_path()` clears entire index (rebuilt on next access)
- No file-watching for `aivi.toml` changes (relies on client notification via `didChangeWatchedFiles`)

**Untested:**
- What if two concurrent diagnostics see different disk indexes (one has old, one has new)?
  - Answer: Impossible if snapshot token prevents concurrent per-file checks
  - But different files could run in parallel with different indexes
  
**Required test:**
```rust
#[tokio::test]
async fn concurrent_independent_files_use_consistent_disk_index() {
    // Edit file A; while A's diagnostics pending, edit file B
    // Verify B's diagnostics use same snapshot of disk index as A
}
```

---

## Summary Table

| Aspect | Implementation | Tests | Coverage | Gap |
|--------|---|---|---|---|
| **didOpen** | ✅ Blocking check, immediate publish, no debounce | `lsp_integration.rs` | Basic open | No export tracking |
| **didChange** | ✅ Incremental text, export detection, debounce (150ms), snapshot token, AbortHandle | Tests 3, 5, 6 | 2 files, private/export rules | No latency SLA, no definition-group granularity |
| **didClose** | ✅ Remove from indexes, clear diagnostics | Implicit (cleanup assumed) | Removal tested | No dangling ref tests |
| **didChangeWatchedFiles** | ✅ Disk index refresh/invalidate, recheck open modules | Test 5 indirectly | Single file changes | No aivi.toml boundary tests, no multi-file mutation |
| **Export Surface Summary** | ✅ Fingerprint + body_sensitive flag, hashing | Implicit in test 5,6 | Dependent recheck fires/doesn't fire | No fingerprint accuracy tests |
| **Typecheck Checkpoint** | ✅ Built once, reused, immutable | Implicit (telemetry shows reuse) | Stdlib skipped | No checkpoint invalidation tests |
| **Module Ordering** | ✅ Topological sort, reverse dependency graph | Implicit (no cycles break) | Dependent detection | No cycle handling tests |
| **Workspace Indexing** | ✅ Lazy disk build, merge stdlib > workspace > open | Implicit in navigation tests | Fallback resolution | No staleness/consistency tests |
| **Debounce/Cancellation** | ✅ 150ms sleep, snapshot token, AbortHandle | Test 3 | Rapid keystrokes | No SLA timing, no edge cases |
| **Diagnostic Publish** | ✅ Snapshot-guarded, per-file scope, parse + semantic + strict | Tests 1-5 | Happy path | No de-duplication, no concurrent-view tests |

---

## Recommendations

### High-Priority (Spec Compliance)
1. **Add latency assertions** to `rapid_changes_keep_latest_diagnostics` and dependent-recheck tests
   - Target: p95 < 300ms from keystroke to diagnostics
   - Measure histogram of 10 runs
   
2. **Add definition-group granularity tests**
   - Verify same-module sibling defs don't recheck unless they depend on changed group
   - Requires new `Scope` tracking in LSP state (future work)

3. **Test export surface fingerprint accuracy**
   - Confirm body_sensitive flag correctly identifies unsafe cases
   - Verify hash collision resistance

### Medium-Priority (Incremental Hardening)
4. **Add concurrent-file-edit tests** to prevent inconsistent workspace views
   - Verify snapshot coherence across dependent checks
   
5. **Add workspace-boundary tests** for aivi.toml
   - Verify disk index invalidation on project boundary changes
   
6. **Add checkpoint reuse diagnostics**
   - Log checkpoint hit/miss rates
   - Measure stdlib setup time vs. reuse speedup

### Low-Priority (Future Extensions)
7. **Schema-aware source changes** (deferred to Phase 4)
   - Add schema summary to export surface
   - Test schema-driven invalidation
   
8. **Partial recheck state cleanup** (deferred)
   - Implement definition-group cache eviction
   - Test cache retention/expiration policies

---

## File Sizes & Complexity

| File | LoC | Responsibility |
|------|-----|---|
| `server.rs` | 1269 | Lifecycle handlers, debounce, snapshot tokens, dependent recheck |
| `diagnostics.rs` | 996 | Workspace-aware checking, checkpoint usage, strict overlay |
| `backend_behavior.rs` | 1012 | Non-incremental diagnostic tests (35 test fns) |
| `lsp_protocol_edits.rs` | 924 | Incremental protocol tests (7 async test fns) |
| `state.rs` | ~150 | Document/workspace state structures |
| `workspace.rs` | 369 | Disk/open indexing, lazy building |
| `check.rs` | 421 | Checkpoint definition & building |
| `ordering.rs` | 155 | Dependency graph algorithms |

---

## Conclusion

The AIVI LSP implementation is **well-architected for incremental compilation** with:
- ✅ Correct export-surface change detection (tests 5, 6 validate)
- ✅ Proper debounce/cancellation (test 3 validates)
- ✅ Checkpoint caching for stdlib (implicit, measured in telemetry)
- ✅ Workspace-aware diagnostics (implicit in tests)

**Key gaps for production readiness:**
1. **No latency SLA tests** → cannot guarantee responsiveness
2. **No definition-group testing** → missing within-module optimization
3. **No fingerprint accuracy validation** → export change detection unvalidated
4. **No concurrent-view correctness tests** → workspace coherence assumed but unproven
5. **Missing schema-aware tests** → will be critical for Phase 4

All gaps are addressable with targeted test additions; the implementation does not require architectural changes.
