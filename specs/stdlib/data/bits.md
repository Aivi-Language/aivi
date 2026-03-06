# Bits

<!-- quick-info: {"module": "aivi.bits", "since": "v0.1"} -->

The `aivi.bits` module provides bitwise operations through stdlib functions instead of infix operators. AIVI does not have bitwise operator syntax (`&`, `|`, `^`, `~`, `<<`, `>>`); all bit manipulation is done via this module.

`Bits` is a type alias for `Bytes` — both are backed by the same immutable byte array at runtime. The distinction is semantic: `Bits` signals intent to perform bitwise operations.

<!-- /quick-info -->

## 1 Imports

```aivi
use aivi.bits
```

## 2 Types

| Type | Definition | Description |
|:-----|:-----------|:------------|
| `Bits` | `Bytes` | Alias — immutable byte array viewed as a bit vector (MSB-first). |
| `BitStream` | `{ data: Bits, offset: Int }` | Position-tracking wrapper for sequential byte-aligned reads. |

## 3 Construction & Conversion

| Function | Type | Description |
|:---------|:-----|:------------|
| `fromInt` | `Int -> Bits` | 8-byte big-endian encoding of a 64-bit integer. |
| `toInt` | `Bits -> Int` | Decode first ≤ 8 bytes as a big-endian `Int`. Fails if length > 8. |
| `fromBytes` | `Bytes -> Bits` | Identity (semantic alias). |
| `toBytes` | `Bits -> Bytes` | Identity (semantic alias). |
| `zero` | `Int -> Bits` | `n` zero bytes. |
| `ones` | `Int -> Bits` | `n` bytes of `0xFF`. |

## 4 Bitwise Operations

| Function | Type | Description |
|:---------|:-----|:------------|
| `and` | `Bits -> Bits -> Bits` | Pairwise AND. Shorter operand is zero-padded. |
| `or` | `Bits -> Bits -> Bits` | Pairwise OR. |
| `xor` | `Bits -> Bits -> Bits` | Pairwise XOR. |
| `complement` | `Bits -> Bits` | Bitwise NOT (flip every bit). |
| `shiftLeft` | `Int -> Bits -> Bits` | Shift all bits left by `n` positions; vacated bits are zero. |
| `shiftRight` | `Int -> Bits -> Bits` | Shift all bits right by `n` positions; vacated bits are zero. |

## 5 Individual Bit Access

Bit indices are MSB-first (bit 0 is the most significant bit of the first byte).

| Function | Type | Description |
|:---------|:-----|:------------|
| `get` | `Int -> Bits -> Bool` | `True` if bit at index is set. Out-of-range returns `False`. |
| `set` | `Int -> Bits -> Bits` | Set bit to 1. |
| `clear` | `Int -> Bits -> Bits` | Set bit to 0. |
| `toggle` | `Int -> Bits -> Bits` | Flip bit. |
| `length` | `Bits -> Int` | Total bit count (`bytes × 8`). |

## 6 Slicing & Inspection

| Function | Type | Description |
|:---------|:-----|:------------|
| `slice` | `Int -> Int -> Bits -> Bits` | Extract bytes from `start` (inclusive) to `end` (exclusive). |
| `popCount` | `Bits -> Int` | Number of set bits. |

## 7 BitStream (Sequential Reader)

`BitStream` threads a byte offset through a sequence of reads — useful for parsing binary protocols, file headers, or wire formats.

| Function | Type | Description |
|:---------|:-----|:------------|
| `streamFromBits` | `Bits -> BitStream` | Create a stream at offset 0. |
| `streamRead` | `Int -> BitStream -> Result Text (Bits, BitStream)` | Read `n` bytes, advance offset. |
| `streamPeek` | `Int -> BitStream -> Result Text Bits` | Read `n` bytes without advancing. |
| `streamSkip` | `Int -> BitStream -> Result Text BitStream` | Advance offset by `n` bytes. |
| `streamRemaining` | `BitStream -> Int` | Remaining bytes. |

## 8 Examples

### Extract RGB from a 24-bit color

```aivi
use aivi.bits

extractRgb : Bits -> { r: Int, g: Int, b: Int }
extractRgb = color =>
  mask = fromInt 0xFF
  { r: color |> shiftRight 16 |> and mask |> toInt
  , g: color |> shiftRight 8  |> and mask |> toInt
  , b: color |> and mask |> toInt
  }
```

### Parse a simple protocol header

```aivi
use aivi.bits
use aivi.logic

parseHeader : Bits -> Result Text { version: Int, flags: Bits, payload: Bits }
parseHeader = raw =>
  stream = streamFromBits raw
  stream |> streamRead 1 |> chain (vb, s1) =>
  s1    |> streamRead 2 |> chain (flags, s2) =>
  s2    |> streamRead 4 |> map (payload, _) =>
  { version: toInt vb, flags: flags, payload: payload }
```
