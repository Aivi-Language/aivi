# 3.5 Classes and HKTs

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_01.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_02.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_03.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_04.aivi{aivi}

<<< ../../snippets/from_md/syntax/types/classes_and_hkts_05.aivi{aivi}

## Type Variable Constraints

Class declarations may attach constraints to the **type variables used in member signatures**
using `given (...)`:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_01.aivi{aivi}

`A with B` in type position denotes **record/type composition** (an intersection-like merge).

Instances:

<<< ../../snippets/from_md/syntax/types/type_variable_constraints_02.aivi{aivi}

Notes:

- `instance ClassName (TypeExpr) = given (A: Any) { ... }` defines a dictionary value for a class implementation.
- In `Result E A`, `E` is a fixed type parameter and `A` is the type variable bound by `given (A: Any)`. Read it as: “`Result` with the error fixed to `E`, as a 1-parameter type constructor”.
- HKT class member signatures use **abbreviated form**: the container type is omitted from the parameter list and added internally by the compiler as the last argument. For example, `map: (A -> B) -> F B` expands to `map: (A -> B) -> F A -> F B`.
- Constructor methods whose return type equals the container (e.g., `of: A -> F A`) are not expanded.

> [!NOTE] Implementation Note: Kinds
> In the v0.1 compiler, kind annotations like `(F A) = given (A: Any)` are enforced by the type checker.
