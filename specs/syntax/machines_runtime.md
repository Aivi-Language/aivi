# Machine Runtime

<!-- quick-info: {"kind":"topic","name":"machine runtime"} -->
Machine runtime behavior defines how transition calls are guarded, how state updates are applied, and when transition handlers run.
<!-- /quick-info -->

This page explains what happens after a `machine` declaration has been compiled into a runtime value.

Start with [State Machines](./state_machines.md) for the overview and [Machine Syntax](./machines.md) for declaration details.

## What exists at runtime

A machine declaration produces a runtime value with:

- one effectful function per transition name
- `currentState`
- `can`, a record of guard-check functions

```aivi
machine AccountSyncMachine = {
             -> Idle     : boot {}
  Idle       -> Acquired : lease {}
  Acquired   -> Syncing  : run { batchId: Int }
  Syncing    -> Idle     : done {}
}

sync = do Effect {
  { boot, lease, run, done, currentState, can } = AccountSyncMachine

  _ <- boot {}
  _ <- assertEq (constructorName (currentState Unit)) "Idle"   // Read the current state
  _ <- assertEq (can.lease Unit) True                           // Guard says lease is legal
  _ <- assertEq (can.run Unit) False                            // run is not legal yet
  pure Unit
}
```

## What happens when you call a transition

Calling a transition performs a runtime guard check and, if the transition is legal, updates the machine state.

```aivi
_ <- lease {}
_ <- run { batchId: 42 }
_ <- done {}
```

A transition call always runs inside `do Effect { ... }`.

If the current state does not match the declared `FromState`, the call fails with:

`InvalidTransition { machine, from, event, expectedFrom }`

That error tells you which machine rejected the move, what state it was in, which transition you tried, and which source state was required.

## The init transition

`-> State : initEvent { ... }` sets the machine’s first runtime state.

Practical meaning:

- it is the entry point that initializes the machine
- it has no source state because it starts the workflow
- calling it again after initialization is invalid

## Handler ordering

Inside `do Effect { ... }`, `on transition => handler` registers a handler for one transition.

When a transition succeeds, runtime behavior is:

1. check the guard
2. update the state
3. run the registered handlers for that transition

```aivi
on run => do Effect {
  _ <- log.info "sync run started"   // Observes the transition after the state change
  pure Unit
}
```

This ordering makes handlers a good fit for follow-up work such as logging, metrics, cache invalidation, or notifications.

## What happens if a handler fails

Handler failure does **not** roll the machine state back.

```aivi
machineFlow = do Effect {
  { run, lease, currentState } = AccountSyncMachine

  on run => fail "run handler failure"

  _ <- lease {}
  runResult <- attempt (run { batchId: 1 })

  _ <- runResult match
    | Err _ => pure Unit
    | Ok _  => fail "expected handler failure"

  _ <- assertEq (constructorName (currentState Unit)) "Syncing"   // State change remains applied
  pure Unit
}
```

This is important in real systems: once the transition itself has succeeded, post-transition observers do not rewind the workflow.

## Practical rules of thumb

- Use `can` when you want a cheap “is this legal right now?” check
- Handle `InvalidTransition` when a failed move is part of normal control flow
- Put business-state changes in transitions and side-effect reactions in `on` handlers
- Do not rely on handler failure to undo a state change; there is no automatic rollback
