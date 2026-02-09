# Modules

## 10.1 Module Definitions

Modules are the primary unit of code organization, encapsulation, and reuse in AIVI. They define a closed scope and explicitly export symbols for public use.

```aivi
module my.utility.math = {
  export add, subtract
  export pi
  
  pi = 3.14159
  add = a b => a + b
  subtract = a b => a - b
  
  // Internal helper, not exported
  abs = n => if n < 0 then -n else n
}
```


## 10.2 Module Pathing (Dot Separator)

Modules are identified by hierarchical paths using common **dot notation**. This separates logical namespaces. By convention:
- `aivi.std.*` — Standard library
- `vendor.name.*` — Foreign libraries
- `user.app.*` — Application-specific logic

Module resolution is static and determined at compile time based on the project manifest.


## 10.3 Importing and Scope

Use the `use` keyword to bring symbols from another module into the current scope.

### Basic Import
```aivi
use aivi.std.core
```

### Selective / Selective Hiding
```aivi
use aivi.std.calendar (Date, isLeapYear)
use aivi.std.list hiding (map, filter)
```

### Renaming / Aliasing
```aivi
use aivi.std.calendar as Cal
use vendor.legacy.math (v1_add as add)
```

Compiler checks:

- Importing a missing module or symbol is a compile-time error.
- Unused imports produce a warning (suppressed if importing solely for a domain side-effect in v0.1).


## 10.4 Domain Exports

Modules are the primary vehicle for delivering **Domains**. Exporting a domain automatically exports its carrier type, delta types, and operators.

```aivi
module geo.vector = {
  export domain Vector
  export Vec2
  
  Vec2 = { x: Float, y: Float }
  
  domain Vector over Vec2 = {
    (+) : Vec2 -> Vec2 -> Vec2
    (+) a b = { x: a.x + b.x, y: a.y + b.y }
  }
}
```

When another module calls `use geo.vector`, it gains the ability to use `+` on `Vec2` records.


## 10.5 First-Class Modules

Modules are statically resolved but behave like first-class records within the compiler's intermediate representation. This enables powerful composition patterns.

### Nested Modules
```aivi
module aivi.std = {
  module core = { ... }
  module calendar = { ... }
}
```

### Module Re-exports
A module can aggregate other modules, acting as a facade.

```aivi
module aivi.prelude = {
  export domain Calendar, Color
  export List, Result, Ok, Err
  
  use aivi.std.calendar (domain Calendar)
  use aivi.std.color (domain Color)
  use aivi.std.core (List, Result, Ok, Err)
}
```


## 10.6 The Prelude

Every AIVI module implicitly starts with `use aivi.prelude`. This provides access to the core language types and the most common domains without boilerplate.

To opt-out of this behavior (mandatory for the core stdlib itself):

```aivi
@no_prelude
module aivi.std.bootstrap = {
  // Pure bootstrap logic
}
```


## 10.7 Circular Dependencies

Circular module dependencies are **strictly prohibited** at the import level. The compiler enforces a Directed Acyclic Graph (DAG) for module resolution. For mutually recursive types or functions, they must reside within the same module or be decoupled via higher-order abstractions.
## 10.8 Expressive Module Orchestration

Modules allow for building clean, layered architectures where complex internal implementations are hidden behind simple, expressive facades.

### Clean App Facade
```aivi
// Aggregate multiple sub-modules into a single clean API
module my.app.api = {
  export login, fetchDashboard, updateProfile
  
  use my.app.auth (login)
  use my.app.data (fetchDashboard)
  use my.app.user (updateProfile)
}
```

### Domain Extension Pattern
```aivi
// Enhance an existing domain with local helpers
module my.geo.utils = {
  export domain Vector
  export distanceToOrigin, isZero
  
  use geo.vector (domain Vector, Vec2)
  
  distanceToOrigin = v => sqrt (v.x * v.x + v.y * v.y)
  isZero = v => v.x == 0 && v.y == 0
}
```

### Context-Specific Environments
```aivi
// Swapping implementations for test vs prod
module my.app.test_env = {
  use my.app.api hiding (fetchDashboard)
  fetchDashboard = _ => mockDashboardData
  
  export *
}
```
