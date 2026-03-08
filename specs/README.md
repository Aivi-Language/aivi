# AIVI Language Specification

AIVI is a statically typed, purely functional language with a Rust-based compiler toolchain. This folder is the main guide to the language: it explains the big ideas, defines the syntax and semantics, and documents the standard library and tooling.

If you are new to AIVI, use this simple path first: **[Language at a Glance](language-overview.md)** for the fast tour, **[Introduction](introduction.md)** for the plain-language overview, then the **Learn AIVI** section in the table of contents from top to bottom. If you are editing or reviewing the docs in the repository, this README also shows how to preview the site locally.

## A simple reading path

1. **Start Here** to learn the feel of the language before you read detailed rules.
2. **Learn AIVI** in order: Basics → Data & Types → Effects & Workflows → Modules & External Data.
3. **Build Native Apps** if you are working on GTK or libadwaita applications.
4. Use **Standard Library**, **Testing & Tooling**, and **Internals** as lookup sections once you know what you need.

## How to read the spec

- **Start Here** gives you the shortest path into the language.
- **Learn AIVI** is the main learning track for everyday language features.
- **Build Native Apps** collects the GTK and UI material in one place.
- **Standard Library** is where you look up the modules you will actually import in code.
- **Testing & Tooling** covers tests, the CLI, the language server, and editor integration.
- **Internals** is for compiler and spec contributors.

## Table of Contents

<!-- toc:start -->

### Start Here
- [Language at a Glance](language-overview.md)
- [Introduction](introduction.md)

### Learn AIVI
#### Basics
- [Bindings & Scope](syntax/bindings.md)
- [Functions & Pipes](syntax/functions.md)
#### Data & Types
- [Type System Overview](syntax/types.md)
- [Primitive Types](syntax/types/primitive_types.md)
- [Custom Data Types (ADTs)](syntax/types/algebraic_data_types.md)
- [Records](syntax/types/closed_records.md)
- [Extending & Reshaping Records](syntax/types/record_row_transforms.md)
- [Pattern Matching](syntax/pattern_matching.md)
- [Predicates](syntax/predicates.md)
- [Updating Records](syntax/patching.md)
- [Helpful Type Conversions](syntax/types/expected_type_coercions.md)
- [Opaque Types](syntax/types/opaque_types.md)
- [Domains & Units](syntax/domains.md)
#### Effects & Workflows
- [Effects](syntax/effects.md)
- [do Notation](syntax/do_notation.md)
- [Resources](syntax/resources.md)
- [Generators](syntax/generators.md)
- [Capabilities](syntax/capabilities.md)
- [Effect Handlers](syntax/effect_handlers.md)
##### State Machines
- [Overview](syntax/state_machines.md)
- [Machine Syntax](syntax/machines.md)
- [Machine Runtime](syntax/machines_runtime.md)
#### Modules & External Data
- [Modules](syntax/modules.md)
##### External Sources
- [Overview](syntax/external_sources.md)
- [Define Sources from Schemas](syntax/external_sources/schema_first.md)
- [Combine Sources](syntax/external_sources/composition.md)
- [File Sources](syntax/external_sources/file.md)
- [REST / HTTP Sources](syntax/external_sources/rest_http.md)
- [Environment Sources](syntax/external_sources/environment.md)
- [IMAP Email Sources](syntax/external_sources/imap_email.md)
- [Image Sources](syntax/external_sources/image.md)
- [Compile-Time Sources](syntax/external_sources/compile_time.md)
#### Advanced Features
- [Reusable Type Patterns (Classes & HKTs)](syntax/types/classes_and_hkts.md)
##### Decorators
- [Overview](syntax/decorators/index.md)
- [@static](syntax/decorators/static.md)
- [@native](syntax/decorators/native.md)
- [@deprecated](syntax/decorators/deprecated.md)
- [@debug](syntax/decorators/debug.md)
- [@test](syntax/decorators/test.md)
- [@no_prelude](syntax/decorators/no_prelude.md)
- [Operators & Context](syntax/operators.md)
- [Grammar Reference](syntax/grammar.md)

### Build Native Apps
#### Getting Started
- [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps.md)
- [App Architecture](stdlib/ui/app_architecture.md)
- [GTK & libadwaita Runtime](stdlib/ui/gtk4.md)
#### Reactive UI
- [Reactive Signals](stdlib/ui/reactive_signals.md)
- [Reactive Dataflow](stdlib/ui/reactive_dataflow.md)
#### UI Building Blocks
- [Forms](stdlib/ui/forms.md)
- [Layout](stdlib/ui/layout.md)
- [Color](stdlib/ui/color.md)
- [HTML Sigil](stdlib/ui/html.md)
- [Virtual DOM](stdlib/ui/vdom.md)

### Standard Library
#### Core Building Blocks
- [Prelude](stdlib/core/prelude.md)
- [Option](stdlib/core/option.md)
- [Result](stdlib/core/result.md)
- [Logic](stdlib/core/logic.md)
- [Collections](stdlib/core/collections.md)
- [Generator](stdlib/core/generator.md)
- [Validation](stdlib/core/validation.md)
#### Text & Data
- [Text](stdlib/core/text.md)
- [Regex](stdlib/core/regex.md)
- [I18n](stdlib/core/i18n.md)
- [JSON](stdlib/data/json.md)
- [Bits](stdlib/data/bits.md)
#### Math, Units & Models
- [Units](stdlib/core/units.md)
- [Math](stdlib/math/math.md)
- [Numbers](stdlib/math/number.md)
- [Vector](stdlib/math/vector.md)
- [Matrix](stdlib/math/matrix.md)
- [Linear Algebra](stdlib/math/linear_algebra.md)
- [Geometry](stdlib/math/geometry.md)
- [Probability](stdlib/math/probability.md)
- [Signal Processing](stdlib/math/signal.md)
- [Graph](stdlib/math/graph.md)
- [Tree](stdlib/math/tree.md)
#### Time & Scheduling
- [Instant](stdlib/chronos/instant.md)
- [Calendar](stdlib/chronos/calendar.md)
- [Duration](stdlib/chronos/duration.md)
- [TimeZone](stdlib/chronos/timezone.md)
- [Scheduler](stdlib/chronos/scheduler.md)
#### Files, System & Security
- [File](stdlib/system/file.md)
- [Console](stdlib/system/console.md)
- [Path](stdlib/system/path.md)
- [URL](stdlib/system/url.md)
- [System](stdlib/system/system.md)
- [Log](stdlib/system/log.md)
- [Concurrency](stdlib/system/concurrency.md)
- [Crypto](stdlib/system/crypto.md)
- [Secrets](stdlib/system/secrets.md)
#### Network, Services & Storage
- [HTTP & HTTPS](stdlib/network/http.md)
- [HTTP Server](stdlib/network/http_server.md)
- [REST](stdlib/network/rest.md)
- [Sockets](stdlib/network/sockets.md)
- [Streams](stdlib/network/streams.md)
- [Database](stdlib/system/database.md)
- [Email](stdlib/system/email.md)

### Testing & Tooling
#### Testing
- [Testing Module](stdlib/core/testing.md)
- [Test Decorator & Mocking](syntax/decorators/test.md)
#### Developer Tools
- [CLI](tools/cli.md)
- [MCP Server](tools/mcp.md)
- [Package Manager](tools/package_manager.md)
- [LSP Server](tools/lsp_server.md)
- [VSCode Extension](tools/vscode_extension.md)
- [Incremental Compilation](tools/incremental_compilation.md)

### Internals
- [Compiler & Backend](typed_codegen/design.md)
- [Minimality Proof](typed_codegen/minimality.md)
- [Spec Doc Markers](doc-markers-spec.md)

<!-- toc:end -->

## Preview the docs locally

### VitePress

Run these commands from `specs/`:

```bash
pnpm install --frozen-lockfile
pnpm docs:dev
```

Build the static site locally:

```bash
pnpm docs:build
```

## Helpful starting points for typed integrations

When you want to understand how AIVI interacts with files, services, environment variables, or other boundaries, begin with:

- [`syntax/external_sources.md`](syntax/external_sources.md)
- [`syntax/external_sources/file.md`](syntax/external_sources/file.md)
- [`syntax/external_sources/rest_http.md`](syntax/external_sources/rest_http.md)
- [`syntax/external_sources/environment.md`](syntax/external_sources/environment.md)
- [`syntax/external_sources/imap_email.md`](syntax/external_sources/imap_email.md)
- [`syntax/external_sources/image.md`](syntax/external_sources/image.md)
- [`syntax/external_sources/compile_time.md`](syntax/external_sources/compile_time.md)

## Code of Conduct

This project follows the Contributor Covenant. Please read `../CODE_OF_CONDUCT.md` before joining discussions or contributing to the specification.

## License

MIT License. See `../LICENCE.md`.
