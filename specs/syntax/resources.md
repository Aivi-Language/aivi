# Cleanup and Managed Lifetimes

<!-- quick-info: {"kind":"syntax","name":"@cleanup","signature":"A -> (A -> Effect E Unit) -> A"} -->
AIVI v0.2 manages acquisition and release with the `@cleanup` flow modifier. Register cleanup on the line that acquires a handle, and the runtime guarantees that finalization runs when the enclosing flow scope exits.
<!-- /quick-info -->

This page covers **resource-style cleanup semantics**. Legacy cleanup blocks are no longer part of the current surface syntax.

## Basic pattern

```aivi
readAllText = path =>
  path
     |> file.open @cleanup file.close #handle
     |> file.readAll handle
```

The successful result of the annotated line keeps flowing as usual:

- `file.open` produces the handle,
- `#handle` binds it for later steps,
- `@cleanup file.close` registers the finalizer,
- later lines can keep using `handle` until the enclosing flow ends.

## Lifecycle guarantees

Cleanup registration is structural rather than ad hoc.

- Cleanup is registered **only if the line succeeds**.
- Cleanup runs on **normal completion**, **typed failure**, and **cancellation**.
- Multiple cleanups unwind in **LIFO** order.
- The cleanup expression receives the successful line result as its final argument.
- Registering cleanup does **not** change the current flow subject.

## Multiple managed handles

```aivi
compareWithRight = rightPath => left =>
  rightPath
     |> file.open @cleanup file.close #right
     |> compareHandles left right

compareFiles = leftPath rightPath =>
  leftPath
     |> file.open @cleanup file.close
     |> compareWithRight rightPath
```

When that flow exits, `right` closes before `left`.

## Error semantics

Cleanup follows the same intent as earlier scoped finalizers, but it is now attached directly to flow lines.

- If acquisition fails, there is nothing to clean up.
- If later work fails after acquisition, registered cleanup still runs.
- If cleanup itself fails, the cleanup failure is secondary to the original flow failure.
- A cancellation signal still triggers registered cleanup before the scope is considered finished.

## Cancellation behavior

Cleanup runs in a cancellation-protected finalization context.

That means:

- cancellation observed before acquisition prevents the line from succeeding,
- cancellation observed after acquisition still unwinds registered cleanup,
- a second cancellation signal does not interrupt cleanup half-way through.

## When to use `@cleanup`

Reach for `@cleanup` whenever a successful step returns a handle-like value that must be released explicitly, for example:

- file handles,
- sockets and listeners,
- temporary directories,
- database sessions,
- long-lived mailbox or UI resources.

For the full syntax of modifiers such as `@cleanup`, `@retry`, and `@timeout`, see [Flow Syntax](flows.md).
