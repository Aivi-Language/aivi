# Machine Runtime

<!-- quick-info: {"kind":"topic","name":"machine runtime"} -->
Machine runtime behavior defines when transitions are legal, when state changes become visible, and what happens if transition hooks fail.
<!-- /quick-info -->

This page explains what the generated machine value does at runtime.

Start with [State Machines](./state_machines.md) for the overview, [Machine Syntax](./machines.md) for declaration details, and [Effects](./effects.md) for the general `on ... => ...` form.

## What exists at runtime

A `machine` declaration produces a record value with:

- one effectful function per transition name
- `currentState`
- `can`, a record of guard-check functions

Using the `AccountSyncMachine` declaration from [Machine Syntax](./machines.md):

<<< ../snippets/from_md/syntax/machines_runtime/block_01.aivi{aivi}


The behavior above is currently verified by `integration-tests/syntax/effects/machine_runtime.aivi`.

## Initial state behavior

The init rule (`-> State : initEvent { ... }`) chooses the machine's initial runtime state.

In the current runtime, that initial state is already active as soon as the machine value exists. In other words, `currentState Unit` reads the init target state before you call any ordinary transition.

The init transition name may still be present on the generated record, but it is not a normal runtime step. After startup, calling it fails with `InvalidTransition`.

## What happens when you call a transition

Calling a non-init transition performs a guard check against the machine's current state. If exactly one declared edge for that transition name matches, the runtime applies the state change and then returns `Unit` unless a later handler fails.

<<< ../snippets/from_md/syntax/machines_runtime/block_02.aivi{aivi}


A transition call always runs inside `do Effect { ... }`.

If the current state does not match the declared `FromState`, the call fails with:

`InvalidTransition { machine, from, event, expectedFrom }`

The payload tells you:

- `machine`: which machine rejected the move
- `from`: the state the machine was in
- `event`: the transition you tried to call
- `expectedFrom`: the declared source state or states for that transition name

This matters when one transition name is reused from more than one source state: `expectedFrom` is a list, not a single state.

## Handler ordering

Inside `do Effect { ... }`, `on transition => handler` registers a handler for one transition.

When a transition succeeds, runtime behavior is:

1. check the guard
2. update the state
3. run the registered handlers for that transition

<<< ../snippets/from_md/syntax/machines_runtime/block_03.aivi{aivi}


This ordering makes handlers a good fit for follow-up work such as logging, metrics, cache invalidation, or notifications. Use the transition body for the workflow step itself, and use `on` for reactions to a successful step.

## What happens if a handler fails

If a registered handler fails, the transition call returns that failure, but the state change remains applied. There is no automatic rollback.

Using the same machine:

<<< ../snippets/from_md/syntax/machines_runtime/block_04.aivi{aivi}



This is important in real systems: once the transition itself has succeeded, post-transition observers do not rewind the workflow.

## Practical rules of thumb

- Use `can` when you want a cheap “is this legal right now?” check.
- Handle `InvalidTransition` when a failed move is part of normal control flow.
- Treat the init rule as a declaration of the starting state, not as a normal step in the runtime workflow.
- Put workflow-state changes in transitions and side-effect reactions in `on` handlers.
- Do not rely on handler failure to undo a state change; there is no automatic rollback.
