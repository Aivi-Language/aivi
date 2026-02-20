//! Typed expression emitter for the native Rust backend.
//!
//! When a definition's type is fully resolved (closed), this emitter produces unboxed Rust code
//! that avoids the `Value` enum entirely for primitives, arithmetic, field access, and direct
//! function calls — yielding significant performance gains for numeric/algorithmic code.
//!
//! When the type is `Dynamic` or missing, the emitter falls back to the existing `emit_expr`
//! path that boxes everything through `Value`.

use std::collections::{BTreeMap, HashMap};

use crate::rust_ir::cg_type::CgType;
use crate::rust_ir::{RustIrBlockItem, RustIrBlockKind, RustIrExpr, RustIrMatchArm, RustIrPattern};
use crate::AiviError;

use super::expr;
use super::utils::{collect_free_locals_in_expr, rust_global_fn_name, rust_local_name};
use std::collections::HashSet;

/// Try to emit a typed expression; if it cannot be typed, fall back to the Value path
/// (`emit_expr`) and unbox the result to the expected `CgType`.
///
/// This bridges the typed↔Value boundary so typed code can call into non-typed globals,
/// builtins, and complex expressions without giving up entirely.
fn emit_typed_or_unbox(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &mut TypedCtx,
    indent: usize,
) -> Result<Option<String>, AiviError> {
    // First try the fully typed path.
    if let Some(code) = emit_typed_expr(expr, ty, ctx, indent)? {
        return Ok(Some(code));
    }
    // Fall back to Value path + unbox.
    // Dynamic types can't be meaningfully unboxed to a typed representation.
    if matches!(ty, CgType::Dynamic) {
        return Ok(None);
    }

    // Any free variable in `expr` that is typed in `ctx` MUST be boxed first!
    let mut bound = Vec::new();
    let mut captured = std::collections::HashSet::new();
    crate::native_rust_backend::utils::collect_free_locals_in_expr(expr, &mut bound, &mut captured);

    let mut captured_vec: Vec<_> = captured.into_iter().collect();
    captured_vec.sort();

    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut boxing_code = String::new();
    for var in captured_vec {
        if let Some(var_ty) = ctx.locals.get(&var) {
            let rname = crate::native_rust_backend::utils::rust_local_name(&var);
            // We clone primitive variables so we can box them into `Value` without moving
            let boxed = var_ty.emit_box(&format!("{rname}.clone()"));
            boxing_code.push_str(&format!("{ind2}let {rname} = {boxed};\n"));
        }
    }

    let value_code = expr::emit_expr(expr, indent + 1)?;
    if boxing_code.is_empty() {
        Ok(Some(format!(
            "({})?",
            ty.emit_unbox(&format!("({value_code})?"))
        )))
    } else {
        Ok(Some(format!(
            "({})?",
            ty.emit_unbox(&format!(
                "({{\n{boxing_code}{ind2}({value_code})?\n{ind}}})"
            ))
        )))
    }
}

/// Context tracking the types of local variables during typed emission.
pub(super) struct TypedCtx {
    /// Local variable name → CgType
    locals: HashMap<String, CgType>,
    /// Global definition name → CgType
    globals: HashMap<String, CgType>,
}

impl TypedCtx {
    pub fn new(globals: HashMap<String, CgType>) -> Self {
        Self {
            locals: HashMap::new(),
            globals,
        }
    }

    fn lookup(&self, name: &str) -> Option<&CgType> {
        self.locals.get(name).or_else(|| self.globals.get(name))
    }

    fn with_local(&mut self, name: &str, ty: CgType) {
        self.locals.insert(name.to_string(), ty);
    }
}

/// Emit a typed expression that returns an unboxed Rust value of the given `CgType`.
///
/// Returns `None` if the expression cannot be emitted in typed mode (caller should fall back to
/// `emit_expr`).
pub(super) fn emit_typed_expr(
    expr: &RustIrExpr,
    ty: &CgType,
    ctx: &mut TypedCtx,
    indent: usize,
) -> Result<Option<String>, AiviError> {
    // If type is Dynamic, fall back immediately.
    if !ty.is_closed() {
        return Ok(None);
    }

    let result = match expr {
        // ── Literals ────────────────────────────────────────────────────
        RustIrExpr::LitNumber { text, .. } => match ty {
            CgType::Int => {
                if let Ok(v) = text.parse::<i64>() {
                    Some(format!("{v}_i64"))
                } else {
                    None
                }
            }
            CgType::Float => {
                if let Ok(v) = text.parse::<f64>() {
                    Some(format!("{v:?}_f64"))
                } else {
                    None
                }
            }
            _ => None,
        },
        RustIrExpr::LitBool { value, .. } => match ty {
            CgType::Bool => Some(format!("{value}")),
            _ => None,
        },
        RustIrExpr::LitString { text, .. } => match ty {
            CgType::Text => Some(format!("{text:?}.to_string()")),
            _ => None,
        },

        // ── Variables ───────────────────────────────────────────────────
        RustIrExpr::Local { name, .. } => {
            if let Some(local_ty) = ctx.lookup(name) {
                if local_ty == ty {
                    match ty {
                        // Primitives are Copy — no clone needed.
                        CgType::Int | CgType::Float | CgType::Bool | CgType::Unit => {
                            Some(rust_local_name(name))
                        }
                        // Non-Copy types need clone.
                        _ => Some(format!("{}.clone()", rust_local_name(name))),
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }
        RustIrExpr::Global { name, .. } => {
            if let Some(global_ty) = ctx.globals.get(name).cloned() {
                if global_ty == *ty {
                    Some(format!("{}_typed(rt)?", rust_global_fn_name(name)))
                } else {
                    None
                }
            } else {
                None
            }
        }
        RustIrExpr::ConstructorValue {
            name: ctor_name, ..
        } => {
            if let CgType::Adt {
                name: _,
                constructors,
            } = ty
            {
                if let Some((_, args)) = constructors.iter().find(|(n, _)| n == ctor_name) {
                    if args.is_empty() {
                        Some(format!(
                            "{}::{ctor_name}",
                            CgType::enum_name(
                                match ty {
                                    CgType::Adt { name, .. } => name,
                                    _ => unreachable!(),
                                },
                                constructors
                            )
                        ))
                    } else {
                        None // Functions/Multi-arg constructors need App or Call around them
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }

        // ── Binary operators ────────────────────────────────────────────
        RustIrExpr::Binary {
            op, left, right, ..
        } => emit_typed_binary(op, left, right, ty, ctx, indent)?,

        // ── If expression ───────────────────────────────────────────────
        RustIrExpr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            let cond_code = emit_typed_or_unbox(cond, &CgType::Bool, ctx, indent)?;
            let then_code = emit_typed_or_unbox(then_branch, ty, ctx, indent)?;
            let else_code = emit_typed_or_unbox(else_branch, ty, ctx, indent)?;
            match (cond_code, then_code, else_code) {
                (Some(c), Some(t), Some(e)) => Some(format!("if {c} {{ {t} }} else {{ {e} }}")),
                _ => None,
            }
        }

        // ── Lambda ──────────────────────────────────────────────────────
        RustIrExpr::Lambda { param, body, .. } => {
            if let CgType::Func(param_ty, ret_ty) = ty {
                let param_name = rust_local_name(param);
                let param_rust_ty = param_ty.rust_type();

                // Capture free variables
                let mut bound = vec![param.clone()];
                let mut captured: HashSet<String> = HashSet::new();
                collect_free_locals_in_expr(body, &mut bound, &mut captured);
                let mut captured: Vec<_> = captured.into_iter().collect();
                captured.sort();

                // Register param type
                ctx.with_local(param, *param_ty.clone());
                let body_code = emit_typed_or_unbox(body, ret_ty, ctx, indent + 1)?;

                if let Some(body_code) = body_code {
                    let ind = "    ".repeat(indent);
                    let ind2 = "    ".repeat(indent + 1);
                    let mut capture_lines = String::new();
                    for name in &captured {
                        let rn = rust_local_name(name);
                        capture_lines.push_str(&format!("{ind2}let {rn} = {rn}.clone();\n"));
                    }
                    // Lambdas must be Box::new(...) because CgType::Func maps to
                    // Box<dyn Fn(A, &mut aivi_native_runtime::Runtime) -> Result<B, RuntimeError>>.
                    Some(format!(
                        "{{\n{capture_lines}{ind2}Box::new(move |{param_name}: {param_rust_ty}, rt: &mut aivi_native_runtime::Runtime| -> Result<{ret_ty}, RuntimeError> {{\n{ind2}    Ok({body_code})\n{ind2}}})\n{ind}}}",
                        ret_ty = ret_ty.rust_type()
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        }

        // ── Function application ────────────────────────────────────────
        RustIrExpr::App { func, arg, .. } => {
            // Try to determine the function's type
            if let Some(func_ty) = infer_expr_type(func, ctx) {
                if let CgType::Func(param_ty, ret_ty) = &func_ty {
                    if ret_ty.as_ref() == ty {
                        let func_code = emit_typed_or_unbox(func, &func_ty, ctx, indent)?;
                        let arg_code = emit_typed_or_unbox(arg, param_ty, ctx, indent)?;
                        if let (Some(f), Some(a)) = (func_code, arg_code) {
                            return Ok(Some(format!("({f})({a}, rt)?")));
                        }
                    }
                }
            }
            None
        }

        // ── Multi-arg call ──────────────────────────────────────────────
        RustIrExpr::Call { func, args, .. } => {
            if let RustIrExpr::ConstructorValue {
                name: ctor_name, ..
            } = func.as_ref()
            {
                if let CgType::Adt {
                    name: _,
                    constructors,
                } = ty
                {
                    if let Some((_, ctor_args)) = constructors.iter().find(|(n, _)| n == ctor_name)
                    {
                        if args.len() == ctor_args.len() {
                            let mut param_types = Vec::new();
                            for arg in ctor_args {
                                param_types.push(arg.clone());
                            }
                            let mut rendered_args = Vec::new();
                            for (arg, pty) in args.iter().zip(param_types.iter()) {
                                if let Some(a) = emit_typed_or_unbox(arg, pty, ctx, indent + 1)? {
                                    rendered_args.push(a);
                                } else {
                                    return Ok(None);
                                }
                            }
                            return Ok(Some(format!(
                                "{}::{ctor_name}({})",
                                CgType::enum_name(
                                    match ty {
                                        CgType::Adt { name, .. } => name,
                                        _ => unreachable!(),
                                    },
                                    constructors
                                ),
                                rendered_args.join(", ")
                            )));
                        }
                    }
                }
                return Ok(None);
            }

            // For direct calls to known globals
            if let RustIrExpr::Global { name, .. } = func.as_ref() {
                if let Some(func_ty) = ctx.globals.get(name).cloned() {
                    // Unwrap chained Func types for the call args
                    let mut param_types = Vec::new();
                    let mut cur = &func_ty;
                    for _ in 0..args.len() {
                        if let CgType::Func(p, r) = cur {
                            param_types.push(p.as_ref().clone());
                            cur = r;
                        } else {
                            return Ok(None);
                        }
                    }
                    if cur == ty {
                        let mut rendered_args = Vec::new();
                        for (arg, pty) in args.iter().zip(param_types.iter()) {
                            if let Some(a) = emit_typed_or_unbox(arg, pty, ctx, indent + 1)? {
                                rendered_args.push(a);
                            } else {
                                return Ok(None);
                            }
                        }
                        let fn_name = format!("{}_typed", rust_global_fn_name(name));
                        // Chain calls: fn_typed(rt)?(arg1)?(arg2)?...
                        let mut code = format!("{fn_name}(rt)?");
                        for a in &rendered_args {
                            code = format!("({code})({a}, rt)?");
                        }
                        return Ok(Some(code));
                    }
                }
            }
            None
        }

        // ── Tuple ───────────────────────────────────────────────────────
        RustIrExpr::Tuple { items, .. } => {
            if let CgType::Tuple(item_types) = ty {
                if items.len() == item_types.len() {
                    let mut parts = Vec::new();
                    for (item, item_ty) in items.iter().zip(item_types.iter()) {
                        if let Some(code) = emit_typed_or_unbox(item, item_ty, ctx, indent)? {
                            parts.push(code);
                        } else {
                            return Ok(None);
                        }
                    }
                    Some(format!("({})", parts.join(", ")))
                } else {
                    None
                }
            } else {
                None
            }
        }

        // ── Record ──────────────────────────────────────────────────────
        RustIrExpr::Record { fields, .. } => {
            if let CgType::Record(field_types) = ty {
                // Record fields in CgType are sorted by name (BTreeMap).
                // We need to emit them in the same order.
                let mut field_exprs: BTreeMap<String, String> = BTreeMap::new();
                for field in fields {
                    if field.spread {
                        return Ok(None); // Spread requires dynamic handling
                    }
                    if field.path.len() != 1 {
                        return Ok(None);
                    }
                    if let crate::rust_ir::RustIrPathSegment::Field(name) = &field.path[0] {
                        if let Some(ft) = field_types.get(name) {
                            if let Some(code) = emit_typed_or_unbox(&field.value, ft, ctx, indent)?
                            {
                                field_exprs.insert(name.clone(), code);
                            } else {
                                return Ok(None);
                            }
                        } else {
                            return Ok(None);
                        }
                    } else {
                        return Ok(None);
                    }
                }
                // Emit as a tuple in sorted field order
                let parts: Vec<_> = field_types
                    .keys()
                    .map(|k| {
                        field_exprs
                            .get(k)
                            .cloned()
                            .unwrap_or_else(|| "Default::default()".to_string())
                    })
                    .collect();
                Some(format!("({})", parts.join(", ")))
            } else {
                None
            }
        }

        // ── Field access ────────────────────────────────────────────────
        RustIrExpr::FieldAccess { base, field, .. } => {
            if let Some(base_ty) = infer_expr_type(base, ctx) {
                if let CgType::Record(fields) = &base_ty {
                    // Find the field index in sorted field order
                    let field_index = fields.keys().position(|k| k == field);
                    if let Some(idx) = field_index {
                        if let Some(ft) = fields.get(field) {
                            if ft == ty {
                                if let Some(base_code) =
                                    emit_typed_or_unbox(base, &base_ty, ctx, indent)?
                                {
                                    return Ok(Some(format!("({base_code}).{idx}")));
                                }
                            }
                        }
                    }
                }
            }
            None
        }

        // ── Pattern matching ────────────────────────────────────────────
        RustIrExpr::Match {
            scrutinee, arms, ..
        } => emit_typed_match(scrutinee, arms, ty, ctx, indent)?,

        // ── Block ───────────────────────────────────────────────────────
        RustIrExpr::Block {
            block_kind, items, ..
        } => {
            if matches!(block_kind, RustIrBlockKind::Plain) {
                emit_typed_block(items, ty, ctx, indent)?
            } else {
                // Effect/Generate/Resource/Do blocks need the Value path
                None
            }
        }

        // ── List ────────────────────────────────────────────────────────
        RustIrExpr::List { items, .. } => {
            if let CgType::ListOf(elem_ty) = ty {
                let mut rendered = Vec::new();
                for item in items {
                    if item.spread {
                        return Ok(None);
                    }
                    if let Some(code) = emit_typed_or_unbox(&item.expr, elem_ty, ctx, indent)? {
                        rendered.push(code);
                    } else {
                        return Ok(None);
                    }
                }
                Some(format!("vec![{}]", rendered.join(", ")))
            } else {
                None
            }
        }

        // ── Everything else falls back to Value ─────────────────────────
        _ => None,
    };
    Ok(result)
}

/// Emit a typed binary operation.
fn emit_typed_binary(
    op: &str,
    left: &RustIrExpr,
    right: &RustIrExpr,
    ty: &CgType,
    ctx: &mut TypedCtx,
    indent: usize,
) -> Result<Option<String>, AiviError> {
    // Infer operand types from the result type and operator
    let (left_ty, right_ty) = match op {
        "+" | "-" | "*" | "/" | "%" => match ty {
            CgType::Int => (CgType::Int, CgType::Int),
            CgType::Float => {
                // Could be Float+Float or Int+Float — try Float first
                (CgType::Float, CgType::Float)
            }
            _ => return Ok(None),
        },
        "<" | "<=" | ">" | ">=" => {
            if ty != &CgType::Bool {
                return Ok(None);
            }
            // Try to infer operand type from the left expr
            if let Some(left_inferred) = infer_expr_type(left, ctx) {
                (left_inferred.clone(), left_inferred)
            } else {
                return Ok(None);
            }
        }
        "==" | "!=" => {
            if ty != &CgType::Bool {
                return Ok(None);
            }
            if let Some(left_inferred) = infer_expr_type(left, ctx) {
                (left_inferred.clone(), left_inferred)
            } else {
                return Ok(None);
            }
        }
        "&&" | "||" => {
            if ty != &CgType::Bool {
                return Ok(None);
            }
            (CgType::Bool, CgType::Bool)
        }
        _ => return Ok(None),
    };

    let left_code = emit_typed_or_unbox(left, &left_ty, ctx, indent)?;
    let right_code = emit_typed_or_unbox(right, &right_ty, ctx, indent)?;

    match (left_code, right_code) {
        (Some(l), Some(r)) => {
            let rust_op = match op {
                "+" => "+",
                "-" => "-",
                "*" => "*",
                "/" => "/",
                "%" => "%",
                "<" => "<",
                "<=" => "<=",
                ">" => ">",
                ">=" => ">=",
                "==" => "==",
                "!=" => "!=",
                "&&" => "&&",
                "||" => "||",
                _ => return Ok(None),
            };
            // Wrap in parens for precedence safety
            Ok(Some(format!("({l} {rust_op} {r})")))
        }
        _ => Ok(None),
    }
}

/// Emit a typed match expression.
fn emit_typed_match(
    scrutinee: &RustIrExpr,
    arms: &[RustIrMatchArm],
    result_ty: &CgType,
    ctx: &mut TypedCtx,
    indent: usize,
) -> Result<Option<String>, AiviError> {
    let scrut_ty = infer_expr_type(scrutinee, ctx);
    let scrut_ty = match scrut_ty {
        Some(t) if t.is_closed() => t,
        _ => return Ok(None),
    };

    let scrut_code = emit_typed_or_unbox(scrutinee, &scrut_ty, ctx, indent)?;
    let scrut_code = match scrut_code {
        Some(c) => c,
        None => return Ok(None),
    };

    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut out = String::new();
    out.push_str(&format!(
        "{{\n{ind2}let __scrut = {scrut_code};\n{ind2}match __scrut {{\n"
    ));

    for arm in arms {
        if arm.guard.is_some() {
            return Ok(None); // Guards need more complex handling
        }
        let (pat_code, bindings) = emit_typed_pattern(&arm.pattern, &scrut_ty)?;
        let pat_code = match pat_code {
            Some(p) => p,
            None => return Ok(None),
        };
        // Register bindings in context
        for (name, bty) in &bindings {
            ctx.with_local(name, bty.clone());
        }
        let body_code = emit_typed_or_unbox(&arm.body, result_ty, ctx, indent + 2)?;
        let body_code = match body_code {
            Some(b) => b,
            None => return Ok(None),
        };
        out.push_str(&format!("{ind2}    {pat_code} => {body_code},\n"));
    }
    out.push_str(&format!("{ind2}}}\n{ind}}}"));
    Ok(Some(out))
}

/// Emit a typed pattern, returning the Rust pattern string and any variable bindings with types.
fn emit_typed_pattern(
    pattern: &RustIrPattern,
    scrutinee_ty: &CgType,
) -> Result<(Option<String>, Vec<(String, CgType)>), AiviError> {
    match pattern {
        RustIrPattern::Wildcard { .. } => Ok((Some("_".to_string()), vec![])),
        RustIrPattern::Var { name, .. } => {
            let rust_name = rust_local_name(name);
            Ok((
                Some(rust_name.clone()),
                vec![(name.clone(), scrutinee_ty.clone())],
            ))
        }
        RustIrPattern::Literal { value, .. } => {
            let code = match value {
                crate::rust_ir::RustIrLiteral::Number(text) => {
                    if text.contains('.') {
                        format!("{text}_f64")
                    } else {
                        format!("{text}_i64")
                    }
                }
                crate::rust_ir::RustIrLiteral::Bool(v) => format!("{v}"),
                crate::rust_ir::RustIrLiteral::String(s) => {
                    // String matching needs a ref pattern
                    return Ok((Some(format!("ref __s if __s == {s:?}")), vec![]));
                }
                // Sigils and DateTimes can't be matched in typed mode
                crate::rust_ir::RustIrLiteral::Sigil { .. }
                | crate::rust_ir::RustIrLiteral::DateTime(_) => {
                    return Ok((None, vec![]));
                }
            };
            Ok((Some(code), vec![]))
        }
        RustIrPattern::Constructor {
            name: ctor_name,
            args,
            ..
        } => {
            if let CgType::Adt {
                name: _,
                constructors,
            } = scrutinee_ty
            {
                if let Some((_, ctor_types)) = constructors.iter().find(|(n, _)| n == ctor_name) {
                    if args.len() == ctor_types.len() {
                        if args.is_empty() {
                            return Ok((
                                Some(format!(
                                    "{}::{ctor_name}",
                                    CgType::enum_name(
                                        match scrutinee_ty {
                                            CgType::Adt { name, .. } => name,
                                            _ => unreachable!(),
                                        },
                                        constructors
                                    )
                                )),
                                vec![],
                            ));
                        }

                        let mut sub_pats = Vec::new();
                        let mut all_bindings = Vec::new();
                        for (arg_pat, arg_ty) in args.iter().zip(ctor_types.iter()) {
                            if let (Some(pat_code), bindings) = emit_typed_pattern(arg_pat, arg_ty)?
                            {
                                sub_pats.push(pat_code);
                                all_bindings.extend(bindings);
                            } else {
                                return Ok((None, vec![]));
                            }
                        }

                        return Ok((
                            Some(format!(
                                "{}::{ctor_name}({})",
                                CgType::enum_name(
                                    match scrutinee_ty {
                                        CgType::Adt { name, .. } => name,
                                        _ => unreachable!(),
                                    },
                                    constructors
                                ),
                                sub_pats.join(", ")
                            )),
                            all_bindings,
                        ));
                    }
                }
            }
            Ok((None, vec![]))
        }
        _ => Ok((None, vec![])), // Complex patterns fall back
    }
}

/// Emit a typed block.
fn emit_typed_block(
    items: &[RustIrBlockItem],
    result_ty: &CgType,
    ctx: &mut TypedCtx,
    indent: usize,
) -> Result<Option<String>, AiviError> {
    if items.is_empty() {
        return Ok(Some("()".to_string()));
    }

    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut out = String::new();
    out.push_str("{\n");

    for (i, item) in items.iter().enumerate() {
        let last = i + 1 == items.len();
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                // For binds, try to infer the expr type and emit typed
                if let RustIrPattern::Var { name, .. } = pattern {
                    if let Some(expr_ty) = infer_expr_type(expr, ctx) {
                        if let Some(code) = emit_typed_or_unbox(expr, &expr_ty, ctx, indent + 1)? {
                            let rust_name = rust_local_name(name);
                            out.push_str(&format!("{ind2}let {rust_name} = {code};\n"));
                            ctx.with_local(name, expr_ty);
                            continue;
                        }
                    }
                }
                return Ok(None);
            }
            RustIrBlockItem::Expr { expr } => {
                if last {
                    if let Some(code) = emit_typed_or_unbox(expr, result_ty, ctx, indent + 1)? {
                        out.push_str(&format!("{ind2}{code}\n"));
                    } else {
                        return Ok(None);
                    }
                } else {
                    // Non-last expressions: emit for side effects (Unit type)
                    if let Some(code) = emit_typed_or_unbox(expr, &CgType::Unit, ctx, indent + 1)? {
                        out.push_str(&format!("{ind2}{code};\n"));
                    } else {
                        return Ok(None);
                    }
                }
            }
            // Filter/Yield/Recurse are generator/effect constructs — fall back to Value
            RustIrBlockItem::Filter { .. }
            | RustIrBlockItem::Yield { .. }
            | RustIrBlockItem::Recurse { .. } => {
                return Ok(None);
            }
        }
    }
    out.push_str(&format!("{ind}}}"));
    Ok(Some(out))
}

/// Try to infer the CgType of an expression from context.
///
/// This is a lightweight inference — it only works for simple cases (variables, literals, known
/// globals). For complex expressions, returns `None`.
fn infer_expr_type(expr: &RustIrExpr, ctx: &TypedCtx) -> Option<CgType> {
    match expr {
        RustIrExpr::Local { name, .. } => ctx.lookup(name).cloned(),
        RustIrExpr::Global { name, .. } => ctx.globals.get(name).cloned(),
        RustIrExpr::LitNumber { text, .. } => {
            if text.contains('.') {
                Some(CgType::Float)
            } else {
                Some(CgType::Int)
            }
        }
        RustIrExpr::LitBool { .. } => Some(CgType::Bool),
        RustIrExpr::LitString { .. } => Some(CgType::Text),
        RustIrExpr::LitDateTime { .. } => Some(CgType::DateTime),
        RustIrExpr::Binary { op, left, .. } => {
            match op.as_str() {
                "+" | "-" | "*" | "/" | "%" => {
                    // Result type is same as operand type
                    infer_expr_type(left, ctx)
                }
                "<" | "<=" | ">" | ">=" | "==" | "!=" | "&&" | "||" => Some(CgType::Bool),
                _ => None,
            }
        }
        RustIrExpr::If { then_branch, .. } => infer_expr_type(then_branch, ctx),
        RustIrExpr::Tuple { items, .. } => {
            let types: Vec<_> = items
                .iter()
                .filter_map(|i| infer_expr_type(i, ctx))
                .collect();
            if types.len() == items.len() {
                Some(CgType::Tuple(types))
            } else {
                None
            }
        }
        RustIrExpr::FieldAccess { base, field, .. } => {
            if let Some(CgType::Record(fields)) = infer_expr_type(base, ctx) {
                fields.get(field).cloned()
            } else {
                None
            }
        }
        RustIrExpr::App { func, .. } => {
            if let Some(CgType::Func(_, ret)) = infer_expr_type(func, ctx) {
                Some(*ret)
            } else {
                None
            }
        }
        _ => None,
    }
}
