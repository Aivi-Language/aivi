# Regex Domain

The `Regex` domain handles **Pattern Matching** for text. Whether you're validating emails, scraping data, or searching logs, simple substring checks often aren't enough. Regex gives you a powerful, concise language to describe *shapes* of text. AIVI's regex support is safe (checked at compile-time with `~r/.../`) and fast (compiling to native matching engines), so you don't have to worry about runtime crashes from bad patterns.

## Overview

```aivi
use aivi.regex (Regex)

email_pattern = ~r/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/
match = Regex.test(email_pattern, "user@example.com")

// With flags (example: case-insensitive)
email_ci = ~r/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/i
```

## Types

```aivi
type RegexError = InvalidPattern Text

Match = {
  full: Text,
  groups: List (Option Text),
  start: Int,
  end: Int
}
```

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **compile** pattern<br><pre><code>`Text -> Result RegexError Regex`</code></pre> | Builds a `Regex`, returning `RegexError` when invalid. |
| **test** regex text<br><pre><code>`Regex -> Text -> Bool`</code></pre> | Returns whether the regex matches anywhere in `text`. |
| **match** regex text<br><pre><code>`Regex -> Text -> Option Match`</code></pre> | Returns the first `Match` with capture groups. |
| **matches** regex text<br><pre><code>`Regex -> Text -> List Match`</code></pre> | Returns all matches in left-to-right order. |
| **find** regex text<br><pre><code>`Regex -> Text -> Option (Int, Int)`</code></pre> | Returns the first match byte index range. |
| **findAll** regex text<br><pre><code>`Regex -> Text -> List (Int, Int)`</code></pre> | Returns all match byte index ranges. |
| **split** regex text<br><pre><code>`Regex -> Text -> List Text`</code></pre> | Splits `text` on regex matches. |
| **replace** regex text replacement<br><pre><code>`Regex -> Text -> Text -> Text`</code></pre> | Replaces the first match. |
| **replaceAll** regex text replacement<br><pre><code>`Regex -> Text -> Text -> Text`</code></pre> | Replaces all matches. |

Notes:
- `match` returns the first match with capture groups (if any).
- `matches` returns all matches in left-to-right order.
- `replace` changes the first match only; `replaceAll` replaces all matches.
- Replacement strings support `$1`, `$2`, ... for capture groups.
