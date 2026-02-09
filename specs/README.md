# AIVI Language Specification

A high-integrity functional language targeting WebAssembly.

**Version:** 0.1 (Draft)


## Table of Contents

### Core Specification

1. [Introduction](01_introduction.md)

### Roadmap

- [Roadmap](roadmap/README.md)

### Syntax

2. [Concrete Syntax (EBNF draft)](02_syntax/00_grammar.md)
3. [Bindings and Scope](02_syntax/01_bindings.md)
4. [Functions and Pipes](02_syntax/02_functions.md)
5. [The Type System](02_syntax/03_types.md)
6. [Predicates](02_syntax/04_predicates.md)
7. [Patching Records](02_syntax/05_patching.md)
8. [Domains, Units, and Deltas](02_syntax/06_domains.md)
9. [Generators](02_syntax/07_generators.md)
10. [Pattern Matching](02_syntax/08_pattern_matching.md)
11. [Effects](02_syntax/09_effects.md)
12. [Modules](02_syntax/10_modules.md)
13. [Domain Definitions](02_syntax/11_domain_definition.md)
14. [External Sources](02_syntax/12_external_sources.md)
15. [Decorators](02_syntax/14_decorators.md)
16. [Resources](02_syntax/15_resources.md)

### Kernel (Core Calculus)

19. [Core Terms](03_kernel/01_core_terms.md)
20. [Types](03_kernel/02_types.md)
21. [Records](03_kernel/03_records.md)
22. [Patterns](03_kernel/04_patterns.md)
23. [Predicates](03_kernel/05_predicates.md)
24. [Traversals](03_kernel/06_traversals.md)
25. [Generators](03_kernel/07_generators.md)
26. [Effects](03_kernel/08_effects.md)
27. [Classes](03_kernel/09_classes.md)
28. [Domains](03_kernel/10_domains.md)
29. [Patching](03_kernel/11_patching.md)
30. [Minimality Proof](03_kernel/12_minimality.md)

### Desugaring (Syntax → Kernel)

31. [Bindings](04_desugaring/01_bindings.md)
32. [Functions](04_desugaring/02_functions.md)
33. [Records](04_desugaring/03_records.md)
34. [Patterns](04_desugaring/04_patterns.md)
35. [Predicates](04_desugaring/05_predicates.md)
36. [Generators](04_desugaring/06_generators.md)
37. [Effects](04_desugaring/07_effects.md)
38. [Classes](04_desugaring/08_classes.md)
39. [Domains and Operators](04_desugaring/09_domains.md)
40. [Patching](04_desugaring/10_patching.md)

### Standard Library

### Standard Library

#### Core & Utils
41. [Prelude](05_stdlib/00_core/01_prelude.md)
42. [Units](05_stdlib/00_core/16_units.md)
43. [Regex](05_stdlib/00_core/24_regex.md)
44. [Testing](05_stdlib/00_core/27_testing.md)
45. [Collections](05_stdlib/00_core/28_collections.md)

#### Math & Science
46. [Vector](05_stdlib/01_math/05_vector.md)
47. [Matrix](05_stdlib/01_math/09_matrix.md)
48. [Complex](05_stdlib/01_math/10_complex.md)
49. [Quaternion](05_stdlib/01_math/11_quaternion.md)
50. [Rational & BigInt](05_stdlib/01_math/12_rational_bigint.md)
51. [Probability](05_stdlib/01_math/13_probability.md)
52. [FFT & Signal](05_stdlib/01_math/14_signal.md)
53. [Geometry](05_stdlib/01_math/15_geometry.md)
54. [Graph](05_stdlib/01_math/17_graph.md)
55. [Linear Algebra](05_stdlib/01_math/18_linear_algebra.md)

#### Chronos (Time)
56. [Calendar](05_stdlib/02_chronos/02_calendar.md)
57. [Duration](05_stdlib/02_chronos/03_duration.md)

#### System & IO
58. [File](05_stdlib/03_system/20_file.md)
59. [Console](05_stdlib/03_system/21_console.md)
60. [HTTP](05_stdlib/03_system/19_http.md)
62. [Crypto](05_stdlib/03_system/22_crypto.md)
63. [System](05_stdlib/03_system/25_system.md)
64. [Log](05_stdlib/03_system/26_log.md)

#### UI
65. [Color](05_stdlib/04_ui/04_color.md)


## Building the Specification

### VitePress (recommended)

```bash
npm install
npm run docs:dev
```

Build static site:

```bash
npm run docs:build
```

