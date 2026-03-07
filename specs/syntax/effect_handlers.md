# Effect Handlers / Interpreters

<!-- quick-info: {"kind":"topic","name":"effect handlers"} -->
Effect handlers let the same capability-based `Effect` or `Resource` code run against different implementations in different scopes, such as real I/O in production and fixture-backed behavior in tests.
<!-- /quick-info -->

If a capability says **what code may do**, a handler says **who performs that work here**.

If you come from another ecosystem, this is similar in spirit to dependency injection or swapping service implementations in tests. The difference is that AIVI does it with capability syntax, so the authority requirements stay visible in the type.

## Start with one example

```aivi
with {
  file.read = fixtureFiles,
  process.env.read = fixtureEnv,
  clock.now
} in readBootConfig
```

Read that block like this:

- use `fixtureFiles` whenever this scope reads files
- use `fixtureEnv` whenever this scope reads environment variables
- keep the nearest outer or default `clock.now` handler unchanged

## What effect handlers are for

Effect handlers are useful when you want to:

- run the same logic against real I/O in production and fixtures in tests
- replace time, file access, or environment reads with deterministic local behavior
- simulate part of the outside world without rewriting the business logic
- override one capability in a small region of code without affecting the rest of the program

Handlers do **not** change the meaning of `Effect E A` or `Resource E A`.

- `E` is still the typed domain error
- `A` is still the success value
- capability clauses still describe the minimum authority required
- handlers only choose the interpreter used for that authority in one scope

## Basic syntax

`with { ... } in expr` supports two kinds of entries:

1. a **bare capability** entry that narrows the available authority
2. a **handler binding** that installs an interpreter for a capability or capability family

```aivi
with {
  file.read = fixtureFiles,      -- Use fixture-backed file reads in this scope
  process.env.read = fixtureEnv, -- Use deterministic environment values here
  clock.now                      -- Keep the nearest outer/default clock.now interpreter
} in readBootConfig
```

| Entry form | Meaning |
| --- | --- |
| `file.read` | `file.read` is allowed in this scope and resolves to the nearest outer or default interpreter |
| `file.read = fixtureFiles` | `file.read` is allowed and handled by `fixtureFiles` in this scope |
| `file = localFs` | the `file` family is allowed and `localFs` handles any `file.*` leaf not overridden by a more specific entry |

## The right-hand side is a handler value

The right-hand side of a handler binding is a regular value naming the handler. It is not itself an `Effect` or `Resource`.

If a handler needs setup or teardown, do that work first and then install the resulting value into the `with` scope.

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

In other words: resource setup happens outside `with`, and the scoped handler uses the prepared value.

## How scope works

Handler scopes are:

- visible only inside the matching `in expr` (**lexical scope**)
- inherited by function calls made from inside that scope (**deep scope**)
- nestable, so an inner scope may override an outer handler
- never allowed to grant a capability the surrounding code did not already have

### How authority is checked

The visible capability set for an expression is still the intersection of:

- the enclosing function signature
- any enclosing `with { ... } in` scopes
- the innermost `with` block currently being checked

An inner scope may narrow authority further, but it cannot widen it.

### How interpreter lookup works

When code needs a capability such as `file.read`, lookup works like this:

1. start at the innermost active `with` scope
2. prefer an **exact** binding such as `file.read = ...`
3. otherwise use a matching **family** binding such as `file = ...`
4. if no scoped binding matches, use the nearest outer or default runtime interpreter

Bare capability entries participate in authority checking, but they do not stop interpreter lookup from continuing outward.

```aivi
with {
  file = realFs,
  file.read = fixtureReads
} in with {
  file.write = auditSink
} in syncProfile
```

In that example:

- `file.read` resolves to `fixtureReads`
- `file.write` resolves to `auditSink`
- `file.metadata` resolves to `realFs`

The same exact capability path must not be bound twice in one `with` block.

## Handlers in tests and local simulations

Tests use the same mechanism as ordinary code: install handlers, then run the production logic unchanged.

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

Use effect handlers when your code is already written against capability requirements.

Keep [`mock ... in` expressions](/syntax/decorators/test#mock-expressions) for cases where you want to replace one specific imported binding rather than interpret a capability.

## Handlers and `resource`

Handlers apply to the full resource lifecycle:

- acquisition before `yield`
- helper effects inside the resource body
- cleanup after `yield`

When a resource is acquired with `<-`, the runtime captures the active handler environment for that acquisition. The cleanup phase later runs with that same captured environment, even if inner scopes shadow the capability before the enclosing block exits.

This keeps acquisition and release paired with the same interpreter.

## Cleanup and cancellation

Handlers may change the meaning of file I/O, HTTP, clocks, or cancellation observation, but they do **not** weaken AIVI’s cleanup guarantees.

- resource finalizers still run in LIFO order
- cleanup still runs after normal completion, typed failure, and cancellation
- cleanup remains cancellation-protected automatically
- cleanup errors are still suppressed or logged instead of replacing the original failure

If a handler value itself owns external state that needs teardown, model that state with `resource` and install the resulting value into the handler scope. Do not rely on leaving the `with` block to release it automatically.

## How handlers relate to capabilities

Handlers do not replace capability clauses; they work within them.

```aivi
readConfig : Effect ConfigError Config with { file.read, process.env.read }

readConfigForTests : Effect ConfigError Config with { file.read, process.env.read }
readConfigForTests =
  with {
    file.read = fixtureFiles,
    process.env.read = fixtureEnv
  } in readConfig
```

The inner scope above does not widen `readConfig`’s authority. It only changes which interpreter serves the already-required capabilities.

## Related features that look similar

These features are distinct from effect handlers:

- `on transition => handler` in machine code is a **transition hook**, not a capability interpreter
- `mock ... in` in [`@test`](decorators/test.md#mock-expressions) is **binding substitution**, not capability interpretation

Use effect handlers when you want to reinterpret capability-driven effects in a scope. Use the other features when you are working with transitions or named bindings instead.
