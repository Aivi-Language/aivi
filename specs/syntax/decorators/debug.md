# `@debug` — Structured Debug Tracing

<!-- quick-info: {"kind":"decorator","name":"@debug"} -->
`@debug` is a tooling pragma for compiler-emitted trace logs. No semantic effect unless compiled with `--debug-trace`.
<!-- /quick-info -->

## Syntax

```aivi
@debug
@debug()
@debug(pipes, args, return, time)
```

`@debug` / `@debug()` defaults to function-level timing only. Parameters are order-insensitive; duplicates ignored. Allowed: `pipes`, `args`, `return`, `time`.

## Example

<<< ../../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

When enabled, emits JSONL-friendly structured events: `fn.enter` / `fn.exit` per call, `pipe.in` / `pipe.out` per `|>` step (when `pipes` enabled).
