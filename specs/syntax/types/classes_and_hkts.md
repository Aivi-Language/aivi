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

- `instance ClassName (TypeExpr) = { ... }` defines a dictionary value for a class implementation.
- In `Result E *`, `E` is a type parameter and `*` is the remaining type slot for higher-kinded types. Read it as: “`Result` with the error fixed to `E`, as a 1-parameter type constructor”.

> [!NOTE] Implementation Note: Kinds
> In the v0.1 compiler, kind annotations like `(F *)` are enforced by the type checker.
