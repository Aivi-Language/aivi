---
title: AIVI Language Specification
---

# AIVI Language Specification

A high-integrity functional language targeting WebAssembly.

## Table of Contents

### Core Specification

1. [Introduction](01_introduction)

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
15. [JSX Literals](02_syntax/13_jsx_literals)
16. [Decorators](02_syntax/14_decorators)
17. [Resources](02_syntax/15_resources)

### Kernel (Core Calculus)

18. [Core Terms](03_kernel/01_core_terms)
19. [Types](03_kernel/02_types)
20. [Records](03_kernel/03_records)
21. [Patterns](03_kernel/04_patterns)
22. [Predicates](03_kernel/05_predicates)
23. [Traversals](03_kernel/06_traversals)
24. [Generators](03_kernel/07_generators)
25. [Effects](03_kernel/08_effects)
26. [Classes](03_kernel/09_classes)
27. [Domains](03_kernel/10_domains)
28. [Patching](03_kernel/11_patching)
29. [Minimality Proof](03_kernel/12_minimality)

### Desugaring (Syntax → Kernel)

30. [Bindings](04_desugaring/01_bindings)
31. [Functions](04_desugaring/02_functions)
32. [Records](04_desugaring/03_records)
33. [Patterns](04_desugaring/04_patterns)
34. [Predicates](04_desugaring/05_predicates)
35. [Generators](04_desugaring/06_generators)
36. [Effects](04_desugaring/07_effects)
37. [Classes](04_desugaring/08_classes)
38. [Domains and Operators](04_desugaring/09_domains)
39. [Patching](04_desugaring/10_patching)

### Standard Library

40. [Prelude](05_stdlib/01_prelude)
41. [Calendar Domain](05_stdlib/02_calendar)
42. [Duration Domain](05_stdlib/03_duration)
43. [Color Domain](05_stdlib/04_color)
44. [Vector Domain](05_stdlib/05_vector)
45. [HTML Domain](05_stdlib/06_html)
46. [Style Domain](05_stdlib/07_style)
47. [SQLite Domain](05_stdlib/08_sqlite)

### Runtime

48. [Concurrency](06_runtime/01_concurrency)

### Ideas & Future Directions

49. [WASM Target](ideas/01_wasm_target)
50. [LiveView Frontend](ideas/02_liveview_frontend)
51. [HTML Domains](ideas/03_html_domains)
52. [Meta-Domain](ideas/04_meta_domain)
53. [Tooling](ideas/05_tooling)

### Guides

54. [From TypeScript](guides/01_from_typescript)
55. [From Haskell](guides/02_from_haskell)

### Meta

- [TODO](TODO)
- [Open Questions](OPEN_QUESTIONS)

