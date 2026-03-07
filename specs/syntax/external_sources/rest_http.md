# REST / HTTP Sources

<!-- quick-info: {"kind":"topic","name":"rest http sources"} -->

AIVI supports both low-level HTTP (`http`/`https`) and a REST-oriented facade (`rest`) as typed `Source` boundaries.

<!-- /quick-info -->

HTTP sources are for reading typed data from web services.

Use them when your program needs to:

- fetch JSON from an API,
- submit requests with headers or authentication,
- treat network responses as typed values at the boundary.

## APIs

- `http.get/post/fetch`
- `https.get/post/fetch`
- `rest.get/post/fetch`

`rest.fetch` supports request-level options such as timeouts, retries, bearer authentication, and strict status handling.

## Choosing between `http` and `rest`

- use `http` or `https` when you want a lower-level HTTP boundary
- use `rest` when you want a more REST-oriented, typed API surface

Both fit into the same `Source` model and are loaded with `load`.

## Capability mapping

Loading a REST or HTTP source requires `network.http` (or the broader `network` family shorthand).

## Simple example

```aivi
User = { name: Text, age: Int, gender: Text }

usersSource : Source RestApi (List User)
usersSource = rest.get ~u(https://api.example.com/users)  -- decode the response body as a list of User

do Effect {
  users <- load usersSource
  pure users
}
```

## Example with request options

```aivi
Request = {
  method: Text
  url: Url
  headers: List { name: Text, value: Text }
  body: Option Text
  timeoutMs: Option Int
  retryCount: Option Int
  bearerToken: Option Text
  strictStatus: Option Bool
}

User = { name: Text, age: Int, gender: Text }

usersSource : Source RestApi (List User)
usersSource =
  rest.fetch {
    method: "GET"
    url: ~u(https://api.example.com/users)
    headers: []
    body: None
    timeoutMs: Some 5_000
    retryCount: Some 2
    bearerToken: Some apiToken       -- attach bearer auth at the request boundary
    strictStatus: Some True          -- treat non-2xx responses as failures
  }
```

## How request options relate to source composition

HTTP-specific request fields still describe wire-level behavior for that connector.

Cross-source policies such as:

- `source.timeout`
- `source.retry`
- `source.cache`
- `source.provenance`

apply at the source-composition layer and work consistently across REST, file, environment, and database sources.

See [Source Composition](composition.md) for the reusable execution model.
