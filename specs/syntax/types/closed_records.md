# 3.3 Closed Records

<!-- quick-info: {"kind":"topic","name":"closed records"} -->
Closed records describe fixed-shape data with named fields. Record compatibility is structural: field names and field types must match, regardless of where the record type was introduced.
<!-- /quick-info -->

Records are AIVI's built-in way to group related named fields into one value.

AIVI records are:

- **structural** — compatibility depends on field names and field types
- **closed** — a record type describes an exact set of fields

```aivi
User = { id: Int, name: Text, email: Option Text }
```

Here `Option Text` means the `email` field always exists, but its value is either `Some "ada@example.com"` or `None`.
If `Option` is new to you, see [3.2 Algebraic Data Types](algebraic_data_types.md).

## What “closed” means in practice

A closed record type says exactly which fields are present.
That makes record-based APIs easy to read: when you see the type, you know the full shape of the value.

If you need a related shape such as “just these fields” or “the same fields, but optional”, see [3.4 Record Row Transforms](record_row_transforms.md).

## Creating record values

Use a record literal to build a value whose fields match the record type:

```aivi
user : User
user = {
  id: 1,
  name: "Ada",
  email: None
}
```

## Reusing an existing record with spread

Record literals can spread an existing record and then override selected fields:

```aivi
base = { name: "Ada", role: "user" }
admin = { ...base, role: "admin" }
```

Spreads merge fields from left to right.
If the same field appears more than once, the later entry wins.

Use spread when you want to build a nearby value inline.
For typed structural updates that can reach nested data, see [Patching Records](../patching.md).

## Records in function signatures

Functions can use record types to state the exact shape they expect:

```aivi
displayName : { id: Int, name: Text } -> Text
displayName = user => user.name
```

Because records are structural, any value with exactly the fields `id : Int` and `name : Text` is compatible with that signature, even if it came from a different type alias.
For destructuring record fields in patterns, see [Pattern Matching](../pattern_matching.md).
