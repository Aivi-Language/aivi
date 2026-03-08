# Secrets Module

<!-- quick-info: {"kind":"module","name":"aivi.secrets"} -->
`aivi.secrets` is a small abstraction for storing and retrieving encrypted secret values by key.
<!-- /quick-info -->

<div class="import-badge">use aivi.secrets</div>

## What this module is for

Use this module when your program needs to keep an encrypted blob in a secret store and look it up later by an identifier.
A common example is storing an API credential, refresh token, or other sensitive payload that has already been encrypted.

This module focuses on storage and retrieval. It does not turn plaintext into ciphertext for you; instead, it works with `EncryptedBlob` values.

## Types

### `SecretKeyId`

Alias for `Text`; used as the lookup key for a stored secret.

```aivi
SecretKeyId = Text
```

### `SecretError`

Alias for `Text`; used as the error type for secret operations.

```aivi
SecretError = Text
```

### `EncryptedBlob`

A structured encrypted value with the information needed to store and validate it.

<<< ../../snippets/from_md/stdlib/system/secrets/block_03.aivi{aivi}


## Core API

### Storage operations

| Function | What it does |
| --- | --- |
| **put** keyId blob<br><code>SecretKeyId -> EncryptedBlob -> Effect SecretError Unit</code> | Stores `blob` under `keyId`. |
| **get** keyId<br><code>SecretKeyId -> Effect SecretError (Option EncryptedBlob)</code> | Loads the blob for `keyId`, returning `None` when nothing is stored there. |
| **delete** keyId<br><code>SecretKeyId -> Effect SecretError Unit</code> | Removes the stored blob for `keyId`. |

### Blob construction and inspection

| Function | What it does |
| --- | --- |
| **blob** keyId algorithm ciphertext<br><code>SecretKeyId -> Text -> Bytes -> EncryptedBlob</code> | Builds an `EncryptedBlob` value. |
| **blobKeyId** blob<br><code>EncryptedBlob -> SecretKeyId</code> | Returns the blob's key identifier. |
| **blobAlgorithm** blob<br><code>EncryptedBlob -> Text</code> | Returns the algorithm label. |
| **blobCiphertext** blob<br><code>EncryptedBlob -> Bytes</code> | Returns the encrypted bytes. |
| **validateBlob** blob<br><code>EncryptedBlob -> Bool</code> | Checks that the required blob fields are present and usable. |

## Typical workflow

1. Create or obtain encrypted bytes.
2. Wrap them in an `EncryptedBlob` with a key id and algorithm label.
3. Store the blob with `put`.
4. Load it later with `get` and inspect or validate it before use.

## Notes

- Runtime backends may map this API to operating-system secret stores such as libsecret.
- The runtime surface stays the same even when the backing storage implementation changes.
