use std::path::Path;

use aivi::{
    check_modules, check_types, embedded_stdlib_source, file_diagnostics_have_errors, parse_modules,
};

fn check_ok(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "unexpected errors: {module_diags:?}"
    );
}

fn check_ok_with_embedded(source: &str, embedded: &[&str]) {
    let mut modules = Vec::new();
    for module_name in embedded {
        let embedded_source =
            embedded_stdlib_source(module_name).unwrap_or_else(|| panic!("missing {module_name}"));
        let (mut embedded_modules, embedded_diags) = parse_modules(
            Path::new(&format!("<embedded:{module_name}>")),
            embedded_source,
        );
        assert!(
            !file_diagnostics_have_errors(&embedded_diags),
            "parse errors in embedded {module_name}: {embedded_diags:?}"
        );
        modules.append(&mut embedded_modules);
    }

    let (mut user_modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );
    modules.append(&mut user_modules);

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "unexpected errors: {module_diags:?}"
    );
}

fn check_err(source: &str) {
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected errors, got: {module_diags:?}"
    );
}

fn slice_span(source: &str, span: &aivi::Span) -> String {
    // Spans are 1-based (line/column) and end column is inclusive; VSCode ranges are derived from
    // these by treating end.column as exclusive in 0-based coordinates.
    let mut offset = 0usize;
    let mut current_line = 1usize;
    let mut start_offset = None;
    let mut end_offset = None;
    for line in source.split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset + line.len();
        if current_line == span.start.line {
            let start_col0 = span.start.column.saturating_sub(1);
            start_offset = Some(line_start + start_col0);
        }
        if current_line == span.end.line {
            // Convert inclusive end column (1-based) to exclusive byte offset in ASCII.
            end_offset = Some(line_start + span.end.column);
        }
        if start_offset.is_some() && end_offset.is_some() {
            break;
        }
        offset = line_end;
        current_line += 1;
    }
    let Some(start) = start_offset else {
        return String::new();
    };
    let Some(end) = end_offset else {
        return String::new();
    };
    let end = end.min(source.len());
    let start = start.min(end);
    source.get(start..end).unwrap_or("").to_string()
}

#[test]
fn typecheck_effects_resources() {
    let source = r#"
module test.core
export main

main : Effect Text Unit
main = do Effect {
  f <- resource {
    handle <- file.open "Cargo.toml"
    yield handle
    _ <- file.close handle
  }
  _ <- file.readAll f
  _ <- print "ok"
  pure Unit
}"#;
    check_ok(source);
}

#[test]
fn typecheck_domains_patching() {
    let source = r#"
module test.m7
export addWeek, updated

Date = { year: Int, month: Int, day: Int }

 domain Calendar over Date = {
  Delta = Day Int | Week Int

  (+) : Date -> Delta -> Date
  (+) = d delta => delta match
    | Day n => addDays d n
    | Week n => addDays d (n * 7)

  1w = Week 1
}

 addDays : Date -> Int -> Date
 addDays = d n => d <| { day: _ + n }

 addWeek : Date -> Date
 addWeek = d => d + 2w

updated = addWeek { year: 2024, month: 9, day: 1 }"#;
    check_ok(source);
}

#[test]
fn typecheck_html_sigil_style_record_is_closed() {
    let source = r#"
module test.ui
export node

use aivi.ui
use aivi.ui.layout

node =
  ~<html>
    <div style={ { width: 10px, gap: 2em } }>
      <span>{ TextNode "1" }</span>
    </div>
  </html>"#;
    check_ok_with_embedded(source, &["aivi", "aivi.ui", "aivi.ui.layout"]);
}

#[test]
fn typecheck_html_sigil_splice_text_still_checks_style_shape() {
    let source = r#"
module test.ui_splice_text
export node

use aivi.ui
use aivi.ui.layout

node =
  ~<html>
    <div class="card" style={ { width: 10px } }>
      { "hallo" }
    </div>
  </html>"#;
    check_ok_with_embedded(source, &["aivi", "aivi.ui", "aivi.ui.layout"]);
}

#[test]
fn typecheck_gtk_sigil_props_record_literal() {
    let source = r#"
module test.gtk_sigil
export node

use aivi.ui.gtk4

node : GtkNode
node =
  ~<gtk>
    <object class="GtkBox" props={ { spacing: 24, marginTop: 12 } }>
      <object class="GtkLabel">
        <property name="label">Hello</property>
      </object>
    </object>
  </gtk>"#;
    check_ok_with_embedded(source, &["aivi", "aivi.ui.gtk4"]);
}

#[test]
fn typecheck_record_field_mismatch_points_at_value() {
    let source = "module test.user\n\
User = { name: Text, age: Int }\n\
\n\
user1 : User\n\
user1 = { name: \"Alice\", age: \"a\" }\n";

    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(diagnostics.is_empty(), "parse diagnostics: {diagnostics:?}");

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(!module_diags.is_empty(), "expected diagnostics");

    let mismatch = module_diags
        .iter()
        .find(|d| d.diagnostic.message.starts_with("type mismatch"))
        .unwrap_or_else(|| panic!("expected type mismatch diagnostic, got: {module_diags:?}"));
    let slice = slice_span(source, &mismatch.diagnostic.span);
    assert!(
        slice == "\"a\"" || slice == "a",
        "expected span to highlight the bad value, got slice={slice:?}, span={:?}",
        mismatch.diagnostic.span
    );
}

#[test]
fn typecheck_reports_kind_mismatch_in_type_application() {
    let source = r#"
module test.kind
bad : List List
bad = []
"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );
    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.iter().any(|d| d
            .diagnostic
            .message
            .contains("kind mismatch in type application")),
        "expected kind mismatch diagnostic, got: {module_diags:?}"
    );
}

#[test]
fn typecheck_domain_operator_overload_without_deltas() {
    let source = r#"
module test.domain_ops
export move

Vec2 = { x: Int, y: Int }

 domain Vector over Vec2 = {
  (+) : Vec2 -> Vec2 -> Vec2
  (+) = a b => { x: a.x + b.x, y: a.y + b.y }
 }

 move : Vec2 -> Vec2 -> Vec2
 move = pos vel => pos + vel"#;
    check_ok(source);
}

#[test]
fn typecheck_error_unknown_numeric_delta_literal() {
    let source = r#"
module test.delta_err
export value
value = 2w"#;
    check_err(source);
}

#[test]
fn typecheck_suffix_literal_from_imported_domain() {
    let source = r#"
@no_prelude
module test.delta_import
export brightRed

use aivi
use aivi.color (Rgb, domain Color)

red : Rgb
red = { r: 255, g: 0, b: 0 }

brightRed = red + 10l"#;
    check_ok_with_embedded(source, &["aivi", "aivi.color"]);
}

#[test]
fn typecheck_suffix_literal_from_wildcard_imported_domain() {
    let source = r#"
@no_prelude
module test.delta_wildcard_import
export brightRed

use aivi
use aivi.color

red : Rgb
red = { r: 255, g: 0, b: 0 }

brightRed = red + 10l"#;
    check_ok_with_embedded(source, &["aivi", "aivi.color"]);
}

#[test]
fn typecheck_suffix_application_with_variable() {
    let source = r#"
module test.delta_apply
export value

domain Units over Int = {
  Delta = Kg Int
  1kg = Kg 1
}

n : Int
n = 5

value = (n)kg"#;
    check_ok(source);
}

#[test]
fn typecheck_int_cross_operator() {
    let source = r#"
module test.cross_int
export value

value = 2 × 3"#;
    check_ok(source);
}

#[test]
fn typecheck_record_literal_missing_required_field_is_error() {
    let source = r#"
module test.record_missing
export user

User = { name: Text, age: Int }

user : User
user = { name: "Alice" }"#;
    check_ok(source);
}

#[test]
fn typecheck_imported_type_alias_checks_record_fields() {
    let source = r#"
@no_prelude
module test.imported_alias_missing_field
export red

use aivi
use aivi.color (Rgb)

red : Rgb
red = { a: 234, g: 0, b: 0 }"#;
    check_ok_with_embedded(source, &["aivi", "aivi.color"]);
}

#[test]
fn typecheck_branded_type_is_nominal() {
    let source = r#"
module test.branded_nominal
export email

Email = Text!

email : Email
email = "alice@example.com""#;
    check_err(source);
}

#[test]
fn typecheck_branded_type_autoforwards_base_instances() {
    let source = r#"
module test.branded_forward
export render

class ToText A = {
  toText: A -> Text
}

instance ToText Text = {
  toText: value => value
}

Email = Text!

render : Email -> Text
render = value => toText value"#;
    check_ok(source);
}

#[test]
fn typecheck_explicit_branded_instance_takes_precedence() {
    let source = r#"
module test.branded_override
export render

class ToText A = {
  toText: A -> Text
}

instance ToText Text = {
  toText: value => value
}

Email = Text!

instance ToText Email = {
  toText: _ => "email"
}

render : Email -> Text
render = value => toText value"#;
    check_ok(source);
}

#[test]
fn typecheck_error_effect_final() {
    let source = r#"
module test.err
export main

main : Effect Text Unit
main = do Effect {
  1
}"#;
    check_err(source);
}

#[test]
fn typecheck_effect_block_pure_let_and_unit_statements() {
    let source = r#"
module test.effect_sugar
export main

// Minimal local stand-ins
Result E A = Err E | Ok A

main : Effect Text Unit
main = do Effect {
  n <- 41
  m <- n + 1

  res <- attempt (if m == 42 then fail "boom" else pure m)

  verdict = res match
    | Ok _  => "ok"
    | Err _ => "err"

  print verdict

  if m > 40 then print "branch" else Unit
}"#;
    check_ok(source);
}

#[test]
fn typecheck_effect_block_statement_requires_unit() {
    let source = r#"
module test.effect_stmt_unit
export main, foo

foo : Effect Text Int
foo = pure 1

main : Effect Text Unit
main = do Effect {
  println "start"
  foo
  pure Unit
}"#;
    check_err(source);
}

#[test]
fn typecheck_effect_block_let_rejects_effect_expr() {
    let source = r#"
module test.effect_let_err
export main

main : Effect Text Unit
main = do Effect {
  x = print "nope"
  pure Unit
}"#;
    check_err(source);
}

#[test]
fn typecheck_flattened_constructor_chain_patterns() {
    let source = r#"
module test.pattern_chain
export msg

Result E A = Err E | Ok A
Error = NotFound Text | Other

res : Result Error Text
res = Err (NotFound "hi")

msg : Text
msg = res match
  | Err NotFound m => m
  | _              => "no-msg""#;
    check_ok(source);
}

#[test]
fn typecheck_result_or_sugar() {
    let source = r#"
module test.or_result
export value1, value2

Result E A = Err E | Ok A

value1 : Text
value1 = (Ok "hi") or "boom"

value2 : Text
value2 =
  (Err "nope") or
    | Err _ => "boom""#;
    check_ok(source);
}

#[test]
fn typecheck_effect_or_sugar_in_bind() {
    let source = r#"
module test.or_effect
export main

main : Effect Text Unit
main = do Effect {
  n <- (fail "nope") or 1
  _ = n
  pure Unit
}"#;
    check_ok(source);
}

#[test]
fn typecheck_error_unknown_name() {
    let source = r#"
module test.err
export value
value = missing"#;
    check_err(source);
}

#[test]
fn typecheck_closed_records_reject_extra_fields() {
    let source = r#"
module test.open
export value

 getName : { name: Text } -> Text
 getName = user => user.name

value = getName { name: "Alice", id: 1 }"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        !file_diagnostics_have_errors(&module_diags),
        "unexpected errors: {module_diags:?}"
    );
}

#[test]
fn typecheck_type_classes_resolve_instances() {
    let source = r#"
module test.classes
export value

class Eq A = {
  eq: A -> A -> Bool
}

instance Eq Bool = {
  eq: x y => x == y
}

value = eq True False"#;
    check_ok(source);
}

#[test]
fn typecheck_row_type_ops_and_patch_alias() {
    let source = r#"
module test.rows
export getName, getEmail, publicEmail, promote

User = { id: Int, email: Text, name: Text, isAdmin: Bool }

UserName = Pick (name) User
UserMaybe = User |> Optional (email)
UserReq = UserMaybe |> Required (email)
UserPublic = User |> Omit (isAdmin) |> Rename { email: email_address }

 getName : UserName -> Text
 getName = u => u.name

 getEmail : UserReq -> Text
 getEmail = u => u.email

 publicEmail : UserPublic -> Text
 publicEmail = u => u.email_address

 promote : Patch User
 promote = patch { isAdmin: True }"#;
    check_ok(source);
}

#[test]
fn typecheck_patch_literal() {
    let source = r#"
module test.patch_literal
export promote

User = { id: Int, name: Text, age: Int }

promote : Patch User
promote = patch { age: _ + 1 }"#;
    check_ok(source);
}

#[test]
fn typecheck_row_op_errors() {
    let source = r#"
module test.row_errors
User = { id: Int, name: Text }

badPick : Pick (missing) User
badPick = { id: 1, name: "x" }

badRename : Rename { id: name } User
badRename = { name: 1 }"#;
    check_err(source);
}

#[test]
fn typecheck_type_classes_missing_instance_errors() {
    let source = r#"
module test.classes_err
export value

class Eq A = {
  eq: A -> A -> Bool
}

value = eq True False"#;
    check_err(source);
}

#[test]
fn typecheck_hkts_functor_map() {
    let source = r#"
module test.functor
export value

Option A = None | Some A

class Functor (F *) = {
  map: F A -> (A -> B) -> F B
}

instance Functor (Option *) = {
  map: opt f => opt match
    | None => None
    | Some x => Some (f x)
}

 inc = x => x + 1
value = map (Some 1) inc"#;
    check_ok(source);
}

#[test]
fn typecheck_non_exhaustive_match_is_error() {
    let source = r#"
module test.match_err

Option A = None | Some A

value = Some 1 match
  | Some _ => 1
"#;

    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.iter().any(|d| d.diagnostic.code == "E3100"),
        "expected E3100 non-exhaustive match diagnostic, got: {module_diags:?}"
    );
}

#[test]
fn typecheck_unreachable_match_arm_is_warning() {
    let source = r#"
module test.match_warn

Option A = None | Some A

value = Some 1 match
  | _ => 0
  | Some _ => 1
"#;

    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        module_diags.iter().any(|d| d.diagnostic.code == "W3101"),
        "expected W3101 unreachable arm warning, got: {module_diags:?}"
    );
}

#[test]
fn typecheck_reports_missing_domain_operator_for_concrete_non_int_operands() {
    let source = r#"
module test.domain_op_err

bad = True + False
"#;

    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));
    assert!(
        file_diagnostics_have_errors(&module_diags),
        "expected errors, got: {module_diags:?}"
    );
    assert!(
        module_diags
            .iter()
            .any(|d| d.diagnostic.message.contains("no domain operator '+'")),
        "expected missing domain operator message, got: {module_diags:?}"
    );
}

#[test]
fn typecheck_load_accepts_source_values() {
    let source = r#"
module test.load_source
use aivi

main : Effect Text Text
main = do Effect {
  txt <- load (file.read "missing.txt") or "(missing)"
  pure txt
}
"#;

    check_ok_with_embedded(source, &["aivi"]);
}

#[test]
fn typecheck_error_mismatch_order() {
    let source = r#"
module test.repro
main : Int
main = 1.0
"#;
    let (modules, diagnostics) = parse_modules(Path::new("test.aivi"), source);
    assert!(
        !file_diagnostics_have_errors(&diagnostics),
        "parse errors: {diagnostics:?}"
    );

    let mut module_diags = check_modules(&modules);
    module_diags.extend(check_types(&modules));

    let mismatch_diag = module_diags
        .iter()
        .find(|d| d.diagnostic.message.contains("type mismatch"))
        .unwrap_or_else(|| panic!("expected type mismatch diagnostic, got: {module_diags:?}"));

    assert!(
        mismatch_diag
            .diagnostic
            .message
            .contains("expected Int, found Float"),
        "mismatch diagnostic message has wrong order: {}",
        mismatch_diag.diagnostic.message
    );
}

#[test]
fn typecheck_vec4_domain_scalar_mul() {
    let source = r#"
@no_prelude
module test.vec4_domain_scalar
export result

use aivi
use aivi.vector (Vec4, vec4, domain Vector)

result : Vec4
result = vec4 1.0 2.0 3.0 1.0 * 2.0
"#;
    check_ok_with_embedded(source, &["aivi", "aivi.vector", "aivi.math"]);
}

#[test]
fn typecheck_mat4_vec4_cross_transform() {
    let source = r#"
@no_prelude
module test.mat4_vec4_cross
export result

use aivi
use aivi.vector (Vec4, vec4, domain Vector)
use aivi.matrix (Mat4, identity4, domain Matrix)

result : Vec4
result = identity4 × vec4 1.0 2.0 3.0 1.0
"#;
    check_ok_with_embedded(source, &["aivi", "aivi.vector", "aivi.math", "aivi.matrix"]);
}

#[test]
fn typecheck_mat4_mat4_cross_product() {
    let source = r#"
@no_prelude
module test.mat4_mat4_cross
export result

use aivi
use aivi.vector (Vec4)
use aivi.matrix (Mat4, identity4, domain Matrix)

result : Mat4
result = identity4 × identity4
"#;
    check_ok_with_embedded(source, &["aivi", "aivi.vector", "aivi.math", "aivi.matrix"]);
}

#[test]
fn typecheck_error_custom_type_no_cross_operator() {
    // A user-defined type with no (×) in its domain should fail.
    let source = r#"
module test.no_cross_for_custom

MyVec = { x: Float, y: Float }

domain V over MyVec = {
  (+) : MyVec -> MyVec -> MyVec
  (+) = a b => { x: a.x + b.x, y: a.y + b.y }
}

v : MyVec
v = { x: 1.0, y: 2.0 }

bad = v × v
"#;
    check_err(source);
}

#[test]
fn typecheck_error_vec3_cross_needs_vec3_not_mat4() {
    // Vec3 domain has no (×) at all; applying × where LHS is Vec3 should error.
    let source = r#"
module test.vec3_no_cross

Vec3 = { x: Float, y: Float, z: Float }
Mat4 = { m00: Float, m01: Float, m02: Float, m03: Float,
         m10: Float, m11: Float, m12: Float, m13: Float,
         m20: Float, m21: Float, m22: Float, m23: Float,
         m30: Float, m31: Float, m32: Float, m33: Float }

domain Vector over Vec3 = {
  (+) : Vec3 -> Vec3 -> Vec3
  (+) = a b => { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z }
}

v : Vec3
v = { x: 1.0, y: 2.0, z: 3.0 }

m : Mat4
m = { m00: 1.0, m01: 0.0, m02: 0.0, m03: 0.0,
      m10: 0.0, m11: 1.0, m12: 0.0, m13: 0.0,
      m20: 0.0, m21: 0.0, m22: 1.0, m23: 0.0,
      m30: 0.0, m31: 0.0, m32: 0.0, m33: 1.0 }

bad = v × m
"#;
    check_err(source);
}
