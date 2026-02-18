pub const MODULE_NAME: &str = "aivi.text";

pub const SOURCE: &str = r#"
@no_prelude
module aivi.text
export Bytes, Encoding, TextError
export Utf8, Utf16, Utf32, Latin1
export ToText
export length, isEmpty, isDigit, isAlpha, isAlnum, isSpace, isUpper, isLower
export contains, startsWith, endsWith, indexOf, lastIndexOf, count, compare
export slice, split, splitLines, chunk
export trim, trimStart, trimEnd, padStart, padEnd
export replace, replaceAll, remove, repeat, reverse, concat
export toLower, toUpper, capitalize, titleCase, caseFold
export normalizeNFC, normalizeNFD, normalizeNFKC, normalizeNFKD
export toBytes, fromBytes, debugText, parseInt, parseFloat

use aivi

Encoding = Utf8 | Utf16 | Utf32 | Latin1
TextError = InvalidEncoding Encoding

// ------------------------------------------------------------
// Expected-type coercions: `A` -> `Text` via instances
// ------------------------------------------------------------

class ToText A = {
  toText: A -> Text
}

// Any record (open by default) can be coerced to `Text` using the runtime
// `text.toText` representation. We pattern-match on `{}` so this clause fails
// for non-record values, allowing other `toText` clauses to apply.
instance ToText {} = {
  toText: value =>
    value match
      | {} => text.toText value
}

length : Text -> Int
length = value => text.length value

isEmpty : Text -> Bool
isEmpty = value => text.isEmpty value

isDigit : Char -> Bool
isDigit = value => text.isDigit value

isAlpha : Char -> Bool
isAlpha = value => text.isAlpha value

isAlnum : Char -> Bool
isAlnum = value => text.isAlnum value

isSpace : Char -> Bool
isSpace = value => text.isSpace value

isUpper : Char -> Bool
isUpper = value => text.isUpper value

isLower : Char -> Bool
isLower = value => text.isLower value

contains : Text -> Text -> Bool
contains = needle haystack => text.contains haystack needle

startsWith : Text -> Text -> Bool
startsWith = prefix value => text.startsWith value prefix

endsWith : Text -> Text -> Bool
endsWith = suffix value => text.endsWith value suffix

indexOf : Text -> Text -> Option Int
indexOf = needle haystack => text.indexOf haystack needle

lastIndexOf : Text -> Text -> Option Int
lastIndexOf = needle haystack => text.lastIndexOf haystack needle

count : Text -> Text -> Int
count = needle haystack => text.count haystack needle

compare : Text -> Text -> Int
compare = left right => text.compare left right

slice : Int -> Int -> Text -> Text
slice = start end value => text.slice start end value

split : Text -> Text -> List Text
split = sep value => text.split sep value

splitLines : Text -> List Text
splitLines = value => text.splitLines value

chunk : Int -> Text -> List Text
chunk = size value => text.chunk size value

trim : Text -> Text
trim = value => text.trim value

trimStart : Text -> Text
trimStart = value => text.trimStart value

trimEnd : Text -> Text
trimEnd = value => text.trimEnd value

padStart : Int -> Text -> Text -> Text
padStart = width fill value => text.padStart width fill value

padEnd : Int -> Text -> Text -> Text
padEnd = width fill value => text.padEnd width fill value

replace : Text -> Text -> Text -> Text
replace = needle replacement value => text.replace value needle replacement

replaceAll : Text -> Text -> Text -> Text
replaceAll = needle replacement value => text.replaceAll value needle replacement

remove : Text -> Text -> Text
remove = needle value => text.remove value needle

repeat : Int -> Text -> Text
repeat = count value => text.repeat count value

reverse : Text -> Text
reverse = value => text.reverse value

concat : List Text -> Text
concat = values => text.concat values

toLower : Text -> Text
toLower = value => text.toLower value

toUpper : Text -> Text
toUpper = value => text.toUpper value

capitalize : Text -> Text
capitalize = value => text.capitalize value

titleCase : Text -> Text
titleCase = value => text.titleCase value

caseFold : Text -> Text
caseFold = value => text.caseFold value

normalizeNFC : Text -> Text
normalizeNFC = value => text.normalizeNFC value

normalizeNFD : Text -> Text
normalizeNFD = value => text.normalizeNFD value

normalizeNFKC : Text -> Text
normalizeNFKC = value => text.normalizeNFKC value

normalizeNFKD : Text -> Text
normalizeNFKD = value => text.normalizeNFKD value

toBytes : Encoding -> Text -> Bytes
toBytes = encoding value => text.toBytes encoding value

fromBytes : Encoding -> Bytes -> Result TextError Text
fromBytes = encoding value => text.fromBytes encoding value

debugText : A -> Text
debugText = value => text.toText value

parseInt : Text -> Option Int
parseInt = value => text.parseInt value

parseFloat : Text -> Option Float
parseFloat = value => text.parseFloat value
"#;
