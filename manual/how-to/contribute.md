# How to Contribute

This guide is the shortest path from a clean checkout to a change that satisfies the repository's
quality gates. Read `AGENTS.md` first: it defines the language, threading, IR, diagnostic, and
delivery invariants.

## 1. Install the toolchains

Rust 1.98.0 is selected automatically by `rust-toolchain.toml`. Documentation and extension work
uses Node.js 24 and pnpm 10.19.0. On Debian or Ubuntu, install the native desktop dependencies:

```sh
sudo apt-get install libgtk-4-dev libadwaita-1-dev libwebkitgtk-6.0-dev \
  libssl-dev libdbus-1-dev libsecret-1-dev
```

Install the two independent Node workspaces when you touch them:

```sh
pnpm --dir manual install --frozen-lockfile
pnpm --dir tooling install --frozen-lockfile
```

## 2. Orient the change

### Connect Crusty

[Crusty](https://github.com/mendrik/crusty) runs locally as `rust-repo-intelligence`. Install it
from a Crusty checkout with `cargo install --path . --locked`, and make that executable available
on the MCP client's `PATH`.

The repository's `.mcp.json` registers `Crusty` for clients that support this configuration file.
Launch the client from the AIVI repository root: `--workspace .` selects this checkout. For clients
with their own MCP configuration, use the same command and pass the absolute AIVI checkout path
after `--workspace`. An existing host-level Crusty connection can be reused when it selects this
repository; avoid registering it twice. Restart the connection after changing its configuration.

Verify the checked-in configuration with Node.js 24, without installing Node dependencies:

```sh
node tooling/check-crusty-mcp.mjs
```

The check starts the configured server, negotiates MCP, verifies the workflow tools, calls
`repo.consult`, and checks that the returned repository is this checkout. The separate `Aivi`
entry runs this checkout's compiler through Cargo for live application introspection. Build it
with `cargo build --bin aivi` before connecting so the client's startup timeout does not include
a first build. It requires the desktop dependencies listed above.

Crusty stores local state under `.rust-repo-intelligence/`, which Git ignores. Back up the whole
directory with Crusty stopped: both SQLite databases contain project records as well as derived
data. Keep this directory when cleaning build artifacts. A fresh clone does not include another
checkout's ledger.

### Prepare and validate

The specification and implementation are authoritative for language behavior. Crusty is the sole
durable project ledger:

1. call `repo.consult` with the full task intent and inspect the returned decisions, steering,
   problems, constraints, and authorized work
2. check `index.status`; use `index.refresh` and poll `task.get` when a fresh published index is
   needed; `repo.search` with `mode: "exact"` reads live source without a refresh
3. call `change.prepare` with the intended files, poll `task.get`, and retain its `context_id`
4. identify semantic, ownership, threading, stack-safety, IR, and diagnostic invariants, then
   implement at the owning compiler/runtime/tooling layer
5. call `change.validate` with the context ID and poll `task.get`; in a dirty worktree, supply
   the task's focused `git_diff` so unrelated edits do not become this change's evidence
6. inspect `validation.queue`, record applicable obligation outcomes with `validation.record`,
   and attach actual command results to the authorized work item before completing it

Use `audit.start` for a persisted architecture audit and `audit.get` to retrieve its report.
Check inferred findings against callers and tests before acting on them. Use `task.list` and
`change.get` to recover interrupted preparation or validation.

Do not create a second Markdown backlog or architecture log. Put discoverable implementation facts
in code, the specification, or the manual; use Crusty for durable project memory.

## 3. Run focused checks while iterating

```sh
cargo fmt --all -- --check
cargo check -p <affected-crate> --all-targets --all-features
cargo clippy -p <affected-crate> --all-targets --all-features --no-deps -- -D warnings
cargo test -p <affected-crate> --all-features
```

Use the artifact dependency graph in `AGENTS.md`. A syntax change may also require grammar,
semantic-token, completion, snippet, fixture, and manual updates. A runtime or GTK change requires
the corresponding stress/integration coverage and may affect MCP introspection.

## 4. Check documentation

Every fenced `aivi` block is checked documentation and must format, parse, resolve, and
type-check. This gate does not execute its effects or verify claimed output:

```sh
./tooling/check-manual-aivi-snippets.sh
```

If the checker reports formatting-only changes, apply them explicitly and then rerun the read-only
gate:

```sh
./tooling/check-manual-aivi-snippets.sh --write
./tooling/check-manual-aivi-snippets.sh
```

Check page targets, heading anchors, navigation coverage, and code-fence languages, then
build the VitePress site:

```sh
pnpm --dir manual test
pnpm --dir manual check
pnpm --dir manual build
```

Use `node manual/scripts/check-docs.mjs --external` to check HTTP reachability of outbound
links. Access errors require manual review; external heading fragments are not checked.
The checker covers maintained project Markdown (manual, specifications, crate READMEs,
contribution/agent instructions, and the editor README), not installed skill packages or
generated dependencies. It does not prove prose claims or exercise a graphical UI.

Use `text` for diagrams/output and `sh` for shell commands. Incomplete or multi-file AIVI
examples can use `aivi-fragment`: it shares the AIVI highlighter but is explicitly outside
the standalone snippet gate. Explain the required context beside each fragment; do not
relabel a failing complete example to bypass checking. Empty fences are rejected.

Keep tutorials task-led, how-to guides goal-led, reference pages exact, and explanation pages
conceptual. Mark unimplemented behavior plainly; do not present roadmap ideas as shipped features.

## 5. Check the VS Code extension

```sh
pnpm --dir tooling -F vscode-aivi lint
pnpm --dir tooling -F vscode-aivi test
pnpm --dir tooling -F vscode-aivi build
pnpm --dir tooling -F vscode-aivi package
```

The integration suite builds the real `aivi` binary and performs an LSP stdio handshake. The
package command also inspects the VSIX allowlist.

## 6. Check dependency policy

```sh
cargo deny check
cargo machete
pnpm --dir manual audit --audit-level high
pnpm --dir tooling audit --audit-level high
```

Explain every new crate in terms of invariants, runtime and compile-time cost, binary size, and
maintenance risk. Keep Cranelift and GTK-family versions aligned.

## 7. Run the full gate before handoff

```sh
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features --no-deps -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
cargo build --workspace --release --all-features
```

Parser and decoder fuzz targets also receive bounded smoke runs in CI. For a local smoke run:

```sh
cd fuzz
cargo +nightly fuzz run parser_lossless -- -runs=512 -max_total_time=30
cargo +nightly fuzz run decoder_paths -- -runs=512 -max_total_time=30
```

For performance changes, follow [Benchmarking](/reference/benchmarking) and retain reproducible
before/after evidence.
