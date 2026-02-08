# Collections (Lists, Tuples, Records)

AIVI’s core data structures are **immutable** and work naturally with pattern matching, predicate lifting, and patching.

---

## 16.1 Tuples

Tuples are positional products:

```aivi
point = (10, 20)
(x, y) = point
```

Tuples are commonly used for multi-value returns and for matching on multiple inputs at once.

---

## 16.2 Lists

Lists are the default sequence type in the core language.

### Literals and ranges

```aivi
xs = [1, 2, 3]
ys = [0..10]
```

### Destructuring

```aivi
head =
  | [] => None
  | [x, ...] => Some x

split =
  | [] => (None, [])
  | [h, ...t] => (Some h, t)
```

### Working with lists

In general, prefer pipelines with higher-order functions:

```aivi
users
  |> filter active
  |> map .name
  |> sort
```

Predicates can be written point-free via predicate lifting:

```aivi
adults = users |> filter (age >= 18)
```

---

## 16.3 Records

Records are structural and open by default.

### Literals and projection

```aivi
user = { id: 1, name: "Alice", active: True }
name = user.name
```

`.field` is an accessor function:

```aivi
getName = .name // x => x.name
names = users |> map getName
```

### Destructuring and dotted keys

```aivi
{ name, active } = user
{ data.user.profile@{ name } } = response
```

Record patterns also support dotted keys:

```aivi
process =
  | Ok { data.users: [first, ...] } => first
  | _ => None
```

### Updating with patches

Patching is the preferred way to update nested structures immutably:

```aivi
user2 = user <= { name: toUpper }
store2 = store <= { categories[name == "Hardware"].items[active].price: _ * 1.1 }
```

---

## 16.4 Arrays, Maps, Sets

This spec currently focuses on the syntax for tuples, lists, and records.

Array-like and key-value data structures are expected to be provided by the standard library (and/or domains) with immutable APIs and predicate-friendly combinators.
