# Patching Records

The `<|` operator applies a **declarative structural patch**. This avoids overloading `<=`, which is expected to be a normal comparison operator.

The compiler enforces that the patch shape matches the target record's type, ensuring that only existing fields are updated or new fields are added according to the record's openness. When a patch path selects a `Map` entry, the patch applies to the **value** stored at that key.

<<< ../snippets/from_md/02_syntax/05_patching/block_01.aivi{aivi}

Patching is:

* immutable
* compositional
* type-checked

`Patch A` is a first-class type alias for `A -> A` and is the canonical type for patch values.
Applying a patch is done with `<|`.

Patch literals can be lifted into patch functions:

<<< ../snippets/from_md/02_syntax/05_patching/block_02.aivi{aivi}

`patch { ... }` produces a patch value that can be applied later with `<|`.

Compiler checks:

* Patch paths must resolve against the target type (unknown fields/constructors are errors).
* Predicate selectors (`items[price > 80]`) must type-check as `Bool`.
* Map key selectors (`map["k"]` or `map[key == "k"]`) must use the map's key type.
* Removing fields (`-`) is only allowed when the resulting record type remains valid (e.g. not removing required fields of a closed record).


## 5.1 Path addressing

### Dot paths

<<< ../snippets/from_md/02_syntax/05_patching/block_03.aivi{aivi}

### Traversals

<<< ../snippets/from_md/02_syntax/05_patching/block_04.aivi{aivi}

### Predicates

<<< ../snippets/from_md/02_syntax/05_patching/block_05.aivi{aivi}

### Map key selectors

When the focused value is a `Map`, selectors address entries by key. After selection, the focus is the **value** at that key.

<<< ../snippets/from_md/02_syntax/05_patching/block_06.aivi{aivi}

In map predicates, the current element is an entry record `{ key, value }`, so `key == "id-1"` is shorthand for `_.key == "id-1"`.

### Sum-type focus (prisms)

<<< ../snippets/from_md/02_syntax/05_patching/block_07.aivi{aivi}

If the constructor does not match, the value is unchanged.


## 5.2 Instructions

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert |
| `Function` | Transform existing value |
| `:= Function` | Replace with function **as data** |
| `-` | Remove field (shrinks record type) |


## 5.3 Replace / insert

<<< ../snippets/from_md/02_syntax/05_patching/block_08.aivi{aivi}

Intermediate records are created if missing.


## 5.4 Transform

<<< ../snippets/from_md/02_syntax/05_patching/block_09.aivi{aivi}


## 5.5 Removal

<<< ../snippets/from_md/02_syntax/05_patching/block_10.aivi{aivi}

Removal is structural and reflected in the resulting type.


## 5.7 Expressive Data Manipulation

Patching allows for very concise updates to deeply nested data structures and collections.

### Deep Collection Updates
<<< ../snippets/from_md/02_syntax/05_patching/block_11.aivi{aivi}

<<< ../snippets/from_md/02_syntax/05_patching/block_12.aivi{aivi}

### Complex Sum-Type Patching
<<< ../snippets/from_md/02_syntax/05_patching/block_13.aivi{aivi}

### Record Bulk Update
<<< ../snippets/from_md/02_syntax/05_patching/block_14.aivi{aivi}
