# Effects

## 9.1 The `Effect E A` Type

Effectful operations in AIVI are modeled using the `Effect E A` type, where:
- `E` is the **error domain** (describing what could go wrong).
- `A` is the **successful return value**.

### Semantics
- **Atomic Progress**: Effects are either successfully completed, failed with `E`, or **cancelled**.
- **Cancellation**: Cancellation is an asynchronous signal that stops the execution of an effect. When cancelled, the effect is guaranteed to run all registered cleanup (see [Resources](15_resources.md)).
- **Transparent Errors**: Errors in `E` are part of the type signature, forcing explicit handling or propagation.

### Core operations (surface names)

Effect sequencing is expressed via `effect { ... }` blocks, but the underlying interface is:

- `pure : A -> Effect E A` (return a value)
- `bind : Effect E A -> (A -> Effect E B) -> Effect E B` (sequence)
- `fail : E -> Effect E A` (abort with an error)

For *handling* an effect error as a value, the standard library provides:

- `attempt : Effect E A -> Effect E (Result E A)`

### Examples (core operations)

`pure` lifts a value into an effect:

```aivi
pureExample : Effect Text Int
pureExample =
  pure 42
```

`bind` sequences effects explicitly (the `effect { ... }` block desugars to `bind`):

```aivi
bindExample : Effect Text Int
bindExample =
  (pure 41 |> bind) (n => pure (n + 1))
```

`fail` aborts an effect with an error value:

```aivi
failExample : Effect Text Int
failExample =
  fail "boom"
```

`attempt` runs an effect and captures success/failure as a `Result`:

```aivi
attemptExample : Effect Text Text
attemptExample = effect {
  res <- attempt (fail "nope")
  res ?
    | Ok _  => pure "unexpected"
    | Err e => pure e
}
```

### `load`

The standard library function `load` lifts a typed `Source` (see [External Sources](12_external_sources.md)) into an `Effect`.

```aivi
load : Source K A -> Effect (SourceError K) A
```

## 9.2 `effect` blocks

```aivi
main = effect {
  cfg <- load (file.json "config.json")
  print "loaded"
}
```

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside an `effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding (does not run effects)
- `x <- res` acquires a `Resource` (see [Resources](15_resources.md))
- Branching is done with ordinary expressions (`if`, `case`, `?`); `->` guards are generator-only.
- If a final expression is present, it must be an `Effect` (commonly `pure value` or an effect call like `print "..."`).
- If there is no final expression, the block defaults to `pure Unit`.

Compiler checks:

- `x = e` requires `e` to be a pure expression (not `Effect` and not `Resource`).
  If you want to run an effect, use `<-`:
  `use '<-' to run effects; '=' binds pure values`.
- Expression statements in statement position (not the final expression) must be `Effect E Unit`.
  If an effect returns a non-`Unit` value, you must bind it explicitly (even if you bind to `_`).

### Fallback with `or` (fallback-only)

`or` is **not** a general matcher. It is fallback-only sugar for common "default on error" patterns.

Two forms exist:

1) **Effect fallback** (inside `effect {}` and only after `<-`):

```aivi
txt <- load (file.read path) or "(missing)"
```

This runs the effect; if it fails, it produces the fallback value instead.

You can also match on the error value using arms (patterns match the **error**, not `Err`):

```aivi
txt <- load (file.read path) or
  | NotFound _ => "(missing)"
  | _          => "(other-error)"
```

2) **Result fallback** (expression form):

```aivi
msg = res or "boom"
```

Or with explicit `Err ...` arms:

```aivi
msg =
  res or
    | Err NotFound m => m
    | Err _          => "boom"
```

Restrictions (v0.1):

- Effect fallback arms match the error value (so write `NotFound m`, not `Err NotFound m`).
- In `effect { ... }`, `x <- eff or | Err ... => ...` is parsed as a **Result** fallback (for ergonomics).
  If you mean effect-fallback, write error patterns directly (`NotFound ...`) rather than `Err ...`.
- Result fallback arms must match only `Err ...` at the top level (no `Ok ...`, no `_`).
  Include a final `Err _` catch-all arm.

### `if ... else Unit` as a statement

In `effect { ... }`, this common pattern is allowed without `_ <-`:

```aivi
effect {
  if cond then print "branch" else Unit
}
```

Conceptually, the `Unit` branch is lifted to `pure Unit` so both branches have an `Effect` type.

### Concise vs explicit `effect` style

These are equivalent:

```aivi
// Concise (recommended in effect blocks)
main = effect {
  res <- attempt (foo x)
  verdict = res ?
    | Ok _  => "ok"
    | Err _ => "err"

  print verdict

  if cond then print "branch" else Unit
}
```

```aivi
// More explicit (same semantics)
main = effect {
  res <- attempt (foo x)
  verdict = res ?
    | Ok _  => "ok"
    | Err _ => "err"

  _ <- print verdict

  _ <- if cond then print "branch" else pure Unit
}
```

### `if` with nested blocks inside `effect`

`if` is an expression, so you can branch inside an `effect { … }` block. When a branch needs multiple steps, use a nested `effect { … }` block (since `{ … }` is reserved for record-shaped forms).

This pattern is common when a branch needs multiple effectful steps:

```aivi
main = effect {
  u     <- loadUser
  token <- if u.isAdmin then effect {
    log "admin login"
    token <- mintToken u
    pure token
  } else pure "guest"
  pure token
}
```

Desugaring-wise, the `if … then … else …` appears inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `effect { … }` expressions inside `if`

An explicit `effect { … }` is itself an expression of type `Effect E A`. If you write `effect { … }` in an `if` branch, you usually want to run (bind) the chosen effect:

```aivi
main = effect {
  token <- if shouldMint then mintToken user else pure "guest"
  pure token
}
```

If you instead write `if … then effect { … } else effect { … }` *without* binding it, the result of the `if` is an `Effect …` value, not a sequence of steps in the surrounding block (unless it is the final expression of that surrounding `effect { … }`).


## 9.3 Effects and patching

```aivi
authorize = user => user <| {
  roles: _ ++ ["Admin"]
  lastLogin: now
}
```

Patches are pure values. Apply them where you have the record value available (often inside an `effect` block after decoding/loading).


## 9.4 Comparison and Translation

The `effect` block is the primary way to sequence impure operations. It translates directly to monadic binds.

| `effect` Syntax | Explicit Monadic Syntax |
| :--- | :--- |
| `val = effect { x <- f; g x }` | `val = (f |> bind) (x => g x)` |
| `effect { f; g }` | `(f |> bind) (_ => g)` |

Example translation:

```aivi
// Sequence with effect block
transfer fromAccount toAccount amount = effect {
  balance <- getBalance fromAccount
  if balance >= amount then effect {
    withdraw fromAccount amount
    deposit toAccount amount
  } else fail InsufficientFunds
}

// Equivalent functional composition
transfer fromAccount toAccount amount =
  (getBalance fromAccount |> bind) (balance =>
    if balance >= amount then
      (withdraw fromAccount amount |> bind) (_ =>
        (deposit toAccount amount |> bind) (_ =>
          pure Unit))
    else
      fail InsufficientFunds
  )
```
## 9.5 Expressive Effect Composition

Effect blocks can be combined with pipelines and pattern matching to create very readable business logic.

### Concatenating effectful operations
```aivi
// Fetch config, then fetch data, then log
setup = effect {
  cfg  <- loadConfig "prod.json"
  data <- fetchRemoteData cfg
  logSuccess data
}
```

### Expressive Error Handling
```aivi
// Attempt operation, providing a typed default on error
getUser = id => effect {
  res <- attempt (api.fetchUser id)
  res ?
    | Ok user => pure user
    | Err _   => pure GuestUser
}

validatedUser = effect {
  u <- getUser 123
  if u.age > 18 then pure (toAdmin u) else fail TooYoung
}
```
