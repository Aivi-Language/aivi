# Regex Module

<!-- quick-info: {"kind":"module","name":"aivi.regex"} -->
The `Regex` module handles **Pattern Matching** for text. Whether you're validating emails, scraping data, or searching logs, simple substring checks often aren't enough. Regex gives you a powerful, concise language to describe *shapes* of text. AIVI's regex support is safe (checked at compile-time with `~r/.../`) and fast (compiling to native matching engines), so you don't have to worry about runtime crashes from bad patterns.

<!-- /quick-info -->
<div class="import-badge">use aivi.regex</div>

## What this module is for

Use `aivi.regex` when you need more than a plain substring search. Regular expressions are useful for tasks such as:

- validating structured text,
- extracting pieces of a larger string,
- splitting text on patterns instead of fixed separators,
- replacing matched text with capture groups.

If a simple `contains`, `startsWith`, or `split` from `aivi.text` is enough, prefer those first. Regex is most helpful when the shape of the text matters.

## Overview

<<< ../../snippets/from_md/stdlib/core/regex/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/core/regex/types.aivi{aivi}

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **compile** pattern<br><code>Text -> Result RegexError Regex</code> | Compiles a pattern at runtime. Invalid patterns return `RegexError` instead of crashing. |
| **test** regex text<br><code>Regex -> Text -> Bool</code> | Checks whether the regex matches anywhere inside `text`. |
| **match** regex text<br><code>Regex -> Text -> Option Match</code> | Returns the first match together with its capture groups, if any. |
| **matches** regex text<br><code>Regex -> Text -> List Match</code> | Returns all matches from left to right. |
| **find** regex text<br><code>Regex -> Text -> Option (Int, Int)</code> | Returns the first match range as a byte-index pair. |
| **findAll** regex text<br><code>Regex -> Text -> List (Int, Int)</code> | Returns all match ranges as byte-index pairs. |
| **split** regex text<br><code>Regex -> Text -> List Text</code> | Splits `text` wherever the pattern matches. |
| **replace** regex text replacement<br><code>Regex -> Text -> Text -> Text</code> | Replaces only the first match. |
| **replaceAll** regex text replacement<br><code>Regex -> Text -> Text -> Text</code> | Replaces every match. |

## Two ways to build a regex

- Use **`~r/.../`** when the pattern is known in source code and should be validated at compile time.
- Use **`compile`** when the pattern comes from configuration, user input, or another runtime source.

Compile-time validation is usually the safer and more convenient choice because bad patterns are caught before the program runs.

## Practical notes

- `match` and `matches` include capture-group information.
- `find` and `findAll` return ranges, which is useful when you want positions rather than matched text.
- Replacement strings support `$1`, `$2`, and similar placeholders for capture groups.
- `replace` affects the first match only; `replaceAll` affects every match.

## Typical workflow

A common regex workflow is:

1. Build or compile a regex.
2. Test or match it against text.
3. Read capture groups, ranges, or replacement output.

That lets you keep validation, parsing, and text cleanup in one small pipeline.
