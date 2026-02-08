# AIVI Language Specification

A high-integrity functional language targeting WebAssembly.

**Version:** 0.1 (Draft)

---

## Table of Contents

### Core Specification

1. [Introduction](01_introduction.md)

### Syntax

2. [Bindings and Scope](02_syntax/01_bindings.md)
3. [Functions and Pipes](02_syntax/02_functions.md)
4. [The Type System](02_syntax/03_types.md)
5. [Predicates](02_syntax/04_predicates.md)
6. [Patching Records](02_syntax/05_patching.md)
7. [Domains, Units, and Deltas](02_syntax/06_domains.md)
8. [Generators](02_syntax/07_generators.md)
9. [Pattern Matching](02_syntax/08_pattern_matching.md)
10. [Effects](02_syntax/09_effects.md)
11. [Modules and External Sources](02_syntax/10_modules.md)
12. [Domain Definitions](02_syntax/11_domain_definition.md)
13. [External Sources](02_syntax/12_external_sources.md)

### Kernel (Core Calculus)

14. [Core Terms](03_kernel/01_core_terms.md)
15. [Types](03_kernel/02_types.md)
16. [Records](03_kernel/03_records.md)
17. [Patterns](03_kernel/04_patterns.md)
18. [Predicates](03_kernel/05_predicates.md)
19. [Traversals](03_kernel/06_traversals.md)
20. [Generators](03_kernel/07_generators.md)
21. [Effects](03_kernel/08_effects.md)
22. [Classes](03_kernel/09_classes.md)
23. [Domains](03_kernel/10_domains.md)
24. [Patching](03_kernel/11_patching.md)
25. [Minimality Proof](03_kernel/12_minimality.md)

### Desugaring (Syntax → Kernel)

26. [Bindings](04_desugaring/01_bindings.md)
27. [Functions](04_desugaring/02_functions.md)
28. [Records](04_desugaring/03_records.md)
29. [Patterns](04_desugaring/04_patterns.md)
30. [Predicates](04_desugaring/05_predicates.md)
31. [Generators](04_desugaring/06_generators.md)
32. [Effects](04_desugaring/07_effects.md)
33. [Classes](04_desugaring/08_classes.md)
34. [Domains and Operators](04_desugaring/09_domains.md)
35. [Patching](04_desugaring/10_patching.md)

### Standard Library

36. [Prelude](05_stdlib/01_prelude.md)
37. [Calendar Domain](05_stdlib/02_calendar.md)
38. [Duration Domain](05_stdlib/03_duration.md)
39. [Color Domain](05_stdlib/04_color.md)
40. [Vector Domain](05_stdlib/05_vector.md)
41. [HTML Domain](05_stdlib/06_html.md)
42. [Style Domain](05_stdlib/07_style.md)
43. [SQLite Domain](05_stdlib/08_sqlite.md)

### Ideas & Future Directions

44. [WASM Target](ideas/01_wasm_target.md)
45. [LiveView Frontend](ideas/02_liveview_frontend.md)
46. [HTML Domains](ideas/03_html_domains.md)
47. [Meta-Domain](ideas/04_meta_domain.md)
48. [Tooling](ideas/05_tooling.md)

### Guides

49. [From TypeScript](guides/01_from_typescript.md)
50. [From Haskell](guides/02_from_haskell.md)

### Meta

- [TODO](TODO.md)
- [Open Questions](OPEN_QUESTIONS.md)

---

## Building the Specification

Generate PDF:

```bash
./build-pdf.sh
```

Requires: `pandoc`, `wkhtmltopdf` or `weasyprint`
