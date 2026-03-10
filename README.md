# AIVI

> [!CAUTION]
> This is a vibe coded project. Do not use for anything serious. I'm still rewriting a lot of fundamentals.

<p align="center">
  <img src="assets/aivi-logo.png" alt="AIVI logo" width="120" />
</p>

**AIVI** is a purely functional language that compiles to native binaries via Rust/Cranelift. It's designed for developers who want strong static types, explicit effects, and a first-class GTK4 UI story — without writing C, C++, or Rust as their day-to-day language.

---

## Why now?

The open desktop is having a moment. Governments, enterprises, and individuals across Europe are actively looking to reduce dependency on proprietary American platforms. Linux is growing. GNOME is maturing. GTK4 is a genuinely excellent toolkit — but building apps on top of it in C or even Rust requires significant boilerplate, manual memory management discipline, and a steep learning curve.

**AIVI wants to fix that.** It sits at the sweet spot between "high-level enough to move fast" and "low-level enough to ship a real native app":

- 🦀 **Rust-powered runtime** — no GC pauses, no Electron bloat, no 200 MB runtime. Your app links against GTK4 directly.
- 🧠 **Purely functional** — immutable by default, exhaustive pattern matching, no nulls, typed errors. The compiler catches whole classes of bugs before they ship.
- 🖥️ **GTK4 as a first-class citizen** — a dedicated XML sigil, first-class `Signal` / `Event` values, direct widget bindings, and mounted structural updates make native UI code feel like it belongs in the language.
- 📦 **Schema-first data pipelines** — typed `Source K A` declarations keep connector config, schema, transforms, and validation visible before `load` performs any effect.
- ⚡ **Developer experience that doesn't quit** — a built-in LSP with workspace-aware incremental checking, autocomplete, hover, diagnostics, formatting, and VS Code integration.

---

## A taste of the language

AIVI is expression-oriented with a clean, minimal syntax. Bindings are immutable, functions are curried, and effects are tracked in the type system.

```ocaml
module user.myapp

use aivi
use aivi.rest

// Types are structural records and ADTs
User = { id: Int, name: Text, email: Option Text }

ApiError = NotFound | Timeout | ParseError Text

// Functions look like math
greet : User -> Text
greet = user => "Hello, { user.name }!"

// Source declarations stay pure; `load` is still the only effectful step
userSource : Int -> Source RestApi User
userSource = id =>
  rest.get {
    url: ~u(https://api.example.com/users/{ id }),
    schema: source.schema.derive,
    strictStatus: True
  }

fetchUser : Int -> Effect ApiError User
fetchUser = id => do Effect {
  load (userSource id)
}

// Pattern matching is exhaustive — the compiler rejects non-exhaustive cases
describe : ApiError -> Text
describe =
  | NotFound      => "user not found"
  | Timeout       => "request timed out"
  | ParseError m  => "bad response: { m }"
```

No null checks. No forgotten error branches. The compiler makes the happy path and every failure mode equally visible.

---

## GTK4 native apps — without the ceremony

This is where AIVI really shines. The `~<gtk>...</gtk>` sigil lets you write GTK4 UI trees inline, with full type checking, signal wiring, and dynamic child lists.

### Signal-first native apps

The primary public UI story is signal-first: state lives in `Signal` values, GTK props and child structure bind directly to those signals, and callbacks mutate signals or trigger `Event` handles.

```ocaml
use aivi.reactive

state = signal { count: 0 }
countLabel = derive state (s => "Count: {s.count}")
increment = _ => update state (patch { count: _ + 1 })

view = ~<gtk>
  <GtkBox orientation="vertical" spacing="12" marginTop="16" marginStart="16">
    <GtkLabel label={countLabel} />
    <GtkButton label="Increment" onClick={increment} />
  </GtkBox>
</gtk>

main : Effect GtkError Unit
main = do Effect {
  _ <- init Unit
  appId <- appNew "com.example.counter"
  win <- windowNew appId "Counter" 480 240
  root <- buildFromNode view
  _ <- windowSetChild win root
  _ <- windowPresent win
  appRun appId
}
```

Lower-level `signalStream`, `signalPoll`, `gtkSetInterval`, `buildWithIds`, and `reconcileNode` remain available for custom hosting and integrations, but the normal app shape is: mount once with `buildFromNode`, then let signals keep the widgets live.

### Dynamic lists with `<each>`

```ocaml
renderTodos : List Text -> GtkNode
renderTodos = items =>
  ~<gtk>
    <GtkBox orientation="vertical" spacing="4">
      <each items={items} as={item}>
        <GtkLabel label={ item } xalign="0" />
      </each>
    </GtkBox>
  </gtk>
```

One list value, one `<each>` — no manual widget creation loops, no index tracking.

---

## Structural record patching

State updates read like plain data declarations, not mutating assignments. The `<|` operator applies a type-checked patch:

```ocaml
// Update a single field
newState = state <| { title: "New Title" }

// Update a nested field
newState = state <| { user.profile.avatar: "new.png" }

// Transform all items in a list
discounted = cart <| { items[*].price: _ * 0.9 }  // 10% off

// Target by predicate
flagged = cart <| { items[price > 80].tag: "premium" }
```

The compiler verifies every path exists and every value matches the expected type.

---

## Domains: units baked into the type system

AIVI's domain system lets you give meaning to numeric literals and operators at the type level. Common suffixes from the standard library just work:

```ocaml
use aivi.chronos.duration (domain Duration)
use aivi.color (domain Color)

// Typed duration literals — not raw numbers
timeout  = 30s
debounce = 200ms
animDur  = 0.3s

// Color math with perceptual adjustments (l=lightness, s=saturation, h=hue)
hoverColor = brand + 10l   // brighter variant
mutedColor = brand - 30s   // desaturated variant
shiftedHue = brand + 45h   // hue rotation
```

You can define your own domains to give operators meaning in your problem space — pixel coordinates, monetary values, physical units, anything.

---

## The standard library

The stdlib covers the typical surface you need to ship a real app. A few highlights:

| Area            | Modules                                                                          |
|:--------------- |:-------------------------------------------------------------------------------- |
| **Collections** | `list`, `map`, `set`, `queue`, `heap`                                            |
| **Text**        | `text`, `regex`, `i18n`                                                          |
| **Time**        | `chronos.instant`, `chronos.duration`, `chronos.calendar`, `chronos.timezone`    |
| **Math**        | `math`, `vector`, `matrix`, `geometry`, `probability`, `signal`, `linearAlgebra` |
| **I/O**         | `file`, `console`, `database`, `database.pool`, `path`, `url`                    |
| **Network**     | `net.http`, `net.https`, `net.httpServer`, `net.rest`, `net.sockets`             |
| **Concurrency** | `concurrency` (scoped tasks, typed channels, `Send`/`Recv`)                      |
| **System**      | `system`, `crypto`, `secrets`, `log`                                             |
| **UI**          | `ui.gtk4`, `ui.color`, `ui.layout`, `ui.html`, `ui.vdom`                         |

The `aivi.concurrency` module gives you scoped tasks and typed channels — real async concurrency modelled as values, without shared mutable state.

---

## Schema-first sources and typed pipelines

Phase 3 keeps `Source K A` and `load`, but moves schema and connector information onto the declaration itself so tooling can explain the contract before runtime:

```ocaml
use aivi.validation

User = { id: Int, name: Text }

nonEmpty : List A -> Validation (List DecodeError) (List A)
nonEmpty = xs =>
  if List.length xs == 0 then
    Invalid [{ path: [], message: "expected at least one row" }]
  else
    Valid xs

usersSource : Source File (List User)
usersSource =
  file.json {
    path: "./users.json",
    schema: source.schema.derive
  }
    |> source.validate nonEmpty

usersCount : Effect (SourceError File) Int
usersCount = do Effect {
  users <- load usersSource
  pure (List.length users)
}
```

The record forms of `file.json`, `env.decode`, and `rest.get` are now the preferred public surface. They line up with source hovers/diagnostics in the LSP and with the wider Phase 3 source-pipeline story in `specs/syntax/external_sources/`.

---

## Effects, resources, and typed data

Effects are part of the type. You can't accidentally call an effectful function in a pure context, and error types are tracked like any other:

```ocaml
use aivi.database as db

// Schema is a typed value — the compiler knows the row shape
User = { id: Int, name: Text, active: Bool, createdAt: Instant }

@static
userTable : Table User
userTable = db.table "users" [
  { name: "id",        type: IntType,       constraints: [AutoIncrement, NotNull] }
  { name: "name",      type: Varchar 100,   constraints: [NotNull] }
  { name: "active",    type: BoolType,      constraints: [NotNull] }
  { name: "createdAt", type: TimestampType, constraints: [NotNull], default: Some DefaultNow }
]

// Query returns typed rows — pipe directly into list operations
getActiveUsers : Effect DbError (List User)
getActiveUsers = do Effect {
  _ <- db.configure { driver: Sqlite, url: "./local.db" }
  _ <- db.runMigrations [userTable]
  users <- db.load userTable
  pure (users |> filter active |> sortBy.createdAt)
}

// Precondition guards read like prose
withdraw : Float -> Account -> Effect BankError Account
withdraw = amount => account => do Effect {
  given amount > 0                or fail (InvalidAmount amount)
  given account.balance >= amount or fail InsufficientFunds
  pure (account <| { balance: account.balance - amount })
}
```

### Typed API clients from OpenAPI specs — at compile time

Point AIVI at any OpenAPI spec and it generates a fully typed client, checked at compile time. No code generation step, no stale types:

```ocaml
// Fetched and type-checked at compile time; zero runtime overhead
@static
api = openapi.fromUrl ~url(https://petstore.swagger.io/v2/swagger.json)

main = do Effect {
  // Return type is inferred from the spec — no json.decode needed
  pets <- api.listPets { limit: Some 10 }
  print "Found { pets |> length } pets: { pets |> map name }"

  newPet <- api.createPet { name: "Fido", tag: Some "dog" }
  print "Created: { newPet.name } (id: { newPet.id })"
}
```

## Incremental workspace tooling

Phase 4 also tightens the editor/compiler story:

- the LSP works over **workspace snapshots**, not isolated files,
- open buffers shadow on-disk files for that snapshot,
- cached checkpoints are reused only when their fingerprints still match,
- dependent modules are rechecked incrementally when an edited module's export surface or exported schema summary changes.

That same tooling layer now knows about the signal-first GTK and schema-first source stories: hover/completion cover `Signal`, `Event`, `buildFromNode`, `signalStream`, GTK callback attrs, `file.json`, `env.decode`, `source.transform`, `source.validate`, and `source.schema.derive`.

For concrete proof points, see `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi` for signal-first GTK coverage and `integration-tests/runtime/source_pipeline.aivi` for the schema-first source pipeline slice.

Works equally well with a local file: `openapi.fromFile "./specs/api.yaml"`. The same type-driven decoding applies everywhere — HTTP sources, file sources, environment variables — if the type is known at the call site, AIVI decodes into it automatically.

---

## Tooling

- **LSP server** (`aivi-lsp`) — autocomplete, hover with inline docs, go-to-definition, rename, real-time diagnostics, semantic highlighting; works with VS Code, Neovim, Zed, and any LSP-compatible editor
- **Formatter** — `aivi fmt` formats any `.aivi` file to canonical style; also available as a format-on-save action in the VS Code extension
- **VS Code extension** — bundles the LSP, grammar highlighting, and formatting in one install
- **MCP server** — `aivi mcp serve` exposes the language specs as MCP resources for AI-assisted development workflows; add `--ui` to also expose underscore-safe GTK inspection and driver tools such as `aivi_gtk_launch`, `aivi_gtk_focus`, `aivi_gtk_moveFocus`, `aivi_gtk_scroll`, and `aivi_gtk_keyPress` for widget trees, richer live state, keyboard/focus navigation, scrolling, local UI actions, and keypress injection (legacy dotted spellings remain accepted, and effectful tools require `--allow-effects`)

---

## Getting started

You need Rust installed. Then:

```bash
cargo install --path crates/aivi

aivi --help
```

From there:

```bash
# Run a file
aivi run myapp.aivi

# Format code
aivi fmt myapp.aivi

# Start the language server (for editors)
aivi-lsp

# Explore the language specs
ls specs/
```

The `specs/` folder is a VitePress site — run `pnpm docs:dev` inside it to browse the full language and stdlib documentation locally.

---

## The bigger picture

Electron gave us cross-platform apps at the cost of hundreds of megabytes of runtime and noticeable input lag. Native toolkits give us performance but ask you to write C. AIVI believes there's a better path: **a high-level, safe, functional language that targets the native Linux desktop directly.**

GTK4 and GNOME have never been better. The Linux desktop ecosystem is getting real investment. And there's a growing appetite — especially in Europe — for software infrastructure that isn't controlled by a handful of American platform vendors.

AIVI wants to be the language that makes building that software a pleasure.

---

## Status

Early but active. The core language, typechecker, formatter, and LSP are functional. GTK4 bindings are being expanded. The Cranelift JIT/AOT backend is in progress. Expect breaking changes.

Contributions, feedback, and bug reports are welcome.
