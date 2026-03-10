# The Type System

<!-- quick-info: {"kind":"topic","name":"type system"} -->
AIVI's type system describes the shape of values and effects, catches mismatches early, and makes APIs easier to read.
If you are new to functional programming, think of it as a precise contract system: every value has a known shape, and the compiler checks that those shapes line up.
<!-- /quick-info -->

This overview points you to the pages that define each part of the type system.
Read them as complementary pieces: primitives and records describe data, ADTs model alternatives, opaque types hide representations across module boundaries, and the later pages cover reuse, abstraction, and boundary conveniences.

## Start with the page that matches your problem

- [3.1 Primitive Types](types/primitive_types.md) — built-in scalar types, time-related literals, and branded aliases such as `Email = Text!`.
- [3.2 Algebraic Data Types](types/algebraic_data_types.md) — named cases such as `Option`, `Result`, or your own sum types like `Draft | Published`.
- [3.3 Closed Records](types/closed_records.md) — exact structural record types and record literals for fixed-shape data.
- [3.4 Record Row Transforms](types/record_row_transforms.md) — derive nearby record shapes with tools such as `Pick`, `Omit`, `Optional`, and `Rename`.
- [3.5 Classes and Higher-Kinded Types (HKTs)](types/classes_and_hkts.md) — shared behaviour across concrete types and container shapes.
- [3.6 Expected-Type Coercions](types/expected_type_coercions.md) — context-sensitive rewrites such as `toText`, `Body`, record defaults, and `Option` wrapping when the destination type is already known.
- [3.7 Opaque Types](types/opaque_types.md) — expose a public type name while hiding its representation outside the defining module.

## Closely related pages

These pages are not type-definition forms themselves, but they are the main ways you use the shapes described here:

- [Pattern Matching](pattern_matching.md) — inspect ADTs, records, tuples, and lists by shape.
- [Patching Records](patching.md) — apply typed structural updates to immutable data.
- [Domains](domains.md) — add typed operators and suffix literals for specific carrier types such as dates, durations, angles, or colors.

## A practical reading order

If you are learning AIVI for the first time, this order usually works well:

1. Primitive types
2. Closed records
3. Algebraic data types
4. Opaque types
5. Expected-type coercions
6. Classes and HKTs
7. Record row transforms

After that, read Pattern Matching, Patching Records, and Domains as needed for the style of code you are writing.

That order mirrors how most programs are written: start with simple values, group them into records, model alternatives with ADTs, hide representations where invariants matter, and then learn the more advanced reuse and abstraction tools.

