# Console Domain

<!-- quick-info: {"kind":"module","name":"aivi.console"} -->
The `Console` domain is your program's voice. It handles basic interactions with the terminal. Whether you're debugging with a quick `print`, logging a status update, or asking the user for input, this is where your program talks to the human running it.

<!-- /quick-info -->
<<< ../../snippets/from_md/05_stdlib/03_system/21_console/block_01.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **log** message<br><pre><code>`Text -> Effect ConsoleError Unit`</code></pre> | Prints `message` to standard output with a trailing newline. |
| **println** message<br><pre><code>`Text -> Effect ConsoleError Unit`</code></pre> | Alias for `log`. |
| **print** message<br><pre><code>`Text -> Effect ConsoleError Unit`</code></pre> | Prints `message` without a trailing newline. |
| **error** message<br><pre><code>`Text -> Effect ConsoleError Unit`</code></pre> | Prints `message` to standard error. |
| **readLine** :()<br><pre><code>`Unit -> Effect ConsoleError Text`</code></pre> | Reads a line from standard input. |
| **color** color text<br><pre><code>`AnsiColor -> Text -> Text`</code></pre> | Wraps `text` in ANSI foreground color codes. |
| **bgColor** color text<br><pre><code>`AnsiColor -> Text -> Text`</code></pre> | Wraps `text` in ANSI background color codes. |
| **style** style text<br><pre><code>`AnsiStyle -> Text -> Text`</code></pre> | Applies multiple ANSI attributes to `text`. |
| **strip** text<br><pre><code>`Text -> Text`</code></pre> | Removes ANSI escape sequences from `text`. |

## ANSI Types

<<< ../../snippets/from_md/05_stdlib/03_system/21_console/block_02.aivi{aivi}
