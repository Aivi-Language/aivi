# REST / HTTP Sources

<!-- quick-info: {"kind":"topic","name":"rest http sources"} -->

AIVI supports both low-level HTTP (`http`/`https`) and a REST-oriented facade (`rest`) as typed `Source` boundaries.

<!-- /quick-info -->

## APIs (v0.1)

- `http.get/post/fetch`
- `https.get/post/fetch`
- `rest.get/post/fetch`

`rest.fetch` supports request-level options (timeouts, retries, bearer auth, strict status handling).

## Composition alignment (Phase 3)

REST request fields such as `timeoutMs`, `retryCount`, `bearerToken`, and `strictStatus` are still part of the connector-facing request shape.

Phase 3 source composition adds a **connector-independent** policy layer on top of those request fields:

- `source.timeout` and `source.retry` define reusable acquisition policy for any source kind,
- `source.cache` and `source.provenance` apply uniformly across REST, file, env, and database sources,
- connector-specific request options continue to control HTTP-specific wire behavior.

See [Source Composition](composition.md) for the cross-source execution model.

## Capability mapping (Phase 1 surface)

Loading a REST or HTTP source requires `network.http` (or the broader `network` family shorthand).

## Example

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
usersSource = rest.get ~u(https://api.example.com/users)

do Effect {
  users <- load usersSource
  pure users
}
```
