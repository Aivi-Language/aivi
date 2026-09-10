# aivi.core.bytes

`aivi.core.bytes` provides immutable byte sequences. Its operations are synchronous and pure. Indexes and lengths count bytes rather than Unicode characters.

## Type class operations

`Bytes` implements `Semigroup`, `Monoid`, and `Default` through this module. Ambient `append`
concatenates bytes; `empty` and `default` produce empty bytes. The module's explicitly imported
`append` and `empty` helpers have the same behavior.

## API

| Export | Type | Behavior |
| --- | --- | --- |
| `empty` | `Bytes` | The empty byte sequence |
| `length` | `Bytes -> Int` | Count bytes |
| `get` | `Int -> Bytes -> Option Int` | Read a byte from zero through 255 |
| `slice` | `Int -> Int -> Bytes -> Bytes` | Return the half-open byte range `[from, to)` with bounds clamped |
| `append` | `Bytes -> Bytes -> Bytes` | Concatenate two byte sequences |
| `fromText` | `Text -> Bytes` | Encode UTF-8 |
| `toText` | `Bytes -> Option Text` | Decode UTF-8, returning `None` for invalid data |
| `repeat` | `Int -> Int -> Bytes` | Repeat a byte value a requested number of times |
| `isEmpty` | `Bytes -> Bool` | Test whether the sequence has zero bytes |
| `nonEmpty` | `Bytes -> Bool` | Test whether the sequence has at least one byte |
| `concat` | `List Bytes -> Bytes` | Concatenate a list of byte sequences |

`repeat` requires a byte value in the range zero through 255. A nonpositive repeat count returns `empty`.

```aivi
use aivi.core.bytes (
    append
    fromText
    toText
)

value payload : Bytes = append (fromText "hello") (fromText "!")
value decoded : Option Text = toText payload
```
