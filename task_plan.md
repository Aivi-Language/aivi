# Task Plan

## Goal

Implement the remaining `partial` and `no` items from [manual/guide/surface-feature-matrix.md](manual/guide/surface-feature-matrix.md) end to end across:

- syntax / HIR
- typed core
- typed lambda / backend lowering
- runtime / execute / run
- Cranelift compile coverage
- manual + verification

Do not mark the work complete until the matrix can be re-audited with proportionate passing tests and the remaining gaps, if any, are explicitly documented.

## Architecture Decision

Treat the matrix as a dependency-ordered language backlog, not a flat checklist.

Implementation order:

1. Close semantics that are currently blocked before typed-core/runtime.
2. Close runtime execution gaps for source-backed and recurrent features.
3. Expand backend/codegen coverage only after earlier IRs carry the missing semantics cleanly.
4. Tighten provider option execution and cross-module evidence after the runtime and lowering paths are stable.
5. Re-audit the matrix against tests.

## Phases

| Phase | Status | Scope |
| --- | --- | --- |
| 1 | in_progress | Convert the matrix into concrete backlog items with exact owning layers and current blockers |
| 2 | pending | Implement missing pre-runtime core semantics (`!|>`, `~|>`, `-|>`, patch gaps where feasible) |
| 3 | pending | Implement runtime / execute / run parity for source-backed and recurrent features |
| 4 | pending | Expand backend / Cranelift coverage for inline case / truthy-falsy / tap / dynamic text / aggregates |
| 5 | pending | Close provider option gaps and cross-module higher-kinded execution gaps |
| 6 | pending | Re-run matrix verification, update docs, and summarize remaining blockers if any |

## Backlog Buckets

### Core semantics currently not end to end

- `!|>` validation stage
- `~|>` previous
- `-|>` diff
- structural patch removal and broader patch lowering
- regex typed-core/runtime support

### Runtime / source gaps

- custom `provider` declarations: executable runtime path
- recurrence parity across `execute` / `run`
- source option completeness:
  - timer `jitter`, `coalesce = False`
  - fs.watch `recursive`
  - socket/mailbox heartbeat / reconnect
  - db.live `optimistic`, `onRollback`
  - window.keyDown `capture`, non-default `focusOnly`
  - dbus.method non-`Unit` replies
  - process.spawn bytes streams
  - db.connect actual pooling semantics

### Higher-kinded / cross-module gaps

- imported user-authored instances
- imported polymorphic class-member execution

### Codegen gaps

- inline pipe `Case`
- inline pipe `TruthyFalsy`
- tap / debug stage
- dynamic text lowering
- aggregate / collection lowering
- remaining domain / builtin apply coverage

## Success Criteria

- The relevant matrix rows become `yes` or remain intentionally `n/a`.
- New semantics have regression tests at the right layer.
- `aivi check`, `aivi execute`, `aivi run` preparation, and `aivi compile` all agree on the implemented subset.
- No new “accepted but not executable” surface is introduced without an explicit manual note.

## Errors Encountered

| Error | Attempt | Resolution |
| --- | --- | --- |
| none yet | 0 | n/a |
