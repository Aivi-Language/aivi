# 3.4 Record Row Transforms

Record row transforms are type-level utilities for reusing and reshaping [closed record types](closed_records.md).
They help when you have one canonical record shape and need a few closely related variants, such as create-input, update-input, or public-response types.

If you know TypeScript utility types such as `Pick`, `Omit`, or `Partial`, this is the same general idea applied to AIVI record types.

## Syntax

`Pick`, `Omit`, `Optional`, `Required`, and `Defaulted` take a tuple of field labels.
`Rename` takes a record-shaped mapping from existing field names to replacement field names:

<<< ../../snippets/from_md/syntax/types/record_row_transforms_01.aivi{aivi}

::: repl
```aivi
type User = { id: Int, name: Text, email: Text, password: Text }
type PublicUser = Omit (password) User
// PublicUser = { id: Int, name: Text, email: Text }
```
:::

In the prefix form above, the transform comes first and the source record type comes last.
For example, `Pick (id, name) User` means “start from `User`, then keep only `id` and `name`”.
Likewise, `Rename { createdAt: created_at } User` means “start from `User`, then rename `createdAt` to `created_at`”.

These operators work at the type level only.
After elaboration (the compiler's expansion of shorthand into full record types), the compiler still works with ordinary closed record types.

## What each transform does

- `Pick` keeps only the listed fields.
- `Omit` removes the listed fields.
- `Optional` wraps each listed field type in `Option`; a field that already has type `Option T` stays `Option T`.
- `Required` unwraps `Option` for each listed field; non-optional fields are left unchanged.
- `Rename` renames only the listed fields. The mapping is `oldName: newName`; any field not mentioned stays unchanged, and collisions are errors.
- `Defaulted` produces the same type shape as `Optional`, but it communicates a different intent: the field may be omitted now because a later defaulting or codec step can supply it.

## Practical example

<<< ../../snippets/from_md/syntax/types/record_row_transforms/block_01.aivi{aivi}

The example above keeps a single source-of-truth record type and derives narrower or more convenient variants from it.
`PublicUser` removes a private field, `PatchUser` makes selected fields optional, and `RenamedUser` keeps the data but exposes a different field name.
That reduces duplication while keeping the final types explicit.

If you need to add or override fields rather than reshape existing ones, use type-level record spread from [Closed Records](closed_records.md):

```aivi
UserCore = { id: Int, name: Text }
UserWithEmail = { ...UserCore, email: Text }
```

The two features compose: row transforms reshape existing fields, while type-level spread extends the final closed record shape.

## Errors to expect

The compiler reports a type error when:

- you try to select, omit, or rename a field that does not exist
- `Rename` maps two fields to the same new name
- `Rename` maps a field onto the name of another field that still remains in the record

## Type-level piping

You can chain these transforms with type-level piping. It mirrors expression piping: the type on the left becomes the final argument to the transform on the right.

<<< ../../snippets/from_md/syntax/types/record_row_transforms_02.aivi{aivi}

desugars to (rewrites into):

<<< ../../snippets/from_md/syntax/types/record_row_transforms_03.aivi{aivi}

Both forms mean the same thing. The piped form is usually easier to read once you are applying more than one transform.
