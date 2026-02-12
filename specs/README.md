# AIVI Language Specification

A high-integrity functional language targeting WebAssembly.

**Version:** 0.1 (Draft)


## Table of Contents

### Core Specification

1. [Introduction](01_introduction.md)

### Roadmap

- [Missing Features & Gap Analysis (v0.1)](missing_features_v0.1.md)

### Syntax

2. [Concrete Syntax (EBNF draft)](02_syntax/00_grammar.md)
3. [Bindings and Scope](02_syntax/01_bindings.md)
4. [Functions and Pipes](02_syntax/02_functions.md)
5. [The Type System](02_syntax/03_types.md)
6. [Predicates](02_syntax/04_predicates.md)
7. [Patching Records](02_syntax/05_patching.md)
826. [Domains, Units, and Sigils](02_syntax/06_domains.md)
27. [Generators](02_syntax/07_generators.md)
28. [Pattern Matching](02_syntax/08_pattern_matching.md)
29. [Effects](02_syntax/09_effects.md)
30. [Modules](02_syntax/10_modules.md)
31. [External Sources](02_syntax/12_external_sources.md)
32. [Decorators](02_syntax/14_decorators.md)
33. [Resources](02_syntax/15_resources.md)

### Kernel (Core Calculus)

17. [Core Terms](03_kernel/01_core_terms.md)
18. [Types](03_kernel/02_types.md)
19. [Records](03_kernel/03_records.md)
20. [Patterns](03_kernel/04_patterns.md)
21. [Predicates](03_kernel/05_predicates.md)
22. [Traversals](03_kernel/06_traversals.md)
23. [Generators](03_kernel/07_generators.md)
24. [Effects](03_kernel/08_effects.md)
25. [Classes](03_kernel/09_classes.md)
26. [Domains](03_kernel/10_domains.md)
27. [Patching](03_kernel/11_patching.md)
28. [Minimality Proof](03_kernel/12_minimality.md)

### Desugaring (Syntax → Kernel)

29. [Bindings](04_desugaring/01_bindings.md)
30. [Functions](04_desugaring/02_functions.md)
31. [Records](04_desugaring/03_records.md)
32. [Patterns](04_desugaring/04_patterns.md)
33. [Predicates](04_desugaring/05_predicates.md)
34. [Generators](04_desugaring/06_generators.md)
35. [Effects](04_desugaring/07_effects.md)
36. [Classes](04_desugaring/08_classes.md)
37. [Domains and Operators](04_desugaring/09_domains.md)
38. [Patching](04_desugaring/10_patching.md)

### Standard Library

### Standard Library

#### Core & Utils
39. [Prelude](05_stdlib/00_core/01_prelude.md)
40. [Text](05_stdlib/00_core/02_text.md)
41. [Units](05_stdlib/00_core/16_units.md)
42. [Regex](05_stdlib/00_core/24_regex.md)
43. [Testing](05_stdlib/00_core/27_testing.md)
44. [Collections](05_stdlib/00_core/28_collections.md)
45. [I18n](05_stdlib/00_core/29_i18n.md)

#### Math & Science
45. [Math](05_stdlib/01_math/01_math.md)
46. [Vector](05_stdlib/01_math/05_vector.md)
47. [Matrix](05_stdlib/01_math/09_matrix.md)
48. [Number (BigInt, Rational, Complex, Quaternion)](05_stdlib/01_math/10_number.md)
49. [Probability](05_stdlib/01_math/13_probability.md)
50. [FFT & Signal](05_stdlib/01_math/14_signal.md)
51. [Geometry](05_stdlib/01_math/15_geometry.md)
52. [Graph](05_stdlib/01_math/17_graph.md)
53. [Linear Algebra](05_stdlib/01_math/18_linear_algebra.md)

#### Chronos (Time)
54. [Calendar](05_stdlib/02_chronos/02_calendar.md)
55. [Duration](05_stdlib/02_chronos/03_duration.md)

#### Network
56. [Network Package](05_stdlib/03_network/00_network.md)
57. [HTTP](05_stdlib/03_network/01_http.md)
58. [HTTPS](05_stdlib/03_network/02_https.md)
59. [HTTP Server](05_stdlib/03_network/03_http_server.md)
60. [Sockets](05_stdlib/03_network/04_sockets.md)
61. [Streams](05_stdlib/03_network/05_streams.md)

#### System & IO
62. [File](05_stdlib/03_system/20_file.md)
63. [Console](05_stdlib/03_system/21_console.md)
64. [Database](05_stdlib/03_system/23_database.md)
65. [URL](05_stdlib/03_system/25_url.md)
66. [Crypto](05_stdlib/03_system/22_crypto.md)
67. [System](05_stdlib/03_system/25_system.md)
68. [Log](05_stdlib/03_system/26_log.md)

#### UI
69. [Color](05_stdlib/04_ui/04_color.md)

### Execution & Concurrency

70. [Concurrency](06_runtime/01_concurrency.md)
71. [Package Manager (Cargo-backed)](06_runtime/03_package_manager.md)


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
