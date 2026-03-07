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

Because console operations can fail—for example, if output is redirected to a closed stream—they live in `Effect`.

## Common tasks

| Function | What it does | Typical use |
| --- | --- | --- |
| **log** message<br><code>Text -> Effect ConsoleError Unit</code> | Prints `message` followed by a newline to standard output. | Status updates such as “Finished importing 42 rows”. |
| **println** message<br><code>Text -> Effect ConsoleError Unit</code> | Alias for `log`. | Same as `log`; choose whichever reads better in your code. |
| **print** message<br><code>Text -> Effect ConsoleError Unit</code> | Prints `message` without adding a newline. | Prompts such as `"Name: "` before calling `readLine`. |
| **error** message<br><code>Text -> Effect ConsoleError Unit</code> | Prints `message` to standard error. | User-visible error reporting that should stay separate from normal output. |
| **readLine** :() <br><code>Unit -> Effect ConsoleError Text</code> | Reads one line of text from standard input. | Simple prompts, REPL-style tools, or confirmation flows. |

## Styling terminal output

ANSI styling is helpful when you want important messages to stand out.
For example, green text can signal success and red text can signal failure.

| Function | What it does |
| --- | --- |
| **color** color text<br><code>AnsiColor -> Text -> Text</code> | Wraps `text` with a foreground color. |
| **bgColor** color text<br><code>AnsiColor -> Text -> Text</code> | Wraps `text` with a background color. |
| **style** style text<br><code>AnsiStyle -> Text -> Text</code> | Applies one or more ANSI text styles, such as bold or underline. |
| **strip** text<br><code>Text -> Text</code> | Removes ANSI escape codes from `text`. Useful before writing to logs or files. |

## ANSI Types

<<< ../../snippets/from_md/stdlib/system/console/ansi_types.aivi{aivi}

## Practical tips

- Use `print` when you want the cursor to stay on the same line, then follow it with `readLine`.
- Use `error` for problems so scripts can redirect normal output and errors separately.
- If output may be consumed by another program, consider calling `strip` before storing colored text.
