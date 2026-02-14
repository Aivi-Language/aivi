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
    let mut seen: HashSet<&str> = HashSet::new();
    for export in &module.exports {
        if !seen.insert(export.name.name.as_str()) {
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

fn check_debug_decorators(def: &Def, diagnostics: &mut Vec<FileDiagnostic>, module: &Module) {
    fn expr_span(expr: &Expr) -> crate::diagnostics::Span {
        match expr {
            Expr::Ident(name) => name.span.clone(),
            Expr::Literal(literal) => match literal {
                Literal::Number { span, .. }
                | Literal::String { span, .. }
                | Literal::Sigil { span, .. }
                | Literal::Bool { span, .. }
                | Literal::DateTime { span, .. } => span.clone(),
            },
            Expr::TextInterpolate { span, .. }
            | Expr::List { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::Record { span, .. }
            | Expr::PatchLit { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::FieldSection { span, .. }
            | Expr::Index { span, .. }
            | Expr::Call { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::Match { span, .. }
            | Expr::If { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Block { span, .. }
            | Expr::Raw { span, .. } => span.clone(),
        }
    }

    let allowed = ["pipes", "args", "return", "time"];
    let has_debug = def.decorators.iter().any(|d| d.name.name == "debug");
    if !has_debug {
        return;
    }
    if def.params.is_empty() {
        diagnostics.push(file_diag(
            module,
            Diagnostic {
                code: "E2010".to_string(),
                severity: DiagnosticSeverity::Error,
                message: "`@debug` can only be applied to function definitions".to_string(),
                span: def.name.span.clone(),
                labels: Vec::new(),
            },
        ));
    }

    for decorator in def.decorators.iter().filter(|d| d.name.name == "debug") {
        let mut params: Vec<crate::surface::SpannedName> = Vec::new();
        match &decorator.arg {
            None => {}
            Some(Expr::Tuple { items, .. }) => {
                for item in items {
                    match item {
                        Expr::Ident(name) => params.push(name.clone()),
                        other => {
                            diagnostics.push(file_diag(
                                module,
                                Diagnostic {
                                    code: "E2011".to_string(),
                                    severity: DiagnosticSeverity::Error,
                                    message:
                                        "`@debug` expects a list of parameter names (e.g. `@debug(pipes, args, return, time)`)".to_string(),
                                    span: expr_span(other),
                                    labels: Vec::new(),
                                },
                            ));
                        }
                    }
                }
            }
            Some(Expr::Ident(name)) => params.push(name.clone()),
            Some(other) => {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2011".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message:
                            "`@debug` expects `@debug(pipes, args, return, time)` (or `@debug()`)"
                                .to_string(),
                        span: expr_span(other),
                        labels: Vec::new(),
                    },
                ));
                continue;
            }
        }

        for param in params {
            if !allowed.contains(&param.name.as_str()) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2012".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message: format!(
                            "unknown `@debug` parameter `{}` (expected: pipes, args, return, time)",
                            param.name
                        ),
                        span: param.span,
                        labels: Vec::new(),
                    },
                ));
            }
        }
    }
}

fn check_expr(
    expr: &Expr,
    scope: &mut HashMap<String, Option<String>>,
    diagnostics: &mut Vec<FileDiagnostic>,
    module: &Module,
    allow_unknown: bool,
) {
    match expr {
        Expr::TextInterpolate { parts, .. } => {
            for part in parts {
                if let TextPart::Expr { expr, .. } = part {
                    check_expr(expr, scope, diagnostics, module, allow_unknown);
                }
            }
        }
        Expr::Ident(name) => {
            if name.name == "_" {
                return;
            }
            if is_constructor_name(&name.name) {
                return;
            }
            if is_builtin_name(&name.name) {
                return;
            }
            if allow_unknown {
                return;
            }
            if let Some(Some(message)) = scope.get(&name.name) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "W2500".to_string(),
                        severity: DiagnosticSeverity::Warning,
                        message: format!("use of deprecated name '{}': {}", name.name, message),
                        span: name.span.clone(),
                        labels: Vec::new(),
                    },
                ));
            }
            if !scope.contains_key(&name.name) {
                let message = special_unknown_name_message(&name.name)
                    .unwrap_or_else(|| format!("unknown name '{}'", name.name));
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2005".to_string(),
                        severity: DiagnosticSeverity::Error,
                        message,
                        span: name.span.clone(),
                        labels: Vec::new(),
                    },
                ));
            }
        }
        Expr::Literal(_) => {}
        Expr::List { items, .. } => {
            for item in items {
                check_expr(&item.expr, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::Tuple { items, .. } => {
            for item in items {
                check_expr(item, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                check_expr(&field.value, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::PatchLit { fields, .. } => {
            for field in fields {
                check_expr(&field.value, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::FieldAccess { base, .. } => {
            check_expr(base, scope, diagnostics, module, allow_unknown);
        }
        Expr::FieldSection { .. } => {}
        Expr::Index { base, index, .. } => {
            check_expr(base, scope, diagnostics, module, allow_unknown);
            check_expr(index, scope, diagnostics, module, allow_unknown);
        }
        Expr::Call { func, args, .. } => {
            check_expr(func, scope, diagnostics, module, allow_unknown);
            for arg in args {
                check_expr(arg, scope, diagnostics, module, allow_unknown);
            }
        }
        Expr::Lambda { params, body, .. } => {
            let mut inner_scope = scope.clone();
            collect_pattern_bindings(params, &mut inner_scope);
            check_expr(body, &mut inner_scope, diagnostics, module, allow_unknown);
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            if let Some(scrutinee) = scrutinee {
                check_expr(scrutinee, scope, diagnostics, module, allow_unknown);
            }
            for arm in arms {
                let mut arm_scope = scope.clone();
                collect_pattern_binding(&arm.pattern, &mut arm_scope);
                if let Some(guard) = &arm.guard {
                    check_expr(guard, &mut arm_scope, diagnostics, module, allow_unknown);
                }
                check_expr(
                    &arm.body,
                    &mut arm_scope,
                    diagnostics,
                    module,
                    allow_unknown,
                );
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            check_expr(cond, scope, diagnostics, module, allow_unknown);
            check_expr(then_branch, scope, diagnostics, module, allow_unknown);
            check_expr(else_branch, scope, diagnostics, module, allow_unknown);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(left, scope, diagnostics, module, allow_unknown);
            check_expr(right, scope, diagnostics, module, allow_unknown);
        }
        Expr::Block { items, .. } => {
            let mut block_scope = scope.clone();
            for item in items {
                match item {
                    BlockItem::Bind { pattern, expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                        collect_pattern_binding(pattern, &mut block_scope);
                    }
                    BlockItem::Let { pattern, expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                        collect_pattern_binding(pattern, &mut block_scope);
                    }
                    BlockItem::Filter { expr, .. }
                    | BlockItem::Yield { expr, .. }
                    | BlockItem::Recurse { expr, .. }
                    | BlockItem::Expr { expr, .. } => {
                        check_expr(expr, &mut block_scope, diagnostics, module, allow_unknown);
                    }
                }
            }
        }
        Expr::Raw { .. } => {}
    }
}

fn special_unknown_name_message(name: &str) -> Option<String> {
    // Common “ported” keywords from other languages that AIVI intentionally does not have.
    match name {
        "return" => Some("unknown name 'return' (AIVI has no `return`; the last expression is the result)".to_string()),
        "mut" => Some("unknown name 'mut' (AIVI is immutable; use a new binding instead of mutation)".to_string()),
        "for" | "while" => Some(format!(
            "unknown name '{name}' (AIVI has no loops; use recursion, `generate`, or higher-order functions)"
        )),
        "null" | "undefined" => Some(format!(
            "unknown name '{name}' (AIVI has no nulls; use `Option`/`Result`)"
        )),
        _ => None,
    }
}

fn collect_pattern_bindings(patterns: &[Pattern], scope: &mut HashMap<String, Option<String>>) {
    for pattern in patterns {
        collect_pattern_binding(pattern, scope);
    }
}

fn collect_pattern_binding(pattern: &Pattern, scope: &mut HashMap<String, Option<String>>) {
    match pattern {
        Pattern::Wildcard(_) => {}
        Pattern::Ident(name) => {
            if !is_constructor_name(&name.name) {
                scope.insert(name.name.clone(), None);
            }
        }
        Pattern::Literal(_) => {}
        Pattern::Constructor { args, .. } => {
            for arg in args {
                collect_pattern_binding(arg, scope);
            }
        }
        Pattern::Tuple { items, .. } => {
            for item in items {
                collect_pattern_binding(item, scope);
            }
        }
        Pattern::List { items, rest, .. } => {
            for item in items {
                collect_pattern_binding(item, scope);
            }
            if let Some(rest) = rest {
                collect_pattern_binding(rest, scope);
            }
        }
        Pattern::Record { fields, .. } => {
            for field in fields {
                collect_pattern_binding(&field.pattern, scope);
            }
        }
    }
}

fn detect_cycles(module_map: &HashMap<String, &Module>) -> HashSet<String> {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut in_cycle = HashSet::new();

    for name in module_map.keys() {
        if visited.contains(name) {
            continue;
        }
        dfs(
            name,
            module_map,
            &mut visiting,
            &mut visited,
            &mut stack,
            &mut in_cycle,
        );
    }

    in_cycle
}

fn dfs(
    name: &str,
    module_map: &HashMap<String, &Module>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
    in_cycle: &mut HashSet<String>,
) {
    visiting.insert(name.to_string());
    stack.push(name.to_string());

    if let Some(module) = module_map.get(name) {
        for use_decl in &module.uses {
            let next = &use_decl.module.name;
            if !module_map.contains_key(next) {
                continue;
            }
            if visiting.contains(next) {
                if let Some(pos) = stack.iter().position(|entry| entry == next) {
                    for entry in &stack[pos..] {
                        in_cycle.insert(entry.clone());
                    }
                }
                continue;
            }
            if !visited.contains(next) {
                dfs(next, module_map, visiting, visited, stack, in_cycle);
            }
        }
    }

    visiting.remove(name);
    visited.insert(name.to_string());
    stack.pop();
}

fn is_constructor_name(name: &str) -> bool {
    name.chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
}

fn is_builtin_name(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "println"
            | "attempt"
            | "load"
            | "file"
            | "pure"
            | "fail"
            | "Unit"
            | "Text"
            | "Char"
            | "Int"
            | "Float"
            | "Bool"
            | "Bytes"
            | "List"
            | "Effect"
            | "Stream"
            | "Listener"
            | "Connection"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "True"
            | "False"
            | "Map"
            | "Set"
            | "Queue"
            | "Deque"
            | "Heap"
            | "text"
            | "regex"
            | "math"
            | "calendar"
            | "color"
            | "bigint"
            | "rational"
            | "decimal"
            | "url"
            | "system"
            | "logger"
            | "database"
            | "http"
            | "https"
            | "collections"
            | "linalg"
            | "signal"
            | "graph"
            | "console"
            | "clock"
            | "random"
            | "channel"
            | "concurrent"
            | "httpServer"
            | "ui"
            | "sockets"
            | "streams"
    )
}

fn file_diag(module: &Module, diagnostic: Diagnostic) -> FileDiagnostic {
    FileDiagnostic {
        path: module.path.clone(),
        diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_aliasing_rewrites_and_resolves_wildcard_imports() {
        let source = r#"
module test.db_alias
use aivi.database as db

// `db.*` gets rewritten to `aivi.database.*` during parsing; resolver must treat these as in-scope.
x = db.table
y = db.applyDelta
z = db.configure
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:#?}");
    }

    #[test]
    fn module_aliasing_handles_call_and_index_syntax() {
        let source = r#"
module test.db_alias_syntax
use aivi.database as db

User = { id: Int, name: Text }
userTable = db.table "users"[]

main = effect {
  _ <- db.configure { driver: db.Sqlite, url: ":memory:" }
  _ <- db.runMigrations[userTable]
  _ <- userTable + db.ins { id: 1, name: "Alice" }
  db.load userTable
}
"#;

        let path = std::path::Path::new("test.aivi");
        let (mut modules, diags) = crate::surface::parse_modules(path, source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");

        let mut all = crate::stdlib::embedded_stdlib_modules();
        all.append(&mut modules);
        let diags = check_modules(&all);

        let errors: Vec<_> = diags
            .into_iter()
            .filter(|d| d.path == "test.aivi" && d.diagnostic.code == "E2005")
            .collect();
        assert!(errors.is_empty(), "unexpected unknown-name errors: {errors:#?}");
    }

    #[test]
    fn debug_unknown_param_is_error() {
        let source = r#"
module test.debug_params

@debug(pipes, nope, time)
f x = x
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2012"),
            "expected E2012, got: {diags:?}"
        );
    }

    #[test]
    fn debug_requires_function_binding() {
        let source = r#"
module test.debug_params

@debug()
x = 1
"#;
        let (modules, diags) =
            crate::surface::parse_modules(std::path::Path::new("test.aivi"), source);
        assert!(diags.is_empty(), "unexpected parse diagnostics: {diags:?}");
        let diags = check_modules(&modules);
        assert!(
            diags.iter().any(|d| d.diagnostic.code == "E2010"),
            "expected E2010, got: {diags:?}"
        );
    }
}
