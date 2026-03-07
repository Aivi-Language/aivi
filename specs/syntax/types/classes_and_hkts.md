# 3.5 Classes and HKTs

This page covers two related ideas:

- **classes** describe behaviour that many types can share
- **HKTs** let those classes talk about type shapes such as `List`, `Option`, or `Result E`, not just fully finished types

If you know interfaces or traits, a class fills a similar role. The main difference is that the compiler chooses an instance from the type, rather than looking for methods stored on an object.

## A few terms in plain language

| Term | Plain-language meaning |
| --- | --- |
| class | A shared behaviour contract |
| instance | One implementation of that contract for one type or type shape |
| higher-kinded type (HKT) | A parameter such as `F` that stands for a type-building shape like `List` or `Option` |
| universally quantified | If a signature mentions `A` or `B`, read it as “for any `A`” or “for any `B`” unless a `given (...)` constraint says otherwise |

## Classes in plain language

Reach for a class when you want many types to support the same operation name with type-safe implementations.

Typical examples include equality, conversion to text, default values, and container-oriented operations such as `map`.

The caller-side benefit is simple:

- the caller writes one operation name, such as `toText value`
- the compiler picks the matching implementation from the value’s type

## What HKTs add

Ordinary type parameters talk about finished types such as `Int`, `Text`, or `User`.

HKTs go one step more abstract: they talk about a **type shape** that still needs a value type. For example:

- `List` is a shape that becomes `List Int`, `List Text`, and so on
- `Option` is a shape that becomes `Option User`, `Option Error`, and so on
- `Result E` becomes a one-parameter shape once the error type `E` is fixed

That extra level is what lets one class describe operations that work across many containers.

## The basic building blocks

The examples below use container-oriented classes, so HKTs show up immediately.

### A class over a container shape

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_01.aivi{aivi}

Read `class Functor (F A)` as “for any container shape `F` and element type `A`”.

The member signature uses AIVI’s abbreviated HKT form. The compiler reads:

```text
map : (A -> B) -> F B
```

as:

```text
map : (A -> B) -> F A -> F B
```

In other words, the container input is added internally as the last argument.

### Building a class family

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_02.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_03.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_04.aivi{aivi}

These declarations say that `Applicative` builds on `Apply`, and `Chain` builds on `Apply` too. The container shape `F` stays the same; each class simply asks that shape to support more operations.

### Combining classes

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_05.aivi{aivi}

This reads as: a `Monad` is anything that already has both the `Applicative` and `Chain` behaviour.

### Writing instances

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_06.aivi{aivi}

An instance picks one concrete type or type expression and supplies the implementation for that class.

## Type variable constraints

Class declarations may attach constraints to the type variables used in member signatures with `given (...)`.

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_01.aivi{aivi}

`A with B` in a type position means record/type composition: combine the requirements of both sides.

Instances use the same idea from the implementation side:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_02.aivi{aivi}

In plain language, a `given (...)` clause says “this member or instance works for any type variables that also satisfy these class requirements”.

## Practical notes

- `instance ClassName (TypeExpr) = { ... }` defines an implementation for one concrete type or type expression.
- In `Result E A`, `E` can be fixed while `A` remains the varying type parameter. Read `Result E` as “a one-parameter container once the error type is chosen”.
- Type variables in class and instance declarations are implicitly universally quantified. If you see `A` or `B`, read that as “for any `A`” or “for any `B`” unless a `given (...)` clause narrows it.
- HKT class member signatures use the abbreviated form described above: the container argument is omitted from the parameter list and added internally by the compiler as the last argument.
- Constructor-style members whose return type already names the container, such as `of: A -> F A`, are not expanded.

## Zero-argument members and expected types

Some class members are values rather than functions.
A classic example is `empty` from `Monoid`: there are no arguments to inspect, so the compiler uses the surrounding expected type to decide which instance to pick.

```aivi
emptyList : List Int
emptyList = empty       // expected type says: use the `List` instance

emptyMap : Map Text Int
emptyMap = empty        // expected type says: use the `Map` instance
```

When there is no type context, the compiler cannot guess:

```text
error: cannot resolve class member 'empty' (from Monoid) without type context
       — add a type annotation or use a qualified form (e.g. List.empty)
```

Qualified forms such as `List.empty` and `Map.empty` work even when no surrounding type annotation is available.
