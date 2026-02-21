# Patching Records

<!-- quick-info: {"kind":"operator","name":"<|"} -->
The `<|` operator applies a **declarative structural patch**.

The compiler enforces that the patch shape matches the target record's type, ensuring that only existing fields are updated or new fields are added according to the record's openness. When a patch path selects a `Map` entry, the patch applies to the **value** stored at that key.
<!-- /quick-info -->

<<< ../snippets/from_md/syntax/patching/patching_records_01.aivi{aivi}

Patching is:

* immutable
* compositional
* type-checked

`Patch A` is a first-class type alias for `A -> A` and is the canonical type for patch values.
Applying a patch is done with `<|`.

Patch literals can be lifted into patch functions:

<<< ../snippets/from_md/syntax/patching/patching_records_02.aivi{aivi}

`patch { ... }` produces a patch value that can be applied later with `<|`.

Compiler checks:

* Patch paths must resolve against the target type (unknown fields/constructors are errors).
* Predicate selectors (`items[price > 80]`) must type-check as `Bool`.
* Map key selectors (`map["k"]` or `map[key == "k"]`) must use the map's key type.
* Removing fields (`-`) is only allowed when the resulting record type remains valid (e.g. not removing required fields of a closed record).


## 5.1 Path addressing

### Dot paths

<<< ../snippets/from_md/syntax/patching/dot_paths.aivi{aivi}

### Traversals

<<< ../snippets/from_md/syntax/patching/traversals.aivi{aivi}

### Predicates

<<< ../snippets/from_md/syntax/patching/predicates.aivi{aivi}

### Map key selectors

When the focused value is a `Map`, selectors address entries by key. After selection, the focus is the **value** at that key.

<<< ../snippets/from_md/syntax/patching/map_key_selectors.aivi{aivi}

In map predicates, the current element is an entry record `{ key, value }`, so `key == "id-1"` is shorthand for `_.key == "id-1"`.

### Sum-type focus (prisms)

<<< ../snippets/from_md/syntax/patching/sum_type_focus_prisms.aivi{aivi}

If the constructor does not match, the value is unchanged.


## 5.2 Instructions

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert |
| `Function` | Transform existing value |
| `:= Function` | Replace with function **as data** |
| `-` | Remove field (shrinks record type) |


## 5.3 Replace / insert

<<< ../snippets/from_md/syntax/patching/replace_insert.aivi{aivi}

Intermediate records are created if missing.


## 5.4 Transform

<<< ../snippets/from_md/syntax/patching/transform.aivi{aivi}


## 5.5 Removal

<<< ../snippets/from_md/syntax/patching/removal.aivi{aivi}

Removal is structural and reflected in the resulting type.


## 5.6 Expressive Data Manipulation

Patching allows for very concise updates to deeply nested data structures and collections.

### Deep Collection Updates

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_01.aivi{aivi}

<<< ../snippets/from_md/syntax/patching/deep_collection_updates_02.aivi{aivi}

### Complex Sum-Type Patching

<<< ../snippets/from_md/syntax/patching/complex_sum_type_patching.aivi{aivi}

### Record Bulk Update

<<< ../snippets/from_md/syntax/patching/record_bulk_update.aivi{aivi}
