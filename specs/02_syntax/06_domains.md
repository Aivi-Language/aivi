# Domains, Units, and Deltas

Domains define **semantics**, not just values. They provide a context in which operators (like `+`, `-`, `*`) and literals (like `1m`, `1d`) are interpreted. This allows AIVI to handle complex, domain-specific logic like calendar arithmetic, color blending, or physical units with type safety and without polluting the core language with special cases.

```aivi
domain Calendar
domain Color
domain Vector
```

A domain typically defines:
1.  **Carrier Types**: The underlying data types (e.g., a `ZonedDateTime` record for `Calendar`).
2.  **Delta Types**: Representing changes or intervals (e.g., `Duration`).
3.  **Interpretation Rules**: How literals and operators map to functions.


## 6.1 Delta literals

Deltas represent **change**, not quantities.

```aivi
1d
1m
1y
3l
90deg
```

Properties:

* deltas are not numbers
* deltas have no arithmetic
* deltas are interpreted by domains


## 6.2 Domain-directed operators

Operators have **no intrinsic meaning**.

```aivi
date + 1m
color + 3l
vector1 + vector2
```

Valid only if the domain defines the operation.
## 6.3 Expressive Domain Logic

Domains allow for "semantic arithmetic" where the types ensure that only operations that make sense in that domain are permitted.

### Semantic Time Calculation
```aivi
// Domains handle complex calendars (Leap years, DST) automatically
deadline = now + 2w + 3d
isLate = current_time > deadline

// Interval calculations
remaining = deadline - now // Returns a Duration
```

### Visual and Spatial Logic
```aivi
// Color blending and manipulation
highlight = baseColor + 20l - 10s // Lighter, less saturated
transparent = activeColor <| { alpha: 0.5 }

// Vector arithmetic
velocity = (10, 5) // Inferred as Vector
position2 = position1 + (velocity * 2.0s)
```

### Typed Custom Domains
```aivi
// Financial domain prevents adding different currencies
total = usd 100 + usd 50 // OK
err = usd 100 + eur 50   // Compile-time Error
```

If you want more literal-style surface syntax, the same idea could be expressed with currency suffixes (as sugar):

```aivi
total = 100$ + 50$ // OK
err = 100$ + 50€   // Compile-time Error
```

### Built-in operator domains

Some domains are effectively built in for practicality (but can still be specified in the same “operators come from domains” model):

* `Int` / `Float` / `Decimal`: numeric operators like `+`, `-`, `*`, `/`
* `Int` (and/or a dedicated `Bits` carrier): bitwise operators like `&`, `|`, `^`, `~`, `<<`, `>>`
* `Bool`: boolean operators `!`, `&&`, `||` (typically defined with short-circuit semantics)

#### Predicates

A predicate is just a function:

```aivi
Pred A = A => Bool
```

Predicate composition is ordinary boolean logic inside a predicate position:

```aivi
isGoodUser : Pred User
isGoodUser = active && tier == Premium && isValidEmail email

goodUsers = users |> filter isGoodUser
```

#### Bits

Bitwise operators can be viewed as coming from a `Bits` domain (often implemented on `Int`):

```aivi
// Test a bit
isSet = flags n => (flags & (1 << n)) != 0

// Combine masks
mask = readMask1 | readMask2
```


### Standard library domains

The standard library exports domains as modules (see Modules). Typical domains include:

* `Calendar` (date arithmetic with calendar-aware deltas)
* `Duration` (fixed time deltas)
* `Color`, `Vector`
* `Html`, `Style`
* `SQLite`

### Behind the Scenes: Interpretation
Every operation like `date + 1m` is desugared into a domain-specific function call. The compiler uses the type of `date` to look up the `Calendar` domain's `(+)` implementation for that carrier.

1.  **Delta Interpretation**: `1m` is interpreted by the `Calendar` domain as "one month".
2.  **Operator Mapping**: `+` is mapped to `Calendar.add`.
3.  **Result**: `Calendar.add date (Calendar.delta "1m")`.
