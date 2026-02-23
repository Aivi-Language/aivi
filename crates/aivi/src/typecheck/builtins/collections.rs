use super::TypeChecker;
use crate::typecheck::types::{Scheme, Type, TypeEnv};

pub(super) fn register(checker: &mut TypeChecker, env: &mut TypeEnv) {
    let int_ty = Type::con("Int");
    let bool_ty = Type::con("Bool");

    let map_k = checker.fresh_var_id();
    let map_v = checker.fresh_var_id();
    let map_v2 = checker.fresh_var_id();
    let map_ty = Type::con("Map").app(vec![Type::Var(map_k), Type::Var(map_v)]);
    let map_ty_v2 = Type::con("Map").app(vec![Type::Var(map_k), Type::Var(map_v2)]);
    let map_tuple_ty = Type::Tuple(vec![Type::Var(map_k), Type::Var(map_v)]);
    let list_map_tuple_ty = Type::con("List").app(vec![map_tuple_ty.clone()]);
    let map_record = Type::Record {
        fields: vec![
            ("empty".to_string(), map_ty.clone()),
            (
                "size".to_string(),
                Type::Func(Box::new(map_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "has".to_string(),
                Type::Func(
                    Box::new(Type::Var(map_k)),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "get".to_string(),
                Type::Func(
                    Box::new(Type::Var(map_k)),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(Type::con("Option").app(vec![Type::Var(map_v)])),
                    )),
                ),
            ),
            (
                "insert".to_string(),
                Type::Func(
                    Box::new(Type::Var(map_k)),
                    Box::new(Type::Func(
                        Box::new(Type::Var(map_v)),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "update".to_string(),
                Type::Func(
                    Box::new(Type::Var(map_k)),
                    Box::new(Type::Func(
                        Box::new(Type::Func(
                            Box::new(Type::Var(map_v)),
                            Box::new(Type::Var(map_v)),
                        )),
                        Box::new(Type::Func(
                            Box::new(map_ty.clone()),
                            Box::new(map_ty.clone()),
                        )),
                    )),
                ),
            ),
            (
                "remove".to_string(),
                Type::Func(
                    Box::new(Type::Var(map_k)),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(map_ty.clone()),
                    )),
                ),
            ),
            (
                "map".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(map_v)),
                        Box::new(Type::Var(map_v2)),
                    )),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(map_ty_v2.clone()),
                    )),
                ),
            ),
            (
                "mapWithKey".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(map_k)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(map_v)),
                            Box::new(Type::Var(map_v2)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(map_ty_v2.clone()),
                    )),
                ),
            ),
            (
                "keys".to_string(),
                Type::Func(
                    Box::new(map_ty.clone()),
                    Box::new(Type::con("List").app(vec![Type::Var(map_k)])),
                ),
            ),
            (
                "values".to_string(),
                Type::Func(
                    Box::new(map_ty.clone()),
                    Box::new(Type::con("List").app(vec![Type::Var(map_v)])),
                ),
            ),
            (
                "entries".to_string(),
                Type::Func(
                    Box::new(map_ty.clone()),
                    Box::new(list_map_tuple_ty.clone()),
                ),
            ),
            (
                "fromList".to_string(),
                Type::Func(
                    Box::new(list_map_tuple_ty.clone()),
                    Box::new(map_ty.clone()),
                ),
            ),
            (
                "toList".to_string(),
                Type::Func(
                    Box::new(map_ty.clone()),
                    Box::new(list_map_tuple_ty.clone()),
                ),
            ),
            (
                "union".to_string(),
                Type::Func(
                    Box::new(map_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(map_ty.clone()),
                        Box::new(map_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let map_record_value = map_record.clone();

    let set_a = checker.fresh_var_id();
    let set_ty = Type::con("Set").app(vec![Type::Var(set_a)]);
    let set_record = Type::Record {
        fields: vec![
            ("empty".to_string(), set_ty.clone()),
            (
                "size".to_string(),
                Type::Func(Box::new(set_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "has".to_string(),
                Type::Func(
                    Box::new(Type::Var(set_a)),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(bool_ty.clone()),
                    )),
                ),
            ),
            (
                "insert".to_string(),
                Type::Func(
                    Box::new(Type::Var(set_a)),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(set_ty.clone()),
                    )),
                ),
            ),
            (
                "remove".to_string(),
                Type::Func(
                    Box::new(Type::Var(set_a)),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(set_ty.clone()),
                    )),
                ),
            ),
            (
                "union".to_string(),
                Type::Func(
                    Box::new(set_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(set_ty.clone()),
                    )),
                ),
            ),
            (
                "intersection".to_string(),
                Type::Func(
                    Box::new(set_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(set_ty.clone()),
                    )),
                ),
            ),
            (
                "difference".to_string(),
                Type::Func(
                    Box::new(set_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(set_ty.clone()),
                        Box::new(set_ty.clone()),
                    )),
                ),
            ),
            (
                "fromList".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![Type::Var(set_a)])),
                    Box::new(set_ty.clone()),
                ),
            ),
            (
                "toList".to_string(),
                Type::Func(
                    Box::new(set_ty.clone()),
                    Box::new(Type::con("List").app(vec![Type::Var(set_a)])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let set_record_value = set_record.clone();

    let queue_a = checker.fresh_var_id();
    let queue_ty = Type::con("Queue").app(vec![Type::Var(queue_a)]);
    let queue_tuple_ty = Type::Tuple(vec![Type::Var(queue_a), queue_ty.clone()]);
    let queue_record = Type::Record {
        fields: vec![
            ("empty".to_string(), queue_ty.clone()),
            (
                "enqueue".to_string(),
                Type::Func(
                    Box::new(Type::Var(queue_a)),
                    Box::new(Type::Func(
                        Box::new(queue_ty.clone()),
                        Box::new(queue_ty.clone()),
                    )),
                ),
            ),
            (
                "dequeue".to_string(),
                Type::Func(
                    Box::new(queue_ty.clone()),
                    Box::new(Type::con("Option").app(vec![queue_tuple_ty.clone()])),
                ),
            ),
            (
                "peek".to_string(),
                Type::Func(
                    Box::new(queue_ty.clone()),
                    Box::new(Type::con("Option").app(vec![Type::Var(queue_a)])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let queue_record_value = queue_record.clone();

    let deque_a = checker.fresh_var_id();
    let deque_ty = Type::con("Deque").app(vec![Type::Var(deque_a)]);
    let deque_tuple_ty = Type::Tuple(vec![Type::Var(deque_a), deque_ty.clone()]);
    let deque_record = Type::Record {
        fields: vec![
            ("empty".to_string(), deque_ty.clone()),
            (
                "pushFront".to_string(),
                Type::Func(
                    Box::new(Type::Var(deque_a)),
                    Box::new(Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(deque_ty.clone()),
                    )),
                ),
            ),
            (
                "pushBack".to_string(),
                Type::Func(
                    Box::new(Type::Var(deque_a)),
                    Box::new(Type::Func(
                        Box::new(deque_ty.clone()),
                        Box::new(deque_ty.clone()),
                    )),
                ),
            ),
            (
                "popFront".to_string(),
                Type::Func(
                    Box::new(deque_ty.clone()),
                    Box::new(Type::con("Option").app(vec![deque_tuple_ty.clone()])),
                ),
            ),
            (
                "popBack".to_string(),
                Type::Func(
                    Box::new(deque_ty.clone()),
                    Box::new(Type::con("Option").app(vec![deque_tuple_ty.clone()])),
                ),
            ),
            (
                "peekFront".to_string(),
                Type::Func(
                    Box::new(deque_ty.clone()),
                    Box::new(Type::con("Option").app(vec![Type::Var(deque_a)])),
                ),
            ),
            (
                "peekBack".to_string(),
                Type::Func(
                    Box::new(deque_ty.clone()),
                    Box::new(Type::con("Option").app(vec![Type::Var(deque_a)])),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let deque_record_value = deque_record.clone();

    let heap_a = checker.fresh_var_id();
    let heap_ty = Type::con("Heap").app(vec![Type::Var(heap_a)]);
    let heap_tuple_ty = Type::Tuple(vec![Type::Var(heap_a), heap_ty.clone()]);

    let heap_record = Type::Record {
        fields: vec![
            ("empty".to_string(), heap_ty.clone()),
            (
                "push".to_string(),
                Type::Func(
                    Box::new(Type::Var(heap_a)),
                    Box::new(Type::Func(
                        Box::new(heap_ty.clone()),
                        Box::new(heap_ty.clone()),
                    )),
                ),
            ),
            (
                "popMin".to_string(),
                Type::Func(
                    Box::new(heap_ty.clone()),
                    Box::new(Type::con("Option").app(vec![heap_tuple_ty.clone()])),
                ),
            ),
            (
                "peekMin".to_string(),
                Type::Func(
                    Box::new(heap_ty.clone()),
                    Box::new(Type::con("Option").app(vec![Type::Var(heap_a)])),
                ),
            ),
            (
                "size".to_string(),
                Type::Func(Box::new(heap_ty.clone()), Box::new(int_ty.clone())),
            ),
            (
                "fromList".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![Type::Var(heap_a)])),
                    Box::new(heap_ty.clone()),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let heap_record_value = heap_record.clone();

    // ── List record ─────────────────────────────────────────────────
    let list_a = checker.fresh_var_id();
    let list_b = checker.fresh_var_id();
    let list_c = checker.fresh_var_id();
    let list_ty = Type::con("List").app(vec![Type::Var(list_a)]);
    let list_ty_b = Type::con("List").app(vec![Type::Var(list_b)]);
    let option_a = Type::con("Option").app(vec![Type::Var(list_a)]);
    let option_b = Type::con("Option").app(vec![Type::Var(list_b)]);

    let list_record = Type::Record {
        fields: vec![
            // empty : List a
            ("empty".to_string(), list_ty.clone()),
            // isEmpty : List a -> Bool
            (
                "isEmpty".to_string(),
                Type::Func(Box::new(list_ty.clone()), Box::new(bool_ty.clone())),
            ),
            // length : List a -> Int
            (
                "length".to_string(),
                Type::Func(Box::new(list_ty.clone()), Box::new(int_ty.clone())),
            ),
            // map : (a -> b) -> List a -> List b
            (
                "map".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(Type::Var(list_b)),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty_b.clone()),
                    )),
                ),
            ),
            // filter : (a -> Bool) -> List a -> List a
            (
                "filter".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(bool_ty.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // flatMap : (a -> List b) -> List a -> List b
            (
                "flatMap".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(list_ty_b.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty_b.clone()),
                    )),
                ),
            ),
            // foldl : (b -> a -> b) -> b -> List a -> b
            (
                "foldl".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_b)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(list_a)),
                            Box::new(Type::Var(list_b)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_b)),
                        Box::new(Type::Func(
                            Box::new(list_ty.clone()),
                            Box::new(Type::Var(list_b)),
                        )),
                    )),
                ),
            ),
            // foldr : (a -> b -> b) -> b -> List a -> b
            (
                "foldr".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(list_b)),
                            Box::new(Type::Var(list_b)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_b)),
                        Box::new(Type::Func(
                            Box::new(list_ty.clone()),
                            Box::new(Type::Var(list_b)),
                        )),
                    )),
                ),
            ),
            // scanl : (b -> a -> b) -> b -> List a -> List b
            (
                "scanl".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_b)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(list_a)),
                            Box::new(Type::Var(list_b)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_b)),
                        Box::new(Type::Func(
                            Box::new(list_ty.clone()),
                            Box::new(list_ty_b.clone()),
                        )),
                    )),
                ),
            ),
            // take : Int -> List a -> List a
            (
                "take".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // drop : Int -> List a -> List a
            (
                "drop".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // takeWhile : (a -> Bool) -> List a -> List a
            (
                "takeWhile".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(bool_ty.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // dropWhile : (a -> Bool) -> List a -> List a
            (
                "dropWhile".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(bool_ty.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // partition : (a -> Bool) -> List a -> (List a, List a)
            (
                "partition".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(bool_ty.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(Type::Tuple(vec![list_ty.clone(), list_ty.clone()])),
                    )),
                ),
            ),
            // find : (a -> Bool) -> List a -> Option a
            (
                "find".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(bool_ty.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(option_a.clone()),
                    )),
                ),
            ),
            // findMap : (a -> Option b) -> List a -> Option b
            (
                "findMap".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(option_b.clone()),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(option_b.clone()),
                    )),
                ),
            ),
            // at : Int -> List a -> Option a
            (
                "at".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(option_a.clone()),
                    )),
                ),
            ),
            // indexOf : a -> List a -> Option Int
            (
                "indexOf".to_string(),
                Type::Func(
                    Box::new(Type::Var(list_a)),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(Type::con("Option").app(vec![int_ty.clone()])),
                    )),
                ),
            ),
            // zip : List a -> List b -> List (a, b)
            (
                "zip".to_string(),
                Type::Func(
                    Box::new(list_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(list_ty_b.clone()),
                        Box::new(Type::con("List").app(vec![Type::Tuple(vec![
                            Type::Var(list_a),
                            Type::Var(list_b),
                        ])])),
                    )),
                ),
            ),
            // zipWith : (a -> b -> c) -> List a -> List b -> List c
            (
                "zipWith".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(Type::Func(
                            Box::new(Type::Var(list_b)),
                            Box::new(Type::Var(list_c)),
                        )),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(Type::Func(
                            Box::new(list_ty_b.clone()),
                            Box::new(Type::con("List").app(vec![Type::Var(list_c)])),
                        )),
                    )),
                ),
            ),
            // unzip : List (a, b) -> (List a, List b)
            (
                "unzip".to_string(),
                Type::Func(
                    Box::new(Type::con("List").app(vec![Type::Tuple(vec![
                        Type::Var(list_a),
                        Type::Var(list_b),
                    ])])),
                    Box::new(Type::Tuple(vec![list_ty.clone(), list_ty_b.clone()])),
                ),
            ),
            // intersperse : a -> List a -> List a
            (
                "intersperse".to_string(),
                Type::Func(
                    Box::new(Type::Var(list_a)),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
            // chunk : Int -> List a -> List (List a)
            (
                "chunk".to_string(),
                Type::Func(
                    Box::new(int_ty.clone()),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(Type::con("List").app(vec![list_ty.clone()])),
                    )),
                ),
            ),
            // dedup : List a -> List a
            (
                "dedup".to_string(),
                Type::Func(Box::new(list_ty.clone()), Box::new(list_ty.clone())),
            ),
            // uniqueBy : (a -> b) -> List a -> List a
            (
                "uniqueBy".to_string(),
                Type::Func(
                    Box::new(Type::Func(
                        Box::new(Type::Var(list_a)),
                        Box::new(Type::Var(list_b)),
                    )),
                    Box::new(Type::Func(
                        Box::new(list_ty.clone()),
                        Box::new(list_ty.clone()),
                    )),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };
    let list_record_value = list_record.clone();

    let collections_record = Type::Record {
        fields: vec![
            ("map".to_string(), map_record),
            ("set".to_string(), set_record),
            ("queue".to_string(), queue_record),
            ("deque".to_string(), deque_record),
            ("heap".to_string(), heap_record),
            ("list".to_string(), list_record),
        ]
        .into_iter()
        .collect(),
    };
    env.insert(
        "collections".to_string(),
        Scheme {
            vars: vec![map_k, map_v, map_v2, set_a, queue_a, deque_a, heap_a],
            ty: collections_record,
            origin: None,
        },
    );
    env.insert(
        "Map".to_string(),
        Scheme {
            vars: vec![map_k, map_v, map_v2],
            ty: map_record_value,
            origin: None,
        },
    );
    env.insert(
        "Set".to_string(),
        Scheme {
            vars: vec![set_a],
            ty: set_record_value,
            origin: None,
        },
    );
    env.insert(
        "Queue".to_string(),
        Scheme {
            vars: vec![queue_a],
            ty: queue_record_value,
            origin: None,
        },
    );
    env.insert(
        "Deque".to_string(),
        Scheme {
            vars: vec![deque_a],
            ty: deque_record_value,
            origin: None,
        },
    );
    env.insert(
        "Heap".to_string(),
        Scheme {
            vars: vec![heap_a],
            ty: heap_record_value,
            origin: None,
        },
    );
    env.insert(
        "List".to_string(),
        Scheme {
            vars: vec![list_a, list_b, list_c],
            ty: list_record_value,
            origin: None,
        },
    );
}
