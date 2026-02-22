use crate::rust_ir::RustIrDef;

/// Reuse opportunities discovered by the Perceus-style ownership analysis.
///
/// v0 keeps this conservative: it only tracks definitions that are monomorphic
/// and closed, where backend lowering can later safely add in-place update paths.
#[derive(Debug, Default, Clone)]
pub(super) struct ReusePlan {
    pub(super) reusable_defs: Vec<String>,
}

pub(super) fn analyze_reuse(defs: &[RustIrDef]) -> ReusePlan {
    let reusable_defs = defs
        .iter()
        .filter_map(|def| {
            def.cg_type
                .as_ref()
                .filter(|ty| ty.is_closed())
                .map(|_| def.name.clone())
        })
        .collect();
    ReusePlan { reusable_defs }
}
