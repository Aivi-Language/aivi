# Record Patterns & Projections

Record patterns destructure values by field name. This page covers the full range of record destructuring forms, including dotted paths and projection expressions.

## Basic record patterns

Match a record and bind its fields to names:

```aivi
type Profile = {
    name: Text,
    score: Int
}

type Profile -> Text
func greet = arg1 => arg1
 ||> { name } -> "Hello, {name}!"
```

You can bind multiple fields in one pattern:

```aivi
type Profile -> Text
func summary = arg1 => arg1
 ||> { name, score } -> "{name} scored {score}"
```

See [Pattern Matching](/guide/pattern-matching) for full pattern syntax including tuples, constructors, and wildcards.

## Dotted path destructuring

When a record contains nested records, dotted paths reach into the structure without writing nested patterns manually:

```aivi group=user-projection
type City = {
    name: Text,
    population: Int
}

type Address = {
    city: City,
    street: Text
}

type User = {
    name: Text,
    address: Address
}

value user : User = {
    name: "Ada",
    address: {
        city: {
            name: "London",
            population: 8982000
        },
        street: "St James's Square"
    }
}

type User -> Text
func cityName = arg1 => arg1
 ||> { address.city.name } -> name
```

`{ address.city.name }` is sugar for nested patterns:


The leaf segment (`name`) becomes the bound variable. This works at any depth:

```aivi group=user-projection
type User -> Text
func streetName = arg1 => arg1
 ||> { address.street } -> street
```

You can combine dotted paths with ordinary fields:

```aivi group=user-projection
type User -> Text
func userCity = arg1 => arg1
 ||> { name, address.city.name: cityName } -> "{name} lives in {cityName}"
```

Here `address.city.name: cityName` renames the bound variable to `cityName` instead of the default leaf name.

## Record projection expressions

The `{ field: . }` form extracts a field and makes it the subject for further piping. The dot (`.`) means "this becomes the ambient subject":

```aivi
type Profile = {
    name: Text,
    score: Int
}

type Profile -> Bool
func isTopScore = arg1 =>
    arg1.score >= 100
```

This is sugar for:

```aivi
type Profile -> Bool
func isTopScore = profile => profile
 ||> { score } -> score >= 100
```

The key insight: `{ field: . }` is not record construction — it is a **projection** that extracts the named field from the input.

## Dotted projection

Dotted paths combine with the projection form to reach into nested structures:

```aivi group=user-projection
type User -> Text
func getCityName = arg1 =>
    arg1.address.city.name
```

This extracts `address.city.name` from the input and makes it available for downstream pipes:

```aivi group=user-projection
type User -> Text
func upperCityName = arg1 =>
    toUpper (getCityName arg1)
```

The same dotted-path idea is also available in selected-subject function headers:

```aivi
type Z = { z: Int }

type Y = { y: Z }

type X = { x: Y }

type Int -> Int
func addOne = value =>
    value + 1

type X -> Int
func readNested = state => state.x.y.z
  |> addOne
```

Here `{ x.y.z! }` means "select `state.x.y.z` as the subject for the continuation." It is
projection sugar in the header, not a new general parameter-pattern form.

## Projection in pipes

Projection expressions work naturally as pipe stages:

```aivi group=user-projection
value uppercasedCity = user
  |> { address.city.name: . }
  |> toUpper
```

This is equivalent to the `.field` ambient projection form, but for deeper paths:

```aivi group=user-projection
value uppercasedCityStepwise = user
  |> .address
  |> .city
  |> .name
  |> toUpper
```

## Patch removal

The `: -` syntax in a patch removes a field from a record. The result type is the input type minus the removed field:

```aivi
type Full = {
    name: Text,
    email: Text,
    debug: Bool
}

type Full -> { name: Text, email: Text }
func stripDebug = arg1 =>
    arg1 <| { debug: - }
```

Removal can target nested fields using selectors:


See [Values & Functions § Structural patches](/guide/values-and-functions#structural-patches) for more on the `<|` operator and patch selectors.
