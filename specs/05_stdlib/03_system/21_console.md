# Console Domain

The `Console` domain provides basic input/output capabilities.

```aivi
use std.Console
```

## Functions

### `log`

```aivi
log : String -> Effect Unit
```

Prints a message to the standard output, followed by a newline.

### `print`

```aivi
print : String -> Effect Unit
```

Prints a message to the standard output, **without** a trailing newline.
```

Prints a message to the standard output.

### `error`

```aivi
error : String -> Effect Unit
```

Prints a message to the standard error.

### `read_line`

```aivi
read_line : Unit -> Effect (Result String Error)
```

Reads a line from standard input.
