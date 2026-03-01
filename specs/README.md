# AIVI Language Specification

A high-integrity functional language with a Rust-first compilation pipeline.

**Version:** 0.1 (Draft)

## Table of Contents

<!-- toc:start -->

### Overview
- [Spec Home](index.md)
- [Introduction](introduction.md)
- [Missing Features (v0.1)](missing_features_v0.1.md)

### Syntax
#### Core Forms
- [Bindings and Scope](syntax/bindings.md)
- [Functions and Pipes](syntax/functions.md)
- [Primitive Types](syntax/types/primitive_types.md)
- [Algebraic Data Types](syntax/types/algebraic_data_types.md)
- [Closed Records](syntax/types/closed_records.md)
- [Record Row Transforms](syntax/types/record_row_transforms.md)
- [Classes and HKTs](syntax/types/classes_and_hkts.md)
- [Expected-Type Coercions](syntax/types/expected_type_coercions.md)
- [Predicates](syntax/predicates.md)
#### Data & Flow
- [Patching Records](syntax/patching.md)
- [Domains, Units, and Deltas](syntax/domains.md)
- [Generators](syntax/generators.md)
- [Pattern Matching](syntax/pattern_matching.md)
#### Effects & Control
- [Effects](syntax/effects.md)
- [Machines](syntax/machines_runtime.md)
- [Resources](syntax/resources.md)
- [Generic `do` Notation](syntax/do_notation.md)
#### Modules & Interop
- [Modules](syntax/modules.md)
- [External Sources](syntax/external_sources.md)
##### Source Integrations
- [File Sources](syntax/external_sources/file.md)
- [REST/HTTP Sources](syntax/external_sources/rest_http.md)
- [Environment Sources](syntax/external_sources/environment.md)
- [IMAP Email Sources](syntax/external_sources/imap_email.md)
- [Image Sources](syntax/external_sources/image.md)
- [Compile-Time Sources](syntax/external_sources/compile_time.md)
#### Notation & Grammar
- [Operators and Context](syntax/operators.md)
- [Sigils](syntax/sigils.md)
##### Decorators
- [Overview](syntax/decorators.md)
- [@static](syntax/decorators/static.md)
- [@native](syntax/decorators/native.md)
- [@deprecated](syntax/decorators/deprecated.md)
- [@debug](syntax/decorators/debug.md)
- [@no_prelude](syntax/decorators/no_prelude.md)
- [Comments](syntax/comments.md)
- [Concrete Syntax](syntax/grammar.md)

### Standard Library
#### Core & Utils
- [Prelude](stdlib/core/prelude.md)
- [Text](stdlib/core/text.md)
- [Logic](stdlib/core/logic.md)
- [Units](stdlib/core/units.md)
- [Regex](stdlib/core/regex.md)
- [Collections](stdlib/core/collections.md)
- [I18n](stdlib/core/i18n.md)
- [Generator](stdlib/core/generator.md)
- [Validation](stdlib/core/validation.md)
#### Data
- [JSON](stdlib/data/json.md)
#### Math & Science
- [Math](stdlib/math/math.md)
- [Vector](stdlib/math/vector.md)
- [Matrix](stdlib/math/matrix.md)
- [Numbers](stdlib/math/number.md)
- [Probability](stdlib/math/probability.md)
- [FFT & Signal](stdlib/math/signal.md)
- [Geometry](stdlib/math/geometry.md)
- [Graph](stdlib/math/graph.md)
- [Linear Algebra](stdlib/math/linear_algebra.md)
- [Tree](stdlib/math/tree.md)
#### Time
- [Instant](stdlib/chronos/instant.md)
- [Calendar](stdlib/chronos/calendar.md)
- [Duration](stdlib/chronos/duration.md)
- [TimeZone](stdlib/chronos/timezone.md)
- [Scheduler](stdlib/chronos/scheduler.md)
#### System
- [Crypto](stdlib/system/crypto.md)
- [GOA](stdlib/system/goa.md)
- [Secrets](stdlib/system/secrets.md)
- [System](stdlib/system/system.md)
- [Log](stdlib/system/log.md)
- [Concurrency](stdlib/system/concurrency.md)
#### IO
- [File](stdlib/system/file.md)
- [Console](stdlib/system/console.md)
- [Database](stdlib/system/database.md)
- [Email](stdlib/system/email.md)
- [Path](stdlib/system/path.md)
- [URL](stdlib/system/url.md)
#### Network
- [HTTP](stdlib/network/http.md)
- [HTTPS](stdlib/network/https.md)
- [HTTP Server](stdlib/network/http_server.md)
- [REST](stdlib/network/rest.md)
- [Sockets](stdlib/network/sockets.md)
- [Streams](stdlib/network/streams.md)
#### UI
- [GTK4](stdlib/ui/gtk4.md)
- [Layout](stdlib/ui/layout.md)
- [VDOM](stdlib/ui/vdom.md)
- [HTML Sigil](stdlib/ui/html.md)
- [Color](stdlib/ui/color.md)
- [ServerHtml](stdlib/ui/server_html.md)

### Runtime
- [Concurrency](runtime/concurrency.md)
- [Package Manager & Packaging](runtime/package_manager.md)

### Tooling
- [CLI](tools/cli.md)
- [LSP Server](tools/lsp_server.md)
- [VSCode Extension](tools/vscode_extension.md)
- [Spec Doc Markers](doc-markers-spec.md)
#### Testing
- [Testing Module](stdlib/core/testing.md)
- [@test Decorator](syntax/decorators/test.md)
- [Mock Expressions](syntax/decorators/mock.md)

### Compiler & Backend
- [Compiler & Backend Design](typed_codegen/design.md)
- [Minimality Proof](typed_codegen/minimality.md)

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
