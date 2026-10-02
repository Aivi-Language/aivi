# Typeclasses & Higher-Kinded Support

If you have never used typeclasses, here is the core idea: sometimes you want to write code that works with **any type that supports a certain operation**. For example, you want to `map` over both lists and optional values, or compare any two values for equality. Typeclasses let you describe that capability once and use it everywhere.

If you have used interfaces in Java, traits in Rust, or protocols in Swift, typeclasses are a similar idea — but they also work at a higher level, letting you abstract over type constructors like `List`, `Option`, and `Signal`, not just concrete types.

This page documents the **executable** compiler/runtime slice that exists today, not just surface syntax.
Its builtin support section is the canonical doc source for higher-kinded executable class support: it is generated from the registry in `crates/aivi-core/src/class_support.rs`, and other docs should link here instead of copying carrier/class matrices.
For class declaration and instance syntax, see [Classes](/guide/classes).

## When to use what

| Abstraction | Use when... | Example |
| --- | --- | --- |
| A concrete type | You know exactly what the data is | `type Score = Int` |
| A domain | You want a branded wrapper with its own operators | `domain Score over Int` |
| A class | You want to write generic code over types sharing a capability | `class Eq A` |
| A higher-kinded class | You want to abstract over containers like `List`, `Option`, `Signal` | `class Functor F` |

## Current hierarchy

The ambient prelude includes a broader class graph, but the main higher-kinded slice currently centers on these relationships:

```text
Functor
├─ Apply
│  ├─ Applicative
│  └─ Chain
│     └─ Monad
├─ Filterable
└─ Traversable

Foldable
└─ Traversable

Bifunctor
```

`Monad` depends on both `Applicative` and `Chain`; `Chain` itself depends on `Apply`.

| Class | Direct superclasses | Primary member |
| --- | --- | --- |
| `Functor F` | — | `map : (A -> B) -> F A -> F B` |
| `Apply F` | `Functor F` | `apply : F (A -> B) -> F A -> F B` |
| `Applicative F` | `Apply F` | `pure : A -> F A` |
| `Chain M` | `Apply M` | `chain : (A -> M B) -> M A -> M B` |
| `Monad M` | `Applicative M`, `Chain M` | `join : M (M A) -> M A` |
| `Foldable F` | — | `reduce : (B -> A -> B) -> B -> F A -> B` |
| `Traversable T` | `Functor T`, `Foldable T` | `traverse : Applicative G => (A -> G B) -> T A -> G (T B)` |
| `Filterable F` | `Functor F` | `filterMap : (A -> Option B) -> F A -> F B` |
| `Bifunctor F` | — | `bimap : (A -> C) -> (B -> D) -> F A B -> F C D` |

## Advanced ambient classes (secondary today)

The ambient prelude also declares additional classes beyond the primary slice above:

- `Setoid` (`equals`)
- `Semigroupoid` (`compose`)
- `Contravariant` (`contramap`)
- `Category` (`id`)
- `Profunctor` (`dimap`)
- `Semigroup` / `Monoid` / `Group` (`append`, `empty`, `invert`)
- `Alt` / `Plus` / `Alternative` (`alt`, `zero`, `guard`)
- `Extend` / `Comonad` (`extend`, `extract`)
- `ChainRec` (`chainRec`)

These names are real surface declarations, but they are **not** part of the primary executable support
story on this page unless a later section says so explicitly.

That means:

- the builtin carrier table below does **not** claim runtime-backed support for them
- this guide does not present them as the default user-facing abstraction path today
- if a feature or module relies on one of them, document and validate that exact class/instance path instead of assuming broad runtime coverage

## Canonical builtin executable support

In this section, **executable support** means the current compiler lowers class-member use to first-class executable evidence in `aivi-core`.
Builtin carriers use builtin executable evidence intrinsics; authored instances use authored executable evidence that points at their lowered item bodies. If a carrier is not listed here for a builtin class, that class is **not** runtime-backed for that carrier today, even if parser, HIR, or checker support exists for related syntax.

<!-- BEGIN builtin-executable-support -->
This registry-backed table is the canonical documentation source for builtin executable higher-kinded support. Other docs should link here instead of restating carrier/class matrices.

| Builtin carrier | Functor | Apply | Applicative | Monad | Foldable | Traversable | Filterable | Bifunctor |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `List` | yes | yes | yes | yes | yes | yes | yes | — |
| `Option` | yes | yes | yes | yes | yes | yes | yes | — |
| `Result E` | yes | yes | yes | yes | yes | yes | — | yes |
| `Validation E` | yes | yes | yes | — | yes | yes | — | yes |
| `Signal` | yes | yes | yes | — | — | — | — | — |
| `Task E` | yes | yes | yes | yes | — | — | — | — |

- The `Monad` column means builtin executable lowering for `chain` and `join`; `Chain` uses the same registry entries.
- `—` means the canonical executable-support registry marks that builtin class/carrier pair unsupported.
- `Signal` is intentionally **not** a `Monad`: executable signals keep a static dependency graph.
- `Validation E` is intentionally **not** a `Monad`: independent accumulation uses the applicative `&|>` pipe, while dependent `!|>` checks are a dedicated pipe primitive rather than class-backed `bind`.
- `traverse` is builtin-supported for `List`, `Option`, `Result`, and `Validation`. Its result uses explicit `Applicative` evidence, including builtin `List`, `Option`, `Result`, `Validation`, `Signal`, and `Task`, or an authored applicative instance.
<!-- END builtin-executable-support -->

For the law contract behind this hierarchy and the rationale for why `Signal` and `Validation`
intentionally stop at `Applicative`, see [Class Laws & Design Boundaries](/guide/class-laws).

## Standard library instances

These instances are implemented in their owning modules. Import that module's type or helpers to
make its instances available; no carrier-specific spelling of the class operation is required.

| Carrier | Classes | Behavior |
| --- | --- | --- |
| `Either E` | `Functor`, `Apply`, `Applicative`, `Chain`, `Monad`, `Foldable`, `Traversable` | Operate on `Right`; preserve and short-circuit `Left`. `pure` creates `Right`; traversal lifts `Left` unchanged. |
| `Either` | `Bifunctor` | `bimap` transforms both alternatives independently. |
| `Dict K` | `Functor`, `Foldable`, `Traversable`, `Filterable` | Transform or visit values, preserving keys and entry order. |
| `Dict K V` | `Default` | An empty dictionary. |
| `Set` from `aivi.core.set` | `Foldable` | Visit members once, in insertion order. |
| `Set A` from `aivi.core.set` | `Default` | An empty set. |
| `NonEmptyList` | `Functor`, `Apply`, `Applicative`, `Chain`, `Monad`, `Foldable`, `Traversable` | Preserve non-emptiness; application uses function-major Cartesian order, chaining concatenates results in input order, and traversal preserves length. |
| `NonEmptyList A` | `Semigroup` | Concatenate in order. |
| `Matrix` | `Functor`, `Foldable`, `Traversable` | Preserve dimensions when mapping or traversing; visit cells in row-major order. |
| `Bytes` from `aivi.core.bytes` | `Semigroup`, `Monoid`, `Default` | Concatenation and empty bytes. |
| `Text`, `Int`, `Bool` from `aivi.defaults` | `Default` | Empty text, zero, and false, respectively. |

The class laws constrain this table. Sets do not get an unrestricted `Functor`: mapping can merge
members and would need output equality evidence. Non-empty lists do not get `Monoid`, `Default`,
or `Filterable`: those operations could produce no elements. Dictionaries do not choose an
implicit key-collision policy for `append` or `apply`. Matrix filtering would lose its rectangular
shape. `Signal` and `Validation` retain the applicative boundaries described above.

Use the same constrained function for builtin and stdlib carriers:

```aivi
use aivi.core.either (Either, Right)
use aivi.core.dict (Dict, fromList as dictFromList)
use aivi.nonEmpty (NonEmptyList, fromHeadTail)
use aivi.matrix (Matrix, MatrixError, fromRows)

type Int -> Option Int
func positiveIncrement = n => n > 0
 T|> Some (n + 1)
 F|> None

type Traversable F => F Int -> Option (F Int)
func advanceAll = values => traverse positiveIncrement values

value eitherInput : Either Text Int = Right 2
value eitherOutput : Option (Either Text Int) = advanceAll eitherInput
value dictionaryOutput : Option (Dict Text Int) = advanceAll (dictFromList [("a", 1), ("b", 2)])
value nonEmptyOutput : Option (NonEmptyList Int) = advanceAll (fromHeadTail 1 [2, 3])
value matrixOutput : Result MatrixError (Option (Matrix Int)) = map advanceAll (fromRows [[1, 2], [3, 4]])
```

`traverse` keeps the source shape. Into `Option`, any `None` fails the whole traversal; into
`Validation (NonEmptyList E)`, failures accumulate in element order; into `List`, each alternative retains that
shape. Into `Task`, effects stay deferred and run in the source's declared order at execution.

## Generic class-constrained functions

A class constraint can abstract over a constructor as well as an element type. The compiler passes
the required method evidence to the function, including through nested and imported calls.

```aivi
type Functor F => (A -> B) -> F A -> F B
func transform = f values!
  |> map f

type Int -> Int
func increment = . + 1

value optional : Option Int = transform increment (Some 2)

value numbers : List Int =
    transform increment [
        1,
        2,
        3
    ]
```

Keep constraints explicit. A `Functor` constraint does not imply `Monad`, and a `Foldable` constraint
does not imply that a collection can be empty. Class resolution must select one instance; it does
not choose between overlapping instance implementations by import order.

### Contextual callback inference

Arguments and the expected result constrain callbacks, including callbacks passed before the
collection argument. A callback's known inputs can determine its result constructor without
requiring a separate annotated helper:

```aivi
type (Traversable F, Applicative G) => (Int -> G Int) -> F Int -> G (F Int)
func visit = transform values => traverse transform values

value rejected : Bool = visit (n => None) [1, 2] == None
value noChoices : Bool = visit (n => []) [1, 2] == []
```

Here the signature fixes the transformed element type to `Int`; `None` determines `Option` and
`[]` determines `List`. Empty constructors do not choose an otherwise unknown element type.
For a direct `traverse (n => None)` call, supply a result or callback annotation when no other
argument fixes that result element type. Generic constructor callbacks instantiate at each use,
and captured values keep their lexical types; an expected callback type cannot specialize a
caller's generic parameter to a concrete type.

## Instance coherence and ownership

The compiler checks instance declarations even when no expression uses them. Two heads conflict
when their type variables can be unified after expanding transparent aliases. Record field order
does not distinguish instances. Prerequisites such as `Eq A` and `Ord A` do not make otherwise
overlapping heads disjoint. Constructor aliases are compared with shared rigid arguments, so
their behavior must agree for every argument rather than for one chosen concrete type.

An instance belongs in the module declaring its class or its outer carrier. Local sums, domains,
and named record declarations own their carriers. An alias of a primitive or imported carrier
does not create ownership; wrapping a local type in `List` does not make that module own `List`.

Shipped providers implement the standard primitive instances in `aivi.defaults` and
`aivi.core.bytes`, and the compiler-declared `NonEmptyList` instances in `aivi.nonEmpty`.
Provider authority follows the registered shipped source, so overriding an `aivi.*` module in
a project does not grant it ownership. Existing import-sensitive record defaults retain their
scope rules.

Imported authored classes retain their declaration identity, superclass requirements, method
signatures, and method-local constraints. Importing a class opens its methods for type-directed
lookup. Aliases and re-exports keep that identity, while separate classes with the same name
remain separate declarations. Nominal carriers retain their declaration identity across aliases
and re-exports as well; unrelated carriers with the same name remain distinct. Compiler-provided
instances belong to the standard class
declarations; declaring another class named `Functor` does not provide instances for it.

Authored dictionaries retain every declared member, including classes named `Eq`, `Ord` or
`Setoid`. Class identity determines dictionary layout: an unrelated authored class does not inherit
the standard class’s filtered executable dictionary layout merely by sharing its name.

The compiler bounds alias normalization and instance unification with explicit complexity
diagnostics. It does not select an instance when coherence cannot be established.

## Comparison evidence

Comparison operators select executable members by their types and declaration identities.
Equality requires `A -> A -> Bool`, using `(==)` before `equals`. Ordering requires
`compare : A -> A -> Ordering`, where `Ordering` is the standard ordering type. `!=` negates
the selected equality operation; it does not select a separate inequality member.

Function constraints supply comparison dictionaries first. Otherwise, the compiler considers
members opened by local class declarations and explicit class imports, then ambient members.
Class aliases and re-exports retain the original operation and instance identity. Importing only
a helper function does not open its private class's members. An authored class named `Eq` or
`Ord` must still supply a member with the required type; the name alone supplies no evidence.

For example, this class supports the ordinary equality operators:

```aivi
class Same A = { (==) : A -> A -> Bool }

type Same A => A -> A -> Bool
func equivalent = left right => left == right
```

Instance prerequisites and method-local constraints must be available for the selected member.
Multiple matching dictionaries produce an ambiguity diagnostic, including candidate class names;
narrow the function's constraints or call the desired member explicitly.

## Execution boundary: builtin carriers vs authored instances

AIVI has two executable higher-kinded paths today, and they are intentionally different:

| Path | What backs execution | What is proven today | What it does **not** mean |
| --- | --- | --- | --- |
| Builtin carriers | Registry-backed builtin executable evidence in `aivi-core` | The class/carrier pairs listed in the builtin table above | Declaring a new class or instance does **not** extend this builtin runtime table |
| Authored instances | Authored executable evidence pointing at compiler-lowered member bodies | Same-module and imported unary higher-kinded member calls such as `map` and `reduce`, when the checker can choose one concrete evidence item | Multi-parameter / indexed higher-kinded heads are still not an end-to-end executable slice |

### Hidden lowered member bodies

When you write an authored instance member, the compiler lowers each `(instance, member)` pair to a
hidden executable item body, then stores authored executable evidence that points at that lowered
body. Surface code still looks ordinary — you write `map f box`, not a synthetic helper call — but
the selected evidence ultimately dispatches to that hidden lowered member body.

The hidden callable takes instance prerequisites first, then the class member's own requirements,
then its visible arguments. For example, `instance Eq A => Eq (Box A)` receives `Eq A` evidence;
an authored `Traversable Box` receives the `Applicative G` operations required by `traverse`.
Imports preserve this order and share type-variable indices between the member signature and its
evidence. Direct calls, partial applications, pipes, and generic constrained functions use the same
callable contract.

```aivi
type Box A = Box A
instance Functor Box = { map = f box => box ||> Box a -> Box (f a) }
instance Foldable Box = { reduce = f seed box => box ||> Box a -> f seed a }
instance Traversable Box = { traverse = f box => box ||> Box a -> map Box (f a) }

type Int -> Option Int
func increment = n => Some (n + 1)
type Traversable F => F Int -> Option (F Int)
func advance = box => traverse increment box

value advanced : Option (Box Int) = advance (Box 2)
```

Both authored and builtin instances supply generic `Traversable F` dictionaries. Their `traverse`
callables receive `Applicative G` evidence before the visible mapper and source arguments; `G` can
remain abstract in a generic function. Empty inputs and error alternatives use that dictionary's
`pure`, and successful payloads use its `map` and `apply`. Traversal into `Task` constructs task
plans; execution stays at the task boundary. Native
compilation of authored evidence remains limited to the callable and constructor shapes supported
by the backend; interpreter or lazy JIT execution does not prove strict AOT support.

That is the key boundary to remember:

- builtin carriers execute through builtin evidence intrinsics from the canonical registry
- authored instances execute through hidden lowered member bodies chosen by evidence resolution
- imported unary higher-kinded calls work today because the compiler can export/import that authored evidence path; they do **not** turn user-defined carriers into new builtin carriers
- parser or checker acceptance alone is not a runtime promise if evidence cannot be selected concretely

## Comparison classes

`Eq A` and `Ord A` are the comparison-facing classes in the ambient prelude:

- `Eq A` backs `==`; surface `!=` reuses the same `Eq` evidence and behaves as inequality syntax over equality.
- `Ord A` exposes the primitive member `compare : A -> A -> Ordering`.
- Ordinary `<`, `>`, `<=`, and `>=` are derived from `Ord.compare`; they are not separate class members.
- Operator sections like `(<)` and `(>=)` follow the same `Ord.compare` lowering rule.

That means a nominal domain becomes orderable by implementing `Ord.compare` directly:

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

type Calendar -> Calendar -> Bool
func inOrder = start finish =>
    start <= finish

value differentWeek : Bool = 10day != 12day
```

You normally explain equality once and let surface `!=` reuse that same evidence. You also do not need
to author separate domain members for `<`, `>`, `<=`, or `>=`; those operators come from `Ord`.
Exported ordinary instances also travel across module boundaries, so imported values of that type pick
up the same operators.

Standard `Eq` instances need only implement `(==)`. A user-authored class named `Eq` keeps
every member it declares, including `(!=)` when present.

Compiler-derived equality checks every payload of a closed sum, record, or domain, including
imported declarations. Importing a type does not establish equality for its payloads: a sum
containing `Bytes` or a function still needs an explicit instance. Recursive proofs distinguish
the declaration and its actual type arguments, so `Box Int` cannot establish equality for a
recursive `Box Bytes` payload. Unavailable imported representations require explicit evidence.
Derived equality invokes the selected equality dictionary for each payload. An authored or
scoped instance therefore applies inside a list, tuple, record, option, result, validation, or
closed sum as well as in a direct comparison. Container length, field positions, and outer
constructor tags still determine which payloads are compared. First-class comparisons and
constrained helpers carry the same dictionaries across imports and cached execution.

Growing or excessively large proofs report `hir::equality-proof-complexity`; simplify the type
or provide explicit equality evidence. Native execution also requires a supported closed layout;
accepting a recursive proof does not make recursive native layouts executable.
Native dictionary calls also require matching callable representations. Passing a concrete
scalar dictionary to a helper whose generic operands use a boxed representation requires a
callable adapter; native compilation reports this unsupported conversion. Source execution
retains the same dictionary through its interpreter path.

## User-authored higher-kinded classes and instances

### Supported end to end today

- Same-module class declarations, including `with` superclasses and `require` constraints
- Same-module and imported use of ordinary first-order instances such as `Eq Date` or `Ord Calendar`
- Unary `instance` blocks for higher-kinded heads such as `instance Applicative Option`
- Partially applied heads with fixed or polymorphic prefixes, such as `instance Functor (Either E)`
- Same-module and imported use of unary higher-kinded members such as `map` and `reduce`, which lower to authored executable evidence when the checker can choose concrete evidence
- Bundled stdlib carriers can rely on this path; `aivi.matrix` exposes ambient `map` / `reduce` through user-authored `Functor` / `Foldable` instances rather than a new builtin carrier

### Not end to end today

- Multi-parameter indexed-style higher-kinded instance heads are not yet proven end to end
- Declaring a new higher-kinded class or instance does **not** create new builtin runtime support for arbitrary carriers

In practice, unary user-authored higher-kinded classes and instances are trustworthy today for imported execution through the current executable-evidence lowering path, but indexed / multi-parameter evidence remains a design frontier rather than a finished executable slice.

## Related pages

- [Classes](/guide/classes) for syntax and local examples
- [Class Laws & Design Boundaries](/guide/class-laws) for the semantic contract behind each class
- [Pipes & Operators](/guide/pipes) for `*|>` and applicative clustering with `&|>`
- [aivi.prelude](/stdlib/prelude) for the ambient types and class names
