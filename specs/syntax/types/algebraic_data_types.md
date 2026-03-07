# 3.2 Algebraic Data Types

Algebraic data types (ADTs) let you model values that can be in one of several named cases.
If you come from an OO background, an ADT often plays the role that an enum, tagged union, or small class hierarchy would play in other languages.

## `Bool`

`Bool` is the simplest ADT: it has exactly two values.

<<< ../../snippets/from_md/syntax/types/bool.aivi{aivi}

`if ... then ... else ...` requires a `Bool` condition.
You can think of `if` as a convenient form of pattern matching over `True` and `False`.

## Why ADTs are useful

Use an ADT when a value must be one of a small set of clearly named cases.
Common examples include:

- success vs. failure (`Result`)
- present vs. missing (`Option`)
- states in a workflow (`Draft | Published | Archived`)

Because each case is named, code that consumes the value can stay explicit and readable.

## Creating values

AIVI does not have "objects" in the OO sense.
You create values using:

- **constructors** for algebraic data types
- **literals** for primitives and records
- **domain-owned literals/operators** for domain types such as `Duration`

<<< ../../snippets/from_md/syntax/types/creating_values_objects_01.aivi{aivi}

To create an ADT value, call its constructor like an ordinary function:

<<< ../../snippets/from_md/syntax/types/creating_values_objects_02.aivi{aivi}

Nullary constructors such as `None`, `True`, and `False` are already complete values, so they do not take arguments.

## A small mental model

```aivi
maybeName = Some "Ada"   // `Some` wraps a value and tags which case we are using
isReady = True           // `True` is a nullary constructor

message = maybeName match
  | Some name => "Hello {name}"   // read the payload carried by `Some`
  | None      => "Hello"          // handle the empty case explicitly
```

A good rule of thumb: if you find yourself reaching for sentinel values like empty strings or `0` to mean "special case", an ADT is often a better fit.
