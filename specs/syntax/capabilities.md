# Capabilities

<!-- quick-info: {"kind":"topic","name":"capabilities"} -->
Capabilities say which outside-world operations an `Effect E A` or `Resource E A` may use.
<!-- /quick-info -->

Capabilities answer a practical question: **what is this code allowed to touch?** They describe external authority such as file access, network access, clocks, UI integration, or process state.

## Think of capabilities as a permission list

A capability clause sits on top of `Effect E A` or `Resource E A` as a permission list:

- `E` still describes typed domain failures
- `A` is still the success value
- capabilities describe which external operations the code may use
- missing capabilities are compile-time diagnostics, not runtime `E` values

`attempt`, fallback `or`, `resource`, and `do Effect { ... }` keep the semantics described in [Effects](effects.md) and [Resources](resources.md). A capability clause only narrows the external authority available inside that code.

## How capabilities look in code

Write capability requirements as a postfix clause on an effectful type.

```aivi
readConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }

openStore : DbConfig -> Resource DbError DbConn with { db.connect }

launchUi : Model -> Effect GtkError Unit with { ui.window, ui.signal }
```

Here is a slightly larger example:

<<< ../snippets/from_md/syntax/capabilities/block_02.aivi{aivi}


That signature tells the reader two things at once:

- the domain error is `ProfileError`
- the function needs permission to use `network.http` and `file.write`

## How to read a capability signature

A capability clause describes the **minimum** authority a function needs. Callers may always run that function from a scope that has a **superset** of the listed capabilities.

<<< ../snippets/from_md/syntax/capabilities/block_03.aivi{aivi}


`loadUserCache` only asks for `file.read`, so it can run inside a larger scope that also includes `network.http`, `ui`, `db`, or other authority. The practical rule is simple: **smaller requirement sets compose into larger ones**.

Pure functions carry no capability clause because they do not need outside-world authority.

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

Family names such as `network`, `process`, or `ui` also work as broader shorthands for everything beneath them. In public APIs, prefer the narrowest leaf that states the real requirement.

## How to use capabilities

### 1. Annotate effectful public APIs

Put capability clauses on public `Effect` and `Resource` signatures.

```aivi
readConfig : Text -> Effect ConfigError AppConfig with { file.read, process.env.read }
saveDraft : Text -> Effect FileError Unit with { file.write }
openStore : DbConfig -> Resource DbError DbConn with { db.connect }
```

### 2. Narrow helper bodies with `with { ... } in`

`with { ... } in expr` narrows the visible capability set for `expr`.

<<< ../snippets/from_md/syntax/capabilities/block_05.aivi{aivi}


Rules:

- the inner scope may use only the listed capabilities
- nested `with` blocks intersect, so an inner scope can narrow further but never widen authority
- this form only narrows authority; it does not install implementations by itself
- `with { capability = handler } in expr` uses the same surface form to install scoped interpreters; see [Effect Handlers](effect_handlers.md)

### 3. Keep failures and permissions separate

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

For the full host model, see [GTK App Architecture](../stdlib/ui/app_architecture.md) and [`aivi.ui.gtk4`](../stdlib/ui/gtk4.md).

- `gtkApp` is the coarse-grained `ui` entry point
- low-level widget and window construction/presentation use `ui.window`
- signal helpers such as `signalPoll`, `signalStream`, and `signalEmit` use `ui.signal`
- clipboard helpers such as `clipboardDefault`, `clipboardSetText`, and `clipboardText` use `ui.clipboard`
- notification helpers such as `notificationNew`, `notificationSetBody`, and app notification delivery use `ui.notification`
- `commandPerform` inherits the capabilities of the enclosed `Effect`
- `commandAfter` and `subscriptionEvery` use `clock.sleep` or `clock.schedule`
- `commandCancel` and keyed subscription/timer replacement use `cancellation.propagate`
- `subscriptionSource` inherits the capabilities of its underlying `Resource`

This keeps UI code aligned with the rest of the language instead of introducing a separate permission model.

## How today’s APIs map to capabilities

These are the most common current surface mappings, not an exhaustive list.

| Current surface | Capability requirement |
| --- | --- |
| `load (file.*)`, `load (file.image*)`, `file.readText`, `file.readJson`, `file.readCsv` | `file.read` |
| `file.writeText`, `file.writeBytes`, `file.appendText`, `file.delete`, `file.mkdir*`, `file.rmdir` | `file.write` |
| `file.exists`, `file.stat`, `file.listDir` | `file.metadata` |
| `file.copy`, `file.move` | `file.read` + `file.write` |
| `rest.*`, `http.*`, `https.*` | `network.http` |
| `email.imap*`, `smtpSend` | `network` |
| `listen` (sockets) | `network.socket.listen` (or broader `network`) |
| `connect` (sockets) | `network.socket.connect` (or broader `network`) |
| `accept`, `send`, `recv`, `close` (sockets) | `network` |
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
| low-level widget/window helpers in `aivi.ui.gtk4` | `ui.window` (or broader `ui`) |
| `signalPoll`, `signalStream`, `signalEmit` | `ui.signal` (or broader `ui`) |
| `clipboardDefault`, `clipboardSetText`, `clipboardText` | `ui.clipboard` (or broader `ui`) |
| `notificationNew`, `notificationSetBody`, `appSendNotification`, `appWithdrawNotification` | `ui.notification` (or broader `ui`) |
| `gtkApp` | `ui` |
| `commandPerform { run, ... }` | capabilities required by the enclosed `Effect` |
| `commandAfter`, `subscriptionEvery` | `clock.sleep` / `clock.schedule` (or broader `clock`) |
| `commandCancel` | `cancellation.propagate` |
| `subscriptionSource { open, ... }` | capabilities required by the underlying `Resource` |
| `@static` embedded sources | no runtime capability after compilation |

## Practical rules of thumb

1. Put capabilities on public `Effect` and `Resource` signatures.
2. Prefer narrow leaves such as `file.read` over broad family names such as `file`.
3. Use `with { ... } in` to make helper bodies’ authority explicit.
4. Use `with { capability = handler } in` when you want a scoped interpreter for a capability.
5. Treat capabilities as a permission list, not as part of your domain error model.
