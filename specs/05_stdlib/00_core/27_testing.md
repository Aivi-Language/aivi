# Testing Domain

The `Testing` domain is built right into the language because reliability shouldn't be an afterthought. Instead of hunting for third-party runners or configuring complex suites, you can just write `@test` next to your code. It provides a standard, unified way to define, discover, and run tests, making sure your code does exactly what you think it does (and keeps doing it after you refactor).

## Overview

```aivi
use aivi.testing (assert, assertEq)

@test
additionWorks _ = {
    assertEq (1 + 1) 2
}
```

## Goals for v1.0

- `test` keyword or block construct.
- Assertions with rich diffs (`assertEq`, etc.).
- Test discovery and execution via `aivi test`.
- Property-based testing basics (generators) integration.
