# Outstanding implementation gaps

- Eq constraints are collected but still do not go through a dedicated solver pass.
  - Evidence: `crates/aivi-hir/src/typecheck.rs` explicitly warns that collected `Eq` constraints can otherwise remain unsolved.

- Higher-kinded user-authored class and instance support is still only partial end to end.
  - Evidence: ambient prelude declarations exist, but public `aivi check` still rejects examples like `instance Applicative Option` and class members shaped like `F Int`.

- Record-default evidence is still narrower than the RFC originally claimed.
  - Evidence: `use aivi.defaults (Option)` is a compiler-recognized special path, while other `Default` support is limited to same-module instances rather than general imported evidence.

- Ambient `Monad` / `Chain` declarations still do not have end-to-end builtin lowering support.
  - Evidence: the executable carrier list includes `Functor`, `Apply`, `Applicative`, `Foldable`, `Bifunctor`, `Traversable`, and `Filterable`, but not `Monad` / `Chain`.

- `&|>` applicative clusters are still HIR/typechecker-only for many executable paths.
  - Evidence: valid cluster surfaces type-check, but typed-core lowering still rejects general executable use sites.

- RFC source-option claims are partially outdated.
  - Evidence: `timer.jitterMs` is stale in favor of `jitter : Duration`, and `fs.read.activeWhen` is not a current option.
