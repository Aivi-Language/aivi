# aivi.core.bytes

Byte sequence operations. All functions are runtime intrinsics — import them directly:

```aivi
use aivi.core.bytes (empty, length, get, slice, append, fromText, toText, repeat)
```

---

## Type

`Bytes` is a built-in immutable byte sequence. Individual bytes are `Int` values in the range `0–255`.

---

## Type

### `BytesDecodeError`

```aivi
type BytesDecodeError =
  | InvalidUtf8
```

Returned when `toText` fails because the byte sequence is not valid UTF-8.

---

## Intrinsics

### `empty : Bytes`

The empty byte sequence.

```aivi
use aivi.core.bytes (empty, length)

length empty  // 0
```

### `length : Bytes -> Int`

Number of bytes in the sequence.

```aivi
use aivi.core.bytes (fromText, length)

fromText "hello" |> length  // 5
```

### `get : Int -> Bytes -> Option Int`

Return the byte at a zero-based index as an `Option Int` (0–255). Returns `None` when the index is out of bounds.

```aivi
use aivi.core.bytes (fromText, get)

fromText "ABC" |> get 0  // Some 65
fromText "ABC" |> get 10 // None
```

### `slice : Int -> Int -> Bytes -> Bytes`

Return the sub-sequence from index `from` (inclusive) to `to` (exclusive). Out-of-range indices are clamped.

```aivi
use aivi.core.bytes (fromText, slice)

fromText "hello world" |> slice 6 11  // bytes for "world"
```

### `append : Bytes -> Bytes -> Bytes`

Concatenate two byte sequences.

```aivi
use aivi.core.bytes (fromText, append)

append (fromText "foo") (fromText "bar")  // bytes for "foobar"
```

### `fromText : Text -> Bytes`

UTF-8 encode a `Text` value into `Bytes`.

```aivi
use aivi.core.bytes (fromText, length)

fromText "café" |> length  // 5 (the é is 2 bytes in UTF-8)
```

### `toText : Bytes -> Option Text`

UTF-8 decode `Bytes` into a `Text`. Returns `None` when the bytes are not valid UTF-8.

```aivi
use aivi.core.bytes (fromText, toText)

fromText "hello" |> toText  // Some "hello"
```

### `repeat : Int -> Int -> Bytes`

Create a byte sequence of `count` copies of a single byte value (0–255).

```aivi
use aivi.core.bytes (repeat)

repeat 0 4   // four zero bytes: [0, 0, 0, 0]
repeat 255 2 // [255, 255]
```

---

## Real-world example

```aivi
use aivi.core.bytes (fromText, toText, length, slice, append)
use aivi.fs (readBytes, writeBytes)

fun prependHeader:Task Text Unit path:Text header:Text =>
    let headerBytes = fromText header in
    readBytes path
     |> map (append headerBytes)
     |> andThen (writeBytes path)
```

::: tip
Use `fromText`/`toText` for UTF-8 text round-trips. For binary data (images, archives, network frames) work directly with `Bytes` using `slice`, `append`, and `get`.
:::
