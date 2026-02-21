# Regex Module

<!-- quick-info: {"kind":"module","name":"aivi.regex"} -->
The `Regex` module handles **Pattern Matching** for text. Whether you're validating emails, scraping data, or searching logs, simple substring checks often aren't enough. Regex gives you a powerful, concise language to describe *shapes* of text. AIVI's regex support is safe (checked at compile-time with `~r/.../`) and fast (compiling to native matching engines), so you don't have to worry about runtime crashes from bad patterns.

<!-- /quick-info -->
<div class="import-badge">use aivi.regex</div>

## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/24_regex/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/00_core/24_regex/block_02.aivi{aivi}

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
