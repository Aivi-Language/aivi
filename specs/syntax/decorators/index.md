# Decorators

Decorators attach **compile-time metadata** to definitions.
They tell the compiler or tooling how to treat a binding, but they are not a substitute for normal types, values, or APIs.

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
They must not be used to smuggle domain semantics into hidden metadata.
For example, database mapping, HTTP schemas, validation rules, and integration configuration belong in typed values and ordinary types, not decorators.

## Core rules

- Unknown decorators are a compile error.
- `@native` is restricted to top-level definitions and requires an explicit type signature.
- Decorators apply to the binding or module that immediately follows them.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear directly above the definition they annotate.
If you are scanning a file, read them as instructions to the compiler before the binding itself.

## Available decorators

| Decorator | What it is for |
|:--------- |:--------------- |
| [`@static`](./static) | Evaluate deterministic source reads at compile time and embed the result |
| [`@native`](./native) | Bind an AIVI name to a runtime or Rust-native function |
| [`@deprecated`](./deprecated) | Warn callers and point them to a replacement |
| [`@debug`](./debug) | Emit structured trace events for debugging |
| [`@test`](./test) | Mark definitions or modules as test-only |
| [`@no_prelude`](./no_prelude) | Disable the implicit prelude import for a module |

## Mental model

A decorator should answer a tooling question such as:

- should this be evaluated now or at runtime?
- is this binding implemented outside AIVI?
- should callers see a warning?
- should the test runner collect this definition?

If the question is about your program's business meaning, prefer ordinary AIVI types and values instead.

## Desugaring

Some decorators expand to simpler internal forms:

| Surface | Desugared |
|:------- |:--------- |
| `@static x = file.read ...` | compile-time evaluation and value embedding |
| `@native "mod.fn"
f : A -> B` | auto-generates `f __arg0 = mod.fn __arg0` (type signature required) |
| `@native "crate::fn"
f : A -> B` | auto-generates a bridge function for AOT builds |
