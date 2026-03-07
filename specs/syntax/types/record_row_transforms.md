# 3.4 Record Row Transforms

Record row transforms are type-level utilities for reusing and reshaping closed record types.
They help when you have one canonical record shape and need a few closely related variants, such as create-input, update-input, or public-response types.

If you know TypeScript utility types such as `Pick`, `Omit`, or `Partial`, this is the same general idea applied to AIVI record types.

## Syntax

Field lists are written as tuples of field labels, and rename maps use record-like syntax:

<<< ../../snippets/from_md/syntax/types/record_row_transforms_01.aivi{aivi}

Read the snippet as “take the record type on the left, then derive a nearby record shape with the transform on the right”.

These operators work at the type level only.
After elaboration, the compiler still works with ordinary closed record types.

## What each transform does

- `Pick` keeps only the listed fields.
- `Omit` removes the listed fields.
- `Optional` wraps each listed field type in `Option` if it is not already optional.
- `Required` unwraps `Option` for each listed field; non-optional fields are left unchanged.
- `Rename` changes field names; collisions are errors.
- `Defaulted` is equivalent to `Optional` at the type level and is reserved for codec and default-derivation workflows.

## Practical example

```aivi
User = {
  id: Int,
  email: Text,
  name: Text,
  isAdmin: Bool
}

PublicUser = User |> Omit ("isAdmin")
PatchUser = User |> Optional ("email", "name")
RenamedUser = User |> Rename { email: "loginEmail" }
```

The example above keeps a single source-of-truth record type and derives narrower or more convenient variants from it.
That reduces duplication while keeping the final types explicit.

## Errors to expect

The compiler reports a type error when:

- you try to select, omit, or rename a field that does not exist
- `Rename` would cause two fields to end up with the same name
- a rename collides with an existing field name that remains in the record

## Type-level piping

You can chain these transforms with type-level piping. It mirrors expression piping: the type on the left becomes the final argument to the transform on the right.

<<< ../../snippets/from_md/syntax/types/record_row_transforms_02.aivi{aivi}

desugars to:

<<< ../../snippets/from_md/syntax/types/record_row_transforms_03.aivi{aivi}
