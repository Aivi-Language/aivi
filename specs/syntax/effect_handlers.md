# Effect Handlers / Interpreters

> **Status:** Phase 1 defines the official surface syntax and runtime contract for scoped capability interpreters on this page. Existing implementations may continue to route through ambient builtins until the follow-up compiler/runtime milestones land, but the semantics here are authoritative.

<!-- quick-info: {"kind":"topic","name":"effect handlers"} -->
Effect handlers install scoped interpreters for capability-bearing `Effect E A` and `Resource E A` code. They let the same program logic run against real IO, test doubles, or local simulations without changing `E`, `A`, or the capability vocabulary.
<!-- /quick-info -->

## Overview

AIVI does **not** introduce a second effect type for handlers. The existing Phase 1 capability clause remains the authority story:

- `Effect E A` and `Resource E A` keep their existing meaning.
- capability clauses still describe the **minimum authority** required.
- `with { ... } in expr` remains the lexical scope form.
- handler entries attach interpreters to those same capability names inside that same lexical scope.

Handlers are therefore about **how a capability is interpreted in one scope**, not about changing the type of an effectful computation.

## Surface syntax

The `with { ... } in expr` form supports two entry kinds:

1. a **bare capability** for lexical narrowing
2. a **handler binding** that installs an interpreter for a capability or capability family

```aivi
with {
  file.read = fixtureFiles,
  process.env.read = fixtureEnv,
  clock.now
} in readBootConfig
```

Each entry behaves as follows:

| Entry form | Meaning |
| --- | --- |
| `file.read` | `file.read` is in scope; resolve to the nearest outer or ambient interpreter. |
| `file.read = fixtureFiles` | `file.read` is in scope and resolves to `fixtureFiles` in this lexical region. |
| `file = localFs` | the `file` family is in scope and `localFs` interprets any `file.*` leaf not overridden by a more specific entry. |

The right-hand side of a handler binding is a **pure expression** naming a handler value. It must not itself be an `Effect` or `Resource`. If a handler needs setup or teardown, acquire that support outside the `with` and bind the resulting value into the handler scope:

```aivi
testConfigRead =
  do Effect {
    fixtures <- openFixtureStore "./fixtures"
    with {
      file.read = fixtureReader fixtures,
      process.env.read = fixtureEnv fixtures
    } in readBootConfig
  }
```

## Scope, nesting, and precedence

Handler scopes are:

- **lexical** — active only inside the matching `in expr`
- **deep** — calls made from inside the body see the scoped interpreter too
- **nestable** — inner scopes may shadow outer interpreters
- **authority-preserving** — installing a handler never widens the capability set already in scope

### Authority resolution

The effective capability set for an expression is still the intersection of:

- the enclosing function signature
- any enclosing `with { ... } in` scopes
- the innermost `with` block currently being checked

An inner scope may narrow authority further, but it cannot grant a capability that was not already available from an outer scope.

### Interpreter resolution

When an operation requires a capability such as `file.read`, interpreter lookup proceeds as follows:

1. walk outward from the innermost active `with` scope
2. in each scope, prefer an **exact** match such as `file.read = ...`
3. otherwise use a matching **family** binding such as `file = ...`
4. if no scoped binding matches, fall back to the ambient runtime interpreter for that capability

Bare capability entries participate in authority checking, but they do **not** stop interpreter lookup from continuing outward.

```aivi
with {
  file = realFs,
  file.read = fixtureReads
} in with {
  file.write = auditSink
} in syncProfile
```

In this example:

- `file.read` resolves to `fixtureReads`
- `file.write` resolves to `auditSink`
- `file.metadata` resolves to `realFs`

The same exact capability path must not be bound twice in one `with` block.

## Handler values

Each capability family defines the interpreter shape for that family in the relevant standard-library surface. Phase 1 only standardizes the **binding mechanism**:

- leaf handlers satisfy one capability path such as `clock.now`
- family handlers may satisfy multiple leaves such as `clock.now`, `clock.sleep`, and `clock.schedule`
- a leaf binding may override one operation inside a broader family interpreter

The handler value is part of the surrounding pure environment. It is not a special runtime object outside the language; it is simply the value used when that capability is invoked in the current scope.

## Tests and local simulation

Tests use the same handler mechanism as ordinary code. A test installs interpreters with `with { capability = handler } in`, then runs the production logic unchanged:

```aivi
@test "refreshSession uses deterministic time"
refreshSessionDeterministic =
  with {
    clock.now = fixedClock,
    clock.sleep = immediateClock,
    process.env.read = fixtureEnv
  } in do Effect {
    session <- refreshSession
    _ <- assertEq session.expiresAt expectedExpiry
    pure Unit
  }
```

Use effect handlers when the code under test is written against capability requirements. Keep [`mock ... in` expressions](/syntax/decorators/test#mock-expressions) for:

- binding-level substitution of a specific imported name
- snapshot capture / replay
- legacy ambient APIs that have not yet been tightened to capability signatures

Handlers interpret **capabilities**. `mock ... in` substitutes **qualified bindings**. The two mechanisms may be nested when needed.

## Interaction with `resource`

Handlers apply to the full resource lifecycle:

- acquisition before `yield`
- helper effects inside the resource body
- cleanup after `yield`

When a resource is acquired with `<-`, the runtime captures the active handler environment that was in scope for that acquisition. The resource's cleanup phase later runs with that captured handler environment, even if inner scopes shadow the same capability before the enclosing block exits.

This rule preserves pairing between acquisition and cleanup. A resource acquired through a test or local interpreter must also release through the matching interpreter.

## Cleanup and cancellation

Handlers may change the meaning of operations such as file IO, HTTP, clocks, or cancellation observation, but they do **not** weaken AIVI's structural cleanup guarantees:

- resource finalizers still run in LIFO order
- cleanup still runs after normal completion, typed failure, and cancellation
- cleanup remains cancellation-protected automatically
- cleanup errors are still suppressed/logged rather than replacing the original failure

In particular, handler bindings do **not** override the runtime rule that ordinary resource cleanup is masked from second-stage cancellation. The `cancellation.mask` capability remains for explicit user-facing cancellation-control APIs, not for everyday finalizer safety.

If a handler value itself owns external state that needs teardown, model that state with `resource` and then install the resulting value into the handler scope. Do not rely on handler exit alone to release it.

## Interaction with capabilities

Handlers do not replace capability clauses; they consume them.

```aivi
readConfig : Effect ConfigError Config with { file.read, process.env.read }

readConfigForTests : Effect ConfigError Config with { file.read, process.env.read }
readConfigForTests =
  with {
    file.read = fixtureFiles,
    process.env.read = fixtureEnv
  } in readConfig
```

The inner scope above does **not** widen `readConfig`'s authority. It only changes which interpreter is used for the already-required capabilities.

## Relation to other handler-shaped features

- `on Transition => handler` in [Effects](effects.md) is a **machine transition handler**, not a capability interpreter.
- `mock ... in` in [`@test`](decorators/test.md#mock-expressions) is binding substitution, not capability interpretation.

Those features remain valid and distinct.
