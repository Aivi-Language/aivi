# Plan: AIVI standard library scope

## Status: design draft - not yet implemented

---

## 1. Goal

Define a small, typed, GNOME-first standard library that matches the current
language model in `AGENTS.md` and `AIVI_RFC.md`.

The stdlib should center four ideas:

- pure value-level programming by default
- one-shot effects through `Task E A`
- long-lived external input through `sig` plus `@source`
- explicit domain-backed wrappers for values such as `Duration`, `Url`, and `Path`

The initial stdlib should be opinionated and narrow. It should provide the
pieces needed for native desktop applications without turning the language into
a grab bag of unrelated utility packages.

---

## 2. Architectural rules

These rules are normative for the first stdlib wave.

1. Pure helpers stay pure. They must not hide runtime handles, blocking I/O, or
   mutable state.
2. One-shot external work uses `Task E A`.
3. Long-lived subscriptions, polling, watches, and event feeds use `@source`
   providers.
4. Source options are closed and typed. Unknown or duplicate options are errors.
5. Source decoding is strict by default and uses typed error channels.
6. Public surfaces should expose domain values rather than raw carrier types
   when invariants matter.
7. GTK, D-Bus, network clients, file watching, and similar runtime integrations
   remain behind controlled effect or source boundaries.
8. The stdlib must not re-expose runtime internals as public APIs:
   - no public mutable `Signal` API
   - no public scheduler API
   - no generic UI tree API
   - no general resource choreography as the default user model
9. Prefer one canonical surface per capability. Avoid duplicated umbrella
   namespaces.

---

## 3. First-wave modules to implement

### 3.1 Core export surface

#### `aivi`

Keep the root surface small.

It should export only:

- core types committed to by the RFC
- core constructors such as `Some`, `None`, `Ok`, `Err`, `Valid`, `Invalid`
- the small class surface that the language commits to

It should **not** become a namespace full of runtime handles or subsystem
facades.

#### `aivi.prelude`

Provide a compact import surface for ordinary programs.

The prelude should re-export:

- primitive types
- `List`, `Option`, `Result`, `Validation`, `Signal`, `Task`
- the core class surface actually required by the RFC
- a minimal set of high-value helpers

The prelude should stay intentionally small.

### 3.2 Pure foundation modules

These modules should be part of the first implementation wave.

| Module | What to implement | Shape notes |
| --- | --- | --- |
| `aivi.defaults` | `Default` instance bundles | The first required bundle is `Option`. Record omission support should rely on this module. |
| `aivi.list` | compact list helper set | Focus on traversal, search, partitioning, zipping, and safe access. |
| `aivi.option` | compact option helper set | `isSome`, `isNone`, `getOrElse`, and conversion helpers. |
| `aivi.result` | compact result helper set | `isOk`, `isErr`, `mapErr`, and conversion helpers. |
| `aivi.validation` | applicative validation surface | Match RFC accumulation semantics with `NonEmptyList`. |
| `aivi.text` | Unicode-safe text and encoding helpers | Keep it focused on text operations, encoding, and parsing helpers that clearly belong here. |
| `aivi.duration` | domain-backed duration type | Explicit constructors, explicit `value`, literal suffixes, and domain-local operators. |
| `aivi.url` | domain-backed URL type | Explicit parse and explicit unwrap. |
| `aivi.path` | domain-backed path type | Explicit parse, explicit unwrap, and path-join operator. |
| `aivi.color` | domain-backed color type | Keep it small and GTK-friendly. |
| `aivi.nonEmpty` | `NonEmpty` / `NonEmptyList` | Needed to make `Validation` match the RFC cleanly. |

### 3.3 Domain shapes

The domain modules should follow the RFC's explicit-construction model.

#### `aivi.duration`

Recommended surface:

```aivi
domain Duration over Int
    literal ms  : Int -> Duration
    literal sec : Int -> Duration
    literal min : Int -> Duration
    millis      : Int -> Duration
    trySeconds  : Int -> Result DurationError Duration
    value       : Duration -> Int
    (+)         : Duration -> Duration -> Duration
    (-)         : Duration -> Duration -> Duration
```

#### `aivi.url`

Recommended surface:

```aivi
domain Url over Text
    parse : Text -> Result UrlError Url
    value : Url -> Text
```

Add only focused helpers that preserve the explicit domain model.

#### `aivi.path`

Recommended surface:

```aivi
domain Path over Text
    parse : Text -> Result PathError Path
    (/)   : Path -> Text -> Path
    value : Path -> Text
```

Path normalization should be part of the domain's invariant story, not a loose
string helper.

#### `aivi.color`

Use a domain-backed color representation with a small constructor and unwrap
surface. The goal is to support GTK-facing style and property work, not to ship
an extensive graphics toolkit.

---

## 4. Runtime boundary surfaces

### 4.1 HTTP

Implement HTTP as:

- a typed request/response surface
- one-shot `Task` entry points for imperative use
- an `@source` provider family for reactive use

Required user-facing source surface:

```aivi
@source http.get "/users"
sig users : Signal (Result HttpError (List User))

@source http.post "/login" with {
    body: creds,
    headers: authHeaders,
    decode: Strict,
    timeout: 5sec
}
sig login : Signal (Result HttpError Session)
```

Required option concepts:

- `headers`
- `query`
- `body`
- `decode`
- `timeout`
- `retry`
- `refreshOn`
- `refreshEvery`
- `activeWhen`

Required runtime behavior:

- request-like sources must cancel in-flight work or mark stale results so they
  cannot publish into the live graph
- reconfiguration must be transactional
- decoding happens before publication
- failures stay typed

### 4.2 Filesystem

Implement filesystem support as two distinct source families plus a small task
surface.

Required source surface:

```aivi
@source fs.watch configPath with {
    events: [Created, Changed, Deleted]
}
sig fileEvents : Signal FsEvent

@source fs.read configPath with {
    decode: Strict,
    reloadOn: fileEvents
}
sig fileText : Signal (Result FsError Text)
```

Required rules:

- `fs.watch` publishes events only
- `fs.read` publishes snapshots only
- reads and watches are separate concepts
- file path inputs should use the `Path` domain where practical

The task surface should stay small and explicit:

- write text or bytes
- delete
- create directories if needed
- optionally rename or copy if clearly justified

### 4.3 Timer

Implement a dedicated `timer` provider family.

Required surface:

```aivi
@source timer.every 120ms
sig tick : Signal Unit

@source timer.after 1sec
sig ready : Signal Unit
```

Required option concepts:

- `immediate`
- `jitter`
- `coalesce`
- `activeWhen`

### 4.4 Logging

Implement a minimal structured logging surface under `aivi.log`.

It should support:

- a closed log-level enum
- message text
- structured key-value context
- one-shot logging tasks

This surface is for tracing, diagnostics, and application logs. It should stay
small and not grow into a general observability framework.

---

## 5. GNOME-first integration surfaces

### 5.1 `aivi.gnome.onlineAccounts`

This module should provide a typed GNOME Online Accounts boundary.

Its design should be account-centric, not protocol-centric.

Required concepts:

- account identity
- provider identity
- capability filtering
- attention-needed state
- typed account listing
- typed credential refresh
- typed token retrieval where GOA supports it
- account change observation

Recommended shape:

```aivi
type GoaAccountId
type GoaCapability
type GoaProvider

type GoaAccount = {
    id: GoaAccountId,
    provider: GoaProvider,
    label: Text,
    capabilities: Set GoaCapability,
    attentionNeeded: Bool
}

type GoaError = ...

@source goa.accounts with {
    capability: Mail
}
sig accounts : Signal (Result GoaError (List GoaAccount))

ensureCredentials : GoaAccountId -> Task GoaError Unit
accessToken       : GoaAccountId -> Task GoaError AccessToken
```

Implementation guidance:

- use D-Bus internally
- keep D-Bus details out of the language-facing types
- expose only typed account and credential concepts
- publish account changes through a source, not polling hidden inside helpers

---

## 6. What is not in the first stdlib wave

The first wave should stay focused. The following areas are out of scope unless
later work proves they are necessary:

- database abstraction layers
- IMAP/SMTP or other mail protocol clients
- generic secret-storage APIs
- raw sockets and generic streaming APIs
- general HTTP server frameworks
- PKCE and localhost loopback auth helpers
- public signal or scheduler manipulation APIs
- UI tree or form helper DSLs
- broad math, graph, geometry, matrix, vector, or linear-algebra libraries
- large generic crypto toolkits

These capabilities can be reconsidered later, but they should not shape the v1
stdlib architecture.

---

## 7. Later phases

These are reasonable follow-on candidates after the first wave is stable:

- raw JSON escape hatch APIs
- regex
- testing helpers
- gettext-oriented i18n
- limited process and mailbox provider surfaces
- carefully scoped system access
- GNOME-native secret-store integration if real needs appear
- PKCE or other localhost loopback auth support if a concrete integration needs it
- calendar and time-zone support once the domain and source foundations are solid

Later work should reuse the same rules:

- pure helpers stay pure
- one-shot work uses `Task`
- long-lived input uses `@source`
- no duplicate facades

---

## 8. Implementation order

### Phase 1: core foundation

- `aivi`
- `aivi.prelude`
- `aivi.defaults`
- `aivi.list`
- `aivi.option`
- `aivi.result`
- `aivi.validation`
- `aivi.nonEmpty`
- `aivi.text`
- `aivi.duration`
- `aivi.url`
- `aivi.path`
- `aivi.color`

### Phase 2: source and task boundaries

- HTTP types plus `http` provider family
- filesystem types plus `fs.read` and `fs.watch`
- timer provider family
- minimal `aivi.log`
- typed decode support and source option types

### Phase 3: GNOME-native account support

- `aivi.gnome.onlineAccounts`
- internal D-Bus plumbing needed for that surface

### Phase 4: later expansions

- JSON
- regex
- testing
- i18n
- process and mailbox providers
- limited system and secret-store integrations
- optional PKCE or localhost loopback auth helper

---

## 9. Definition of done

This plan is complete only when the implementation follows these constraints:

1. The first stdlib wave is small and coherent.
2. Public APIs clearly separate pure helpers, `Task` work, and `@source`
   providers.
3. Domain-backed values enforce explicit construction and explicit unwrapping.
4. No umbrella duplicate namespaces are introduced.
5. No public API re-exposes signal mutation, scheduler control, or UI tree
   machinery.
6. GOA support matches the GNOME-first philosophy and remains typed, narrow, and
   deterministic.
7. Tests cover:
   - domain invariants
   - strict decode behavior
   - source reconfiguration and stale-result suppression
   - GOA account change delivery

---

## 10. Final recommendation

Implement the smallest stdlib that makes the current language real:

- a strong pure foundation
- explicit domains
- source-first external input
- task-based one-shot effects
- GNOME Online Accounts

Everything else should wait until it is justified by the current architecture.
