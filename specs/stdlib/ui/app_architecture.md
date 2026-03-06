# GTK App Architecture

> **Status: Blessed Phase 2 boundary**  
> The `Model` / `View` / `Msg` / `Update` architecture described here is the single official GTK app pattern in AIVI. Commands, subscriptions, and forms are intentionally deferred to later milestones; when they land, they must extend this architecture rather than replace it.

<!-- quick-info: {"kind":"topic","name":"gtk app architecture"} -->
AIVI GTK applications have one public architecture: `Model` / `View` / `Msg` / `Update` hosted by `gtkApp`. Lower-level primitives such as `signalStream`, `buildFromNode`, and `reconcileNode` are escape hatches and implementation building blocks, not competing top-level app patterns.
<!-- /quick-info -->

## Blessed shape

Every standard single-window GTK app should be organized around these parts:

1. **`Model`** — the full state needed to render the current window.
2. **`view : Model -> GtkNode`** — a pure projection from state to GTK node tree.
3. **`Msg`** — a closed ADT describing domain events that matter to the app.
4. **`toMsg : GtkSignalEvent -> Option Msg`** — an adapter from low-level GTK signals into domain messages.
5. **`update : Msg -> Model -> Effect GtkError Model`** — the only steady-state transition function.
6. **`gtkApp`** — the runtime host that wires startup, rendering, event ingestion, and reconciliation together.

```aivi
Msg = Increment | Reset

view : { count: Int } -> GtkNode
view = state => ~<gtk>
  <GtkBox orientation="vertical" spacing="8">
    <GtkLabel label={ Int.toString state.count } />
    <GtkButton id="incBtn" label="Increment" onClick={ Increment } />
    <GtkButton id="resetBtn" label="Reset" onClick={ Reset } />
  </GtkBox>
</gtk>

toMsg : GtkSignalEvent -> Option Msg
toMsg = event =>
  event match
    | GtkClicked _ "incBtn"   => Some Increment
    | GtkClicked _ "resetBtn" => Some Reset
    | _                       => None

update : Msg -> { count: Int } -> Effect GtkError { count: Int }
update = msg => state =>
  msg match
    | Increment => pure (state <| { count: state.count + 1 })
    | Reset     => pure (state <| { count: 0 })

main : Effect GtkError Unit
main = gtkApp {
  id:      "com.example.counter",
  title:   "Counter",
  size:    (480, 240),
  model:   { count: 0 },
  onStart: _ _ => pure Unit,
  view:    view,
  toMsg:   toMsg,
  update:  update
}
```

## Runtime flow

`gtkApp` executes the architecture in this order:

1. initialize GTK and create the application/window,
2. run `onStart` once,
3. build the initial `view model`,
4. attach the root widget to the window,
5. open a `signalStream`,
6. translate each `GtkSignalEvent` through `toMsg`,
7. call `update` for each accepted `Msg`,
8. reconcile the new `view` with `reconcileNode`.

This gives AIVI one official mental model:

- **GTK emits signals**
- **the app maps signals into `Msg`**
- **`update` computes the next `Model`**
- **`view` re-renders from that `Model`**

## Startup hooks and options

### `onStart`

`onStart : AppId -> WindowId -> Effect GtkError Unit` is the blessed startup boundary. Use it for one-time boot work such as:

- registering application CSS with `appSetCss`,
- starting timers like `gtkSetInterval`,
- registering actions/shortcuts,
- applying initial window configuration that is not driven by the app model.

`onStart` is **not** a second steady-state update loop. After startup, business logic should continue to flow through `Msg` and `update`.

### Advanced options

The public architecture does **not** define a second top-level API for uncommon window flags or update-time access to GTK handles. Today, the module still exports `gtkAppFull`, but it is a deprecated compatibility shim rather than a second blessed path.

Use `gtkAppFull` only when you are blocked by a capability that the blessed architecture does not yet expose, such as:

- uncommon window flags like `decorated` / `hideOnClose`,
- legacy code that still requires `AppId` or `WindowId` inside `update`.

New specs, demos, examples, and application code should teach and prefer `gtkApp`.

## Relation to lower-level primitives

### `signalStream`

`signalStream` is the low-level event source used underneath `gtkApp`. It is appropriate for:

- custom runtime loops,
- library/framework internals,
- experiments that intentionally bypass the blessed architecture,
- advanced multi-window flows not yet modeled by `gtkApp`.

It is **not** the recommended starting point for standard applications.

### `reconcileNode`

`reconcileNode` is the rendering primitive used by `gtkApp` after each successful `update`. Use it directly only when you are manually hosting a GTK tree and need explicit control over patching.

### `buildFromNode`

`buildFromNode` is the initial mount primitive. It belongs in low-level code, libraries, and manual escape-hatch flows. Apps that fit the blessed architecture should let `gtkApp` call it for them.

## Boundary for later milestones

This milestone intentionally stops at the `Model` / `View` / `Msg` / `Update` core plus `onStart`.

- **Commands** will later extend the effect story after `update`.
- **Subscriptions** will later provide an official home for timers, background work, and external event feeds.
- **Forms/validation** will later layer on top of `Model` and `Msg`, not invent a second app architecture.

Those additions must remain compatible with this page's vocabulary and keep `gtkApp` as the one public host for standard GTK apps.
