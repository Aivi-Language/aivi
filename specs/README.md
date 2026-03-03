# AIVI Language Specification

A high-integrity functional language with a Rust-first compilation pipeline.

**Version:** 0.1 (Draft)

## Table of Contents

<!-- toc:start -->

### Getting Started
- [Language at a Glance](language-overview.md)
- [Introduction](introduction.md)

### Language Reference
#### Foundations
- [Bindings & Scope](syntax/bindings.md)
- [Functions & Pipes](syntax/functions.md)
#### Types
- [Primitive Types](syntax/types/primitive_types.md)
- [Algebraic Data Types](syntax/types/algebraic_data_types.md)
- [Closed Records](syntax/types/closed_records.md)
- [Record Row Transforms](syntax/types/record_row_transforms.md)
- [Classes & HKTs](syntax/types/classes_and_hkts.md)
- [Expected-Type Coercions](syntax/types/expected_type_coercions.md)
#### Data & Patterns
- [Pattern Matching](syntax/pattern_matching.md)
- [Predicates](syntax/predicates.md)
- [Patching Records](syntax/patching.md)
- [Domains & Units](syntax/domains.md)
- [Generators](syntax/generators.md)
#### Effects & Control
- [Effects](syntax/effects.md)
- [do Notation](syntax/do_notation.md)
- [Resources](syntax/resources.md)
- [Machines](syntax/machines_runtime.md)
#### Modules & Interop
- [Modules](syntax/modules.md)
- [External Sources](syntax/external_sources.md)
- [File Sources](syntax/external_sources/file.md)
- [REST/HTTP Sources](syntax/external_sources/rest_http.md)
- [Environment Sources](syntax/external_sources/environment.md)
- [IMAP Email Sources](syntax/external_sources/imap_email.md)
- [Image Sources](syntax/external_sources/image.md)
- [Compile-Time Sources](syntax/external_sources/compile_time.md)
#### Notation
- [Operators & Sigils](syntax/operators.md)
- [Decorators](syntax/decorators.md)
- [Grammar](syntax/grammar.md)

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
- [JSON](stdlib/data/json.md)
#### Math & Science
- [Math](stdlib/math/math.md)
- [Numbers](stdlib/math/number.md)
- [Vector](stdlib/math/vector.md)
- [Matrix](stdlib/math/matrix.md)
- [Linear Algebra](stdlib/math/linear_algebra.md)
- [Probability](stdlib/math/probability.md)
- [FFT & Signal](stdlib/math/signal.md)
- [Geometry](stdlib/math/geometry.md)
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
#### UI
- [GTK4](stdlib/ui/gtk4.md)
- [Layout](stdlib/ui/layout.md)
- [VDOM](stdlib/ui/vdom.md)
- [HTML Sigil](stdlib/ui/html.md)
- [Color](stdlib/ui/color.md)

### Testing
- [Testing Module](stdlib/core/testing.md)
- [@test & Mocks](syntax/decorators.md#test-test-declarations)

### Tooling
- [CLI](tools/cli.md)
- [LSP Server](tools/lsp_server.md)
- [VSCode Extension](tools/vscode_extension.md)
- [Package Manager](tools/package_manager.md)

### Internals
- [Compiler & Backend](typed_codegen/design.md)
- [Minimality Proof](typed_codegen/minimality.md)

### Appendix
- [Spec Doc Markers](doc-markers-spec.md)

<!-- toc:end -->

## Building the Specification

### VitePress (recommended)

```bash
npm install
npm run docs:dev
```

Build static site:

```bash
npm run docs:build
```

## Source Documentation

For typed boundary integrations, start with:

- [`syntax/external_sources.md`](syntax/external_sources.md)
- `syntax/external_sources/file.md`
- `syntax/external_sources/rest_http.md`
- `syntax/external_sources/environment.md`
- `syntax/external_sources/imap_email.md`
- `syntax/external_sources/image.md`
- `syntax/external_sources/compile_time.md`

## Code of Conduct

This project follows the Contributor Covenant. Please read `../CODE_OF_CONDUCT.md`
before participating in discussions or contributions that touch the
specification.

## License

MIT License. See `../LICENCE.md`.
