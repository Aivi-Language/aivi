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

- [Missing Features & Gap Analysis (v0.1)](missing_features_v0.1.md)

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

17. [Core Terms](03_kernel/01_core_terms)
18. [Types](03_kernel/02_types)
19. [Records](03_kernel/03_records)
20. [Patterns](03_kernel/04_patterns)
21. [Predicates](03_kernel/05_predicates)
22. [Traversals](03_kernel/06_traversals)
23. [Generators](03_kernel/07_generators)
24. [Effects](03_kernel/08_effects)
25. [Classes](03_kernel/09_classes)
26. [Domains](03_kernel/10_domains)
27. [Patching](03_kernel/11_patching)
28. [Minimality Proof](03_kernel/12_minimality)

### Desugaring (Syntax → Kernel)

29. [Bindings](04_desugaring/01_bindings)
30. [Functions](04_desugaring/02_functions)
31. [Records](04_desugaring/03_records)
32. [Patterns](04_desugaring/04_patterns)
33. [Predicates](04_desugaring/05_predicates)
34. [Generators](04_desugaring/06_generators)
35. [Effects](04_desugaring/07_effects)
36. [Classes](04_desugaring/08_classes)
37. [Domains and Operators](04_desugaring/09_domains)
38. [Patching](04_desugaring/10_patching)

### Standard Library

### Standard Library

#### Core & Utils
39. [Prelude](05_stdlib/00_core/01_prelude)
40. [Text Module](05_stdlib/00_core/02_text)
41. [Logic Module](05_stdlib/00_core/03_logic)
42. [Units Domain](05_stdlib/00_core/16_units)
43. [Regex Domain](05_stdlib/00_core/24_regex)
44. [Testing Domain](05_stdlib/00_core/27_testing)
45. [Collections Domain](05_stdlib/00_core/28_collections)

#### Math & Science
46. [Math Module](05_stdlib/01_math/01_math)
47. [Vector Domain](05_stdlib/01_math/05_vector)
48. [Matrix Domain](05_stdlib/01_math/09_matrix)
49. [Number Domains (BigInt, Rational, Complex, Quaternion)](05_stdlib/01_math/10_number)
50. [Probability](05_stdlib/01_math/13_probability)
51. [FFT & Signal](05_stdlib/01_math/14_signal)
52. [Geometry Domain](05_stdlib/01_math/15_geometry)
53. [Graph Domain](05_stdlib/01_math/17_graph)
54. [Linear Algebra](05_stdlib/01_math/18_linear_algebra)

#### Chronos (Time)
55. [Calendar Domain](05_stdlib/02_chronos/02_calendar)
56. [Duration Domain](05_stdlib/02_chronos/03_duration)

#### Network
57. [Network Package](05_stdlib/03_network/00_network)
58. [HTTP Domain](05_stdlib/03_network/01_http)
59. [HTTPS Domain](05_stdlib/03_network/02_https)
60. [HTTP Server Domain](05_stdlib/03_network/03_http_server)
61. [Sockets Domain](05_stdlib/03_network/04_sockets)
62. [Streams Domain](05_stdlib/03_network/05_streams)

#### System & IO
63. [File Domain](05_stdlib/03_system/20_file)
64. [Console Domain](05_stdlib/03_system/21_console)
65. [Database Domain](05_stdlib/03_system/23_database)
66. [URL Domain](05_stdlib/03_system/25_url)
67. [Crypto Domain](05_stdlib/03_system/22_crypto)
68. [System Domain](05_stdlib/03_system/25_system)
69. [Log Domain](05_stdlib/03_system/26_log)

#### UI
70. [Color Domain](05_stdlib/04_ui/04_color)

### Execution & Concurrency

71. [Concurrency](06_runtime/01_concurrency)
72. [Package Manager (Cargo-backed)](06_runtime/03_package_manager)
