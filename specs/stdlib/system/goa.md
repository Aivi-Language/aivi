# GNOME Online Accounts Module

<!-- quick-info: {"kind":"module","name":"aivi.goa"} -->
`aivi.goa` exposes GNOME Online Accounts (GOA) discovery and OAuth token retrieval via D-Bus (`org.gnome.OnlineAccounts`).
<!-- /quick-info -->

<div class="import-badge">use aivi.goa</div>

## Types

### `GoaAccount`

A discovered GOA account, identified by its D-Bus object path key.

```aivi
GoaAccount = { key: Text }
```

### `GoaToken`

An OAuth access token with expiry information.

```aivi
GoaToken = { token: Text, expiresUnix: Int }
```

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **listAccounts**<br><code>Effect Text (List GoaAccount)</code> | Discovers configured GOA accounts from the GNOME session bus. |
| **getAccessToken** key<br><code>Text -> Effect Text GoaToken</code> | Fetches OAuth access token metadata for an OAuth2-capable GOA account object path. |
| **accountKey** account<br><code>GoaAccount -> Text</code> | Returns the GOA object path key. |
| **filterByKey** key accounts<br><code>Text -> List GoaAccount -> List GoaAccount</code> | Filters discovered accounts by object path key. |

## Notes

- On hosts without GNOME Online Accounts or a running session bus, calls return explicit errors.
- This integration reuses existing desktop credentials and avoids duplicate login UX in apps built on AIVI.
