# 3.5 Classes and HKTs

Classes let you describe behaviour that many types can share.
If you are familiar with interfaces, a class fills a similar role, but implementations are selected by type rather than stored on objects.

Higher-kinded types (HKTs) let a class talk about type constructors such as `List`, `Option`, or `Result E` instead of only concrete types.
That is what makes abstractions like `map` work across many different containers.

## The basic building blocks

The following examples show the core syntax for declaring classes, defining instances, and working with higher-kinded parameters:

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_01.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_02.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_03.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_04.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_05.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_06.aivi{aivi}

## When to reach for a class

Use a class when you want many types to support the same operation name with type-safe implementations.
Typical examples include equality, conversion to text, default values, and container-oriented operations such as `map`.

This is especially useful when you want caller code to stay simple:

- the caller writes `toText value`
- the compiler picks the right implementation from the value's type

## Type variable constraints

Class declarations may attach constraints to the type variables used in member signatures with `given (...)`:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_01.aivi{aivi}

`A with B` in a type position means record/type composition: combine the requirements of both sides.

Instances use the same idea from the implementation side:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_02.aivi{aivi}

## Practical notes

- `instance ClassName (TypeExpr) = { ... }` defines an implementation for one concrete type or type expression.
- In `Result E A`, `E` can be fixed while `A` remains the varying type parameter. Read `Result E` as "a one-parameter container once the error type is chosen".
- Type variables in class and instance declarations are implicitly universally quantified. Add `given (...)` only when you need a real constraint.
- HKT class member signatures use an abbreviated form: the container argument is omitted from the parameter list and added internally by the compiler as the last argument. For example, `map: (A -> B) -> F B` expands to `map: (A -> B) -> F A -> F B`.
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
