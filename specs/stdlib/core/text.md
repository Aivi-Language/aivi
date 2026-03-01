# Text Module

<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
It focuses on predictable, Unicode-aware behavior, and uses `Option`/`Result` instead of
sentinel values like `-1`.
<!-- /quick-info -->
<div class="import-badge">use aivi.text</div>

## Overview

<<< ../../snippets/from_md/stdlib/core/text/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/core/text/types.aivi{aivi}

## Core API (v0.1)

### Length and inspection

| Function | Explanation |
| --- | --- |
| **length** text<br><code>Text -> Int</code> | Returns the number of Unicode scalar values in `text`. |
| **isEmpty** text<br><code>Text -> Bool</code> | Returns `true` when `text` has zero length. |

### Character predicates

| Function | Explanation |
| --- | --- |
| **isDigit** char<br><code>Char -> Bool</code> | Returns whether `char` is a Unicode digit. |
| **isAlpha** char<br><code>Char -> Bool</code> | Returns whether `char` is a Unicode letter. |
| **isAlnum** char<br><code>Char -> Bool</code> | <!-- quick-info: {"kind":"function","name":"isAlnum","module":"aivi.text"} -->Returns whether `char` is a Unicode letter or digit.<!-- /quick-info --> |
| **isSpace** char<br><code>Char -> Bool</code> | Returns whether `char` is a Unicode whitespace. |
| **isUpper** char<br><code>Char -> Bool</code> | Returns whether `char` is uppercase. |
| **isLower** char<br><code>Char -> Bool</code> | Returns whether `char` is lowercase. |

### Search and comparison

| Function | Explanation |
| --- | --- |
| **contains** needle haystack<br><code>Text -> Text -> Bool</code> | Returns whether `needle` occurs in `haystack`. |
| **startsWith** prefix text<br><code>Text -> Text -> Bool</code> | Returns whether `text` starts with `prefix`. |
| **endsWith** suffix text<br><code>Text -> Text -> Bool</code> | Returns whether `text` ends with `suffix`. |
| **indexOf** needle haystack<br><code>Text -> Text -> Option Int</code> | Returns the first index of `needle`, or `None` when not found. |
| **lastIndexOf** needle haystack<br><code>Text -> Text -> Option Int</code> | Returns the last index of `needle`, or `None` when not found. |
| **count** needle haystack<br><code>Text -> Text -> Int</code> | Returns the number of non-overlapping occurrences. |
| **compare** a b<br><code>Text -> Text -> Int</code> | Returns `-1`, `0`, or `1` in Unicode codepoint order (not locale-aware). |

Notes:
- `indexOf` and `lastIndexOf` return `None` when not found.

### Slicing and splitting

| Function | Explanation |
| --- | --- |
| **slice** start end text<br><code>Int -> Int -> Text -> Text</code> | Returns the substring from `start` (inclusive) to `end` (exclusive). |
| **split** sep text<br><code>Text -> Text -> List Text</code> | Splits `text` on `sep`. |
| **splitLines** text<br><code>Text -> List Text</code> | Splits on line endings. |
| **chunk** size text<br><code>Int -> Text -> List Text</code> | Splits into codepoint chunks of length `size`. |

Notes:
- `slice start end text` is half-open (`start` inclusive, `end` exclusive) and clamps out-of-range indices.
- `chunk` splits by codepoint count, not bytes.

### Trimming and padding

| Function | Explanation |
| --- | --- |
| **trim** text<br><code>Text -> Text</code> | Removes Unicode whitespace from both ends. |
| **trimStart** text<br><code>Text -> Text</code> | Removes Unicode whitespace from the start. |
| **trimEnd** text<br><code>Text -> Text</code> | Removes Unicode whitespace from the end. |
| **padStart** width fill text<br><code>Int -> Text -> Text -> Text</code> | Pads on the left to reach `width` using repeated `fill`. |
| **padEnd** width fill text<br><code>Int -> Text -> Text -> Text</code> | Pads on the right to reach `width` using repeated `fill`. |

Notes:
- `padStart width fill text` repeats `fill` as needed and truncates extra.

### Modification

| Function | Explanation |
| --- | --- |
| **replace** needle replacement text<br><code>Text -> Text -> Text -> Text</code> | Replaces the first occurrence of `needle`. |
| **replaceAll** needle replacement text<br><code>Text -> Text -> Text -> Text</code> | Replaces all occurrences of `needle`. |
| **remove** needle text<br><code>Text -> Text -> Text</code> | Removes all occurrences of `needle`. |
| **repeat** count text<br><code>Int -> Text -> Text</code> | Repeats `text` `count` times. |
| **reverse** text<br><code>Text -> Text</code> | Reverses grapheme clusters. |
| **concat** parts<br><code>List Text -> Text</code> | Concatenates all parts into one `Text`. |

Notes:
- `replace` changes the first occurrence only.
- `remove needle text` is `replaceAll needle "" text`.
- `reverse` is grapheme-aware and may be linear-time with extra allocations.

### Case and normalization

| Function | Explanation |
| --- | --- |
| **toLower** text<br><code>Text -> Text</code> | Converts to lowercase using Unicode rules. |
| **toUpper** text<br><code>Text -> Text</code> | Converts to uppercase using Unicode rules. |
| **capitalize** text<br><code>Text -> Text</code> | Uppercases the first grapheme and lowercases the rest. |
| **titleCase** text<br><code>Text -> Text</code> | Converts to title case using Unicode rules. |
| **caseFold** text<br><code>Text -> Text</code> | Produces a case-folded form for case-insensitive comparisons. |
| **normalizeNFC** text<br><code>Text -> Text</code> | Normalizes to NFC. |
| **normalizeNFD** text<br><code>Text -> Text</code> | Normalizes to NFD. |
| **normalizeNFKC** text<br><code>Text -> Text</code> | Normalizes to NFKC. |
| **normalizeNFKD** text<br><code>Text -> Text</code> | Normalizes to NFKD. |

### Encoding / decoding

| Function | Explanation |
| --- | --- |
| **toBytes** encoding text<br><code>Encoding -> Text -> Bytes</code> | Encodes `text` into `Bytes` using `encoding`. |
| **fromBytes** encoding bytes<br><code>Encoding -> Bytes -> Result TextError Text</code> | Decodes `bytes` and returns `TextError` on invalid input. |

### Formatting and conversion

| Function | Explanation |
| --- | --- |
| **toText** value<br><code>A -> Text</code> | Converts `value` to `Text` (via in-scope `ToText` instances; otherwise uses the default debug formatter). |
| **parseInt** text<br><code>Text -> Option Int</code> | Parses a decimal integer, returning `None` on failure. |
| **parseFloat** text<br><code>Text -> Option Float</code> | Parses a decimal float, returning `None` on failure. |

## Usage Examples

<<< ../../snippets/from_md/stdlib/core/text/usage_examples.aivi{aivi}
