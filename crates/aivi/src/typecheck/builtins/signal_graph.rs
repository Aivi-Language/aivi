use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(_checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");

    let signal_ty = Type::con("Signal");
    let spectrum_ty = Type::con("Spectrum");
    let signal_record = Type::Record {
        fields: vec![
            (
                "fft".to_string(),
                Type::Func(Box::new(signal_ty.clone()), Box::new(spectrum_ty.clone())),
            ),
            (
                "ifft".to_string(),
                Type::Func(Box::new(spectrum_ty.clone()), Box::new(signal_ty.clone())),
            ),
            (
                "windowHann".to_string(),
                Type::Func(Box::new(signal_ty.clone()), Box::new(signal_ty.clone())),
            ),
            (
                "normalize".to_string(),
                Type::Func(Box::new(signal_ty.clone()), Box::new(signal_ty.clone())),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("signal".to_string(), Scheme::mono(signal_record));

    let graph_ty = Type::con("Graph");
    let edge_ty = Type::con("Edge");
    let list_node_ty = Type::con("List").app(vec![int_ty.clone()]);
    let graph_record = Type::Record {
        fields: vec![
            (
                "addEdge".to_string(),
                Type::Func(
                    Box::new(graph_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(edge_ty.clone()),
                        Box::new(graph_ty.clone()),
                    )),
                ),
            ),
            (
                "neighbors".to_string(),
                Type::Func(
                    Box::new(graph_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(list_node_ty.clone()),
                    )),
                ),
            ),
            (
                "shortestPath".to_string(),
                Type::Func(
                    Box::new(graph_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(int_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(int_ty.clone()),
                            Box::new(list_node_ty.clone()),
                        )),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("graph".to_string(), Scheme::mono(graph_record));
}
