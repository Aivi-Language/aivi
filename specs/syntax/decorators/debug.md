# `@debug` — Debug Instrumentation

<!-- quick-info: {"kind":"decorator","name":"@debug"} -->
`@debug` marks a function definition for debug instrumentation. It does not change program meaning, and it only has an effect when debug tracing is enabled.
<!-- /quick-info -->

Use `@debug` when you want tooling to keep extra tracing metadata for a function instead of hand-writing temporary logging into the body.
It is useful for troubleshooting call flow, timing, and pipeline-heavy code during focused investigation.

Current implementation status: the parser and HIR lowering recognize the decorator forms below, but the CLI currently rejects `@debug` on ordinary `name = x => ...` lambda definitions with error `E2010`.
This page therefore documents the currently implemented decorator shapes and lowering behaviour, while also calling out the validator gap that still blocks normal CLI use.

## Syntax

<<< ../../snippets/from_md/syntax/decorators/debug/block_01.aivi{aivi}


`@debug` and `@debug()` both default to function-level timing only.
When you supply one or more flags, only the named flags are enabled, so `@debug(args)` does **not** imply `time`.
Flag order does not matter, duplicate flags are ignored, and the allowed flags are `pipes`, `args`, `return`, and `time`.

`@debug` is only intended for function definitions.
Applying it to a non-function binding is a compile error (`E2010`).

In the current implementation, the same `E2010` diagnostic also fires on ordinary v0.1 top-level lambda definitions such as:

<<< ../../snippets/from_md/syntax/decorators/debug/block_02.aivi{aivi}


That validator behaviour is stricter than the lowering code, which still knows how to attach debug metadata to the lambda body when debug tracing is forced on internally.

## What each flag enables

| Flag | Effect |
| --- | --- |
| `time` | Keep timing information for the function call |
| `args` | Keep function argument information |
| `return` | Keep returned-value information |
| `pipes` | Keep per-step metadata for `|>` pipeline stages |

In practice:

- keep the default `time` mode when you only care about duration,
- add `args` when you need to inspect the inputs that caused a bad path,
- add `return` when you need to inspect output values,
- add `pipes` when a long pipeline needs step-by-step tracing.

## Example

<<< ../../snippets/from_md/syntax/decorators/debug_example.aivi{aivi}

The snippet shows the two decorator shapes that the compiler currently recognizes for instrumentation metadata:

- a minimal timing-only annotation, and
- a verbose annotation for a pipeline where arguments, return values, and pipe steps all matter.

In today's CLI flow, these examples still hit `E2010` because the resolver checks only legacy parameter slots, not the RHS lambda form shown elsewhere in the language docs.

## When it takes effect

- Use `@debug` together with CLI commands that enable debug tracing, such as the `--debug-trace` flag documented in [CLI](/tools/cli).
- In the current v0.1 implementation, enabling debug tracing preserves extra debug metadata during lowering; without debug tracing, the lowering step erases the decorator before later compilation stages.
- With `pipes`, pipeline steps carry stable step numbers and labels derived from the source text, which helps tooling distinguish steps like `g 1` and `h`.

## Diagnostics

- `E2010`: `@debug` is not on a function binding. In the current implementation this also fires for ordinary `name = x => ...` definitions.
- `E2011`: the decorator argument is not empty and not a comma-separated list of identifiers.
- `E2012`: one of the identifiers is not one of `pipes`, `args`, `return`, or `time`.

## Practical guidance

- `@debug` does not change program meaning; it only affects debug instrumentation.
- Prefer it over ad-hoc print debugging when you want tracing to stay outside the function body.
- Leave it off normal production code unless you are intentionally investigating a problem.
- See [Decorators overview](./index) for the full list of supported decorators.
