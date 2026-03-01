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
| **put** keyId blob<br><code>SecretKeyId -> EncryptedBlob -> Effect SecretError Unit</code> | Stores a blob under a key id. |
| **get** keyId<br><code>SecretKeyId -> Effect SecretError (Option EncryptedBlob)</code> | Loads a blob by key id. |
| **delete** keyId<br><code>SecretKeyId -> Effect SecretError Unit</code> | Removes a blob by key id. |

### Blob constructors and accessors

| Function | Explanation |
| --- | --- |
| **blob** keyId algorithm ciphertext<br><code>SecretKeyId -> Text -> Bytes -> EncryptedBlob</code> | Builds a typed encrypted blob value. |
| **blobKeyId** blob<br><code>EncryptedBlob -> SecretKeyId</code> | Returns the key id of a blob. |
| **blobAlgorithm** blob<br><code>EncryptedBlob -> Text</code> | Returns the algorithm field of a blob. |
| **blobCiphertext** blob<br><code>EncryptedBlob -> Bytes</code> | Returns the ciphertext field of a blob. |
| **validateBlob** blob<br><code>EncryptedBlob -> Bool</code> | Validates required blob fields (`keyId`, `algorithm`, `ciphertext`). |

## Notes

- Runtime backends may map this API to OS keyring services (e.g. libsecret).
- The v0.1 runtime keeps a local in-process backend while preserving typed surface compatibility.
