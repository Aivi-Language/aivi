# 3.5 Classes and HKTs

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_01.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_02.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_03.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_04.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_05.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_06.aivi{aivi}

## Type Variable Constraints

Class declarations may attach constraints to the **type variables used in member signatures**
using `given (...)`:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_01.aivi{aivi}

`A with B` in type position denotes **record/type composition** (an intersection-like merge).

Instances:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_02.aivi{aivi}

Notes:

- `instance ClassName (TypeExpr) = { ... }` defines a dictionary value for a class implementation.
- In `Result E A`, `E` is a fixed type parameter and `A` is the HKT type variable. Read it as: “`Result` with the error fixed to `E`, as a 1-parameter type constructor”.
- Type variables in class/instance declarations are implicitly universally quantified. Use `given (A: ClassName)` only when a real constraint is needed (e.g., `given (A: Eq)`). `given (A: Any)` is redundant and should be omitted.
- HKT class member signatures use **abbreviated form**: the container type is omitted from the parameter list and added internally by the compiler as the last argument. For example, `map: (A -> B) -> F B` expands to `map: (A -> B) -> F A -> F B`.
- Constructor methods whose return type equals the container (e.g., `of: A -> F A`) are not expanded.

## Bidirectional Resolution of Zero-Argument Members

Class members that are values (not functions) — such as `empty` from `Monoid` — cannot
be dispatched by argument type at runtime. Instead, the compiler resolves them at
compile time using **bidirectional type inference**: the expected type from the
surrounding context (type annotation, function signature, etc.) determines which
instance to use.

```aivi
emptyList : List Int
emptyList = empty       -- resolves to Monoid (List A) instance → []

emptyMap : Map Text Int
emptyMap = empty        -- resolves to Monoid (Map K V) instance → Map.empty
```

When no type context is available, the compiler reports an error and suggests
adding a type annotation or using a qualified form:

```
error: cannot resolve class member 'empty' (from Monoid) without type context
       — add a type annotation or use a qualified form (e.g. List.empty)
```

Qualified forms (`List.empty`, `Map.empty`) always work regardless of context.

