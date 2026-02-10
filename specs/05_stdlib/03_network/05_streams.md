# Streams Domain

The `Streams` domain provides stream-oriented utilities for processing inbound and outbound data without loading everything into memory.

```aivi
use aivi.std.network.streams
```

## Types

```aivi
type StreamError = { message: Text }
```

## Functions

### `fromSocket`

```aivi
fromSocket : Connection -> Stream (List Int)
```

Creates a byte stream from a socket connection.

### `toSocket`

```aivi
toSocket : Connection -> Stream (List Int) -> Effect StreamError Unit
```

Writes a byte stream to a socket connection.

### `chunks`

```aivi
chunks : Int -> Stream (List Int) -> Stream (List Int)
```

Rechunks a stream into fixed-size blocks.
