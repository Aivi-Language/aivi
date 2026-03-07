# The Type System

AIVI's type system helps you describe the shape of data, catch mistakes early, and make APIs easier to read.
If you are new to functional programming, you can think of it as a precise contract system: every value has a known shape, and the compiler checks that shapes line up.

## How to use this section

Start with the pages that match the kind of data you are working with:

- [3.1 Primitive Types](types/primitive_types.md) — numbers, text, booleans, time-related values, and branded single-field types.
- [3.2 Algebraic Data Types](types/algebraic_data_types.md) — named cases like `Option`, `Result`, or your own custom sum types.
- [3.3 Closed Records](types/closed_records.md) — fixed-shape records for structured data.
- [3.4 Record Row Transforms](types/record_row_transforms.md) — type-level tools such as `Pick` and `Omit` for reusing record shapes.
- [3.5 Classes and HKTs](types/classes_and_hkts.md) — shared behaviour across types and containers.
- [3.6 Expected-Type Coercions](types/expected_type_coercions.md) — limited, instance-driven conversions in places where the expected type is already known.

## A practical reading order

If you are learning AIVI for the first time, this order usually works well:

1. Primitive types
2. Closed records
3. Algebraic data types
4. Expected-type coercions
5. Classes and HKTs
6. Record row transforms

That order mirrors how most programs are written: start with simple values, group them into records, model alternatives with ADTs, then learn the more advanced reuse and abstraction tools.

## Related: machine types

Machine declarations and runtime behaviour are documented in the dedicated Machines section:

- [Machine Runtime Semantics](machines_runtime.md)
