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
        expr_contains_ident(&def.expr, "gtkElement") && expr_contains_ident(&def.expr, "gtkAttr"),
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
        expr_contains_string(&def.expr, "signal:clicked")
            && expr_contains_ident(&def.expr, "Msg")
            && expr_contains_ident(&def.expr, "Save"),
        "expected onClick sugar to lower into signal:clicked attribute with Msg.Save handler"
    );
}

#[test]
fn gtk_sigil_signal_on_requires_compile_time_value() {
    let src = r#"
module Example

x =
  ~<gtk>
    <object class="GtkButton">
      <signal name="clicked" on={ x => x } />
    </object>
  </gtk>
"#;
    let (_modules, diags) = parse_modules(Path::new("test.aivi"), src);
    let codes = diag_codes(&diags);
    assert!(
        codes.iter().any(|code| code == "E1614"),
        "expected E1614 for non-compile-time signal handler, got: {codes:?}"
    );
}

#[test]
fn gtk_sigil_each_lowers_to_mapped_children() {
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
        expr_contains_ident(&def.expr, "each") && expr_contains_ident(&def.expr, "gtkElement"),
        "expected <each> to lower to mapped GtkNode children"
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
        expr_contains_ident(&def.expr, "gtkElement") || expr_contains_ident(&def.expr, "gtkAttr")
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
    assert!(expr_contains_ident(&def.expr, "myLabel"));
}

#[test]
fn gtk_sigil_boolean_attribute() {
    let src = "module Example\n\nx = ~<gtk><object class=\"GtkButton\" visible={ True } sensitive={ False } /></gtk>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
    assert!(
        diags.is_empty(),
        "unexpected diags: {:?}",
        diag_codes(&diags)
    );
}

#[test]
fn gtk_sigil_numeric_attribute_splice() {
    let src = "module Example\n\nx = ~<gtk><object class=\"GtkBox\" spacing={ 12 } /></gtk>\n";
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
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
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
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
    let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
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
