# Text Module

<!-- quick-info: {"kind":"module","name":"aivi.text"} -->
The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
It focuses on predictable behavior, Unicode-aware operations where available, and
`Option`/`Result` return values instead of sentinel values like `-1`.
<!-- /quick-info -->
<div class="import-badge">use aivi.text</div>

## What this module is for

`aivi.text` is the main toolbox for working with user-facing strings and individual
characters. It covers common tasks such as searching, slicing, trimming, case conversion,
parsing numbers from text, and encoding or decoding bytes. When you need pattern-based
matching instead of fixed-text operations, use [`aivi.regex`](/stdlib/core/regex).

The module is designed to be predictable:

- Unicode rules are respected where that matters.
- Missing search results use `Option` instead of magic values.
- Failed decodes use `Result` instead of silent corruption.

## Start here

Start here by matching the job to the section:

- Need to **find or compare text** → [Search and comparison](#search-and-comparison)
- Need to **split, join, or slice text** → [Slicing and splitting](#slicing-and-splitting)
- Need to **clean up or normalize user input** → [Trimming and padding](#trimming-and-padding) or [Case and normalization](#case-and-normalization)
- Need to **turn text into numbers or bytes** → [Encoding and decoding](#encoding-and-decoding) or [Formatting and conversion](#formatting-and-conversion)

## Overview

<<< ../../snippets/from_md/stdlib/core/text/overview.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/core/text/types.aivi{aivi}

## Core API (v0.1)

### Length and inspection

| Function | Explanation |
| --- | --- |
| **length** text<br><code>Text -> Int</code> | Returns the number of user-perceived characters (grapheme clusters) in `text`. |
| **isEmpty** text<br><code>Text -> Bool</code> | Checks whether the text has any content. |

### Character predicates

| Function | Explanation |
| --- | --- |
| **isDigit** char<br><code>Char -> Bool</code> | Returns whether `char` is an ASCII digit (`0`-`9`). |
| **isAlpha** char<br><code>Char -> Bool</code> | Returns whether `char` is a Unicode letter. |
| **isAlnum** char<br><code>Char -> Bool</code> | <!-- quick-info: {"kind":"function","name":"isAlnum","module":"aivi.text"} -->Returns whether `char` is a Unicode letter or digit.<!-- /quick-info --> |
| **isSpace** char<br><code>Char -> Bool</code> | Returns whether `char` is Unicode whitespace. |
| **isUpper** char<br><code>Char -> Bool</code> | Returns whether `char` is uppercase. |
| **isLower** char<br><code>Char -> Bool</code> | Returns whether `char` is lowercase. |

### Search and comparison

Use this group when you want to answer “does this text contain X?”, “where is X?”, or “how should these two values sort?”.

For pattern-based search, capture groups, or regex replacement, use [`aivi.regex`](/stdlib/core/regex).

| Function | Explanation |
| --- | --- |
| **contains** needle haystack<br><code>Text -> Text -> Bool</code> | Checks whether `needle` appears anywhere in `haystack`. |
| **startsWith** prefix text<br><code>Text -> Text -> Bool</code> | Checks whether `text` starts with `prefix`. |
| **endsWith** suffix text<br><code>Text -> Text -> Bool</code> | Checks whether `text` ends with `suffix`. |
| **indexOf** needle haystack<br><code>Text -> Text -> Option Int</code> | Returns the first byte offset of `needle`, or `None` when it is not found. |
| **lastIndexOf** needle haystack<br><code>Text -> Text -> Option Int</code> | Returns the last byte offset of `needle`, or `None` when it is not found. |
| **count** needle haystack<br><code>Text -> Text -> Int</code> | Counts non-overlapping occurrences of `needle`. |
| **compare** a b<br><code>Text -> Text -> Int</code> | Returns `-1`, `0`, or `1` using Unicode codepoint order. |

The comparison operators `<`, `<=`, `>`, and `>=` are built in for `Text` and follow the same Unicode codepoint ordering as `compare`.

<<< ../../snippets/from_md/stdlib/core/text/block_01.aivi{aivi}


Notes:

- `indexOf` and `lastIndexOf` return `None` when the search fails.
- In v0.1, `indexOf` and `lastIndexOf` return UTF-8 byte offsets. Treat them as search results, not as guaranteed inputs to `slice` on non-ASCII text.
- Text ordering here is not locale-aware. Use locale-aware tooling when human sorting rules matter.

### Slicing and splitting

These helpers are the usual choice when text arrives as one value and you need to break it into smaller pieces or stitch pieces back together.

| Function | Explanation |
| --- | --- |
| **slice** start end text<br><code>Int -> Int -> Text -> Text</code> | Returns the substring from `start` (inclusive) to `end` (exclusive). |
| **split** sep text<br><code>Text -> Text -> List Text</code> | Splits `text` on a fixed separator. |
| **splitLines** text<br><code>Text -> List Text</code> | Splits text into lines. |
| **chunk** size text<br><code>Int -> Text -> List Text</code> | Breaks text into chunks of `size` Unicode scalar values. |
| **join** sep parts<br><code>Text -> List Text -> Text</code> | Joins pieces with `sep` between them. |

Notes:

- `slice` clamps out-of-range indices rather than failing.
- `split` uses a fixed separator, not a pattern. For pattern-based splitting, use [`aivi.regex`](/stdlib/core/regex).
- `chunk` counts Unicode scalar values, not bytes.
- `join sep []` returns `""`, and `join sep [x]` returns `x` unchanged.
- If exact positions matter for text with combining marks or zero-width-joiner emoji, add regression tests around `slice` and `chunk`.

### Trimming and padding

| Function | Explanation |
| --- | --- |
| **trim** text<br><code>Text -> Text</code> | Removes Unicode whitespace from both ends. |
| **trimStart** text<br><code>Text -> Text</code> | Removes Unicode whitespace from the start. |
| **trimEnd** text<br><code>Text -> Text</code> | Removes Unicode whitespace from the end. |
| **padStart** width fill text<br><code>Int -> Text -> Text -> Text</code> | Pads on the left until `text` reaches `width`. |
| **padEnd** width fill text<br><code>Int -> Text -> Text -> Text</code> | Pads on the right until `text` reaches `width`. |

`padStart` and `padEnd` repeat `fill` as needed and truncate any extra padding.
If `fill` is `""`, the original text is returned unchanged.

### Modification

| Function | Explanation |
| --- | --- |
| **replace** needle replacement text<br><code>Text -> Text -> Text -> Text</code> | Replaces the first occurrence of `needle`. |
| **replaceAll** needle replacement text<br><code>Text -> Text -> Text -> Text</code> | Replaces every occurrence of `needle`. |
| **remove** needle text<br><code>Text -> Text -> Text</code> | Removes every occurrence of `needle`. |
| **repeat** count text<br><code>Int -> Text -> Text</code> | Repeats `text` `count` times. |
| **reverse** text<br><code>Text -> Text</code> | Reverses the text. |
| **concat** parts<br><code>List Text -> Text</code> | Concatenates a list of text values into one. |

Notes:

- `replace` changes only the first match.
- `replace`, `replaceAll`, and `remove` work on fixed text, not regex patterns. For pattern-aware replacement, use [`aivi.regex`](/stdlib/core/regex).
- `remove needle text` is equivalent to `replaceAll needle "" text`.
- In the current v0.1 runtime, `reverse` works at the Unicode scalar-value level. Test carefully if your data includes combining marks or complex emoji.

### Case and normalization

Reach for this section when you are comparing human text, cleaning input before storage, or making visually similar Unicode text behave consistently.

| Function | Explanation |
| --- | --- |
| **toLower** text<br><code>Text -> Text</code> | Converts text to lowercase using Unicode rules. |
| **toUpper** text<br><code>Text -> Text</code> | Converts text to uppercase using Unicode rules. |
| **capitalize** text<br><code>Text -> Text</code> | Uppercases the first character and leaves the rest unchanged. |
| **titleCase** text<br><code>Text -> Text</code> | Splits on whitespace and capitalizes each segment. |
| **caseFold** text<br><code>Text -> Text</code> | Produces a case-folded form for case-insensitive comparison. |
| **normalizeNFC** text<br><code>Text -> Text</code> | Normalizes to NFC. |
| **normalizeNFD** text<br><code>Text -> Text</code> | Normalizes to NFD. |
| **normalizeNFKC** text<br><code>Text -> Text</code> | Normalizes to NFKC. |
| **normalizeNFKD** text<br><code>Text -> Text</code> | Normalizes to NFKD. |

Normalization is useful when visually similar text can have different underlying Unicode representations.

### Encoding and decoding

Use this section when text must cross a byte boundary, such as a file format, network
protocol, or external API.

| Function | Explanation |
| --- | --- |
| **toBytes** encoding text<br><code>Encoding -> Text -> Bytes</code> | Encodes text into bytes using the chosen encoding. |
| **fromBytes** encoding bytes<br><code>Encoding -> Bytes -> Result TextError Text</code> | Decodes bytes into text, returning `TextError` when decoding fails. |

Notes:

- In v0.1, `Utf16` and `Utf32` use little-endian byte order and do not add or require a BOM.
- ⚠️ `toBytes Latin1 text` replaces characters outside Latin-1 with `?` — this is lossy, so verify your input if exact encoding matters.
- Handle `fromBytes` with `match`, `attempt`, or `or` so decode failures stay explicit.

<<< ../../snippets/from_md/stdlib/core/text/block_02.aivi{aivi}


### Formatting and conversion

This section is the bridge between raw text and other value types.

| Function | Explanation |
| --- | --- |
| **debugText** value<br><code>A -> Text</code> | Converts a value to `Text` with the default debug formatter. For ordinary `ToText` conversion, use [`toText` from `aivi.prelude`](/stdlib/core/prelude). |
| **parseInt** text<br><code>Text -> Option Int</code> | Parses a decimal integer and returns `None` on failure. |
| **parseFloat** text<br><code>Text -> Option Float</code> | Parses a decimal float and returns `None` on failure. |

When you are parsing user input, the usual flow is: clean the text, parse it, then decide what to do with `None`. `parseInt` and `parseFloat` do not trim whitespace for you.

<<< ../../snippets/from_md/stdlib/core/text/block_03.aivi{aivi}


## Usage examples

<<< ../../snippets/from_md/stdlib/core/text/usage_examples.aivi{aivi}
