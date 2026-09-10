# aivi.auth

Types for OAuth 2.0 PKCE sign-in flows.

This module gives you the records and tagged unions used to model a browser-based sign-in flow plus
the `AuthSource` handle marker for `@source auth`.

## Import

```aivi
use aivi.auth (
    AuthSource
    AuthTask
    PkceConfig
    PkceToken
    PkceError
    UserCancelled
    NetworkError
    InvalidResponse
    PkceTimeout
    PkceState
    PkceIdle
    PkceInProgress
    PkceComplete
    PkceFailed
)
```

## Overview

| Type | Purpose |
|------|---------|
| `AuthSource` | Handle annotation for `@source auth` |
| `AuthTask A` | Background auth work returning `A` |
| `PkceConfig` | Settings for one OAuth provider |
| `PkceToken` | Access token details returned after sign-in |
| `PkceError` | Why the flow failed or stopped |
| `PkceState` | High-level state of the sign-in process |

---

## Capability handle

```aivi
use aivi.auth (AuthSource)

@source auth
signal auth : AuthSource
```

Current canonical handle members:

| Member | Type | Description |
| --- | --- | --- |
| `auth.pkce config` | `AuthTask PkceToken` | Run browser-based PKCE flow, wait for loopback callback, exchange code for tokens |
| `auth.refresh config refreshToken` | `AuthTask PkceToken` | Exchange one refresh token for a fresh access token |

`auth.pkce` launches the external browser with `xdg-open` and listens on
`http://127.0.0.1:{redirectPort}/callback`.

Construct `Url` fields with `aivi.url.parse`, handle its `Err` branch, then pass the completed
`PkceConfig` to `auth.pkce` or `auth.refresh`.

---

## `PkceConfig`

```aivi
type PkceConfig = {
    clientId: Text,
    authEndpoint: Url,
    tokenEndpoint: Url,
    scopes: List Text,
    redirectPort: Int
}
```

Configuration for one PKCE login flow.

- `clientId` — the OAuth client ID issued by the provider
- `authEndpoint` — the browser URL where the user grants access
- `tokenEndpoint` — the URL used to exchange the code for a token
- `scopes` — the permissions your app is asking for
- `redirectPort` — the local port that receives the browser callback

```aivi
use aivi.auth (PkceConfig)

use aivi.url (Url)

type Url -> Url -> PkceConfig
func githubPkce = authEndpoint tokenEndpoint =>
    {
        clientId: "desktop-client",
        authEndpoint: authEndpoint,
        tokenEndpoint: tokenEndpoint,
        scopes: ["read:user", "user:email"],
        redirectPort: 43123
    }
```

---

## `PkceToken`

```aivi
type PkceToken = {
    accessToken: Text,
    refreshToken: Option Text,
    expiresAt: Option Int
}
```

Token data returned after a successful exchange.

- `accessToken` — the token you send with authenticated requests
- `refreshToken` — an optional token for getting a fresh access token later
- `expiresAt` — an optional expiration time supplied by the provider

---

## `PkceError`

```aivi
type PkceError =
  | UserCancelled
  | NetworkError Text
  | InvalidResponse Text
  | PkceTimeout
```

Reasons a PKCE flow can stop:

- `UserCancelled` — the person closed or cancelled the sign-in flow
- `NetworkError Text` — the browser callback or token exchange failed because of networking
- `InvalidResponse Text` — the provider replied, but the data could not be used
- `PkceTimeout` — the flow took too long and was abandoned

```aivi
use aivi.auth (
    PkceError
    UserCancelled
    NetworkError
    InvalidResponse
    PkceTimeout
)

type PkceError -> Text
func describePkceError = error => error
 ||> UserCancelled       -> "sign-in cancelled"
 ||> NetworkError msg    -> "network error: {msg}"
 ||> InvalidResponse msg -> "invalid response: {msg}"
 ||> PkceTimeout         -> "sign-in timed out"
```

---

## `PkceState`

```aivi
use aivi.auth (
    PkceError
    PkceToken
)

type PkceState =
  | PkceIdle
  | PkceInProgress
  | PkceComplete PkceToken
  | PkceFailed PkceError
```

A simple state machine for the whole sign-in process.

- `PkceIdle` — nothing has started yet
- `PkceInProgress` — the browser flow is running
- `PkceComplete token` — sign-in finished and produced a token
- `PkceFailed error` — sign-in ended with a `PkceError`

```aivi
use aivi.auth (
    PkceState
    PkceIdle
    PkceInProgress
    PkceComplete
    PkceFailed
)

type PkceState -> Bool
func signedIn = state => state
 ||> PkceComplete _ -> True
 ||> _              -> False
```

## Remaining public constructors and aliases

`AuthTask A` is `Task PkceError A`. `NetworkError message` and `InvalidResponse message` preserve
provider failure details. `PkceComplete token` and `PkceFailed error` are the terminal `PkceState`
constructors.
