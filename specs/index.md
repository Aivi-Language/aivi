---
title: AIVI Language Specification
---

<p style="
    background-color: #334;
    border-radius: 20px;
    width: fit-content;
    padding: 1rem;
    margin: 0 auto 3rem auto;
">
    <img src="../assets/aivi-128.png" alt="AIVI Logo" width="128" height="128">
</p>

# AIVI Language Specification

A high-integrity functional language with a Rust-first compilation pipeline.

## Table of Contents

### Core Specification

- [Introduction](01_introduction)

### Syntax

- [Bindings and Scope](02_syntax/01_bindings)
- [Functions and Pipes](02_syntax/02_functions)
- [The Type System](02_syntax/03_types)
- [Predicates](02_syntax/04_predicates)
- [Patching Records](02_syntax/05_patching)
- [Domains, Units, and Deltas](02_syntax/06_domains)
- [Generators](02_syntax/07_generators)
- [Pattern Matching](02_syntax/08_pattern_matching)
- [Effects](02_syntax/09_effects)
- [Modules](02_syntax/10_modules)
- [External Sources](02_syntax/12_external_sources)
- [Sigils](02_syntax/13_sigils)
- [Decorators](02_syntax/14_decorators)
- [Resources](02_syntax/15_resources)
- [Generic `do` Notation](02_syntax/16_do_notation)
- [Operators and Context](02_syntax/11_operators)
- [Concrete Syntax (EBNF draft)](02_syntax/00_grammar)

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

#### Math & Science
- [Math](05_stdlib/01_math/01_math)
- [Vector](05_stdlib/01_math/05_vector)
- [Matrix](05_stdlib/01_math/09_matrix)
- [Number](05_stdlib/01_math/10_number)
- [Probability](05_stdlib/01_math/13_probability)
- [FFT & Signal](05_stdlib/01_math/14_signal)
- [Geometry](05_stdlib/01_math/15_geometry)
- [Graph](05_stdlib/01_math/17_graph)
- [Linear Algebra](05_stdlib/01_math/18_linear_algebra)
- [Tree](05_stdlib/01_math/19_tree)

#### Chronos (Time)
- [Instant](05_stdlib/02_chronos/01_instant) *(planned   not yet implemented)*
- [Calendar](05_stdlib/02_chronos/02_calendar)
- [Duration](05_stdlib/02_chronos/03_duration)
- [TimeZone](05_stdlib/02_chronos/04_timezone) *(planned   not yet implemented)*

#### Network
- [HTTP Utils](05_stdlib/03_network/01_http)
- [HTTPS](05_stdlib/03_network/02_https)
- [HTTP Server](05_stdlib/03_network/03_http_server)
- [Sockets](05_stdlib/03_network/04_sockets)
- [Streams](05_stdlib/03_network/05_streams)

#### System & IO
- [File](05_stdlib/03_system/20_file)
- [Console](05_stdlib/03_system/21_console)
- [Crypto](05_stdlib/03_system/22_crypto)
- [Database](05_stdlib/03_system/23_database)
- [Path](05_stdlib/03_system/24_path)
- [URL](05_stdlib/03_system/25_url)
- [System](05_stdlib/03_system/26_system)
- [Log](05_stdlib/03_system/27_log) *(planned   not yet implemented)*
- [Concurrency](05_stdlib/03_system/30_concurrency)

#### UI
- [Layout](05_stdlib/04_ui/01_layout)
- [Virtual DOM](05_stdlib/04_ui/02_vdom) *(planned   not yet implemented)*
- [HTML Sigil](05_stdlib/04_ui/03_html) *(planned   not yet implemented)*
- [Color](05_stdlib/04_ui/04_color)
- [ServerHtml (server-driven HTML)](05_stdlib/04_ui/05_server_html)

### Execution & Concurrency

- [Concurrency](06_runtime/01_concurrency)
- [Memory Management](06_runtime/02_memory_management)
- [Package Manager (Cargo-backed)](06_runtime/03_package_manager)

### Tools & Ecosystem

- [CLI](07_tools/01_cli)
- [LSP Server](07_tools/02_lsp_server)
- [VSCode Extension](07_tools/03_vscode_extension)
- [Packaging](07_tools/04_packaging)

### Kernel (Core Calculus)

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

### Desugaring (Syntax → Kernel)

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
