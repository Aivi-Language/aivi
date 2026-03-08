# Patching (Structural Updates)

<!-- quick-info: {"kind":"operator","name":"<|"} -->
The `<|` operator applies a typed structural update to immutable data.

The compiler checks that the patch matches the target type, so only valid fields, map keys, constructors, and removals are allowed. When a patch path selects a `Map` entry, the patch applies to the **value** stored at that key.
<!-- /quick-info -->

Patching is AIVI’s built-in way to say “take this value and change these parts” without rebuilding every nested layer by hand.
It is the main immutable-update tool for records, nested collections, maps, and sum types.

If you are new to the syntax, `<|` is the patch operator described in [Operators and Context](operators.md), and predicate selectors use the same rules as [Predicates](predicates.md).

## What patching is for

Use patching when you want to:

- update deeply nested data in one expression
- transform part of a structure while leaving the rest unchanged
- reuse a structural update as a value
- keep updates type-checked even when they cross records, collections, maps, or sum types

A patch never mutates the original value. It returns a new value with the requested structural changes.

## Start small

Apply a patch immediately with `<|`:

<<< ../snippets/from_md/syntax/patching/patching_records_01.aivi{aivi}

::: repl
```aivi
user = { name: "Ada", age: 30, role: "dev" }
user <| { age: 31, role: "lead" }
// => { name: "Ada", age: 31, role: "lead" }
```
:::

Build a reusable patch value with `patch { ... }`:

<<< ../snippets/from_md/syntax/patching/patching_records_02.aivi{aivi}

`Patch A` is a first-class alias for `A -> A`, so you can store, pass, and compose patches like ordinary functions.

> Comment: use `<|` when you already have the value in hand; use `patch { ... }` when you want a reusable update function.

## What the compiler checks

The compiler validates the shape of a patch against the target type.

It reports an error when:

- a patch path refers to an unknown field or constructor
- a predicate selector does not type-check to `Bool`
- a map selector uses the wrong key type
- a removal would leave an invalid closed record type

That means patching stays convenient without becoming “stringly typed”.

## Choosing the part you want to change

The next few snippets show **selector fragments**: the left-hand side that appears before `:` inside a patch block.
For example, `{ items[*].price: _ * 1.1 }` combines the selector `items[*].price` with the instruction `_ * 1.1`.

### Dot paths

Use dot paths to reach nested record fields from the current focus.

<<< ../snippets/from_md/syntax/patching/dot_paths.aivi{aivi}

Read a path such as `profile.avatar.url` as “follow the record shape until you reach the field you want to update”.

### Traversals

Use traversals when you want to patch many values inside a collection-like structure.

<<< ../snippets/from_md/syntax/patching/traversals.aivi{aivi}

A traversal applies the instruction to every selected element, not just the first one.

### Predicates

Use a predicate when you want to patch only the elements that match a condition.

<<< ../snippets/from_md/syntax/patching/predicates.aivi{aivi}

A predicate selector must evaluate to `Bool`, and bare field names are read from the current element just as described in [Predicates](predicates.md).

### Map key selectors

When the current focus is a `Map`, bracket selectors choose entries before the patch continues deeper:

- `settings["theme"]` selects one entry by key
- `usersById[key == "id-1"]` filters entries with a predicate over `{ key, value }`
- `rolesById[*]` traverses all entry values

After selection, the patch continues on the **value** stored at that key, so selectors can keep chaining, for example `["id-1"].profile.name`.

<<< ../snippets/from_md/syntax/patching/map_key_selectors.aivi{aivi}

Inside a map predicate, the current element is an entry record `{ key, value }`. That is why `key == "id-1"` is shorthand for checking the key field of each entry.

### Constructor focus

Patches can also use constructor-based focuses (selecting a specific constructor branch of an ADT) inside a sum type.
For example, `Circle.radius` means “only continue if the current value is a `Circle`, then select its `radius` field”.

<<< ../snippets/from_md/syntax/patching/sum_type_focus_prisms.aivi{aivi}

If the current value is not the selected constructor, the patch leaves it unchanged.

## Patch instructions

Once a path selects a location, the instruction says what to do there.

| Instruction | What it does |
| :--- | :--- |
| `value` | Replace or insert |
| `function` | Transform the current value (applied to the old value) |
| `:= function` | Replace with the function as data (do not apply it) |
| `-` | Remove a field when the resulting record type remains valid |

### Replace or insert

Use a plain value when you want to set the selected location directly.

<<< ../snippets/from_md/syntax/patching/replace_insert.aivi{aivi}

Use this form when you already know the exact new value. Nested paths work the same way as shallow ones as long as the selected path is valid for the target type.

### Transform an existing value

Use a function instruction when the new value depends on the old one.

<<< ../snippets/from_md/syntax/patching/transform.aivi{aivi}

This is the “update based on the current value” form, such as incrementing a counter or appending to a list.

### Remove a field

Use `-` to remove a field when the target type allows that removal.

<<< ../snippets/from_md/syntax/patching/removal.aivi{aivi}

Removal is structural, so the resulting type reflects the missing field.

## Common patching patterns

### Record bulk update

Use one patch block to express several related changes together.

<<< ../snippets/from_md/syntax/patching/record_bulk_update.aivi{aivi}

### Deep collection updates

Use traversals and predicates together when you need to update records inside lists, maps, or similar nested collections.

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_01.aivi{aivi}

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_02.aivi{aivi}

### Complex sum-type patching

Use constructor focus when only one branch of a sum type should change.

<<< ../snippets/from_md/syntax/patching/complex_sum_type_patching.aivi{aivi}

## Practical rules of thumb

- Use patching when immutable updates start to feel repetitive.
- Prefer paths that read like the data shape a reader already knows.
- Use a function instruction when the new value depends on the old one.
- Use reusable `Patch A` values for repeated business rules such as “mark as archived” or “apply discount”.
