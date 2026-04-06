# Log

Append-only chronological record of wiki activity.  
Parse with: `grep "^## \[" log.md | tail -10`

---

## [2026-04-06] ingest | Initial wiki seeded from codebase

Seeded wiki from source files in `src/`, `crates/`, `manual/`, `stdlib/`, `syntax.md`, `AIVI_RFC.md`.  
Pages created: architecture, compiler-pipeline, type-system, signal-model, runtime, gtk-bridge, query-layer, lsp-server, cli, stdlib.  
Sources read: all `crates/*/src/lib.rs` files, `AGENTS.md`, `manual/guide/*.md` listing.

## [2026-04-06] query | Snake & Reversi demo audit

Read `demos/snake.aivi` and `demos/reversi.aivi`. Found and fixed 8 issues across both files. Created [demo-audit.md](demo-audit.md).  
Key fixes: domain abstraction leak in snake; three manual full-record constructions in reversi (should use `<|`); reversi timer missing unit; dead `Candidate.flips` field.

## [2026-04-06] query | Manual structure improvements

Restructured `manual/guide/README.md` into five story arcs (Functional programming → Pipe algebra → Domains → Signals & reactivity → GTK & markup) plus an External integrations arc.  
Created `manual/guide/integrations.md` — the missing integration patterns page (HTTP, timers, filesystem, database, D-Bus, custom providers, tips).  
Added sidecar note to `manual/stdlib/index.md`.  
Key insight: the old guide was a feature inventory; the new structure tells a learning journey.
