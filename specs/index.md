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
- [Primitive Types](syntax/types/primitive_types)
- [Algebraic Data Types](syntax/types/algebraic_data_types)
- [Closed Records](syntax/types/closed_records)
- [Record Row Transforms](syntax/types/record_row_transforms)
- [Classes and HKTs](syntax/types/classes_and_hkts)
- [Expected-Type Coercions](syntax/types/expected_type_coercions)
- [Predicates](syntax/predicates)
#### Data & Flow
- [Patching Records](syntax/patching)
- [Domains, Units, and Deltas](syntax/domains)
- [Generators](syntax/generators)
- [Pattern Matching](syntax/pattern_matching)
#### Effects & Control
- [Effects](syntax/effects)
- [Machines](syntax/machines_runtime)
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
#### System
- [Crypto](stdlib/system/crypto)
- [GOA](stdlib/system/goa)
- [Secrets](stdlib/system/secrets)
- [System](stdlib/system/system)
- [Log](stdlib/system/log)
- [Concurrency](stdlib/system/concurrency)
#### IO
- [File](stdlib/system/file)
- [Console](stdlib/system/console)
- [Database](stdlib/system/database)
- [Path](stdlib/system/path)
- [URL](stdlib/system/url)
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

### Runtime
- [Concurrency](runtime/concurrency)
- [Package Manager & Packaging](runtime/package_manager)

### Tooling
- [CLI](tools/cli)
- [LSP Server](tools/lsp_server)
- [VSCode Extension](tools/vscode_extension)
- [Spec Doc Markers](doc-markers-spec)

### Compiler & Backend
- [Compiler & Backend Design](typed_codegen/design)
- [Minimality Proof](typed_codegen/minimality)

<!-- toc:end -->
