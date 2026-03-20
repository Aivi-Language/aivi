# Proposal: Unified Workflow Pipelines

> **Proposal status:** this page is a forward-looking design proposal. It does **not** replace the current v0.1 language reference yet.
> 
> Today, workflow sequencing is still described across [Effects](../syntax/effects.md), [Cleanup & Lifetimes](../syntax/resources.md), and [Fan-out & Collection Shaping](../syntax/generators.md). If this proposal lands, those pages become migration references during the refactor and are then removed or rewritten at cutover.

## Why this proposal exists

AIVI currently splits workflow-shaped code across several surfaces:

- plain `|>` pipelines for ordinary transforms,
- dedicated effect blocks for effects and resource acquisition,
- `do M { ... }` for generic chaining,
- dedicated applicative blocks for independent validation,
- legacy fan-out blocks for zero-many workflows,
- legacy cleanup blocks for explicit acquisition and release.

Each of those forms is coherent on its own, but together they force programmers, formatters, and tooling to switch between several syntactic models for what is often the same left-to-right idea: take a current value, run the next step, and decide what to do with success, absence, multiplicity, failure, cleanup, and control flow.

This proposal replaces that split with one pipeline calculus so ordinary sequencing, optional binding, failure propagation, applicative fan-out, generator expansion, resource cleanup, retries, timeouts, concurrency, and looping all read in the same surface language.

The intended end state is deliberate: **one workflow style, not several equivalent ones**.

## Design goals

1. **One workflow surface.** Effects, `Option`, `Result`, applicatives, generators, and resources should look like variations of one core idea rather than separate notations.
2. **Left-to-right reading.** The common case should read as a single pipeline without nested dedicated effect blocks.
3. **Explicit control.** Failure handling, retries, timeouts, cleanup, concurrency, and branching should remain visible in the syntax.
4. **Formatter-friendly structure.** Indentation and subflow boundaries should come from the parsed tree, not from heuristics.
5. **Mechanical migration.** The compiler should be able to lower the new syntax to the same internal workflow machinery during the refactor.
6. **No permanent aliases.** If adopted, legacy workflow block forms should be removed instead of kept as long-term duplicates.

## Non-goals

- This proposal does **not** redesign patching (`<|`) or signal writes (`<<-`).
- This proposal does **not** make JSON decoding implicit by expected type; explicit decode stages stay clearer and give better diagnostics.
- This proposal does **not** keep legacy workflow blocks as permanent sugar. Temporary migration support is acceptable during the refactor, but the target language should end with one way to express workflow sequencing.

## Core model

Every workflow pipeline conceptually carries three things:

- the **current** subject value,
- a lexical map of remembered names introduced by `:|> #name`,
- a stack of **control policies** that affect the next stage or the next nested subflow.

Data-flow stages transform or unwrap the current subject.
Control stages do not directly replace the current subject; instead they remember names, attach policies, open grouped subflows, or branch between subflows.

In other words:

- `|>`, `!|>`, `?|>`, `&|>`, `*|>` and `+|>` describe **what data flow happens next**,
- `:|>` describes **how the next part of the workflow behaves**.

## Stage family

| Surface Kind | Meaning                   |
| ------------ | ------------------------- |
| `\|>`        | pure transform            |
| `!\|>`       | fail-fast bind            |
| `?\|>`       | optional bind             |
| `&\|>`       | applicative branch        |
| `*\|>`       | sequence / generator bind |
| `+\|>`       | as * but applicative      |
| `:\|>`       | control / meta stage      |

The current `|>` rule still applies: the value on the left flows into the final argument position on the right.

## Control clauses

`:` remains outside the data-flow family on purpose. A `:|>` stage never means "transform the current subject". Instead, it configures the next stage or opens a scoped subflow. However it does take the previous subject and passes it on to
policies that take an unspecified arg still. ie, yes/no/cleanup etc

Comma-separated clauses inside one `:|>` stage are applied left-to-right.

```aivi
fetch url
  :|> retry 3x, timeout 3s
   |> .body
  !|> decode User
   |> user => { user }
```

### Remembered names

`#name` remembers the current subject without changing it.

```aivi
fetch url #response
   |> .body
  !|> decode User #user
   |> { user, response }
```

Remembered names are visible to later stages in the same pipeline and to nested subflows opened from that point onward.

### Failure, fallback, and recovery

This proposal replaces the old `or` split with explicit control clauses:

| Clause                  | Meaning                                                                                                                                |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `attempt`               | Capture the failure of the next fail-fast stage or grouped subflow as `Result`.                                                        |
| `recover \| ... => ...` | Handle the failure of the current fail-aware result and resume with a replacement value or failure.                                    |
| `default expr`          | Provide a default for short-circuiting `Option`, `Result`, or effect failure when the fallback does not depend on the failure payload. |
| `expect err`            | Upgrade an in-scope `None` short-circuit into an explicit failure value.                                                               |
| `guard pred else err`   | Fail immediately unless the predicate holds for the current subject.                                                                   |

Examples:

```aivi
url
  :|> attempt
  !|> fetch
  :|> timeout 3s
  !|> decode User
```

```aivi
lookup settings "port"
  :|> expect MissingPort
  !|> parsePort
```

```aivi
config
  :|> guard .ok or BadConfig
  !|> startSystem
```

`!|>` is fail-fast by default. `attempt`, `recover`, `default`, and `expect` are the explicit ways to turn that failure into data or a fallback.

### Guards and conditional subflows

`when` and `unless` become control clauses that gate the next stage or the next grouped subflow.

If the gated stage is skipped, the current subject is preserved.

```aivi
user
  :|> when .needsNormalization
      |> normalizeUser
  :|> unless .isActive
      |> markInactive
```

Branching uses a dedicated control form so grouped branch pipelines remain formatter-friendly:

```aivi
request
  |> useCache
    :|> yes
        |> cache.read key
    :|> no
      :|> timeout 3s
      !|> fetch key
      !|> decode User #user
  |> "{user.firstName} {user.lastName}"
```

## Resources and cleanup

This proposal removes the dedicated cleanup-block workflow surface. Resource lifetime becomes a pipeline concern through explicit cleanup registration.

| Clause          | Meaning                                                                                            |
| --------------- | -------------------------------------------------------------------------------------------------- |
| `cleanup expr`  | Register a finalizer to run when the enclosing subflow exits by success, failure, or cancellation. |
| `cancelOnError` | Cancel in-flight sibling work when one branch fails.                                               |

Example:

```aivi
path
  !|> file.open
  :|> cleanup file.close
  !|> file.readAll
```

The concrete runtime may still lower resource-aware stages through the existing `Resource` machinery. The source language no longer needs a separate block form for that lifetime rule.

## Applicatives and validation

Applicative composition becomes an explicitly grouped pipeline shape rather than a separate applicative block. Consecutive &|> collect all errors

Example:

```aivi
form
  &|> .name  |> validateName #name
  &|> .email |> validateEmail #email
  &|> .age   |> validateAge #age
  |> User { name, email, age }
```

Inside an `all` group:

- each `&|>` branch receives the same pre-group subject,
- remembered names from successful branches become available after the group,

## Generators, zero-many workflows, and concurrency

`*|>` replaces the separate legacy fan-out block surface. A `*|>` stage expands zero-many outputs and flattens them into the surrounding workflow.

suggestion: +|> should fan out with applicative mode

| Clause         | Meaning                                                                                         |
| -------------- | ----------------------------------------------------------------------------------------------- |
| `concurrent n` | Permit up to `n` in-flight branches for the next fan-out stage or grouped subflow.              |
| `ordered`      | Preserve source order when a concurrent fan-out stage would otherwise emit in completion order. |

Example:

```aivi
seed
  :|> loop #cursor // loop receives seed as subject
      !|> fetchPage cursor #page
      *|> .items
      :|> recurse page.nextCursor while page.hasMore
```

Concurrent fan-out:

```aivi
urls
  :|> concurrent 8, timeout 3s
  *|> fetch // stops on first fetch fail or decode error  
      !|> decode Item
```
Q: would this work?

```aivi
urls
  :|> concurrent 8, timeout 3s
  +|> fetch // collects validations
      !|> decode Item
```

The compiler should reject `concurrent` on stages that cannot fan out, because silent no-op concurrency settings would be misleading.Idea

## Grouping and subflows

The proposal needs one generic grouping rule rather than many unrelated block syntaxes.

Any `:|>` clause that owns an indented body opens a **subflow**. Dedenting closes that subflow.

Examples:

```aivi
request
  :|> flow #user // Q:should this be here or after decode User?
      :|> timeout 3s
      !|> fetch key
      !|> decode User
   |> "{user.firstName} {user.lastName}"
```

```aivi
url
  :|> attempt
      !|> fetch
      !|> decode User
```

This keeps grouping uniform:

- if/else could be done with :|> yes :|> no
- `when/yes` and `unless/no` can own subflows,
- `attempt` can own a subflow,
- `flow` is the generic "just group these stages" form.

## Loop and recurse

This proposal keeps structured feedback, but it moves it into the control-stage family instead of a separate block statement syntax.

| Clause                                 | Meaning                                                                                  |
| -------------------------------------- | ---------------------------------------------------------------------------------------- |
| `loop #name = expr, ...`               | Introduce named loop-carried state for the current subflow.                              |
| `recurse expr, ... while pred` | Jump back to the nearest loop head with updated carried state while the predicate holds. |

Example:

```aivi
all = initialCursor
  :|> loop #cursor
  !|> fetchPage cursor #page
  *|> page
  :|> recurse page.nextCursor while page.hasMore
```

Resource cleanup registered during one iteration runs before the next iteration begins unless the cleanup is intentionally registered outside the loop scope.

## Formatting and indentation

The formatter should own this style. Authors should not have to hand-align complex subflows.

### Alignment rule

In a multi-line workflow pipeline, the formatter aligns stage operators on the `|` column.

That means the bare pipe is rendered with one leading space when it appears alongside sigiled stages:

```aivi
request
  |> useCache
   :|> yes
      |> cache.read key
   :|> no
     :|> timeout 3s
     !|> fetch key
     !|> decode User #user
  |> "{user.firstName} {user.lastName}"
```

Formatting rules:

- sigiled stages (`:|>`, `!|>`, `?|>`, `&|>`, `*|>`, `+|>`) keep their prefix immediately before `|`,
- the plain stage is rendered as ` |>`,
- `then` and `else` align with each other,
- branch-local stages indent under their owning branch,
- dedenting closes the current subflow and returns to the outer pipeline.

### Why this matters

If the `|` column always lines up, long workflow pipelines stay visually scannable even when they mix pure transforms, failure-aware steps, metadata stages, and grouped branches.

Q: how do we replace
@test "demo test"
myTest =
  assertEq 1 1
     ~|> assertEq 2 1

Q: can we just start from the first effect expression? That would also let `*|>` and `+|>` keep a consistent root-expression story.
 
## Migration and refactor plan

Adopting this proposal would require a deliberate end-to-end refactor.
