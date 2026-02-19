# Comments

AIVI supports two comment styles: line comments and block comments. Comments are stripped by the lexer and do not affect program semantics.

## Line Comments

A line comment begins with `//` and extends to the end of the line (the newline character is not part of the comment).

```aivi
// This is a line comment
x = 42 // inline comment after an expression
```

Line comments are commonly used for:
- Explaining the intent of a binding or expression
- Temporarily disabling a line of code during development
- Adding short inline annotations

## Block Comments

A block comment begins with `/*` and ends with `*/`. It can span multiple lines. Block comments do **not** nest.

```aivi
/*
  This is a block comment.
  It can span multiple lines.
*/
result = /*inline block*/ 42
```

> **Note**: Block comments do not nest. `/* outer /* inner */ still open */` — the comment closes at the first `*/`.

Block comments are commonly used for:
- Temporarily disabling a larger section of code
- Multi-line explanatory prose that doesn't belong in a doc annotation
- Annotating complex expressions inline without breaking the expression

## Placement

Comments may appear anywhere whitespace is allowed — before or after any token, between expressions, and at the top of a file. The formatter preserves comments and does not reorder them.

```aivi
// Module-level documentation comment
foo = bar // call bar
  |> baz  /* pipe to baz */
```

## What Is Not Supported

- **Nested block comments** — `/* /* */ */` is not valid; the inner `*/` terminates the outer comment.
- **Doc comments** — there is no dedicated `///` or `/** */` doc-comment syntax in v0.1. Use plain line comments for documentation purposes.
- **Shebangs** — `#!` is not a comment form.
