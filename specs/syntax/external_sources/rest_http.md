# REST / HTTP Sources

<!-- quick-info: {"kind":"topic","name":"rest http sources"} -->

AIVI supports both low-level HTTP (`http` / `https`) and a REST-oriented facade (`rest`) as typed `Source` boundaries.

<!-- /quick-info -->

REST and HTTP sources let you describe network reads as reusable `Source` values.

Use them when your program needs to:

- call a web API,
- send headers, request bodies, or bearer authentication,
- decide whether you want the raw HTTP envelope or a decoded application value at the boundary.

## API surface

| Family | Common entries | What `load` gives back | Best fit |
| --- | --- | --- | --- |
| `http` | `http.get`, `http.post`, `http.fetch` | `Result Error Response` | You want raw status, headers, and body text. |
| `https` | `https.get`, `https.post`, `https.fetch` | `Result Error Response` | Same as `http`, but for HTTPS-only endpoints. |
| `rest` | `rest.get`, `rest.post`, `rest.fetch` | `A` | You want the response body decoded into the expected type. |

`rest.fetch` extends the usual request shape with request-level options such as timeouts, retries, bearer authentication, and strict status handling.

If you want one-off `Effect` helpers instead of reusable source values, see [`aivi.net.http`](../../stdlib/network/http.md) and [`aivi.rest`](../../stdlib/network/rest.md).

## Choosing between `http`, `https`, and `rest`

- use `http` when the raw HTTP envelope matters,
- use `https` when you want that same raw envelope but only for HTTPS endpoints,
- use `rest` when the response body should decode into a typed AIVI value at the boundary.

All three fit into the same `Source` model and become effects only when you call `load`.

## Capability mapping

Loading any REST or HTTP source requires `network.http` (or the broader `network` family shorthand).

## Decoded REST example

<<< ../../snippets/from_md/syntax/external_sources/rest_http/block_01.aivi{aivi}


`load` turns the reusable source into an effect. Here the expected result type, `List User`, tells the REST boundary what shape to decode from the response body.

## Raw HTTP example

<<< ../../snippets/from_md/syntax/external_sources/rest_http/block_02.aivi{aivi}


Use `http.*` or `https.*` when you want to inspect `status`, `headers`, and `body` yourself instead of decoding the body immediately.

## Example with request options

<<< ../../snippets/from_md/syntax/external_sources/rest_http/block_03.aivi{aivi}


Here `apiToken` is just a normal `Text` binding supplied elsewhere in your program. The extra request fields mean:

- `timeoutMs`: fail the request if one attempt takes too long,
- `retryCount`: retry transient request failures,
- `bearerToken`: add `Authorization: Bearer ...`,
- `strictStatus`: treat non-2xx responses as failures instead of normal REST results.

## Failure modes and diagnostics

Loading network sources can fail in different ways:

- `http.*` and `https.*` return transport failures as `Err { message }`; successful calls stay in `Ok { status, headers, body }`, including non-2xx statuses unless you interpret them yourself.
- `rest.*` uses the same transport boundary and then decodes the response body into the expected type.
- `strictStatus: Some True` upgrades non-2xx responses into failures at the REST boundary.
- if the response body does not match the expected type, the loader should report a source parse error that points at the mismatched path.

See [External Sources](../external_sources.md) for the shared `SourceError` model.

## How request options relate to source composition

HTTP-specific request fields still describe wire-level behavior for that connector.

Cross-source policies such as:

- `source.timeout`
- `source.retry`
- `source.cache`
- `source.provenance`

apply at the source-composition layer and work consistently across REST, file, environment, and database sources.

See [Source Composition](composition.md) for the reusable execution model.
