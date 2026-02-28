# Secrets Module

<!-- quick-info: {"kind":"module","name":"aivi.secrets"} -->
`aivi.secrets` provides a generic secret-storage abstraction with typed encrypted blobs.
<!-- /quick-info -->

<div class="import-badge">use aivi.secrets</div>

## Types

### `SecretKeyId`

Alias for `Text`; used as an opaque identifier for stored secrets.

```aivi
SecretKeyId = Text
```

### `SecretError`

Alias for `Text`; used as the error type for secret operations.

```aivi
SecretError = Text
```

### `EncryptedBlob`

A typed encrypted blob with key id, algorithm, and ciphertext.

```aivi
EncryptedBlob = {
  keyId: SecretKeyId
  algorithm: Text
  ciphertext: Bytes
}
```

## Core API (v0.1)

### Storage

| Function | Explanation |
| --- | --- |
| **put** keyId blob<br><pre><code>`SecretKeyId -> EncryptedBlob -> Effect SecretError Unit`</code></pre> | Stores a blob under a key id. |
| **get** keyId<br><pre><code>`SecretKeyId -> Effect SecretError (Option EncryptedBlob)`</code></pre> | Loads a blob by key id. |
| **delete** keyId<br><pre><code>`SecretKeyId -> Effect SecretError Unit`</code></pre> | Removes a blob by key id. |

### Blob constructors and accessors

| Function | Explanation |
| --- | --- |
| **blob** keyId algorithm ciphertext<br><pre><code>`SecretKeyId -> Text -> Bytes -> EncryptedBlob`</code></pre> | Builds a typed encrypted blob value. |
| **blobKeyId** blob<br><pre><code>`EncryptedBlob -> SecretKeyId`</code></pre> | Returns the key id of a blob. |
| **blobAlgorithm** blob<br><pre><code>`EncryptedBlob -> Text`</code></pre> | Returns the algorithm field of a blob. |
| **blobCiphertext** blob<br><pre><code>`EncryptedBlob -> Bytes`</code></pre> | Returns the ciphertext field of a blob. |
| **validateBlob** blob<br><pre><code>`EncryptedBlob -> Bool`</code></pre> | Validates required blob fields (`keyId`, `algorithm`, `ciphertext`). |

## Notes

- Runtime backends may map this API to OS keyring services (e.g. libsecret).
- The v0.1 runtime keeps a local in-process backend while preserving typed surface compatibility.
