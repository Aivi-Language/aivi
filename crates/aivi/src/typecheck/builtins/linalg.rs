use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(_checker: &mut TypeChecker, env: &mut TypeEnv) {
    let float_ty = Type::con("Float");

    let vec_ty = Type::con("Vec");
    let mat_ty = Type::con("Mat");
    let linalg_record = Type::Record {
        fields: vec![
            (
                "addVec".to_string(),
                Type::Func(
                    Box::new(vec_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(vec_ty.clone()),
                        Box::new(vec_ty.clone()),
                    )),
                ),
            ),
            (
                "subVec".to_string(),
                Type::Func(
                    Box::new(vec_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(vec_ty.clone()),
                        Box::new(vec_ty.clone()),
                    )),
                ),
            ),
            (
                "scaleVec".to_string(),
                Type::Func(
                    Box::new(vec_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(float_ty.clone()),
                        Box::new(vec_ty.clone()),
                    )),
                ),
            ),
            (
                "dot".to_string(),
                Type::Func(
                    Box::new(vec_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(vec_ty.clone()),
                        Box::new(float_ty.clone()),
                    )),
                ),
            ),
            (
                "matMul".to_string(),
                Type::Func(
                    Box::new(mat_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(mat_ty.clone()),
                        Box::new(mat_ty.clone()),
                    )),
                ),
            ),
            (
                "solve2x2".to_string(),
                Type::Func(
                    Box::new(mat_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(vec_ty.clone()),
                        Box::new(vec_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    env.insert("linalg".to_string(), Scheme::mono(linalg_record));
}
