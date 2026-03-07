# Capabilities

<!-- quick-info: {"kind":"topic","name":"capabilities"} -->
Capabilities describe the external authority an `Effect E A` or `Resource E A` needs, without changing the meaning of `E` or `A`.
<!-- /quick-info -->

Capabilities answer a practical question: **what is this effectful code allowed to touch?** They describe outside-world authority such as file access, network access, clocks, UI integration, or process state.

## What capabilities are

AIVI does **not** use capabilities as a second error channel. They sit on top of `Effect E A` and `Resource E A` as an authority contract:

- `E` still describes typed domain failures
- `A` is still the success value
- capabilities describe which external operations may be used
- missing capabilities are compile-time diagnostics, not runtime `E` values

`attempt`, `or`, `resource`, and `do Effect { ... }` keep their existing meaning. A capability clause only narrows the external authority available inside that effectful code.

## What capabilities are for

Capabilities make effectful code easier to understand and review.

They help you answer questions like:

- “Does this function only read a file, or can it also write?”
- “Can this helper make network calls?”
- “Does this UI action schedule timers or spawn background work?”

In practice, capabilities help keep APIs narrow, make authority creep visible during code review, and separate business failures from permission or scope mistakes.

## How capabilities look in code

Write capability requirements as a postfix clause on an effectful type.

```aivi
readConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }

openStore : DbConfig -> Resource DbError DbConn with { db.connect }

launchUi : Model -> Effect GtkError Unit with { ui.window, ui.signal }
```

```aivi
saveProfile : Text -> Bytes -> Effect ProfileError Unit with { file.write, network.http }
saveProfile = userId avatar => do Effect {
  _ <- uploadAvatar userId avatar                       -- Needs network.http
  _ <- file.writeBytes "./profile-avatar.bin" avatar    -- Needs file.write
  pure Unit
}
```

The capability clause is:

- **unordered** — `{ file.read, process.env.read }` is the same set as `{ process.env.read, file.read }`
- **duplicate-free** — repeating the same capability does not change the meaning
- **best kept explicit on public APIs** — readers should be able to see the minimum required authority at the function boundary

Family names can act as coarse shorthands. For example, `file` covers `file.read`, `file.write`, and `file.metadata`. Public APIs should usually prefer the narrowest useful leaf name.

## Common capability vocabulary

| Family | Common atoms | Meaning |
| --- | --- | --- |
| `file` | `file.read`, `file.write`, `file.metadata`, `file.watch` | Local filesystem and path authority |
| `network` | `network.http`, `network.socket.connect`, `network.socket.listen` | Outbound and inbound network access |
| `db` | `db.connect`, `db.query`, `db.mutate`, `db.migrate` | Database connectivity, reads, writes, and schema changes |
| `clock` | `clock.now`, `clock.sleep`, `clock.schedule` | Reading time and scheduling timers |
| `randomness` | `randomness.secure`, `randomness.pseudo` | Entropy and random number generation |
| `process` | `process.args`, `process.env.read`, `process.env.write`, `process.exit`, `process.spawn` | Interaction with the host process and environment |
| `ui` | `ui.window`, `ui.signal`, `ui.clipboard`, `ui.notification` | Native UI creation, event delivery, and desktop integration |
| `cancellation` | `cancellation.observe`, `cancellation.propagate`, `cancellation.mask` | Structured cancellation and cancellation control |

The first segment names the family. Later segments narrow that authority to the smallest useful operation.

## How to read a capability signature

A capability clause describes the **minimum** authority a function needs. Callers may always invoke that function from a scope that has a **superset** of the listed capabilities.

```aivi
loadUserCache : Text -> Effect CacheError User with { file.read }

refreshUser : Url -> Effect SyncError User with { network.http, file.read }
refreshUser = url => do Effect {
  remote <- load (rest.get url)
  cached <- loadUserCache "./user-cache.json"
  pure (merge remote cached)
}
```

`loadUserCache` only asks for `file.read`, so it can run inside a larger scope that also includes `network.http`, `ui`, `db`, or other authority. The practical rule is simple: **smaller requirement sets compose into larger ones**.

Pure functions carry no capability clause because they do not need outside-world authority.

## How to use capabilities

### 1. Annotate effectful public APIs

Put capability clauses on public `Effect` and `Resource` signatures.

```aivi
readConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }
saveDraft : Text -> Effect FileError Unit with { file.write }
openStore : DbConfig -> Resource DbError DbConn with { db.connect }
```

### 2. Narrow helper bodies with `with { ... } in`

`with { ... } in expr` lexically narrows the visible capability set for `expr`.

```aivi
loadBootConfig : Effect ConfigError BootConfig with { file.read, process.env.read }
loadBootConfig =
  with { file.read, process.env.read } in do Effect {
    cfg  <- load (file.json "./config.json")
    mode <- load (env.get "AIVI_MODE")
    pure { cfg, mode }
  }
```

Rules:

- the inner scope may use only the listed capabilities
- nested `with` blocks intersect, so an inner scope can narrow further but never widen authority
- this form only narrows authority; it does not install implementations by itself
- `with { capability = handler } in expr` uses the same surface form to install scoped interpreters; see [Effect Handlers](effect_handlers.md)

### 3. Keep failures and authority separate

`Effect E A` still means:

- `E` is the typed domain error
- `A` is the success value
- capabilities describe which external operations the effect may use

Missing capabilities are compile-time problems, not runtime `E` values.

## Resources and cancellation

`Resource E A` accepts the same clause shape as `Effect E A`.

```aivi
openAuditLog : Text -> Resource FileError Handle with { file.write }
```

The clause covers the whole resource lifecycle:

- acquisition before `yield`
- helper effects inside the resource body
- cleanup after `yield`

Ordinary resource cleanup is still cancellation-protected automatically. You do **not** add `cancellation.mask` just to get normal finalizer safety. Explicit cancellation-control APIs such as `scope`, `spawn`, `race`, or task cancellation are where `cancellation.*` becomes part of the public signature.

## Capabilities in GTK apps, timers, and background work

GTK commands and subscriptions reuse the same capability vocabulary.

- `gtkApp` is the coarse-grained `ui` entry point
- `Command.perform` and `Command.startTask` inherit the capabilities of the enclosed `Effect`
- `Command.after` and `Subscription.every` consume `clock.sleep` or `clock.schedule`
- keyed task or subscription cancellation uses `cancellation.propagate`
- `Subscription.source` inherits the capabilities of its underlying `Resource`

This keeps UI code aligned with the rest of the language instead of introducing a separate permission model.

## How today’s APIs map to capabilities

| Current surface | Capability requirement |
| --- | --- |
| `load (file.*)`, `load (file.image*)`, `file.readText`, `file.readJson`, `file.readCsv` | `file.read` |
| `file.writeText`, `file.writeBytes`, `file.appendText`, `file.delete`, `file.mkdir*`, `file.rmdir` | `file.write` |
| `file.exists`, `file.stat`, `file.listDir` | `file.metadata` |
| `file.copy`, `file.move` | `file.read` + `file.write` |
| `rest.*`, `http.*`, `https.*` | `network.http` |
| `email.imap*`, `smtpSend`, socket APIs | `network` |
| `db.configure`, pool creation / acquisition | `db.connect` |
| `db.load` | `db.query` |
| `db.applyDelta`, transactions, savepoints | `db.mutate` |
| `db.runMigrations`, `db.runMigrationSql` | `db.migrate` |
| `env.get`, `env.decode` | `process.env.read` |
| `env.set`, `env.remove` | `process.env.write` |
| `args`, `localeTag` | `process.args` |
| `exit` | `process.exit` |
| `Instant.now` and equivalent wall-clock reads | `clock.now` |
| `concurrency.sleep` | `clock.sleep` |
| `timeoutWith` | `clock.sleep` + `cancellation.propagate` |
| `scope`, `spawn`, `race`, explicit task cancellation | `cancellation.propagate` |
| `crypto.randomUuid`, `crypto.randomBytes`, salted password hashing | `randomness.secure` |
| `gtkApp`, `signalStream`, `reconcileNode`, clipboard helpers, notification helpers | `ui` |
| `Command.after`, `Subscription.every` | `clock.sleep` / `clock.schedule` |
| keyed `Command.startTask`, `Command.cancel`, subscription replacement/removal | `cancellation.propagate` |
| `Subscription.source` over files, sockets, db notifications, or custom feeds | capabilities required by the underlying `Resource` |
| `@static` embedded sources | no runtime capability after compilation |

## Practical rules of thumb

1. Put capabilities on public `Effect` and `Resource` signatures.
2. Prefer narrow leaves such as `file.read` over broad family names such as `file`.
3. Use `with { ... } in` to make helper bodies’ authority explicit.
4. Use `with { capability = handler } in` when you want a scoped interpreter for a capability.
5. Treat capabilities as an authority contract, not as part of your domain error model.
