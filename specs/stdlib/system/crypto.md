# Crypto Domain

<!-- quick-info: {"kind":"module","name":"aivi.crypto"} -->
The `Crypto` domain provides building blocks for hashing, password verification, keyed message authentication, and secure random values.

Use it when you need stable fingerprints of data, password-safe storage, verification tags, random IDs, or raw cryptographically secure bytes.

<!-- /quick-info -->
<div class="import-badge">use aivi.crypto</div>

## What this module is for

`aivi.crypto` is the small standard-library surface for four common jobs:

- creating deterministic fingerprints of text,
- signing and verifying byte payloads with a shared secret,
- hashing passwords for storage,
- generating secure random identifiers, salts, and token bytes.

Two distinctions matter right away:

- plain hashes such as `sha256` are for fingerprints, not authenticity,
- HMAC helpers work on `Bytes`, which makes them the right tool for signed payloads and webhook-style verification.

## Quick start

```aivi
use aivi.crypto

fingerprint = sha256 "hello world"

makeSessionToken : Effect Text Text with { randomness.secure }
makeSessionToken = do Effect {
  bytes <- randomBytes 32
  pure (toHex bytes)
}

checkPassword : Text -> Text -> Effect Text Bool
checkPassword = password storedHash => verifyPassword password storedHash
```

## Choose the right tool

Different security tasks need different primitives:

- Use a **hash** when you want a fixed-size fingerprint of data.
- Use **HMAC** when you need to prove a message came from someone who knows a shared secret.
- Use **password hashing** for stored user passwords.
- Use **secure random values** for tokens, IDs, salts, and secret bytes.

## Hashing

These functions are pure and deterministic: the same input always produces the same output.
They are useful for content fingerprints, cache keys, and deduplication.
A plain hash does **not** prove who produced the data; if an attacker can change both the message and the digest, use HMAC instead.

| Function | What it does |
| --- | --- |
| **sha256** text<br><code>Text -> Text</code> | Returns the SHA-256 hash of `text` as lowercase hexadecimal. |
| **sha384** text<br><code>Text -> Text</code> | Returns the SHA-384 hash of `text` as lowercase hexadecimal. |
| **sha512** text<br><code>Text -> Text</code> | Returns the SHA-512 hash of `text` as lowercase hexadecimal. |

## HMAC

HMAC adds a secret key to a hash-based signature.
This is a common choice when one service signs a message and another service verifies that it came from someone who knows the shared secret.
All HMAC helpers operate on `Bytes`, so they fit best when your protocol already defines an exact byte representation.

| Function | What it does |
| --- | --- |
| **hmacSha256** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes an HMAC-SHA-256 tag. |
| **hmacSha512** key message<br><code>Bytes -> Bytes -> Bytes</code> | Computes an HMAC-SHA-512 tag. |
| **hmacVerify** key message tag<br><code>Bytes -> Bytes -> Bytes -> Bool</code> | Verifies a tag produced by `hmacSha256`. |

If you need a text form for storage or logging, convert the resulting tag with `toHex`.

## Password hashing

Passwords need a deliberately slow algorithm so attackers cannot test guesses cheaply.
Do **not** store user passwords with `sha256`, `sha384`, or `sha512`.
Store the returned hash string as an opaque value and hand it back to `verifyPassword` unchanged.

| Function | What it does |
| --- | --- |
| **hashPassword** password<br><code>Text -> Effect Text Text</code> | Hashes a password with the current bcrypt-based runtime implementation and returns the stored hash string. |
| **verifyPassword** password hash<br><code>Text -> Text -> Effect Text Bool</code> | Checks a plaintext password against a stored hash string. Returns `False` for a wrong password and `Text` errors for malformed hashes or backend failures. |

## Secure random values

| Function | What it does | Common use |
| --- | --- | --- |
| **randomUuid**<br><code>Effect Text Text</code> | Generates a random UUID v4 string. | Public identifiers that should be hard to guess. |
| **randomBytes** n<br><code>Int -> Effect Text Bytes</code> | Generates `n` cryptographically secure random bytes. | Tokens, salts, keys, and nonces. |

## Utility helpers

| Function | What it does |
| --- | --- |
| **secureEquals** a b<br><code>Bytes -> Bytes -> Bool</code> | Compares two byte arrays in constant time to reduce timing-leak risk. |
| **toHex** bytes<br><code>Bytes -> Text</code> | Encodes bytes as lowercase hexadecimal text. |
| **fromHex** text<br><code>Text -> Result Text Bytes</code> | Decodes hexadecimal text into raw bytes. Returns `Err message` when the input is not valid hexadecimal. |

## Capabilities

Pure hashing and HMAC helpers do not require capabilities.
The following operations consume secure randomness and therefore map to `randomness.secure`; see [Capabilities](../../syntax/capabilities.md):

- `randomUuid`
- `randomBytes`
- `hashPassword`

`verifyPassword` is still effectful because malformed stored hashes or backend failures can produce `Text` errors, but it does not generate randomness itself.

## Verification

The exported `aivi.crypto` module surface is exercised in `integration-tests/stdlib/aivi/crypto/crypto.aivi`, covering digest lengths, HMAC round-trips, password hashing and verification, random byte lengths, UUID generation, and hex encode/decode helpers.

## Practical guidance

- If you need to store passwords, use `hashPassword` and `verifyPassword` together.
- If you need to sign API payloads or webhook bodies, use an HMAC helper rather than a plain hash.
- If you need a human-readable digest, call `toHex` on raw bytes instead of inventing your own encoding.
