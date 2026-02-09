# MCP Integration Plan (Rust host + AIVI tools/resources compiled to WASM)

Goal: `aivi mcp serve` starts an MCP server that exposes AIVI functions annotated as tools/resources, with schemas generated from AIVI types.

This aligns with `specs/ideas/05_tooling.md`.

## What “MCP support” means for AIVI

- AIVI source can declare:
  - `@mcp_tool` functions (callable by an MCP client)
  - `@mcp_resource` values (readable/listable by an MCP client)
- The AIVI compiler produces:
  - an executable artifact (WASM) plus
  - a metadata bundle describing exported tools/resources and their types/docs.
- The Rust host:
  - loads the artifact
  - implements any required effects (WASI + extra host APIs)
  - serves MCP HTTP/stdio transport (depending on your MCP host choice)

## Proposed artifacts

From `aivi build`:
- `target/aivi/<pkg>/<module>.wasm` (core module)
- `target/aivi/<pkg>/<module>.component.wasm` (optional component)
- `target/aivi/<pkg>/<module>.aivi-meta.json` (metadata)

Metadata includes:
- tool list: name, docs, parameter types, return type, required effects/capabilities
- resource list: name, docs, “read” type
- type schema graph (for JSON schema generation)

## Execution models

### Model A (recommended early): host-driven JSON bridge

1. MCP request arrives as JSON.
2. Rust host converts JSON → AIVI values (in the runtime representation).
3. Call the WASM export for the tool.
4. Convert the return value back to JSON.

Pros:
- Works with WASM MVP today.
- Can be implemented before the component model is fully integrated.

Cons:
- You must maintain a stable value encoding and JSON conversion layer.

### Model B (recommended long-term): component-model typed calls

1. MCP JSON is mapped to WIT types.
2. Rust host uses `wasmtime::component` to call exports with structured values.
3. Values cross the boundary via canonical ABI.

Pros:
- Strongly typed, no pointer/len marshaling.
- Great for “tool” interop beyond MCP.

Cons:
- Requires the component model end-to-end.

## Mapping AIVI types to JSON Schema

Define a single canonical mapping (versioned) for:
- Primitives:
  - `Int` → `{ "type": "integer" }` (decide i64 vs arbitrary precision)
  - `Float` → `{ "type": "number" }`
  - `Bool` → `{ "type": "boolean" }`
  - `Text` → `{ "type": "string" }`
- Records → `{ "type": "object", "properties": ..., "required": ... }`
- ADTs → `oneOf` with tagged representation (recommend explicit tag field)
- `Option A` → `anyOf: [A, null]` (or tagged option)
- `Result A E` → `{ "oneOf": [ { ok: A }, { err: E } ] }` (tagged)
- Lists → `{ "type": "array", "items": A }`

Decision: pick a “wire representation” that is stable and ergonomic for MCP clients.

## Effects and capabilities for tools

Tools will be effectful in practice (filesystem, HTTP, etc).

Plan:
- The tool metadata includes required effects (e.g. `Effect File _`, `Effect Http _`).
- The MCP host decides which effects are allowed:
  - deny by default
  - allow via config (capability-based)
- WASI permissions are configured at runtime (preopened dirs, env vars, etc).

## CLI UX

`aivi mcp serve` should support:
- `--entry <module>` (which module contains the annotated tools)
- `--allow-fs <dir>` / `--deny-net` / `--allow-net <host>` etc
- `--transport stdio|http` (depending on MCP deployment)
- `--watch` (recompile on change, restart server)

## Test strategy

- Snapshot metadata extraction for a sample tools module.
- Integration test: start MCP host, invoke a tool, assert JSON result.
- Security tests: ensure denied capabilities are not accessible.

