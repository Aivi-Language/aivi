# Introduction

AIVI is a statically typed, purely functional language for building software that needs clear data models, predictable behavior, and explicit handling of real-world side effects.

In plain language, **statically typed** means the compiler checks data shapes and function signatures before the program runs, and **purely functional** means values do not mutate in place and side effects stay visible instead of being hidden behind ordinary-looking calls.

That can still sound abstract if you do not already live in functional-programming terminology, so here is the practical version: AIVI is built for codebases where you want the compiler to help you keep track of data shape, missing values, failures, units, and system boundaries before the program runs.

## A simple learning path

If this is your first pass through the docs, use this order:

1. Read [Language at a Glance](language-overview) for a short syntax tour.
2. Continue into the **Learn AIVI** track: [Bindings & Scope](syntax/bindings), [Functions & Pipes](syntax/functions), [Primitive Types](syntax/types/primitive_types), [Custom Data Types (ADTs)](syntax/types/algebraic_data_types), [Records](syntax/types/closed_records), and [Effects](syntax/effects).
3. Read [Modules](syntax/modules) and [External Sources](syntax/external_sources) when you are ready to work with files, APIs, or other outside data.
4. Jump to [`aivi.ui.gtk4`](stdlib/ui/gtk4) if your goal is desktop UI work.

## What AIVI is for

AIVI is a good fit when your program spends a lot of time doing things like:

- moving data between files, APIs, databases, and user interfaces,
- validating structured input and transforming it into safer internal models,
- coordinating time, units, or domain-specific calculations,
- keeping error cases visible instead of hiding them in `null`, mutable state, or unchecked exceptions.

Typical examples include ETL jobs, backend services, API clients, automation tools, data-heavy business applications, and GTK-based desktop apps.

## The core idea in one example

<<< ./snippets/from_md/introduction/block_01.aivi{aivi}


A few important ideas show up immediately:

- `User` has a **closed, explicit shape**.
- `email` is not allowed to be `null`; it is `Option Text`, which means "either a `Text` value or no value".
- `match` forces you to handle both cases, so missing data cannot be ignored by accident.

## Thinking in AIVI if you come from other languages

If you know Rust, TypeScript, Python, Kotlin, or JavaScript, these mental shifts matter most:

### 1. Bindings are immutable

You do not update a variable in place. You compute a new value.

<<< ./snippets/from_md/introduction/block_01.aivi{aivi}


### 2. Everything important is in the type

Instead of treating failure or missing data as a runtime surprise, AIVI keeps it in the function signature.

<<< ./snippets/from_md/introduction/block_02.aivi{aivi}


That reads as: "finding a user performs effects, can fail with `LoadError`, and if it succeeds it may still return no user." You do not have to guess whether a function can return `null`, throw, or talk to the network — the type tells you.

### 3. Pattern matching replaces many `if` trees

<<< ./snippets/from_md/introduction/block_04.aivi{aivi}


Pattern matching is the standard way to work with structured data like `Option`, `Result`, tuples, records, and your own tagged unions.

### 4. Loops give way to transformations

Instead of mutating counters or building collections step by step, you usually transform data with functions, flow fan-out (`*|>` ... `*-|`), folds, or recursion.

<<< ./snippets/from_md/introduction/block_03.aivi{aivi}


That pipeline says: start with `users`, keep only the active ones, then extract each `name`.

## Effects are explicit, not hidden

AIVI separates pure calculations from operations that interact with the outside world.

<<< ./snippets/from_md/introduction/block_06.aivi{aivi}


This is one of the biggest differences from mainstream languages:

- pure functions are easy to test and reason about,
- side effects are visible in the type system,
- error handling stays structured instead of spreading through ad-hoc `try/catch` logic.

The detailed model lives in [Flow Syntax](syntax/flows), [Effects](syntax/effects), and [Cleanup & Lifetimes](syntax/resources).

## Missing values and failures stay explicit

AIVI uses familiar building blocks instead of special runtime behavior:

- `Option A` for "a value might be absent",
- `Result E A` for "this can fail with error `E` or succeed with `A`",
- `Effect E A` for "this computation performs effects and may fail with `E`".

<<< ./snippets/from_md/introduction/block_07.aivi{aivi}


If you are used to `null`, exceptions, or out-of-band sentinel values, this style may feel more explicit at first. In practice, it makes programs easier to understand because the important edge cases are not hidden.

## AIVI is domain-oriented

AIVI is not just about pure functions. It is also designed to let code speak in the language of the problem domain.

Domains define the meaning of operators, literals, and units for specific kinds of values. That lets code stay readable without losing type safety.

<<< ./snippets/from_md/introduction/block_08.aivi{aivi}


This matters when you work with time, geometry, UI layout, finance, measurements, and other areas where plain `Int` or `Float` values are too weak to describe intent.

## Working with files, APIs, and other boundaries

AIVI puts special emphasis on **typed external sources**. The goal is simple: when data crosses into your program, the expected shape should already be known and checked.

<<< ./snippets/from_md/introduction/block_09.aivi{aivi}


In many languages, the risky part of a boundary is hidden in manual parsing code. In AIVI, boundary operations are part of the language and the type system, so decoding, failure modes, and expected structure are easier to see.

Start with [External Sources](syntax/external_sources) if this is the part of the language you care about most.

## Functional programming concepts, in plain language

If some of the formal terms are new, this is the shortest useful translation:

- **Pure function**: a function whose result depends only on its inputs and does not change anything outside itself.
- **Algebraic data type (ADT)**: a custom type made from named cases, such as `Option` being either `None` or `Some value`.
- **Pattern matching**: unpacking a value by shape instead of manually checking flags or tags.
- **Higher-kinded type / type class**: reusable abstractions over container shapes such as `List` or `Option`, used for operations like `map`, `chain`, and combining values. You can ignore the theory at first and still write practical AIVI code.

## Where to go next

- **Just learning the language?** Follow the **Learn AIVI** track in this order: [Bindings & Scope](syntax/bindings), [Functions & Pipes](syntax/functions), [Primitive Types](syntax/types/primitive_types), [Custom Data Types (ADTs)](syntax/types/algebraic_data_types), [Records](syntax/types/closed_records), then [Effects](syntax/effects).
- **Working with real-world inputs?** Continue with [Modules](syntax/modules) and [External Sources](syntax/external_sources).
- **Building desktop apps?** Jump to [`aivi.ui.gtk4`](stdlib/ui/gtk4), then follow [Signals](stdlib/ui/reactive_signals) and [Reactive Dataflow](stdlib/ui/reactive_dataflow).
- **Need exact rules or advanced features?** Use the rest of the sidebar as a reference, especially Advanced Features, Standard Library, and Internals.
