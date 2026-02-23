# 3.4 Record Row Transforms

To avoid duplicating similar record shapes across layers, AIVI provides derived type operators
that transform record rows. These are type-level only and elaborate to plain record types.

Field lists are written as tuples of field labels, and rename maps use record-like syntax:

<<< ../../snippets/from_md/syntax/types/record_row_transforms_01.aivi{aivi}

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

<<< ../../snippets/from_md/syntax/types/record_row_transforms_02.aivi{aivi}

desugars to:

<<< ../../snippets/from_md/syntax/types/record_row_transforms_03.aivi{aivi}
