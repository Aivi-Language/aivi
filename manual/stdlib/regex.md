# aivi.regex

Compiler-backed regular-expression search and replacement. Patterns use Rust `regex` syntax, and
ordinary AIVI string escaping still applies.

The MVP keeps one canonical name for each operation. Application-specific validation policy, such
as what counts as an email address or URL, belongs in application code.

```aivi
use aivi.regex (
    Pattern
    RegexError
    isMatch
    find
    findText
    findAll
    replace
    replaceAll
)
```

## Types

| Type | Meaning |
| --- | --- |
| `Pattern` | Alias for regex pattern text |
| `RegexError` | Descriptive error text returned when a pattern is invalid |

## Operations

| Function | Type | Result |
| --- | --- | --- |
| `isMatch` | `Pattern -> Text -> Task RegexError Bool` | Whether the text contains a match |
| `find` | `Pattern -> Text -> Task RegexError (Option Int)` | Character index of the first match |
| `findText` | `Pattern -> Text -> Task RegexError (Option Text)` | Text of the first match |
| `findAll` | `Pattern -> Text -> Task RegexError (List Text)` | All non-overlapping matched snippets |
| `replace` | `Pattern -> Text -> Text -> Task RegexError Text` | Replace the first match |
| `replaceAll` | `Pattern -> Text -> Text -> Task RegexError Text` | Replace every non-overlapping match |

The replacement argument comes before the input text. A valid pattern with no match succeeds with
`False`, `None`, an empty list, or the unchanged input, depending on the operation. An invalid
pattern fails the task with `RegexError`.

```aivi
use aivi.regex (isMatch)

value hasDigits : Task Text Bool = isMatch "[0-9]+" "room 42"
```
