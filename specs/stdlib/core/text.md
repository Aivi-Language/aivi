# Text Module

<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
It focuses on predictable, Unicode-aware behavior, and uses `Option`/`Result` instead of
sentinel values like `-1`.
<!-- /quick-info -->
<div class="import-badge">use aivi.text</div>

## Overview

<<< ../../snippets/from_md/05_stdlib/00_core/02_text/block_01.aivi{aivi}

## Types

<<< ../../snippets/from_md/05_stdlib/00_core/02_text/block_02.aivi{aivi}

## Core API (v0.1)

### Length and inspection

| Function | Explanation |
| --- | --- |
| **length** text<br><pre><code>`Text -> Int`</code></pre> | Returns the number of Unicode scalar values in `text`. |
| **isEmpty** text<br><pre><code>`Text -> Bool`</code></pre> | Returns `true` when `text` has zero length. |

### Character predicates

| Function | Explanation |
| --- | --- |
| **isDigit** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode digit. |
| **isAlpha** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode letter. |
| **isAlnum** char<br><pre><code>`Char -> Bool`</code></pre> | <!-- quick-info: {"kind":"function","name":"isAlnum","module":"aivi.text"} -->Returns whether `char` is a Unicode letter or digit.<!-- /quick-info --> |
| **isSpace** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is a Unicode whitespace. |
| **isUpper** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is uppercase. |
| **isLower** char<br><pre><code>`Char -> Bool`</code></pre> | Returns whether `char` is lowercase. |

### Search and comparison

| Function | Explanation |
| --- | --- |
| **contains** needle haystack<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `needle` occurs in `haystack`. |
| **startsWith** prefix text<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `text` starts with `prefix`. |
| **endsWith** suffix text<br><pre><code>`Text -> Text -> Bool`</code></pre> | Returns whether `text` ends with `suffix`. |
| **indexOf** needle haystack<br><pre><code>`Text -> Text -> Option Int`</code></pre> | Returns the first index of `needle`, or `None` when not found. |
| **lastIndexOf** needle haystack<br><pre><code>`Text -> Text -> Option Int`</code></pre> | Returns the last index of `needle`, or `None` when not found. |
| **count** needle haystack<br><pre><code>`Text -> Text -> Int`</code></pre> | Returns the number of non-overlapping occurrences. |
| **compare** a b<br><pre><code>`Text -> Text -> Int`</code></pre> | Returns `-1`, `0`, or `1` in Unicode codepoint order (not locale-aware). |

Notes:
- `indexOf` and `lastIndexOf` return `None` when not found.

### Slicing and splitting

| Function | Explanation |
| --- | --- |
| **slice** start end text<br><pre><code>`Int -> Int -> Text -> Text`</code></pre> | Returns the substring from `start` (inclusive) to `end` (exclusive). |
| **split** sep text<br><pre><code>`Text -> Text -> List Text`</code></pre> | Splits `text` on `sep`. |
| **splitLines** text<br><pre><code>`Text -> List Text`</code></pre> | Splits on line endings. |
| **chunk** size text<br><pre><code>`Int -> Text -> List Text`</code></pre> | Splits into codepoint chunks of length `size`. |

Notes:
- `slice start end text` is half-open (`start` inclusive, `end` exclusive) and clamps out-of-range indices.
- `chunk` splits by codepoint count, not bytes.

### Trimming and padding

| Function | Explanation |
| --- | --- |
| **trim** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from both ends. |
| **trimStart** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from the start. |
| **trimEnd** text<br><pre><code>`Text -> Text`</code></pre> | Removes Unicode whitespace from the end. |
| **padStart** width fill text<br><pre><code>`Int -> Text -> Text -> Text`</code></pre> | Pads on the left to reach `width` using repeated `fill`. |
| **padEnd** width fill text<br><pre><code>`Int -> Text -> Text -> Text`</code></pre> | Pads on the right to reach `width` using repeated `fill`. |

Notes:
- `padStart width fill text` repeats `fill` as needed and truncates extra.

### Modification

| Function | Explanation |
| --- | --- |
| **replace** needle replacement text<br><pre><code>`Text -> Text -> Text -> Text`</code></pre> | Replaces the first occurrence of `needle`. |
| **replaceAll** needle replacement text<br><pre><code>`Text -> Text -> Text -> Text`</code></pre> | Replaces all occurrences of `needle`. |
| **remove** needle text<br><pre><code>`Text -> Text -> Text`</code></pre> | Removes all occurrences of `needle`. |
| **repeat** count text<br><pre><code>`Int -> Text -> Text`</code></pre> | Repeats `text` `count` times. |
| **reverse** text<br><pre><code>`Text -> Text`</code></pre> | Reverses grapheme clusters. |
| **concat** parts<br><pre><code>`List Text -> Text`</code></pre> | Concatenates all parts into one `Text`. |

Notes:
- `replace` changes the first occurrence only.
- `remove needle text` is `replaceAll needle "" text`.
- `reverse` is grapheme-aware and may be linear-time with extra allocations.

### Case and normalization

| Function | Explanation |
| --- | --- |
| **toLower** text<br><pre><code>`Text -> Text`</code></pre> | Converts to lowercase using Unicode rules. |
| **toUpper** text<br><pre><code>`Text -> Text`</code></pre> | Converts to uppercase using Unicode rules. |
| **capitalize** text<br><pre><code>`Text -> Text`</code></pre> | Uppercases the first grapheme and lowercases the rest. |
| **titleCase** text<br><pre><code>`Text -> Text`</code></pre> | Converts to title case using Unicode rules. |
| **caseFold** text<br><pre><code>`Text -> Text`</code></pre> | Produces a case-folded form for case-insensitive comparisons. |
| **normalizeNFC** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFC. |
| **normalizeNFD** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFD. |
| **normalizeNFKC** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFKC. |
| **normalizeNFKD** text<br><pre><code>`Text -> Text`</code></pre> | Normalizes to NFKD. |

### Encoding / decoding

| Function | Explanation |
| --- | --- |
| **toBytes** encoding text<br><pre><code>`Encoding -> Text -> Bytes`</code></pre> | Encodes `text` into `Bytes` using `encoding`. |
| **fromBytes** encoding bytes<br><pre><code>`Encoding -> Bytes -> Result TextError Text`</code></pre> | Decodes `bytes` and returns `TextError` on invalid input. |

### Formatting and conversion

| Function | Explanation |
| --- | --- |
| **toText** value<br><pre><code>`A -> Text`</code></pre> | Converts `value` to `Text` (via in-scope `ToText` instances; otherwise uses the default debug formatter). |
| **parseInt** text<br><pre><code>`Text -> Option Int`</code></pre> | Parses a decimal integer, returning `None` on failure. |
| **parseFloat** text<br><pre><code>`Text -> Option Float`</code></pre> | Parses a decimal float, returning `None` on failure. |

## Usage Examples

<<< ../../snippets/from_md/05_stdlib/00_core/02_text/block_03.aivi{aivi}
