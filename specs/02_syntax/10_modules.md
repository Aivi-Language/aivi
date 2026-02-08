# Modules and External Sources

## 10.1 External Sources

```aivi
Kind = File | Http | Db | Email | Llm | Image | ...
Source K A
```

## 10.2 Modules

```aivi
module aivi/app/main = {
  export main
  use aivi/std/core
}
```

Modules are first-class citizens but resolved statically.

---

## 10.3 Domain Exports

Modules can export domains alongside types and functions:

```aivi
module aivi/std/calendar = {
  export domain Calendar
  export Date, Day, Month, Year, EndOfMonth
  export isLeapYear, daysInMonth
  
  Date = { year: Int, month: Int, day: Int }
  
  domain Calendar over Date = {
    type Delta = Day Int | Month Int | Year Int | End EndOfMonth
    
    (+) : Date -> Delta -> Date
    (+) date (Day n)   = addDays date n
    (+) date (Month n) = addMonths date n
    ...
  }
}
```

Importing brings both the domain operators and delta literals into scope:

```aivi
use aivi/std/calendar

today = { year: 2025, month: 2, day: 8 }
nextMonth = today + 1m  -- Uses Calendar domain
```

---

## 10.4 The Prelude

The **prelude** is an implicit module containing common domains and types:

```aivi
module aivi/prelude = {
  export domain Calendar, Duration, Color, Vector
  export Int, Float, Text, Bool, List, Maybe, Result
  
  use aivi/std/calendar
  use aivi/std/duration
  use aivi/std/color
  use aivi/std/vector
  use aivi/std/core
}
```

All programs implicitly `use aivi/prelude` unless the `@no_prelude` pragma is specified:

```aivi
@no_prelude
module my/bare/module = {
  -- Must explicitly import everything
  use aivi/std/core
}
```

---

## 10.5 Standard Library Structure

```text
aivi/std/
├── core       -- Int, Float, Bool, Text, List, Maybe, Result
├── calendar   -- domain Calendar, Date types
├── duration   -- domain Duration, time spans
├── color      -- domain Color, Rgb, Hsl
├── vector     -- domain Vector, Vec2, Vec3, Vec4
├── html       -- domain Html, elements and attributes
├── style      -- domain Style, CSS units and properties
└── io         -- Source types, effects
```

---

## 10.6 Qualified Imports

To avoid name collisions, imports can be qualified:

```aivi
use aivi/std/calendar as Cal
use aivi/std/physics as Phys

-- Disambiguate delta literals
nextMonth = today + Cal.1m
distance = position + Phys.1m
```

Or selectively import:

```aivi
use aivi/std/calendar (Date, isLeapYear)
use aivi/std/calendar hiding (eom)
```
