# Effect Handlers / Interpreters

<!-- quick-info: {"kind":"topic","name":"effect handlers"} -->
Effect handlers let the same capability-based `Effect` or `Resource` code run against different implementations in different scopes, such as real I/O in production and fixture-backed behavior in tests.
<!-- /quick-info -->

If a capability says **what code may do**, a handler says **who performs that work here**.

In plain language, a handler is the value that actually performs a capability such as `file.read` or `clock.now` in the current scope.

If you come from another ecosystem, this is similar in spirit to dependency injection or swapping service implementations in tests. The difference is that AIVI does it with capability syntax, so the authority requirements stay visible in the type.

## Start with one example

<<< ../snippets/from_md/syntax/effect_handlers/block_01.aivi{aivi}


Read that block like this:

- allow code in this inner scope to use exactly the listed capabilities
- interpret `file.read` with `fixtureFiles`
- interpret `process.env.read` with `fixtureEnv`
- keep `clock.now` available, but continue resolving it from the nearest outer or default interpreter

`clock.now` appears as a bare entry because `with { ... } in` still narrows the inner capability set to the listed entries, even when some entries also install handlers.

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

With that mental model in place, the surface syntax stays small.

## Basic syntax

`with { ... } in expr` supports two kinds of entries. Every entry contributes to the inner scope's visible capability set; some entries also install handlers.

1. a **bare capability** entry that keeps a capability available in the narrowed inner scope without installing a new handler
2. a **handler binding** that keeps a capability or capability family available and also installs an interpreter for it

<<< ../snippets/from_md/syntax/effect_handlers/block_02.aivi{aivi}


| Entry form | Meaning |
| --- | --- |
| `file.read` | `file.read` remains available as part of this inner scope, and interpreter lookup continues to the nearest outer or default interpreter |
| `file.read = fixtureFiles` | `file.read` remains available in this inner scope and is handled by `fixtureFiles` here |
| `file = localFs` | the `file` family remains available in this inner scope, and `localFs` handles any `file.*` leaf not overridden by a more specific entry |

Bare entries and handler bindings both count toward the inner capability set. If code in the body still needs `clock.now`, `db.query`, or another capability that you are not rebinding, you still list it explicitly (or list a covering family entry) so the narrowed scope continues to allow it.

## The right-hand side is a handler value

The right-hand side of a handler binding is a regular value naming the handler. It is not itself an `Effect` or `Resource`.

If a handler needs setup or teardown, do that work first and then install the resulting value into the `with` scope.

<<< ../snippets/from_md/syntax/effect_handlers/block_03.aivi{aivi}


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
- any enclosing `with { ... } in` scopes, where each scope contributes every capability path named by its entries, including `capability = handler` bindings
- the innermost `with` block currently being checked

An inner scope may narrow authority further, but it cannot widen it.

So `with { file.read = fixtureFiles } in expr` allows `file.read` in that inner scope, not every capability from the surrounding scope. If `expr` also needs `clock.now`, you must list `clock.now` explicitly or add a broader entry that covers it.

### How interpreter lookup works

When code needs a capability such as `file.read`, lookup works like this:

1. start at the innermost active `with` scope
2. prefer an **exact** binding such as `file.read = ...`
3. otherwise use a matching **family** binding such as `file = ...`
4. if nothing in the current scope matches, continue outward until a scoped binding or the default runtime interpreter handles it

Bare capability entries participate in authority checking, but they do not stop interpreter lookup from continuing outward.

That is why bare entries are useful: they narrow which capabilities the inner code may use without forcing you to install a fresh handler for each one.

<<< ../snippets/from_md/syntax/effect_handlers/block_04.aivi{aivi}


In that example:

- `file.read` resolves to `fixtureReads`
- `file.write` resolves to `auditSink`
- `file.metadata` resolves to `realFs`

If the same exact capability path appears more than once in one `with` block, current runtime behavior is “last binding wins”. Avoid repeating the same path in one block; one explicit binding is clearer.

## Handlers in tests and local simulations

Tests use the same mechanism as ordinary code: install handlers, then run the production logic unchanged.

<<< ../snippets/from_md/syntax/effect_handlers/block_05.aivi{aivi}


Use effect handlers when your code is already written against capability requirements.

Keep [`mock ... in` expressions](/syntax/decorators/test#mock-expressions) for cases where you want to replace one specific imported binding rather than interpret a capability.

## Handlers and `resource`

Handlers apply to the full resource lifecycle:

- acquisition before `yield`
- helper effects inside the resource body
- cleanup after `yield`

When a resource is acquired, typically with `<-` inside `do Effect`, the runtime captures the active handler environment for that acquisition. The cleanup phase later runs with that same captured environment, even if inner scopes shadow the capability before the enclosing block exits.

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

<<< ../snippets/from_md/syntax/effect_handlers/block_06.aivi{aivi}


The inner scope above does not widen `readConfig`’s authority. It only changes which interpreter serves the already-required capabilities.

## Related features that look similar

These features are distinct from effect handlers:

- `on transition => handler` in machine code is a **transition hook**, not a capability interpreter
- `mock ... in` in [`@test`](/syntax/decorators/test#mock-expressions) is **binding substitution**, not capability interpretation

Use effect handlers when you want to reinterpret capability-driven effects in a scope. Use the other features when you are working with transitions or named bindings instead.
