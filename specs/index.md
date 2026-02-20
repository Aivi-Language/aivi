---
title: AIVI Language Specification
---

<p>
    <img src="/aivi-logo.png" alt="AIVI Logo" width="256" height="auto">
</p>

# domain-driven & purely functional

A high-integrity purely functional language with a Rust-first compilation pipeline.

## Table of Contents

<!-- toc:start -->

### Overview
- [Spec Home](index)
- [Introduction](01_introduction)
- [Missing Features (v0.1)](missing_features_v0.1)

### Syntax
#### Core Forms
- [Bindings and Scope](02_syntax/01_bindings)
- [Functions and Pipes](02_syntax/02_functions)
- [The Type System](02_syntax/03_types)
- [Predicates](02_syntax/04_predicates)
#### Data & Flow
- [Patching Records](02_syntax/05_patching)
- [Domains, Units, and Deltas](02_syntax/06_domains)
- [Generators](02_syntax/07_generators)
- [Pattern Matching](02_syntax/08_pattern_matching)
#### Effects & Control
- [Effects](02_syntax/09_effects)
- [Resources](02_syntax/15_resources)
- [Generic `do` Notation](02_syntax/16_do_notation)
#### Modules & Interop
- [Modules](02_syntax/10_modules)
- [External Sources](02_syntax/12_external_sources)
#### Notation & Grammar
- [Operators and Context](02_syntax/11_operators)
- [Sigils](02_syntax/13_sigils)
- [Decorators](02_syntax/14_decorators)
- [Comments](02_syntax/17_comments)
- [Concrete Syntax](02_syntax/00_grammar)

### Standard Library
#### Core & Utils
- [Prelude](05_stdlib/00_core/01_prelude)
- [Text](05_stdlib/00_core/02_text)
- [Logic](05_stdlib/00_core/03_logic)
- [Units](05_stdlib/00_core/16_units)
- [Regex](05_stdlib/00_core/24_regex)
- [Testing](05_stdlib/00_core/27_testing)
- [Collections](05_stdlib/00_core/28_collections)
- [I18n](05_stdlib/00_core/29_i18n)
- [Generator](05_stdlib/00_core/30_generator)
- [MutableMap](05_stdlib/00_core/31_mutable_map)
- [MutableMap](05_stdlib/00_core/31_mutable_map)
#### Math & Science
- [Math](05_stdlib/01_math/01_math)
- [Vector](05_stdlib/01_math/05_vector)
- [Matrix](05_stdlib/01_math/09_matrix)
- [Numbers](05_stdlib/01_math/10_number)
- [Probability](05_stdlib/01_math/13_probability)
- [FFT & Signal](05_stdlib/01_math/14_signal)
- [Geometry](05_stdlib/01_math/15_geometry)
- [Graph](05_stdlib/01_math/17_graph)
- [Linear Algebra](05_stdlib/01_math/18_linear_algebra)
- [Tree](05_stdlib/01_math/19_tree)
#### Time (Chronos)
- [Instant](05_stdlib/02_chronos/01_instant)
- [Calendar](05_stdlib/02_chronos/02_calendar)
- [Duration](05_stdlib/02_chronos/03_duration)
- [TimeZone](05_stdlib/02_chronos/04_timezone)
#### System & IO
- [File](05_stdlib/03_system/20_file)
- [Console](05_stdlib/03_system/21_console)
- [Crypto](05_stdlib/03_system/22_crypto)
- [Database](05_stdlib/03_system/23_database)
- [Path](05_stdlib/03_system/24_path)
- [URL](05_stdlib/03_system/25_url)
- [System](05_stdlib/03_system/26_system)
- [Log](05_stdlib/03_system/27_log)
- [Concurrency](05_stdlib/03_system/30_concurrency)
#### Network
- [HTTP](05_stdlib/03_network/01_http)
- [HTTPS](05_stdlib/03_network/02_https)
- [HTTP Server](05_stdlib/03_network/03_http_server)
- [Sockets](05_stdlib/03_network/04_sockets)
- [Streams](05_stdlib/03_network/05_streams)
#### UI
- [Layout](05_stdlib/04_ui/01_layout)
- [VDOM](05_stdlib/04_ui/02_vdom)
- [HTML Sigil](05_stdlib/04_ui/03_html)
- [Color](05_stdlib/04_ui/04_color)
- [ServerHtml](05_stdlib/04_ui/05_server_html)

### Semantics
#### Kernel (Core Calculus)
- [Core Terms](03_kernel/01_core_terms)
- [Types](03_kernel/02_types)
- [Records](03_kernel/03_records)
- [Patterns](03_kernel/04_patterns)
- [Predicates](03_kernel/05_predicates)
- [Traversals](03_kernel/06_traversals)
- [Generators](03_kernel/07_generators)
- [Effects](03_kernel/08_effects)
- [Classes](03_kernel/09_classes)
- [Domains](03_kernel/10_domains)
- [Patching](03_kernel/11_patching)
- [Minimality Proof](03_kernel/12_minimality)
#### Desugaring (Surface -> Kernel)
- [Bindings](04_desugaring/01_bindings)
- [Functions](04_desugaring/02_functions)
- [Records](04_desugaring/03_records)
- [Patterns](04_desugaring/04_patterns)
- [Predicates](04_desugaring/05_predicates)
- [Generators](04_desugaring/06_generators)
- [Effects](04_desugaring/07_effects)
- [Classes](04_desugaring/08_classes)
- [Domains and Operators](04_desugaring/09_domains)
- [Patching](04_desugaring/10_patching)
- [Resources](04_desugaring/11_resources)

### Runtime
- [Concurrency](06_runtime/01_concurrency)
- [Memory Management](06_runtime/02_memory_management)
- [Package Manager (Cargo-backed)](06_runtime/03_package_manager)

### Tooling
- [CLI](07_tools/01_cli)
- [LSP Server](07_tools/02_lsp_server)
- [VSCode Extension](07_tools/03_vscode_extension)
- [Packaging](07_tools/04_packaging)
- [Spec Doc Markers](doc-markers-spec)

### Compiler & Backend
- [Typed Codegen Design](08_typed_codegen/01_design)

<!-- toc:end -->
