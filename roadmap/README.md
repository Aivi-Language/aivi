# Roadmap: Turning AIVI into a Real Language (Rust + LSP + MCP + WASM/WASI)

This folder is an implementation “battle plan” for evolving the AIVI spec into a working toolchain and ecosystem.

Assumptions:
- Implementation language: Rust.
- Primary compilation target: WebAssembly (`wasm32-wasi`, with an upgrade path to the WASM Component Model / WASI Preview 2).
- Editor support: LSP (Rust `tower-lsp`) and a VS Code extension that delegates to the LSP.
- AI/tooling integration: MCP server host written in Rust, with AIVI tools/resources compiled to WASM and executed under WASI.

Start here:
- `roadmap/01_overall_phases.md`
- `roadmap/02_rust_workspace_layout.md`
- `roadmap/03_language_implementation.md`
- `roadmap/04_compiler_wasm_wasi.md`
- `roadmap/05_language_server_lsp.md`
- `roadmap/06_mcp_integration.md`
- `roadmap/07_standard_library_plan.md`

