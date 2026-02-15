# AIVI Language Summary (for LLM Context)

## 1. Core Philosophy
AIVI is a **statically typed, purely functional language** designed for high-integrity data pipelines.
*   **Immutable**: All bindings are immutable. No `mut`.
*   **No Null/Exceptions**: Uses `Option` and `Result`.
*   **Expression-Oriented**: Everything is an expression.
*   **No Loops**: Use recursion, `fold`, or `generate` blocks.
*   **WASM-Native**: Targets WASM/WASI.

## 2. Syntax Reference

### 2.1 Bindings & Functions
```aivi
// Value binding (immutable)
x = 42

// Function (lambda)
inc = n => n + 1

// Multi-argument function (explicit)
add = a b => a + b

// Piping (Data-last convention)
result = data |> filter valid |> map transform

// Choosing the pipe subject (argument position)
// x |> f      == f x
// x |> f a b  == f a b x

// Pattern Matching
whatIs = 
  | 0 => "zero"
  | n when n < 0 => "negative"
  | _ => "positive"

// Choosing the match subject (scrutinee)
// `?` matches on the expression immediately to its left.
value = parse input
value ?
  | Ok x  => x
  | Err _ => 0
```

### 2.2 Data Types
*   **Records**: Open, row-polymorphic.
    ```aivi
    p = { x: 1, y: 2 }
    x = p.x            // Accessor
    p2 = p <| { x: 3 } // Patching (Update) - NEVER use mutation or spread for updates
    ```

    Record destructuring uses a record pattern on the left-hand side:
    ```aivi
    { x, y } = p
    ```
*   **Lists**: `[1, 2, 3]`. Spread: `[head, ...tail]`.
*   **Tuples**: `(1, "a")`.
*   **Unions (ADTs)**:
    ```aivi
    type Shape = Circle Float | Rect Float Float
    ```

### 2.3 Blocks & Control Flow
*   **Do Block** (Sequential execution):
    ```aivi
    do {
      x <- computation // Bind result (monadic bind-like)
      y = x + 1        // Plain let
      y                // Return value
    }
    ```
*   **Generators** (Streams/Iterators):
    ```aivi
    // Replaces loops. Uses `yield`.
    generate {
      loop n = 0 => {
        yield n
        recurse (n + 1)
      }
    }
    ```
*   **If-Then-Else**: `if cond then a else b` (Expression, must have else).

## 3. Advanced Semantics: Domains & Coercion

### 3.1 Domains & Semantic Algebra
AIVI delegates operator semantics (`+`, `-`, `×`, etc.) to **Domains**.
*   **Context-Aware**: `+` means addition for numbers, but "shift" for dates.
*   **Algebraic Rules**: Domains define how types interact.
    *   `Carrier + Delta -> Carrier` (e.g., `Date + Month -> Date`)
    *   `Delta + Delta -> Delta` (e.g., `Month + Month -> Month`)

### 3.2 Units & Deltas
*   **Typed Literals**: Values like `10m`, `30s`, `100px` are **not** strings. They are typed symbols (Deltas).
*   **Suffix Application**: You can also apply a suffix to a parenthesized expression, e.g. `(x)kg`, `(n + 5)s`.
*   **Resolution**:
    1.  **Lexical**: Finds the delta binding (e.g., `m` -> `Month`).
    2.  **Carrier**: Selects the domain based on the operand type (e.g., `date + 1m` uses `Calendar` domain).
*   **Desugaring**: `date + 1m` becomes `Calendar.addMonth date (Month 1)`.

### 3.3 Auto-Coercion (Instance-Driven)
*   **Explicit**: Coercion is NOT implicit casting. It is driven by **Type Class Instances** (e.g., `ToText`).
*   **Context-Sensitive**: Only occurs in **expected-type positions** (function args, annotated bindings).
    *   If `func` expects `Text` but gets `Int`, and `instance ToText Int` exists, the compiler inserts `toText`.

### 3.4 Sigils (Custom Literals)
AIVI uses **Sigils** for complex literals that are validated at compile-time by domains.
*   **Syntax**: `~tag(content)` or `~tag[content]` or `~tag{content}`.
*   **Common Sigils**:
    *   `~d(2024-01-01)`: Date literal (Calendar domain).
    *   `~t(12:00:00)`: Time literal.
    *   `~r/[a-z]+/`: Regex literal.
    *   `~json{ "x": 1 }`: JSON literal (parsed at compile time).
*   **Structured Sigils**: `~map{ k => v }` and `~set[1, 2]` are syntactic sugar for collection construction.
*   **UI Sigil**: `~html~> <div>{expr}</div> <~html` parses HTML-like syntax into typed `aivi.ui.VNode msg` values (not strings) and supports `{expr}` splices.

## 4. Kernel & Semantics (Under the Hood)
*   **Desugaring**: Surface syntax desugars into a minimal **Kernel** (AST defined in `crates/aivi/src/kernel.rs`).
*   **Generators**: Compiled via **CPS transformation** into generic lambdas (`\k \z -> ...`).
*   **Modules**: `module Name { export * }`. Files are implicit modules.
*   **Effects**: Typed effect tracking `Effect E A`.

## 5. Rust Implementation Details (`crates/`)
*   **Parser**: Error-tolerant, preserves whitespace in CST for LSP.
*   **Kernel**:
    *   `KernelExpr::Lambda`: Standard closure.
    *   `KernelExpr::App`: Function application.
    *   `KernelExpr::Patch`: Optimized record update.
*   **Runtime**: Native Rust runtime executing the desugared program.
*   **LSP**: Deeply integrated, relies on `specs/` as source of truth.

## 6. Important Rules for Code Generation
1.  **Use `|>`**: Prefer pipelines over nested function calls.
2.  **Use `<|`**: ALWAYS use patching for record updates.
3.  **No `return`**: The last expression is the return value.
4.  **No `for/while`**: Use `generate` blocks or recursion.
5.  **Strict Identifiers**: Types/Modules MUST be `UpperCamelCase`. Variables/Functions MUST be `lowerCamelCase`.
6.  **Imports**: `use aivi.list` or `use aivi.list (map, filter)`.
