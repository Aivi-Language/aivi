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
- `Validation E` is intentionally **not** a `Monad`: independent accumulation stays applicative (`&|>` / `zipValidation`), while dependent `!|>` checks are a dedicated pipe primitive rather than class-backed `bind`.
- `Traversable` support and traverse-result applicative support are distinct registry checks: `traverse` itself is builtin-supported for `List`, `Option`, `Result`, and `Validation`, while traverse results may use `List`, `Option`, `Result`, `Validation`, or `Signal` applicatives, but not `Task`.
<!-- END builtin-executable-support -->

For the law contract behind this hierarchy and the rationale for why `Signal` and `Validation`
intentionally stop at `Applicative`, see [Class Laws & Design Boundaries](/guide/class-laws).

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

## User-authored higher-kinded classes and instances

### Supported end to end today

- Same-module class declarations, including `with` superclasses and `require` constraints
- Same-module and imported use of ordinary first-order instances such as `Eq Date` or `Ord Calendar`
- Unary `instance` blocks for higher-kinded heads such as `instance Applicative Option`
- Partially applied heads such as `instance Functor (Result Text)`
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
