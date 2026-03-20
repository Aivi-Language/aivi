# Effects

<!-- quick-info: {"kind":"topic","name":"Effects"} -->
`Effect E A` is AIVI's typed representation of observable work. Use it for file I/O, network calls, UI actions, database reads, and other operations that may fail or be cancelled.
<!-- /quick-info -->

## The `Effect E A` type

Think of `Effect E A` as a computation with three explicit parts:

- it may perform observable work,
- it may fail with a typed error `E`,
- if it succeeds, it yields a value `A`.

That keeps side effects visible in the type system instead of hiding them inside ordinary-looking function calls.

```aivi
pure    : A -> Effect E A
fail    : E -> Effect E A
bind    : Effect E A -> (A -> Effect E B) -> Effect E B
attempt : Effect E A -> Effect F (Result E A)
```

### Core operations

- `pure` lifts an ordinary value into an effect.
- `fail` stops the current effect with a typed error.
- `bind` is the sequencing primitive behind flat flows.
- `attempt` captures the inner failure as `Result E A` data instead of propagating it immediately.
- `load` turns a typed `Source K A` into an `Effect (SourceError K) A`.

## Everyday effectful code uses flat flows

In v0.2, AIVI writes effectful workflows with flat flow syntax from [Flow Syntax](flows.md), not with a dedicated `do` block.

```aivi
loadConfig : Effect ConfigError Config
loadConfig =
  file.json {
    path: "./config.json"
    schema: source.schema.derive
  }
     |> load #cfg
     >|> cfg.enabled or fail (ConfigError "config is disabled")
     ~|> log.info "Loaded config for {cfg.appName}"
      |> normalizeConfig
```

Read that flow top to bottom:

1. build a typed source,
2. `load` it,
3. bind the successful result with `#cfg`,
4. reject invalid state with `>|> ... or fail ...`,
5. observe it with `~|>` without changing the subject,
6. continue with the normalized value.

### Common flow operators for effectful code

- `|>` — sequential step; pure steps map over the current subject, effectful steps bind and unwrap it.
- `~|>` — tap; run an effect for observation or logging and keep the incoming subject.
- `>|>` — guard; keep going when the predicate holds, or fail when `or fail ...` is provided.
- `?|>` / `!|>` — attempt and recover around a fallible step.
- `@cleanup` — register finalization for the successful result of a line.

For the full operator table, binding rules, and modifiers such as `@retry` or `@timeout`, see [Flow Syntax](flows.md).

## Recovering inline with `?|>` / `!|>`

Use `?|>` when you want to keep the workflow flat but recover from one specific failing step.

```aivi
chargeCustomer = request =>
  request
     ?|> payments.charge
     !|> CardDeclined err => notifyCardDeclined request err
     !|> RateLimited _    => queueForRetry request
      |> recordCharge request
```

`!|>` arms are contiguous, the first matching arm wins, and unmatched failures keep propagating.

## Capturing errors as data with `attempt`

`attempt` stays useful when you want explicit `Result`-shaped control flow rather than inline recovery.

```aivi
readGreeting = path =>
  path
     |> file.read
     |> attempt
     |> result => result match
          | Ok text  => "Loaded: {text}"
          | Err _    => "(missing)"
```

This is the right tool when the next step wants to inspect success and failure with ordinary `match` logic.

## Branching stays explicit

Flow syntax does not replace pattern matching.

- use `match` when you are branching on a value such as `Option`, `Result`, or an ADT,
- use `||>` when the branch itself is most naturally written as a flow-shaped handler,
- use helper functions when a branch needs several named steps and would be noisy inline.

## Cleanup and cancellation

Effects may register cleanup during a flow, usually with `@cleanup`.

```aivi
readAllText = path =>
  path
     |> file.open @cleanup file.close #handle
     |> file.readAll handle
```

Cleanup registration is scope-based:

- it happens only after the annotated line succeeds,
- it runs when the enclosing flow exits normally,
- it still runs when the flow fails,
- it still runs when the flow is cancelled,
- multiple cleanup registrations unwind in reverse order.

See [Cleanup & Lifetimes](resources.md) for the lifecycle rules.

## Conceptual lowering

Flat flows are the readable surface. Conceptually, effectful lines still lower through the same primitives:

- sequential success paths use `bind`,
- pure steps use `map`-like lifting over the current carrier,
- inline recovery is the flow surface for `attempt` plus pattern-based handling,
- cleanup is registered structurally on the enclosing flow scope.
