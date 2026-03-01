# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Unknown decorators are a compile error.
- `@native` is restricted to top-level definitions and requires an explicit type signature for type-safe bindings.

## Syntax

<<< ../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear before the binding they annotate.

## Available Decorators

| Decorator     | Purpose                                              | Page                                  |
|:------------- |:---------------------------------------------------- |:------------------------------------- |
| `@static`     | Compile-time evaluation and embedding                | [Static](./decorators/static)         |
| `@native`     | Bind to runtime/native functions                     | [Native](./decorators/native)         |
| `@deprecated` | Emit usage warnings with migration hints             | [Deprecated](./decorators/deprecated) |
| `@debug`      | Structured debug tracing                             | [Debug](./decorators/debug)           |
| `@test`       | Mark definitions or modules as test-only             | [Test](./decorators/test)             |
| `@no_prelude` | Skip implicit prelude import                         | [No Prelude](./decorators/no_prelude) |

## Desugaring

| Surface                        | Desugared                                                   |
|:------------------------------ |:----------------------------------------------------------- |
| `@static x = file.read ...`    | Compile-time evaluation                                     |
| `@native "mod.fn" f x y = ...` | Rewritten to `f x y = mod.fn x y` (type signature required) |

## Related

- [Mock Expressions](./decorators/mock) — scoped binding substitution for testing
