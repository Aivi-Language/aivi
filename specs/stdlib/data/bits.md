# Bits

<!-- quick-info: {"kind":"module","name":"aivi.bits","since":"v0.1"} -->

The `aivi.bits` module provides bitwise operations through stdlib functions instead of infix operators. AIVI does not have bitwise operator syntax (`&`, `|`, `^`, `~`, `<<`, `>>`); all bit manipulation is done via this module.

`Bits` is a type alias for `Bytes` — both are backed by the same immutable byte array at runtime. The distinction is semantic: `Bits` signals intent to perform bitwise operations.

<!-- /quick-info -->

`aivi.bits` is the standard library module for working with raw binary data. Use it when you need masks, flags, packed values, protocol headers, or other low-level formats where individual bits matter.

The module uses named functions instead of operator symbols. That makes examples easier to read once you know the pattern: build or load some `Bits`, transform them with functions such as `and`, `xor`, or `shiftLeft`, and convert the result back into the shape you need.

## Import

```aivi
use aivi.bits
```

## Core ideas

- `Bits` is an immutable byte sequence viewed as binary data.
- `BitStream` adds a read position so you can step through bytes in order.
- Bit positions are **MSB-first**: bit `0` is the most significant bit of the first byte.
- Most functions work on whole bytes, but you can also inspect or update individual bits.

## Build and convert bit data

| Function | Type | What it does |
|:---------|:-----|:-------------|
| `fromInt` | `Int -> Bits` | Encodes an `Int` as 8 big-endian bytes. Useful when you want a fixed-width binary representation. |
| `toInt` | `Bits -> Int` | Decodes up to 8 bytes as a big-endian `Int`. Fails if more than 8 bytes are provided. |
| `fromBytes` | `Bytes -> Bits` | Re-labels ordinary bytes as `Bits`. Use this when the same data should now be treated as a bit field. |
| `toBytes` | `Bits -> Bytes` | Re-labels `Bits` back to `Bytes`. |
| `zero` | `Int -> Bits` | Creates `n` zero bytes. Handy for padding or initializing an empty mask. |
| `ones` | `Int -> Bits` | Creates `n` bytes of `0xFF`. Handy when you want an "all bits set" mask. |

## Combine and shift values

| Function | Type | What it does |
|:---------|:-----|:-------------|
| `and` | `Bits -> Bits -> Bits` | Pairwise AND. The shorter input is zero-padded first. Useful for masking out bits you do not want. |
| `or` | `Bits -> Bits -> Bits` | Pairwise OR. Useful for combining flags. |
| `xor` | `Bits -> Bits -> Bits` | Pairwise XOR. Useful for toggling matching bits or comparing bit patterns. |
| `complement` | `Bits -> Bits` | Flips every bit. |
| `shiftLeft` | `Int -> Bits -> Bits` | Shifts all bits left by `n` positions and fills the new space with zeroes. |
| `shiftRight` | `Int -> Bits -> Bits` | Shifts all bits right by `n` positions and fills the new space with zeroes. |

## Read or update a single bit

Bit indexes are MSB-first, so bit `0` is the left-most bit in the first byte.

| Function | Type | What it does |
|:---------|:-----|:-------------|
| `get` | `Int -> Bits -> Bool` | Returns `True` when the bit at the given index is set. Out-of-range indexes return `False`. |
| `set` | `Int -> Bits -> Bits` | Sets the bit at the given index to `1`. |
| `clear` | `Int -> Bits -> Bits` | Sets the bit at the given index to `0`. |
| `toggle` | `Int -> Bits -> Bits` | Flips the bit at the given index. |
| `length` | `Bits -> Int` | Returns the total number of bits (`bytes × 8`). |

## Slice and inspect data

| Function | Type | What it does |
|:---------|:-----|:-------------|
| `slice` | `Int -> Int -> Bits -> Bits` | Extracts bytes from `start` (inclusive) to `end` (exclusive). Indexes are byte offsets, not bit indexes. This is useful when a format stores separate fields in fixed byte ranges. |
| `popCount` | `Bits -> Int` | Counts how many bits are set to `1`. |

## Read structured binary data with `BitStream`

`BitStream` is a simple sequential reader. It is helpful when a binary format is laid out field-by-field and you want each read to return the next chunk plus the updated stream.

Despite the name, the v0.1 `BitStream` API advances in whole bytes. `streamRead 2` reads two bytes, `streamSkip 1` skips one byte, and `streamRemaining` reports the number of unread bytes.

| Function | Type | What it does |
|:---------|:-----|:-------------|
| `streamFromBits` | `Bits -> BitStream` | Creates a stream starting at offset `0`. |
| `streamRead` | `Int -> BitStream -> Result Text (Bits, BitStream)` | Reads `n` bytes and advances the stream. |
| `streamPeek` | `Int -> BitStream -> Result Text Bits` | Reads `n` bytes without changing the current position. |
| `streamSkip` | `Int -> BitStream -> Result Text BitStream` | Advances the stream by `n` bytes without returning the skipped data. |
| `streamRemaining` | `BitStream -> Int` | Returns how many bytes are left unread. |

## Examples

### Extract RGB values from a 24-bit color

<<< ../../snippets/from_md/stdlib/data/bits/block_02.aivi{aivi}


### Parse a simple binary header

This example also imports `aivi.logic` so it can use `chain` and `map` with `Result`.

<<< ../../snippets/from_md/stdlib/data/bits/block_03.aivi{aivi}
