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

```aivi
compile : Text -> Result RegexError Regex
test : Regex -> Text -> Bool
match : Regex -> Text -> Option Match
matches : Regex -> Text -> List Match
find : Regex -> Text -> Option (Int, Int)
findAll : Regex -> Text -> List (Int, Int)
split : Regex -> Text -> List Text
replace : Regex -> Text -> Text -> Text
replaceAll : Regex -> Text -> Text -> Text
```

Notes:
- `match` returns the first match with capture groups (if any).
- `matches` returns all matches in left-to-right order.
- `replace` changes the first match only; `replaceAll` replaces all matches.
- Replacement strings support `$1`, `$2`, ... for capture groups.
