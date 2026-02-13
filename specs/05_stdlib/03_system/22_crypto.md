# Crypto Domain

The `Crypto` domain provides essential tools for security and uniqueness.

From generating unguessable **UUIDs** for database keys to hashing passwords with **SHA-256**, these functions ensure your program's sensitive data remains secure, unique, and tamper-evident.

<<< ../../snippets/from_md/05_stdlib/03_system/22_crypto/block_01.aivi{aivi}

## Functions

| Function | Explanation |
| --- | --- |
| **sha256** text<br><pre><code>`String -> String`</code></pre> | Returns the SHA-256 hash of `text` encoded as hex. |
| **randomUuid** :()<br><pre><code>`Unit -> Effect String`</code></pre> | Generates a random UUID v4. |
| **randomBytes** n<br><pre><code>`Int -> Effect Bytes`</code></pre> | Generates `n` random bytes. |
