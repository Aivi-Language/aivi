# HTTPS Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.https"} -->
The `Https` domain mirrors `Http`, but enforces secure (TLS) connections. It is intended for production use where transport security is required.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.https</div>

<<< ../../snippets/from_md/05_stdlib/03_network/02_https/block_01.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect HttpError Response`</code></pre> | Performs a secure GET request and returns a `Response`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect HttpError Response`</code></pre> | Performs a secure POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect HttpError Response`</code></pre> | Performs a secure request with custom method, headers, and body. |

## Types

Uses the same `Request` and `Response` types as `aivi.net.http`.
