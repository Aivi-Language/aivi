# Domains

A score is not just an integer. A player ID is not just an integer either. When you treat them as their raw types, mistakes happen: you pass a score where a player ID was expected, or an ID where a count belongs.

Domains solve this by wrapping a **carrier type** with a **semantic name** and its own operations. The compiler prevents you from mixing them up.

```aivi
domain Score over Int = {
    type fromInt : Int -> Score
    fromInt = value => value
}

domain PlayerId over Int = {
    type playerId : Int -> PlayerId
    playerId = value => value
}

domain Tag over Text = {
    type fromText : Text -> Tag
    fromText = value => value
}

value highScore : Score = fromInt 9000
value currentPlayer : PlayerId = playerId 7
value label : Tag = fromText "featured"
```

You cannot pass a `Score` where a `PlayerId` is expected, even though both are backed by `Int`.

The standard library already ships [`Duration`](/stdlib/duration), [`Url`](/stdlib/url), and [`Path`](/stdlib/path) as built-in domains — you do not need to declare those yourself.

## Declaring a domain

```aivi
domain Score over Int
```

This declares a `Score` domain whose runtime carrier is `Int`.

## Literal suffixes

A domain can define integer suffix constructors:

```aivi
domain Score over Int = {
    suffix pts
    type pts : Int
    pts = n => Score n
}

value highScore : Score = 9000pts
```

Suffixes must be explicit and unambiguous. In current AIVI they must also be at least two characters long.

## Operators and named members

Domains can attach operators and named methods directly under the declaration:

```aivi group=score-operators
domain Score over Int = {
    suffix pts
    type pts : Int
    pts = n => Score n

    type (+) : Score -> Score -> Score
    (+) = left right => left + right

    type toInt : Score -> Int
    toInt = score => score
}
```

That lets you write domain-aware expressions such as:

```aivi group=score-operators
value total : Score = 10pts + 5pts
value raw : Int = toInt total
```

Callable members use the same two-line pattern: annotate the member, then bind it.

```aivi
domain Score over Int = {
    type fromRaw : Int -> Score
    fromRaw = raw => raw
}
```

The body is checked against the carrier view of the domain, while callers still see the nominal signature.

For comparison, prefer `Eq` / `Ord` instances over authored domain operator members. Once a domain implements `Ord.compare`, ordinary `<`, `>`, `<=`, and `>=` work automatically for that domain.

## Receiver-style members

When a member operates on a domain value, name the receiver in its annotation and implementation:

```aivi
use aivi.list (
    head as listHead
    length as listLength
)

type Cell = Cell Int Int

domain Snake over List Cell = {
    type fromCells : List Cell -> Snake
    fromCells = cells => cells

    type head : Snake -> Cell
    head = snake => getOrElse (Cell 0 0) (listHead snake)

    type length : Snake -> Int
    length = snake => listLength snake
}
```

`fromCells` constructs a `Snake` from the carrier. `head` and `length` accept a `Snake` explicitly;
callers may use ordinary function application or dot-call syntax.

## Generic domains

Domains can also be parameterised:

```aivi
domain NonEmpty A over List A
```

This is useful when you want stronger guarantees than the carrier type alone can express.

## Explicit carrier access

A domain does not implicitly coerce to its carrier and does not synthesize a `.carrier` field. Expose
an elimination member when callers genuinely need the underlying representation:

```aivi
domain Score over Int = {
    suffix pts
    type pts : Int
    pts = n => Score n

    type toInt : Score -> Int
    toInt = score => score
}

value raw : Int = toInt 100pts
```

This is useful when you need to pass a domain value to a function that expects the carrier type:

```aivi
type Cell = Cell Int Int

domain Snake over List Cell = {
    type fromCells : List Cell -> Snake
    fromCells = cells => cells

    type cells : Snake -> List Cell
    cells = snake => snake
}

value snake : Snake =
    fromCells [
        Cell 1 2
    ]

value snakeCells : List Cell = cells snake
```

Keeping carrier access named lets the domain preserve invariants and evolve its representation.

## Summary

| Form | Meaning |
| --- | --- |
| `domain Name over Carrier` | Declare a domain |
| `suffix pts` + `type pts : Int` + `pts = n => expr` | Add an integer suffix constructor |
| `type (+) : D -> D -> D` + `(+) = x y => expr` | Add an operator |
| `type member : T` + `member = x => expr` | Add an authored callable member |
| `type member : D -> T` + `member = receiver => expr` | Add an explicit receiver member |
| Explicit elimination member | Convert a domain value to a carrier-shaped value |
