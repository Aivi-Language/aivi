# Regex Domain

The `Regex` domain provides text pattern matching.

## Overview

```aivi
import aivi.std.regex use { Regex }

let email_pattern = ~r/^[\w-\.]+@([\w-]+\.)+[\w-]{2,4}$/
let match = Regex.test(email_pattern, "user@example.com")
```

## Why this exists

Text processing often requires pattern matching beyond simple substring searches. The `Regex` domain provides a safe, efficient way to find and extract data from text.

- **Compile-time safety**: Using the `~r/.../` sigil ensures your regex is valid before the code runs.
- **Efficiency**: Compiles to optimized native matching engines.
