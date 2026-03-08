# Secrets Module

<!-- quick-info: {"kind":"module","name":"aivi.secrets"} -->
`aivi.secrets` stores and retrieves already-encrypted secret values by application-chosen keys.
<!-- /quick-info -->

<div class="import-badge">use aivi.secrets</div>

## What this module is for

Use this module when your program already has ciphertext and wants to store it under a stable identifier, then load it again later.
Common examples include encrypted API credentials, refresh tokens, or service-specific session material.

This module focuses on storage and retrieval. It does not turn plaintext into ciphertext for you; instead, it works with `EncryptedBlob` values.
If your host already injects a secret as plaintext configuration, [`aivi.system.env.get`](./system.md) is often a better fit.

## Quick example

<<< ../../snippets/from_md/stdlib/system/secrets/block_01.aivi{aivi}


This example shows the intended shape of the API: create a blob, store it, then read it back as an `Option`.

## Types

### `SecretKeyId`

Alias for `Text`; used as the lookup key for a stored secret.

<<< ../../snippets/from_md/stdlib/system/secrets/block_02.aivi{aivi}


### `SecretError`

Alias for `Text`; used as the error type for backend or store failures.
Missing secrets are reported as `None` by `get`, not as `SecretError`.

<<< ../../snippets/from_md/stdlib/system/secrets/block_03.aivi{aivi}


### `EncryptedBlob`

An opaque encrypted value containing the metadata needed to store and later inspect ciphertext (a chunk of binary data that is unreadable without decryption).

<<< ../../snippets/from_md/stdlib/system/secrets/block_04.aivi{aivi}


## Core API

### Storage operations

| Function | What it does |
| --- | --- |
| **put** keyId blob<br><code>SecretKeyId -> EncryptedBlob -> Effect SecretError Unit</code> | Stores `blob` under `keyId`. The storage lookup uses the explicit `keyId` argument, so keep it consistent with `blobKeyId blob`. |
| **get** keyId<br><code>SecretKeyId -> Effect SecretError (Option EncryptedBlob)</code> | Loads the blob for `keyId`, returning `None` when nothing is stored there. |
| **delete** keyId<br><code>SecretKeyId -> Effect SecretError Unit</code> | Removes the stored blob for `keyId` if one is present. |

### Blob construction and inspection

| Function | What it does |
| --- | --- |
| **blob** keyId algorithm ciphertext<br><code>SecretKeyId -> Text -> Bytes -> EncryptedBlob</code> | Builds an `EncryptedBlob` value from a key id, an algorithm label, and raw ciphertext bytes. |
| **blobKeyId** blob<br><code>EncryptedBlob -> SecretKeyId</code> | Returns the blob's key identifier. |
| **blobAlgorithm** blob<br><code>EncryptedBlob -> Text</code> | Returns the algorithm label. |
| **blobCiphertext** blob<br><code>EncryptedBlob -> Bytes</code> | Returns the encrypted bytes. |
| **validateBlob** blob<br><code>EncryptedBlob -> Bool</code> | Performs a lightweight structural check. In the current implementation this means the key id and algorithm label are non-empty; it does not decrypt or authenticate the ciphertext. |

## Typical workflow

1. Create or obtain encrypted bytes from the system that manages your encryption keys.
2. Wrap them in an `EncryptedBlob` with a key id and an algorithm label that your application understands.
3. Store the blob with `put`, using the same lookup key you placed inside the blob.
4. Load it later with `get`.
5. Use `validateBlob` for a quick structural sanity check before handing the blob to your own decryption or verification layer.

## Notes

- Backends may implement this API in different ways, including in-process storage or operating-system secret services.
- Do not assume a secret written by one process will still be available in a later process unless the target runtime explicitly documents persistence.
- The public AIVI API stays the same even when the backing storage strategy changes.
