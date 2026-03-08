# Regex Module

<!-- quick-info: {"kind":"module","name":"aivi.regex"} -->
The `aivi.regex` module helps you validate, search, extract, split, and rewrite text with regular expressions. Use regex when the *shape* of the text matters, and use [`aivi.text`](text.md) when a simple substring, prefix, suffix, or fixed separator is enough.
<!-- /quick-info -->

<div class="import-badge">use aivi.regex</div>

## What this module is for

Use `aivi.regex` when you need more than a plain text search. Common jobs include:

- validating structured text such as emails, IDs, or timestamps,
- extracting pieces of a larger string with capture groups,
- splitting on patterns instead of fixed separators,
- rewriting matched text with capture placeholders.

If `contains`, `startsWith`, `endsWith`, or a fixed-separator `split` from [`aivi.text`](text.md) is enough, prefer those first. Regex is more powerful, but it is also harder to read at a glance.

## Start here

Reach for this module based on the job you need to do:

- **Check whether a pattern appears at all** → `test`
- **Read the first match or every match** → `match` / `matches`
- **Get positions instead of matched text** → `find` / `findAll`
- **Break text apart on a pattern** → `split`
- **Rewrite matching text** → `replace` / `replaceAll`

## Overview

```aivi
use aivi.regex

emailPattern = ~r/^[\w.-]+@([\w-]+\.)+[\w-]{2,4}$/
hasEmail     = test emailPattern "contact hello@example.com"

assignment = ~r/(\w+)=(\d+)/
firstPair  = match assignment "x=10 y=20"
rewritten  = replaceAll assignment "x=10 y=20" "$1:$2"

headerName = ~r/^content-type$/i
isHeader   = test headerName "CONTENT-TYPE"
```

This example shows the three most common workflows: yes/no matching, extracting a match record, and rewriting text with capture groups.

## Types

```aivi
RegexError = | InvalidPattern Text

Match =
  {
    full: Text
    groups: List (Option Text)
    start: Int
    end: Int
  }
```

- `InvalidPattern` carries the pattern-parse error text returned by `compile`.
- `full` is the entire matched text.
- `groups` contains capture groups `1..N`; the full match is **not** repeated there because it already appears in `full`.
- `start` and `end` are the match bounds.

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **compile** pattern<br><code>Text -> Result RegexError Regex</code> | Compiles a pattern from runtime `Text`. Invalid patterns return `Err (InvalidPattern message)`. |
| **test** regex text<br><code>Regex -> Text -> Bool</code> | Checks whether `regex` matches anywhere inside `text`. |
| **match** regex text<br><code>Regex -> Text -> Option Match</code> | Returns the first match record, including capture groups, if any. |
| **matches** regex text<br><code>Regex -> Text -> List Match</code> | Returns all matches from left to right. |
| **find** regex text<br><code>Regex -> Text -> Option (Int, Int)</code> | Returns the first match range as UTF-8 byte offsets `(start, end)`. |
| **findAll** regex text<br><code>Regex -> Text -> List (Int, Int)</code> | Returns every match range as UTF-8 byte offsets. |
| **split** regex text<br><code>Regex -> Text -> List Text</code> | Splits `text` wherever the pattern matches. |
| **replace** regex text replacement<br><code>Regex -> Text -> Text -> Text</code> | Replaces only the first match. |
| **replaceAll** regex text replacement<br><code>Regex -> Text -> Text -> Text</code> | Replaces every match. |

## Two ways to build a regex

- Use **`~r/.../flags`** when the pattern is fixed in source code.
- Use **`compile`** when the pattern comes from configuration, user input, or another runtime source and you want an explicit `Result`.

`compile` is the recoverable path: invalid patterns become `Err (InvalidPattern ...)` values you can handle. When you need the same options with `compile`, put inline modifiers such as `(?i)` directly inside the pattern text.

## Pattern flags for `~r/.../flags`

Flags come after the closing `/` in a regex literal and can be combined, such as `~r/^item$/im`.

| Flag | Meaning | Example |
| --- | --- | --- |
| `i` | Case-insensitive matching | `~r/^hello$/i` matches `"HELLO"` |
| `m` | Multi-line anchors (`^` and `$` match line boundaries) | `~r/^b$/m` matches the middle line in `"a\nb\nc"` |
| `s` | Dot matches newlines | `~r/a.b/s` matches `"a\nb"` |
| `x` | Ignore literal whitespace in the pattern | `~r/a b/x` matches `"ab"` |

## Practical notes

- `match` here is the regex function from `aivi.regex`, not the language form `value match | ... => ...`.
- `match` and `matches` return `Match` records. Optional capture groups appear as `None` when that group did not participate in the match.
- `find` and `findAll` return UTF-8 byte offsets, not character or grapheme indices.
- Replacement text supports `$0` for the full match, `$1`, `$2`, and so on for capture groups, plus `$$` for a literal dollar sign.
- `replace` changes only the first match; `replaceAll` changes every match.

## Typical workflow

A common regex workflow is:

1. Build a regex with `~r/.../flags` or `compile`.
2. Use `test`, `match`, `matches`, `find`, or `split` depending on what result shape you need.
3. If you are rewriting text, finish with `replace` or `replaceAll`.

That keeps validation, extraction, and cleanup in one small pipeline while still making failures explicit when patterns come from runtime data.
