# Domain Definitions

Domains can be fully defined in AIVI, making them **self-hosted**.

## 11.1 Domain Declaration Syntax

```aivi
domain Calendar over Date = {
  type Delta = Day Int | Month Int | Year Int | End EndOfMonth
  
  (+) : Date -> Delta -> Date
  (+) date (Day n)   = addDays date n
  (+) date (Month n) = addMonths date n
  (+) date (Year n)  = addYears date n
  (+) date End       = endOfMonth date
  
  (-) : Date -> Delta -> Date
  (-) date delta = date + (negate delta)
  
  -- Delta shorthand literals
  1d = Day 1
  1m = Month 1
  1y = Year 1
  eom = End
}
```

Components:
- `domain Name over CarrierType` — binds operators to a specific carrier type
- `type Delta = ...` — defines the domain's change vocabulary
- Operator implementations — regular AIVI functions
- Delta literals — syntactic sugar bound within the domain

---

## 11.2 Carrier Type Binding

A domain is **always** associated with a carrier type. Operators dispatch based on this binding:

```aivi
domain Color over Rgb = {
  type Delta = Lightness Int | Saturation Int | Hue Int
  
  (+) : Rgb -> Delta -> Rgb
  (+) col (Lightness n) = adjustLightness col n
  (+) col (Saturation n) = adjustSaturation col n
  (+) col (Hue n) = adjustHue col n
}
```

The carrier type determines when domain operators apply:

```aivi
myColor : Rgb
myColor = #ff5500

-- Resolved via Color domain because myColor : Rgb
result = myColor + 10l
```

---

## 11.3 Delta Literals

Delta literals are **domain-scoped**, not global. They desugar to Delta constructors:

| Literal | Desugars To | Domain |
| :--- | :--- | :--- |
| `1d` | `Day 1` | Calendar |
| `3m` | `Month 3` | Calendar |
| `10l` | `Lightness 10` | Color |
| `90deg` | `Degrees 90` | Angle |

When multiple domains define overlapping literals, resolution follows import order or requires qualification (see Open Questions).

---

## 11.4 Multi-Carrier Domains

Some domains span multiple carrier types:

```aivi
domain Vector over (Vec2 | Vec3 | Vec4) = {
  (+) : a -> a -> a where a in (Vec2, Vec3, Vec4)
  (+) v1 v2 = componentWiseAdd v1 v2
  
  (*) : a -> Scalar -> a where a in (Vec2, Vec3, Vec4)
  (*) v s = scale v s
}
```

---

## 11.5 Domain Functions

Domains may export helper functions alongside operators:

```aivi
domain Calendar over Date = {
  -- Operators
  (+) : Date -> Delta -> Date
  ...
  
  -- Exported functions
  isLeapYear : Date -> Bool
  isLeapYear date = ...
  
  daysInMonth : Date -> Int
  daysInMonth date = ...
}
