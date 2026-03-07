# AIVI Phase 4 Reactive Implementation Map

## Overview

Phase 4 introduces **pure memoized computed signals** sitting on top of the existing `gtkApp` architecture. This is a memoization layer inside the `Model -> View -> Msg -> Update` loop, not a second effect system or FRP library.

---

## 1. Spec Details & Public API Names

### Spec Location & Key References

| Component | File | Lines | Status |
|-----------|------|-------|--------|
| **Reactive Dataflow** | `specs/stdlib/ui/reactive_dataflow.md` | 1–159 | ✓ Spec'd, Phase 4 model specified |
| **App Architecture** | `specs/stdlib/ui/app_architecture.md` | 83–100 | ✓ Includes reactive integration points |
| **Forms integration** | `specs/stdlib/ui/forms.md` | 140–148 | ✓ Field state + computed errors |

### Core Vocabulary (Spec Lines 17–26)

- **Source value**: Authoritative snapshot in committed `Model` (e.g., `query`, `rows`, `status`)
- **Derived value**: Pure projection over sources or other derived values (plain helper function)
- **Signal**: Named read-only derived value with no side effects
- **Computed signal**: Signal with **stable identity** and **memoization** — the host tracks dependency revisions and reuses cached results until dependencies change

### Public API (To Implement)

```aivi
-- Export from aivi.ui.gtk4

Signal a                                    -- opaque memoized value
computed : Text -> (Model -> a) -> Signal a -- create named computed signal
signal : Text -> (Model -> a) -> Signal a   -- alias for computed
readSignal : Signal a -> Model -> a         -- evaluate signal against model (uses memo)
```

### Semantic Boundaries (Spec Lines 115–128)

| Layer | Owns State? | Can Use IO? | Can Read Signals? | When? | Lifetime |
|-------|-----------|----------|----------------|-------|----------|
| **Reactive values** | No; reads committed snapshots | No | Yes (other signals) | Sync during view/subscriptions | Current turn + memo cache |
| **Effects/Commands** | No; capture snapshots | Yes; Effect/Resource | No; read plain Model | After update commits | One-shot or keyed task |
| **Subscriptions** | No; describe producers | Yes; Resource | **Yes, to decide membership** | Installation/diffing at boundaries | Long-lived until removed |

**Critical rules**:
- Effects capture **snapshots**, not live signals
- Subscriptions use signals to **decide which exist**, not to keep them alive
- No subscription/effect can mutate signal caches directly

---

## 2. Current Implementation Baseline

### GTK App Architecture (Fully Implemented)

**File**: `crates/aivi/src/stdlib/gtk4.rs` (lines 1–920)

#### Type Definitions (lines 150–169)
```aivi
AppStep model msg = { model: model, commands: List (Command msg) }
CommandKey = Text
SubscriptionKey = Text
```

#### gtkApp Signature (lines 870–880)
```aivi
gtkApp : {
  id: Text, title: Text, size: (Int, Int),
  model: s, onStart: AppId -> WindowId -> Effect GtkError Unit,
  subscriptions: s -> List (Subscription msg),
  view: s -> GtkNode,
  toMsg: GtkSignalEvent -> Option msg,
  update: msg -> s -> Effect GtkError (AppStep s msg)
} -> Effect GtkError Unit
```

#### Command & Subscription Types (lines 120–148)
- **Commands**: `CommandNone`, `CommandBatch`, `CommandEmit`, `CommandPerform`, `CommandAfter`, `CommandCancel`
- **Subscriptions**: `SubscriptionNone`, `SubscriptionBatch`, `SubscriptionEvery {key, millis, tag}`, `SubscriptionSource {key, open: Resource GtkError (Recv msg), ...}`

### Runtime Event Loop

**File**: `crates/aivi/src/runtime/builtins/gtk4_real.rs` (792 lines)

Key function: `runGtkAppHost` (around line 821)

**Current loop structure** (lines 844–867):
```rust
loop acc = {
  st: config.model
  rootId: root
  commands: emptyHostedCommands
  subscriptions: activeSubscriptions
} => {
  result <- concurrent.recv msgRx
  result match
    | Ok msg => do Effect {
        step <- config.update appId win msg acc.st
        newView = config.view step.model          // View reads from committed model
        newRoot <- reconcileNode acc.rootId newView
        nextSubscriptions <- syncSubscriptions msgTx (config.subscriptions step.model) acc.subscriptions
        nextCommands <- launchCommands msgTx (flattenCommands step.commands) acc.commands
        recurse { st: step.model, rootId: newRoot, commands: nextCommands, subscriptions: nextSubscriptions }
    }
}
```

### Existing Subscription/Effect Primitives

**Concurrency module** (`crates/aivi/src/stdlib/concurrency.rs`):
- `Sender`/`Recv` types for async channels
- `send`, `recv`, `spawn`, `scope` helpers
- Resource acquisition/cleanup for long-lived operations

**Signal types** (gtk4.rs lines 95–105):
- `GtkSignalEvent`: `GtkClicked`, `GtkInputChanged`, `GtkActivated`, `GtkToggled`, `GtkValueChanged`, `GtkKeyPressed`, `GtkFocusIn`, `GtkFocusOut`, `GtkUnknownSignal`, `GtkTick`

---

## 3. Integration Points & Flow

### Architecture Integration Points (app_architecture.md Lines 95–99)

The `gtkApp` event loop must execute this sequence:

```
8. commit the returned `model`
9. assign fresh revisions to changed source snapshots and invalidate affected computed values
10. evaluate the new `view` against the committed model (dirty computed values recalculate lazily on first read)
```

**Steps 9 and 10 do not yet exist in the runtime.** These are the reactive layer hooks.

### Invalidation Strategy (reactive_dataflow.md Lines 76–92)

1. After `update` commits new model, each **changed source snapshot** gets a new **revision**
2. Every computed signal that **read a changed revision** becomes **dirty**
3. Dirtiness **propagates transitively** through dependent computed signals
4. Dirty signals do **not** recompute immediately; they recompute **lazily on next read**
5. First read of a dirty signal in a turn **reevaluates**, records fresh dependencies, **caches result**
6. Later reads in same turn **reuse cache**

### Forms Integration (reactive_dataflow.md Lines 140–148)

Form field state (`Field.value`, `Field.touched`, `Field.dirty`) remains **source values** in the model. Validation errors are **ideal computed-signal candidates**:

```aivi
nameErrors : Model -> List Text
nameErrors = model => visibleErrors model.submitted nameRule model.name
```

Can be promoted to:
```aivi
nameErrorsComputed = computed "nameErrors" (model => visibleErrors model.submitted nameRule model.name)
```

---

## 4. Minimum Vertical Slice: Computed Signals with Invalidation

### 4.1 Public API Surface (AIVI)

**Add to** `crates/aivi/src/stdlib/gtk4.rs` (new exports & types):

```aivi
export Signal
export computed, signal, readSignal

Signal a = ...opaque...

computed : Text -> (Model -> a) -> Signal a
signal : Text -> (Model -> a) -> Signal a       -- alias; same semantics
readSignal : Signal a -> Model -> a             -- evaluate with memoization
```

**Guarantees**:
- Same `Text` key across app turns → same memoization cache entry
- `readSignal sig model` is **pure** — no side effects
- First call in a turn (or after invalidation) triggers reevaluation; later calls return cache

### 4.2 Runtime Data Structure

**Add to** `crates/aivi/src/runtime/` (new file or extend gtk4_real.rs):

```rust
struct ComputedSignalCache {
    key: String,
    // Dependency tracking
    last_source_revisions: HashMap<String, usize>,  // source field → last revision observed
    last_signal_deps: Vec<String>,                  // computed signal keys read during last eval
    // Result cache
    cached_value: Option<Value>,
    is_dirty: bool,
}

struct SignalRegistry {
    signals: HashMap<String, ComputedSignalCache>,
    source_revisions: HashMap<String, usize>,      // global source field revisions
}
```

### 4.3 Event Loop Integration

**Modify** `runGtkAppHost` in gtk4_real.rs:

Add `signalRegistry: SignalRegistry` to loop accumulator state.

**After `update` commits** (before `view` evaluation):

```rust
// STEP 9: invalidate computed signals for changed sources
nextRegistry <- invalidateAndMarkDirty(
    acc.signalRegistry, 
    acc.st,           // old model
    step.model        // new model
)?

// STEP 10: view evaluates computed signals lazily on dirty reads
newView = config.view step.model
```

**Key functions to implement**:

```rust
fn invalidateAndMarkDirty(
    registry: SignalRegistry,
    old_model: Value,
    new_model: Value
) -> Result<SignalRegistry, RuntimeError>
```

This function:
1. Compares old and new models to detect which source fields changed
2. Increments revision for each changed field
3. Marks computed signals that read a changed revision as dirty
4. Propagates dirty transitively through signal→signal dependencies
5. Returns updated registry

### 4.4 Builtin Implementations

**In** `crates/aivi/src/runtime/builtins/gtk4.rs` or gtk4_real.rs:

```rust
// Return opaque Signal value wrapping (key, f)
fn builtin_computed(key: String, f: Value, registry: &SignalRegistry) -> Value

// Look up in registry, reevaluate if dirty, return cached value
// Requires dependency tracking context during function eval
fn builtin_readSignal(sig: Value, model: Value, registry: &mut SignalRegistry) -> Result<Value, RuntimeError>
```

**Dependency tracking**: During reevaluation, intercept all field accesses and computed signal reads to build the dependency set. Options:
- Wrap evaluation in a tracking context that logs reads
- Run function in "tracking mode" first to discover dependencies, then cache + return

### 4.5 Typecheck Changes

**File**: `crates/aivi/src/typecheck/builtins/system_db.rs`

Add function signatures:
```rust
"computed" -> (Text -> (Model -> a) -> Signal a)
"readSignal" -> (Signal a -> Model -> a)
"signal" -> (Text -> (Model -> a) -> Signal a)
```

Ensure `computed` and `readSignal` have **no capability requirements** (pure functions).

### 4.6 Testing

**Add to** `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi`:

```aivi
@test "computed signals memoize within same turn"
test_memo = do Effect {
  counter = reference 0
  sig = computed "counter" (state => 
    counter <- !counter + 1
    state.value
  )
  model = { value: 42 }
  v1 <- readSignal sig model
  v2 <- readSignal sig model
  assertEq v1 v2
  assertEq !counter 1  -- Function ran only once
}

@test "computed signals invalidate on source change"
test_invalidate = do Effect {
  counter = reference 0
  sig = computed "counter" (state => 
    counter <- !counter + 1
    state.value * 2
  )
  model1 = { value: 5 }
  v1 <- readSignal sig model1
  assertEq !counter 1
  
  model2 = { value: 10 }  -- Source changed
  v2 <- readSignal sig model2
  assertEq !counter 2     -- Reevaluated
}

@test "subscriptions can use computed signals to decide membership"
test_subs_computed = do Effect {
  shouldPoll = computed "shouldPoll" (state => state.count > 0)
  model = { count: 5 }
  
  subs = [
    subscriptionEvery { key: "tick", millis: 100, tag: Tick }
    if readSignal shouldPoll model
  ]
  
  assertEq (List.length subs) 1
}
```

### 4.7 Demo Update

**Update** `demos/snake.aivi` (222 lines) to showcase memoization:

```aivi
-- Extract expensive derivations into computed signals
gridComputed = computed "grid" (state =>
  map (y => rowText y state) [0.. gridH - 1]
)

scoreComputed = computed "score" (state =>
  "Snake · Score: {toText state.score}"
)

view : State -> GtkNode
view = state =>
  ~<gtk>
    <GtkBox orientation="vertical" spacing="0" marginTop="8" marginStart="8" marginEnd="8" marginBottom="8">
      <GtkLabel label={readSignal scoreComputed state} cssClass="snakeheader" />
      <each items={readSignal gridComputed state} as={row}>
        <GtkLabel label={row} cssClass="snakerow" />
      </each>
      <GtkLabel label={statusText state} cssClass="snakestatus" />
    </GtkBox>
  </gtk>
```

Benefits:
- If snake/food/score don't change, cached grid and score are reused
- Amortizes cost of expensive list operations across multiple view passes

---

## 5. Boundaries & Guarantees

### Must Preserve (Design Constraints, spec lines 150–158)

1. ✓ **No ambient observer graph**: Signals live inside `gtkApp`, not a separate FRP runtime
2. ✓ **No hidden mutation**: Subscriptions/effects cannot mutate signal caches
3. ✓ **No second scheduler**: Signals run synchronously inside app turn
4. ✓ **No separate capability model**: Same effect/resource model as effects/subscriptions
5. ✓ **Additive to existing architecture**: Reactive layer is optimization inside `Model -> View -> Msg -> Update`, not replacement

### Must NOT Cross (Purity Boundary)

- ❌ Computed signals cannot emit `Msg`
- ❌ Computed signals cannot acquire `Resource` or spawn tasks
- ❌ Computed signals cannot perform `Effect` with side effects
- ❌ Computed signals cannot mutate model or widgets
- ❌ Subscriptions cannot write directly into signal caches

### Integration Points (Who Reads Signals?)

| Reader | Can Read? | Notes |
|--------|-----------|-------|
| `view` | ✓ Yes | Ideal client; reads evaluated against committed model |
| `subscriptions` | ✓ Yes | Only to decide **which subscriptions exist**, not to keep them alive |
| `update` | ❌ No | Use plain model fields; effects capture snapshots separately |
| `commands` / effects | ❌ No | Capture plain Model snapshots; cannot read live signals |

---

## 6. Estimated Effort

### Implementation LOC

| Component | File(s) | Est. LOC | Difficulty |
|-----------|---------|---------|------------|
| Typecheck builtins | `typecheck/builtins/system_db.rs` | 20–40 | Low |
| Stdlib exports & types | `stdlib/gtk4.rs` | 50–100 | Low |
| Runtime registry & invalidation | `runtime/gtk4_real.rs` | 200–300 | **Medium–High** |
| Event loop integration | `runtime/gtk4_real.rs` (loop) | 30–50 | Medium |
| Builtin functions | `runtime/builtins/gtk4.rs` or gtk4_real.rs | 100–150 | **Medium–High** |
| **Total implementation** | | **400–640** | |

### Testing & Demo LOC

| Item | File(s) | Est. LOC |
|------|---------|---------|
| Integration tests | `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi` | 50–100 |
| Demo update | `demos/snake.aivi` | 10–20 |
| **Total test/demo** | | **60–120** |

### Risk & Effort Profile

**High-difficulty items**:
- **Dependency tracking during function evaluation** — requires wrapping evaluation context or running in tracking mode; novel in this runtime
- **Invalidation propagation** — must correctly traverse signal→signal dependency graph; cycle detection critical
- **Integration without breaking existing loop** — careful state threading and error handling

**Mitigations**:
- Start with proof-of-concept on single computed signal before full registry
- Use immutable revision counters; invalidation is append-only
- Add extensive cycle detection tests before integration
- Keep signal logic isolated; minimal touching of core event loop

---

## 7. Deployment Checklist

### Compiler/Type System
- [ ] Add `Signal a` type to system_db
- [ ] Add `computed`, `signal`, `readSignal` function signatures
- [ ] Ensure no capability requirements on signal operations
- [ ] Test type inference: `computed key (state => ...)` → `Signal T`
- [ ] Error on suspicious patterns (Effect inside computed)

### Runtime
- [ ] Implement `SignalRegistry` with revision tracking
- [ ] Implement `invalidateAndMarkDirty` with dependency graph traversal
- [ ] Implement cycle detection for signal dependencies
- [ ] Integrate invalidation into gtkApp event loop (step 9)
- [ ] Implement dependency tracking context/mode for function evaluation
- [ ] Implement `builtin_computed` and `builtin_readSignal`

### Testing
- [ ] Memoization within single turn
- [ ] Invalidation on source change
- [ ] Transitive dirty propagation
- [ ] Signal→signal dependencies
- [ ] Subscriptions using computed signals
- [ ] Cycle detection and error reporting
- [ ] Snake demo runs with computed signals

### Documentation
- [ ] Spec remains normative (reactive_dataflow.md; no changes needed)
- [ ] Update app_architecture.md step 9–10 explanation
- [ ] Add examples to gtk4.md: "Derived Values and Computed Signals"
- [ ] LSP hover docs for `computed`, `readSignal`

---

## 8. Key Files to Modify/Create

| File Path | Purpose | LOC |
|-----------|---------|-----|
| `crates/aivi/src/stdlib/gtk4.rs` | Add Signal type & exports | +50–100 |
| `crates/aivi/src/typecheck/builtins/system_db.rs` | Add signal function sigs | +20–40 |
| `crates/aivi/src/runtime/builtins/gtk4.rs` | Add signal builtins | +100–150 |
| `crates/aivi/src/runtime/builtins/gtk4_real.rs` | Add registry, invalidation, loop integration | +250–400 |
| `crates/aivi/src/runtime/` (new?) | SignalRegistry struct (or gtk4_real.rs) | +150–200 |
| `integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi` | Signal tests | +50–100 |
| `demos/snake.aivi` | Demo update | +10–20 |

---

## 9. Next Phases (Out of Scope for Phase 4)

- **Signal→signal cascades**: Computed signals depending on other computed signals (can be added incrementally)
- **Subscription signal binding**: Automatic subscription management via signals (deferred; subscriptions use signals only to decide membership)
- **Async computed signals**: Streaming or deferred computation (deferred; all Phase 4 signals are synchronous)
- **Signal groups / named caches beyond single key**: Deferred; Phase 4 uses simple string keys
- **Effect handlers / effect composition**: Deferred to Phase 1 extension or later phase

---

## Summary

**Phase 4 reactive dataflow is a coherent, bounded slice** that:
- Introduces memoized computed signals on top of existing gtkApp
- Requires 400–640 LOC of implementation
- Integrates at two clear points in the event loop (steps 9–10)
- Preserves all existing guardrails (purity, no hidden mutation, no second scheduler)
- Enables efficient UI derivations without changing the app architecture

The slice is **small enough to land cleanly** but **large enough to be useful**: computed signals immediately improve performance of expensive list/filter operations and provide a foundation for later reactive features.

