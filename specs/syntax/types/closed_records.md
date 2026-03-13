# 3.3 Closed Records

<!-- quick-info: {"kind":"topic","name":"closed records"} -->
Closed records describe fixed-shape data with named fields. Record compatibility is structural: field names and field types must match, regardless of where the record type was introduced.
<!-- /quick-info -->

Records are AIVI's built-in way to group related named fields into one value.

AIVI records are:

- **structural** — compatibility depends on field names and field types
- **closed** — a record type describes an exact set of fields

<<< ../../snippets/from_md/syntax/types/closed_records/block_01.aivi{aivi}

::: repl
```aivi
user = { name: "Ada", age: 30 }
user.name
// => "Ada"
updated = { ...user, age: 31 }
updated.age
// => 31
```
:::

Here `Option Text` means the `email` field always exists, but its value is either `Some "ada@example.com"` or `None`.
If `Option` is new to you, see [3.2 Algebraic Data Types](algebraic_data_types.md).

## What “closed” means in practice

A closed record type says exactly which fields are present.
That makes record-based APIs easy to read: when you see the type, you know the full shape of the value.

If you need a related shape such as “just these fields” or “the same fields, but optional”, see [3.4 Record Row Transforms](record_row_transforms.md).

## Creating record values

Use a record literal to build a value whose fields match the record type:

<<< ../../snippets/from_md/syntax/types/closed_records/block_02.aivi{aivi}


## Reusing an existing record with spread

Record literals can spread an existing record and then override selected fields:

<<< ../../snippets/from_md/syntax/types/closed_records/block_03.aivi{aivi}


Spreads merge fields from left to right.
If the same field appears more than once, the later entry wins.

Use spread when you want to build a nearby value inline.
For typed structural updates that can reach nested data, see [Patching Records](../patching.md).

## Reusing an existing record type with spread

Record types can also spread another closed record type and then add or override fields:

```aivi
UserCore = { id: Int, name: Text }
UserWithEmail = { ...UserCore, email: Text }
UserDraft = { ...UserWithEmail, email: Option Text }
```

Type-level spreads follow the same left-to-right rule as value-level spreads:

- later entries override earlier ones
- the spread target must elaborate to a closed record type
- after elaboration the result is still an ordinary closed record type

Use type-level spread when you want to extend a canonical record shape with a few extra fields.
Use [Record Row Transforms](record_row_transforms.md) when you want to pick, omit, rename, or change optionality of existing fields.

## Records in function signatures

Functions can use record types to state the exact shape they expect:

<<< ../../snippets/from_md/syntax/types/closed_records/block_04.aivi{aivi}


Because records are structural, any value with exactly the fields `id : Int` and `name : Text` is compatible with that signature, even if it came from a different type alias.
For destructuring record fields in patterns, see [Pattern Matching](../pattern_matching.md).
