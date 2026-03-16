# Runtime Diagnostics

<!-- quick-info: {"kind":"topic","name":"runtime diagnostics","extractSignature":false} -->
When an AIVI program fails **after** it has compiled successfully, the CLI and REPL should report the failure as a **runtime diagnostic**, not as an unstructured error string. The goal is to answer the questions application authors actually have: what failed, where it failed, how execution reached that point, and what to try next.
<!-- /quick-info -->

If you mainly use AIVI as an application author, this page defines the user-facing contract. If you are contributing to AIVI itself, treat this page as the runtime equivalent of the compile-time diagnostics behavior documented elsewhere in the tooling specs.

## Start here

In the common case, a runtime diagnostic should include:

- a short one-line summary
- a stable runtime diagnostic code such as `RT1203`
- the best available source location
- a source snippet with a caret when source text is available
- notes about the failing value or call
- a stack trace or call chain
- at least one actionable help message when AIVI can offer one confidently

The ideal experience should feel closer to the best parts of Rust, Elm, or modern Python tracebacks than to a plain exception string.

## What counts as a runtime diagnostic

Runtime diagnostics cover failures that happen after parsing and type-checking succeeded, such as:

- non-exhaustive matches that only become evident at runtime
- bad values flowing into builtins or native/runtime helpers
- invalid arguments discovered while executing an effect
- parse or conversion failures triggered by dynamic input
- index errors, division by zero, arithmetic overflow, and similar runtime faults
- errors raised by effectful subsystems such as files, networking, database access, GTK callbacks, or reactive refresh work

Compile-time diagnostics such as syntax or type errors remain separate and continue to use the normal compiler diagnostic pipeline.

## Rendering contract

A runtime diagnostic should be rendered in this order when the data is available:

1. header
2. primary source location and source frame
3. notes and hints
4. stack trace / call chain
5. any final plain-text fallback details that do not fit elsewhere

### Header

The header should include:

- severity label, usually `error`
- runtime diagnostic code
- short summary message

For example:

```text
error[RT1203]: `text.join` expected a list of `Text`
```

The summary should prefer user-language over implementation-language. For example, “expected a list of `Text`” is better than “TypeError in builtin”.

### Source frame

When AIVI can identify a source location in user code, it should render:

- `path:line:column`
- the relevant source line or lines
- a caret or highlight for the primary span
- optional secondary labels when a second span is useful

If only a point location is known, AIVI may render a one-column caret until richer spans are available.

If the failure originated in user code but was detected by a builtin or helper, the primary frame should still prefer the best user-facing call site over an internal runtime helper location.

### Notes and help

Runtime diagnostics may include:

- **notes** — factual extra context such as a value preview, an argument index, or the element index inside a list
- **help** — suggestions that are reasonably actionable, such as “add a wildcard arm” or “convert items to text before joining”

Help text should be specific when possible. Generic advice like “check your code” is not useful enough.

### Stack trace / call chain

When a call chain is available, AIVI should render frames from **innermost to outermost**:

```text
stack:
  0: app.main.renderNames at src/main.aivi:42:15
  1: app.main.main        at src/main.aivi:58:3
```

Each frame should include the best available combination of:

- qualified function or binding name
- source location
- a frame kind when that adds clarity, such as builtin, effect callback, reactive refresh, or native boundary

If a frame has no precise source span, AIVI should still include the frame name rather than dropping the frame entirely.

## Embedded stdlib and generated code

Not every failure originates in a user-authored file on disk.

### Embedded stdlib

When a frame points into embedded stdlib code such as `<embedded:aivi.text>`, AIVI should:

- include the embedded frame in the stack trace
- prefer a user-code call site as the primary source frame when one exists
- show the embedded location textually when source text is unavailable

If the embedded source text is available, AIVI may render it as a normal source frame. If not, it should degrade gracefully instead of printing a broken source block.

### Generated or synthetic code

For generated or synthetic runtime frames, AIVI should still expose:

- the frame name
- the best known synthetic location or label
- any user-facing parent frame that led into it

Generated internals should not replace a better user-facing frame when both are known.

## Value previews and truncation

Runtime diagnostics often need to show “what value failed” without dumping an entire program state.

Value previews should therefore be:

- short
- deterministic
- safe to print in terminals and logs

Guidelines:

- short texts may be shown inline
- long texts should be truncated with an obvious marker such as `…`
- large lists, records, maps, or nested structures should show only a small leading preview
- binary or otherwise unreadable payloads should prefer a typed summary over raw bytes
- stack traces should not duplicate a full value dump in every frame

When a list-processing builtin can identify the failing element index, AIVI should prefer “element 2 had type `Int`” over printing the entire list.

## Color policy

Runtime diagnostics should support the same three color modes everywhere AIVI presents them:

- `auto` — use color in an interactive terminal, disable it in logs and redirected output
- `always` — force ANSI colors
- `never` — disable ANSI colors

The default for CLI commands should be `auto`.

When colors are disabled, the diagnostic must remain readable and fully informative. Color is an enhancement, not a dependency.

## CLI behavior

Commands that surface runtime diagnostics, such as `aivi run` and `aivi test`, should use this contract directly.

When stderr is not attached to a terminal:

- ANSI escapes should be omitted by default
- the structure of the diagnostic should remain the same
- stack traces, hints, and value previews should still be included

When a command supports explicit color flags, those flags override automatic detection.

## REPL behavior

The REPL uses the same runtime-diagnostic content model as the CLI, but presentation differs by mode:

- **plain mode** should render the runtime diagnostic as ordinary text using the selected color mode
- **TUI mode** may restyle the same information using TUI widgets instead of raw ANSI, but it should preserve the same information content

The REPL should not collapse runtime failures down to a one-line string just because the user is in an interactive session.

## Stability expectations

The exact line wrapping, spacing, and ANSI escape sequences are not part of the public contract.

The stable parts are:

- the presence of the summary
- the runtime code
- the source location when known
- the notes and help semantics
- the stack frame ordering
- the auto / always / never color behavior

Tests should therefore prefer semantic assertions for behavior and targeted snapshots for representative pretty-print output.

## Example

```text
error[RT1203]: `text.join` expected a list of `Text`
  --> src/main.aivi:42:15
   |
42 | names |> text.join ", "
   |          ^^^^^^^^^^^^^^ this call failed at runtime
   |
note: list item at index 2 has type `Int`
help: convert each item to text before joining, for example with `map toText`

stack:
  0: app.main.renderNames at src/main.aivi:42:15
  1: app.main.main        at src/main.aivi:58:3
```

This example is illustrative rather than byte-for-byte normative, but it shows the level of clarity AIVI should target.
