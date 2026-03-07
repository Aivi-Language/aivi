# Machine Syntax

<!-- quick-info: {"kind":"syntax","name":"machine"} -->
A `machine` declaration defines a **finite-state machine** type with named states and typed transitions. Machines are used inside `do Effect { ... }` blocks to enforce that state-dependent operations occur only in valid states.
<!-- /quick-info -->

Start with [State Machines](./state_machines.md) for the reader-facing explanation of when to use machines and what problem they solve. This page focuses on declaration syntax and the generated API surface.

## What a `machine` declaration does

A machine declaration names the valid states of a workflow and the legal transitions between those states. Each transition becomes an effectful function you can call from ordinary AIVI code.

Use a machine when your program has steps that must happen in order, such as leasing work before running it or authorizing a payment before capturing it.

## Declaration syntax

```aivi
machine MachineName = {
               -> InitialState : initTransition { FieldDecl, ... }
  StateA       -> StateB       : transitionName { FieldDecl, ... }
  StateB       -> StateC       : anotherTransition { FieldDecl, ... }
}
```

Each line is a transition rule.

| Part | Meaning |
|:-----|:--------|
| `FromState` | Required source state (omit this for the init transition) |
| `-> ToState` | Destination state after the transition fires |
| `transitionName` | `lowerCamelCase` name exposed as an effectful function |
| `{ FieldDecl, ... }` | Payload record type for this transition (`{}` when there is no payload) |

The rule that starts with bare `->` is the **init transition**. It sets the initial state when the machine is first used.

## A practical example

```aivi
machine AccountSyncMachine = {
             -> Idle     : boot {}               -- Sets the initial state
  Idle       -> Acquired : lease {}              -- Only legal while the machine is Idle
  Acquired   -> Syncing  : run { batchId: Int }  -- Carries data into the transition
  Syncing    -> Idle     : done {}               -- Returns the workflow to Idle
}
```

This declaration introduces three states — `Idle`, `Acquired`, and `Syncing` — plus four transitions that describe the legal workflow.

## Using a machine in code

Destructure the machine value to access its transition functions and helper fields.

```aivi
sync = do Effect {
  { boot, lease, run, done, currentState, can } = AccountSyncMachine

  _ <- boot {}                                      -- Enter the initial state once
  _ <- lease {}
  _ <- run { batchId: 42 }
  _ <- done {}

  pure (currentState Unit)                          -- Read the live state when needed
}
```

Every transition function is effectful. Calling one from the wrong state fails with `InvalidTransition`.

## Generated API surface

A machine value exposes:

| Field | Type | Description |
|:------|:-----|:------------|
| `transitionName` | `Payload -> Effect TransitionError Unit` | One function per declared transition |
| `currentState` | `Unit -> State` | Returns the current state constructor |
| `can` | `{ transitionName: Unit -> Bool, ... }` | Guard checks without firing the transition |

This keeps the machine practical in application code: you can drive the workflow through transitions, inspect the current state, and ask whether a move is currently allowed.

## Guard checks with `can`

Use `can` when you want a safe pre-check before attempting a transition.

```aivi
_ <- if can.lease Unit
     then lease {}
     else fail "not in Idle state"
```

`can` is especially useful for UI enablement, command preconditions, and explicit error messages.

## Transition payloads

A transition may carry a record payload. That is how you attach data that matters only for that specific step.

For example, `run { batchId: Int }` means:

- the transition is named `run`
- it can only be called from `Acquired`
- it must be passed a record containing `batchId`
- a successful call moves the machine to `Syncing`

## `on` handlers

Inside `do Effect { ... }`, you can register follow-up behavior for a transition.

```aivi
on run => do Effect {
  _ <- log.info "run transition fired"
  pure Unit
}
```

Handlers run **after** the state has been updated. They are useful for logging, metrics, notifications, or other side effects that should happen because a transition succeeded.

See [Machine Runtime](./machines_runtime.md) for the exact runtime contract.
