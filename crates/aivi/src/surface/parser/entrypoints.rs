use std::path::Path;

use crate::cst::CstToken;
use crate::diagnostics::{
    Diagnostic, DiagnosticLabel, DiagnosticSeverity, FileDiagnostic, Position, Span,
};
use crate::lexer::{filter_tokens, lex, Token, TokenKind};

use super::ast::*;

pub fn parse_modules(path: &Path, content: &str) -> (Vec<Module>, Vec<FileDiagnostic>) {
    let (cst_tokens, lex_diags) = lex(content);
    let tokens = filter_tokens(&cst_tokens);
    let mut parser = Parser::new(tokens, path);
    let mut modules = parser.parse_modules();
    inject_prelude_imports(&mut modules);
    expand_domain_exports(&mut modules);
    expand_module_aliases(&mut modules);
    let mut decorator_diags = apply_native_decorators(&mut modules);
    decorator_diags.append(&mut apply_static_decorators(&mut modules));
    let mut diagnostics: Vec<FileDiagnostic> = lex_diags
        .into_iter()
        .map(|diag| FileDiagnostic {
            path: path.display().to_string(),
            diagnostic: diag,
        })
        .collect();
    diagnostics.append(&mut parser.diagnostics);
    diagnostics.append(&mut decorator_diags);
    (modules, diagnostics)
}

pub fn parse_modules_from_tokens(
    path: &Path,
    tokens: &[CstToken],
) -> (Vec<Module>, Vec<FileDiagnostic>) {
    let tokens = filter_tokens(tokens);
    let mut parser = Parser::new(tokens, path);
    let mut modules = parser.parse_modules();
    inject_prelude_imports(&mut modules);
    expand_domain_exports(&mut modules);
    (modules, parser.diagnostics)
}

fn inject_prelude_imports(modules: &mut [Module]) {
    for module in modules {
        if module.name.name == "aivi.prelude" {
            continue;
        }
        if module
            .annotations
            .iter()
            .any(|decorator| decorator.name.name == "no_prelude")
        {
            continue;
        }
        if module
            .uses
            .iter()
            .any(|use_decl| use_decl.module.name == "aivi.prelude")
        {
            continue;
        }
        let span = module.name.span.clone();
        module.uses.insert(
            0,
            UseDecl {
                module: SpannedName {
                    name: "aivi.prelude".into(),
                    span: span.clone(),
                },
                items: Vec::new(),
                span,
                wildcard: true,
                alias: None,
            },
        );
    }
}

fn expand_domain_exports(modules: &mut [Module]) {
    use std::collections::HashSet;

    for module in modules {
        let mut exported_values: HashSet<String> = module
            .exports
            .iter()
            .filter(|item| item.kind == crate::surface::ScopeItemKind::Value)
            .map(|item| item.name.name.clone())
            .collect();
        let exported_domains: HashSet<String> = module
            .exports
            .iter()
            .filter(|item| item.kind == crate::surface::ScopeItemKind::Domain)
            .map(|item| item.name.name.clone())
            .collect();
        let mut extra_exports = Vec::new();
        for item in &module.items {
            let ModuleItem::DomainDecl(domain) = item else {
                continue;
            };
            if !exported_domains.contains(&domain.name.name) {
                continue;
            }
            for domain_item in &domain.items {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        if exported_values.insert(def.name.name.clone()) {
                            extra_exports.push(crate::surface::ExportItem {
                                kind: crate::surface::ScopeItemKind::Value,
                                name: def.name.clone(),
                            });
                        }
                    }
                    DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                }
            }
        }
        module.exports.extend(extra_exports);
    }
}

fn expand_module_aliases(modules: &mut [Module]) {
    use std::collections::HashMap;

    fn rewrite_type_expr(expr: TypeExpr, aliases: &HashMap<String, String>) -> TypeExpr {
        match expr {
            TypeExpr::Name(mut name) => {
                if let Some((head, tail)) = name.name.split_once('.') {
                    if aliases.contains_key(head) {
                        name.name = tail.to_string();
                    }
                }
                TypeExpr::Name(name)
            }
            TypeExpr::And { items, span } => TypeExpr::And {
                items: items
                    .into_iter()
                    .map(|item| rewrite_type_expr(item, aliases))
                    .collect(),
                span,
            },
            TypeExpr::Apply { base, args, span } => TypeExpr::Apply {
                base: Box::new(rewrite_type_expr(*base, aliases)),
                args: args
                    .into_iter()
                    .map(|arg| rewrite_type_expr(arg, aliases))
                    .collect(),
                span,
            },
            TypeExpr::Func {
                params,
                result,
                span,
            } => TypeExpr::Func {
                params: params
                    .into_iter()
                    .map(|p| rewrite_type_expr(p, aliases))
                    .collect(),
                result: Box::new(rewrite_type_expr(*result, aliases)),
                span,
            },
            TypeExpr::Record { fields, span } => TypeExpr::Record {
                fields: fields
                    .into_iter()
                    .map(|(label, ty)| (label, rewrite_type_expr(ty, aliases)))
                    .collect(),
                span,
            },
            TypeExpr::Tuple { items, span } => TypeExpr::Tuple {
                items: items
                    .into_iter()
                    .map(|item| rewrite_type_expr(item, aliases))
                    .collect(),
                span,
            },
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => expr,
        }
    }

    fn rewrite_expr(expr: Expr, aliases: &HashMap<String, String>) -> Expr {
        match expr {
            Expr::Suffixed { base, suffix, span } => Expr::Suffixed {
                base: Box::new(rewrite_expr(*base, aliases)),
                suffix,
                span,
            },
            Expr::UnaryNeg { expr, span } => Expr::UnaryNeg {
                expr: Box::new(rewrite_expr(*expr, aliases)),
                span,
            },
            Expr::FieldAccess { base, field, span } => {
                // Best-effort support for `use some.module as alias`.
                //
                // Historically we rewrote `alias.x` into `x` and relied on wildcard imports.
                // That loses disambiguation for colliding names (e.g. `load`) and diverges
                // from the spec's "modules are records" model.
                //
                // For now, rewrite `alias.x` into a qualified identifier `some.module.x`.
                // Later passes treat this as a normal identifier, and we also emit qualified
                // defs during lowering so codegen can resolve it.
                if let Expr::Ident(name) = *base.clone() {
                    if let Some(module) = aliases.get(name.name.as_str()) {
                        return Expr::Ident(SpannedName {
                            name: format!("{module}.{}", field.name).into(),
                            span: field.span,
                        });
                    }
                }
                Expr::FieldAccess {
                    base: Box::new(rewrite_expr(*base, aliases)),
                    field,
                    span,
                }
            }
            Expr::TextInterpolate { parts, span } => Expr::TextInterpolate {
                parts: parts
                    .into_iter()
                    .map(|part| match part {
                        TextPart::Text { .. } => part,
                        TextPart::Expr { expr, span } => TextPart::Expr {
                            expr: Box::new(rewrite_expr(*expr, aliases)),
                            span,
                        },
                    })
                    .collect(),
                span,
            },
            Expr::List { items, span } => Expr::List {
                items: items
                    .into_iter()
                    .map(|item| ListItem {
                        expr: rewrite_expr(item.expr, aliases),
                        spread: item.spread,
                        span: item.span,
                    })
                    .collect(),
                span,
            },
            Expr::Tuple { items, span } => Expr::Tuple {
                items: items
                    .into_iter()
                    .map(|item| rewrite_expr(item, aliases))
                    .collect(),
                span,
            },
            Expr::Record { fields, span } => Expr::Record {
                fields: fields
                    .into_iter()
                    .map(|field| RecordField {
                        path: field.path,
                        value: rewrite_expr(field.value, aliases),
                        spread: field.spread,
                        span: field.span,
                    })
                    .collect(),
                span,
            },
            Expr::PatchLit { fields, span } => Expr::PatchLit {
                fields: fields
                    .into_iter()
                    .map(|field| RecordField {
                        path: field.path,
                        value: rewrite_expr(field.value, aliases),
                        spread: field.spread,
                        span: field.span,
                    })
                    .collect(),
                span,
            },
            Expr::Index { base, index, span } => Expr::Index {
                base: Box::new(rewrite_expr(*base, aliases)),
                index: Box::new(rewrite_expr(*index, aliases)),
                span,
            },
            Expr::Call { func, args, span } => Expr::Call {
                func: Box::new(rewrite_expr(*func, aliases)),
                args: args
                    .into_iter()
                    .map(|arg| rewrite_expr(arg, aliases))
                    .collect(),
                span,
            },
            Expr::Lambda { params, body, span } => Expr::Lambda {
                params,
                body: Box::new(rewrite_expr(*body, aliases)),
                span,
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => Expr::Match {
                scrutinee: scrutinee.map(|e| Box::new(rewrite_expr(*e, aliases))),
                arms: arms
                    .into_iter()
                    .map(|arm| MatchArm {
                        pattern: arm.pattern,
                        guard: arm.guard.map(|g| rewrite_expr(g, aliases)),
                        body: rewrite_expr(arm.body, aliases),
                        span: arm.span,
                    })
                    .collect(),
                span,
            },
            Expr::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => Expr::If {
                cond: Box::new(rewrite_expr(*cond, aliases)),
                then_branch: Box::new(rewrite_expr(*then_branch, aliases)),
                else_branch: Box::new(rewrite_expr(*else_branch, aliases)),
                span,
            },
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => Expr::Binary {
                op,
                left: Box::new(rewrite_expr(*left, aliases)),
                right: Box::new(rewrite_expr(*right, aliases)),
                span,
            },
            Expr::Block { kind, items, span } => Expr::Block {
                kind,
                items: items
                    .into_iter()
                    .map(|item| match item {
                        BlockItem::Bind {
                            pattern,
                            expr,
                            span,
                        } => BlockItem::Bind {
                            pattern,
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::Let {
                            pattern,
                            expr,
                            span,
                        } => BlockItem::Let {
                            pattern,
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::Filter { expr, span } => BlockItem::Filter {
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::Yield { expr, span } => BlockItem::Yield {
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::Expr { expr, span } => BlockItem::Expr {
                            expr: rewrite_expr(expr, aliases),
                            span,
                        },
                        BlockItem::When { cond, effect, span } => BlockItem::When {
                            cond: rewrite_expr(cond, aliases),
                            effect: rewrite_expr(effect, aliases),
                            span,
                        },
                        BlockItem::Unless { cond, effect, span } => BlockItem::Unless {
                            cond: rewrite_expr(cond, aliases),
                            effect: rewrite_expr(effect, aliases),
                            span,
                        },
                        BlockItem::Given {
                            cond,
                            fail_expr,
                            span,
                        } => BlockItem::Given {
                            cond: rewrite_expr(cond, aliases),
                            fail_expr: rewrite_expr(fail_expr, aliases),
                            span,
                        },
                        BlockItem::On {
                            transition,
                            handler,
                            span,
                        } => BlockItem::On {
                            transition: rewrite_expr(transition, aliases),
                            handler: rewrite_expr(handler, aliases),
                            span,
                        },
                    })
                    .collect(),
                span,
            },
            Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {
                expr
            }
        }
    }

    for module in modules {
        let mut aliases: HashMap<String, String> = HashMap::new();
        for use_decl in &module.uses {
            if let Some(alias) = &use_decl.alias {
                aliases.insert(alias.name.clone(), use_decl.module.name.clone());
            }
        }
        if aliases.is_empty() {
            continue;
        }

        for item in module.items.iter_mut() {
            match item {
                ModuleItem::TypeSig(sig) => {
                    sig.ty = rewrite_type_expr(sig.ty.clone(), &aliases);
                }
                ModuleItem::TypeAlias(alias) => {
                    alias.aliased = rewrite_type_expr(alias.aliased.clone(), &aliases);
                }
                ModuleItem::TypeDecl(decl) => {
                    for ctor in &mut decl.constructors {
                        ctor.args = ctor
                            .args
                            .iter()
                            .cloned()
                            .map(|arg| rewrite_type_expr(arg, &aliases))
                            .collect();
                    }
                }
                ModuleItem::Def(def) => {
                    def.expr = rewrite_expr(def.expr.clone(), &aliases);
                }
                ModuleItem::DomainDecl(domain) => {
                    domain.over = rewrite_type_expr(domain.over.clone(), &aliases);
                    for domain_item in domain.items.iter_mut() {
                        match domain_item {
                            DomainItem::TypeSig(sig) => {
                                sig.ty = rewrite_type_expr(sig.ty.clone(), &aliases);
                            }
                            DomainItem::TypeAlias(decl) => {
                                for ctor in &mut decl.constructors {
                                    ctor.args = ctor
                                        .args
                                        .iter()
                                        .cloned()
                                        .map(|arg| rewrite_type_expr(arg, &aliases))
                                        .collect();
                                }
                            }
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                def.expr = rewrite_expr(def.expr.clone(), &aliases);
                            }
                        }
                    }
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in instance.defs.iter_mut() {
                        def.expr = rewrite_expr(def.expr.clone(), &aliases);
                    }
                }
                ModuleItem::ClassDecl(class_decl) => {
                    for member in class_decl.members.iter_mut() {
                        member.ty = rewrite_type_expr(member.ty.clone(), &aliases);
                    }
                    class_decl.supers = class_decl
                        .supers
                        .iter()
                        .cloned()
                        .map(|super_expr| rewrite_type_expr(super_expr, &aliases))
                        .collect();
                }
                ModuleItem::MachineDecl(_) => {}
            }
        }
    }
}

fn apply_static_decorators(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    fn has_decorator(decorators: &[Decorator], name: &str) -> bool {
        decorators
            .iter()
            .any(|decorator| decorator.name.name == name)
    }

    fn emit_diag(
        module_path: &str,
        out: &mut Vec<FileDiagnostic>,
        code: &str,
        message: String,
        span: Span,
    ) {
        out.push(FileDiagnostic {
            path: module_path.to_string(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity: DiagnosticSeverity::Error,
                message,
                span,
                labels: Vec::new(),
            },
        });
    }

    fn apply_static_to_def(
        module_path: &str,
        base_dir: &std::path::Path,
        is_static: bool,
        def: &mut Def,
        out: &mut Vec<FileDiagnostic>,
    ) {
        if !is_static {
            return;
        }
        if !def.params.is_empty() {
            emit_diag(
                module_path,
                out,
                "E1514",
                "`@static` can only be applied to value definitions (no parameters)".to_string(),
                def.span.clone(),
            );
            return;
        }

        // Compile-time evaluation for deterministic source calls.
        let original_span = expr_span(&def.expr);
        let expr = def.expr.clone();
        let Expr::Call { func, args, .. } = &expr else {
            // `@static` is allowed on any value definition; compile-time evaluation is best-effort.
            return;
        };
        let (base_name, field_name) = match func.as_ref() {
            Expr::FieldAccess { base, field, .. } => match base.as_ref() {
                Expr::Ident(name) => (Some(name.name.as_str()), Some(field.name.as_str())),
                _ => (None, None),
            },
            _ => (None, None),
        };
        if args.len() == 1 {
            if base_name == Some("env") && field_name == Some("get") {
                let Some(Expr::Literal(Literal::String { text: key, .. })) = args.first() else {
                    return;
                };
                let value = std::env::var(key).unwrap_or_default();
                def.expr = Expr::Literal(Literal::String {
                    text: value,
                    span: original_span,
                });
                return;
            }
            if base_name == Some("file") && matches!(field_name, Some("read" | "json" | "csv")) {
                let Some(Expr::Literal(Literal::String { text: rel, .. })) = args.first() else {
                    return;
                };
                // Try source-relative first, then fall back to CWD-relative (workspace root).
                let source_relative = base_dir.join(rel);
                let cwd_relative = std::path::PathBuf::from(rel);
                let (full_path, contents) = match std::fs::read_to_string(&source_relative) {
                    Ok(c) => (source_relative, c),
                    Err(_) => match std::fs::read_to_string(&cwd_relative) {
                        Ok(c) => (cwd_relative, c),
                        Err(err) => {
                            emit_diag(
                                module_path,
                                out,
                                "E1515",
                                format!(
                                    "`@static` failed to read {} (also tried {}): {}",
                                    source_relative.display(),
                                    cwd_relative.display(),
                                    err
                                ),
                                original_span,
                            );
                            return;
                        }
                    },
                };
                let _digest = {
                    use sha2::{Digest, Sha256};
                    let mut hasher = Sha256::new();
                    hasher.update(contents.as_bytes());
                    hasher.finalize()
                };
                match field_name {
                    Some("read") => {
                        def.expr = Expr::Literal(Literal::String {
                            text: contents,
                            span: original_span,
                        });
                    }
                    Some("json") => {
                        let parsed = match serde_json::from_str::<serde_json::Value>(&contents) {
                            Ok(value) => value,
                            Err(err) => {
                                emit_diag(
                                    module_path,
                                    out,
                                    "E1516",
                                    format!(
                                        "`@static` failed to parse JSON from {}: {}",
                                        full_path.display(),
                                        err
                                    ),
                                    original_span,
                                );
                                return;
                            }
                        };
                        def.expr = json_to_expr(&parsed, &original_span);
                    }
                    Some("csv") => match csv_to_expr(&contents, &original_span) {
                        Ok(expr) => def.expr = expr,
                        Err(err) => {
                            emit_diag(
                                module_path,
                                out,
                                "E1517",
                                format!(
                                    "`@static` failed to parse CSV from {}: {}",
                                    full_path.display(),
                                    err
                                ),
                                original_span,
                            );
                        }
                    },
                    _ => {}
                }
            }
        }
    }

    let mut diags = Vec::new();
    for module in modules {
        let module_path = module.path.clone();
        let base_dir = std::path::Path::new(&module_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();

        let mut static_sigs = std::collections::HashSet::<String>::new();
        for item in &module.items {
            let ModuleItem::TypeSig(sig) = item else {
                continue;
            };
            if has_decorator(&sig.decorators, "static") {
                static_sigs.insert(sig.name.name.clone());
            }
        }
        for item in &mut module.items {
            match item {
                ModuleItem::Def(def) => {
                    let is_static = has_decorator(&def.decorators, "static")
                        || static_sigs.contains(&def.name.name);
                    apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags)
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &mut instance.defs {
                        let is_static = has_decorator(&def.decorators, "static");
                        apply_static_to_def(&module_path, &base_dir, is_static, def, &mut diags);
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &mut domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let is_static = has_decorator(&def.decorators, "static");
                                apply_static_to_def(
                                    &module_path,
                                    &base_dir,
                                    is_static,
                                    def,
                                    &mut diags,
                                );
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                ModuleItem::TypeSig(_)
                | ModuleItem::TypeDecl(_)
                | ModuleItem::TypeAlias(_)
                | ModuleItem::ClassDecl(_)
                | ModuleItem::MachineDecl(_) => {}
            }
        }
    }
    diags
}

fn apply_native_decorators(modules: &mut [Module]) -> Vec<FileDiagnostic> {
    fn native_target(decorators: &[Decorator]) -> Option<String> {
        decorators.iter().find_map(|decorator| {
            if decorator.name.name != "native" {
                return None;
            }
            match decorator.arg.as_ref() {
                Some(Expr::Literal(Literal::String { text, .. })) => Some(text.clone()),
                _ => None,
            }
        })
    }

    fn emit_diag(
        module_path: &str,
        out: &mut Vec<FileDiagnostic>,
        code: &str,
        message: String,
        span: Span,
    ) {
        out.push(FileDiagnostic {
            path: module_path.to_string(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                severity: DiagnosticSeverity::Error,
                message,
                span,
                labels: Vec::new(),
            },
        });
    }

    fn native_target_expr(path: &str, span: &Span) -> Option<Expr> {
        let mut segments = path.split('.').filter(|seg| !seg.is_empty());
        let first = segments.next()?;
        if !is_valid_ident(first) {
            return None;
        }
        let mut expr = Expr::Ident(SpannedName {
            name: first.to_string(),
            span: span.clone(),
        });
        for seg in segments {
            if !is_valid_ident(seg) {
                return None;
            }
            expr = Expr::FieldAccess {
                base: Box::new(expr),
                field: SpannedName {
                    name: seg.to_string(),
                    span: span.clone(),
                },
                span: span.clone(),
            };
        }
        Some(expr)
    }

    fn is_valid_ident(seg: &str) -> bool {
        let mut chars = seg.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    }

    fn apply_native_to_def(
        module_path: &str,
        type_sigs: &std::collections::HashSet<String>,
        target: Option<String>,
        allow_in_context: bool,
        def: &mut Def,
        out: &mut Vec<FileDiagnostic>,
    ) {
        let Some(path) = target else {
            return;
        };
        if !allow_in_context {
            emit_diag(
                module_path,
                out,
                "E1526",
                "`@native` is only supported on top-level module definitions".to_string(),
                def.span.clone(),
            );
            return;
        }
        if !type_sigs.contains(&def.name.name) {
            emit_diag(
                module_path,
                out,
                "E1526",
                format!(
                    "`@native` definition `{}` requires an explicit type signature",
                    def.name.name
                ),
                def.span.clone(),
            );
            return;
        }
        let Some(target_expr) = native_target_expr(&path, &def.span) else {
            emit_diag(
                module_path,
                out,
                "E1526",
                format!("`@native` target must be a dotted identifier path, got `{path}`"),
                def.span.clone(),
            );
            return;
        };
        let params: Vec<Pattern> = if !def.params.is_empty() {
            def.params.clone()
        } else if let Expr::Lambda { params, .. } = def.expr.clone() {
            params
        } else {
            Vec::new()
        };
        let mut args = Vec::with_capacity(params.len());
        for param in &params {
            match param {
                Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
                    args.push(Expr::Ident(name.clone()));
                }
                _ => {
                    emit_diag(
                        module_path,
                        out,
                        "E1526",
                        format!(
                            "`@native` definition `{}` only supports identifier parameters",
                            def.name.name
                        ),
                        def.span.clone(),
                    );
                    return;
                }
            }
        }
        def.expr = if args.is_empty() {
            target_expr
        } else {
            Expr::Call {
                func: Box::new(target_expr),
                args,
                span: def.span.clone(),
            }
        };
    }

    let mut diags = Vec::new();
    for module in modules {
        let module_path = module.path.clone();
        let mut native_sigs = std::collections::HashSet::<String>::new();
        let mut native_sig_targets = std::collections::HashMap::<String, String>::new();
        for item in &module.items {
            let ModuleItem::TypeSig(sig) = item else {
                continue;
            };
            if let Some(path) = native_target(&sig.decorators) {
                native_sigs.insert(sig.name.name.clone());
                native_sig_targets.insert(sig.name.name.clone(), path);
            }
        }
        for item in &mut module.items {
            match item {
                ModuleItem::Def(def) => {
                    let target = native_target(&def.decorators)
                        .or_else(|| native_sig_targets.get(&def.name.name).cloned());
                    apply_native_to_def(&module_path, &native_sigs, target, true, def, &mut diags)
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in &mut instance.defs {
                        let target = native_target(&def.decorators);
                        apply_native_to_def(
                            &module_path,
                            &native_sigs,
                            target,
                            false,
                            def,
                            &mut diags,
                        );
                    }
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in &mut domain.items {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let target = native_target(&def.decorators);
                                apply_native_to_def(
                                    &module_path,
                                    &native_sigs,
                                    target,
                                    false,
                                    def,
                                    &mut diags,
                                );
                            }
                            DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                        }
                    }
                }
                ModuleItem::TypeSig(_)
                | ModuleItem::TypeDecl(_)
                | ModuleItem::TypeAlias(_)
                | ModuleItem::ClassDecl(_)
                | ModuleItem::MachineDecl(_) => {}
            }
        }
    }
    diags
}
fn spanned_name(name: &str, span: &Span) -> SpannedName {
    SpannedName {
        name: name.to_string(),
        span: span.clone(),
    }
}

fn json_to_expr(value: &serde_json::Value, span: &Span) -> Expr {
    match value {
        serde_json::Value::Null => Expr::Ident(spanned_name("None", span)),
        serde_json::Value::Bool(value) => Expr::Literal(Literal::Bool {
            value: *value,
            span: span.clone(),
        }),
        serde_json::Value::Number(number) => Expr::Literal(Literal::Number {
            text: number.to_string(),
            span: span.clone(),
        }),
        serde_json::Value::String(text) => Expr::Literal(Literal::String {
            text: text.clone(),
            span: span.clone(),
        }),
        serde_json::Value::Array(items) => Expr::List {
            items: items
                .iter()
                .map(|item| ListItem {
                    expr: json_to_expr(item, span),
                    spread: false,
                    span: span.clone(),
                })
                .collect(),
            span: span.clone(),
        },
        serde_json::Value::Object(object) => Expr::Record {
            fields: object
                .iter()
                .map(|(key, value)| RecordField {
                    spread: false,
                    path: vec![PathSegment::Field(spanned_name(key, span))],
                    value: json_to_expr(value, span),
                    span: span.clone(),
                })
                .collect(),
            span: span.clone(),
        },
    }
}

fn csv_to_expr(raw: &str, span: &Span) -> Result<Expr, csv::Error> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(raw.as_bytes());
    let headers = reader
        .headers()?
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for row in reader.records() {
        let row = row?;
        let mut fields = Vec::new();
        for (idx, value) in row.iter().enumerate() {
            let key = headers
                .get(idx)
                .cloned()
                .unwrap_or_else(|| format!("col{idx}"));
            fields.push(RecordField {
                spread: false,
                path: vec![PathSegment::Field(spanned_name(&key, span))],
                value: Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                }),
                span: span.clone(),
            });
        }
        items.push(ListItem {
            expr: Expr::Record {
                fields,
                span: span.clone(),
            },
            spread: false,
            span: span.clone(),
        });
    }
    Ok(Expr::List {
        items,
        span: span.clone(),
    })
}
