# AIVI Language Specification

AIVI is a statically typed, purely functional language with a Rust-based compiler toolchain. This folder is the main guide to the language: it explains the ideas, defines the syntax and semantics, and documents the standard library and tooling.

If you are reading the spec in a browser, start with **[Language at a Glance](language-overview.md)** and **[Introduction](introduction.md)**. If you are editing or reviewing the docs in the repository, this README also shows how to preview the site locally.

## How to read the spec

- **Language at a Glance** is the short tour. It is the fastest way to learn the feel of the language.
- **Introduction** explains the design in plain language, including how AIVI handles data, effects, errors, and external systems.
- **Language Reference** is the precise reference for syntax and behavior.
- **Standard Library** is where you look up the modules you will actually import in code.
- **Tooling** covers the CLI, LSP, and editor integration.

## Table of Contents

<!-- toc:start -->

### Getting Started
- [Language at a Glance](language-overview.md)
- [Introduction](introduction.md)

### Native Apps
- [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps.md)
- [App Architecture](stdlib/ui/app_architecture.md)
#### Signals & Dataflow
- [Reactive Signals](stdlib/ui/reactive_signals.md)
- [Reactive Dataflow](stdlib/ui/reactive_dataflow.md)
- [GTK & libadwaita Runtime](stdlib/ui/gtk4.md)
- [Forms](stdlib/ui/forms.md)
- [Layout](stdlib/ui/layout.md)
- [Color](stdlib/ui/color.md)
- [HTML Sigil](stdlib/ui/html.md)
- [Virtual DOM](stdlib/ui/vdom.md)

### Language Reference
#### Foundations
- [Bindings & Scope](syntax/bindings.md)
- [Functions & Pipes](syntax/functions.md)
#### Types
- [Primitive Types](syntax/types/primitive_types.md)
- [Algebraic Data Types](syntax/types/algebraic_data_types.md)
- [Closed Records](syntax/types/closed_records.md)
- [Record Row Transforms](syntax/types/record_row_transforms.md)
- [Higher Kinded Types](syntax/types/classes_and_hkts.md)
- [Expected-Type Coercions](syntax/types/expected_type_coercions.md)
- [Opaque Types](syntax/types/opaque_types.md)
#### Data & Patterns
- [Pattern Matching](syntax/pattern_matching.md)
- [Predicates](syntax/predicates.md)
- [Patching Records](syntax/patching.md)
- [Domains & Units](syntax/domains.md)
- [Generators](syntax/generators.md)
#### Effects & Control
- [Effects](syntax/effects.md)
- [Capabilities](syntax/capabilities.md)
- [Effect Handlers](syntax/effect_handlers.md)
- [do Notation](syntax/do_notation.md)
- [Resources](syntax/resources.md)
#### State Machines
- [Overview](syntax/state_machines.md)
- [Machine Syntax](syntax/machines.md)
- [Machine Runtime](syntax/machines_runtime.md)
#### Modules & Interop
- [Modules](syntax/modules.md)
- [External Sources](syntax/external_sources.md)
- [Schema-First Source Definitions](syntax/external_sources/schema_first.md)
- [Source Composition](syntax/external_sources/composition.md)
- [File Sources](syntax/external_sources/file.md)
- [REST / HTTP Sources](syntax/external_sources/rest_http.md)
- [Environment Sources](syntax/external_sources/environment.md)
- [IMAP Email Sources](syntax/external_sources/imap_email.md)
- [Image Sources](syntax/external_sources/image.md)
- [Compile-Time Sources](syntax/external_sources/compile_time.md)
#### Decorators
- [Overview](syntax/decorators/index.md)
- [@static](syntax/decorators/static.md)
- [@native](syntax/decorators/native.md)
- [@deprecated](syntax/decorators/deprecated.md)
- [@debug](syntax/decorators/debug.md)
- [@test](syntax/decorators/test.md)
- [@no_prelude](syntax/decorators/no_prelude.md)
#### Syntax Details
- [Operators & Context](syntax/operators.md)
- [Grammar Reference](syntax/grammar.md)

### Standard Library
#### Core
- [Prelude](stdlib/core/prelude.md)
- [Option](stdlib/core/option.md)
- [Result](stdlib/core/result.md)
- [Text](stdlib/core/text.md)
- [Logic](stdlib/core/logic.md)
- [Collections](stdlib/core/collections.md)
- [Generator](stdlib/core/generator.md)
- [Regex](stdlib/core/regex.md)
- [Validation](stdlib/core/validation.md)
- [I18n](stdlib/core/i18n.md)
- [Units](stdlib/core/units.md)
#### Data
- [Bits](stdlib/data/bits.md)
- [JSON](stdlib/data/json.md)
#### Math & Science
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
#### Time
- [Instant](stdlib/chronos/instant.md)
- [Calendar](stdlib/chronos/calendar.md)
- [Duration](stdlib/chronos/duration.md)
- [TimeZone](stdlib/chronos/timezone.md)
- [Scheduler](stdlib/chronos/scheduler.md)
#### System & IO
- [File](stdlib/system/file.md)
- [Console](stdlib/system/console.md)
- [Path](stdlib/system/path.md)
- [URL](stdlib/system/url.md)
- [System](stdlib/system/system.md)
- [Log](stdlib/system/log.md)
- [Concurrency](stdlib/system/concurrency.md)
#### Security
- [Crypto](stdlib/system/crypto.md)
- [Secrets](stdlib/system/secrets.md)
#### Database
- [Database](stdlib/system/database.md)
#### Networking
- [HTTP & HTTPS](stdlib/network/http.md)
- [HTTP Server](stdlib/network/http_server.md)
- [REST](stdlib/network/rest.md)
- [Sockets](stdlib/network/sockets.md)
- [Streams](stdlib/network/streams.md)
#### Email
- [Email](stdlib/system/email.md)

### Testing
- [Testing Module](stdlib/core/testing.md)
- [Test Decorator & Mocking](syntax/decorators/test.md)

### Tooling
- [CLI](tools/cli.md)
- [LSP Server](tools/lsp_server.md)
- [Incremental Compilation](tools/incremental_compilation.md)
- [VSCode Extension](tools/vscode_extension.md)
- [Package Manager](tools/package_manager.md)

### Internals
- [Compiler & Backend](typed_codegen/design.md)
- [Minimality Proof](typed_codegen/minimality.md)

### Appendix
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
