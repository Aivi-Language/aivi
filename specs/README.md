# AIVI Language Specification

A high-integrity functional language with a Rust-first compilation pipeline.

**Version:** 0.1 (Draft)


## Table of Contents

<!-- toc:start -->

### Overview
- [Spec Home](index.md)
- [Introduction](01_introduction.md)
- [Missing Features (v0.1)](missing_features_v0.1.md)

### Syntax
#### Core Forms
- [Bindings and Scope](02_syntax/01_bindings.md)
- [Functions and Pipes](02_syntax/02_functions.md)
- [The Type System](02_syntax/03_types.md)
- [Predicates](02_syntax/04_predicates.md)
#### Data & Flow
- [Patching Records](02_syntax/05_patching.md)
- [Domains, Units, and Deltas](02_syntax/06_domains.md)
- [Generators](02_syntax/07_generators.md)
- [Pattern Matching](02_syntax/08_pattern_matching.md)
#### Effects & Control
- [Effects](02_syntax/09_effects.md)
- [Resources](02_syntax/15_resources.md)
- [Generic `do` Notation](02_syntax/16_do_notation.md)
#### Modules & Interop
- [Modules](02_syntax/10_modules.md)
- [External Sources](02_syntax/12_external_sources.md)
#### Notation & Grammar
- [Operators and Context](02_syntax/11_operators.md)
- [Sigils](02_syntax/13_sigils.md)
- [Decorators](02_syntax/14_decorators.md)
- [Comments](02_syntax/17_comments.md)
- [Concrete Syntax](02_syntax/00_grammar.md)

### Standard Library
#### Core & Utils
- [Prelude](05_stdlib/00_core/01_prelude.md)
- [Text](05_stdlib/00_core/02_text.md)
- [Logic](05_stdlib/00_core/03_logic.md)
- [Units](05_stdlib/00_core/16_units.md)
- [Regex](05_stdlib/00_core/24_regex.md)
- [Testing](05_stdlib/00_core/27_testing.md)
- [Collections](05_stdlib/00_core/28_collections.md)
- [I18n](05_stdlib/00_core/29_i18n.md)
- [Generator](05_stdlib/00_core/30_generator.md)
- [MutableMap](05_stdlib/00_core/31_mutable_map.md)
#### Math & Science
- [Math](05_stdlib/01_math/01_math.md)
- [Vector](05_stdlib/01_math/05_vector.md)
- [Matrix](05_stdlib/01_math/09_matrix.md)
- [Numbers](05_stdlib/01_math/10_number.md)
- [Probability](05_stdlib/01_math/13_probability.md)
- [FFT & Signal](05_stdlib/01_math/14_signal.md)
- [Geometry](05_stdlib/01_math/15_geometry.md)
- [Graph](05_stdlib/01_math/17_graph.md)
- [Linear Algebra](05_stdlib/01_math/18_linear_algebra.md)
- [Tree](05_stdlib/01_math/19_tree.md)
#### Time (Chronos)
- [Instant](05_stdlib/02_chronos/01_instant.md)
- [Calendar](05_stdlib/02_chronos/02_calendar.md)
- [Duration](05_stdlib/02_chronos/03_duration.md)
- [TimeZone](05_stdlib/02_chronos/04_timezone.md)
#### System & IO
- [File](05_stdlib/03_system/20_file.md)
- [Console](05_stdlib/03_system/21_console.md)
- [Crypto](05_stdlib/03_system/22_crypto.md)
- [Database](05_stdlib/03_system/23_database.md)
- [Path](05_stdlib/03_system/24_path.md)
- [URL](05_stdlib/03_system/25_url.md)
- [System](05_stdlib/03_system/26_system.md)
- [Log](05_stdlib/03_system/27_log.md)
- [Concurrency](05_stdlib/03_system/30_concurrency.md)
#### Network
- [HTTP](05_stdlib/03_network/01_http.md)
- [HTTPS](05_stdlib/03_network/02_https.md)
- [HTTP Server](05_stdlib/03_network/03_http_server.md)
- [Sockets](05_stdlib/03_network/04_sockets.md)
- [Streams](05_stdlib/03_network/05_streams.md)
#### UI
- [Layout](05_stdlib/04_ui/01_layout.md)
- [VDOM](05_stdlib/04_ui/02_vdom.md)
- [HTML Sigil](05_stdlib/04_ui/03_html.md)
- [Color](05_stdlib/04_ui/04_color.md)
- [ServerHtml](05_stdlib/04_ui/05_server_html.md)

### Semantics
#### Kernel (Core Calculus)
- [Core Terms](03_kernel/01_core_terms.md)
- [Types](03_kernel/02_types.md)
- [Records](03_kernel/03_records.md)
- [Patterns](03_kernel/04_patterns.md)
- [Predicates](03_kernel/05_predicates.md)
- [Traversals](03_kernel/06_traversals.md)
- [Generators](03_kernel/07_generators.md)
- [Effects](03_kernel/08_effects.md)
- [Classes](03_kernel/09_classes.md)
- [Domains](03_kernel/10_domains.md)
- [Patching](03_kernel/11_patching.md)
- [Minimality Proof](03_kernel/12_minimality.md)
#### Desugaring (Surface -> Kernel)
- [Bindings](04_desugaring/01_bindings.md)
- [Functions](04_desugaring/02_functions.md)
- [Records](04_desugaring/03_records.md)
- [Patterns](04_desugaring/04_patterns.md)
- [Predicates](04_desugaring/05_predicates.md)
- [Generators](04_desugaring/06_generators.md)
- [Effects](04_desugaring/07_effects.md)
- [Classes](04_desugaring/08_classes.md)
- [Domains and Operators](04_desugaring/09_domains.md)
- [Patching](04_desugaring/10_patching.md)
- [Resources](04_desugaring/11_resources.md)

### Runtime
- [Concurrency](06_runtime/01_concurrency.md)
- [Memory Management](06_runtime/02_memory_management.md)
- [Package Manager (Cargo-backed)](06_runtime/03_package_manager.md)

### Tooling
- [CLI](07_tools/01_cli.md)
- [LSP Server](07_tools/02_lsp_server.md)
- [VSCode Extension](07_tools/03_vscode_extension.md)
- [Packaging](07_tools/04_packaging.md)
- [Spec Doc Markers](doc-markers-spec.md)

### Compiler & Backend
- [Typed Codegen Design](08_typed_codegen/01_design.md)

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
