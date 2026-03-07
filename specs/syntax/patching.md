# Patching Records

<!-- quick-info: {"kind":"operator","name":"<|"} -->
The `<|` operator applies a **declarative structural patch**.

The compiler enforces that the patch shape matches the target record's type, ensuring that only existing fields are updated or new fields are added according to the record's openness. When a patch path selects a `Map` entry, the patch applies to the **value** stored at that key.
<!-- /quick-info -->

Patching is AIVI’s built-in way to update immutable records and collections without rebuilding every nested layer by hand.

## What patching is for

Use patching when you want to:

- update deeply nested data in one expression
- transform part of a structure while leaving the rest unchanged
- reuse a structural update as a value
- keep updates type-checked even when they cross records, collections, maps, or sum types

A patch never mutates the original value. It returns a new value with the requested structural changes.

<<< ../snippets/from_md/syntax/patching/patching_records_01.aivi{aivi}

Patching is:

- immutable
- compositional
- type-checked

`Patch A` is a first-class alias for `A -> A`, so you can store, pass, and compose patches like ordinary functions.

## Applying a patch now or building one for later

Apply a patch immediately with `<|`.

Lift a patch literal into a reusable function with `patch { ... }`.

<<< ../snippets/from_md/syntax/patching/patching_records_02.aivi{aivi}

> Comment: use `<|` when you already have the value in hand; use `patch { ... }` when you want a reusable update function.

## What the compiler checks

The compiler validates the shape of a patch against the target type.

It reports an error when:

- a patch path refers to an unknown field or constructor
- a predicate selector does not type-check to `Bool`
- a map selector uses the wrong key type
- a removal would leave an invalid closed record type

That means patching stays convenient without becoming “stringly typed”.

## Addressing the part you want to change

### Dot paths

Use dot paths to reach nested record fields.

<<< ../snippets/from_md/syntax/patching/dot_paths.aivi{aivi}

### Traversals

Use traversals when you want to patch many values inside a collection-like structure.

<<< ../snippets/from_md/syntax/patching/traversals.aivi{aivi}

> Comment: a traversal applies the instruction to every selected element, not just the first one.

### Predicates

Use a predicate when you want to patch only the elements that match a condition.

<<< ../snippets/from_md/syntax/patching/predicates.aivi{aivi}

> Comment: a predicate must evaluate to `Bool`, so the compiler can check that the selection logic is meaningful.

### Map key selectors

When the current focus is a `Map`, selectors address entries by key. After selection, the patch works on the **value** stored at that key.

<<< ../snippets/from_md/syntax/patching/map_key_selectors.aivi{aivi}

Inside a map predicate, the current element is an entry record `{ key, value }`. That is why `key == "id-1"` is shorthand for checking the key field of each entry.

### Sum-type focus (prisms)

Patches can also focus on one constructor inside a sum type.

<<< ../snippets/from_md/syntax/patching/sum_type_focus_prisms.aivi{aivi}

If the current value is not the selected constructor, the patch leaves it unchanged.

## Patch instructions

Once a path selects a location, the instruction says what to do there.

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert a value |
| `Function` | Transform the existing value |
| `:= Function` | Store the function itself as data |
| `-` | Remove a field when the resulting record type remains valid |

## Replace or insert

Use a plain value when you want to set the selected location directly.

<<< ../snippets/from_md/syntax/patching/replace_insert.aivi{aivi}

Intermediate records are created automatically when the path requires them.

## Transform an existing value

Use a function instruction when the new value depends on the old one.

<<< ../snippets/from_md/syntax/patching/transform.aivi{aivi}

> Comment: this is the “update based on the current value” form, such as incrementing a counter or appending to a list.

## Remove a field

Use `-` to remove a field when the target type allows that removal.

<<< ../snippets/from_md/syntax/patching/removal.aivi{aivi}

Removal is structural, so the resulting type reflects the missing field.

## Common patching patterns

### Deep collection updates

Use traversals and predicates together when you need to update records inside lists, maps, or similar nested collections.

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_01.aivi{aivi}

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_02.aivi{aivi}

### Complex sum-type patching

Use constructor focus when only one branch of a sum type should change.

<<< ../snippets/from_md/syntax/patching/complex_sum_type_patching.aivi{aivi}

### Record bulk update

Use one patch block to express several related updates together.

<<< ../snippets/from_md/syntax/patching/record_bulk_update.aivi{aivi}

## Practical rules of thumb

- Use patching when immutable updates start to feel repetitive
- Prefer paths that read like the data shape a reader already knows
- Use a function instruction when the new value depends on the old one
- Use reusable `Patch A` values for repeated business rules such as “mark as archived” or “apply discount”
