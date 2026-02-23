#![allow(dead_code)]

use std::collections::HashMap;

use crate::diagnostics::Span;

use super::types::TypeVarId;

#[derive(Clone, Debug)]
pub(super) enum DeferredConstraint {
    Equal(super::types::Type, super::types::Type, Span),
}

#[derive(Default, Clone, Debug)]
pub(super) struct UnionFindVars {
    parent: HashMap<TypeVarId, TypeVarId>,
    rank: HashMap<TypeVarId, u8>,
}

impl UnionFindVars {
    pub(super) fn ensure(&mut self, id: TypeVarId) {
        self.parent.entry(id).or_insert(id);
        self.rank.entry(id).or_insert(0);
    }

    pub(super) fn find(&mut self, id: TypeVarId) -> TypeVarId {
        self.ensure(id);
        let parent = self.parent[&id];
        if parent == id {
            return id;
        }
        let root = self.find(parent);
        self.parent.insert(id, root);
        root
    }

    pub(super) fn union(&mut self, a: TypeVarId, b: TypeVarId) -> TypeVarId {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return ra;
        }
        let rank_a = self.rank[&ra];
        let rank_b = self.rank[&rb];
        if rank_a < rank_b {
            self.parent.insert(ra, rb);
            rb
        } else if rank_a > rank_b {
            self.parent.insert(rb, ra);
            ra
        } else {
            self.parent.insert(rb, ra);
            self.rank.insert(ra, rank_a.saturating_add(1));
            ra
        }
    }
}

#[derive(Default, Clone, Debug)]
pub(super) struct ConstraintState {
    pub(super) vars: UnionFindVars,
    pub(super) deferred: Vec<DeferredConstraint>,
}
