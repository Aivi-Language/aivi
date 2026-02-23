# 3.2 Algebraic Data Types

## `Bool`

`Bool` has exactly two values:

<<< ../../snippets/from_md/syntax/types/bool.aivi{aivi}

`if ... then ... else ...` requires a `Bool` condition, and can be understood as desugaring to a `case` on `True`/`False`.

## Creating values (“objects”)

AIVI does not have “objects” in the OO sense. You create values using:

- **Constructors** for algebraic data types (ADTs)
- **Literals** for primitives and records
- **Domain-owned literals/operators** for domain types (e.g. `2w + 3d` for `Duration`)

<<< ../../snippets/from_md/syntax/types/creating_values_objects_01.aivi{aivi}

To create ADT values, apply constructors like ordinary functions:

<<< ../../snippets/from_md/syntax/types/creating_values_objects_02.aivi{aivi}

Nullary constructors (like `None`, `True`, `False`) are values.
