# aivi.url

A nominal URL type with a pure parser. Parsing uses the same URL parser as the runtime's networking code and returns its normalized serialization. Relative references without a base URL are rejected.

```aivi
use aivi.url (
    Url
    UrlError
    parse
    toText
)

value endpoint : Result UrlError Url = parse "https://example.org/api"
```

| Function | Type | Behavior |
| --- | --- | --- |
| `parse` | `Text -> Result UrlError Url` | Validate and normalize an absolute URL |
| `toText` | `Url -> Text` | Return its normalized text |

`UrlError` is a `Text` alias. The `Url` domain cannot be interchanged with raw text implicitly. Parsing establishes URL syntax, not network reachability or permission to access a resource. Component access and URL editing are not currently offered.
