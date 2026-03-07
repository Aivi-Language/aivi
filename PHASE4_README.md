# AIVI Phase 4 Reactive Implementation — Complete Map

This directory contains a comprehensive implementation map for **Phase 4 Reactive Dataflow**, the next major milestone for AIVI's language and runtime.

## Quick Navigation

| Document | Purpose | Size | Audience |
|----------|---------|------|----------|
| **[PHASE4_REACTIVE_IMPLEMENTATION_MAP.md](./PHASE4_REACTIVE_IMPLEMENTATION_MAP.md)** | Complete spec with all details, file-by-file changes, test examples, and deployment checklist | 484 lines | Implementers, architects |
| **[PHASE4_QUICK_REFERENCE.txt](./PHASE4_QUICK_REFERENCE.txt)** | Quick lookup: spec locations, API names, integration points, effort estimates | 107 lines | Developers, team leads |
| **[PHASE4_ARCHITECTURE_DIAGRAM.txt](./PHASE4_ARCHITECTURE_DIAGRAM.txt)** | Visual data flow, registry structure, invalidation algorithm, turn semantics | 295 lines | Designers, code reviewers |

## Summary

Phase 4 introduces **pure memoized computed signals** on top of the existing `gtkApp` architecture:

- **Spec**: `specs/stdlib/ui/reactive_dataflow.md` ✓ (159 lines, fully specified)
- **API**: `computed :: Text -> (Model -> a) -> Signal a` and `readSignal :: Signal a -> Model -> a`
- **Integration**: Two new event loop steps (9: invalidate, 10: lazy reevaluate)
- **Scope**: ~400–640 LOC implementation, ~60–120 LOC testing/demo
- **Effort**: 3–4 weeks with 1–2 engineers

## What Phase 4 Delivers

✅ **Pure reactive signals** with stable identity and memoization  
✅ **Revision-based invalidation** for efficient dependency tracking  
✅ **Turn-local caching** — reuses computed values across multiple reads within single app turn  
✅ **Subscription integration** — signals can decide which subscriptions exist  
✅ **Zero hidden mutation** — preserves explicit effects model  
✅ **Backward compatible** — no changes to existing `gtkApp` API  

## Files to Modify (6 total)

1. **`crates/aivi/src/stdlib/gtk4.rs`** (~50–100 LOC)  
   Add: Signal type, computed, signal, readSignal exports

2. **`crates/aivi/src/typecheck/builtins/system_db.rs`** (~20–40 LOC)  
   Add: Signal type and function signatures (no capability requirements)

3. **`crates/aivi/src/runtime/builtins/gtk4_real.rs`** (~250–400 LOC) ⭐ PRIMARY  
   Add: SignalRegistry, invalidateAndMarkDirty, loop integration, dependency tracking

4. **`crates/aivi/src/runtime/builtins/gtk4.rs`** (~100–150 LOC)  
   Add: builtin_computed, builtin_readSignal registrations

5. **`integration-tests/stdlib/aivi/ui/gtk4/gtk4.aivi`** (~50–100 LOC)  
   Add: Comprehensive tests for memoization, invalidation, cascades

6. **`demos/snake.aivi`** (~10–20 LOC)  
   Update: Extract gridRows, scoreText to computed signals

## Key Design Boundaries

| Concept | Allowed | Not Allowed |
|---------|---------|-------------|
| **`view` reads signals** | ✓ `view = state => readSignal sig state` | ❌ Update cannot read signals |
| **`subscriptions` read signals** | ✓ For deciding membership: `if readSignal check state` | ❌ Not to keep alive |
| **Effects read signals** | ❌ Effects must capture plain Model snapshots | ✓ `effect = state => doWork state.field` |
| **Signal purity** | ✓ Pure function, no Effect/Resource/Msg | ❌ No side effects |

## Integration in Event Loop

The `gtkApp` event loop gains two new steps after `update` commits the model:

```
8. commit new model
9. [NEW] assign fresh revisions & invalidate computed signals
   └─ invalidateAndMarkDirty(old_model, new_model, registry)
10. [NEW] evaluate view; dirty signals recalculate lazily on first read
    └─ view calls readSignal(sig, model); triggers reevaluation if dirty
11. reconcile widget tree
12. sync subscriptions
13. launch commands
```

## Example Usage

```aivi
-- Define expensive derived value as computed signal with stable key
visibleRows = computed "visibleRows" (state =>
  state.rows
    |> filter (matchesQuery state.query)
    |> filter (matchesTags state.selectedTags)
)

-- Use in view; reads from cache if not invalidated
view : State -> GtkNode
view = state =>
  ~<gtk>
    <each items={readSignal visibleRows state} as={row}>
      <GtkLabel label={row.name} />
    </each>
  </gtk>

-- Use in subscriptions to decide membership
subscriptions : State -> List (Subscription Msg)
subscriptions = state => [
  subscriptionEvery { key: "poll", millis: 1000, tag: Poll }
  if readSignal (computed "shouldPoll" (_ => state.count > 0)) state
]
```

## Risk & Effort Profile

| Area | Difficulty | Notes |
|------|------------|-------|
| **Dependency tracking** | 🔴 HIGH | Novel; requires tracking all field/signal reads during function evaluation |
| **Invalidation propagation** | 🔴 HIGH | Must correctly traverse signal→signal dependencies and detect cycles |
| **Event loop integration** | 🟡 MEDIUM | Careful state threading; minimal touching of core loop |
| **Public API & typecheck** | 🟢 LOW | Straightforward additions to stdlib and type system |

**Recommendation**: Start with proof-of-concept for dependency tracking before full integration.

## Success Criteria

1. Computed signals memoize within a single app turn (no re-evaluation on repeated reads)
2. Invalidation propagates correctly after model changes (dirty marking reaches all dependents)
3. Lazy reevaluation on first read of dirty signals
4. View and subscriptions can read computed signals; effects cannot
5. Cycle detection catches invalid signal definitions
6. Snake demo runs with 2–3 extracted computed signals
7. All existing tests pass (no regressions)

## Next Phases (Out of Scope)

- **Phase 4b**: Signal→signal cascades, equality-based short-circuiting
- **Phase 5+**: Async computed signals, signal groups, query composition

## Related Documentation

- **Spec**: `specs/stdlib/ui/reactive_dataflow.md` — Normative semantic model
- **Architecture**: `specs/stdlib/ui/app_architecture.md` — Event loop integration points (lines 95–99)
- **Forms**: `specs/stdlib/ui/forms.md` — Interaction with form field state (lines 140–148)
- **Roadmap**: `plan.md` — Phase 4 in broader context (lines 129–157)

## Implementation Checklist

### Compiler
- [ ] Add `Signal a` type to system_db
- [ ] Add `computed`, `signal`, `readSignal` function signatures
- [ ] Ensure no capability requirements
- [ ] Error on suspicious patterns (Effect inside computed)

### Runtime
- [ ] Implement `SignalRegistry` with revision tracking
- [ ] Implement `invalidateAndMarkDirty` with cycle detection
- [ ] Integrate into gtkApp loop (step 9)
- [ ] Implement dependency tracking context
- [ ] Implement `builtin_computed`, `builtin_readSignal`

### Testing
- [ ] Memoization within single turn
- [ ] Invalidation on source change
- [ ] Transitive dirty propagation
- [ ] Signal→signal dependencies
- [ ] Subscriptions using signals
- [ ] Cycle detection
- [ ] Snake demo updated

### Documentation
- [ ] Update app_architecture.md with new steps 9–10
- [ ] Add examples to gtk4.md
- [ ] LSP hover docs for signal operations

---

**Status**: Specification complete (Phase 4 model specified); runtime implementation pending.

**Next Step**: Begin implementation with proof-of-concept for dependency tracking.

For full details, see [PHASE4_REACTIVE_IMPLEMENTATION_MAP.md](./PHASE4_REACTIVE_IMPLEMENTATION_MAP.md).
