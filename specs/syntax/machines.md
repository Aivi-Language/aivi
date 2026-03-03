# Machines

<!-- quick-info: {"kind":"syntax","name":"machine"} -->
A `machine` declaration defines a **finite-state machine** type with named states and typed transitions. Machines are used inside `do Effect { ... }` blocks to enforce that state-dependent operations occur only in valid states.
<!-- /quick-info -->

## Declaration syntax

```aivi
machine MachineName = {
               -> InitialState : initTransition { FieldDecl, ... }
  StateA       -> StateB       : transitionName { FieldDecl, ... }
  StateB       -> StateC       : anotherTransition { FieldDecl, ... }
}
```

Each line is a **transition rule**:

| Part | Meaning |
|:-----|:--------|
| `FromState` | Required source state (omit for the init transition) |
| `-> ToState` | Destination state after the transition fires |
| `transitionName` | `lowerCamelCase` name exposed as an effectful function |
| `{ FieldDecl, ... }` | Payload record type for this transition (use `{}` for no payload) |

The transition with no `FromState` (the `->` at the start of the block) is the **init transition**. It sets the initial state when the machine is first used.

## Example

```aivi
machine AccountSyncMachine = {
             -> Idle     : boot {}
  Idle       -> Acquired : lease {}
  Acquired   -> Syncing  : run { batchId: Int }
  Syncing    -> Idle     : done {}
}
```

This defines four states (`Idle`, `Acquired`, `Syncing`) and four transitions.

## Using a machine

Destructure the machine record to access its transition functions and state helpers:

```aivi
sync = do Effect {
  { boot, lease, run, done, currentState, can } = AccountSyncMachine

  _ <- boot {}
  _ <- lease {}
  _ <- run { batchId: 42 }
  _ <- done {}
  pure Unit
}
```

Every transition function is effectful (`Effect TransitionError A`). Calling a transition from a wrong state raises `InvalidTransition`.

## Runtime record shape

After destructuring, a machine exposes:

| Field | Type | Description |
|:------|:-----|:------------|
| `transitionName` | `Payload -> Effect TransitionError Unit` | One function per declared transition |
| `currentState` | `Unit -> State` | Returns the current state constructor |
| `can` | `{ transitionName: Unit -> Bool, ... }` | Guard checks without firing the transition |

## State guards with `can`

```aivi
_ <- if can.lease Unit
     then lease {}
     else fail "not in Idle state"
```

## `on` handlers

Register a reaction to a transition inside a `do Effect` block:

```aivi
on run => do Effect {
  _ <- log.info "run transition fired"
  pure Unit
}
```

Handlers run **after** the state has been updated. If a handler fails, the state update is not rolled back.

See [Machine Runtime Semantics](machines_runtime.md) for the complete runtime contract.
