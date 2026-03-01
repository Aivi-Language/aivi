# Streams Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.streams"} -->
The `Streams` domain provides stream-oriented utilities for processing inbound and outbound data without loading everything into memory.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.streams</div>

<<< ../../snippets/from_md/stdlib/network/streams/streams_domain.aivi{aivi}

## Types

<<< ../../snippets/from_md/stdlib/network/streams/types.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **fromSocket** connection<br><code>Connection -> Stream (List Int)</code> | Creates a stream of byte chunks from the connection. |
| **toSocket** connection stream<br><code>Connection -> Stream (List Int) -> Effect StreamError Unit</code> | Writes byte chunks from `stream` to the connection. |
| **chunks** size stream<br><code>Int -> Stream (List Int) -> Stream (List Int)</code> | Rechunks a byte stream into fixed-size blocks of `size`. |
| **fromList** items<br><code>List A -> Stream A</code> | Creates a finite stream that yields each item from the list in order. Useful for testing and in-memory pipelines. |

## Stream Combinators

| Function | Explanation |
| --- | --- |
| **map** f stream<br><code>(A -> B) -> Stream A -> Stream B</code> | Transforms each stream item. |
| **filter** pred stream<br><code>(A -> Bool) -> Stream A -> Stream A</code> | Keeps items matching `pred`. |
| **take** n stream<br><code>Int -> Stream A -> Stream A</code> | Takes first `n` items then closes. |
| **drop** n stream<br><code>Int -> Stream A -> Stream A</code> | Skips first `n` items. |
| **flatMap** f stream<br><code>(A -> Stream B) -> Stream A -> Stream B</code> | Maps and flattens nested streams. |
| **merge** left right<br><code>Stream A -> Stream A -> Stream A</code> | Interleaves events from both streams. |
| **fold** f seed stream<br><code>(B -> A -> B) -> B -> Stream A -> Effect StreamError B</code> | Consumes stream into one value. |
