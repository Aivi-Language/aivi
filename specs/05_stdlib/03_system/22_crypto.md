# Crypto Domain

The `Crypto` domain provides essential tools for security and uniqueness.

From generating unguessable **UUIDs** for database keys to hashing passwords with **SHA-256**, these functions ensure your program's sensitive data remains secure, unique, and tamper-evident.

```aivi
use aivi.crypto
```

## Functions

### `sha256`

```aivi
sha256 : String -> String
```

Computes the SHA-256 hash of a string (hex encoded).

### `randomUuid`

```aivi
randomUuid : Unit -> Effect String
```

Generates a random UUID v4.

### `randomBytes`

```aivi
randomBytes : Int -> Effect Bytes
```

Generates `n` random bytes.
