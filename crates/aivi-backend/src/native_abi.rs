//! Physical representation checks shared by backend ABI adaptation and codegen.
//! Callable descriptors may be forwarded only when every nested representation
//! already agrees. Ordinary direct arguments can instead use explicit repacks.

use std::collections::HashSet;

use crate::{AbiPassMode, Layout, LayoutId, LayoutKind, Program};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LayoutCompatibility {
    Repack,
    CallableRepresentation,
}

pub(crate) fn runtime_payload_layout(program: &Program, mut layout: LayoutId) -> Option<LayoutId> {
    for _ in 0..program.layouts().len() {
        match &program.layouts().get(layout)?.kind {
            LayoutKind::Signal { element } => layout = *element,
            _ => return Some(layout),
        }
    }
    None
}

pub(crate) fn layouts_compatible(
    program: &Program,
    expected: LayoutId,
    found: LayoutId,
    mode: LayoutCompatibility,
) -> bool {
    if expected == found {
        return true;
    }
    let mut pending = vec![(expected, found, mode)];
    let mut visited = HashSet::new();
    while let Some((expected, found, mode)) = pending.pop() {
        let (Some(expected), Some(found)) = (
            runtime_payload_layout(program, expected),
            runtime_payload_layout(program, found),
        ) else {
            return false;
        };
        if expected == found || !visited.insert((expected, found, mode)) {
            continue;
        }
        let left = &program.layouts()[expected];
        let right = &program.layouts()[found];
        if matches!(left.kind, LayoutKind::Domain { .. })
            || matches!(right.kind, LayoutKind::Domain { .. })
        {
            if mode == LayoutCompatibility::Repack {
                continue;
            }
            if left.abi != AbiPassMode::ByReference || right.abi != AbiPassMode::ByReference {
                return false;
            }
            let carrier = |id, layout: &Layout| match &layout.kind {
                LayoutKind::Domain { .. } => program.named_domain_carrier(id),
                _ => Some(id),
            };
            let (Some(left), Some(right)) = (carrier(expected, left), carrier(found, right)) else {
                // An erased generic domain is an uninterpreted reference.
                continue;
            };
            if left != expected || right != found {
                pending.push((left, right, mode));
                continue;
            }
            return false;
        }
        if left.abi != right.abi {
            return false;
        }
        match (&left.kind, &right.kind) {
            (LayoutKind::Primitive(left), LayoutKind::Primitive(right)) if left == right => {}
            (LayoutKind::Tuple(left), LayoutKind::Tuple(right)) if left.len() == right.len() => {
                pending.extend(
                    left.iter()
                        .zip(right)
                        .map(|(left, right)| (*left, *right, mode)),
                )
            }
            (LayoutKind::Record(left), LayoutKind::Record(right))
                if left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(left, right)| left.name == right.name) =>
            {
                pending.extend(
                    left.iter()
                        .zip(right)
                        .map(|(left, right)| (left.layout, right.layout, mode)),
                )
            }
            (
                LayoutKind::Arrow {
                    parameter: left_parameter,
                    result: left_result,
                },
                LayoutKind::Arrow {
                    parameter: right_parameter,
                    result: right_result,
                },
            ) => {
                pending.push((
                    *left_parameter,
                    *right_parameter,
                    LayoutCompatibility::CallableRepresentation,
                ));
                pending.push((
                    *left_result,
                    *right_result,
                    LayoutCompatibility::CallableRepresentation,
                ));
            }
            (LayoutKind::List { element: left }, LayoutKind::List { element: right })
            | (LayoutKind::Set { element: left }, LayoutKind::Set { element: right })
            | (LayoutKind::Option { element: left }, LayoutKind::Option { element: right })
            | (
                LayoutKind::AnonymousDomain { carrier: left, .. },
                LayoutKind::AnonymousDomain { carrier: right, .. },
            ) => pending.push((*left, *right, mode)),
            (
                LayoutKind::Map {
                    key: left_key,
                    value: left_value,
                },
                LayoutKind::Map {
                    key: right_key,
                    value: right_value,
                },
            )
            | (
                LayoutKind::Result {
                    error: left_key,
                    value: left_value,
                },
                LayoutKind::Result {
                    error: right_key,
                    value: right_value,
                },
            )
            | (
                LayoutKind::Validation {
                    error: left_key,
                    value: left_value,
                },
                LayoutKind::Validation {
                    error: right_key,
                    value: right_value,
                },
            )
            | (
                LayoutKind::Task {
                    error: left_key,
                    value: left_value,
                },
                LayoutKind::Task {
                    error: right_key,
                    value: right_value,
                },
            ) => {
                pending.push((*left_key, *right_key, mode));
                pending.push((*left_value, *right_value, mode));
            }
            (LayoutKind::Sum(left), LayoutKind::Sum(right)) => {
                if !append_compatible_variants(left, right, mode, &mut pending) {
                    return false;
                }
            }
            (
                LayoutKind::Opaque {
                    item: left_item,
                    name: left_name,
                    variants: left,
                    ..
                },
                LayoutKind::Opaque {
                    item: right_item,
                    name: right_name,
                    variants: right,
                    ..
                },
            ) if opaque_layout_identity_matches(*left_item, left_name, *right_item, right_name) => {
                if !left.is_empty()
                    && !right.is_empty()
                    && !append_compatible_variants(left, right, mode, &mut pending)
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

fn append_compatible_variants(
    left: &[crate::VariantLayout],
    right: &[crate::VariantLayout],
    mode: LayoutCompatibility,
    pending: &mut Vec<(LayoutId, LayoutId, LayoutCompatibility)>,
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for (left, right) in left.iter().zip(right) {
        if left.name != right.name || left.field_count != right.field_count {
            return false;
        }
        match (left.payload, right.payload) {
            (None, None) => {}
            (Some(left), Some(right)) => pending.push((left, right, mode)),
            _ => return false,
        }
    }
    true
}

pub(crate) fn opaque_layout_identity_matches(
    left_item: Option<aivi_hir::ItemId>,
    left_name: &str,
    right_item: Option<aivi_hir::ItemId>,
    right_name: &str,
) -> bool {
    match (left_item, right_item) {
        (Some(left_item), Some(right_item)) => left_item == right_item,
        _ => left_name == right_name,
    }
}
