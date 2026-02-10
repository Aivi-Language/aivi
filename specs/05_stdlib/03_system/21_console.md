# Console Domain

The `Console` domain is your program's voice. It handles basic interactions with the terminal. Whether you're debugging with a quick `print`, logging a status update, or asking the user for input, this is where your program talks to the human running it.

```aivi
use aivi.console
```

## Functions

| Function | Explanation |
| --- | --- |
| **log** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` to standard output with a trailing newline. |
| **println** message<br><pre><code>`String -> Effect Unit`</code></pre> | Alias for `log`. |
| **print** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` without a trailing newline. |
| **error** message<br><pre><code>`String -> Effect Unit`</code></pre> | Prints `message` to standard error. |
| **readLine** :()<br><pre><code>`Unit -> Effect (Result String Error)`</code></pre> | Reads a line from standard input. |
