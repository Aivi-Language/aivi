# Streams Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.streams"} -->
The `Streams` domain helps you process data piece by piece instead of loading it all at once. It is useful for network I/O, large payloads, and any pipeline where data naturally arrives in chunks.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.streams</div>

<<< ../../snippets/from_md/stdlib/network/streams/streams_domain.aivi{aivi}

## What this module is for

`aivi.net.streams` is about incremental processing. Instead of waiting for one large value, you work with a `Stream A` that can produce many values over time.

That is especially helpful when:

- reading bytes from a socket
- writing output in chunks
- transforming a large sequence without keeping everything in memory
- building reusable data pipelines

## Core idea

A `Stream A` represents a sequence of values of type `A`. For example:

- `Stream (List Int)` can represent chunks of bytes from a socket
- `Stream Text` could represent lines of text
- `Stream Request` could represent incoming events in a higher-level system

The module includes both conversion functions for sockets and combinators for transforming streams.

## Types

<<< ../../snippets/from_md/stdlib/network/streams/types.aivi{aivi}

- `Stream A` is the stream itself.
- `StreamError` is the error type used by stream operations that perform effects.

## Common examples

Create a stream from a list and transform it:

```aivi
use aivi.net.streams
use aivi.logic

// fromList is handy for examples, tests, and in-memory pipelines.
nums    = fromList [1, 2, 3, 4, 5]
evens   = filter (x => x % 2 == 0) nums
doubled = map (x => x * 2) evens
```

Read bytes from a socket, then regroup them into fixed-size chunks:

```aivi
use aivi.net.streams

prepareInput = connection =>
  fromSocket connection
    |> chunks 1024 // emit blocks of up to 1024 bytes
```

## Functions

| Function | Explanation |
| --- | --- |
| **fromSocket** connection<br><code>Connection -> Stream (List Int)</code> | Creates a stream of byte chunks coming from a socket connection. |
| **toSocket** connection stream<br><code>Connection -> Stream (List Int) -> Effect StreamError Unit</code> | Writes each byte chunk from `stream` to the socket connection. |
| **chunks** size stream<br><code>Int -> Stream (List Int) -> Stream (List Int)</code> | Re-groups a byte stream into blocks of roughly `size` bytes, which can simplify downstream processing. |
| **fromList** items<br><code>List A -> Stream A</code> | Creates a finite stream from an in-memory list. This is especially useful for testing and examples. |

## Stream combinators

These functions let you build pipelines without needing to consume the stream immediately.

| Function | Explanation |
| --- | --- |
| **map** f stream<br><code>(A -> B) -> Stream A -> Stream B</code> | Transforms each item in the stream. |
| **filter** pred stream<br><code>(A -> Bool) -> Stream A -> Stream A</code> | Keeps only the items that satisfy `pred`. |
| **take** n stream<br><code>Int -> Stream A -> Stream A</code> | Keeps the first `n` items, then closes the resulting stream. |
| **drop** n stream<br><code>Int -> Stream A -> Stream A</code> | Skips the first `n` items and yields the rest. |
| **flatMap** f stream<br><code>(A -> Stream B) -> Stream A -> Stream B</code> | Turns each item into a stream, then flattens the results into one stream. |
| **merge** left right<br><code>Stream A -> Stream A -> Stream A</code> | Interleaves values from two streams into one combined stream. |
| **fold** f seed stream<br><code>(B -> A -> B) -> B -> Stream A -> Effect StreamError B</code> | Consumes the stream and combines all items into one final value. |

## HKT instances

`Stream A` implements the following type-class instances from `aivi.logic`:

| Class | Method | Behaviour |
| --- | --- | --- |
| **Functor** | `map f stream` | Lazily transforms each item and returns a new stream. |
| **Filterable** | `filter pred stream` | Lazily keeps matching items and returns a new stream. |

> **Why no Foldable?** `fold` consumes the stream and may perform I/O, so it returns `Effect StreamError B` instead of a pure `B`. That means it does not match the pure `Foldable` class shape.

## Practical guidance

- Use `fromList` when you want a simple stream for tests or examples.
- Use `fromSocket` and `toSocket` when moving bytes through a network connection.
- Use `map`, `filter`, and `flatMap` to build transformation pipelines.
- Use `fold` when you are ready to consume the stream and produce one final result.
