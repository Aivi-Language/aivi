# Guide Topics

The manual now starts from four entry points:

- [Tutorials](/tutorials/) for learning by doing
- [How-to Guides](/how-to/) for solving a concrete problem
- [Reference](/reference/) for exact surface details
- [Explanation](/explanation/) for the design logic behind AIVI

This `/guide/` directory holds the long-form topic pages those sections point to.

## Recommended manual path

| If you want to... | Start here |
| --- | --- |
| Learn AIVI from scratch | [Tutorials](/tutorials/) |
| Build one practical app first | [Build a Small Task Tracker](/guide/your-first-app) |
| Understand the philosophy | [Explanation](/explanation/) |
| Look up exact language details | [Reference](/reference/) |

## Topic map

### Core language

| Page | Description |
| --- | --- |
| [values-and-functions.md](values-and-functions.md) | Values, functions, signatures, and when inference helps |
| [types.md](types.md) | Primitives, records, tagged unions, and aliases |
| [pipes.md](pipes.md) | The pipe operators and how data flows through them |
| [pattern-matching.md](pattern-matching.md) | Exhaustive case analysis with `\|\|>` |
| [record-patterns.md](record-patterns.md) | Record destructuring and projection |
| [predicates.md](predicates.md) | Inline predicates and selectors |
| [domains.md](domains.md) | Semantic wrapper types and domain-specific operations |

### Reactivity and UI

| Page | Description |
| --- | --- |
| [signals.md](signals.md) | Reactive values, merge, accumulation, and derivation |
| [sources.md](sources.md) | The typed boundary to the outside world |
| [source-catalog.md](source-catalog.md) | Conservative reference for built-in `@source` variants |
| [markup.md](markup.md) | GTK/libadwaita markup, control nodes, and widget catalog |

### Abstractions and structure

| Page | Description |
| --- | --- |
| [classes.md](classes.md) | Typeclass-style abstraction with `class` and `instance` |
| [typeclasses.md](typeclasses.md) | Higher-kinded support and executable built-in coverage |
| [class-laws.md](class-laws.md) | Lawfulness and design boundaries |
| [modules.md](modules.md) | Imports, exports, and file layout |

### Examples

| Page | Description |
| --- | --- |
| [building-snake.md](building-snake.md) | Optional deep example that scales the core ideas into a full game |

The standard library reference lives at [/stdlib/](/stdlib/).
