# AIVI Language Specification

A high-integrity functional language targeting WebAssembly.

**Version:** 0.1 (Draft)

---

## Table of Contents

### Core Specification

1. [Introduction](01_introduction.md)

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
15. [JSX Literals](02_syntax/13_jsx_literals.md)
16. [Decorators](02_syntax/14_decorators.md)
17. [Resources](02_syntax/15_resources.md)

### Kernel (Core Calculus)

18. [Core Terms](03_kernel/01_core_terms.md)
19. [Types](03_kernel/02_types.md)
20. [Records](03_kernel/03_records.md)
21. [Patterns](03_kernel/04_patterns.md)
22. [Predicates](03_kernel/05_predicates.md)
23. [Traversals](03_kernel/06_traversals.md)
24. [Generators](03_kernel/07_generators.md)
25. [Effects](03_kernel/08_effects.md)
26. [Classes](03_kernel/09_classes.md)
27. [Domains](03_kernel/10_domains.md)
28. [Patching](03_kernel/11_patching.md)
29. [Minimality Proof](03_kernel/12_minimality.md)

### Desugaring (Syntax → Kernel)

30. [Bindings](04_desugaring/01_bindings.md)
31. [Functions](04_desugaring/02_functions.md)
32. [Records](04_desugaring/03_records.md)
33. [Patterns](04_desugaring/04_patterns.md)
34. [Predicates](04_desugaring/05_predicates.md)
35. [Generators](04_desugaring/06_generators.md)
36. [Effects](04_desugaring/07_effects.md)
37. [Classes](04_desugaring/08_classes.md)
38. [Domains and Operators](04_desugaring/09_domains.md)
39. [Patching](04_desugaring/10_patching.md)

### Standard Library

40. [Prelude](05_stdlib/01_prelude.md)
41. [Calendar Domain](05_stdlib/02_calendar.md)
42. [Duration Domain](05_stdlib/03_duration.md)
43. [Color Domain](05_stdlib/04_color.md)
44. [Vector Domain](05_stdlib/05_vector.md)
45. [HTML Domain](05_stdlib/06_html.md)
46. [Style Domain](05_stdlib/07_style.md)
47. [SQLite Domain](05_stdlib/08_sqlite.md)

### Ideas & Future Directions

48. [WASM Target](ideas/01_wasm_target.md)
49. [LiveView Frontend](ideas/02_liveview_frontend.md)
50. [HTML Domains](ideas/03_html_domains.md)
51. [Meta-Domain](ideas/04_meta_domain.md)
52. [Tooling](ideas/05_tooling.md)

### Guides

53. [From TypeScript](guides/01_from_typescript.md)
54. [From Haskell](guides/02_from_haskell.md)

### Meta

- [TODO](TODO.md)
- [Open Questions](OPEN_QUESTIONS.md)

---

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

### Legacy (pandoc)

```bash
./build.sh
```

Requires: `pandoc`, `python3`
