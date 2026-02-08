# Standard Library: Prelude

The prelude is implicitly imported by all AIVI programs.

```aivi
module aivi.prelude = {
  // Core types
  export Int, Float, Bool, Text, Char
  export List, Option, Result, Tuple
  
  // Standard domains
  export domain Calendar
  export domain Duration
  export domain Color
  export domain Vector
  
  // Re-exports
  use aivi.std.core
  use aivi.std.calendar
  use aivi.std.duration
  use aivi.std.color
  use aivi.std.vector
}
```

## Opting Out

```aivi
@no_prelude
module my.custom.module = {
  // Nothing imported automatically
  use aivi.std.core (Int, Bool)
}
```

## Rationale

- Common domains (dates, colors, vectors) are used universally
- Delta literals should "just work" without explicit imports
- Explicit opt-out preserves control for advanced use cases
