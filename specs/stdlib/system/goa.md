# GNOME Online Accounts Module

<!-- quick-info: {"kind":"module","name":"aivi.goa"} -->
`aivi.goa` exposes GNOME Online Accounts (GOA) discovery and OAuth token retrieval via D-Bus (`org.gnome.OnlineAccounts`).
<!-- /quick-info -->

<div class="import-badge">use aivi.goa</div>

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **listAccounts**<br><pre><code>`Effect Text (List GoaAccount)`</code></pre> | Discovers configured GOA accounts from the GNOME session bus. |
| **getAccessToken** key<br><pre><code>`Text -> Effect Text GoaToken`</code></pre> | Fetches OAuth access token metadata for an OAuth2-capable GOA account object path. |
| **accountKey** account<br><pre><code>`GoaAccount -> Text`</code></pre> | Returns the GOA object path key. |
| **filterByKey** key accounts<br><pre><code>`Text -> List GoaAccount -> List GoaAccount`</code></pre> | Filters discovered accounts by object path key. |

## Notes

- On hosts without GNOME Online Accounts or a running session bus, calls return explicit errors.
- This integration reuses existing desktop credentials and avoids duplicate login UX in apps built on AIVI.
