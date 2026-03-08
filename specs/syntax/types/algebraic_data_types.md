# 3.2 Algebraic Data Types

Algebraic data types (ADTs) let you say that a value is **exactly one of several named cases**.
They are a good fit when only one alternative can be true at a time: present or missing, success or failure, draft or published.

If you come from an OO background, an ADT often fills the role of an enum, tagged union, or small closed class hierarchy.
If you come from a data-modeling background, think of an ADT as a fixed set of alternatives where each alternative can carry its own payload.

## `Bool`: the smallest ADT

`Bool` is the simplest ADT: it has exactly two constructors.

<<< ../../snippets/from_md/syntax/types/bool.aivi{aivi}

`if ... then ... else ...` requires a `Bool` condition.
You can think of `if` as the two-case special form of branching on `True` and `False`; for general branching over ADTs, use [`match`](../pattern_matching.md).

## Why ADTs are useful

Use an ADT when a value must be one of a **fixed** set of clearly named cases.
Common examples include:

- success vs. failure ([`Result`](../../stdlib/core/result.md))
- present vs. missing ([`Option`](../../stdlib/core/option.md))
- states in a workflow (`Status = Draft | Published | Archived`)

Because each case is named, code that consumes the value stays explicit and readable.
An ADT also makes invalid combinations unrepresentable: a `Result` cannot be both `Err` and `Ok` at the same time.

If you find yourself reaching for sentinel values like an empty string or `0` to mean "special case", an ADT is often a better fit.

## Declaring algebraic data types

An ADT declaration puts the type name on the left of `=` and a list of constructors on the right, separated by `|`.
Constructors start with uppercase names.
Each constructor may carry zero values (`None`), one value (`Some A`), or several values (`Connected Text Int`).
Type parameters appear between the type name and `=`.

<<< ../../snippets/from_md/syntax/types/creating_values_objects_01.aivi{aivi}

Those examples define two generic ADTs:

- `Option A = None | Some A`
- `Result E A = Err E | Ok A`

The same syntax works for your own types too:

- `Status = Draft | Published | Archived`
- `Connection = Disconnected | Connecting Text | Connected Text Int`

## Creating values

AIVI does not have OO-style objects with instance state and methods.
Instead, you create values using:

- **constructors** for algebraic data types
- **literals** for primitives and records
- **domain-owned literals/operators** for domain types such as `Duration`

To create an ADT value, call its constructor like an ordinary function:

<<< ../../snippets/from_md/syntax/types/creating_values_objects_02.aivi{aivi}

Nullary constructors such as `None`, `True`, and `False` are already complete values, so they do not take arguments.
If a constructor carries several values, pass them in order, for example `Connected "db" 5432`.

## Consuming ADTs with pattern matching

To read which constructor you received, and to extract any values it carries, use `match`:

<<< ../../snippets/from_md/syntax/types/algebraic_data_types/block_01.aivi{aivi}

Each arm starts with `|`, names a pattern, and ends with `=> expression`.
The compiler checks constructor matches for exhaustiveness, so you normally handle every case explicitly or use `_` only when a catch-all is genuinely appropriate.

For nested patterns, guards, and more advanced forms, see [Pattern Matching](../pattern_matching.md).

## ADTs vs. closed records

Use an ADT when exactly one alternative is valid at a time:

- `Option A = None | Some A`
- `Result E A = Err E | Ok A`

Use a [closed record](closed_records.md) when several named fields coexist in one value:

- `{ name: Text, email: Text, age: Int }`

A record can hold many facts at once.
An ADT says "pick one case, and only that case."

## Visibility and `opaque`

You can make an ADT `opaque` to hide its constructors outside the defining module.
That is useful when callers should go through validated constructors or helper functions instead of constructing cases directly.
See [Opaque Types](opaque_types.md) for the full visibility rules.

## Related pages

- [Pattern Matching](../pattern_matching.md) — branching on constructors and extracting payloads
- [Closed Records](closed_records.md) — fixed-shape data with named fields
- [Opaque Types](opaque_types.md) — hiding constructors outside a module
- [Option](../../stdlib/core/option.md) and [Result](../../stdlib/core/result.md) — the standard-library ADTs you will use most often
- [Prelude](../../stdlib/core/prelude.md) — core types and helper functions available by default
