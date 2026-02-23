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
- [Introduction](introduction)
- [Missing Features (v0.1)](missing_features_v0.1)

### Syntax
#### Core Forms
- [Bindings and Scope](syntax/bindings)
- [Functions and Pipes](syntax/functions)
- [The Type System](syntax/types)
- [Predicates](syntax/predicates)
#### Data & Flow
- [Patching Records](syntax/patching)
- [Domains, Units, and Deltas](syntax/domains)
- [Generators](syntax/generators)
- [Pattern Matching](syntax/pattern_matching)
#### Effects & Control
- [Effects](syntax/effects)
- [Resources](syntax/resources)
- [Generic `do` Notation](syntax/do_notation)
#### Modules & Interop
- [Modules](syntax/modules)
- [External Sources](syntax/external_sources)
##### Source Integrations
- [File Sources](syntax/external_sources/file)
- [REST/HTTP Sources](syntax/external_sources/rest_http)
- [Environment Sources](syntax/external_sources/environment)
- [IMAP Email Sources](syntax/external_sources/imap_email)
- [Image Sources](syntax/external_sources/image)
- [Compile-Time Sources](syntax/external_sources/compile_time)
#### Notation & Grammar
- [Operators and Context](syntax/operators)
- [Sigils](syntax/sigils)
- [Decorators](syntax/decorators)
- [Comments](syntax/comments)
- [Concrete Syntax](syntax/grammar)

### Standard Library
#### Core & Utils
- [Prelude](stdlib/core/prelude)
- [Text](stdlib/core/text)
- [Logic](stdlib/core/logic)
- [Units](stdlib/core/units)
- [Regex](stdlib/core/regex)
- [Testing](stdlib/core/testing)
- [Collections](stdlib/core/collections)
- [I18n](stdlib/core/i18n)
- [Generator](stdlib/core/generator)
- [Validation](stdlib/core/validation)
#### Math & Science
- [Math](stdlib/math/math)
- [Vector](stdlib/math/vector)
- [Matrix](stdlib/math/matrix)
- [Numbers](stdlib/math/number)
- [Probability](stdlib/math/probability)
- [FFT & Signal](stdlib/math/signal)
- [Geometry](stdlib/math/geometry)
- [Graph](stdlib/math/graph)
- [Linear Algebra](stdlib/math/linear_algebra)
- [Tree](stdlib/math/tree)
#### Time
- [Instant](stdlib/chronos/instant)
- [Calendar](stdlib/chronos/calendar)
- [Duration](stdlib/chronos/duration)
- [TimeZone](stdlib/chronos/timezone)
- [Scheduler](stdlib/chronos/scheduler)
#### System & IO
- [File](stdlib/system/file)
- [Console](stdlib/system/console)
- [Crypto](stdlib/system/crypto)
- [Database](stdlib/system/database)
- [GOA](stdlib/system/goa)
- [Secrets](stdlib/system/secrets)
- [Path](stdlib/system/path)
- [URL](stdlib/system/url)
- [System](stdlib/system/system)
- [Log](stdlib/system/log)
- [Concurrency](stdlib/system/concurrency)
#### Network
- [HTTP](stdlib/network/http)
- [HTTPS](stdlib/network/https)
- [HTTP Server](stdlib/network/http_server)
- [Sockets](stdlib/network/sockets)
- [Streams](stdlib/network/streams)
#### UI
- [GTK4](stdlib/ui/gtk4)
- [Layout](stdlib/ui/layout)
- [VDOM](stdlib/ui/vdom)
- [HTML Sigil](stdlib/ui/html)
- [Color](stdlib/ui/color)
- [ServerHtml](stdlib/ui/server_html)

### Semantics
#### Kernel (Core Calculus)
- [Core Terms](kernel/core_terms)
- [Types](kernel/types)
- [Records](kernel/records)
- [Patterns](kernel/patterns)
- [Predicates](kernel/predicates)
- [Traversals](kernel/traversals)
- [Generators](kernel/generators)
- [Effects](kernel/effects)
- [Classes](kernel/classes)
- [Domains](kernel/domains)
- [Patching](kernel/patching)
- [Minimality Proof](kernel/minimality)
#### Desugaring (Surface -> Kernel)
- [Bindings](desugaring/bindings)
- [Functions](desugaring/functions)
- [Records](desugaring/records)
- [Patterns](desugaring/patterns)
- [Predicates](desugaring/predicates)
- [Generators](desugaring/generators)
- [Effects](desugaring/effects)
- [Classes](desugaring/classes)
- [Domains and Operators](desugaring/domains)
- [Patching](desugaring/patching)
- [Resources](desugaring/resources)

### Runtime
- [Concurrency](runtime/concurrency)
- [Memory Management](runtime/memory_management)
- [Package Manager (Cargo-backed)](runtime/package_manager)

### Tooling
- [CLI](tools/cli)
- [LSP Server](tools/lsp_server)
- [VSCode Extension](tools/vscode_extension)
- [Packaging](tools/packaging)
- [Spec Doc Markers](doc-markers-spec)

### Compiler & Backend
- [Typed Codegen Design](typed_codegen/design)

<!-- toc:end -->
