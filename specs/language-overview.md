# Language at a Glance

This page is the fastest way to get comfortable reading AIVI code.

It is written for people who already know how to program, but may be new to functional programming. You do not need to memorize everything here; use it as a guided cheat sheet and follow the linked reference pages when you want more depth.

## Suggested first pass

1. Skim this page once without trying to learn every rule.
2. Read [Introduction](introduction.md) for the bigger picture and the plain-language explanation of effects, data, and boundaries.
3. Then move through the **Learn AIVI** track in the sidebar, starting with Basics and Data & Types.

---

## The short version

AIVI code is built around a few simple rules:

- values are immutable,
- functions are first-class,
- data shapes are explicit,
- missing values and failures are modeled with types,
- side effects are written explicitly instead of being hidden.

If you keep those rules in mind, most of the syntax becomes easier to read.

---

## What looks familiar, what is different

| If you know... | In AIVI... |
|:---------------|:-----------|
| variables that change over time | you usually create a new value instead |
| `null` / `undefined` | use `Option A` |
| exceptions | use `Result E A` or `Effect E A` |
| loops | use pipelines, generators, folds, or recursion |
| objects with mutable fields | use typed records and patch them into new records |
| helper methods like `users.map(...)` | use functions like `map f users` or `users |> map f` |

---

## Core design

- **Statically typed** and expression-oriented
- **Pure by default** with explicit effects
- **Immutable bindings** — no mutation and no `let`
- **No `null`** and no unchecked exceptions
- **Closed records** instead of open object shapes
- **Pattern matching** for structured data
- **Domains** for units, operator meaning, and domain-specific literals

---

## Lexical basics

| Element | Syntax |
|:--------|:-------|
| Line comment | `// to end of line` |
| Block comment | `/* ... */` |
| Value / function name | `lowerCamelCase` |
| Type / constructor / domain name | `UpperCamelCase` |
| Module path segment / file name | `snake_case` |
| Text literal | `"hello { name }"` |
| Numeric literals | `42`, `3.14` |
| Character literal | `'a'` |
| ISO instant | `2024-05-21T12:00:00Z` |
| Suffixed number | `10px`, `30s`, `100%` |
| Common constructors | `True`, `False`, `None`, `Some`, `Ok`, `Err` |

---

## Bindings and functions

<<< ./snippets/from_md/language-overview/block_01.aivi{aivi}


Useful reading rules:

- `=` creates a binding.
- `a b => ...` means a function taking `a` and `b`.
- `f x y` means "call `f` with `x` and `y`".
- `x |> f` means the same as `f x`.

See [Bindings & Scope](syntax/bindings) and [Functions & Pipes](syntax/functions).

---

## Records, lists, and common data shapes

<<< ./snippets/from_md/language-overview/block_02.aivi{aivi}


AIVI makes data shapes explicit. `User` is a record type with exactly those fields. `Option Text` means the email may be missing, but that possibility is part of the type. A custom data type such as `Option` is often called an **algebraic data type (ADT)**, which simply means a type made from named cases like `Some` and `None`.

See [Primitive Types](syntax/types/primitive_types), [Records](syntax/types/closed_records), and [Custom Data Types (ADTs)](syntax/types/algebraic_data_types).

---

## Handling missing values and failures

<<< ./snippets/from_md/language-overview/block_03.aivi{aivi}


This is a major AIVI habit:

- use `Option` when a value may be absent,
- use `Result` when an operation may fail,
- use `match` to handle each case clearly.

See [Pattern Matching](syntax/pattern_matching), [Option](stdlib/core/option), and [Result](stdlib/core/result).

---

## Updating data without mutation

```aivi
user2 = user <| { name: "Ada Lovelace" }
// Create a new record with one field changed.

user3 = user <| { profile.avatar: "new.png" }
// Nested fields can be updated structurally as well.
```

`<|` is a patch operator. It does not modify `user`; it produces a new value.

See [Patching Records](syntax/patching).

---

## Working with collections

<<< ./snippets/from_md/language-overview/block_05.aivi{aivi}


Read that as:

1. start with `users`,
2. keep only active users,
3. extract each `name`,
4. sort the resulting list.

AIVI leans heavily on this pipeline style because it keeps transformations readable and avoids temporary mutable state.

See [Predicates](syntax/predicates), [Collections](stdlib/core/collections), and [Generator](stdlib/core/generator).

---

## Effects and `do Effect`

Pure code and effectful code are separated.

<<< ./snippets/from_md/language-overview/block_06.aivi{aivi}


`Effect FileError Text` means: this computation may perform effects, may fail with `FileError`, and if it succeeds it returns `Text`.

If you are new to FP, the important thing is not the jargon. The important thing is that file access, network access, printing, and other side effects are visible in the type instead of being hidden.

See [Effects](syntax/effects), [do Notation](syntax/do_notation), and [Resources](syntax/resources).

---

## Generators instead of loops

<<< ./snippets/from_md/language-overview/block_07.aivi{aivi}


Generators are a convenient way to describe a stream of values without writing a mutable loop.

See [Generators](syntax/generators).

---

## Modules and imports

<<< ./snippets/from_md/language-overview/block_08.aivi{aivi}


AIVI uses one module per file. Module paths and file names are written in `snake_case`.

See [Modules](syntax/modules).

---

## Domains and units

<<< ./snippets/from_md/language-overview/block_09.aivi{aivi}


Domains let operators and literals carry domain meaning. This is useful for units, time, finance, geometry, UI layout, and other areas where plain numbers are not descriptive enough.

See [Domains & Units](syntax/domains).

---

## External sources

<<< ./snippets/from_md/language-overview/block_10.aivi{aivi}


AIVI treats boundaries such as files, REST APIs, environment variables, and email as typed sources. The expected output type guides decoding and error reporting.

See [External Sources](syntax/external_sources) and the source-specific pages under `syntax/external_sources/`.

---

## Decorators at a glance

Decorators are compile-time annotations that affect tooling or compilation behavior.

- `@static` embeds deterministic source reads at compile time.
- `@native` connects a declaration to a native host function.
- `@deprecated` marks an API as obsolete and emits warnings.
- `@debug` enables trace instrumentation.
- `@test` marks tests or test-only helpers.
- `@no_prelude` disables the automatic prelude import for a module.

See [Decorators](syntax/decorators/).

---

## A tiny end-to-end example

<<< ./snippets/from_md/language-overview/block_11.aivi{aivi}


This single example shows much of the language style:

- a typed record,
- an explicit effect,
- a typed file boundary,
- a pipeline that transforms immutable data.

---

## Where to go next

- **Start writing code:** continue with [Bindings & Scope](syntax/bindings) and [Functions & Pipes](syntax/functions).
- **Learn how AIVI models data:** read [Primitive Types](syntax/types/primitive_types), [Custom Data Types (ADTs)](syntax/types/algebraic_data_types), and [Records](syntax/types/closed_records).
- **Work with I/O and failures:** read [Effects](syntax/effects), [do Notation](syntax/do_notation), and [Resources](syntax/resources).
- **Connect to files and services:** read [Modules](syntax/modules) and [External Sources](syntax/external_sources).
- **Need the big picture first?** Read [Introduction](introduction).
