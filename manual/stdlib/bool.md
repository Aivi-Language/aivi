# aivi.bool

Boolean utilities for AIVI. These functions complement the built-in `and`, `or`, and bool branching operators with named combinators for common logical patterns.

```aivi
use aivi.bool (
    not
    xor
    implies
    both
    either
    neither
    fromInt
)
```

## At a glance

| Function | Type | Use it for |
| --- | --- | --- |
| `not` | `Bool -> Bool` | Negate a boolean |
| `xor` | `Bool -> Bool -> Bool` | Check whether exactly one side is true |
| `implies` | `Bool -> Bool -> Bool` | Express logical implication |
| `both` | `Bool -> Bool -> Bool` | Named `and` |
| `either` | `Bool -> Bool -> Bool` | Named `or` |
| `neither` | `Bool -> Bool -> Bool` | Check whether both sides are false |
| `fromInt` | `Int -> Bool` | Treat `0` as false and everything else as true |

---

## not

Negates a boolean value.

```aivi
```

```aivi
use aivi.bool (not)

type Bool -> Bool
func isInactive = active =>
    not active
```

---

## xor

Returns `True` if exactly one of the two arguments is `True` (exclusive or).

```aivi
```

```aivi
use aivi.bool (xor)

type Bool -> Bool -> Bool
func toggleChanged = previous current =>
    xor previous current
```

---

## implies

Logical implication: `implies a b` is `False` only when `a` is `True` and `b` is `False`.

```aivi
```

```aivi
use aivi.bool (implies)

type Bool -> Bool -> Bool
func checkRule = hasPermission canAccess =>
    implies hasPermission canAccess
```

---

## both

Returns `True` if both arguments are `True`. Equivalent to `a and b`.

```aivi
```

```aivi
use aivi.bool (both)

type Bool -> Bool -> Bool
func isAdminAndActive = isAdmin isActive =>
    both isAdmin isActive
```

---

## either

Returns `True` if at least one argument is `True`. Equivalent to `a or b`.

```aivi
```

```aivi
use aivi.bool (either)

type Bool -> Bool -> Bool
func canProceed = hasTokenA hasTokenB =>
    either hasTokenA hasTokenB
```

---

## neither

Returns `True` only if both arguments are `False`.

```aivi
```

```aivi
use aivi.bool (neither)

type Bool -> Bool -> Bool
func isSilent = isPlaying isPaused =>
    neither isPlaying isPaused
```

---

## fromInt

Converts an integer to a boolean: `0` becomes `False`, any other value becomes `True`.

```aivi
```

```aivi
use aivi.bool (fromInt)

type Int -> Bool
func hasFlags = flagBits =>
    fromInt flagBits
```
