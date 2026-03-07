---
title: AIVI Language Specification
---

<p>
    <img src="/aivi-logo.png" alt="AIVI Logo" width="256" height="auto">
</p>

# domain-driven & purely functional

A high-integrity purely functional language  with on top of the Rust eco system.

## Table of Contents

<!-- toc:start -->

### Getting Started
- [Language at a Glance](language-overview)
- [Introduction](introduction)

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
- [Machines](syntax/machines)
- [Machine Runtime](syntax/machines_runtime)
#### Modules & Interop
- [Modules](syntax/modules)
- [External Sources](syntax/external_sources)
- [File Sources](syntax/external_sources/file)
- [REST/HTTP Sources](syntax/external_sources/rest_http)
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
#### Notation
- [Operators & Sigils](syntax/operators)
- [Grammar](syntax/grammar)

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
#### UI
- [App Architecture](stdlib/ui/app_architecture)
- [GTK4](stdlib/ui/gtk4)
- [Layout](stdlib/ui/layout)
- [VDOM](stdlib/ui/vdom)
- [HTML Sigil](stdlib/ui/html)
- [Color](stdlib/ui/color)

### Testing
- [Testing Module](stdlib/core/testing)
- [@test & Mocks](syntax/decorators/test)

### Tooling
- [CLI](tools/cli)
- [LSP Server](tools/lsp_server)
- [VSCode Extension](tools/vscode_extension)
- [Package Manager](tools/package_manager)

### Internals
- [Compiler & Backend](typed_codegen/design)
- [Minimality Proof](typed_codegen/minimality)

### Appendix
- [Spec Doc Markers](doc-markers-spec)

<!-- toc:end -->
