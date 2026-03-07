---
title: AIVI Language Specification
---

<p>
    <img src="/aivi-logo.png" alt="AIVI Logo" width="256">
</p>

# AIVI language guide

AIVI is a statically typed, purely functional language for software that has to handle real data carefully: APIs, data pipelines, automation, services, and user interfaces.

If you can already program but terms like *algebraic data type*, *effect tracking*, or *domain-driven design* are not everyday tools for you, start here. These docs explain what AIVI is for, how its pieces fit together, and where to look when you want a concrete answer.

## Start here

- **New to AIVI?** Read [Language at a Glance](language-overview) for a quick tour of the syntax and the most important ideas.
- **Want the bigger picture?** Read [Introduction](introduction) to understand the language model, what problems AIVI is designed to solve, and how it approaches effects, data, and boundaries.
- **Need a precise rule?** Jump into the reference sections below.

## What makes AIVI different?

- **Immutable data by default** — you build new values instead of mutating old ones.
- **Explicit effects** — code that talks to files, networks, or the system is marked in the type system.
- **No `null` or unchecked exceptions** — use `Option` and `Result` so missing data and failures stay visible.
- **Closed records and strong typing** — data shapes are explicit and checked.
- **Domain-oriented syntax** — units, operators, and literals can carry meaning from the problem domain rather than being treated as raw numbers or strings.

## How to use this specification

Use the table of contents as a map:

- **Language Reference** explains the core syntax and semantics.
- **Standard Library** documents the built-in modules you use in everyday code.
- **Tooling** covers the CLI, language server, and editor support.
- **Internals** is useful when you want to understand how the compiler and backend are structured.

## Table of Contents

<!-- toc:start -->

### Getting Started
- [Language at a Glance](language-overview)
- [Introduction](introduction)

### Native Apps
- [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps)
- [App Architecture](stdlib/ui/app_architecture)
#### Signals & Dataflow
- [Reactive Signals](stdlib/ui/reactive_signals)
- [Reactive Dataflow](stdlib/ui/reactive_dataflow)
- [GTK & libadwaita Runtime](stdlib/ui/gtk4)
- [Forms](stdlib/ui/forms)
- [Layout](stdlib/ui/layout)
- [Color](stdlib/ui/color)
- [HTML Sigil](stdlib/ui/html)
- [Virtual DOM](stdlib/ui/vdom)

### Language Reference
#### Foundations
- [Bindings & Scope](syntax/bindings)
- [Functions & Pipes](syntax/functions)
#### Types
- [Primitive Types](syntax/types/primitive_types)
- [Algebraic Data Types](syntax/types/algebraic_data_types)
- [Closed Records](syntax/types/closed_records)
- [Record Row Transforms](syntax/types/record_row_transforms)
- [Higher Kinded Types](syntax/types/classes_and_hkts)
- [Expected-Type Coercions](syntax/types/expected_type_coercions)
- [Opaque Types](syntax/types/opaque_types)
#### Data & Patterns
- [Pattern Matching](syntax/pattern_matching)
- [Predicates](syntax/predicates)
- [Patching Records](syntax/patching)
- [Domains & Units](syntax/domains)
- [Generators](syntax/generators)
#### Effects & Control
- [Effects](syntax/effects)
- [Capabilities](syntax/capabilities)
- [Effect Handlers](syntax/effect_handlers)
- [do Notation](syntax/do_notation)
- [Resources](syntax/resources)
#### Modules & Interop
- [Modules](syntax/modules)
- [External Sources](syntax/external_sources)
- [Schema-First Source Definitions](syntax/external_sources/schema_first)
- [Source Composition](syntax/external_sources/composition)
- [File Sources](syntax/external_sources/file)
- [REST / HTTP Sources](syntax/external_sources/rest_http)
- [Environment Sources](syntax/external_sources/environment)
- [IMAP Email Sources](syntax/external_sources/imap_email)
- [Image Sources](syntax/external_sources/image)
- [Compile-Time Sources](syntax/external_sources/compile_time)
#### Decorators
- [Overview](syntax/decorators/)
- [@static](syntax/decorators/static)
- [@native](syntax/decorators/native)
- [@deprecated](syntax/decorators/deprecated)
- [@debug](syntax/decorators/debug)
- [@test](syntax/decorators/test)
- [@no_prelude](syntax/decorators/no_prelude)
#### Syntax Details
- [Operators & Context](syntax/operators)
- [Grammar Reference](syntax/grammar)

### State Machines
- [Overview](syntax/state_machines)
- [Machine Syntax](syntax/machines)
- [Machine Runtime](syntax/machines_runtime)

### Standard Library
#### Core
- [Prelude](stdlib/core/prelude)
- [Option](stdlib/core/option)
- [Result](stdlib/core/result)
- [Text](stdlib/core/text)
- [Logic](stdlib/core/logic)
- [Collections](stdlib/core/collections)
- [Generator](stdlib/core/generator)
- [Regex](stdlib/core/regex)
- [Validation](stdlib/core/validation)
- [I18n](stdlib/core/i18n)
- [Units](stdlib/core/units)
#### Data
- [Bits](stdlib/data/bits)
- [JSON](stdlib/data/json)
#### Math & Science
- [Math](stdlib/math/math)
- [Numbers](stdlib/math/number)
- [Vector](stdlib/math/vector)
- [Matrix](stdlib/math/matrix)
- [Linear Algebra](stdlib/math/linear_algebra)
- [Geometry](stdlib/math/geometry)
- [Probability](stdlib/math/probability)
- [Signal Processing](stdlib/math/signal)
- [Graph](stdlib/math/graph)
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
- [Path](stdlib/system/path)
- [URL](stdlib/system/url)
- [System](stdlib/system/system)
- [Log](stdlib/system/log)
- [Concurrency](stdlib/system/concurrency)
#### Security
- [Crypto](stdlib/system/crypto)
- [Secrets](stdlib/system/secrets)
#### Database
- [Database](stdlib/system/database)
#### Networking
- [HTTP & HTTPS](stdlib/network/http)
- [HTTP Server](stdlib/network/http_server)
- [REST](stdlib/network/rest)
- [Sockets](stdlib/network/sockets)
- [Streams](stdlib/network/streams)
#### Email
- [Email](stdlib/system/email)

### Testing
- [Testing Module](stdlib/core/testing)
- [Test Decorator & Mocking](syntax/decorators/test)

### Tooling
- [CLI](tools/cli)
- [LSP Server](tools/lsp_server)
- [Incremental Compilation](tools/incremental_compilation)
- [VSCode Extension](tools/vscode_extension)
- [Package Manager](tools/package_manager)

### Internals
- [Compiler & Backend](typed_codegen/design)
- [Minimality Proof](typed_codegen/minimality)

### Appendix
- [Spec Doc Markers](doc-markers-spec)

<!-- toc:end -->
