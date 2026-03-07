# Introduction

AIVI is a statically typed, purely functional language for building software that needs clear data models, predictable behavior, and explicit handling of real-world side effects.

That description can sound abstract if you do not already live in functional-programming terminology, so here is the practical version: AIVI is built for codebases where you want the compiler to help you keep track of data shape, missing values, failures, units, and system boundaries before the program runs.

## A simple learning path

If this is your first pass through the docs, use this order:

1. Read [Language at a Glance](language-overview.md) for a short syntax tour.
2. Continue into the **Learn AIVI** track: [Bindings & Scope](syntax/bindings.md), [Functions & Pipes](syntax/functions.md), then the core data and effects pages.
3. Read [Modules](syntax/modules.md) and [External Sources](syntax/external_sources.md) when you are ready to work with files, APIs, or other outside data.
4. Jump to [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps.md) if your goal is desktop UI work.

## What AIVI is for

AIVI is a good fit when your program spends a lot of time doing things like:

- moving data between files, APIs, databases, and user interfaces,
- validating structured input and transforming it into safer internal models,
- coordinating time, units, or domain-specific calculations,
- keeping error cases visible instead of hiding them in `null`, mutable state, or unchecked exceptions.

Typical examples include ETL jobs, backend services, API clients, automation tools, data-heavy business applications, and GTK-based desktop apps.

## The core idea in one example

```aivi
User = {
  id: Int,
  name: Text,
  email: Option Text
}

formatUser : User -> Text
formatUser = user =>
  // `match` makes the "email might be missing" case explicit.
  user.email match
    | Some email => "{user.name} <{email}>"
    | None       => user.name
```

A few important ideas show up immediately:

- `User` has a **closed, explicit shape**.
- `email` is not allowed to be `null`; it is `Option Text`, which means "either a `Text` value or no value".
- `match` forces you to handle both cases, so missing data cannot be ignored by accident.

## Thinking in AIVI if you come from other languages

If you know Rust, TypeScript, Python, Kotlin, or JavaScript, these mental shifts matter most:

### 1. Bindings are immutable

You do not update a variable in place. You compute a new value.

```aivi
cart = { total: 100, tax: 20 }
updatedCart = cart <| { total: 120 }
// `cart` is unchanged; `updatedCart` is the new record.
```

### 2. Everything important is in the type

Instead of treating failure or missing data as a runtime surprise, AIVI keeps it in the function signature.

```aivi
findUser : Int -> Effect LoadError (Option User)
```

That reads as: "finding a user performs effects, can fail with `LoadError`, and if it succeeds it may still return no user." You do not have to guess whether a function can return `null`, throw, or talk to the network — the type tells you.

### 3. Pattern matching replaces many `if` trees

```aivi
label : Result Text User -> Text
label = result =>
  result match
    | Ok user  => "Hello, {user.name}!"
    | Err text => "Could not load user: {text}"
```

Pattern matching is the standard way to work with structured data like `Option`, `Result`, tuples, records, and your own tagged unions.

### 4. Loops give way to transformations

Instead of mutating counters or building collections step by step, you usually transform data with functions, generators, folds, or recursion.

```aivi
activeNames =
  users
  |> filter (_.active)
  |> map .name
```

That pipeline says: start with `users`, keep only the active ones, then extract each `name`.

## Effects are explicit, not hidden

AIVI separates pure calculations from operations that interact with the outside world.

```aivi
loadConfig : Path -> Effect ConfigError Config
loadConfig = path => do Effect {
  raw <- file.read path                // effect: read a file
  cfg <- json.decode raw               // effectful decode that may fail
  pure cfg                             // return the final value
}
```

This is one of the biggest differences from mainstream languages:

- pure functions are easy to test and reason about,
- side effects are visible in the type system,
- error handling stays structured instead of spreading through ad-hoc `try/catch` logic.

The detailed model lives in [Effects](syntax/effects.md), [do Notation](syntax/do_notation.md), and [Resources](syntax/resources.md).

## Missing values and failures stay explicit

AIVI uses familiar building blocks instead of special runtime behavior:

- `Option A` for "a value might be absent",
- `Result E A` for "this can fail with error `E` or succeed with `A`",
- `Effect E A` for "this computation performs effects and may fail with `E`".

```aivi
parsePort : Text -> Result Text Int
parsePort = text =>
  textToInt text match
    | Some port => Ok port
    | None      => Err "Port must be a whole number"
```

If you are used to `null`, exceptions, or out-of-band sentinel values, this style may feel more explicit at first. In practice, it makes programs easier to understand because the important edge cases are not hidden.

## AIVI is domain-oriented

AIVI is not just about pure functions. It is also designed to let code speak in the language of the problem domain.

Domains define the meaning of operators, literals, and units for specific kinds of values. That lets code stay readable without losing type safety.

```aivi
domain Distance = {
  (+) : Distance -> Distance -> Distance
}

trip = 5km + 800m
// The units are part of the program, not comments on a raw number.
```

This matters when you work with time, geometry, UI layout, finance, measurements, and other areas where plain `Int` or `Float` values are too weak to describe intent.

## Working with files, APIs, and other boundaries

AIVI puts special emphasis on **typed external sources**. The goal is simple: when data crosses into your program, the expected shape should already be known and checked.

```aivi
loadCustomers : Effect LoadError (List Customer)
loadCustomers = do Effect {
  customers <- load (file.csv "customers.csv")
  pure customers
}
```

In many languages, the risky part of a boundary is hidden in manual parsing code. In AIVI, boundary operations are part of the language and the type system, so decoding, failure modes, and expected structure are easier to see.

Start with [External Sources](syntax/external_sources.md) if this is the part of the language you care about most.

## Functional programming concepts, in plain language

If some of the formal terms are new, this is the shortest useful translation:

- **Pure function**: a function whose result depends only on its inputs and does not change anything outside itself.
- **Algebraic data type (ADT)**: a custom type made from named cases, such as `Option` being either `None` or `Some value`.
- **Pattern matching**: unpacking a value by shape instead of manually checking flags or tags.
- **Higher-kinded type / type class**: abstraction tools for reusable container-style behavior such as mapping, chaining, and combining values. You can ignore the theory at first and still write practical AIVI code.

## Where to go next

- **Just learning the language?** Follow the **Learn AIVI** track in this order: [Bindings & Scope](syntax/bindings.md), [Functions & Pipes](syntax/functions.md), [Primitive Types](syntax/types/primitive_types.md), [Custom Data Types (ADTs)](syntax/types/algebraic_data_types.md), [Records](syntax/types/closed_records.md), then [Effects](syntax/effects.md).
- **Working with real-world inputs?** Continue with [Modules](syntax/modules.md) and [External Sources](syntax/external_sources.md).
- **Building desktop apps?** Jump to [GTK & libadwaita Apps](stdlib/ui/native_gtk_apps.md) and [App Architecture](stdlib/ui/app_architecture.md).
- **Need exact rules or advanced features?** Use the rest of the sidebar as a reference, especially Advanced Features, Standard Library, and Internals.
