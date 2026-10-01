//! Flat, immutable structural dictionaries. Edges and evidence slots have
//! plan-local identities; recursive nominal shapes use back edges. A plan owns
//! its closed field/constructor schema and can be shared across compiler stages
//! and worker snapshots without borrowing a module or relying on addresses.
//!
//! The enclosing callable type supplies each evidence slot's two operand types
//! and Bool result. Every IR must additionally validate this shape against that
//! type (or its closed layouts). Debug prints use node/slot IDs and source names.

use std::fmt;

aivi_base::define_arena_id!(pub EqualityNodeId);
aivi_base::define_arena_id!(pub EqualityEvidenceId);

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EqualityShape {
    root: EqualityNodeId,
    nodes: Vec<EqualityShapeNode>,
    evidence_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EqualityShapeNode {
    /// An entire component proved to use compiler-owned structural equality.
    Structural,
    Evidence(EqualityEvidenceId),
    Carrier(EqualityNodeId),
    Tuple(Vec<EqualityNodeId>),
    Record(Vec<EqualityRecordField>),
    Sum(Vec<EqualitySumVariant>),
    List(EqualityNodeId),
    Option(EqualityNodeId),
    Result {
        error: EqualityNodeId,
        value: EqualityNodeId,
    },
    Validation {
        error: EqualityNodeId,
        value: EqualityNodeId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EqualityRecordField {
    pub name: Box<str>,
    pub node: EqualityNodeId,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EqualitySumVariant {
    pub name: Box<str>,
    pub fields: Vec<EqualityNodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EqualityShapeError(pub &'static str);

impl fmt::Display for EqualityShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for EqualityShapeError {}

impl EqualityShape {
    pub fn new(
        root: EqualityNodeId,
        nodes: Vec<EqualityShapeNode>,
        evidence_count: usize,
    ) -> Result<Self, EqualityShapeError> {
        let shape = Self {
            root,
            nodes,
            evidence_count,
        };
        shape.validate()?;
        Ok(shape)
    }

    pub fn root(&self) -> EqualityNodeId {
        self.root
    }
    pub fn nodes(&self) -> &[EqualityShapeNode] {
        &self.nodes
    }
    pub fn evidence_count(&self) -> usize {
        self.evidence_count
    }
    pub fn node(&self, id: EqualityNodeId) -> Option<&EqualityShapeNode> {
        self.nodes.get(id.as_raw() as usize)
    }

    /// Validate against an IR's operand types. `children` checks the node's
    /// representation and returns payload types in edge order. A nominal type
    /// may defer its representation until the stage that closes its layout.
    /// Evidence signatures are always checked, including deferred payloads.
    pub fn validate_operands<T: Clone + Eq + std::hash::Hash>(
        &self,
        operand: T,
        evidence: &[(T, T, bool)],
        mut children: impl FnMut(&EqualityShapeNode, &T) -> Result<Option<Vec<T>>, EqualityShapeError>,
    ) -> Result<(), EqualityShapeError> {
        self.validate()?;
        if evidence.len() != self.evidence_count
            || evidence
                .iter()
                .any(|(left, right, boolean)| left != right || !boolean)
        {
            return Err(EqualityShapeError(
                "equality evidence requires two matching operands and Bool",
            ));
        }
        let mut visited = std::collections::HashSet::new();
        let mut pending = vec![(self.root, operand)];
        while let Some((id, ty)) = pending.pop() {
            if !visited.insert((id, ty.clone())) {
                continue;
            }
            if visited.len() > 4096 {
                return Err(EqualityShapeError(
                    "typed equality shape exceeds its resource limit",
                ));
            }
            let node = &self.nodes[id.as_raw() as usize];
            if let EqualityShapeNode::Evidence(slot) = node {
                if evidence[slot.as_raw() as usize].0 != ty {
                    return Err(EqualityShapeError(
                        "equality evidence operand differs from its payload",
                    ));
                }
                continue;
            }
            let Some(types) = children(node, &ty)? else {
                continue;
            };
            let edges = match node {
                EqualityShapeNode::Structural | EqualityShapeNode::Evidence(_) => Vec::new(),
                EqualityShapeNode::Carrier(id)
                | EqualityShapeNode::List(id)
                | EqualityShapeNode::Option(id) => vec![*id],
                EqualityShapeNode::Tuple(fields) => fields.clone(),
                EqualityShapeNode::Record(fields) => {
                    fields.iter().map(|field| field.node).collect()
                }
                EqualityShapeNode::Sum(variants) => variants
                    .iter()
                    .flat_map(|variant| variant.fields.iter().copied())
                    .collect(),
                EqualityShapeNode::Result { error, value }
                | EqualityShapeNode::Validation { error, value } => vec![*error, *value],
            };
            if edges.len() != types.len() {
                return Err(EqualityShapeError(
                    "equality representation has a different payload arity",
                ));
            }
            pending.extend(edges.into_iter().zip(types));
        }
        Ok(())
    }

    /// Validate decoded data as well as generated plans. Cycles are allowed;
    /// the worklist visits each node once and validates all reachable slots.
    pub fn validate(&self) -> Result<(), EqualityShapeError> {
        if self.nodes.is_empty() || self.nodes.len() > 4096 || self.evidence_count > 4096 {
            return Err(EqualityShapeError(
                "equality shape exceeds its node or evidence limit",
            ));
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut slots = vec![false; self.evidence_count];
        let mut pending = vec![self.root];
        while let Some(id) = pending.pop() {
            let index = id.as_raw() as usize;
            let Some(node) = self.nodes.get(index) else {
                return Err(EqualityShapeError(
                    "equality shape contains an invalid node reference",
                ));
            };
            if std::mem::replace(&mut seen[index], true) {
                continue;
            }
            match node {
                EqualityShapeNode::Structural => {}
                EqualityShapeNode::Evidence(id) => {
                    let Some(slot) = slots.get_mut(id.as_raw() as usize) else {
                        return Err(EqualityShapeError(
                            "equality shape contains an invalid evidence slot",
                        ));
                    };
                    *slot = true;
                }
                EqualityShapeNode::Carrier(id)
                | EqualityShapeNode::List(id)
                | EqualityShapeNode::Option(id) => pending.push(*id),
                EqualityShapeNode::Tuple(fields) => pending.extend(fields.iter().copied()),
                EqualityShapeNode::Record(fields) => {
                    let mut names = std::collections::HashSet::new();
                    for field in fields {
                        if !names.insert(field.name.as_ref()) {
                            return Err(EqualityShapeError("equality record repeats a field"));
                        }
                        pending.push(field.node);
                    }
                }
                EqualityShapeNode::Sum(variants) => {
                    let mut names = std::collections::HashSet::new();
                    for variant in variants {
                        if !names.insert(variant.name.as_ref()) {
                            return Err(EqualityShapeError("equality sum repeats a constructor"));
                        }
                        pending.extend(variant.fields.iter().copied());
                    }
                }
                EqualityShapeNode::Result { error, value }
                | EqualityShapeNode::Validation { error, value } => {
                    pending.extend([*error, *value])
                }
            }
        }
        if seen.iter().any(|seen| !seen) || slots.iter().any(|seen| !seen) {
            return Err(EqualityShapeError(
                "equality shape contains unreachable nodes or unused evidence slots",
            ));
        }
        // A cycle must descend through data. Alias/domain forwarding alone
        // would revisit the same value forever even for a finite input.
        let mut carriers = vec![0_u8; self.nodes.len()];
        for start in 0..self.nodes.len() {
            let mut path = Vec::new();
            let mut next = start;
            while let EqualityShapeNode::Carrier(child) = &self.nodes[next] {
                match carriers[next] {
                    1 => {
                        return Err(EqualityShapeError(
                            "equality shape contains a nonproductive carrier cycle",
                        ));
                    }
                    2 => break,
                    _ => {}
                }
                carriers[next] = 1;
                path.push(next);
                next = child.as_raw() as usize;
            }
            for index in path {
                carriers[index] = 2;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_validation_checks_references_and_accepts_nominal_back_edges() {
        let root = EqualityNodeId::from_raw(0);
        let leaf = EqualityNodeId::from_raw(1);
        let shape = EqualityShape::new(
            root,
            vec![
                EqualityShapeNode::Sum(vec![
                    EqualitySumVariant {
                        name: "End".into(),
                        fields: vec![leaf],
                    },
                    EqualitySumVariant {
                        name: "Next".into(),
                        fields: vec![root],
                    },
                ]),
                EqualityShapeNode::Evidence(EqualityEvidenceId::from_raw(0)),
            ],
            1,
        )
        .unwrap();
        assert!(shape.validate().is_ok());
        assert!(EqualityShape::new(root, vec![EqualityShapeNode::List(leaf)], 0).is_err());
        assert!(
            EqualityShape::new(
                root,
                vec![EqualityShapeNode::Evidence(EqualityEvidenceId::from_raw(0))],
                0
            )
            .is_err()
        );
        assert!(EqualityShape::new(root, vec![EqualityShapeNode::Structural], 1).is_err());
        assert!(EqualityShape::new(root, vec![EqualityShapeNode::Carrier(root)], 0).is_err());
    }
}
