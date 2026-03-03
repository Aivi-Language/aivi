pub const MODULE_NAME: &str = "aivi.bits";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.bits
export Bits, BitStream
export fromInt, toInt, fromBytes, toBytes, zero, ones
export and, or, xor, complement
export shiftLeft, shiftRight
export get, set, clear, toggle, length
export slice, popCount
export streamFromBits, streamRead, streamPeek, streamSkip, streamRemaining

use aivi

Bits = Bytes

fromInt : Int -> Bits
fromInt = n => bits.fromInt n

toInt : Bits -> Int
toInt = b => bits.toInt b

fromBytes : Bytes -> Bits
fromBytes = b => bits.fromBytes b

toBytes : Bits -> Bytes
toBytes = b => bits.toBytes b

zero : Int -> Bits
zero = byteCount => bits.zero byteCount

ones : Int -> Bits
ones = byteCount => bits.ones byteCount

and : Bits -> Bits -> Bits
and = a b => bits.and a b

or : Bits -> Bits -> Bits
or = a b => bits.or a b

xor : Bits -> Bits -> Bits
xor = a b => bits.xor a b

complement : Bits -> Bits
complement = b => bits.complement b

shiftLeft : Int -> Bits -> Bits
shiftLeft = n b => bits.shiftLeft n b

shiftRight : Int -> Bits -> Bits
shiftRight = n b => bits.shiftRight n b

get : Int -> Bits -> Bool
get = idx b => bits.get idx b

set : Int -> Bits -> Bits
set = idx b => bits.set idx b

clear : Int -> Bits -> Bits
clear = idx b => bits.clear idx b

toggle : Int -> Bits -> Bits
toggle = idx b => bits.toggle idx b

length : Bits -> Int
length = b => bits.length b

slice : Int -> Int -> Bits -> Bits
slice = start end b => bits.slice start end b

popCount : Bits -> Int
popCount = b => bits.popCount b

// BitStream — a position-tracking wrapper for sequential bit reading.
// Represented as a record { data: Bits, offset: Int } (offset in bytes).
BitStream = { data: Bits, offset: Int }

streamFromBits : Bits -> BitStream
streamFromBits = data => { data: data, offset: 0 }

streamRead : Int -> BitStream -> Result Text (Bits, BitStream)
streamRead = byteCount stream =>
  (stream.offset + byteCount > bits.length stream.data / 8) match
    | True => Err "BitStream: read past end"
    | False =>
        Ok (bits.slice stream.offset (stream.offset + byteCount) stream.data, stream <| { offset: stream.offset + byteCount })

streamPeek : Int -> BitStream -> Result Text Bits
streamPeek = byteCount stream =>
  (stream.offset + byteCount > bits.length stream.data / 8) match
    | True => Err "BitStream: peek past end"
    | False => Ok (bits.slice stream.offset (stream.offset + byteCount) stream.data)

streamSkip : Int -> BitStream -> Result Text BitStream
streamSkip = byteCount stream =>
  (stream.offset + byteCount > bits.length stream.data / 8) match
    | True => Err "BitStream: skip past end"
    | False => Ok (stream <| { offset: stream.offset + byteCount })

streamRemaining : BitStream -> Int
streamRemaining = stream => bits.length stream.data / 8 - stream.offset
"#;
