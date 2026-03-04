# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Unknown decorators are a compile error.
- `@native` is restricted to top-level definitions and requires an explicit type signature for type-safe bindings.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear before the binding they annotate.

## Available Decorators

| Decorator     | Purpose                                              |
|:------------- |:---------------------------------------------------- |
| [`@static`](./static)     | Compile-time evaluation and embedding                |
| [`@native`](./native)     | Bind to runtime/native functions                     |
| [`@deprecated`](./deprecated) | Emit usage warnings with migration hints         |
| [`@debug`](./debug)      | Structured debug tracing                             |
| [`@test`](./test)       | Mark definitions or modules as test-only             |
| [`@no_prelude`](./no_prelude) | Skip implicit prelude import                   |

## Desugaring

| Surface                                    | Desugared                                                                   |
|:------------------------------------------ |:--------------------------------------------------------------------------- |
| `@static x = file.read ...`               | Compile-time evaluation                                                     |
| `@native "mod.fn"\nf : A -> B`            | Auto-generates `f __arg0 = mod.fn __arg0` (type signature required)        |
| `@native "crate::fn"\nf : A -> B`         | Auto-generates bridge fn; AOT-only                                          |
