# Decorators

<!-- quick-info: {"kind":"topic","name":"decorators"} -->
Decorators are compiler-visible instructions that attach to a module declaration or top-level definition.
They tell the compiler or tooling how to treat that item, but they are not a substitute for normal types, values, or APIs.
<!-- /quick-info -->

## What decorators are for

Use a decorator when you need compiler- or tooling-level behaviour such as:

- compile-time evaluation
- native bindings
- deprecation warnings
- debug tracing
- test discovery
- module-level control over implicit imports

## What decorators are not for

Decorators are intentionally narrow.
They must not be used to smuggle domain semantics into hidden compiler instructions.
For example, database mapping, HTTP schemas, validation rules, and integration configuration belong in typed values and ordinary types, not decorators.

## Core rules

- In v0.1, only the six decorators listed below are valid; unknown decorators are a compile error.
- Decorators apply to the module declaration or top-level definition that immediately follows them.
- `@native` is restricted to top-level definitions and requires an explicit type signature.
- `@debug` is only valid on function definitions.
- `@test` can decorate a definition or a whole module. Definition-level `@test` requires a description string, while module-level `@test` takes no argument.
- `@no_prelude` is module-only.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear directly above the module or definition they annotate.
The snippet shows the three surface shapes:

- a bare marker such as `@debug`
- a decorator with one argument such as `@test "parses empty input"`
- a decorator with structured configuration such as `@decorator_name { key: value }`

If you are scanning a file, read decorator lines as instructions to the compiler before you read the item itself.

## Available decorators

| Decorator | Applies to | What it is for |
|:--------- |:---------- |:--------------- |
| [`@static`](./static) | top-level definition | Evaluate deterministic source reads at compile time and embed the result |
| [`@native`](./native) | top-level definition | Bind an AIVI name to a runtime or Rust-native function |
| [`@deprecated`](./deprecated) | top-level definition | Warn callers and point them to a replacement |
| [`@debug`](./debug) | function definition | Emit structured trace events for debugging |
| [`@test`](./test) | definition or module | Mark tests for discovery, or mark a whole module as test-only |
| [`@no_prelude`](./no_prelude) | module | Disable the implicit prelude import for a module |

See each decorator page for exact argument rules and diagnostics.

## Mental model

A decorator should answer a tooling question such as:

- should this binding be evaluated now or at runtime?
- is this binding implemented outside AIVI?
- should callers see a warning?
- should the test runner collect this definition?
- should this module opt out of the implicit prelude?

If the question is about your program's business meaning, prefer ordinary AIVI types and values instead.

## Desugaring

Some decorators do more than carry metadata: the compiler may lower them to simpler internal forms or use them to change compilation behaviour.

| Surface | What the compiler does |
|:------- |:---------------------- |
| `@static x = file.read ...` | evaluates the source read during compilation and embeds the resulting value |
| `@native "mod.fn"\nf : A -> B` | lowers calls through the runtime-native target described by the dotted path (type signature required) |
| `@native "crate::fn"\nf : A -> B` | during `aivi build`, generates a Rust bridge for the crate-native target in AOT builds |
