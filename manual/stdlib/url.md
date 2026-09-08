# aivi.url

Typed URLs with explicit parsing.

`aivi.url` gives you a `Url` domain over `Text`. That means you can keep validated URLs as
their own type instead of passing around raw strings everywhere.

The current module publishes the nominal type and declares its intended domain members, but it
does not yet export constructors, parsers, accessors, or raw-carrier conversion to user code.

## Import

```aivi
use aivi.url (
    Url
    UrlError
)
```

## Overview

| Name | Type | Description |
| --- | --- | --- |
| `Url` | domain over `Text` | A validated URL value |
| `UrlError` | `Text` | Parse failure message |
| domain members | not exported | Intended parsing and component-access surface |

## Domain

```aivi
use aivi.url (UrlError)

domain Url over Text = {
    type parse : Text -> Result UrlError Url

    type scheme : Url -> Option Text

    type host : Url -> Option Text

    type port : Url -> Option Int

    type path : Url -> Text

    type query : Url -> Option Text

    type fragment : Url -> Option Text

    type withPath : Url -> Text -> Url

    type withQuery : Url -> Text -> Url
}
```

The domain members — `parse`, `scheme`, `host`, `port`, `path`, `query`, `fragment`,
`withPath`, `withQuery` — are part of the domain's internal implementation and are not
individually importable from user code. Use `Url` as an opaque type supplied by a typed boundary.

## Opaque boundary

Imported domain values do not expose `.carrier`. Consumers can preserve the nominal value without
breaking the abstraction:

```aivi
use aivi.url (Url)

type Url -> Url
func retainUrl = url =>
    url
```

## Error type

```aivi
type UrlError = Text
```

When parsing fails, the module reports a plain text error message.

## Current limits

`aivi.url` is still a small `Text`-backed domain:

- no path-segment composition helpers
- no query-parameter merge/split helpers
- no `withScheme`, `withHost`, `withPort`, or `withFragment`
- no record-patch style field updates over URL parts
- no public text parser or raw-text conversion yet
