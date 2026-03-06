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
    expand_type_constructor_exports(&mut modules);
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
    expand_type_constructor_exports(&mut modules);
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

fn expand_type_constructor_exports(modules: &mut [Module]) {
    use std::collections::HashSet;

    for module in modules {
        let mut exported_values: HashSet<String> = module
            .exports
            .iter()
            .filter(|item| item.kind == crate::surface::ScopeItemKind::Value)
            .map(|item| item.name.name.clone())
            .collect();
        let mut extra_exports = Vec::new();

        for item in &module.items {
            let ModuleItem::TypeDecl(type_decl) = item else {
                continue;
            };
            if !exported_values.contains(&type_decl.name.name) {
                continue;
            }
            for ctor in &type_decl.constructors {
                if exported_values.insert(ctor.name.name.clone()) {
                    extra_exports.push(crate::surface::ExportItem {
                        kind: crate::surface::ScopeItemKind::Value,
                        name: ctor.name.clone(),
                    });
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
            TypeExpr::CapabilityClause {
                base,
                capabilities,
                span,
            } => TypeExpr::CapabilityClause {
                base: Box::new(rewrite_type_expr(*base, aliases)),
                capabilities,
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
                            name: format!("{module}.{}", field.name),
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
            Expr::Mock { substitutions, body, span } => {
                let substitutions = substitutions
                    .into_iter()
                    .map(|mut sub| {
                        sub.value = sub.value.map(|v| rewrite_expr(v, aliases));
                        sub
                    })
                    .collect();
                Expr::Mock {
                    substitutions,
                    body: Box::new(rewrite_expr(*body, aliases)),
                    span,
                }
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

/// Resolve wildcard-imported names to their module-qualified forms.
///
/// When a module has `use aivi.database` and references `load`, this pass rewrites the bare
/// `load` identifier to `aivi.database.load`.  The qualified form later flows through HIR →
/// Kernel → RustIR without being captured by `resolve_builtin`, which only recognises short
/// names.
///
/// **Must** be called after `expand_domain_exports`, `expand_type_constructor_exports` and
/// `expand_module_aliases` so that export lists and alias rewrites are already finalised.
///
/// **Must** receive *all* modules (stdlib + user) so the export index is complete.
pub fn resolve_import_names(modules: &mut [Module]) {
    use std::collections::{HashMap, HashSet};

    // 1. Build an export index: module_name → set of value names that can be imported.
    //    Only include names that the module actually DEFINES (has a `Def` for).
    //    Names that are merely re-exported (e.g. builtins from the root `aivi` module)
    //    are excluded — they are resolved by the runtime's `resolve_builtin` instead.
    let mut export_index: HashMap<String, HashSet<String>> = HashMap::new();
    for module in modules.iter() {
        let defined_names: HashSet<String> = module
            .items
            .iter()
            .flat_map(|item| match item {
                ModuleItem::Def(def) => vec![def.name.name.clone()],
                ModuleItem::DomainDecl(domain) => domain
                    .items
                    .iter()
                    .filter_map(|di| match di {
                        crate::surface::DomainItem::Def(def)
                        | crate::surface::DomainItem::LiteralDef(def) => {
                            Some(def.name.name.clone())
                        }
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            })
            .collect();
        let exported_values: HashSet<String> = module
            .exports
            .iter()
            .filter(|e| e.kind == ScopeItemKind::Value)
            .map(|e| e.name.name.clone())
            .collect();
        // Collect domain member names from exported domains.
        let domain_member_names: HashSet<String> = module
            .exports
            .iter()
            .filter(|e| e.kind == ScopeItemKind::Domain)
            .flat_map(|e| {
                module
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        ModuleItem::DomainDecl(domain) if domain.name.name == e.name.name => {
                            Some(domain.items.iter().filter_map(|di| match di {
                                crate::surface::DomainItem::Def(def)
                                | crate::surface::DomainItem::LiteralDef(def) => {
                                    Some(def.name.name.clone())
                                }
                                _ => None,
                            }))
                        }
                        _ => None,
                    })
                    .flatten()
            })
            .collect();
        let all_exported: HashSet<String> = exported_values
            .into_iter()
            .chain(domain_member_names)
            .collect();
        let names = if all_exported.is_empty() {
            // No export list → all defs are public.
            defined_names
        } else {
            // Intersection: only export names that have actual definitions.
            all_exported
                .into_iter()
                .filter(|name| defined_names.contains(name))
                .collect()
        };
        export_index.insert(module.name.name.clone(), names);
    }

    // 2. For each module, compute which bare names should be qualified using the
    //    shared `compute_import_pairs` helper.
    for module in modules.iter_mut() {
        // Collect module-local def names (they shadow imports).
        let local_defs: HashSet<String> = module
            .items
            .iter()
            .filter_map(|item| match item {
                ModuleItem::Def(def) => Some(def.name.name.clone()),
                _ => None,
            })
            .collect();

        let import_map = super::compute_import_pairs(&module.uses, &export_index, &local_defs);

        if import_map.is_empty() {
            continue;
        }

        // 3. Rewrite expressions in this module's items.
        for item in module.items.iter_mut() {
            match item {
                ModuleItem::Def(def) => {
                    // Def params introduce local bindings that shadow imports.
                    let mut scope: HashSet<String> = HashSet::new();
                    for param in &def.params {
                        collect_pattern_names(param, &mut scope);
                    }
                    def.expr = qualify_expr(def.expr.clone(), &import_map, &scope);
                }
                ModuleItem::DomainDecl(domain) => {
                    for domain_item in domain.items.iter_mut() {
                        match domain_item {
                            DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                let mut scope: HashSet<String> = HashSet::new();
                                for param in &def.params {
                                    collect_pattern_names(param, &mut scope);
                                }
                                def.expr = qualify_expr(def.expr.clone(), &import_map, &scope);
                            }
                            DomainItem::TypeSig(_) | DomainItem::TypeAlias(_) => {}
                        }
                    }
                }
                ModuleItem::InstanceDecl(instance) => {
                    for def in instance.defs.iter_mut() {
                        let mut scope: HashSet<String> = HashSet::new();
                        for param in &def.params {
                            collect_pattern_names(param, &mut scope);
                        }
                        def.expr = qualify_expr(def.expr.clone(), &import_map, &scope);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Collect all names bound by a pattern (for scope tracking).
fn collect_pattern_names(pattern: &Pattern, names: &mut std::collections::HashSet<String>) {
    match pattern {
        Pattern::Ident(name) | Pattern::SubjectIdent(name) => {
            names.insert(name.name.clone());
        }
        Pattern::At {
            name,
            pattern: inner,
            ..
        } => {
            names.insert(name.name.clone());
            collect_pattern_names(inner, names);
        }
        Pattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_names(arg, names);
            }
        }
        Pattern::Tuple { items, .. }
        | Pattern::List {
            items, rest: None, ..
        } => {
            for item in items {
                collect_pattern_names(item, names);
            }
        }
        Pattern::List {
            items,
            rest: Some(rest),
            ..
        } => {
            for item in items {
                collect_pattern_names(item, names);
            }
            collect_pattern_names(rest, names);
        }
        Pattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_names(&field.pattern, names);
            }
        }
        Pattern::Wildcard(_) | Pattern::Literal(_) => {}
    }
}

/// Split a number literal text into (number_part, suffix_part).
/// E.g. "30s" → Some(("30", "s")), "100ms" → Some(("100", "ms")), "42" → None.
fn split_number_suffix(text: &str) -> Option<(String, String)> {
    let mut chars = text.chars().peekable();
    let mut number = String::new();
    if matches!(chars.peek(), Some('-')) {
        number.push('-');
        chars.next();
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            number.push(ch);
            chars.next();
            continue;
        }
        if ch == '.' && !saw_dot {
            saw_dot = true;
            number.push(ch);
            chars.next();
            continue;
        }
        break;
    }
    if !saw_digit {
        return None;
    }
    let suffix: String = chars.collect();
    if suffix.is_empty() {
        return None;
    }
    if !suffix
        .chars()
        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some((number, suffix))
}

include!("entrypoints/qualify.rs");
include!("entrypoints/decorators.rs");
