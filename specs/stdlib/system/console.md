# Console Domain

<!-- quick-info: {"kind":"module","name":"aivi.console"} -->
The `Console` domain is your program's voice. It handles basic interactions with the terminal. Whether you're debugging with a quick `print`, logging a status update, or asking the user for input, this is where your program talks to the human running it.

<!-- /quick-info -->
<div class="import-badge">use aivi.console</div>

<<< ../../snippets/from_md/stdlib/system/console/console_domain.aivi{aivi}

## Functions

| Function                                                                          | Explanation |
|-----------------------------------------------------------------------------------| --- |
| **log** message<br><code>Text -> Effect ConsoleError Unit</code>     | Prints `message` to standard output with a trailing newline. |
| **println** message<br><code>Text -> Effect ConsoleError Unit</code> | Alias for `log`. |
| **print** message<br><code>Text -> Effect ConsoleError Unit</code>   | Prints `message` without a trailing newline. |
| **error** message<br><code>Text -> Effect ConsoleError Unit</code>   | Prints `message` to standard error. |
| **readLine** :\(\)<br><code>Unit -> Effect ConsoleError Text</code>  | Reads a line from standard input. |
| **color** color text<br><code>AnsiColor -> Text -> Text</code>       | Wraps `text` in ANSI foreground color codes. |
| **bgColor** color text<br><code>AnsiColor -> Text -> Text</code>     | Wraps `text` in ANSI background color codes. |
| **style** style text<br><code>AnsiStyle -> Text -> Text</code>       | Applies multiple ANSI attributes to `text`. |
| **strip** text<br><code>Text -> Text</code>                          | Removes ANSI escape sequences from `text`. |

## ANSI Types

<<< ../../snippets/from_md/stdlib/system/console/ansi_types.aivi{aivi}
