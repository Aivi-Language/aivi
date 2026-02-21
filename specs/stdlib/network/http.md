# HTTP Domain

<!-- quick-info: {"kind":"module","name":"aivi.net.http"} -->
The `Http` domain connects your program to the world. Whether you're fetching data from an API, submitting a form, or scraping a website, this domain provides the standard tools (`get`, `post`, `fetch`) to speak the language of the web reliably.

<!-- /quick-info -->
<div class="import-badge">use aivi.net.http</div>

<<< ../../snippets/from_md/stdlib/network/http/http_domain.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **get** url<br><pre><code>`Url -> Effect HttpError Response`</code></pre> | Performs a GET request and returns a `Response`. |
| **post** url body<br><pre><code>`Url -> Text -> Effect HttpError Response`</code></pre> | Performs a POST request with a text body. |
| **fetch** request<br><pre><code>`Request -> Effect HttpError Response`</code></pre> | Performs a request with custom method, headers, and body. |

## Types

### `Response`

<<< ../../snippets/from_md/stdlib/network/http/response.aivi{aivi}

### `Request`

<<< ../../snippets/from_md/stdlib/network/http/request.aivi{aivi}
