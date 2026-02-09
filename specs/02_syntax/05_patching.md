# Patching Records

The `<|` operator applies a **declarative structural patch**. This avoids overloading `<=`, which is expected to be a normal comparison operator.

The compiler enforces that the patch shape matches the target record's type, ensuring that only existing fields are updated or new fields are added according to the record's openness.

```aivi
updated = record <| { path: instruction }
```

Patching is:

* immutable
* compositional
* type-checked

Compiler checks:

* Patch paths must resolve against the target type (unknown fields/constructors are errors).
* Predicate selectors (`items[price > 80]`) must type-check as `Bool`.
* Removing fields (`-`) is only allowed when the resulting record type remains valid (e.g. not removing required fields of a closed record).


## 5.1 Path addressing

### Dot paths

```aivi
user.profile.avatar.url
```

### Traversals

```aivi
items[*]
```

### Predicates

```aivi
items[price > 80]
items[id == 1]
```

### Sum-type focus (prisms)

```aivi
Ok.value
Some.val
Circle.radius
```

If the constructor does not match, the value is unchanged.


## 5.2 Instructions

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert |
| `Function` | Transform existing value |
| `:= Function` | Replace with function **as data** |
| `-` | Remove field (shrinks record type) |


## 5.3 Replace / insert

```aivi
user2 = user <| {
  name: "Grace"
  profile.avatar.url: "https://img"
}
```

Intermediate records are created if missing.


## 5.4 Transform

```aivi
user3 = user <| {
  name: toUpper
  stats.loginCount: _ + 1
}
```


## 5.5 Removal

```aivi
user4 = user <| {
  email: -
  preferences.notifications.email: -
}
```

Removal is structural and reflected in the resulting type.


## 5.7 Expressive Data Manipulation

Patching allows for very concise updates to deeply nested data structures and collections.

### Deep Collection Updates
```aivi
// Update prices of all active items in a category
store2 = store <| {
  categories[name == "Hardware"].items[active].price: _ * 1
}
```

### Complex Sum-Type Patching
```aivi
// Move all shapes to the origin
scene2 = scene <| {
  shapes[*].Circle.center: origin
  shapes[*].Square.origin: origin
}
```

### Record Bulk Update
```aivi
// Set multiple fields based on previous state
user2 = user <| {
  name: toUpper
  status: if admin then SuperUser else Normal
  stats.lastVisit: now
}
```
