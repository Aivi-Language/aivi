---
title: AIVI Language Specification
---

<p align="left">
    <img src="/aivi-logo.png" alt="AIVI Logo" width="256">
    <br/>
    <b>A research project by Andreas Herd</b>
</p>


# AIVI language guide


AIVI is a statically typed, purely functional language for software that needs clear data models, predictable behavior, and explicit handling of side effects. It is built for APIs, data pipelines, automation, services, and user interfaces—places where you want the compiler to help catch missing cases, invalid data shapes, and boundary mistakes before the program runs.

If you already know how to program but terms like *algebraic data type* (a custom type with named cases), *effect tracking* (making I/O and failure visible in types), or *domain-driven design* (shaping code around the problem space) are new, start here. These docs explain what AIVI is for, how the pieces fit together, and where to look when you want either a quick tour or a precise rule.

## Start here

- **New to AIVI?** Read [Language at a Glance](language-overview) for a quick tour of the syntax and the most important ideas.
- **Want the bigger picture?** Read [Introduction](introduction) for the plain-language explanation of what AIVI is for and how it approaches data, effects, and boundaries.
- **Need a precise rule?** Use the table of contents below to jump straight to the reference page you need.

## A simple learning path

If you want a full guided tour instead of jumping around, follow this order:

1. Read [Language at a Glance](language-overview) and [Introduction](introduction).
2. Work through **Learn AIVI** in order: Basics → Data & Types → Effects & Workflows → Modules & External Data.
3. Jump to **Build Native Apps** when you are ready to build GTK or libadwaita interfaces.
4. Keep **Standard Library**, **Testing & Tooling**, and **Internals** open as lookup sections once you start building.

## What makes AIVI different?

- **Immutable data by default** — you build new values instead of mutating old ones.
- **Explicit effects** — code that talks to files, networks, or the system is marked in the type system.
- **No `null` or unchecked exceptions** — use `Option` and `Result` so missing data and failures stay visible.
- **Closed records and strong typing** — data shapes are explicit and checked.
- **Domain-oriented syntax** — units, operators, and literals can carry meaning from the problem domain rather than being treated as raw numbers or strings.

## How to use this specification

Use the table of contents below as a map once you know the kind of answer you need:

- **Start Here** gives you a first-reading path and the quickest orientation.
- **Learn AIVI** is the main track for learning the language itself.
- **Build Native Apps** keeps the GTK and libadwaita UI material together.
- **Standard Library** documents the built-in modules you use in everyday code.
- **Testing & Tooling** covers tests, the CLI, the language server, and editor support.
- **Internals** is where to go when you want compiler and backend design details.

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
- [Type System Overview](syntax/types)
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
- [Mental Model](stdlib/ui/gtk4/mental_model)
- [First Window](stdlib/ui/gtk4/first_window)
- [State & Patches](stdlib/ui/gtk4/state_patches)
- [GTK Sugar](stdlib/ui/gtk4/gtk_sugar)
- [Callbacks](stdlib/ui/gtk4/callbacks)
- [Events](stdlib/ui/gtk4/events)
- [Structure](stdlib/ui/gtk4/structure)
- [Lifecycle](stdlib/ui/gtk4/lifecycle)
- [MCP Debugging](stdlib/ui/gtk4/mcp_debugging)
#### Supporting Docs
- [Signals](stdlib/ui/reactive_signals)
- [Reactive Dataflow](stdlib/ui/reactive_dataflow)
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
- [REPL](tools/repl)
- [MCP Server](tools/mcp)
- [Package Manager](tools/package_manager)
- [LSP Server](tools/lsp_server)
- [VSCode Extension](tools/vscode_extension)
- [Incremental Compilation](tools/incremental_compilation)

### Internals
- [Compiler & Backend](typed_codegen/design)
- [Minimality Proof](typed_codegen/minimality)
- [Spec Doc Markers](doc-markers-spec)

<!-- toc:end -->
