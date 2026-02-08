# Record Patching (`<=`)

The `<=` operator applies a **declarative structural patch**.

```aivi
updated = record <= { path: instruction }
```

Patching is:

* immutable
* compositional
* type-checked

---

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

---

## 5.2 Instructions

| Instruction | Meaning |
| :--- | :--- |
| `value` | Replace or insert |
| `Function` | Transform existing value |
| `:= Function` | Replace with function **as data** |
| `-` | Remove field (shrinks record type) |

---

## 5.3 Replace / insert

```aivi
user2 = user <= {
  name: `Grace`
  profile.avatar.url: `https://img`
}
```

Intermediate records are created if missing.

---

## 5.4 Transform

```aivi
user3 = user <= {
  name: toUpper
  stats.loginCount: _ + 1
}
```

---

## 5.5 Removal

```aivi
user4 = user <= {
  email: -
  preferences.notifications.email: -
}
```

Removal is structural and reflected in the resulting type.

---

## 5.6 Automatic lifting

If a patch path targets a value inside `Option` or `Result`, the instruction is **automatically lifted**.

```aivi
user <= { email: toUpper }
```

Works for:

* `String`
* `Option String`
* `Result E String`
