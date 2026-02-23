use std::collections::BTreeSet;

use super::checker::TypeChecker;
use super::types::Type;

fn collect_type_constructors(ty: &Type, out: &mut BTreeSet<String>) {
    match ty {
        Type::Var(_) => {}
        Type::Con(name, args) => {
            out.insert(name.clone());
            for arg in args {
                collect_type_constructors(arg, out);
            }
        }
        Type::App(head, args) => {
            collect_type_constructors(head, out);
            for arg in args {
                collect_type_constructors(arg, out);
            }
        }
        Type::Func(a, b) => {
            collect_type_constructors(a, out);
            collect_type_constructors(b, out);
        }
        Type::Tuple(items) => {
            for item in items {
                collect_type_constructors(item, out);
            }
        }
        Type::Record { fields } => {
            for field_ty in fields.values() {
                collect_type_constructors(field_ty, out);
            }
        }
    }
}

#[test]
fn builtin_values_only_use_registered_type_constructors() {
    let checker = TypeChecker::new();

    let mut referenced = BTreeSet::new();
    for schemes in checker.builtins.raw_values().values() {
        for scheme in schemes {
            collect_type_constructors(&scheme.ty, &mut referenced);
        }
    }

    let mut missing: Vec<String> = referenced
        .into_iter()
        .filter(|name| !checker.builtin_types.contains_key(name))
        .collect();
    missing.sort();

    assert!(
        missing.is_empty(),
        "typechecker builtin kind env is missing constructors referenced by builtin values: {missing:?}"
    );
}
