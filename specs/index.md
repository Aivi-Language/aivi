---
title: AIVI Language Specification
---

<p style="
    background-color: #334;
    border-radius: 20px;
    width: fit-content;
    padding: 1rem;
    margin: 0 auto 3rem auto;
">
    <img src="../assets/aivi-128.png" alt="AIVI Logo" width="128" height="128">
</p>

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
13. [Sigils](02_syntax/13_sigils)
14. [External Sources](02_syntax/12_external_sources)
15. [Decorators](02_syntax/14_decorators)
16. [Resources](02_syntax/15_resources)

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
42. [Text Module](05_stdlib/00_core/02_text)
43. [Units Domain](05_stdlib/00_core/16_units)
44. [Regex Domain](05_stdlib/00_core/24_regex)
45. [Testing Domain](05_stdlib/00_core/27_testing)
46. [Collections Domain](05_stdlib/00_core/28_collections)

#### Math & Science
47. [Math Module](05_stdlib/01_math/01_math)
48. [Vector Domain](05_stdlib/01_math/05_vector)
49. [Matrix Domain](05_stdlib/01_math/09_matrix)
50. [Number Domains (BigInt, Rational, Complex, Quaternion)](05_stdlib/01_math/10_number)
51. [Probability](05_stdlib/01_math/13_probability)
52. [FFT & Signal](05_stdlib/01_math/14_signal)
53. [Geometry Domain](05_stdlib/01_math/15_geometry)
54. [Graph Domain](05_stdlib/01_math/17_graph)
55. [Linear Algebra](05_stdlib/01_math/18_linear_algebra)

#### Chronos (Time)
56. [Calendar Domain](05_stdlib/02_chronos/02_calendar)
57. [Duration Domain](05_stdlib/02_chronos/03_duration)

#### Network
58. [Network Package](05_stdlib/03_network/00_network)
59. [HTTP Domain](05_stdlib/03_network/01_http)
60. [HTTPS Domain](05_stdlib/03_network/02_https)
61. [HTTP Server Domain](05_stdlib/03_network/03_http_server)
62. [Sockets Domain](05_stdlib/03_network/04_sockets)
63. [Streams Domain](05_stdlib/03_network/05_streams)

#### System & IO
64. [File Domain](05_stdlib/03_system/20_file)
65. [Console Domain](05_stdlib/03_system/21_console)
66. [Database Domain](05_stdlib/03_system/23_database)
67. [URL Domain](05_stdlib/03_system/25_url)
68. [Crypto Domain](05_stdlib/03_system/22_crypto)
69. [System Domain](05_stdlib/03_system/25_system)
70. [Log Domain](05_stdlib/03_system/26_log)

#### UI
71. [Color Domain](05_stdlib/04_ui/04_color)

### Execution & Concurrency

107. [Concurrency](06_runtime/01_concurrency)

109. [Package Manager (Cargo-backed)](06_runtime/03_package_manager)
