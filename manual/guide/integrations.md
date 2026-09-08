# External integrations

AIVI keeps ordinary language code pure and crosses into the operating system through typed source
providers. A source declaration creates either a reactive value or a capability handle. Derived
signals remain pure: workers perform I/O and publish immutable updates, while the scheduler commits
those updates without blocking GTK's main thread.

This page focuses on the common integration shape. The complete provider and option inventory lives
in the [source catalog](source-catalog.md).

## Start with a typed boundary

Declare the smallest useful payload type at the source boundary. For an HTTP capability, create one
base handle and derive endpoint signals from it:

```aivi
type User = {
    id: Int,
    name: Text
}

@source http "https://api.example.com"
signal api : HttpSource

signal users : Signal (HttpResponse (List User)) = api.get "/users"
```

`HttpResponse A` is `Result HttpError A`, so transport and decode failures stay explicit. The source
runtime decodes successful responses into `List User`; downstream code never handles untyped JSON.

## Timers are sources

Timer values enter the same reactive graph as external I/O:

```aivi
use aivi.timer (TimerTick)

@source timer.every 500 with {
    immediate: False,
    coalesce: True
}
signal tick : Signal TimerTick
```

`immediate` controls whether the timer emits during startup. `coalesce` prevents a slow consumer
from accumulating an unbounded backlog of timer events.

For a one-shot wakeup, use `timer.after`:

```aivi
use aivi.timer (TimerReady)

@source timer.after 1000
signal ready : Signal TimerReady
```

## Filesystem capabilities

A filesystem source is a capability rooted at one directory. Its members expose reactive reads and
one-shot tasks without giving pure code ambient filesystem access:

```aivi
@source fs "."
signal workspaceFiles : FsSource

signal cargoToml : Signal (Result FsError Text) = workspaceFiles.read "Cargo.toml"

value outputExists : Task Text Bool = workspaceFiles.exists "target/output.txt"
```

Keep the root narrow. A second capability should be declared when another directory has a different
ownership or lifetime boundary.

## Process and environment context

Process, environment, and platform paths are separate capabilities so their costs and ownership are
visible:

```aivi
use aivi.process (ProcessSource)

use aivi.env (EnvSource)

use aivi.path (PathSource)

@source process
signal runtime : ProcessSource

@source env
signal hostEnv : EnvSource

@source path
signal systemPaths : PathSource

signal arguments : Signal (List Text) = runtime.args
signal workingDirectory : Signal Text = runtime.cwd
signal accessToken : Signal (Option Text) = hostEnv.get "ACCESS_TOKEN"
signal configDirectory : Signal Text = systemPaths.configHome
```

Read secrets only at the boundary that needs them, and keep the resulting signal out of logs and UI
trees.

## Database connections

Database sources use typed connection values and explicit `Result` payloads:

```aivi
use aivi.db (
    Connection
    DbError
)

value connection : Connection = {
    database: "sqlite:///var/lib/example/data.db"
}

@source db.connect connection
signal database : Signal (Result DbError Connection)
```

Use `db.live` for a task whose result should be republished when a trigger changes. Query arguments
belong in `DbStatement.arguments`; do not build SQL by interpolating untrusted text.

## Derive pure presentation state

Once a source has decoded its payload, ordinary signal transformations handle presentation state:

```aivi
type User = {
    id: Int,
    name: Text
}

@source http "https://api.example.com"
signal api : HttpSource

signal users : Signal (HttpResponse (List User)) = api.get "/users"

signal status : Signal Text = users
 ||> Ok loaded -> "Loaded {length loaded} users"
 ||> Err _     -> "Could not load users"
```

This separation is the central invariant:

- providers own I/O, cancellation, decoding, and worker lifetimes;
- the scheduler owns deterministic signal publication;
- pure functions own transformations;
- GTK creation and mutation stay on the GTK main thread.

## Choosing a provider

Use a built-in capability whenever one matches the external system. Current families include HTTP,
filesystem, database, D-Bus, environment, process, platform paths, random values, logging, standard
I/O, desktop portals, notifications, settings, and mail-related services. Provider-specific argument,
option, payload, and lifecycle rules are documented in the [source catalog](source-catalog.md).

For a provider implemented outside the repository, define and review its Rust-side contract first.
Do not invent an AIVI `@source` spelling in application code: an unknown provider is a compile-time
error.

See also [Sources](sources.md) for source semantics and [Signals](signals.md) for reactive
composition.
