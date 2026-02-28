# `@debug` — Structured Debug Tracing

<!-- quick-info: {"kind":"decorator","name":"@debug"} -->
`@debug` is a tooling pragma for compiler-emitted trace logs. It has no semantic effect unless you compile with `--debug-trace`.
<!-- /quick-info -->

## Syntax

```aivi
@debug
@debug()
@debug(pipes, args, return, time)
```

- `@debug` or `@debug()` defaults to function-level timing only.
- Parameters are order-insensitive; duplicates are ignored.
- Allowed parameters: `pipes`, `args`, `return`, `time`.

## Example

<<< ../../../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

## Output

When enabled, the compiler emits JSONL-friendly structured events:

- `fn.enter` / `fn.exit` per function call
- `pipe.in` / `pipe.out` per `|>` step (when `pipes` is enabled)

For multiple pipelines in a function body, step numbering restarts per pipeline chain and events include an additional `pipeId` field for disambiguation.
