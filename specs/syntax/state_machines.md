# State Machines

<!-- quick-info: {"kind":"topic","name":"state machines"} -->
State machines model workflows where certain actions are only valid in certain states. In AIVI, `machine` gives those workflows a first-class language surface with named states, typed transitions, guard checks, and inspection helpers such as `currentState` and `can`.
<!-- /quick-info -->

If you want syntax details, read [Machine Syntax](./machines.md). If you want the execution rules, read [Machine Runtime](./machines_runtime.md). This page is the practical guide: what machines are for, when to use them, and how they fit into ordinary AIVI code.

## What a state machine is for

Use a state machine when your program has a workflow or protocol where **order matters**.

Common examples:

- a sync job must be leased before it can run
- a payment must be authorized before it can be captured
- a file import moves through pending, running, finished, or failed
- a network connection moves through disconnected, connecting, ready, or closed

Machines help when you want those rules to be explicit, named, and easy to inspect.

They are a good fit when:

- the same workflow appears in more than one place
- transitions need names and typed payloads
- invalid moves should fail at runtime in a predictable way
- other parts of the program should react to successful transitions

If you only need to branch on immutable data once, an ADT plus `match` is often enough. Reach for `machine` when you want to model a **live workflow** that evolves over time.

## The basic idea

Think of a machine as a small workflow object:

1. declare the valid states and transitions
2. destructure the generated machine value
3. call transition functions inside `do Effect { ... }`
4. inspect `currentState` or `can` when needed
5. attach `on` handlers for follow-up work

## A practical example

<<< ../snippets/from_md/syntax/state_machines/block_01.aivi{aivi}


This says:

- `boot` initializes the machine into `Idle`
- `lease` is only legal from `Idle`
- `run` is only legal from `Acquired` and carries a payload
- `done` returns the machine to `Idle`

The `boot {}` transition is the one-time initializer. Calling it again after startup is an invalid transition.

## Using a machine in ordinary code

<<< ../snippets/from_md/syntax/state_machines/block_02.aivi{aivi}


The main idea is simple: transition names become effectful functions. They are the API of the workflow.

## What you get from a machine

Every machine exposes:

| Field | Meaning |
| --- | --- |
| transition functions | One function per declared transition |
| `currentState` | Read the machine’s current state |
| `can` | Ask whether a transition is currently legal |

These helpers make machines useful in real applications:

- `currentState` is good for logs, diagnostics, and status displays
- `can` is good for button enablement and explicit pre-checks
- transition functions keep legal moves visible and easy to review

## Reacting to transitions with `on`

Machines can trigger follow-up work when a transition succeeds. The `on` form is ordinary `Effect` syntax that observes successful transitions; it is not an extra field on the machine value.

<<< ../snippets/from_md/syntax/state_machines/block_01.aivi{aivi}


An `on` handler is useful for logging, metrics, notifications, cache updates, or similar side effects.

## How machines fit with the rest of AIVI

Machines do not replace records, ADTs, or `Effect`. They work alongside them:

- use records and ADTs to model data
- use `Effect` to model effectful work
- use `machine` when the order of effectful steps matters
- use `currentState` and `can` to connect workflow state to UI or control flow

For GTK applications, machines are often a clear way to model submission flows, setup wizards, sync pipelines, or multi-step tasks while the signal-first GTK runtime owns rendering and widget bindings.

## When not to use a machine

You may not need a machine when:

- the state is just plain data with no transition API
- there is no meaningful notion of “illegal move”
- a single `match` expression already explains the logic clearly

Machines are most valuable when the workflow itself is part of the program’s public shape.

## Where to go next

- [Machine Syntax](./machines.md) — declaration form and generated functions
- [Machine Runtime](./machines_runtime.md) — guard checks, failures, and handler ordering
