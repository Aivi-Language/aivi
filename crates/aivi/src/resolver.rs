use std::collections::{HashMap, HashSet};

use crate::diagnostics::{Diagnostic, FileDiagnostic};
use crate::surface::{BlockItem, Def, DomainItem, Expr, Module, ModuleItem, Pattern, TextPart};

pub fn check_modules(modules: &[Module]) -> Vec<FileDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut module_map: HashMap<String, &Module> = HashMap::new();

    for module in modules {
        if let Some(existing) = module_map.get(&module.name.name) {
            diagnostics.push(file_diag(
                existing,
                Diagnostic {
                    code: "E2000".to_string(),
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
    }

    let cycle_nodes = detect_cycles(&module_map);
    for module_name in cycle_nodes {
        if let Some(module) = module_map.get(&module_name) {
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "E2004".to_string(),
                    message: "cyclic module dependency".to_string(),
                    span: module.name.span.clone(),
                    labels: Vec::new(),
                },
            ));
        }
    }

    diagnostics
}

fn check_duplicate_exports(module: &Module, diagnostics: &mut Vec<FileDiagnostic>) {
    let mut seen: HashSet<&str> = HashSet::new();
    for export in &module.exports {
        if !seen.insert(export.name.as_str()) {
            diagnostics.push(file_diag(
                module,
                Diagnostic {
                    code: "E2001".to_string(),
                    message: format!("duplicate export '{}'", export.name),
                    span: export.span.clone(),
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
            .map(|name| name.name.as_str())
            .collect();
        for item in &use_decl.items {
            if !exports.contains(item.name.as_str()) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2003".to_string(),
                        message: format!(
                            "module '{}' does not export '{}'",
                            use_decl.module.name, item.name
                        ),
                        span: item.span.clone(),
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
    let mut scope: HashSet<String> = HashSet::new();
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
                    .map(|name| name.name.as_str())
                    .collect();
                for export in &target.exports {
                    scope.insert(export.name.clone());
                }
                for item in &target.items {
                    if let ModuleItem::ClassDecl(class_decl) = item {
                        if !exported.contains(class_decl.name.name.as_str()) {
                            continue;
                        }
                        for member in &class_decl.members {
                            scope.insert(member.name.name.clone());
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
                .map(|name| name.name.as_str())
                .collect();
            for item in &use_decl.items {
                if target.exports.iter().any(|export| export.name == item.name) {
                    scope.insert(item.name.clone());
                    if exported.contains(item.name.as_str()) {
                        for module_item in &target.items {
                            if let ModuleItem::ClassDecl(class_decl) = module_item {
                                if class_decl.name.name == item.name {
                                    for member in &class_decl.members {
                                        scope.insert(member.name.name.clone());
                                    }
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

fn collect_value_defs(item: &ModuleItem, scope: &mut HashSet<String>) {
    match item {
        ModuleItem::Def(def) => {
            scope.insert(def.name.name.clone());
        }
        ModuleItem::TypeDecl(type_decl) => {
            for ctor in &type_decl.constructors {
                scope.insert(ctor.name.name.clone());
            }
        }
        ModuleItem::TypeAlias(_) => {}
        ModuleItem::ClassDecl(class_decl) => {
            for member in &class_decl.members {
                scope.insert(member.name.name.clone());
            }
        }
        ModuleItem::InstanceDecl(instance) => {
            for def in &instance.defs {
                scope.insert(def.name.name.clone());
            }
        }
        ModuleItem::DomainDecl(domain) => {
            for domain_item in &domain.items {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        scope.insert(def.name.name.clone());
                    }
                    DomainItem::TypeAlias(type_decl) => {
                        for ctor in &type_decl.constructors {
                            scope.insert(ctor.name.name.clone());
                        }
                    }
                    DomainItem::TypeSig(_) => {}
                }
            }
        }
        ModuleItem::TypeSig(_) => {}
    }
}

fn check_def(
    def: &Def,
    scope: &HashSet<String>,
    diagnostics: &mut Vec<FileDiagnostic>,
    module: &Module,
    allow_unknown: bool,
) {
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

fn check_expr(
    expr: &Expr,
    scope: &mut HashSet<String>,
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
            if !scope.contains(&name.name) {
                diagnostics.push(file_diag(
                    module,
                    Diagnostic {
                        code: "E2005".to_string(),
                        message: format!("unknown name '{}'", name.name),
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

fn collect_pattern_bindings(patterns: &[Pattern], scope: &mut HashSet<String>) {
    for pattern in patterns {
        collect_pattern_binding(pattern, scope);
    }
}

fn collect_pattern_binding(pattern: &Pattern, scope: &mut HashSet<String>) {
    match pattern {
        Pattern::Wildcard(_) => {}
        Pattern::Ident(name) => {
            if !is_constructor_name(&name.name) {
                scope.insert(name.name.clone());
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
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "True"
            | "False"
            | "text"
            | "regex"
            | "math"
            | "calendar"
            | "color"
            | "bigint"
            | "rational"
            | "decimal"
            | "url"
            | "http"
            | "https"
            | "console"
            | "clock"
            | "random"
            | "channel"
            | "concurrent"
            | "httpServer"
    )
}

fn file_diag(module: &Module, diagnostic: Diagnostic) -> FileDiagnostic {
    FileDiagnostic {
        path: module.path.clone(),
        diagnostic,
    }
}
