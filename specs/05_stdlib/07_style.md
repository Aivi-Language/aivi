# Standard Library: Style Domain

## Module

```aivi
module aivi/std/style = {
  export domain Style
  export CssValue, Px, Em, Rem, Pct, Vh, Vw
  export StyleSheet
}
```

## Types

```aivi
CssValue = Px Int | Em Float | Rem Float | Pct Float | Vh Float | Vw Float | Auto

StyleSheet = List (Text, CssValue)  -- property -> value
```

## Domain Definition

```aivi
domain Style over StyleSheet = {
  -- Merge style sheets
  (+) : StyleSheet -> StyleSheet -> StyleSheet
  (+) base override = mergeStyles base override
  
  -- Add single property
  (+) : StyleSheet -> (Text, CssValue) -> StyleSheet
  (+) sheet prop = sheet + [prop]
}
```

## Delta Literals (CSS Units)

```aivi
-- Pixels
10px = Px 10

-- Relative units
1em = Em 1.0
1rem = Rem 1.0

-- Percentage
50pct = Pct 50.0

-- Viewport units
100vh = Vh 100.0
100vw = Vw 100.0
```

## Usage Examples

```aivi
use aivi/std/style

baseCard = [
  (`padding`, 16px)
  (`margin`, 8px)
  (`borderRadius`, 4px)
]

primaryCard = baseCard + [
  (`backgroundColor`, `#007bff`)
  (`color`, `white`)
]

fullWidth = [
  (`width`, 100pct)
  (`maxWidth`, 1200px)
]

container = baseCard + fullWidth
```

## Type Safety

```aivi
-- Type error: cannot add incompatible units
bad = 10px + 5  -- Error: Int is not CssValue

-- Type error: wrong context
bad2 = someDate + 10px  -- Error: Date is not StyleSheet carrier
```
