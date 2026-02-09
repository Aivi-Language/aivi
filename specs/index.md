---
title: AIVI Language Specification
---

# AIVI Language Specification

A high-integrity functional language with a Rust-first compilation pipeline.

## Table of Contents

### Core Specification

1. [Introduction](01_introduction)

### Roadmap

- [Roadmap](roadmap/README.md)

### Syntax

2. [Concrete Syntax (EBNF draft)](02_syntax/00_grammar)
3. [Bindings and Scope](02_syntax/01_bindings)
4. [Functions and Pipes](02_syntax/02_functions)
5. [The Type System](02_syntax/03_types)
6. [Predicates](02_syntax/04_predicates)
7. [Patching Records](02_syntax/05_patching)
8. [Domains, Units, and Deltas](02_syntax/06_domains)
9. [Generators](02_syntax/07_generators)
10. [Pattern Matching](02_syntax/08_pattern_matching)
11. [Effects](02_syntax/09_effects)
12. [Modules](02_syntax/10_modules)
13. [Domain Definitions](02_syntax/11_domain_definition)
14. [External Sources](02_syntax/12_external_sources)
15. [Decorators](02_syntax/14_decorators)
15. [Decorators](02_syntax/14_decorators)
16. [Resources](02_syntax/15_resources)
17. [Tagged Literals](02_syntax/16_tagged_literals)

### Kernel (Core Calculus)

19. [Core Terms](03_kernel/01_core_terms)
20. [Types](03_kernel/02_types)
21. [Records](03_kernel/03_records)
22. [Patterns](03_kernel/04_patterns)
23. [Predicates](03_kernel/05_predicates)
24. [Traversals](03_kernel/06_traversals)
25. [Generators](03_kernel/07_generators)
26. [Effects](03_kernel/08_effects)
27. [Classes](03_kernel/09_classes)
28. [Domains](03_kernel/10_domains)
29. [Patching](03_kernel/11_patching)
30. [Minimality Proof](03_kernel/12_minimality)

### Desugaring (Syntax → Kernel)

31. [Bindings](04_desugaring/01_bindings)
32. [Functions](04_desugaring/02_functions)
33. [Records](04_desugaring/03_records)
34. [Patterns](04_desugaring/04_patterns)
35. [Predicates](04_desugaring/05_predicates)
36. [Generators](04_desugaring/06_generators)
37. [Effects](04_desugaring/07_effects)
38. [Classes](04_desugaring/08_classes)
39. [Domains and Operators](04_desugaring/09_domains)
40. [Patching](04_desugaring/10_patching)

### Standard Library

### Standard Library

#### Core & Utils
41. [Prelude](05_stdlib/00_core/01_prelude)
42. [Units Domain](05_stdlib/00_core/16_units)
43. [Regex Domain](05_stdlib/00_core/24_regex)
44. [Testing Domain](05_stdlib/00_core/27_testing)
45. [Collections Domain](05_stdlib/00_core/28_collections)

#### Math & Science
46. [Vector Domain](05_stdlib/01_math/05_vector)
47. [Matrix Domain](05_stdlib/01_math/09_matrix)
48. [Complex Domain](05_stdlib/01_math/10_complex)
49. [Quaternion Domain](05_stdlib/01_math/11_quaternion)
50. [Rational & BigInt](05_stdlib/01_math/12_rational_bigint)
51. [Probability](05_stdlib/01_math/13_probability)
52. [FFT & Signal](05_stdlib/01_math/14_signal)
53. [Geometry Domain](05_stdlib/01_math/15_geometry)
54. [Graph Domain](05_stdlib/01_math/17_graph)
55. [Linear Algebra](05_stdlib/01_math/18_linear_algebra)

#### Chronos (Time)
56. [Calendar Domain](05_stdlib/02_chronos/02_calendar)
57. [Duration Domain](05_stdlib/02_chronos/03_duration)

#### System & IO
58. [File Domain](05_stdlib/03_system/20_file)
59. [Console Domain](05_stdlib/03_system/21_console)
60. [HTTP Domain](05_stdlib/03_system/19_http)
62. [Crypto Domain](05_stdlib/03_system/22_crypto)
63. [System Domain](05_stdlib/03_system/25_system)
64. [Log Domain](05_stdlib/03_system/26_log)

#### UI
65. [Color Domain](05_stdlib/04_ui/04_color)

### Execution & Concurrency

107. [Concurrency](06_runtime/01_concurrency)
108. [Rustc Native Pipeline](06_runtime/02_rustc_native_pipeline)
