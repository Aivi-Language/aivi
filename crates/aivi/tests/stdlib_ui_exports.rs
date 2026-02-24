use aivi::{embedded_stdlib_modules, ModuleItem};

#[test]
fn stdlib_ui_exports_v_element() {
    let modules = embedded_stdlib_modules();
    let ui = modules
        .iter()
        .find(|m| m.name.name == "aivi.ui")
        .expect("aivi.ui module exists");

    assert!(
        ui.exports.iter().any(|e| e.name.name == "vElement"),
        "expected aivi.ui to export vElement, exports={:?}",
        ui.exports
            .iter()
            .map(|e| e.name.name.as_str())
            .collect::<Vec<_>>()
    );

    let def_names: Vec<&str> = ui
        .items
        .iter()
        .filter_map(|item| match item {
            ModuleItem::Def(def) => Some(def.name.name.as_str()),
            _ => None,
        })
        .collect();

    let v_element_def = ui.items.iter().find_map(|item| match item {
        ModuleItem::Def(def) if def.name.name == "vElement" => Some(def),
        _ => None,
    });
    assert!(
        v_element_def.is_some(),
        "expected aivi.ui to define vElement; defs={def_names:?}"
    );

    for expected in ["vText", "vKeyed", "vClass", "vId", "vStyle", "vAttr"] {
        assert!(
            def_names.iter().any(|n| *n == expected),
            "expected aivi.ui to define {expected}; defs={def_names:?}"
        );
    }

    let export_names: Vec<&str> = ui.exports.iter().map(|e| e.name.name.as_str()).collect();
    for expected in ["vText", "vKeyed", "vClass", "vId", "vStyle", "vAttr"] {
        assert!(
            export_names.iter().any(|n| *n == expected),
            "expected aivi.ui to export {expected}; exports={export_names:?}"
        );
    }

    let _def = v_element_def.expect("vElement def");
}
