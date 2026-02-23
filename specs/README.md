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
- [The Type System](syntax/types.md)
- [Predicates](syntax/predicates.md)
#### Data & Flow
- [Patching Records](syntax/patching.md)
- [Domains, Units, and Deltas](syntax/domains.md)
- [Generators](syntax/generators.md)
- [Pattern Matching](syntax/pattern_matching.md)
#### Effects & Control
- [Effects](syntax/effects.md)
- [Resources](syntax/resources.md)
- [Generic `do` Notation](syntax/do_notation.md)
#### Modules & Interop
- [Modules](syntax/modules.md)
- [External Sources](syntax/external_sources.md)
#### Notation & Grammar
- [Operators and Context](syntax/operators.md)
- [Sigils](syntax/sigils.md)
- [Decorators](syntax/decorators.md)
- [Comments](syntax/comments.md)
- [Concrete Syntax](syntax/grammar.md)

### Standard Library
#### Core & Utils
- [Prelude](stdlib/core/prelude.md)
- [Text](stdlib/core/text.md)
- [Logic](stdlib/core/logic.md)
- [Units](stdlib/core/units.md)
- [Regex](stdlib/core/regex.md)
- [Testing](stdlib/core/testing.md)
- [Collections](stdlib/core/collections.md)
- [I18n](stdlib/core/i18n.md)
- [Generator](stdlib/core/generator.md)
- [Validation](stdlib/core/validation.md)
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
#### Time (Chronos)
- [Instant](stdlib/chronos/instant.md)
- [Calendar](stdlib/chronos/calendar.md)
- [Duration](stdlib/chronos/duration.md)
- [TimeZone](stdlib/chronos/timezone.md)
#### System & IO
- [File](stdlib/system/file.md)
- [Console](stdlib/system/console.md)
- [Crypto](stdlib/system/crypto.md)
- [Database](stdlib/system/database.md)
- [Path](stdlib/system/path.md)
- [URL](stdlib/system/url.md)
- [System](stdlib/system/system.md)
- [Log](stdlib/system/log.md)
- [Concurrency](stdlib/system/concurrency.md)
#### Network
- [HTTP](stdlib/network/http.md)
- [HTTPS](stdlib/network/https.md)
- [HTTP Server](stdlib/network/http_server.md)
- [Sockets](stdlib/network/sockets.md)
- [Streams](stdlib/network/streams.md)
#### UI
- [GTK4](stdlib/ui/gtk4.md)
- [Layout](stdlib/ui/layout.md)
- [VDOM](stdlib/ui/vdom.md)
- [HTML Sigil](stdlib/ui/html.md)
- [Color](stdlib/ui/color.md)
- [ServerHtml](stdlib/ui/server_html.md)

### Semantics
#### Kernel (Core Calculus)
- [Core Terms](kernel/core_terms.md)
- [Types](kernel/types.md)
- [Records](kernel/records.md)
- [Patterns](kernel/patterns.md)
- [Predicates](kernel/predicates.md)
- [Traversals](kernel/traversals.md)
- [Generators](kernel/generators.md)
- [Effects](kernel/effects.md)
- [Classes](kernel/classes.md)
- [Domains](kernel/domains.md)
- [Patching](kernel/patching.md)
- [Minimality Proof](kernel/minimality.md)
#### Desugaring (Surface -> Kernel)
- [Bindings](desugaring/bindings.md)
- [Functions](desugaring/functions.md)
- [Records](desugaring/records.md)
- [Patterns](desugaring/patterns.md)
- [Predicates](desugaring/predicates.md)
- [Generators](desugaring/generators.md)
- [Effects](desugaring/effects.md)
- [Classes](desugaring/classes.md)
- [Domains and Operators](desugaring/domains.md)
- [Patching](desugaring/patching.md)
- [Resources](desugaring/resources.md)

### Runtime
- [Concurrency](runtime/concurrency.md)
- [Memory Management](runtime/memory_management.md)
- [Package Manager (Cargo-backed)](runtime/package_manager.md)

### Tooling
- [CLI](tools/cli.md)
- [LSP Server](tools/lsp_server.md)
- [VSCode Extension](tools/vscode_extension.md)
- [Packaging](tools/packaging.md)
- [Spec Doc Markers](doc-markers-spec.md)

### Compiler & Backend
- [Typed Codegen Design](typed_codegen/design.md)

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

## Code of Conduct

This project follows the Contributor Covenant. Please read `../CODE_OF_CONDUCT.md`
before participating in discussions or contributions that touch the
specification.

## License

MIT License. See `../LICENCE.md`.
