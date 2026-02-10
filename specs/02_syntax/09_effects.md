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

In some examples, `load` is used as a helper to observe or lift a resource operation into an effect.

## 9.2 `effect` blocks

```aivi
main = effect {
  cfg <- load (file.json "config.json")
  _ <- print "loaded"
  pure Unit
}
```

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside an `effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding
- `x <- res` acquires a `Resource` (see [Resources](15_resources.md))
- Branching is done with ordinary expressions (`if`, `case`, `?`); `->` guards are generator-only.
- The final expression must be an `Effect` (commonly `pure value` or an effect call like `print "..."`).

Compiler checks:

- Expression statements of non-`Effect` type in an `effect { ... }` block produce a warning unless they are the final expression or are bound.
- Discarded `Effect` results produce a warning unless explicitly bound (including binding to `_`).

### `if` with nested blocks inside `effect`

`if` is an expression, so you can branch inside an `effect { … }` block. When a branch needs multiple steps, use a nested `effect { … }` block (since `{ … }` is reserved for record-shaped forms).

This pattern is common when a branch needs multiple effectful steps:

```aivi
main = effect {
  u <- loadUser
  token <- if u.isAdmin then effect {
    _ <- log "admin login"
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
    _ <- withdraw fromAccount amount
    _ <- deposit toAccount amount
    pure Unit
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
  cfg <- loadConfig "prod.json"
  data <- fetchRemoteData cfg
  _ <- logSuccess data
  pure Unit
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
