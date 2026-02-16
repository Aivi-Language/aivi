# aivi-lsp

This crate implements the AIVI Language Server Protocol (LSP) server on top of
`tower_lsp` and the `aivi` compiler. It is both the production server and a
reference for building alternate LSP implementations.

If you are adding language behavior, check `specs/` first and keep changes in
sync with the compiler and integration tests.

## Quick start

Install the binary from the repo:

```bash
cargo install --path crates/aivi_lsp
```

Run the server over stdio (for editors that spawn it directly):

```bash
cargo run -p aivi-lsp
```

## Code map

- `src/lib.rs`: main LSP implementation (handlers, parsing, inference glue).
- `src/state.rs`: shared backend state and workspace module index.
- `src/navigation.rs`: definition and reference helpers.
- `src/semantic_tokens.rs`: semantic token legend and mapping.

## Architecture and extension guide

To build a new Language Server Protocol (LSP) implementation for the AIVI
language that compiles to Rust, you can reuse much of the logic from the
existing `aivi_lsp` crate while structuring your own server. The existing
crate demonstrates how to integrate parsing, type inference, and workspace
indexing from the `aivi` compiler with the `tower_lsp` framework to support
completions, hover text, go-to declaration/implementation and signature help.
Below is a guide that explains how the existing implementation works and how to
adapt it for your own server.

### 1. Create the server skeleton with tower_lsp

1. Add dependencies: in your new crate's `Cargo.toml`, depend on `tower_lsp`
   for the LSP implementation and on `aivi` for parsing/inference. You may also
   pull in `tokio` for async runtime and `serde` if you need to manipulate JSON.
2. Implement `LanguageServer`: define a `Backend` struct with a
   `tower_lsp::Client` and some mutable state (open documents, workspace module
   index, etc.). In the existing server this state is encapsulated in
   `BackendState` and protected by an `Arc<Mutex<...>>`. Your `Backend` should
   implement the `LanguageServer` trait from `tower_lsp`, implementing methods
   such as `initialize`, `did_open`, `did_change`, `completion`, `hover`,
   `goto_definition`, `goto_declaration`, `goto_implementation`,
   `signature_help`, `references` and `shutdown`.
3. Advertise capabilities: in `initialize` return an `InitializeResult`
   describing which LSP features you support. The existing server advertises
   `completion_provider`, `hover_provider`, `definition_provider`,
   `declaration_provider`, `implementation_provider`,
   `signature_help_provider`, `references_provider`, `rename_provider`,
   `semantic_tokens_provider` and `code_action_provider`.

### 2. Parsing modules and indexing the workspace

1. Parse documents: for every open document, parse it into modules and collect
   diagnostics. In the existing server this is done in `build_diagnostics` by
   calling `parse_modules` from the `aivi` crate and mapping any returned
   diagnostics into LSP diagnostics. When `did_open` or `did_change` fires,
   update the document text and call `parse_modules` to produce updated
   diagnostics and to rebuild any completions and type information.
2. Index the workspace: to support go-to definition/implementation across
   files, maintain a map from module names to `IndexedModule` instances
   (containing the module AST and its URI). On initialization or whenever the
   workspace root changes, scan the project root for `.aivi` files, parse them
   with `parse_modules` and populate this index. The existing server stores this
   in `BackendState` and updates it when new files are opened or removed.

### 3. Implementing auto-completion and suggestions

1. Completion items: provide a method (like `build_completion_items`) that
   takes the current document text and returns a list of `CompletionItem`s. The
   existing server constructs items by:
   - adding keywords and sigils (punctuation-based snippets),
   - adding module names and their exported names,
   - iterating through each module's items and mapping them to completion labels
     and kinds via a helper (`completion_from_item`), which maps function/type
     signatures to `FUNCTION`, type declarations to `STRUCT`, type aliases to
     `TYPE_PARAMETER`, class declarations to `CLASS`, instance declarations to
     `VARIABLE`, and domain declarations to `MODULE`.
2. LSP handler: in your `completion` method, retrieve the document text, call
   your completion builder and wrap the result in a `CompletionResponse`. The
   existing code simply returns an array of items since it does not implement
   lazy `completionItem/resolve`.

### 4. Hover information and showing types

1. Type inference: use `infer_value_types` from the `aivi` crate to infer the
   types of definitions and expressions in a module. In
   `build_hover_with_workspace` the existing implementation parses the current
   module, collects all workspace modules, runs inference, and then looks up the
   type of the identifier under the cursor.
2. Documentation lookup: extract any preceding doc comments for a symbol using
   a helper (`doc_for_ident`) and merge them with the type. The `hover` method
   returns a `Hover` object with markdown content containing the signature,
   type and documentation.
3. Workspace lookup: if the identifier refers to another module (has a dotted
   prefix) or comes from an import, search the workspace module index for its
   definition and associated documentation, and use those for the hover text.

### 5. Go-to definition, declaration and implementation

1. Find identifier: implement a helper (`extract_identifier`) that finds the
   identifier at the cursor position and another helper
   (`module_member_definition_range`) that returns the range of a definition in
   a module. The existing `build_definition` calls these helpers: it parses
   modules, checks if the identifier matches a module name, module export or
   module item, and returns the location of that definition.
2. Workspace resolution: to support cross-file navigation, implement
   `build_definition_with_workspace` that, after checking the current module,
   inspects imported modules and the workspace module index. If the identifier
   contains a module prefix (for example, `module.name`), resolve the prefix in
   the index and return the definition's `Location`.
3. Reuse for implementations: in the existing `goto_implementation` and
   `goto_declaration` handlers, the server simply calls
   `build_definition_with_workspace` to return the same location for both
   declarations and implementations. If your language distinguishes
   implementations from declarations (for example, separate interface and
   implementation files), adjust the logic to return implementation locations.

### 6. Signature help (argument types)

1. Detect function calls: use an AST traversal to find call expressions that
   encompass the cursor. The existing code uses `find_call_info` to walk the
   expression tree and return the called function expression and which argument
   index is active.
2. Lookup type signature: once the callee's name is known, look up its type
   signature in the current module or imported modules.
   `resolve_type_signature_label` checks for a type signature
   (`ModuleItem::TypeSig`) or falls back to the inferred type of the function
   if no explicit signature is present.
3. Build `SignatureHelp`: construct a `SignatureHelp` object with a single
   `SignatureInformation` whose `label` contains the type signature. Set the
   `active_parameter` based on the call info to highlight the current argument.
   Return this from your `signature_help` handler.

### 7. Diagnostics, references and other features (optional)

1. Diagnostics: use `parse_modules` to return syntax/type errors and convert
   them into LSP diagnostics (severity `ERROR`). Send them using
   `publish_diagnostics` whenever a document is opened or changed.
2. References: if you wish to support "Find References", traverse the AST and
   collect locations where an identifier appears. The existing implementation
   uses `collect_module_references` and `collect_item_references` to gather
   locations from the current module and workspace modules.
3. Rename: after collecting all reference locations, build a `WorkspaceEdit`
   mapping document URIs to lists of text edits. The existing
   `build_rename_with_workspace` enforces simple renaming rules (identifiers
   cannot contain dots or non-alphanumeric characters).

### Summary

The existing `aivi_lsp` crate provides a complete example of integrating the
AIVI compiler's parsing and type inference capabilities with `tower_lsp`. By
examining its functions for completions, hover, go-to definition and signature
help, you can implement a new LSP server that provides:

- auto-completion of keywords, sigils, module names and definitions by
  iterating through parsed modules and mapping `ModuleItem`s to appropriate
  `CompletionItemKind` values,
- hover text that shows type information and documentation by using
  `infer_value_types` and extracting doc comments,
- go-to declaration/implementation by locating the identifier's definition
  within the current module or across the workspace,
- signature help that displays argument types based on explicit or inferred
  type signatures and highlights the active parameter.

Following these patterns, you can build your own LSP server for the AIVI
language that provides a robust developer experience and integrates smoothly
into editors supporting the LSP.
