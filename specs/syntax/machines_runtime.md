# Machine Runtime Semantics

This section defines how `machine` declarations behave at runtime.

## Runtime value shape

A machine declaration creates a runtime value with:

- one function per transition name
- `currentState`
- `can` (record of transition guard functions)

```aivi
machine AccountSyncMachine = {
             -> Idle     : boot {}
  Idle       -> Acquired : lease {}
  Acquired   -> Syncing  : run { batchId: Int }
  Syncing    -> Idle     : done {}
}

sync = do Effect {
  { boot, lease, run, done, currentState, can } = AccountSyncMachine
  _ <- assertEq (constructorName (currentState Unit)) "Idle"
  _ <- assertEq (can.lease Unit) True
  _ <- assertEq (can.run Unit) False
  pure Unit
}
```

## Transition calls

Calling a transition performs a runtime state guard and, on success, updates the machine state:

```aivi
_ <- lease {}
_ <- run { batchId: 42 }
_ <- done {}
```

- Transition functions are effectful and run inside `do Effect { ... }`.
- Calling a transition from the wrong state fails with:
  `InvalidTransition { machine, from, event, expectedFrom }`.

## Initial transition at runtime

`-> State : initEvent { ... }` sets the machine's initial runtime state to `State`.
Calling the init event after initialization is invalid.

## `on` handler ordering

Inside `do Effect { ... }`, `on transition => handler` registers a transition handler.

When a transition is called:

1. guard is checked
2. state is updated
3. registered handlers for that transition run

If a handler fails, the transition state update remains applied (no rollback).

## Full example (state + invalid transition + handler failure)

```aivi
machineFlow = do Effect {
  { run, lease, currentState } = AccountSyncMachine

  on run => fail "run handler failure"
  _ <- lease {}

  runResult <- attempt (run { batchId: 1 })
  _ <- runResult match
    | Err _ => pure Unit
    | Ok _  => fail "expected handler failure"

  _ <- assertEq (constructorName (currentState Unit)) "Syncing"
  pure Unit
}
```
