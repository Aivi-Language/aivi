# The Type System

## 3.1 Primitive Types

AIVI distinguishes:

- **Compiler primitives**: types the compiler/runtime must know about to execute code.
- **Standard library types**: types defined in AIVI source (possibly with compiler-known representation in early implementations).

In v0.1, the recommended minimal set of **compiler primitives** is:

<<< ../snippets/from_md/02_syntax/03_types/block_01.aivi{aivi}

Everything else below should be treated as a **standard library type** (even if an implementation chooses to represent it specially at first for performance/interop).

<<< ../snippets/from_md/02_syntax/03_types/block_02.aivi{aivi}

Numeric suffixes:

* `2024-05-21T12:00:00Z` → `Instant`
* `~d(2024-05-21)` → `Date`
* `~t(12:00:00)` → `Time`
* `~tz(Europe/Paris)` → `TimeZone`
* `~zdt(2024-05-21T12:00:00Z[Europe/Paris])` → `ZonedDateTime`


## 3.2 Algebraic Data Types

### `Bool`

`Bool` has exactly two values:

<<< ../snippets/from_md/02_syntax/03_types/block_03.aivi{aivi}

`if ... then ... else ...` requires a `Bool` condition, and can be understood as desugaring to a `case` on `True`/`False`.

### Creating values (“objects”)

AIVI does not have “objects” in the OO sense. You create values using:

- **Constructors** for algebraic data types (ADTs)
- **Literals** for primitives and records
- **Domain-owned literals/operators** for domain types (e.g. `2w + 3d` for `Duration`)

<<< ../snippets/from_md/02_syntax/03_types/block_04.aivi{aivi}

To create ADT values, apply constructors like ordinary functions:

<<< ../snippets/from_md/02_syntax/03_types/block_05.aivi{aivi}

Nullary constructors (like `None`, `True`, `False`) are values.

## 3.3 Open Records (Row Polymorphism)

Records are:

* structural
* open by default

<<< ../snippets/from_md/02_syntax/03_types/block_06.aivi{aivi}

To create a record value, use a record literal:

<<< ../snippets/from_md/02_syntax/03_types/block_07.aivi{aivi}

Record literals can spread existing records:

<<< ../snippets/from_md/02_syntax/03_types/block_08.aivi{aivi}

Spreads merge fields left-to-right; later entries override earlier ones.

Functions specify **minimum required fields**, not exact shapes.

<<< ../snippets/from_md/02_syntax/03_types/block_09.aivi{aivi}

## 3.4 Record Row Transforms

To avoid duplicating similar record shapes across layers, AIVI provides derived type operators
that transform record rows. These are type-level only and elaborate to plain record types.

Field lists are written as tuples of field labels, and rename maps use record-like syntax:

<<< ../snippets/from_md/02_syntax/03_types/block_10.aivi{aivi}

Semantics:

- `Pick` keeps only the listed fields.
- `Omit` removes the listed fields.
- `Optional` wraps each listed field type in `Option` (if not already `Option`).
- `Required` unwraps `Option` for each listed field (if not `Option`, the type is unchanged).
- `Rename` renames fields; collisions are errors.
- `Defaulted` is equivalent to `Optional` at the type level and is reserved for codec/default derivation.

Errors:

- Selecting or renaming a field that does not exist in the source record is a type error.
- `Rename` collisions (two fields mapping to the same name, or a rename colliding with an existing field) are type errors.

Type-level piping mirrors expression piping and applies the left type as the final argument:

<<< ../snippets/from_md/02_syntax/03_types/block_11.aivi{aivi}

desugars to:

<<< ../snippets/from_md/02_syntax/03_types/block_12.aivi{aivi}


## 3.5 Classes and HKTs

<<< ../snippets/from_md/02_syntax/03_types/block_13.aivi{aivi}

<<< ../snippets/from_md/02_syntax/03_types/block_14.aivi{aivi}

<<< ../snippets/from_md/02_syntax/03_types/block_15.aivi{aivi}

<<< ../snippets/from_md/02_syntax/03_types/block_16.aivi{aivi}

<<< ../snippets/from_md/02_syntax/03_types/block_17.aivi{aivi}

### Type Variable Constraints

Class declarations may attach constraints to the **type variables used in member signatures**
using `with (...)`:

<<< ../snippets/from_md/02_syntax/03_types/block_21.aivi{aivi}

`A with B` in type position denotes **record/type composition** (an intersection-like merge). It is primarily used for class inheritance and trait aggregation in v0.1.

Instances:

<<< ../snippets/from_md/02_syntax/03_types/block_18.aivi{aivi}

Notes:

- `instance ClassName (TypeExpr) = { ... }` defines a dictionary value for a class implementation.
- In `Result E *`, `E` is a type parameter and `*` is the remaining type slot for higher-kinded types. Read it as: “`Result` with the error fixed to `E`, as a 1-parameter type constructor”.

> [!NOTE] Implementation Note: Kinds
> In the v0.1 compiler, kind annotations like `(F *)` were hints. The type checker now (planned) enforces kinds explicitly.

## 3.6 Expected-Type Coercions (Instance-Driven)

In some positions, the surrounding syntax provides an **expected type** (for example, function arguments,
record fields when a record literal is checked against a known record type, or annotated bindings).

In these expected-type positions only, the compiler may insert a conversion call when needed.
This is **not** a global implicit cast mechanism: conversions are only inserted when there is an
in-scope instance that authorizes the coercion.

### `ToText`

The standard library provides:

<<< ../snippets/from_md/02_syntax/03_types/block_19.aivi{aivi}

Rule (informal):

- When a `Text` is expected and an expression has type `A`, the compiler may rewrite the expression to
  `toText expr` if a `ToText A` instance is in scope.

This supports ergonomic boundary code such as HTTP requests:

<<< ../snippets/from_md/02_syntax/03_types/block_20.aivi{aivi}

### Record Instances

AIVI uses open structural records, so a record type like `{}` denotes "any record".
Implementations may ship a default instance `ToText {}` to support record-to-text coercions without
per-record boilerplate.

## 3.7 Machine Types (State Machines)

`machine` declares a finite state machine where **transitions are first-class**
and states are inferred from the transition graph. The compiler checks that every
referenced state appears as a source or target, and can enforce transition safety
at the type level.

### Declaration

```aivi
machine Door = {
  -> Closed   : init   {}
  Closed  -> Opened : open   {}
  Opened  -> Closed : close  {}
  Opened  -> Locked : lock   {}
  Locked  -> Closed : unlock {}
}
```

- `-> Closed : init {}` is the **initial transition** — it has no source state and
  marks `Closed` as the machine's starting state. Every machine must have exactly one
  initial transition.
- `Source -> Target : event { payload }` defines a named transition with an optional
  typed payload.

### Grammar

```text
MachineDecl      := "machine" UpperIdent "=" "{" { MachineTransition } "}"
MachineTransition := [ UpperIdent ] "->" UpperIdent ":" lowerIdent "{" { FieldDecl } "}"
FieldDecl         := lowerIdent ":" TypeExpr
```

### Semantics

- States are **inferred** from the set of sources and targets — no `state` keyword.
- The initial transition (`-> State : init {}`) designates the starting state and
  may carry a payload for initial data.
- `Source -> Target : event { fields }` defines a valid transition carrying typed data.
- The compiler verifies that all states appear on at least one side of a transition.
- Unreachable states or dead-end states produce warnings.
- Machine types can be used to model protocol states, UI states, or workflow steps.

### Formatter alignment

The LSP formatter aligns `->` arrows and `:` colons for readability:

```aivi
machine OrderFlow = {
  -> Pending              : init    {}
  Pending   -> Confirmed  : confirm { confirmedAt : Instant }
  Confirmed -> Shipped    : ship    { trackingId  : Text }
  Shipped   -> Delivered  : deliver { deliveredAt : Instant }
}
```

### Example: Traffic light

```aivi
machine TrafficLight = {
  -> Red   : init {}
  Red    -> Green  : next {}
  Green  -> Yellow : next {}
  Yellow -> Red    : next {}
}
```
