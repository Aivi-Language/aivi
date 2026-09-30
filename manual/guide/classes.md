# Classes

Classes are AIVI's typeclass-style abstraction mechanism. A class describes a set of operations that a
type must provide. For the canonical executable support reference for higher-kinded classes, current
builtin/runtime-backed carriers, the builtin-vs-authored execution boundary, and user-authored
instance limits, see [Typeclasses & Higher-Kinded Support](/guide/typeclasses#execution-boundary-builtin-carriers-vs-authored-instances).
For the semantic contract behind those instances, see [Class Laws & Design Boundaries](/guide/class-laws).

## Declaring a class

```aivi
class Equality A = {
    type equal : A -> A -> Bool
}
```

This says that any type used with `Equality` must provide `equal`.

You can declare ordinary named methods too:

```aivi
class Display A = {
    type display : A -> Text
}
```

## Polymorphic member contracts

An instance must implement its class signature for every quantified type. For example,
`Functor F` provides `map : (A -> B) -> F A -> F B`. Its implementation cannot replace
`B` with a fixed type such as `Text`, or assume that independent parameters `A` and `B`
are interchangeable. Generic functions follow the same rule: a function declared
`A -> B -> A` must return an `A` value.

Calls instantiate generic signatures using their arguments and expected result. A local
argument retains its type throughout the body; calling another generic function does
not change that local type. Container inputs constrain generic callbacks in expressions
such as `items |> map identity`.

A class member may have its own constraints. They apply to that member's quantified
parameters and are available while checking each instance implementation:

```aivi
class Inspect A = {
    type sameWith : Eq B => A -> B -> Bool
}

type Label = Label Text

instance Inspect Label = {
    sameWith = label item => item == item
}
```

Here `B` is the same parameter in `Eq B` and in the argument type, and remains independent
of the class parameter `A`. Each instance is checked against this contract.

## Block body syntax

When a class has multiple members, group them inside `= { ... }`:

```aivi
class Eq A = {
    type (==) : A -> A -> Bool
}

class Display A = {
    type display : A -> Text
    type label : A -> Text
}
```

Instance declarations use the same block form:

```aivi
class BlobEquality A = {
    type blobEqual : A -> A -> Bool
}

type Blob = Blob Bytes

type Blob -> Blob -> Bool
func blobEquals = left right =>
    True

instance BlobEquality Blob = {
    blobEqual = left right => blobEquals left right
}
```

## Superclass declarations

Use `with` inside the class body to declare that your class extends another class.
Any instance of the derived class must also provide an instance of each superclass.

```aivi
class Named A = {
    type name : A -> Text
}

class Displayed A = {
    type display : A -> Text
}

class Logged A = {
    with Named A
    with Displayed A
}
```

Multiple superclasses are listed as separate `with` lines.

```aivi
class Hashable A = {
    type hash : A -> Int
}

class CacheKey A = {
    with Eq A
    with Hashable A
}
```

## Parameter constraints

Use `require` inside the class body to constrain a type parameter. This documents that any type substituted for that parameter must satisfy the given class.

```aivi
class Container A = {
    require Eq A
}
```

## Using class-backed operators

When a type already has an instance, you can use the operator directly:

```aivi
type Int -> Int -> Bool
func equivalent = left right =>
    left == right and left != 0

value sameNumber = equivalent 4 4
```

Surface `!=` uses the same `Eq` evidence as `==`, so once equality exists both operators become
available at use sites.

`Ord` uses `compare : A -> A -> Ordering` as its primitive member. Surface ordering operators are derived from that member, so `<`, `>`, `<=`, and `>=` all work once an `Ord` instance exists.

## Declaring an instance

Instances provide the implementation for a concrete type:

```aivi
class BlobEquality A = {
    type blobEqual : A -> A -> Bool
}

type Blob = Blob Bytes

type Blob -> Blob -> Bool
func blobEquals = left right =>
    True

instance BlobEquality Blob = {
    blobEqual = left right => blobEquals left right
}
```

## Named class methods

A class can expose named operations instead of operators:

```aivi
class Display A = {
    type display : A -> Text
}

type Label = Label Text

instance Display Label = {
    display = label =>
        label
        ||> Label text -> text
}
```

## Eq constraints on functions

When a function needs to compare values of an open type parameter, use a constraint prefix on the annotation:

```aivi
type Eq K => K -> K -> Bool
func matchesKey = key candidate =>
    key == candidate
```

Multiple constraints use a parenthesized comma-separated list:

```aivi
type (Eq A, Eq B) => A -> A -> B -> B -> Bool
func bothEqual = leftA rightA leftB rightB =>
    leftA == rightA and leftB == rightB
```

The constraint ensures the function can only be called when `K` (or `A`, `B`, etc.) has an `Eq` instance. Without the constraint, using `==` on an open type parameter is a type error.

### Conditional instance resolution

An instance context is a prerequisite for selecting that instance. For example,
`instance Eq A => Render (Box A)` can supply `Render (Box Int)` only when `Eq Int`
is available. The checker infers `A` from the requested instance head and proves
every instantiated prerequisite. A prerequisite whose parameters cannot be
inferred from the head produces a diagnostic. Imported instances preserve the
same heads, quantifier indices, and prerequisites.

Resolution accepts finite proofs that move or grow type arguments. An exact
cycle cannot supply evidence by itself. The compiler limits a proof search to
256 active prerequisites and 4096 proof steps, and reports a complexity-limit
diagnostic when either limit is exceeded. These are compiler resource limits;
there is no rule requiring every prerequisite to have a smaller type expression.

The compiler passes executable prerequisite evidence to conditional authored
members before their visible arguments. A member-local context, such as
`Applicative G` on `Traversable.traverse`, adds its own evidence parameters.
These callables also work across imports and through generic constrained
functions. See the [authored evidence boundary](/guide/typeclasses#execution-boundary-builtin-carriers-vs-authored-instances)
for the builtin traversal and native compilation limits.

## Ord constraints and domain ordering

Use `Ord` when a function needs ordering rather than just equality:

```aivi
type Ord A => A -> A -> Bool
func nonDecreasing = left right =>
    left <= right
```

For nominal domains, implement `compare` in the `Ord` instance and then use the ordinary operators:

```aivi
domain Calendar over Int = {
    suffix day
    type day : Int
    day = value => Calendar value

    type toDays : Calendar -> Int
}

instance Ord Calendar = {
    compare = left right => compare (toDays left) (toDays right)
}

value ordered : Bool = 10day < 12day
value distinct : Bool = 10day != 12day
```

You normally explain equality once and let surface `!=` reuse that same evidence. You also do not need
to author separate class or domain members for `<`, `>`, `<=`, or `>=`; those surface operators are
sugar over `Ord.compare`.

## Why classes matter

Classes let generic code talk about capability instead of one hard-coded type. They are useful when you want a common interface for comparison, display, accumulation, or traversal.

## Summary

| Form | Meaning |
| --- | --- |
| `class Eq A` | Declare a class with a type parameter |
| `(==) : A -> A -> Bool` | Require an operator |
| `display : A -> Text` | Require a named method |
| `with Functor F` | Declare a superclass in the class body |
| `require Eq A` | Constrain a class type parameter |
| `instance Eq Blob` | Implement a class for one concrete type |
| `type Eq K => K -> K -> Bool` | Require `K` to have `Eq` in a function annotation |
| `class Name A = { ... }` | Group class members in a block |

---

**See also:** [Typeclasses & Higher-Kinded Support](typeclasses.md#execution-boundary-builtin-carriers-vs-authored-instances) — canonical executable support reference, builtin-vs-authored execution boundary, HKT hierarchy, and user-authored instance limits; [Class Laws & Design Boundaries](class-laws.md) — the semantic contract behind lawful instances
