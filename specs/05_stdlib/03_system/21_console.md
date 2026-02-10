# Console Domain

The `Console` domain is your program's voice. It handles basic interactions with the terminal. Whether you're debugging with a quick `print`, logging a status update, or asking the user for input, this is where your program talks to the human running it.

```aivi
use aivi.console
```

## Functions

### `log`

```aivi
log : String -> Effect Unit
```

Prints a message to the standard output, followed by a newline.

### `println`

```aivi
println : String -> Effect Unit
```

Alias for `log`.

### `print`

```aivi
print : String -> Effect Unit
```

Prints a message to the standard output, **without** a trailing newline.

### `error`

```aivi
error : String -> Effect Unit
```

Prints a message to the standard error.

### `readLine`

```aivi
readLine : Unit -> Effect (Result String Error)
```

Reads a line from standard input.
