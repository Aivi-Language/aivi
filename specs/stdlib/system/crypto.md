# Crypto Domain

<!-- quick-info: {"kind":"module","name":"aivi.crypto"} -->
The `Crypto` domain provides essential tools for security and uniqueness.

From generating unguessable **UUIDs** for database keys to hashing passwords with **SHA-256**, these functions ensure your program's sensitive data remains secure, unique, and tamper-evident.

<!-- /quick-info -->
<div class="import-badge">use aivi.crypto</div>

<<< ../../snippets/from_md/stdlib/system/crypto/crypto_domain.aivi{aivi}

## Hashing

| Function | Explanation |
| --- | --- |
| **sha256** text<br><code>Text -> Text</code> | Returns the SHA-256 hash of `text` encoded as hex. |
| **sha384** text<br><code>Text -> Text</code> | Returns the SHA-384 hash of `text` encoded as hex. |
| **sha512** text<br><code>Text -> Text</code> | Returns the SHA-512 hash of `text` encoded as hex. |

## HMAC

HMAC (Hash-based Message Authentication Code) produces a keyed hash that verifies both authenticity and integrity.

| Function | Explanation |
| --- | --- |
| **hmacSha256** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes HMAC-SHA-256. |
| **hmacSha512** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes HMAC-SHA-512. |
| **hmacVerify** key message tag<br><code>Bytes -> Bytes -> Bytes -> Bool</code> | Constant-time comparison of an HMAC tag. |

## Password Hashing

Password hashing uses deliberately slow algorithms to resist brute-force attacks. Never store passwords with `sha256`   use these instead.

| Function | Explanation |
| --- | --- |
| **hashPassword** password<br><code>Text -> Effect CryptoError Text</code> | Hashes a password using Argon2id with safe defaults. Returns an opaque PHC-format string. |
| **verifyPassword** password hash<br><code>Text -> Text -> Effect CryptoError Bool</code> | Verifies a password against a stored hash. Constant-time. |

## Random

| Function | Explanation |
| --- | --- |
| **randomUuid** :()<br><div class="type-sig"><code>Unit -> Effect CryptoError Text</code></div> | Generates a random UUID v4. |
| **randomBytes** n<br><code>Int -> Effect CryptoError Bytes</code> | Generates `n` cryptographically secure random bytes. |

## Utilities

| Function | Explanation |
| --- | --- |
| **secureEquals** a b<br><code>Bytes -> Bytes -> Bool</code> | Constant-time byte comparison (prevents timing attacks). |
| **toHex** bytes<br><code>Bytes -> Text</code> | Encodes bytes as a lowercase hex string. |
| **fromHex** text<br><code>Text -> Result CryptoError Bytes</code> | Decodes a hex string to bytes. |
