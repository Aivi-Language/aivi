# AIVI Tooling: Compiler, LSP, and MCP

## Compiler Language Candidates

| Language | Strengths | Fit for AIVI |
| :--- | :--- | :--- |
| **Rust** | Memory safety, WASM ecosystem, fast | Primary recommendation |
| **Zig** | Simpler than Rust, C interop, WASM output | Good for bootstrap |
| **OCaml** | Parser/compiler heritage, ADTs, functional | Academic pedigree |

### Recommendation: Rust

- **cranelift** for code generation (used by Wasmtime)
- **wasm-encoder** for direct WASM output
- **logos** + **chumsky** for lexing/parsing
- Strong type system matches AIVI semantics


## Language Server (LSP)

### Option A: Rust-based

```text
aivi-lsp (Rust)
├── tower-lsp for protocol
├── salsa for incremental computation
└── shares parser with compiler
```

Advantages:
- Shared codebase with compiler
- Fast, low memory
- Works in VS Code, Neovim, Helix

### Option B: Self-hosted in AIVI

Once AIVI can compile itself:

```aivi
module aivi.lsp = {
  use aivi.compiler/parser
  use aivi.compiler/types
  
  handleCompletion : Request -> Effect Lsp Response
  handleHover : Request -> Effect Lsp Response
}
```

The LSP runs as WASM module inside any editor supporting WASI.


## MCP Integration (Model Context Protocol)

AIVI has **built-in MCP server capability**.

### Tools as Typed Functions

```aivi
@mcp_tool
fetchWeather : City -> Effect Http WeatherData
fetchWeather city = http.get "https://api.weather.com/{city.id}"

@mcp_resource
configFile : Source File Config
configFile = file.read "./config.aivi"
```

Decorators register functions as MCP tools/resources.

### Schema Generation

AIVI types automatically generate JSON Schema for MCP:

```aivi
City = { id: Text, name: Text, country: Text }

// Generates:
// { "type": "object", "properties": { "id": {...}, ... } }
```

### Runtime

```text
aivi-mcp serve my-tools.aivi
```

Spawns an MCP-compliant server exposing all `@mcp_tool` functions.


## Development Workflow

```text
aivi check   // Type check, no emit
aivi build   // Compile to WASM
aivi run     // Execute via Wasmtime
aivi lsp     // Start language server
aivi mcp     // Start MCP server
aivi repl    // Interactive REPL
```


## Self-Hosting Timeline

1. **Phase 1**: Rust compiler → WASM output
2. **Phase 2**: LSP in Rust, shares compiler
3. **Phase 3**: AIVI standard library complete
4. **Phase 4**: Compiler rewritten in AIVI
5. **Phase 5**: LSP rewritten in AIVI (dogfooding)
