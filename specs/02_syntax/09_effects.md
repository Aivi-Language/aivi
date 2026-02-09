# Effects

## 9.1 The `Effect E A` Type

Effectful operations in AIVI are modeled using the `Effect E A` type, where:
- `E` is the **error domain** (describing what could go wrong).
- `A` is the **successful return value**.

### Semantics
- **Atomic Progress**: Effects are either successfully completed, failed with `E`, or **cancelled**.
- **Cancellation**: Cancellation is an asynchronous signal that stops the execution of an effect. When cancelled, the effect is guaranteed to run all registered cleanup (see [Resources](15_resources.md)).
- **Transparent Errors**: Errors in `E` are part of the type signature, forcing explicit handling or propagation.


## 9.2 `effect` blocks

```aivi
main = effect {
  cfg <- load (file.json "config.json")
  _ <- print "loaded"
}
```

This is syntax sugar for monadic binding (see Desugaring section). All effectful operations within these blocks are automatically sequenced.

Inside an `effect { ... }` block:

- `x <- eff` binds the result of an `Effect` to `x`
- `x = e` is a pure local binding
- `x <- res` acquires a `Resource` (see [Resources](15_resources.md))
- Branching is done with ordinary expressions (`if`, `case`, `?`); `->` guards are generator-only.

Compiler checks:

- Expression statements of non-`Effect` type in an `effect { ... }` block produce a warning unless they are the final expression or are bound.
- Discarded `Effect` results produce a warning unless explicitly bound (including binding to `_`).

### `if` with nested blocks inside `effect`

`if` is an expression, so you can branch inside an `effect { … }` block. When a branch needs multiple statements, use a `do { … }` block to group them (since `{ … }` is reserved for record-shaped forms). The `do { … }` is still inside the surrounding `effect { … }`, so it may contain:

- effect binds (`x <- eff`)
- pure local bindings (`x = e`)
- a final expression that becomes the branch result

This pattern is common when a branch needs multiple effectful steps:

```aivi
main = effect {
  u <- loadUser
  if u.isAdmin then do {
    _ <- log "admin login"
    token <- mintToken u
    token
  } else "guest"
}
```

Desugaring-wise, the `if … then … else …` appears inside the continuation of a `bind`, and each branch desugars to its own sequence of `bind` calls.

### Nested `effect { … }` expressions inside `if`

An explicit `effect { … }` is itself an expression of type `Effect E A`. If you write `effect { … }` in an `if` branch, you usually want to run (bind) the chosen effect:

```aivi
main = effect {
  token <- if shouldMint then effect { mintToken user } else effect { "guest" }
  token
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
| `val = effect { x <- f; g x }` | `val = f |> bind (x => g x)` |
| `effect { f; g }` | `f |> bind (_ => g)` |

Example translation:

```aivi
// Sequence with effect block
transfer fromAccount toAccount amount = effect {
  balance <- getBalance fromAccount
  if balance >= amount then do {
    _ <- withdraw fromAccount amount
    _ <- deposit toAccount amount
    Ok Unit
  } else Err InsufficientFunds
}

// Equivalent functional composition
transfer fromAccount toAccount amount =
  getBalance fromAccount |> bind (balance =>
    if balance >= amount then
      withdraw fromAccount amount |> bind (_ =>
        deposit toAccount amount |> bind (_ =>
          pure (Ok Unit)))
    else
      pure (Err InsufficientFunds)
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
  Unit
}
```

### Expressive Error Handling
```aivi
// Attempt operation, providing a typed default on error
getUser = id => effect {
  res <- api.fetchUser id
  res ?
    | Ok user => user
    | Err _   => GuestUser
}

validatedUser = effect {
  u <- getUser 123
  if u.age > 18 then Ok (toAdmin u) else Err TooYoung
}
```
