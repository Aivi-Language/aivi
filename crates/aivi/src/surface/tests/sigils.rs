use std::path::Path;

use crate::surface::{parse_modules, Expr, Literal, ModuleItem};

use super::{diag_codes, expr_contains_ident, expr_contains_record_field, expr_contains_string};

#[test]
fn parses_structured_sigil_map_literal() {
    let src = r#"
module Example

x = ~map{ "a" => 1 }
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        !matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "map"),
        "expected ~map{{...}} to parse as a structured literal, not a sigil literal"
    );
}

#[test]
fn parses_structured_sigil_html_literal() {
    let src = r#"
module Example

x =
  ~<html>
    <div class="card">
      <span>{ 1 }</span>
    </div>
  </html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        !matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, .. }) if tag == "html"),
        "expected ~<html> to parse as a structured literal, not a sigil literal"
    );

    assert!(
        expr_contains_ident(&def.expr, "vElement") && expr_contains_ident(&def.expr, "vClass"),
        "expected ~<html> to lower into UI helpers"
    );
}

#[test]
fn parses_html_sigil_key_attribute() {
    let src = r#"
module Example

x = ~<html><div key="k">Hi</div></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        expr_contains_ident(&def.expr, "vKeyed"),
        "expected key= to lower into `vKeyed`"
    );
}

#[test]
fn html_sigil_component_tag_lowers_to_component_call() {
    let src = r#"
module Example

x = ~<html><Ui.Card title="Hello"><span>Body</span></Ui.Card></html>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    // Component call should use record-based lowering
    assert!(
        expr_contains_ident(&def.expr, "Ui") && expr_contains_ident(&def.expr, "Card"),
        "expected component tag to produce a component call"
    );
    assert!(
        expr_contains_record_field(&def.expr, "title"),
        "expected component attrs to lower to record fields"
    );
    assert!(
        expr_contains_record_field(&def.expr, "children"),
        "expected component children to be a `children` record field"
    );
}

#[test]
fn html_sigil_multiple_roots_is_error() {
    let src = r#"
module Example

x =
  ~<html>
    <div>One</div>
    <div>Two</div>
  </html>
"#;

    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1601"),
        "expected E1601 for multiple roots, got: {codes:?}"
    );
}

#[test]
fn parses_structured_sigil_gtk_literal() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkBox" props={ { marginTop: 24, spacing: 24 } }>
      <object class="GtkLabel">
        <property name="label">Hello</property>
      </object>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        expr_contains_ident(&def.expr, "gtkElement")
            && expr_contains_ident(&def.expr, "gtkStaticAttr")
            && expr_contains_ident(&def.expr, "gtkStaticProp"),
        "expected ~<gtk> to lower into GTK helper constructors"
    );
}

#[test]
fn gtk_sigil_component_tag_lowers_to_component_call() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkBox">
      <Ui.Row id="one" onClick={ Save } />
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    // Component call should use record-based lowering
    assert!(
        expr_contains_ident(&def.expr, "Ui") && expr_contains_ident(&def.expr, "Row"),
        "expected GTK component tag to produce a component call"
    );
    assert!(
        expr_contains_record_field(&def.expr, "id"),
        "expected component attrs to lower to record fields"
    );
    // onClick on a component tag must NOT be lowered to signal sugar
    assert!(
        expr_contains_record_field(&def.expr, "onClick"),
        "expected onClick on component to be a plain record field, not signal sugar"
    );
    assert!(
        !expr_contains_string(&def.expr, "signal:clicked"),
        "signal sugar must not fire on component tags"
    );
}

#[test]
fn gtk_sigil_function_call_tag_lowers_to_lower_camel_call() {
    let src = r#"
module Example

x = ~<gtk><NavRailNode model.appState.activeSection "sidebar" /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert!(
                matches!(func.as_ref(), Expr::Ident(name) if name.name == "navRailNode"),
                "expected GTK function tag to lower to a lowerCamel function call, got {func:?}"
            );
            assert_eq!(
                args.len(),
                2,
                "expected positional tag args to stay as separate call args"
            );
            assert!(
                matches!(&args[1], Expr::Literal(Literal::String { text, .. }) if text == "sidebar"),
                "expected second positional arg to stay as a string literal, got {:?}",
                args[1]
            );
        }
        other => panic!("expected function call, got {other:?}"),
    }

    assert!(
        !expr_contains_ident(&def.expr, "NavRailNode"),
        "expected sugar to rewrite the tag name to lowerCamel"
    );
}

#[test]
fn gtk_sigil_zero_arg_function_call_tag_lowers_to_lower_camel_unit_call() {
    let src = r#"
module Example

x = ~<gtk><AccountSettingsPage /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    match &def.expr {
        Expr::Call { func, args, .. } => {
            assert!(
                matches!(func.as_ref(), Expr::Ident(name) if name.name == "accountSettingsPage"),
                "expected zero-arg GTK helper tag to lower to a lowerCamel function call, got {func:?}"
            );
            assert_eq!(
                args.len(),
                1,
                "expected zero-arg GTK helper tag to pass Unit"
            );
            assert!(
                matches!(&args[0], Expr::Ident(name) if name.name == "Unit"),
                "expected zero-arg GTK helper tag to pass Unit, got {:?}",
                args[0]
            );
        }
        other => panic!("expected function call, got {other:?}"),
    }

    assert!(
        !expr_contains_ident(&def.expr, "AccountSettingsPage"),
        "expected zero-arg helper sugar to rewrite the tag name to lowerCamel"
    );
}

#[test]
fn gtk_sigil_function_call_tag_requires_self_closing_form() {
    let src = r#"
module Example

x =
  ~<gtk>
    <NavRailNode model.appState.activeSection>
    </NavRailNode>
  </gtk>
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1617"),
        "expected E1617 for non-self-closing GTK function-call sugar, got: {codes:?}"
    );
}

#[test]
fn gtk_sigil_props_requires_record_literal() {
    let src = r#"
module Example

dynamicProps = "nope"
x = ~<gtk><object class="GtkBox" props={ dynamicProps } /></gtk>
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1612"),
        "expected E1612 for non-record props splice, got: {codes:?}"
    );
}

#[test]
fn gtk_sigil_props_accepts_runtime_expressions() {
    let src = r#"
module Example

someValue = 24
x = ~<gtk><object class="GtkBox" props={ { spacing: someValue } } /></gtk>
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.is_empty(),
        "expected no diagnostics for runtime expression in props, got: {codes:?}"
    );
}

#[test]
fn gtk_sigil_onclick_lowers_to_signal_attr() {
    let src = r#"
module Example

Msg = Save
x = ~<gtk><object class="GtkButton" onClick={ Msg.Save } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "clicked")
            && expr_contains_ident(&def.expr, "Msg")
            && expr_contains_ident(&def.expr, "Save"),
        "expected onClick sugar to lower into gtkEventSugarAttr with Msg.Save handler"
    );
}

#[test]
fn gtk_sigil_onkeypress_lowers_to_signal_attr() {
    let src = r#"
module Example

handleKey = event => event
x = ~<gtk><object class="GtkBox" onKeyPress={ handleKey } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "key-pressed")
            && expr_contains_ident(&def.expr, "handleKey"),
        "expected onKeyPress sugar to lower into gtkEventSugarAttr with key-pressed handler"
    );
}

#[test]
fn gtk_sigil_onselect_lowers_to_dropdown_notify_signal() {
    let src = r#"
module Example

handleSelect = idx => idx
x = ~<gtk><GtkDropDown strings="A\nB" selected={0} onSelect={ handleSelect } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "notify::selected")
            && expr_contains_ident(&def.expr, "handleSelect"),
        "expected onSelect sugar to lower into gtkEventSugarAttr with notify::selected"
    );
}

#[test]
fn gtk_sigil_onswitch_toggle_uses_notify_active() {
    let src = r#"
module Example

handleToggle = active => active
x = ~<gtk><GtkSwitch onToggle={ handleToggle } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "notify::active")
            && expr_contains_ident(&def.expr, "handleToggle"),
        "expected GtkSwitch onToggle sugar to lower into notify::active"
    );
}

#[test]
fn gtk_sigil_onclosed_lowers_to_dialog_closed_signal() {
    let src = r#"
module Example

closeDialog = _ => Unit
x = ~<gtk><AdwPreferencesDialog open={True} onClosed={ closeDialog } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "closed")
            && expr_contains_ident(&def.expr, "closeDialog"),
        "expected onClosed sugar to lower into gtkEventSugarAttr with closed"
    );
}

#[test]
fn gtk_sigil_onshowsidebarchanged_lowers_to_overlay_split_view_signal() {
    let src = r#"
module Example

handleSidebar = visible => visible
x = ~<gtk><AdwOverlaySplitView onShowSidebarChanged={ handleSidebar } /></gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventSugarAttr")
            && expr_contains_string(&def.expr, "notify::show-sidebar")
            && expr_contains_ident(&def.expr, "handleSidebar"),
        "expected onShowSidebarChanged sugar to lower into gtkEventSugarAttr with notify::show-sidebar"
    );
}

#[test]
fn gtk_sigil_signal_on_accepts_runtime_handler_value() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkButton">
      <signal name="clicked" on={ x => x } />
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        expr_contains_ident(&def.expr, "gtkEventAttr"),
        "expected <signal on={{...}}> to lower into gtkEventAttr"
    );
}

#[test]
fn gtk_sigil_each_lowers_to_structural_binding() {
    let src = r#"
module Example

items = [1, 2, 3]
x =
  ~<gtk>
    <object class="GtkBox">
      <each items={items} as={item}>
        <child>
          <object class="GtkLabel">
            <property name="label">{ item }</property>
          </object>
        </child>
      </each>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );

    let module = modules.first().expect("module");
    let def = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x def");

    assert!(
        expr_contains_ident(&def.expr, "gtkEach") && expr_contains_ident(&def.expr, "gtkElement"),
        "expected <each> to lower via gtkEach into a structural GTK node"
    );
}

#[test]
fn gtk_sigil_each_key_lowers_to_keyed_structural_binding() {
    let src = r#"
module Example

items = [{ id: "a", label: "A" }]
x =
  ~<gtk>
    <object class="GtkBox">
      <each items={items} as={item} key={item => item.id}>
        <child>
          <object class="GtkLabel">
            <property name="label">{ item.label }</property>
          </object>
        </child>
      </each>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        expr_contains_ident(&def.expr, "gtkEachKeyed"),
        "expected keyed <each> to lower into gtkEachKeyed"
    );
}

#[test]
fn gtk_sigil_show_lowers_to_structural_binding() {
    let src = r#"
module Example

visible = True
x =
  ~<gtk>
    <object class="GtkBox">
      <show when={visible}>
        <object class="GtkLabel" />
      </show>
    </object>
  </gtk>
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        expr_contains_ident(&def.expr, "gtkShow"),
        "expected <show> to lower into gtkShow"
    );
}

#[test]
fn gtk_sigil_each_requires_items_and_as_attributes() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkBox">
      <each>
        <child>
          <object class="GtkLabel" />
        </child>
      </each>
    </object>
  </gtk>
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1615"),
        "expected E1615 for invalid <each> usage, got: {codes:?}"
    );
}

#[test]
fn parses_domain_literal_def_in_embedded_ui_layout() {
    let src = crate::stdlib::embedded_stdlib_source("aivi.ui.layout")
        .expect("embedded stdlib source for aivi.ui.layout");
    let (modules, diags) = parse_modules(Path::new("<embedded:aivi.ui.layout>"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules
        .iter()
        .find(|m| m.name.name == "aivi.ui.layout")
        .expect("aivi.ui.layout module");
    let domain = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::DomainDecl(domain) if domain.name.name == "Layout" => Some(domain),
            _ => None,
        })
        .expect("Layout domain");

    let has_1px = domain.items.iter().any(|item| match item {
        crate::surface::DomainItem::LiteralDef(def) => def.name.name == "1px",
        _ => false,
    });
    let literal_defs: Vec<String> = domain
        .items
        .iter()
        .filter_map(|item| match item {
            crate::surface::DomainItem::LiteralDef(def) => Some(def.name.name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        has_1px,
        "expected Layout domain to define a `1px` literal template"
    );
    assert!(
        literal_defs.contains(&"1%".to_string()),
        "expected Layout domain to define a `1%` literal template"
    );
}

#[test]
fn parses_domain_literal_def_px_suffix() {
    let src = r#"
module Example

UnitVal = { val: Float }

domain Layout over UnitVal = {
  Length = Px Float

  1px = Px 1.0
  1em = Em 1.0
}
"#;
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diag_codes(&diags)
    );
    let module = modules.first().expect("module");
    let domain = module
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::DomainDecl(domain) if domain.name.name == "Layout" => Some(domain),
            _ => None,
        })
        .expect("Layout domain");

    let literal_defs: Vec<String> = domain
        .items
        .iter()
        .filter_map(|item| match item {
            crate::surface::DomainItem::LiteralDef(def) => Some(def.name.name.clone()),
            _ => None,
        })
        .collect();
    assert!(
        literal_defs.contains(&"1px".to_string()),
        "expected literal defs to contain 1px, got {literal_defs:?}"
    );
    assert!(
        literal_defs.contains(&"1em".to_string()),
        "expected literal defs to contain 1em, got {literal_defs:?}"
    );
}

// ─────────────────────────────────────────────────────────
// GTK sigil — additional code paths
// ─────────────────────────────────────────────────────────

#[test]
fn gtk_sigil_with_id_attribute() {
    let src = "module Example\n\nx = ~<gtk><object class=\"GtkEntry\" id=\"nameInput\" /></gtk>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        expr_contains_ident(&def.expr, "gtkIdAttr"),
        "expected id attr to lower via gtkIdAttr"
    );
}

#[test]
fn gtk_sigil_splice_in_property() {
    let src = "module Example\n\nmyLabel = \"hello\"\nx =\n  ~<gtk>\n    <object class=\"GtkLabel\">\n      <property name=\"label\">{ myLabel }</property>\n    </object>\n  </gtk>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        expr_contains_ident(&def.expr, "gtkBoundText") && expr_contains_ident(&def.expr, "myLabel")
    );
}

#[test]
fn gtk_sigil_boolean_attribute() {
    let src = "module Example\n\nx = ~<gtk><object class=\"GtkButton\" visible={ True } sensitive={ False } /></gtk>\n";
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn gtk_sigil_numeric_attribute_splice() {
    let src = "module Example\n\nx = ~<gtk><object class=\"GtkBox\" spacing={ 12 } /></gtk>\n";
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
}

// ─────────────────────────────────────────────────────────
// HTML sigil — additional code paths
// ─────────────────────────────────────────────────────────

#[test]
fn html_sigil_void_elements() {
    let src = "module Example\n\nx =\n  ~<html>\n    <div>\n      <br />\n      <hr />\n      <img src=\"test.png\" />\n    </div>\n  </html>\n";
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn html_sigil_style_attribute() {
    let src =
        "module Example\n\nx = ~<html><div style={ { color: \"red\" } }>Styled</div></html>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(expr_contains_ident(&def.expr, "vElement"));
}

#[test]
fn html_sigil_text_node() {
    let src = "module Example\n\nx = ~<html><p>Just plain text</p></html>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(expr_contains_ident(&def.expr, "vText") || expr_contains_ident(&def.expr, "vElement"));
}

#[test]
fn html_sigil_nested_splices() {
    let src = "module Example\n\ncount = 42\nname = \"world\"\nx = ~<html><div>Hello { name }, count: { count }</div></html>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(expr_contains_ident(&def.expr, "name"));
    assert!(expr_contains_ident(&def.expr, "count"));
}

#[test]
fn html_sigil_empty_element() {
    let src = "module Example\n\nx = ~<html><div></div></html>\n";
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
}

// ─────────────────────────────────────────────────────────
// Non-structured sigils
// ─────────────────────────────────────────────────────────

#[test]
fn parses_sigil_with_quote_delimiter() {
    let src = "module Example\n\nx = ~sql\"SELECT * FROM users\"\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    assert!(
        matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, body, .. }) if tag == "sql" && body == "SELECT * FROM users")
    );
}

#[test]
fn parses_sigil_with_escape_in_slash() {
    let src = "module Example\n\nx = ~regex/a\\/b/\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
    let def = modules[0]
        .items
        .iter()
        .find_map(|i| match i {
            ModuleItem::Def(d) if d.name.name == "x" => Some(d),
            _ => None,
        })
        .expect("x");
    match &def.expr {
        Expr::Literal(Literal::Sigil { tag, body, .. }) => {
            assert_eq!(tag, "regex");
            assert!(body.contains("a\\"), "body: {body}");
        }
        other => panic!("expected Sigil, got {other:?}"),
    }
}

#[test]
fn parses_raw_text_sigil_embedded_language_header() {
    let src = "module Example\n\nx = ~`css\nbody`\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x");

    assert!(
        matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, body, .. }) if tag == "raw" && body == "body")
    );
}

#[test]
fn parses_raw_text_sigil_pipe_margin() {
    let src = "module Example\n\nx = ~`\n    | Hallo\n    | Andreas\n`\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );

    let def = modules[0]
        .items
        .iter()
        .find_map(|item| match item {
            ModuleItem::Def(def) if def.name.name == "x" => Some(def),
            _ => None,
        })
        .expect("x");

    assert!(
        matches!(&def.expr, Expr::Literal(Literal::Sigil { tag, body, .. }) if tag == "raw" && body == "Hallo\nAndreas")
    );
}
