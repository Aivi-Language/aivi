use std::collections::{HashMap, HashSet};

use crate::diagnostics::{Diagnostic, DiagnosticSeverity, FileDiagnostic};
use crate::surface::{
    BlockItem, Decorator, Def, DomainItem, Expr, Literal, Module, ModuleItem, Pattern, TextPart,
    TypeAlias, TypeDecl, TypeExpr, TypeSig,
};

pub fn check_modules(modules: &[Module]) -> Vec<FileDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut module_map: HashMap<String, &Module> = HashMap::new();

    for module in modules {
        if let Some(existing) = module_map.get(&module.name.name) {
            diagnostics.push(file_diag(
                existing,
                Diagnostic {
                    code: "E2000".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("duplicate module '{}'", module.name.name),
                    span: module.name.span.clone(),
                    labels: Vec::new(),
                },
            ));
        } else {
            module_map.insert(module.name.name.clone(), module);
        }
    }

    for module in modules {
        check_duplicate_exports(module, &mut diagnostics);
        check_uses(module, &module_map, &mut diagnostics);
        check_defs(module, &module_map, &mut diagnostics);
        check_unused_imports_and_bindings(module, &mut diagnostics);
    }

    let cycle_nodes = detect_cycles(&module_map);
    for module_name in cycle_nodes {
        if let Some(module) = module_map.get(&module_name) {
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "E2004".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: "cyclic module dependency".to_string(),
                    span: module.name.span.clone(),
                    labels: Vec::new(),
                },
            ));
        }
    }

    diagnostics
}

fn check_unused_imports_and_bindings(module: &Module, diagnostics: &mut Vec<FileDiagnostic>) {
    // Embedded stdlib modules are not held to the same hygiene bar in v0.1; avoid failing
    // compiler/LSP checks due to warning-only lint rules.
    if module.path.starts_with("<embedded:") {
        return;
    }

    let used = collect_used_names(module);
    let exported: HashSet<&str> = module
        .exports
        .iter()
        .map(|e| e.name.name.as_str())
        .collect();

    // Unused explicit imports.
    for use_decl in &module.uses {
        if use_decl.wildcard {
            continue;
        }
        for item in &use_decl.items {
            // Domain imports are often used only via operators/suffix literals (no identifier use).
            if item.kind == crate::surface::ScopeItemKind::Domain {
                continue;
            }
            if !used.contains(item.name.name.as_str()) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "W2100".to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message: format!("unused import '{}'", item.name.name),
                        span: item.name.span.clone(),
                        labels: Vec::new(),
                    },
                ));
            }
        }
    }

    // Unused private (non-exported) top-level value bindings.
    for item in &module.items {
        if let ModuleItem::Def(def) = item {
            if exported.contains(def.name.name.as_str()) {
                continue;
            }
            if used.contains(def.name.name.as_str()) {
                continue;
            }
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "W2101".to_string(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!("unused binding '{}'", def.name.name),
                    span: def.name.span.clone(),
                    labels: Vec::new(),
                },
            ));
        }
    }
}

fn collect_used_names(module: &Module) -> HashSet<String> {
    fn collect_type_expr(expr: &TypeExpr, out: &mut HashSet<String>) {
        match expr {
            TypeExpr::Name(name) => {
                out.insert(name.name.clone());
            }
            TypeExpr::And { items, .. } | TypeExpr::Tuple { items, .. } => {
                for item in items {
                    collect_type_expr(item, out);
                }
            }
            TypeExpr::Apply { base, args, .. } => {
                collect_type_expr(base, out);
                for arg in args {
                    collect_type_expr(arg, out);
                }
            }
            TypeExpr::Func { params, result, .. } => {
                for param in params {
                    collect_type_expr(param, out);
                }
                collect_type_expr(result, out);
            }
            TypeExpr::Record { fields, .. } => {
                for (_label, ty) in fields {
                    collect_type_expr(ty, out);
                }
            }
            TypeExpr::Star { .. } | TypeExpr::Unknown { .. } => {}
        }
    }

    fn collect_pattern_uses(pattern: &Pattern, out: &mut HashSet<String>) {
        match pattern {
            Pattern::Constructor { name, args, .. } => {
                out.insert(name.name.clone());
                for arg in args {
                    collect_pattern_uses(arg, out);
                }
            }
            Pattern::Tuple { items, .. } => {
                for item in items {
                    collect_pattern_uses(item, out);
                }
            }
            Pattern::List { items, rest, .. } => {
                for item in items {
                    collect_pattern_uses(item, out);
                }
                if let Some(rest) = rest.as_deref() {
                    collect_pattern_uses(rest, out);
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields {
                    collect_pattern_uses(&field.pattern, out);
                }
            }
            Pattern::Ident(_) | Pattern::Wildcard(_) | Pattern::Literal(_) => {}
        }
    }

    fn collect_expr(expr: &Expr, out: &mut HashSet<String>) {
        match expr {
            Expr::Ident(name) => {
                out.insert(name.name.clone());
            }
            Expr::Suffixed { base, .. } => {
                collect_expr(base, out);
            }
            Expr::TextInterpolate { parts, .. } => {
                for part in parts {
                    if let TextPart::Expr { expr, .. } = part {
                        collect_expr(expr, out);
                    }
                }
            }
            Expr::List { items, .. } => {
                for item in items {
                    collect_expr(&item.expr, out);
                }
            }
            Expr::Tuple { items, .. } => {
                for item in items {
                    collect_expr(item, out);
                }
            }
            Expr::Record { fields, .. } | Expr::PatchLit { fields, .. } => {
                for field in fields {
                    collect_expr(&field.value, out);
                }
            }
            Expr::FieldAccess { base, field, .. } => {
                out.insert(field.name.clone());
                collect_expr(base, out);
            }
            Expr::Index { base, index, .. } => {
                collect_expr(base, out);
                collect_expr(index, out);
            }
            Expr::Call { func, args, .. } => {
                collect_expr(func, out);
                for arg in args {
                    collect_expr(arg, out);
                }
            }
            Expr::Lambda { params, body, .. } => {
                for param in params {
                    collect_pattern_uses(param, out);
                }
                collect_expr(body, out);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                if let Some(scrutinee) = scrutinee.as_deref() {
                    collect_expr(scrutinee, out);
                }
                for arm in arms {
                    collect_pattern_uses(&arm.pattern, out);
                    if let Some(guard) = &arm.guard {
                        collect_expr(guard, out);
                    }
                    collect_expr(&arm.body, out);
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                collect_expr(cond, out);
                collect_expr(then_branch, out);
                collect_expr(else_branch, out);
            }
            Expr::Binary { left, right, .. } => {
                collect_expr(left, out);
                collect_expr(right, out);
            }
            Expr::Block { items, .. } => {
                for item in items {
                    match item {
                        BlockItem::Bind { pattern, expr, .. }
                        | BlockItem::Let { pattern, expr, .. } => {
                            collect_pattern_uses(pattern, out);
                            collect_expr(expr, out);
                        }
                        BlockItem::Filter { expr, .. }
                        | BlockItem::Yield { expr, .. }
                        | BlockItem::Recurse { expr, .. }
                        | BlockItem::Expr { expr, .. } => collect_expr(expr, out),
                    }
                }
            }
            Expr::Literal(_) | Expr::Raw { .. } | Expr::FieldSection { .. } => {}
        }
    }

    let mut out = HashSet::new();
    for item in &module.items {
        match item {
            ModuleItem::TypeSig(TypeSig { ty, .. }) => collect_type_expr(ty, &mut out),
            ModuleItem::TypeAlias(TypeAlias { aliased, .. }) => {
                collect_type_expr(aliased, &mut out)
            }
            ModuleItem::TypeDecl(TypeDecl { constructors, .. }) => {
                for ctor in constructors {
                    for arg in &ctor.args {
                        collect_type_expr(arg, &mut out);
                    }
                }
            }
            ModuleItem::Def(def) => {
                collect_expr(&def.expr, &mut out);
            }
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::TypeSig(TypeSig { ty, .. }) => collect_type_expr(ty, &mut out),
                        DomainItem::TypeAlias(TypeDecl { constructors, .. }) => {
                            for ctor in constructors {
                                for arg in &ctor.args {
                                    collect_type_expr(arg, &mut out);
                                }
                            }
                        }
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            collect_expr(&def.expr, &mut out);
                        }
                    }
                }
            }
            ModuleItem::InstanceDecl(instance) => {
                for def in &instance.defs {
                    collect_expr(&def.expr, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

fn check_duplicate_exports(module: &Module, diagnostics: &mut Vec<FileDiagnostic>) {
    // Exports live in multiple namespaces (e.g. `domain Path` and `Path`), so only reject
    // duplicates within the same exported kind.
    let mut seen: HashSet<(crate::surface::ScopeItemKind, &str)> = HashSet::new();
    for export in &module.exports {
        if !seen.insert((export.kind, export.name.name.as_str())) {
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "E2001".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("duplicate export '{}'", export.name.name),
                    span: export.name.span.clone(),
                    labels: Vec::new(),
                },
            ));
        }
    }
}

fn check_uses(
    module: &Module,
    module_map: &HashMap<String, &Module>,
    diagnostics: &mut Vec<FileDiagnostic>,
) {
    for use_decl in &module.uses {
        let target = module_map.get(&use_decl.module.name);
        if target.is_none() {
            if use_decl.module.name.starts_with("aivi.") {
                continue;
            }
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "E2002".to_string(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("unknown module '{}'", use_decl.module.name),
                    span: use_decl.module.span.clone(),
                    labels: Vec::new(),
                },
            ));
            continue;
        }
        if use_decl.wildcard {
            continue;
        }
        let target = target.unwrap();
        let exports: HashSet<&str> = target
            .exports
            .iter()
            .map(|item| item.name.name.as_str())
            .collect();
        for item in &use_decl.items {
            if !exports.contains(item.name.name.as_str()) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2003".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "module '{}' does not export '{}'",
                            use_decl.module.name, item.name.name
                        ),
                        span: item.name.span.clone(),
                        labels: Vec::new(),
                    },
                ));
            }
        }
    }
}

fn check_defs(
    module: &Module,
    module_map: &HashMap<String, &Module>,
    diagnostics: &mut Vec<FileDiagnostic>,
) {
    let mut scope: HashMap<String, Option<String>> = HashMap::new();
    let mut allow_unknown = false;

    for item in module.items.iter() {
        collect_value_defs(item, &mut scope);
    }
    for use_decl in &module.uses {
        if use_decl.wildcard {
            if let Some(target) = module_map.get(&use_decl.module.name) {
                let exported: HashSet<&str> = target
                    .exports
                    .iter()
                    .map(|item| item.name.name.as_str())
                    .collect();
                for export in &target.exports {
                    scope.insert(
                        export.name.name.clone(),
                        deprecated_message_for_export(target, &export.name.name),
                    );
                    if use_decl.alias.is_some() {
                        scope.insert(
                            format!("{}.{}", use_decl.module.name, export.name.name),
                            deprecated_message_for_export(target, &export.name.name),
                        );
                    }
                }
                for item in &target.items {
                    if let ModuleItem::ClassDecl(class_decl) = item {
                        if !exported.contains(class_decl.name.name.as_str()) {
                            continue;
                        }
                        for member in &class_decl.members {
                            scope.insert(member.name.name.clone(), None);
                            if use_decl.alias.is_some() {
                                scope.insert(
                                    format!("{}.{}", use_decl.module.name, member.name.name),
                                    None,
                                );
                            }
                        }
                    }
                }
            } else if use_decl.module.name.starts_with("aivi.") {
                allow_unknown = true;
            }
            continue;
        }
        if let Some(target) = module_map.get(&use_decl.module.name) {
            let exported: HashSet<&str> = target
                .exports
                .iter()
                .map(|item| item.name.name.as_str())
                .collect();
            for item in &use_decl.items {
                match item.kind {
                    crate::surface::ScopeItemKind::Value => {
                        if target
                            .exports
                            .iter()
                            .any(|export| export.name.name == item.name.name)
                        {
                            scope.insert(
                                item.name.name.clone(),
                                deprecated_message_for_export(target, &item.name.name),
                            );
                            if use_decl.alias.is_some() {
                                scope.insert(
                                    format!("{}.{}", use_decl.module.name, item.name.name),
                                    deprecated_message_for_export(target, &item.name.name),
                                );
                            }
                            if exported.contains(item.name.name.as_str()) {
                                for module_item in &target.items {
                                    if let ModuleItem::ClassDecl(class_decl) = module_item {
                                        if class_decl.name.name == item.name.name {
                                            for member in &class_decl.members {
                                                scope.insert(member.name.name.clone(), None);
                                                if use_decl.alias.is_some() {
                                                    scope.insert(
                                                        format!(
                                                            "{}.{}",
                                                            use_decl.module.name, member.name.name
                                                        ),
                                                        None,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    crate::surface::ScopeItemKind::Domain => {
                        // Importing a domain brings its operators and literal templates into scope.
                        let exported_domain = target.exports.iter().any(|export| {
                            export.kind == crate::surface::ScopeItemKind::Domain
                                && export.name.name == item.name.name
                        });
                        if !exported_domain {
                            continue;
                        }
                        for module_item in &target.items {
                            let ModuleItem::DomainDecl(domain) = module_item else {
                                continue;
                            };
                            if domain.name.name != item.name.name {
                                continue;
                            }
                            for domain_item in &domain.items {
                                match domain_item {
                                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                                        scope.insert(
                                            def.name.name.clone(),
                                            deprecated_message_for_export(target, &def.name.name),
                                        );
                                        if use_decl.alias.is_some() {
                                            scope.insert(
                                                format!("{}.{}", use_decl.module.name, def.name.name),
                                                deprecated_message_for_export(target, &def.name.name),
                                            );
                                        }
                                    }
                                    DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for item in &module.items {
        match item {
            ModuleItem::Def(def) => {
                check_def(def, &scope, diagnostics, module, allow_unknown);
            }
            ModuleItem::InstanceDecl(instance) => {
                for def in &instance.defs {
                    check_def(def, &scope, diagnostics, module, allow_unknown);
                }
            }
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            check_def(def, &scope, diagnostics, module, allow_unknown);
                        }
                        DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                    }
                }
            }
            ModuleItem::TypeAlias(_) => {}
            _ => {}
        }
    }
}

fn collect_value_defs(item: &ModuleItem, scope: &mut HashMap<String, Option<String>>) {
    match item {
        ModuleItem::Def(def) => {
            scope.insert(def.name.name.clone(), deprecated_message(&def.decorators));
        }
        ModuleItem::TypeDecl(type_decl) => {
            for ctor in &type_decl.constructors {
                scope.insert(ctor.name.name.clone(), None);
            }
        }
        ModuleItem::TypeAlias(_) => {}
        ModuleItem::ClassDecl(class_decl) => {
            for member in &class_decl.members {
                scope.insert(member.name.name.clone(), None);
            }
        }
        ModuleItem::InstanceDecl(instance) => {
            for def in &instance.defs {
                scope.insert(def.name.name.clone(), deprecated_message(&def.decorators));
            }
        }
        ModuleItem::DomainDecl(domain) => {
            for domain_item in &domain.items {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        scope.insert(def.name.name.clone(), deprecated_message(&def.decorators));
                    }
                    DomainItem::TypeAlias(type_decl) => {
                        for ctor in &type_decl.constructors {
                            scope.insert(ctor.name.name.clone(), None);
                        }
                    }
                    DomainItem::TypeSig(_) => {}
                }
            }
        }
        ModuleItem::TypeSig(_) => {}
    }
}

fn deprecated_message(decorators: &[Decorator]) -> Option<String> {
    decorators
        .iter()
        .find(|decorator| decorator.name.name == "deprecated")
        .and_then(|decorator| match &decorator.arg {
            Some(Expr::Literal(Literal::String { text, .. })) => Some(text.clone()),
            _ => None,
        })
}

fn deprecated_message_for_export(module: &Module, name: &str) -> Option<String> {
    for item in &module.items {
        match item {
            ModuleItem::Def(def) if def.name.name == name => {
                return deprecated_message(&def.decorators);
            }
            ModuleItem::InstanceDecl(instance) => {
                for def in &instance.defs {
                    if def.name.name == name {
                        return deprecated_message(&def.decorators);
                    }
                }
            }
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def)
                            if def.name.name == name =>
                        {
                            return deprecated_message(&def.decorators);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn check_def(
    def: &Def,
    scope: &HashMap<String, Option<String>>,
    diagnostics: &mut Vec<FileDiagnostic>,
    module: &Module,
    allow_unknown: bool,
) {
    check_debug_decorators(def, diagnostics, module);
    let mut local_scope = scope.clone();
    collect_pattern_bindings(&def.params, &mut local_scope);
    check_expr(
        &def.expr,
        &mut local_scope,
        diagnostics,
        module,
        allow_unknown,
    );
}
