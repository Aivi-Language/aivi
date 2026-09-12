use std::sync::OnceLock;

use crate::Module;

/// Immutable preparation shared by the independent HIR elaboration passes.
///
/// Default-field inference and synthesis run once, on first use. All passes see the same
/// synthesized expression IDs and source spans. A session is bound to one immutable source
/// module; it cannot survive an edit or be reused for a different workspace module.
pub struct ElaborationSession<'a> {
    source: &'a Module,
    prepared: OnceLock<Module>,
}

impl<'a> ElaborationSession<'a> {
    pub fn new(source: &'a Module) -> Self {
        Self {
            source,
            prepared: OnceLock::new(),
        }
    }

    fn prepared(&self) -> &Module {
        self.prepared
            .get_or_init(|| crate::typecheck::elaborate_default_record_fields(self.source))
    }

    pub fn elaborate_general_expressions(&self) -> crate::GeneralExprElaborationReport {
        crate::general_expr_elaboration::elaborate_general_expressions_prepared(self.prepared())
    }

    pub fn elaborate_ambient_items(&self) -> crate::GeneralExprElaborationReport {
        crate::general_expr_elaboration::elaborate_ambient_items_prepared(self.prepared())
    }

    pub fn elaborate_gates(&self) -> crate::GateElaborationReport {
        crate::gate_elaboration::elaborate_gates_prepared(self.prepared())
    }

    pub fn elaborate_fanouts(&self) -> crate::FanoutElaborationReport {
        crate::fanout_elaboration::elaborate_fanouts_prepared(self.prepared())
    }

    pub fn elaborate_truthy_falsy(&self) -> crate::TruthyFalsyElaborationReport {
        crate::truthy_falsy_elaboration::elaborate_truthy_falsy_prepared(self.prepared())
    }

    pub fn elaborate_temporal_stages(&self) -> crate::TemporalElaborationReport {
        crate::temporal_elaboration::elaborate_temporal_stages_prepared(self.prepared())
    }

    pub fn elaborate_recurrences(&self) -> crate::RecurrenceElaborationReport {
        crate::recurrence_elaboration::elaborate_recurrences_prepared(self.prepared())
    }

    pub fn elaborate_source_lifecycles(&self) -> crate::SourceLifecycleElaborationReport {
        crate::source_lifecycle_elaboration::elaborate_source_lifecycles_prepared(self.prepared())
    }

    pub fn elaborate_source_decodes(&self) -> crate::SourceDecodeElaborationReport {
        crate::decode_elaboration::elaborate_source_decodes_prepared(self.prepared())
    }

    pub fn generate_source_decode_programs(&self) -> crate::SourceDecodeProgramReport {
        crate::decode_generation::generate_source_decode_programs_from_report(
            self.source,
            self.elaborate_source_decodes(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_share_default_field_synthesis_without_changing_source() {
        let mut sources = aivi_base::SourceDatabase::new();
        let file = sources.add_file("session.aivi", "use aivi.defaults (Option)\ntype Profile = { name: Text, bio: Option Text }\nvalue profile:Profile = { name: \"Ada\" }\n");
        let parsed = aivi_syntax::parse_module(&sources[file]);
        assert!(!parsed.has_errors());
        let lowered = crate::lower_module(&parsed.module);
        assert!(!lowered.has_errors());
        let source = lowered.module();
        let count = source.exprs().len();
        let session = ElaborationSession::new(source);
        assert!(session.prepared.get().is_none());
        assert_eq!(
            session.elaborate_general_expressions(),
            crate::elaborate_general_expressions(source)
        );
        let prepared = session.prepared();
        assert!(
            prepared.exprs().len() > count,
            "the omitted Option field must be synthesized"
        );
        assert_eq!(
            session.elaborate_ambient_items(),
            crate::elaborate_ambient_items(source)
        );
        assert_eq!(session.elaborate_gates(), crate::elaborate_gates(source));
        assert_eq!(
            session.elaborate_fanouts(),
            crate::elaborate_fanouts(source)
        );
        assert_eq!(
            session.elaborate_truthy_falsy(),
            crate::elaborate_truthy_falsy(source)
        );
        assert_eq!(
            session.elaborate_temporal_stages(),
            crate::elaborate_temporal_stages(source)
        );
        assert_eq!(
            session.elaborate_recurrences(),
            crate::elaborate_recurrences(source)
        );
        assert_eq!(
            session.elaborate_source_lifecycles(),
            crate::elaborate_source_lifecycles(source)
        );
        assert_eq!(
            session.generate_source_decode_programs(),
            crate::generate_source_decode_programs(source)
        );
        assert!(std::ptr::eq(prepared, session.prepared()));
        assert_eq!(source.exprs().len(), count);
    }
}
