# Decorators

Decorators provide **compile-time metadata** attached to definitions.

## Policy (Constraints)

Decorators are intentionally narrow:

- Decorators MUST NOT be used to model domain semantics (e.g. database schemas/ORM, SQL, HTTP, validation rules).
- Integration behavior belongs in **typed values** (e.g. `Source` configurations) and **types** (decoders), not hidden in decorators.
- Only the standard decorators listed here are allowed in v0.1. Unknown decorators are a compile error.
- User-defined decorators are not supported in v0.1.
- `@native` is restricted to top-level definitions and requires an explicit type signature for type-safe bindings.

## 14.1 Syntax

<<< ../snippets/from_md/syntax/decorators/syntax.aivi{aivi}

Decorators appear before the binding they annotate.


## 14.2 Standard Decorators

### Compile-Time

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@static` | `@static x = file.read "..."` | Embed at compile time |
| `@native` | `@native "gtk4.appRun"` | Bind definition to a runtime/native function path |
| `@inline` | `@inline f = ...` | Always inline function |
| `@deprecated` | `@deprecated msg` | Emit warning on use |
| `@debug` | `@debug()` / `@debug(pipes, args, return, time)` | Emit structured debug trace events when compiled with `--debug-trace` |

### Tooling (MCP)

MCP decorators are not supported in v0.1.

### Testing

| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@test` | `@test "adds two numbers" add_is_commutative = ...` | Mark a definition as a test case (description is mandatory) |

### Pragmas (Module-level)
| Decorator | Usage | Meaning |
| :--- | :--- | :--- |
| `@no_prelude` | `@no_prelude module M` | Skip implicit prelude import |
| `@test` | `@test module M` | Mark module as test-only (excluded from production builds) |
## 14.3 Decorator Desugaring

Decorators desugar to compile-time metadata:

| Surface | Desugared |
| :--- | :--- |
| `@static x = file.read ...` | Compile-time evaluation |
| `@native "mod.fn" f x y = ...` | Rewritten to `f x y = mod.fn x y` (type signature required) |
| (none) | (none) |


## 14.4 Usage Examples

### Compile-Time Embedding

<<< ../snippets/from_md/syntax/decorators/compile_time_embedding.aivi{aivi}

### Debug Tracing

`@debug` is a tooling pragma for compiler-emitted trace logs. It has no semantic effect unless you compile with `--debug-trace`.

- `@debug()` (or `@debug`) defaults to function-level timing only.
- Parameters are order-insensitive; duplicates are ignored.
- Allowed parameters: `pipes`, `args`, `return`, `time`.

When enabled, the compiler emits JSONL-friendly structured events:

- `fn.enter` / `fn.exit` per function call
- `pipe.in` / `pipe.out` per `|>` step (when `pipes` is enabled)

For multiple pipelines in a function body, step numbering restarts per pipeline chain and events include an additional `pipeId` field for disambiguation.
