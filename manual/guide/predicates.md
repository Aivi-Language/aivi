# Predicates & Selectors

Predicates are inline filter expressions used inside patch selectors and collection traversals. They select which elements to update without writing explicit loops or branches.

## Predicate syntax

A predicate appears inside square brackets and uses dot-prefixed field access, for example
`users[.active].role` selects the `role` field of each active user.

The dot (`.`) refers to the current element being tested. The expression must evaluate to `Bool`.

## Predicates in patches

::: warning Current execution limit
The patch functions below pass `aivi check`, but `aivi test` currently rejects their bodies
during typed-core lowering. These examples show checked syntax and the intended updates;
they are not yet executable through that command.
:::

Combine predicates with the `<|` patch operator to update only matching elements:

```aivi
type User = {
    name: Text,
    role: Text,
    active: Bool
}

type { users: List User } -> { users: List User }
func promoteActive = . <| { users[.active].role: "admin" }

value promoted = promoteActive {
    users: [
        { name: "Ada", role: "guest", active: True },
        { name: "Grace", role: "guest", active: False }
    ]
}
```

This updates the `role` field only for users where `.active` is `True`. Non-matching users are left unchanged.

## Predicate expressions

Predicates support the same comparison operators as regular expressions:

| Predicate | Meaning |
| --- | --- |
| `[.active == True]` | Field equals a value |
| `[.score >= 100]` | Numeric comparison |
| `[.name == "Ada"]` | Text equality |
| `[.role == "guest"]` | Match a specific field value |

The dot prefix accesses fields on each element:

```aivi
type Item = {
    name: Text,
    price: Int,
    inStock: Bool
}

type { items: List Item } -> { items: List Item }
func discountExpensive = . <| { items[.price >= 100].price: . - 10 }

value discounted = discountExpensive {
    items: [
        { name: "Desk", price: 120, inStock: True },
        { name: "Lamp", price: 40, inStock: True }
    ]
}
```

The desk's price becomes `110`; the lamp stays at `40`. Inside the predicate, `.price`
reads the item. In the replacement expression, `.` is the selected price.

## Selectors

Selectors are the path expressions inside patch braces that determine what to update. They chain left to right:

| Selector | Meaning |
| --- | --- |
| `field` | Select a record field |
| `.field` | Same as above (dot-prefixed form) |
| `a.b.c` | Nested field path |
| `[*]` | Traverse all `List` elements or `Map` values |
| `[predicate]` | Filter elements by predicate |
| `["key"]` | Select a `Map` entry by key |
| `[.key == "id"]` | Select `Map` entries matching a predicate |
| `Constructor` | Focus through a constructor with one payload |

For example, `users[.active].role` combines a record field, a list filter, and another
record field; `items[*].price` selects every item's price.

## Optional fields

Patch selectors do not implicitly unwrap constructors. Match an optional field explicitly, then
patch the enclosing record:

```aivi
type Int -> Int
func increment = value =>
    value + 1

type Config = {
    retries: Option Int,
    name: Text
}

type Config -> Config
func bumpRetries = config => config.retries
 ||> Some retries -> config <| { retries: Some (increment retries) }
 ||> None         -> config
```

The `None` arm leaves the record unchanged, and the `Some` arm makes the unwrap and rebuild visible.

## Store syntax

Use `:=` to store a function value as data instead of applying it:

```aivi
type Int -> Int
func increment = value =>
    value + 1

type Counter = {
    step: Int -> Int
}

value setStep : (Counter -> Counter) =
    patch {
        step: := increment,
    }
```

Without `:=`, the function would be called during patch application. With `:=`, the function itself becomes the new field value.

## Removal syntax

Use `: -` to remove a field from a record:

```aivi
value record = {
    tempField: 42,
    keep: True
}

value cleaned = record <| { tempField: - }
```

The result type reflects the removal — it has one fewer field than the input. See [Record Patterns § Patch removal](/guide/record-patterns#patch-removal) for details.
