# Crypto Domain

<!-- quick-info: {"kind":"module","name":"aivi.crypto"} -->
The `Crypto` domain provides building blocks for hashing, password verification, keyed message authentication, and secure random values.

Use it when you need stable fingerprints of data, password-safe storage, verification tags, random IDs, or raw cryptographically secure bytes.

<!-- /quick-info -->
<div class="import-badge">use aivi.crypto</div>

<<< ../../snippets/from_md/stdlib/system/crypto/crypto_domain.aivi{aivi}

## Choose the right tool

Different security tasks need different primitives:

- Use a **hash** when you want a fixed-size fingerprint of data.
- Use **HMAC** when you need to prove a message came from someone who knows a shared secret.
- Use **password hashing** for stored user passwords.
- Use **secure random values** for tokens, IDs, salts, and secret bytes.

## Hashing

These functions are pure and deterministic: the same input always produces the same output.
They are useful for content fingerprints, cache keys, and tamper detection.

| Function | What it does |
| --- | --- |
| **sha256** text<br><code>Text -> Text</code> | Returns the SHA-256 hash of `text` as lowercase hexadecimal. |
| **sha384** text<br><code>Text -> Text</code> | Returns the SHA-384 hash of `text` as lowercase hexadecimal. |
| **sha512** text<br><code>Text -> Text</code> | Returns the SHA-512 hash of `text` as lowercase hexadecimal. |

## HMAC

HMAC adds a secret key to a hash-based signature.
This is a common choice when one service signs a message and another service verifies that it was not changed.

| Function | What it does |
| --- | --- |
| **hmacSha256** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes an HMAC-SHA-256 tag. |
| **hmacSha512** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes an HMAC-SHA-512 tag. |
| **hmacVerify** key message tag<br><code>Bytes -> Bytes -> Bytes -> Bool</code> | Verifies an HMAC tag using a constant-time comparison. |

## Password hashing

Passwords need a deliberately slow algorithm so attackers cannot test guesses cheaply.
Do **not** store user passwords with `sha256`, `sha384`, or `sha512`.

| Function | What it does |
| --- | --- |
| **hashPassword** password<br><code>Text -> Effect CryptoError Text</code> | Hashes a password with Argon2id and returns an opaque PHC-format string suitable for storage. |
| **verifyPassword** password hash<br><code>Text -> Text -> Effect CryptoError Bool</code> | Checks a plaintext password against a stored password hash. |

## Secure random values

| Function | What it does | Common use |
| --- | --- | --- |
| **randomUuid** :()<br><div class="type-sig"><code>Unit -> Effect CryptoError Text</code></div> | Generates a random UUID v4. | Public identifiers that should be hard to guess. |
| **randomBytes** n<br><code>Int -> Effect CryptoError Bytes</code> | Generates `n` cryptographically secure random bytes. | Tokens, salts, keys, and nonces. |

## Utility helpers

| Function | What it does |
| --- | --- |
| **secureEquals** a b<br><code>Bytes -> Bytes -> Bool</code> | Compares two byte arrays in constant time to reduce timing-leak risk. |
| **toHex** bytes<br><code>Bytes -> Text</code> | Encodes bytes as lowercase hexadecimal text. |
| **fromHex** text<br><code>Text -> Result CryptoError Bytes</code> | Decodes hexadecimal text into raw bytes. |

## Capabilities

Pure hashing and HMAC helpers do not require randomness.
The functions below do:

- `randomUuid`
- `randomBytes`
- password hashing functions that generate salts internally, such as `hashPassword`

## Practical guidance

- If you need to store passwords, use `hashPassword` and `verifyPassword` together.
- If you need to sign API payloads or webhook bodies, use an HMAC helper rather than a plain hash.
- If you need a human-readable digest, call `toHex` on raw bytes instead of inventing your own encoding.
