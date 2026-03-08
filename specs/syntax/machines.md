# Machine Syntax

<!-- quick-info: {"kind":"syntax","name":"machine"} -->
A `machine` declaration defines a workflow with named states and typed transitions. You use its generated transition functions inside `do Effect { ... }` blocks to move through that workflow safely.
<!-- /quick-info -->

Start with [State Machines](./state_machines.md) if you want the reader-facing introduction. This page focuses on declaration syntax and on the API a `machine` declaration generates.

## Start with a practical example

<<< ../snippets/from_md/syntax/machines/block_01.aivi{aivi}


This declaration says:

- `boot` is the init rule that declares `Idle` as the starting state
- `lease` is only legal while the machine is in `Idle`
- `run` is only legal from `Acquired` and requires a `batchId`
- `done` returns the workflow to `Idle`

A machine is a good fit when a workflow has named steps that must happen in order.

## What a `machine` declaration does

A machine declaration names the valid states of a workflow and the legal transitions between those states. Its transition names become functions on the generated machine value, and that value also exposes helpers such as `currentState` and `can`.

Use a machine when your program has steps that must happen in order, such as leasing work before running it or authorizing a payment before capturing it.

## Declaration syntax

<<< ../snippets/from_md/syntax/machines/block_02.aivi{aivi}


Each line is one transition rule.

| Part | Meaning |
|:-----|:--------|
| `FromState` | Required source state (omit this for the init transition) |
| `-> ToState` | Destination state after the transition fires |
| `transitionName` | `lowerCamelCase` name exposed as an effectful function |
| `{ FieldDecl, ... }` | Payload record type for this transition (`{}` when there is no payload) |

In the running example above, `Idle -> Acquired : lease {}` means “when the machine is in `Idle`, calling `lease {}` moves it to `Acquired`”.

The rule that starts with bare `->` is the **init transition**. It declares the starting state; in the current runtime, that state is already active as soon as the machine value exists.

## Using a machine in code

Destructure the machine value to access its transition functions and helper fields. Because the init state is already active, ordinary code starts with the first non-init transition rather than calling the init rule again.

```aivi
sync = do Effect {
  { lease, run, done, currentState } = AccountSyncMachine

  _ <- lease {}
  _ <- run { batchId: 42 }
  _ <- done {}

  pure (currentState Unit)
}
```


Read that block as “start in `Idle`, step through the workflow, then inspect the resulting state”.

Every transition function is effectful. Calling one from the wrong state fails with `InvalidTransition { machine, from, event, expectedFrom }`; see [Machine Runtime](./machines_runtime.md) for the full failure contract.

## Generated API surface

A machine value exposes:

| Field | Type | Description |
|:------|:-----|:------------|
| `transitionName` | `Payload -> Effect TransitionError Unit` | One function per declared transition name; calling it from the wrong state yields `InvalidTransition { machine, from, event, expectedFrom }` |
| `currentState` | `Unit -> State` | Returns the current state constructor, such as `Idle` or `Syncing` |
| `can` | `{ transitionName: Unit -> Bool, ... }` | Lets you ask whether a transition is currently legal without firing it |

This keeps the machine practical in application code: you can drive the workflow through transitions, inspect the current state, and ask whether a move is currently allowed.

## Pre-checks with `can`

Use `can` when you want to ask whether a transition is legal before attempting it.

Assuming `{ lease, can } = AccountSyncMachine` inside the same `do Effect { ... }` block:

```aivi
_ <- if can.lease Unit
     then lease {}
     else fail "not in Idle state"
```

This avoids an `InvalidTransition` failure when the machine is not currently in `Idle`.

`can` is especially useful for UI enablement, command preconditions, and explicit error messages.

## Transition payloads

A transition may carry a record payload. That is how you attach data that matters only for that specific step.

In the running example, `run { batchId: Int }` means:

- the transition is named `run`
- it can only be called from `Acquired`
- it must be passed a record containing `batchId`
- a successful call moves the machine to `Syncing`

## Transition handlers with `on`

Inside `do Effect { ... }`, use `on transition => handler` to register follow-up behavior that runs after a transition succeeds.

Assuming `run` was destructured from `AccountSyncMachine` in the same `do Effect { ... }` block:

```aivi
on run => do Effect {
  _ <- log.info "run transition fired"
  pure Unit
}
```

Handlers run **after** the state has been updated. They are useful for logging, metrics, notifications, or other side effects that should happen because a transition succeeded.

If a handler fails, the state change remains applied; `on` does not roll the machine back.

See [Effects](./effects.md) for the `on` form itself and [Machine Runtime](./machines_runtime.md) for ordering, failure behavior, and the exact runtime contract.
