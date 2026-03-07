# 3.3 Closed Records

Records are AIVI's built-in way to group named fields.
AIVI records are:

- **structural** — compatibility depends on field names and field types, not on a declared class name
- **closed** — a record type describes an exact set of fields

<<< ../../snippets/from_md/syntax/types/closed_records_01.aivi{aivi}

## What “closed” means in practice

A closed record type says exactly which fields are present.
That makes record-based APIs easy to read: when you see the type, you know the full shape of the value.

## Creating record values

Use a record literal to build a value:

<<< ../../snippets/from_md/syntax/types/closed_records_02.aivi{aivi}

```aivi
user = {
  id: 1,
  name: "Ada"
}   // the value has exactly the fields `id` and `name`
```

## Reusing an existing record with spread

Record literals can spread existing records:

<<< ../../snippets/from_md/syntax/types/closed_records_03.aivi{aivi}

Spreads merge fields from left to right.
If the same field appears more than once, the later entry wins.

```aivi
base = { name: "Ada", role: "user" }
admin = { ...base, role: "admin" }   // later fields override earlier ones
```

## Records in function signatures

Functions use record types to state the exact shape they expect:

<<< ../../snippets/from_md/syntax/types/closed_records_04.aivi{aivi}

Because records are structural, two record values with the same field names and field types are compatible even if they were created in different places.
