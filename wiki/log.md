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

## [2026-04-06] add | AsyncTracker signal lifecycle tracker

**Trigger**: user noted `sig.done`, `sig.error`, `sig.pending`, `sig.do once` were planned but missing.

**Finding**: No implementation existed anywhere — not in RFC, stdlib, HIR, runtime, or manual.

**Implemented**:
- `stdlib/aivi/async.aivi` — `AsyncTracker E A` record type + `step` accumulation function + `isPending`, `isDone`, `isFailed` helpers
- `manual/stdlib/async.md` — full reference page including fire-once idiom
- `manual/guide/signals.md` — "Tracking async state" section: tracker pattern + `sig.pending/done/error` projections + fire-once accumulation idiom
- `manual/stdlib/index.md` — added `aivi.async` to at-a-glance table
- `manual/.vitepress/navigation.ts` — added Async Tracker to stdlib "Core Values & Collections" section
- `wiki/signal-model.md` — AsyncTracker pattern documented

**Design decisions**:
- Tracker fields are `pending/done/error` (not `loading/value/error`) to match user's stated names
- `done` preserves last successful value on subsequent errors (stale-while-revalidate)
- `do once` documented as accumulation idiom; dedicated `@effect`/`doOnce` noted as planned
- No compiler changes needed — pure stdlib + documentation
