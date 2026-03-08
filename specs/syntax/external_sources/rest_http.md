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

<<< ../../snippets/from_md/syntax/external_sources/rest_http/block_01.aivi{aivi}


## Example with request options

<<< ../../snippets/from_md/syntax/external_sources/rest_http/block_02.aivi{aivi}


## How request options relate to source composition

HTTP-specific request fields still describe wire-level behavior for that connector.

Cross-source policies such as:

- `source.timeout`
- `source.retry`
- `source.cache`
- `source.provenance`

apply at the source-composition layer and work consistently across REST, file, environment, and database sources.

See [Source Composition](composition.md) for the reusable execution model.
