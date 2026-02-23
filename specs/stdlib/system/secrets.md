# Secrets Module

<!-- quick-info: {"kind":"module","name":"aivi.secrets"} -->
`aivi.secrets` provides a generic secret-storage abstraction with typed encrypted blobs.
<!-- /quick-info -->

<div class="import-badge">use aivi.secrets</div>

## Core API (v0.1)

| Function | Explanation |
| --- | --- |
| **put** keyId blob<br><pre><code>`Text -> EncryptedBlob -> Effect Text Unit`</code></pre> | Stores a blob under a key id. |
| **get** keyId<br><pre><code>`Text -> Effect Text (Option EncryptedBlob)`</code></pre> | Loads a blob by key id. |
| **delete** keyId<br><pre><code>`Text -> Effect Text Unit`</code></pre> | Removes a blob by key id. |
| **blob** keyId algorithm ciphertext<br><pre><code>`Text -> Text -> Bytes -> EncryptedBlob`</code></pre> | Builds a typed encrypted blob value. |
| **validateBlob** blob<br><pre><code>`EncryptedBlob -> Bool`</code></pre> | Validates required blob fields (`keyId`, `algorithm`, `ciphertext`). |

## Notes

- Runtime backends may map this API to OS keyring services (e.g. libsecret).
- The v0.1 runtime keeps a local in-process backend while preserving typed surface compatibility.
