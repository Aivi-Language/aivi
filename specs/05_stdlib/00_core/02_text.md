# Text Module

The `aivi.text` module provides core string and character utilities for `Text` and `Char`.
It focuses on predictable, Unicode-aware behavior, and uses `Option`/`Result` instead of
sentinel values like `-1`.

## Overview

```aivi
use aivi.text

greeting = "Hello, AIVI!"

len = length greeting
words = split " " greeting
upper = toUpper greeting
```

## Types

```aivi
type Bytes
type Encoding = Utf8 | Utf16 | Utf32 | Latin1
type TextError = InvalidEncoding Encoding
```

## Core API (v0.1)

### Length and inspection

```aivi
length : Text -> Int
isEmpty : Text -> Bool
```

### Character predicates

```aivi
isDigit : Char -> Bool
isAlpha : Char -> Bool
isAlnum : Char -> Bool
isSpace : Char -> Bool
isUpper : Char -> Bool
isLower : Char -> Bool
```

### Search and comparison

```aivi
contains : Text -> Text -> Bool
startsWith : Text -> Text -> Bool
endsWith : Text -> Text -> Bool
indexOf : Text -> Text -> Option Int
lastIndexOf : Text -> Text -> Option Int
count : Text -> Text -> Int
compare : Text -> Text -> Int
```

Notes:
- `compare` returns `-1`, `0`, or `1` for ordering and is Unicode codepoint order (not locale-aware).
- `indexOf` and `lastIndexOf` return `None` when not found.

### Slicing and splitting

```aivi
slice : Int -> Int -> Text -> Text
split : Text -> Text -> List Text
splitLines : Text -> List Text
chunk : Int -> Text -> List Text
```

Notes:
- `slice start end text` is half-open (`start` inclusive, `end` exclusive) and clamps out-of-range indices.
- `chunk` splits by codepoint count, not bytes.

### Trimming and padding

```aivi
trim : Text -> Text
trimStart : Text -> Text
trimEnd : Text -> Text
padStart : Int -> Text -> Text -> Text
padEnd : Int -> Text -> Text -> Text
```

Notes:
- `padStart width fill text` repeats `fill` as needed and truncates extra.

### Modification

```aivi
replace : Text -> Text -> Text -> Text
replaceAll : Text -> Text -> Text -> Text
remove : Text -> Text -> Text
repeat : Int -> Text -> Text
reverse : Text -> Text
concat : List Text -> Text
```

Notes:
- `replace` changes the first occurrence only.
- `remove` is `replaceAll needle ""`.
- `reverse` is grapheme-aware and may be linear-time with extra allocations.

### Case and normalization

```aivi
toLower : Text -> Text
toUpper : Text -> Text
capitalize : Text -> Text
titleCase : Text -> Text
caseFold : Text -> Text
normalizeNFC : Text -> Text
normalizeNFD : Text -> Text
normalizeNFKC : Text -> Text
normalizeNFKD : Text -> Text
```

### Encoding / decoding

```aivi
toBytes : Encoding -> Text -> Bytes
fromBytes : Encoding -> Bytes -> Result TextError Text
```

### Formatting and conversion

```aivi
toText : Show a => a -> Text
parseInt : Text -> Option Int
parseFloat : Text -> Option Float
```

## Usage Examples

```aivi
use aivi.text

slug = "  Hello World  "
clean = slug |> trim |> toLower |> replaceAll " " "-"

maybePort = parseInt "8080"
```
