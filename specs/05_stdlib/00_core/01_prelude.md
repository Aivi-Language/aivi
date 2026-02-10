# Standard Library: Prelude

The **Prelude** is your default toolkit. It acts as the "standard library of the standard library," automatically using the core types and domains you use in almost every program (like `Int`, `List`, `Text`, and `Result`). It ensures you don't have to write fifty `use` lines just to add two numbers or print "Hello World".

```aivi
module aivi.prelude = {
  // Core types
  export Int, Float, Bool, Text, Char, Bytes
  export List, Option, Result, Tuple
  
  // Standard domains
  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector
  
  // Re-exports
  use aivi
  use aivi.text
  use aivi.calendar
  use aivi.duration
  use aivi.color
  use aivi.vector
}
```

## Opting Out

```aivi
@no_prelude
module my.custom.module = {
  // Nothing used automatically
  use aivi (Int, Bool)
}
```

## Rationale

- Common domains (dates, colors, vectors) are used universally
- Delta literals should "just work" without explicit `use`
- Explicit opt-out preserves control for advanced use cases
