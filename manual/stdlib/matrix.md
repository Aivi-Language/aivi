# aivi.matrix

Rectangular two-dimensional collections.

`aivi.matrix` provides a generic `Matrix A` type for row-major grids addressed by zero-based `x` and `y`
coordinates. It is meant for boards, seat maps, tiles, and other structured grids rather than numeric
linear algebra.

## Import

```aivi
use aivi.matrix (
    Matrix
    MatrixIndex
    MatrixError
    init
    filled
    fromRows
    width
    height
    rows
    row
    at
    replaceAt
    coord
    mapWithIndex
    reduceWithIndex
    coords
    entries
    positionsWhere
    count
    modifyAt
    replaceMany
)
```

## Overview

| Name | Type | Description |
| --- | --- | --- |
| `Matrix A` | opaque generic type | A rectangular row-major grid |
| `MatrixIndex` | opaque coordinate type | The index token used by `coords`, `entries`, `modifyAt`, and `replaceMany` |
| `coord` | `Int -> Int -> MatrixIndex` | Construct a `MatrixIndex` from zero-based `x` and `y` |
| `MatrixError` | sum type | Constructor and validation errors |
| `init` | `Int -> Int -> (Int -> Int -> A) -> Result MatrixError (Matrix A)` | Build a matrix from coordinates |
| `filled` | `Int -> Int -> A -> Result MatrixError (Matrix A)` | Build a matrix filled with one repeated value |
| `fromRows` | `List (List A) -> Result MatrixError (Matrix A)` | Validate an existing nested-list shape |
| `width` | `Matrix A -> Int` | Number of columns |
| `height` | `Matrix A -> Int` | Number of rows |
| `rows` | `Matrix A -> List (List A)` | Expose the row-major carrier |
| `row` | `Matrix A -> Int -> Option (List A)` | Read one zero-based row |
| `at` | `Matrix A -> Int -> Int -> Option A` | Read one cell by `x` then `y` |
| `replaceAt` | `Matrix A -> (Int, Int) -> A -> Option (Matrix A)` | Replace one cell using the tuple-shaped compatibility API |
| `mapWithIndex` | `(Int -> Int -> A -> B) -> Matrix A -> Matrix B` | Map with both `x` and `y` |
| `reduceWithIndex` | `(B -> Int -> Int -> A -> B) -> B -> Matrix A -> B` | Fold with both `x` and `y` |
| `coords` | `Matrix A -> List MatrixIndex` | Enumerate every coordinate in row-major order |
| `entries` | `Matrix A -> List (MatrixIndex, A)` | Enumerate every coordinate/value pair |
| `positionsWhere` | `(A -> Bool) -> Matrix A -> List MatrixIndex` | Collect the coordinates of matching cells |
| `count` | `(A -> Bool) -> Matrix A -> Int` | Count matching cells |
| `modifyAt` | `Matrix A -> MatrixIndex -> (A -> A) -> Option (Matrix A)` | Update one indexed cell with a transform |
| `replaceMany` | `Matrix A -> List (MatrixIndex, A) -> Option (Matrix A)` | Apply several indexed replacements transactionally |

## Error type

```aivi
type MatrixError =
  | NegativeWidth Int
  | NegativeHeight Int
  | RaggedRows Int Int Int
```

- `NegativeWidth w` means `init` or `filled` was called with a negative width.
- `NegativeHeight h` means `init` or `filled` was called with a negative height.
- `RaggedRows rowIndex expected actual` means `fromRows` found a row whose length did not match the
  first row. `rowIndex` is zero-based.

## `init`, `filled`, and `fromRows`

`init width height build` calls `build x y` for every zero-based coordinate in the rectangle.
`filled width height value` uses one repeated value for every cell. Use `fromRows` when you already
have nested lists and want AIVI to validate that every row has the same length.

```aivi
use aivi.matrix (
    Matrix
    MatrixError
    init
    filled
    fromRows
)

type Int -> Int -> Int
func seatNumber = x y =>
    x + y * 100

value built : Result MatrixError (Matrix Int) = init 3 2 seatNumber
value blank : Result MatrixError (Matrix Text) = filled 3 2 "."

value fromExisting : Result MatrixError (Matrix Text) =
    fromRows [
        ["A", "B", "C"],
        ["D", "E", "F"]
    ]
```

## Dimensions and access

`width` and `height` report the current shape. `row` and `at` return `None` when the requested
coordinate is out of bounds.

```aivi
use aivi.matrix (
    Matrix
    MatrixError
    init
    width
    height
    row
    at
)

type Int -> Int -> Int
func cell = x y =>
    x + y * 10

value board : Result MatrixError (Matrix Int) = init 4 3 cell

value corner : Result MatrixError (Option Int) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (at matrix 3 2)
```

## `Functor` and `Foldable`

`Matrix` participates in the ordinary higher-kinded collection surface. When `aivi.matrix` is in
scope, ambient `map` and `reduce` work on `Matrix` values through user-authored `Functor` and
`Foldable` instances.

```aivi
use aivi.matrix (
    Matrix
    MatrixError
    init
    rows
)

type Int -> Int
func double = n =>
    n * 2

type Int -> Int -> Int
func add = total item =>
    total + item

type Int -> Int -> Int
func cell = x y =>
    x + y * 10

value board : Result MatrixError (Matrix Int) = init 3 2 cell

value doubledRows : Result MatrixError (List (List Int)) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (rows (map double matrix))

value total : Result MatrixError Int = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (reduce add 0 matrix)
```

## Indexed traversal and updates

Use `coord` when you need to construct a `MatrixIndex` explicitly. `coords`, `entries`,
`mapWithIndex`, `reduceWithIndex`, `positionsWhere`, `modifyAt`, and `replaceMany` all use the indexed
surface.

```aivi
use aivi.matrix (
    Matrix
    MatrixError
    MatrixIndex
    coord
    init
    rows
    mapWithIndex
    reduceWithIndex
    coords
    entries
    positionsWhere
    count
    modifyAt
    replaceMany
)

type Int -> Int -> Int
func cell = x y =>
    x + y * 10

type Int -> Int -> Int -> Int
func withOffset = x y item =>
    item + x + y

type Int -> Int -> Int -> Int -> Int
func sumWithOffset = total x y item =>
    total + item + x + y

type Int -> Bool
func isEvenValue = n =>
    n % 2 == 0

type Int -> Int
func addHundred = n =>
    n + 100

value board : Result MatrixError (Matrix Int) = init 3 2 cell

value boardCoords : Result MatrixError (List MatrixIndex) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (coords matrix)

value boardEntries : Result MatrixError (List (MatrixIndex, Int)) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (entries matrix)

value offsetRows : Result MatrixError (List (List Int)) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (rows (mapWithIndex withOffset matrix))

value indexedTotal : Result MatrixError Int = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (reduceWithIndex sumWithOffset 0 matrix)

value evenCoords : Result MatrixError (List MatrixIndex) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (positionsWhere isEvenValue matrix)

value evenCount : Result MatrixError Int = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (count isEvenValue matrix)

value modified : Result MatrixError (Option (Matrix Int)) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (modifyAt matrix (coord 1 1) addHundred)

value patched : Result MatrixError (Option (Matrix Int)) = board
 ||> Err error -> Err error
 ||> Ok matrix -> Ok (replaceMany matrix [(coord 0 0, 7), (coord 2 1, 8)])
```

## Notes

- Coordinates are zero-based.
- Matrices are row-major: `rows matrix` returns the carrier as `List (List A)`.
- `replaceAt` keeps the original tuple-shaped `(Int, Int)` input for compatibility; the newer indexed
  helper family uses `MatrixIndex` plus `coord`.
- `init 0 height ...` and `init width 0 ...` are valid and produce empty columns or rows; only
  negative dimensions are rejected.
