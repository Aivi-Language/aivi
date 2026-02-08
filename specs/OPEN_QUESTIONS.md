# Open Questions

## Language Features

1.  **Concurrency Model**:
    *   Should fibres be managed manually (`spawn`) or structurally (`race`, `par`)?
    *   How do channels/mailboxes fit into the type system?

2.  **Effect System**:
    *   Is `Effect ε A` enough? Do we need Algebraic Effects (handlers)?
    *   How to handle resource cleanup (bracket/defer)?

3.  **Generators**:
    *   Are they always synchronous? How to handle `AsyncGenerator`?
    *   Should `generate` block allow `await`?

## Syntax

1.  **Pipe Operator**:
    *   Is `|>` sufficient, or do we need a "bind pipe" for monads (`>>=`)?

2.  **String Interpolation**:
    *   Allows arbitrary expressions? `{x + 1}`? Or just variables?

## Ecosystem

1.  **Package Management**:
    *   Central registry vs decentralized git URLs?
    *   Version constraints solver?

2.  **FFI**:
    *   How to call JS/C functions safely?
    *   Auto-generation of bindings?

## Domains

1.  **Delta Literal Collision**:
    *   If two imported domains define `1m` (months vs meters), how to disambiguate?
    *   Options: qualified literals (`Calendar.1m`), carrier-type inference, or import renaming.

2.  **Domain Extension**:
    *   Should `domain RichCalendar extends Calendar` be supported?
    *   How would overriding operators work?

3.  **Runtime vs Static Domains**:
    *   Are domains always static (compile-time rewrite rules)?
    *   Could domains ever be first-class values for metaprogramming?

4.  **Multi-Carrier Domains**:
    *   Can one domain span multiple carrier types (e.g., `Vec2 | Vec3 | Vec4`)?
    *   How does operator resolution work when carriers share operators?

