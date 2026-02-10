# Domains

Domains are AIVI's mechanism for context-aware semantics. They allow the language to adapt to specific problem spaces—like time, geometry, or UI styling—by providing custom interpretations for operators, literals, and type interactions.

Instead of baking specific logic (like "days often have 24 hours but not always") into the core compiler, AIVI delegates this to **Domains**.

## Using Domains

To use a domain, you `use` it. This brings its operators and literals into scope.

```aivi
// Bring Vector math into scope
use aivi.std.math.vector

position = (10, 20)
velocity = (1, 0)

// The '+' operator now knows how to add tuples as vectors
new_pos = position + velocity
```

## Units and Deltas

Domains often introduce **Units** (measurements) and **Deltas** (changes).

### Delta Literals (Suffixes)

Deltas represent a relative change or a typed quantity. They are written as numeric literals with a suffix.

```aivi
10m      // 10 minutes (Duration) or 10 meters (Length)
30s      // 30 seconds
90deg    // 90 degrees
100px    // 100 pixels
```

These are **not** strings; they are typed values. `10m` might compile to a `Duration` struct or a float tagged as `Meters`, depending on the active domain.

```aivi
deadline = now + 10m
```



## Defining Domains

You can define your own domains to encapsulate logic. A domain relates a **Carrier Type** (the data) with **Delta Types** (changes) and **Operators**.

### Syntax

```aivi
domain Name over CarrierType = {
  // 1. Define the "change" type
  type Delta = ...

  // 2. Implement operators
  (+) : CarrierType -> Delta -> CarrierType
  (+) carrier delta = ...

  // 3. Define literals
  1d = Day 1
  ~my_sigil(...) = ...
}
```

### Example: A Simple Color Domain

```aivi
// The data
Rgb = { r: Int, g: Int, b: Int }

// The definition
domain Color over Rgb = {
  // Deltas define how values can change
  type Delta = Lightness Int | Hue Int

  // Operator: Color + Change -> Color
  (+) : Rgb -> Delta -> Rgb
  (+) color (Lightness amount) = adjust_lightness color amount
  (+) color (Hue amount)       = adjust_hue color amount

  // Define suffix literals 
  // "10l" desugars to "Lightness 10"
  1l = Lightness 1
  1h = Hue 1
}
```

### Interpretation

When you write:

```aivi
red = { r: 255, g: 0, b: 0 }
lighter = red + 10l
```

The compiler sees `red` is type `Rgb`. It looks for a domain over `Rgb` (the `Color` domain). It then desugars `10l` using the domain's rules into `Lightness 10`, and maps `+` to the domain's `(+)` function.

## Multi-Carrier Domains

Some domains cover multiple types (e.g., `Vector` over `Vec2` and `Vec3`). In v0.1, this is handled by defining the domain multiple times, once for each carrier.

```aivi
domain Vector over Vec2 = { ... }
domain Vector over Vec3 = { ... }
```
