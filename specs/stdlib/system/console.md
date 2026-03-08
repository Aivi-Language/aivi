# Console Domain

<!-- quick-info: {"kind":"module","name":"aivi.console"} -->
The `Console` domain is the simplest way for an AIVI program to interact with a person in a terminal.

Use it when you want to print progress messages, show errors, read a line of input, or add ANSI color to text output.

<!-- /quick-info -->
<div class="import-badge">use aivi.console</div>

<<< ../../snippets/from_md/stdlib/system/console/console_domain.aivi{aivi}

## What this module is for

`aivi.console` is useful for small command-line tools, scripts, demos, and debugging output.
It focuses on plain terminal interaction:

- writing normal output for the user,
- writing error output separately,
- reading a line from standard input,
- and styling text for terminals that understand ANSI escape codes.

Console operations live in `Effect Text ...`, so terminal interaction stays explicit in the type system.
If you need structured, machine-readable application logs rather than ad-hoc terminal text, prefer the [Log Module](./log.md).

## Common tasks

| Function | What it does | Typical use |
| --- | --- | --- |
| **log** message<br><code>Text -> Effect Text Unit</code> | Prints `message` followed by a newline to standard output. | Status updates such as “Finished importing 42 rows”. |
| **println** message<br><code>Text -> Effect Text Unit</code> | Alias for `log`. | Same as `log`; choose whichever reads better in your code. |
| **print** message<br><code>Text -> Effect Text Unit</code> | Prints `message` without adding a newline. | Prompts such as `"Name: "` before calling `readLine`. |
| **error** message<br><code>Text -> Effect Text Unit</code> | Prints `message` followed by a newline to standard error. | User-visible error reporting that should stay separate from normal output. |
| **readLine**<br><code>Effect Text (Result Text Text)</code> | Attempts to read one line from standard input and removes the trailing newline. Returns `Ok line` on success and `Err message` when input is unavailable, exhausted, or the read fails. | Simple prompts, REPL-style tools, or confirmation flows that need to distinguish “got input” from “no input available”. |

A common prompt flow is `print "Name: "` followed by `line <- readLine` and a match on `Ok` or `Err`.
If you want helpers for working with the returned `Result`, see the [Result Module](../core/result.md).

## Styling terminal output

ANSI styling is helpful when you want important messages to stand out.
For example, green text can signal success and red text can signal failure.
These helpers only build styled `Text`; they do not print anything until you pass that text to `print`, `log`, or `error`.

| Function | What it does |
| --- | --- |
| **color** color text<br><code>AnsiColor -> Text -> Text</code> | Wraps `text` with a foreground color. |
| **bgColor** color text<br><code>AnsiColor -> Text -> Text</code> | Wraps `text` with a background color. |
| **style** style text<br><code>AnsiStyle -> Text -> Text</code> | Applies the flags from an `AnsiStyle` record, including optional foreground/background colors and text styles such as bold or underline. |
| **strip** text<br><code>Text -> Text</code> | Removes ANSI escape codes from `text`. Useful before writing to logs or files. |

## ANSI Types

<<< ../../snippets/from_md/stdlib/system/console/ansi_types.aivi{aivi}

Use `Some color` in `fg` or `bg` when you want to set a color, and `None` when you want to leave that side unchanged.

## Practical tips

- Use `print` when you want the cursor to stay on the same line, then follow it with `readLine`.
- Use `error` for problems so scripts can redirect normal output and errors separately.
- `readLine` is a good fit for interactive prompts, but it can return `Err "stdin is not ready"` or `Err "end of input"` in non-interactive or piped environments.
- ANSI escapes are embedded directly in the returned text, so call `strip` before storing styled output in logs, files, or snapshot-style tests.
