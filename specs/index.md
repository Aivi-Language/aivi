---
title: AIVI Language Specification
---

<p>
    <img src="/aivi-logo.png" alt="AIVI Logo" width="256">
</p>

# AIVI language guide

AIVI is a statically typed, purely functional language for software that has to handle real data carefully: APIs, data pipelines, automation, services, and user interfaces.

If you can already program but terms like *algebraic data type* (a custom type with named cases), *effect tracking* (making I/O and failure visible in types), or *domain-driven design* (shaping code around the problem space) are not everyday tools for you, start here. These docs explain what AIVI is for, how its pieces fit together, and where to look when you want a concrete answer.

## Start here

- **New to AIVI?** Read [Language at a Glance](language-overview) for a quick tour of the syntax and the most important ideas.
- **Want the bigger picture?** Read [Introduction](introduction) to understand the language model, what problems AIVI is designed to solve, and how it approaches effects, data, and boundaries.
- **Need a precise rule?** Jump into the reference sections below.

## A simple learning path

1. Read **Start Here** first.
2. Work through **Learn AIVI** in order: Basics → Data & Types → Effects & Workflows → Modules & External Data.
3. Jump to **Build Native Apps** when you are ready to build GTK or libadwaita interfaces.
4. Use **Standard Library**, **Testing & Tooling**, and **Internals** as lookup sections.

## What makes AIVI different?

- **Immutable data by default** — you build new values instead of mutating old ones.
- **Explicit effects** — code that talks to files, networks, or the system is marked in the type system.
- **No `null` or unchecked exceptions** — use `Option` and `Result` so missing data and failures stay visible.
- **Closed records and strong typing** — data shapes are explicit and checked.
- **Domain-oriented syntax** — units, operators, and literals can carry meaning from the problem domain rather than being treated as raw numbers or strings.

## How to use this specification

Use the table of contents as a map:

- **Start Here** gives you the quickest orientation.
- **Learn AIVI** is the main language-learning track.
- **Build Native Apps** keeps the desktop UI material together.
- **Standard Library** documents the built-in modules you use in everyday code.
- **Testing & Tooling** covers tests, the CLI, the language server, and editor support.
- **Internals** is useful when you want to understand how the compiler and backend are structured.

## Table of Contents

<!-- toc:start -->

### Start Here
- [Language at a Glance](language-overview)
- [Introduction](introduction)

### Learn AIVI
#### Basics
- [Bindings & Scope](syntax/bindings)
- [Functions & Pipes](syntax/functions)
#### Data & Types
- [Primitive Types](syntax/types/primitive_types)
- [Custom Data Types (ADTs)](syntax/types/algebraic_data_types)
- [Records](syntax/types/closed_records)
- [Extending & Reshaping Records](syntax/types/record_row_transforms)
- [Pattern Matching](syntax/pattern_matching)
- [Predicates](syntax/predicates)
- [Updating Records](syntax/patching)
- [Helpful Type Conversions](syntax/types/expected_type_coercions)
- [Opaque Types](syntax/types/opaque_types)
- [Domains & Units](syntax/domains)
#### Effects & Workflows
- [Effects](syntax/effects)
- [do Notation](syntax/do_notation)
- [Resources](syntax/resources)
- [Generators](syntax/generators)
- [Capabilities](syntax/capabilities)
- [Effect Handlers](syntax/effect_handlers)
##### State Machines
- [Overview](syntax/state_machines)
- [Machine Syntax](syntax/machines)
- [Machine Runtime](syntax/machines_runtime)
#### Modules & External Data
- [Modules](syntax/modules)
##### External Sources
- [Overview](syntax/external_sources)
- [Define Sources from Schemas](syntax/external_sources/schema_first)
- [Combine Sources](syntax/external_sources/composition)
- [File Sources](syntax/external_sources/file)
- [REST / HTTP Sources](syntax/external_sources/rest_http)
- [Environment Sources](syntax/external_sources/environment)
- [IMAP Email Sources](syntax/external_sources/imap_email)
- [Image Sources](syntax/external_sources/image)
- [Compile-Time Sources](syntax/external_sources/compile_time)
#### Advanced Features
- [Reusable Type Patterns (Classes & HKTs)](syntax/types/classes_and_hkts)
##### Decorators
- [Overview](syntax/decorators/)
- [@static](syntax/decorators/static)
- [@native](syntax/decorators/native)
- [@deprecated](syntax/decorators/deprecated)
- [@debug](syntax/decorators/debug)
- [@test](syntax/decorators/test)
- [@no_prelude](syntax/decorators/no_prelude)
- [Operators & Context](syntax/operators)
- [Grammar Reference](syntax/grammar)

### Build Native Apps
- [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps)
- [App Architecture](stdlib/ui/app_architecture)
- [GTK & libadwaita Runtime](stdlib/ui/gtk4)
#### Reactive UI
- [Reactive Signals](stdlib/ui/reactive_signals)
- [Reactive Dataflow](stdlib/ui/reactive_dataflow)
#### UI Building Blocks
- [Forms](stdlib/ui/forms)
- [Layout](stdlib/ui/layout)
- [Color](stdlib/ui/color)
- [HTML Sigil](stdlib/ui/html)
- [Virtual DOM](stdlib/ui/vdom)

### Standard Library
#### Core Building Blocks
- [Prelude](stdlib/core/prelude)
- [Option](stdlib/core/option)
- [Result](stdlib/core/result)
- [Logic](stdlib/core/logic)
- [Collections](stdlib/core/collections)
- [Generator](stdlib/core/generator)
- [Validation](stdlib/core/validation)
#### Text & Data
- [Text](stdlib/core/text)
- [Regex](stdlib/core/regex)
- [I18n](stdlib/core/i18n)
- [JSON](stdlib/data/json)
- [Bits](stdlib/data/bits)
#### Math, Units & Models
- [Units](stdlib/core/units)
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
#### Time & Scheduling
- [Instant](stdlib/chronos/instant)
- [Calendar](stdlib/chronos/calendar)
- [Duration](stdlib/chronos/duration)
- [TimeZone](stdlib/chronos/timezone)
- [Scheduler](stdlib/chronos/scheduler)
#### Files, System & Security
- [File](stdlib/system/file)
- [Console](stdlib/system/console)
- [Path](stdlib/system/path)
- [URL](stdlib/system/url)
- [System](stdlib/system/system)
- [Log](stdlib/system/log)
- [Concurrency](stdlib/system/concurrency)
- [Crypto](stdlib/system/crypto)
- [Secrets](stdlib/system/secrets)
#### Network, Services & Storage
- [HTTP & HTTPS](stdlib/network/http)
- [HTTP Server](stdlib/network/http_server)
- [REST](stdlib/network/rest)
- [Sockets](stdlib/network/sockets)
- [Streams](stdlib/network/streams)
- [Database](stdlib/system/database)
- [Email](stdlib/system/email)

### Testing & Tooling
#### Testing
- [Testing Module](stdlib/core/testing)
- [Test Decorator & Mocking](syntax/decorators/test)
#### Developer Tools
- [CLI](tools/cli)
- [Package Manager](tools/package_manager)
- [LSP Server](tools/lsp_server)
- [VSCode Extension](tools/vscode_extension)
- [Incremental Compilation](tools/incremental_compilation)

### Internals
- [Compiler & Backend](typed_codegen/design)
- [Minimality Proof](typed_codegen/minimality)
- [Spec Doc Markers](doc-markers-spec)

<!-- toc:end -->
