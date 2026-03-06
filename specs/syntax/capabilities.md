# Capabilities

<!-- quick-info: {"kind":"topic","name":"capabilities"} -->
Capabilities describe the external authority an `Effect E A` or `Resource E A` needs, without changing the meaning of `E` or `A`.
<!-- /quick-info -->

> **Status:** Phase 1 capability surface is specified here. Signature clauses and lexical narrowing are part of the language design. Handler / interpreter binding and runtime plumbing land in later milestones. Until then, existing ambient builtins remain valid and should be read as implicitly requiring the mapped capabilities from this page.

## Overview

AIVI does **not** introduce a second effect system for permissions. Capabilities sit **on top of** `Effect E A` and `Resource E A` as a minimum-authority contract:

- `E` still models typed domain failures.
- `A` is still the success value.
- capability requirements describe which outside-world access is allowed.
- missing capabilities are compile-time diagnostics, not values inside `E`.

`attempt`, `or`, `resource`, and `do Effect { ... }` keep their existing semantics. The capability clause only refines which ambient operations may appear inside those effects.

## Surface syntax

Capability requirements are written as a postfix clause on an effectful type:

```aivi
readConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }

openStore : DbConfig -> Resource DbError DbConn with { db.connect }

launchUi : Model -> Effect GtkError Unit with { ui.window, ui.signal }
```

The capability clause is:

- **unordered** — `{ file.read, process.env.read }` equals `{ process.env.read, file.read }`
- **duplicate-free** — duplicates are ignored
- **optional during migration** — omitting it means the declaration has not yet been tightened; new public APIs should spell the minimum required set explicitly

Family names may be used as coarse shorthands. `file` satisfies `file.read`, `file.write`, and `file.metadata`. Public library APIs should prefer the narrowest stable leaf names.

## Standard vocabulary

| Family | Common atoms | Meaning |
| --- | --- | --- |
| `file` | `file.read`, `file.write`, `file.metadata`, `file.watch` | Local filesystem and path authority. |
| `network` | `network.http`, `network.socket.connect`, `network.socket.listen` | Outbound and inbound network access. |
| `db` | `db.connect`, `db.query`, `db.mutate`, `db.migrate` | Database connectivity, reads, writes, and schema change authority. |
| `clock` | `clock.now`, `clock.sleep`, `clock.schedule` | Reading time and scheduling timers. |
| `randomness` | `randomness.secure`, `randomness.pseudo` | Entropy and random number generation. |
| `process` | `process.args`, `process.env.read`, `process.env.write`, `process.exit`, `process.spawn` | Interaction with the host process and its environment. |
| `ui` | `ui.window`, `ui.signal`, `ui.clipboard`, `ui.notification` | Native UI creation, event delivery, and desktop integration. |
| `cancellation` | `cancellation.observe`, `cancellation.propagate`, `cancellation.mask` | Structured cancellation, task control, and protected cleanup scopes. |

The first segment of a capability path is the stable family name. Later milestones may add more leaves under those families, but the top-level vocabulary above is the Phase 1 contract.

## Signature meaning and capability-polymorphic code

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

`loadUserCache` is capability-polymorphic in the practical Phase 1 sense: it only asks for `file.read`, so it can run inside any larger effect scope that also happens to include `network`, `ui`, `db`, or other authority. A dedicated row-polymorphic syntax for naming an open capability tail is intentionally deferred to a later milestone.

Pure functions carry no capability clause because they do not need external authority.

## Local scoping

`with { ... } in expr` lexically narrows the visible capability set for `expr`:

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
- local scoping does **not** install implementations, mocks, or handlers; it only reduces the authority already in scope
- later handler / interpreter syntax should reuse these same capability names and lexical scopes rather than inventing a second mechanism

## Resources and cancellation

`Resource E A` accepts the same clause shape as `Effect E A`:

```aivi
openAuditLog : Text -> Resource FileError Handle with { file.write }
```

The clause covers the whole resource lifecycle:

- acquisition before `yield`
- release after `yield`
- helper effects used inside the resource body

Resource cleanup remains cancellation-protected automatically. Authors do **not** add `cancellation.mask` merely to obtain ordinary finalizer guarantees. Explicit cancellation-control APIs (`scope`, `spawn`, `race`, future masking forms, task cancellation) are the places where `cancellation.*` becomes part of the public signature.

## Mapping existing ambient APIs

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
| `gtkApp`, `signalStream`, `reconcileNode`, clipboard / notification helpers | `ui` |
| `@static` embedded sources | no runtime capability after compilation |

## Migration from ambient builtins

1. Keep the existing term names. Phase 1 changes the **type surface** first, not the call syntax.
2. Annotate public `Effect` and `Resource` signatures with the smallest capability set they require.
3. Prefer narrow leaves (`file.read`) over broad families (`file`) unless the API genuinely needs the whole family.
4. Use `with { ... } in` to narrow large helper bodies and make accidental authority creep visible in code review.
5. Continue using `mock ... in` for test substitution until the dedicated handler / interpreter milestone lands.
6. Later compiler and LSP work will turn these capability contracts into first-class diagnostics and quick-fixes.
