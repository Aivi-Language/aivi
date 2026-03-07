# `@debug` — Structured Debug Tracing

<!-- quick-info: {"kind":"decorator","name":"@debug"} -->
`@debug` is a tooling pragma for compiler-emitted trace logs. No semantic effect unless compiled with `--debug-trace`.
<!-- /quick-info -->

Use `@debug` when you want the compiler and runtime to emit structured trace events for a function.
It is useful for understanding call flow, timing, and pipeline behaviour without hand-writing logging statements into the function body.

## Syntax

```aivi
@debug
@debug()
@debug(pipes, args, return, time)
```

`@debug` and `@debug()` both default to function-level timing only.
Parameters are order-insensitive, duplicate flags are ignored, and the allowed flags are `pipes`, `args`, `return`, and `time`.

## What each flag enables

| Flag | Effect |
| --- | --- |
| `time` | Emit timing information for the call |
| `args` | Include function arguments in entry events |
| `return` | Include the returned value in exit events |
| `pipes` | Emit `pipe.in` and `pipe.out` events for `|>` steps |

## Example

<<< ../../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

## Output shape

When debug tracing is enabled, the runtime emits JSONL-friendly events such as:

- `fn.enter`
- `fn.exit`
- `pipe.in`
- `pipe.out`

That makes `@debug` a good fit for troubleshooting, performance investigation, and tool integration.

## Practical guidance

- `@debug` does not change program meaning; it only affects emitted trace information.
- Prefer it over ad-hoc print debugging when you want machine-readable output.
- Leave it off normal production code unless you are intentionally building with `--debug-trace`.
