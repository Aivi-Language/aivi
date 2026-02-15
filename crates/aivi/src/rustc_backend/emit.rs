
fn emit_binary(op: &str, left: String, right: String) -> String {
    match op {
        "+" | "-" | "*" | "/" => {
            let template = r#"({LEFT}).and_then(|l| ({RIGHT}).and_then(|r| match (l, r) {
        (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a <OP> b)),
        (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a <OP> b)),
        (Value::Int(a), Value::Float(b)) => Ok(Value::Float((a as f64) <OP> b)),
        (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a <OP> (b as f64))),
        _ => Err("unsupported operands for binary op".to_string()),
    }))"#;
            template
                .replace("{LEFT}", &left)
                .replace("{RIGHT}", &right)
                .replace("<OP>", op)
        }
        _ => format!(
            "({left}).and_then(|_| ({right}).and_then(|_| Err(\"unsupported binary op\".to_string())))"
        ),
    }
}

fn emit_block(
    kind: RustIrBlockKind,
    items: &[RustIrBlockItem],
    indent: usize,
) -> Result<String, AiviError> {
    match kind {
        RustIrBlockKind::Plain => emit_plain_block(items, indent),
        RustIrBlockKind::Effect => emit_effect_block(items, indent),
        RustIrBlockKind::Generate | RustIrBlockKind::Resource => Err(AiviError::Codegen(
            "generate/resource blocks are not supported by the rustc backend yet".to_string(),
        )),
    }
}

fn emit_plain_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind = "    ".repeat(indent);
    let ind2 = "    ".repeat(indent + 1);
    let mut s = String::new();
    s.push_str("Ok({\n");
    if items.is_empty() {
        s.push_str(&ind2);
        s.push_str("Value::Unit\n");
        s.push_str(&ind);
        s.push_str("})");
        return Ok(s);
    }

    for item in &items[..items.len() - 1] {
        match item {
            RustIrBlockItem::Bind { pattern, expr } => match pattern {
                crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
                }
                crate::rust_ir::RustIrPattern::Var { name, .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
                crate::rust_ir::RustIrPattern::At { name, pattern, .. }
                    if matches!(**pattern, crate::rust_ir::RustIrPattern::Wildcard { .. }) =>
                {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
                _ => {
                    return Err(AiviError::Codegen(
                        "only wildcard/var patterns are supported in block binds".to_string(),
                    ))
                }
            },
            RustIrBlockItem::Expr { expr } => {
                s.push_str(&ind2);
                s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
            }
            RustIrBlockItem::Filter { .. }
            | RustIrBlockItem::Yield { .. }
            | RustIrBlockItem::Recurse { .. } => {
                return Err(AiviError::Codegen(
                    "filter/yield/recurse are not supported by the rustc backend yet".to_string(),
                ))
            }
        }
    }

    match items.last().unwrap() {
        RustIrBlockItem::Bind { pattern, expr } => {
            match pattern {
                crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!("let _ = ({} )?;\n", emit_expr(expr, indent + 1)?));
                }
                crate::rust_ir::RustIrPattern::Var { name, .. } => {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
                crate::rust_ir::RustIrPattern::At { name, pattern, .. }
                    if matches!(**pattern, crate::rust_ir::RustIrPattern::Wildcard { .. }) =>
                {
                    s.push_str(&ind2);
                    s.push_str(&format!(
                        "let {} = ({} )?;\n",
                        rust_local_name(name),
                        emit_expr(expr, indent + 1)?
                    ));
                }
                _ => {
                    return Err(AiviError::Codegen(
                        "only wildcard/var patterns are supported in block binds".to_string(),
                    ))
                }
            }
            s.push_str(&ind2);
            s.push_str("Value::Unit\n");
        }
        RustIrBlockItem::Expr { expr } => {
            s.push_str(&ind2);
            s.push_str(&format!("({} )?\n", emit_expr(expr, indent + 1)?));
        }
        RustIrBlockItem::Filter { .. }
        | RustIrBlockItem::Yield { .. }
        | RustIrBlockItem::Recurse { .. } => {
            return Err(AiviError::Codegen(
                "filter/yield/recurse are not supported by the rustc backend yet".to_string(),
            ))
        }
    }

    s.push_str(&ind);
    s.push_str("})");
    Ok(s)
}

fn emit_effect_block(items: &[RustIrBlockItem], indent: usize) -> Result<String, AiviError> {
    let ind2 = "    ".repeat(indent + 1);
    let ind3 = "    ".repeat(indent + 2);
    let mut s = String::new();
    s.push_str("Ok(Value::Effect(Rc::new(move || {\n");

    for (idx, item) in items.iter().enumerate() {
        let last = idx + 1 == items.len();
        match item {
            RustIrBlockItem::Bind { pattern, expr } => {
                let expr_code = emit_expr(expr, indent + 2)?;
                match pattern {
                    crate::rust_ir::RustIrPattern::Wildcard { .. } => {
                        s.push_str(&ind3);
                        s.push_str(&format!("let _ = run_effect(({} )?)?;\n", expr_code));
                    }
                    crate::rust_ir::RustIrPattern::Var { name, .. } => {
                        s.push_str(&ind3);
                        s.push_str(&format!(
                            "let {} = run_effect(({} )?)?;\n",
                            rust_local_name(name),
                            expr_code
                        ));
                    }
                    crate::rust_ir::RustIrPattern::At { name, pattern, .. }
                        if matches!(**pattern, crate::rust_ir::RustIrPattern::Wildcard { .. }) =>
                    {
                        s.push_str(&ind3);
                        s.push_str(&format!(
                            "let {} = run_effect(({} )?)?;\n",
                            rust_local_name(name),
                            expr_code
                        ));
                    }
                    _ => {
                        return Err(AiviError::Codegen(
                            "only wildcard/var patterns are supported in block binds".to_string(),
                        ))
                    }
                }
                if last {
                    s.push_str(&ind3);
                    s.push_str("Ok(Value::Unit)\n");
                }
            }
            RustIrBlockItem::Expr { expr } => {
                let expr_code = emit_expr(expr, indent + 2)?;
                if last {
                    s.push_str(&ind3);
                    s.push_str(&format!("run_effect(({} )?)\n", expr_code));
                } else {
                    s.push_str(&ind3);
                    s.push_str(&format!("let _ = run_effect(({} )?)?;\n", expr_code));
                }
            }
            RustIrBlockItem::Filter { .. }
            | RustIrBlockItem::Yield { .. }
            | RustIrBlockItem::Recurse { .. } => {
                return Err(AiviError::Codegen(
                    "filter/yield/recurse are not supported by the rustc backend yet".to_string(),
                ))
            }
        }
    }

    if items.is_empty() {
        s.push_str(&ind3);
        s.push_str("Ok(Value::Unit)\n");
    }

    s.push_str(&ind2);
    s.push_str("})))");
    Ok(s)
}

fn rust_local_name(name: &str) -> String {
    let mut s = sanitize_ident(name);
    if s.is_empty() {
        s = "_".to_string();
    }
    if is_rust_keyword(&s) {
        s = format!("v_{s}");
    }
    s
}

fn rust_global_fn_name(name: &str) -> String {
    format!("def_{}", rust_local_name(name))
}

fn sanitize_ident(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        let ok = ch == '_' || ch.is_ascii_alphanumeric();
        if ok {
            if i == 0 && ch.is_ascii_digit() {
                out.push('_');
            }
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn is_rust_keyword(ident: &str) -> bool {
    matches!(
        ident,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
    )
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}
