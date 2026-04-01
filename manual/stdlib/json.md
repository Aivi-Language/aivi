# aivi.data.json

Legacy JSON compatibility helpers.

The current executable helpers operate on raw JSON text fragments and return `Task Text A`.
The stdlib module also carries structural JSON types and predicates, but this page focuses on the
task-backed helper surface that exists today.

Current status: this is a **compatibility** layer. The target architecture is provider-owned decode
straight into the annotated signal or operation result type; JSON-as-text workflows are not the
intended steady state for external data.

## Import

```aivi
use aivi.data.json (
    validate
    get
    at
    keys
    pretty
    minify
)
```

## Overview

| Function   | Type                                      | Description                       |
|------------|-------------------------------------------|-----------------------------------|
| `validate` | `Text -> Task Text Bool`                  | Check whether text is valid JSON  |
| `get`      | `Text -> Text -> Task Text (Option Text)` | Get an object field by key        |
| `at`       | `Text -> Int -> Task Text (Option Text)`  | Get an array element by index     |
| `keys`     | `Text -> Task Text (List Text)`           | List object keys                  |
| `pretty`   | `Text -> Task Text Text`                  | Pretty-print JSON                 |
| `minify`   | `Text -> Task Text Text`                  | Minify JSON (remove whitespace)   |

## Functions

### validate

```aivi
validate : Text -> Task Text Bool
```

Returns `True` if the text is valid JSON, `False` otherwise. Never fails — invalid text yields
`False`, not a task error.

```aivi
use aivi.data.json (validate)

func checkJson = json =>
```

### get

```aivi
get : Text -> Text -> Task Text (Option Text)
```

Retrieve an object field by key. The result is the field value serialised back to JSON text,
so nested objects and arrays are preserved as `Text`. Returns `None` when the key is absent.
Fails the task when the input is not valid JSON.

```aivi
use aivi.data.json (get)

func getName = json =>
```

### at

```aivi
at : Text -> Int -> Task Text (Option Text)
```

Retrieve an array element by zero-based index. Returns `None` when the index is out of bounds.
Fails the task when the input is not valid JSON.

```aivi
use aivi.data.json (at)

func firstItem = json =>
```

### keys

```aivi
keys : Text -> Task Text (List Text)
```

Return the keys of a JSON object in insertion order. Returns an empty list for non-objects.
Fails the task when the input is not valid JSON.

```aivi
use aivi.data.json (keys)

func objectKeys = json =>
```

### pretty

```aivi
pretty : Text -> Task Text Text
```

Re-format JSON with two-space indentation. Fails the task when the input is not valid JSON.

```aivi
use aivi.data.json (pretty)

func format = json =>
```

### minify

```aivi
minify : Text -> Task Text Text
```

Remove all insignificant whitespace from JSON. Fails the task when the input is not valid JSON.

```aivi
use aivi.data.json (minify)

func compact = json =>
```

## Error type

```aivi
type JsonError =
  | InvalidJson
  | MissingKey
  | IndexOutOfBounds
  | WrongType
```

`JsonError` represents the four logical failure modes when working with JSON data.
Task failures carry a descriptive `Text` error message (the `Text` in `Task Text A`).

## Example — decode a simple object

```aivi
use aivi.data.json (
    get
    keys
)

use aivi.option (withDefault)

func extractName = json =>
```

## Example — normalise before storage

Extract each step into a named function so no pipes are nested.

```aivi
use aivi.data.json (
    minify
    validate
)

use aivi.core (Task)

func minifyIfValid = raw isValid =>
func storeJson = raw =>
```
