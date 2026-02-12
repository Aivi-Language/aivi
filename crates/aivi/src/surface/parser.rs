use std::path::Path;

use crate::cst::CstToken;
use crate::diagnostics::{Diagnostic, DiagnosticLabel, FileDiagnostic, Position, Span};
use crate::lexer::{filter_tokens, lex, Token, TokenKind};

use super::ast::*;

pub fn parse_modules(path: &Path, content: &str) -> (Vec<Module>, Vec<FileDiagnostic>) {
    let (cst_tokens, lex_diags) = lex(content);
    let tokens = filter_tokens(&cst_tokens);
    let mut parser = Parser::new(tokens, path);
    let mut modules = parser.parse_modules();
    inject_prelude_imports(&mut modules);
    expand_domain_exports(&mut modules);
    let mut diagnostics: Vec<FileDiagnostic> = lex_diags
        .into_iter()
        .map(|diag| FileDiagnostic {
            path: path.display().to_string(),
            diagnostic: diag,
        })
        .collect();
    diagnostics.append(&mut parser.diagnostics);
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
                    name: "aivi.prelude".to_string(),
                    span: span.clone(),
                },
                items: Vec::new(),
                span,
                wildcard: true,
            },
        );
    }
}

fn expand_domain_exports(modules: &mut [Module]) {
    use std::collections::HashSet;

    for module in modules {
        let mut exported: HashSet<String> = module
            .exports
            .iter()
            .map(|name| name.name.clone())
            .collect();
        let mut extra_exports = Vec::new();
        for item in &module.items {
            let ModuleItem::DomainDecl(domain) = item else {
                continue;
            };
            if !exported.contains(&domain.name.name) {
                continue;
            }
            for domain_item in &domain.items {
                match domain_item {
                    DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                        if exported.insert(def.name.name.clone()) {
                            extra_exports.push(def.name.clone());
                        }
                    }
                    DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                }
            }
        }
        module.exports.extend(extra_exports);
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<FileDiagnostic>,
    path: String,
    gensym: u32,
}

impl Parser {
    fn new(tokens: Vec<Token>, path: &Path) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            path: path.display().to_string(),
            gensym: 0,
        }
    }

    fn fresh_internal_name(&mut self, prefix: &str, span: Span) -> SpannedName {
        let name = format!("__{prefix}{}", self.gensym);
        self.gensym = self.gensym.wrapping_add(1);
        SpannedName { name, span }
    }

    fn build_ctor_pattern(&self, name: &str, args: Vec<Pattern>, span: Span) -> Pattern {
        Pattern::Constructor {
            name: SpannedName {
                name: name.to_string(),
                span: span.clone(),
            },
            args,
            span,
        }
    }

    fn build_ident_expr(&self, name: &str, span: Span) -> Expr {
        Expr::Ident(SpannedName {
            name: name.to_string(),
            span,
        })
    }

    fn build_call_expr(&self, func: Expr, args: Vec<Expr>, span: Span) -> Expr {
        Expr::Call {
            func: Box::new(func),
            args,
            span,
        }
    }

    fn parse_modules(&mut self) -> Vec<Module> {
        let mut modules = Vec::new();
        while self.pos < self.tokens.len() {
            let annotations = self.consume_decorators();
            if self.peek_keyword("module") {
                self.pos += 1;
                if let Some(module) = self.parse_module(annotations) {
                    modules.push(module);
                } else {
                    self.recover_to_module();
                }
            } else if !annotations.is_empty() {
                for annotation in annotations {
                    self.emit_diag(
                        "E1502",
                        "decorators are only allowed before `module` declarations in this parser",
                        annotation.span.clone(),
                    );
                }
                self.recover_to_module();
            } else {
                self.pos += 1;
            }
        }
        modules
    }

    fn consume_decorators(&mut self) -> Vec<Decorator> {
        let mut decorators = Vec::new();
        loop {
            self.consume_newlines();
            if !self.consume_symbol("@") {
                break;
            }
            let at_span = self.previous_span();
            let Some(name) = self.consume_ident() else {
                self.emit_diag(
                    "E1503",
                    "expected decorator name after `@`",
                    at_span.clone(),
                );
                break;
            };
            let arg_starts_same_line = self
                .tokens
                .get(self.pos)
                .is_some_and(|next| next.span.start.line == name.span.end.line);
            let arg = if arg_starts_same_line && self.is_expr_start() {
                let checkpoint = self.pos;
                let arg = self.parse_expr();
                if arg.is_none() {
                    self.pos = checkpoint;
                    self.emit_diag(
                        "E1510",
                        "expected decorator argument expression",
                        merge_span(at_span.clone(), name.span.clone()),
                    );
                }
                arg
            } else {
                None
            };
            let span = match &arg {
                Some(arg) => merge_span(at_span.clone(), expr_span(arg)),
                None => merge_span(at_span.clone(), name.span.clone()),
            };
            if let Some(next) = self.tokens.get(self.pos) {
                if next.span.start.line == span.end.line {
                    self.emit_diag(
                        "E1504",
                        "decorators must be written on their own line",
                        merge_span(span.clone(), next.span.clone()),
                    );
                }
            }
            decorators.push(Decorator { name, arg, span });
        }
        decorators
    }

    fn parse_module(&mut self, annotations: Vec<Decorator>) -> Option<Module> {
        let module_kw = self.previous_span();
        let name = self.parse_dotted_name()?;
        self.consume_newlines();
        let mut explicit_body = false;
        if self.consume_symbol("=") {
            self.expect_symbol("{", "expected '{' to start module body");
            explicit_body = true;
        } else if self.consume_symbol("{") {
            self.emit_diag(
                "E1509",
                "expected '=' before '{' to start module body",
                self.previous_span(),
            );
            explicit_body = true;
        }
        let mut exports = Vec::new();
        let mut uses = Vec::new();
        let mut items = Vec::new();
        loop {
            if self.pos >= self.tokens.len() {
                break;
            }
            let loop_start = self.pos;
            if explicit_body && self.check_symbol("}") {
                break;
            }
            self.consume_newlines();
            if explicit_body && self.check_symbol("}") {
                break;
            }
            if !explicit_body && self.peek_keyword("module") {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag(
                    "E1508",
                    "implicit module bodies must be the last top-level item in a file",
                    span,
                );
                self.pos += 1;
                continue;
            }
            let decorators = self.consume_decorators();
            self.validate_item_decorators(&decorators);
            if !explicit_body && self.peek_keyword("module") {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag(
                    "E1508",
                    "implicit module bodies must be the last top-level item in a file",
                    span,
                );
                self.pos += 1;
                continue;
            }
            if self.match_keyword("export") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators cannot be applied to `export` items",
                        decorator.span,
                    );
                }
                exports.extend(self.parse_export_list());
                continue;
            }
            if self.match_keyword("use") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators cannot be applied to `use` imports",
                        decorator.span,
                    );
                }
                if let Some(use_decl) = self.parse_use_decl() {
                    uses.push(use_decl);
                }
                continue;
            }
            if self.match_keyword("class") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators are not supported on `class` declarations yet",
                        decorator.span,
                    );
                }
                if let Some(class_decl) = self.parse_class_decl() {
                    items.push(ModuleItem::ClassDecl(class_decl));
                }
                continue;
            }
            if self.match_keyword("instance") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators are not supported on `instance` declarations yet",
                        decorator.span,
                    );
                }
                if let Some(instance_decl) = self.parse_instance_decl() {
                    items.push(ModuleItem::InstanceDecl(instance_decl));
                }
                continue;
            }
            if self.match_keyword("domain") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators are not supported on `domain` declarations yet",
                        decorator.span,
                    );
                }
                if let Some(domain) = self.parse_domain_decl() {
                    items.push(ModuleItem::DomainDecl(domain));
                }
                continue;
            }

            if self.match_keyword("type") {
                for decorator in decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators are not supported on type declarations yet",
                        decorator.span,
                    );
                }
                if let Some(item) = self.parse_type_decl_or_alias() {
                    items.push(item);
                }
                continue;
            }

            if let Some(item) = self.parse_type_or_def(decorators) {
                items.push(item);
                continue;
            }

            self.recover_to_item();
            // Guard: if nothing advanced pos this iteration, force advance
            // to prevent infinite loops (e.g. stray `}` in implicit bodies).
            if self.pos == loop_start {
                self.pos += 1;
            }
        }
        let end_span = if explicit_body {
            self.expect_symbol("}", "expected '}' to close module body")
                .unwrap_or_else(|| module_kw.clone())
        } else {
            self.pos = self.tokens.len();
            self.previous_span()
        };
        let span = merge_span(module_kw.clone(), end_span);
        self.validate_module_decorators(&annotations);
        Some(Module {
            name,
            exports,
            uses,
            items,
            annotations,
            span,
            path: self.path.clone(),
        })
    }

    fn parse_export_list(&mut self) -> Vec<SpannedName> {
        let mut exports = Vec::new();
        loop {
            if self.match_keyword("domain") {
                if let Some(name) = self.consume_ident() {
                    exports.push(name);
                } else {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1500", "expected domain name after 'domain'", span);
                    break;
                }
            } else if let Some(name) = self.consume_ident() {
                exports.push(name);
            } else {
                break;
            }
            if !self.consume_symbol(",") {
                break;
            }
        }
        exports
    }

    fn parse_use_decl(&mut self) -> Option<UseDecl> {
        let start = self.previous_span();
        let module = self.parse_dotted_name()?;
        let mut items = Vec::new();
        let mut wildcard = true;
        if self.consume_symbol("(") {
            wildcard = false;
            while !self.check_symbol(")") && self.pos < self.tokens.len() {
                if self.match_keyword("domain") {
                    if let Some(name) = self.consume_ident() {
                        items.push(name);
                    } else {
                        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                        self.emit_diag("E1500", "expected domain name after 'domain'", span);
                        break;
                    }
                } else if let Some(name) = self.consume_ident() {
                    items.push(name);
                }
                if !self.consume_symbol(",") {
                    break;
                }
            }
            self.expect_symbol(")", "expected ')' to close import list");
        }
        let span = merge_span(start, module.span.clone());
        Some(UseDecl {
            module,
            items,
            span,
            wildcard,
        })
    }

    fn validate_module_decorators(&mut self, decorators: &[Decorator]) {
        for decorator in decorators {
            if decorator.name.name != "no_prelude" {
                self.emit_diag(
                    "E1506",
                    &format!("unknown module decorator `@{}`", decorator.name.name),
                    decorator.span.clone(),
                );
                continue;
            }
            if decorator.arg.is_some() {
                self.emit_diag(
                    "E1512",
                    "`@no_prelude` does not take an argument",
                    decorator.span.clone(),
                );
            }
        }
    }

    fn validate_item_decorators(&mut self, decorators: &[Decorator]) {
        for decorator in decorators {
            let name = decorator.name.name.as_str();
            if !matches!(
                name,
                "static" | "inline" | "deprecated" | "mcp_tool" | "mcp_resource" | "test"
            ) {
                self.emit_diag(
                    "E1506",
                    &format!("unknown decorator `@{}`", decorator.name.name),
                    decorator.span.clone(),
                );
                continue;
            }
            match name {
                "deprecated" => {
                    if decorator.arg.is_none() {
                        self.emit_diag(
                            "E1511",
                            "`@deprecated` expects an argument (e.g. `@deprecated \"message\"`)",
                            decorator.span.clone(),
                        );
                    }
                }
                _ => {
                    if decorator.arg.is_some() {
                        self.emit_diag(
                            "E1513",
                            &format!("`@{name}` does not take an argument"),
                            decorator.span.clone(),
                        );
                    }
                }
            }
        }
    }

    fn parse_type_or_def(&mut self, decorators: Vec<Decorator>) -> Option<ModuleItem> {
        let checkpoint = self.pos;
        if self.consume_name().is_some() {
            if self.check_symbol(":") {
                self.pos = checkpoint;
                return self.parse_type_sig(decorators).map(ModuleItem::TypeSig);
            }
            if self.check_symbol("=") || self.is_pattern_start() {
                self.pos = checkpoint;
                return self.parse_def_or_type(decorators);
            }
            self.pos = checkpoint;
        }
        None
    }

    fn parse_type_sig(&mut self, decorators: Vec<Decorator>) -> Option<TypeSig> {
        let name = self.consume_name()?;
        let start = name.span.clone();
        self.expect_symbol(":", "expected ':' for type signature");
        let ty = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: start.clone(),
        });
        let span = merge_span(start, type_span(&ty));
        Some(TypeSig {
            decorators,
            name,
            ty,
            span,
        })
    }

    fn parse_def_or_type(&mut self, decorators: Vec<Decorator>) -> Option<ModuleItem> {
        let checkpoint = self.pos;
        let name = self.consume_name()?;
        if name
            .name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
        {
            for decorator in decorators {
                self.emit_diag(
                    "E1507",
                    "decorators are not supported on type declarations yet",
                    decorator.span,
                );
            }
            self.pos = checkpoint;
            return self.parse_type_decl_or_alias();
        }
        self.pos = checkpoint;
        self.parse_def(decorators).map(ModuleItem::Def)
    }

    fn parse_type_decl_or_alias(&mut self) -> Option<ModuleItem> {
        let checkpoint = self.pos;
        let diag_checkpoint = self.diagnostics.len();
        if let Some(decl) = self.parse_type_decl() {
            if !decl.constructors.is_empty() {
                return Some(ModuleItem::TypeDecl(decl));
            }
        }
        self.pos = checkpoint;
        if let Some(alias) = self.parse_type_alias() {
            if self.check_symbol("=>") {
                self.pos = checkpoint;
                self.diagnostics.truncate(diag_checkpoint);
                return self.parse_def(Vec::new()).map(ModuleItem::Def);
            }
            return Some(ModuleItem::TypeAlias(alias));
        }
        self.diagnostics.truncate(diag_checkpoint);
        None
    }

    fn parse_type_decl(&mut self) -> Option<TypeDecl> {
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        self.expect_symbol("=", "expected '=' in type declaration");

        // Disambiguation: treat `T = ...` as an ADT only when there's a `|`
        // constructor separator in the constructor list. Otherwise parse it as a type alias.
        //
        // This avoids mis-parsing row/type operators like:
        //   UserName = Pick (name) User
        // as an ADT with a `Pick` constructor.
        let rhs_start = self.pos;
        let mut scan = self.pos;
        let mut saw_bar = false;
        while scan < self.tokens.len() {
            let token = &self.tokens[scan];
            if token.kind == TokenKind::Symbol && token.text == "|" {
                saw_bar = true;
                break;
            }
            if token.kind == TokenKind::Newline {
                // If the next non-newline token isn't a `|`, assume the type
                // declaration ends here (and thus has no constructor bars).
                let mut lookahead = scan + 1;
                while lookahead < self.tokens.len()
                    && self.tokens[lookahead].kind == TokenKind::Newline
                {
                    lookahead += 1;
                }
                if lookahead >= self.tokens.len() {
                    break;
                }
                if !(self.tokens[lookahead].kind == TokenKind::Symbol
                    && self.tokens[lookahead].text == "|")
                {
                    break;
                }
            }
            scan += 1;
        }
        if !saw_bar {
            self.pos = rhs_start;
            return None;
        }

        let mut ctors = Vec::new();
        while let Some(ctor_name) = self.consume_ident() {
            let mut args = Vec::new();
            while !self.check_symbol("|") && !self.check_symbol("}") && self.pos < self.tokens.len()
            {
                if let Some(ty) = self.parse_type_expr() {
                    args.push(ty);
                } else {
                    break;
                }
            }
            let span = merge_span(
                ctor_name.span.clone(),
                args.last().map(type_span).unwrap_or(ctor_name.span.clone()),
            );
            ctors.push(TypeCtor {
                name: ctor_name,
                args,
                span,
            });
            if !self.consume_symbol("|") {
                break;
            }
        }
        let span = merge_span(
            name.span.clone(),
            ctors
                .last()
                .map(|ctor| ctor.span.clone())
                .unwrap_or(name.span.clone()),
        );
        Some(TypeDecl {
            name,
            params,
            constructors: ctors,
            span,
        })
    }

    fn parse_type_alias(&mut self) -> Option<TypeAlias> {
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        self.expect_symbol("=", "expected '=' in type alias");
        let aliased = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: name.span.clone(),
        });
        let span = merge_span(name.span.clone(), type_span(&aliased));
        Some(TypeAlias {
            name,
            params,
            aliased,
            span,
        })
    }

    fn parse_class_decl(&mut self) -> Option<ClassDecl> {
        let start = self.previous_span();
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while !self.check_symbol("=") && self.pos < self.tokens.len() {
            if let Some(ty) = self.parse_type_atom() {
                params.push(ty);
            } else {
                break;
            }
        }
        self.consume_newlines();
        self.expect_symbol("=", "expected '=' in class declaration");
        self.consume_newlines();

        // Spec form:
        //   class Monad (M *) =
        //     Functor (M *) & { pure: A -> M A }
        //
        // We parse the RHS as a type expression, then treat record-type operands as the class
        // member set and non-record operands as superclasses.
        let body = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: name.span.clone(),
        });

        fn flatten_and(ty: TypeExpr, out: &mut Vec<TypeExpr>) {
            match ty {
                TypeExpr::And { items, .. } => {
                    for item in items {
                        flatten_and(item, out);
                    }
                }
                other => out.push(other),
            }
        }

        let mut parts = Vec::new();
        flatten_and(body.clone(), &mut parts);

        let mut supers = Vec::new();
        let mut members = Vec::new();
        for part in parts {
            match part {
                TypeExpr::Record { fields, .. } => {
                    for (field_name, field_ty) in fields {
                        let span = merge_span(field_name.span.clone(), type_span(&field_ty));
                        members.push(ClassMember {
                            name: field_name,
                            ty: field_ty,
                            span,
                        });
                    }
                }
                other => supers.push(other),
            }
        }

        let span = merge_span(start, type_span(&body));
        Some(ClassDecl {
            name,
            params,
            supers,
            members,
            span,
        })
    }

    fn parse_instance_decl(&mut self) -> Option<InstanceDecl> {
        let start = self.previous_span();
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while !self.check_symbol("=") && self.pos < self.tokens.len() {
            if let Some(ty) = self.parse_type_atom() {
                params.push(ty);
            } else {
                break;
            }
        }
        self.consume_newlines();
        self.expect_symbol("=", "expected '=' in instance declaration");
        self.expect_symbol("{", "expected '{' to start instance body");
        let mut defs = Vec::new();
        while self.pos < self.tokens.len() {
            self.consume_newlines();
            if self.check_symbol("}") {
                break;
            }
            if let Some(def) = self.parse_instance_def() {
                defs.push(def);
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close instance body");
        let span = merge_span(start, end.unwrap_or(name.span.clone()));
        Some(InstanceDecl {
            name,
            params,
            defs,
            span,
        })
    }

    fn parse_instance_def(&mut self) -> Option<Def> {
        let checkpoint = self.pos;
        let name = self.consume_name()?;
        if self.consume_symbol(":") {
            let expr = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: name.span.clone(),
            });
            let span = merge_span(name.span.clone(), expr_span(&expr));
            return Some(Def {
                decorators: Vec::new(),
                name,
                params: Vec::new(),
                expr,
                span,
            });
        }
        if self.check_symbol("=") {
            self.pos = checkpoint;
            return self.parse_def(Vec::new());
        }
        self.pos = checkpoint;
        None
    }

    fn parse_domain_decl(&mut self) -> Option<DomainDecl> {
        let start = self.previous_span();
        let name = self.consume_ident()?;
        self.expect_keyword("over", "expected 'over' in domain declaration");
        let over = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: name.span.clone(),
        });
        self.consume_newlines();
        self.expect_symbol("=", "expected '=' in domain declaration");
        self.expect_symbol("{", "expected '{' to start domain body");
        let mut items = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            self.consume_newlines();
            if self.check_symbol("}") {
                break;
            }
            let decorators = self.consume_decorators();
            self.validate_item_decorators(&decorators);
            if self.match_keyword("type") {
                for decorator in &decorators {
                    self.emit_diag(
                        "E1507",
                        "decorators are not supported on domain type declarations yet",
                        decorator.span.clone(),
                    );
                }
                if let Some(type_decl) = self.parse_type_decl() {
                    items.push(DomainItem::TypeAlias(type_decl));
                    continue;
                }
            }
            let checkpoint = self.pos;
            if self.consume_name().is_some() {
                if self.check_symbol(":") {
                    self.pos = checkpoint;
                    if let Some(sig) = self.parse_type_sig(decorators) {
                        items.push(DomainItem::TypeSig(sig));
                    }
                    continue;
                }
                self.pos = checkpoint;
            }
            if let Some(def) = self.parse_def(decorators.clone()) {
                items.push(DomainItem::Def(def));
                continue;
            }
            if let Some(literal_def) = self.parse_literal_def(decorators) {
                items.push(DomainItem::LiteralDef(literal_def));
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close domain body");
        let span = merge_span(start, end.unwrap_or(name.span.clone()));
        Some(DomainDecl {
            name,
            over,
            items,
            span,
        })
    }

    fn parse_literal_def(&mut self, decorators: Vec<Decorator>) -> Option<Def> {
        let start = self.pos;
        let number = self.consume_number()?;
        let suffix = self.consume_ident();
        if suffix.is_none() {
            self.pos = start;
            return None;
        }
        self.expect_symbol("=", "expected '=' after domain literal");
        let expr = self.parse_expr().unwrap_or(Expr::Raw {
            text: String::new(),
            span: number.span.clone(),
        });
        let suffix = suffix.unwrap();

        fn rewrite_literal_template(expr: Expr, needle: &str, param: &str) -> Expr {
            match expr {
                Expr::Literal(Literal::Number { text, span }) if text == needle => {
                    Expr::Ident(SpannedName {
                        name: param.to_string(),
                        span,
                    })
                }
                Expr::List { items, span } => Expr::List {
                    items: items
                        .into_iter()
                        .map(|item| ListItem {
                            expr: rewrite_literal_template(item.expr, needle, param),
                            spread: item.spread,
                            span: item.span,
                        })
                        .collect(),
                    span,
                },
                Expr::Tuple { items, span } => Expr::Tuple {
                    items: items
                        .into_iter()
                        .map(|item| rewrite_literal_template(item, needle, param))
                        .collect(),
                    span,
                },
                Expr::Record { fields, span } => Expr::Record {
                    fields: fields
                        .into_iter()
                        .map(|field| RecordField {
                            spread: field.spread,
                            path: field.path,
                            value: rewrite_literal_template(field.value, needle, param),
                            span: field.span,
                        })
                        .collect(),
                    span,
                },
                Expr::PatchLit { fields, span } => Expr::PatchLit {
                    fields: fields
                        .into_iter()
                        .map(|field| RecordField {
                            spread: field.spread,
                            path: field.path,
                            value: rewrite_literal_template(field.value, needle, param),
                            span: field.span,
                        })
                        .collect(),
                    span,
                },
                Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
                    base: Box::new(rewrite_literal_template(*base, needle, param)),
                    field,
                    span,
                },
                Expr::Index { base, index, span } => Expr::Index {
                    base: Box::new(rewrite_literal_template(*base, needle, param)),
                    index: Box::new(rewrite_literal_template(*index, needle, param)),
                    span,
                },
                Expr::Call { func, args, span } => Expr::Call {
                    func: Box::new(rewrite_literal_template(*func, needle, param)),
                    args: args
                        .into_iter()
                        .map(|arg| rewrite_literal_template(arg, needle, param))
                        .collect(),
                    span,
                },
                Expr::Lambda { params, body, span } => Expr::Lambda {
                    params,
                    body: Box::new(rewrite_literal_template(*body, needle, param)),
                    span,
                },
                Expr::Match {
                    scrutinee,
                    arms,
                    span,
                } => Expr::Match {
                    scrutinee: scrutinee.map(|scrutinee| {
                        Box::new(rewrite_literal_template(*scrutinee, needle, param))
                    }),
                    arms: arms
                        .into_iter()
                        .map(|arm| MatchArm {
                            pattern: arm.pattern,
                            guard: arm
                                .guard
                                .map(|guard| rewrite_literal_template(guard, needle, param)),
                            body: rewrite_literal_template(arm.body, needle, param),
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
                    cond: Box::new(rewrite_literal_template(*cond, needle, param)),
                    then_branch: Box::new(rewrite_literal_template(*then_branch, needle, param)),
                    else_branch: Box::new(rewrite_literal_template(*else_branch, needle, param)),
                    span,
                },
                Expr::Binary {
                    op,
                    left,
                    right,
                    span,
                } => Expr::Binary {
                    op,
                    left: Box::new(rewrite_literal_template(*left, needle, param)),
                    right: Box::new(rewrite_literal_template(*right, needle, param)),
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
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Let {
                                pattern,
                                expr,
                                span,
                            } => BlockItem::Let {
                                pattern,
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Filter { expr, span } => BlockItem::Filter {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Yield { expr, span } => BlockItem::Yield {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                            BlockItem::Expr { expr, span } => BlockItem::Expr {
                                expr: rewrite_literal_template(expr, needle, param),
                                span,
                            },
                        })
                        .collect(),
                    span,
                },
                other => other,
            }
        }

        let param = format!("__lit_{}", suffix.name);
        let expr = rewrite_literal_template(expr, &number.text, &param);

        let name_span = merge_span(number.span.clone(), suffix.span.clone());
        let name = SpannedName {
            name: format!("{}{}", number.text, suffix.name),
            span: name_span.clone(),
        };
        let span = merge_span(name_span, expr_span(&expr));
        Some(Def {
            decorators,
            name,
            params: vec![Pattern::Ident(SpannedName {
                name: param,
                span: number.span.clone(),
            })],
            expr,
            span,
        })
    }

    fn parse_def(&mut self, decorators: Vec<Decorator>) -> Option<Def> {
        self.consume_newlines();
        let name = self.consume_name()?;
        let mut params = Vec::new();
        while {
            self.consume_newlines();
            !self.check_symbol("=") && self.pos < self.tokens.len()
        } {
            if let Some(pattern) = self.parse_pattern() {
                params.push(pattern);
                continue;
            }
            break;
        }
        self.consume_newlines();
        self.expect_symbol("=", "expected '=' in definition");
        self.consume_newlines();
        let expr = self.parse_expr().unwrap_or(Expr::Raw {
            text: String::new(),
            span: name.span.clone(),
        });
        let span = merge_span(name.span.clone(), expr_span(&expr));
        Some(Def {
            decorators,
            name,
            params,
            expr,
            span,
        })
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_expr_with_result_or()
    }

    fn parse_expr_with_result_or(&mut self) -> Option<Expr> {
        self.consume_newlines();
        if self.check_symbol("|") {
            let start = self.peek_span().unwrap_or_else(|| self.previous_span());
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(start.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: start.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                start.clone(),
                arms.last().map(|arm| arm.span.clone()).unwrap_or(start),
            );
            return Some(Expr::Match {
                scrutinee: None,
                arms,
                span,
            });
        }
        let mut expr = self.parse_lambda_or_binary()?;
        // Result fallback sugar:
        //   res or "boom"
        //   res or | Err NotFound m => m | Err _ => "boom"
        //
        // This form is result-only (arms must match `Err ...` at the top level).
        loop {
            let checkpoint = self.pos;
            if !self.match_keyword("or") {
                self.pos = checkpoint;
                break;
            }
            expr = self.parse_result_or_suffix(expr)?;
        }
        Some(expr)
    }

    fn parse_expr_without_result_or(&mut self) -> Option<Expr> {
        self.consume_newlines();
        if self.check_symbol("|") {
            // Multi-clause unary function. `or` isn't allowed in the function head.
            // The bodies are parsed with the normal expression parser.
            let start = self.peek_span().unwrap_or_else(|| self.previous_span());
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(start.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: start.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                start.clone(),
                arms.last().map(|arm| arm.span.clone()).unwrap_or(start),
            );
            return Some(Expr::Match {
                scrutinee: None,
                arms,
                span,
            });
        }
        self.parse_lambda_or_binary()
    }

    fn parse_result_or_suffix(&mut self, base: Expr) -> Option<Expr> {
        let or_span = self.previous_span();
        self.consume_newlines();

        // Parse either `or <expr>` or `or | ... => ... | ...`
        let (arms, fallback_expr) = if self.consume_symbol("|") {
            let mut arms = Vec::new();
            loop {
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(or_span.clone()));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in or arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: or_span.clone(),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });

                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
            }
            (Some(arms), None)
        } else {
            let rhs = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: or_span.clone(),
            });
            (None, Some(rhs))
        };

        // Validate result-or arms: fallback-only, no success arms, and no wildcard arms.
        if let Some(arms) = &arms {
            let mut has_catch_all_err = false;
            for arm in arms {
                match &arm.pattern {
                    Pattern::Constructor { name, args, .. } if name.name == "Err" => {
                        if args.len() == 1 && matches!(&args[0], Pattern::Wildcard(_)) {
                            has_catch_all_err = true;
                        }
                    }
                    _ => {
                        self.emit_diag(
                            "E1530",
                            "`or` arms must match only `Err ...` (fallback-only)",
                            pattern_span(&arm.pattern),
                        );
                    }
                }
            }
            if !has_catch_all_err {
                // Without `Err _`, the desugared match would be non-exhaustive.
                self.emit_diag(
                    "E1531",
                    "`or` arms must include a final `| Err _ => ...` catch-all",
                    or_span.clone(),
                );
            }
        }

        // Desugar:
        //   res or rhs
        //     => res ? | Ok x => x | Err _ => rhs
        //
        //   res or | Err p => rhs | Err _ => rhs2
        //     => res ? | Ok x => x | Err p => rhs | Err _ => rhs2
        let ok_value = self.fresh_internal_name("or_ok", expr_span(&base));
        let ok_arm = MatchArm {
            pattern: self.build_ctor_pattern(
                "Ok",
                vec![Pattern::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            guard: None,
            body: Expr::Ident(ok_value.clone()),
            span: ok_value.span.clone(),
        };

        let mut out_arms = vec![ok_arm];
        if let Some(rhs) = fallback_expr {
            let err_pat = self.build_ctor_pattern("Err", vec![Pattern::Wildcard(or_span.clone())], or_span.clone());
            out_arms.push(MatchArm {
                pattern: err_pat,
                guard: None,
                body: rhs,
                span: or_span.clone(),
            });
        } else if let Some(mut parsed_arms) = arms {
            out_arms.append(&mut parsed_arms);
        }

        let span = merge_span(expr_span(&base), out_arms.last().map(|a| a.span.clone()).unwrap_or(or_span));
        Some(Expr::Match {
            scrutinee: Some(Box::new(base)),
            arms: out_arms,
            span,
        })
    }

    fn parse_lambda_or_binary(&mut self) -> Option<Expr> {
        let checkpoint = self.pos;
        let diag_checkpoint = self.diagnostics.len();
        let mut params = Vec::new();
        while let Some(pattern) = self.parse_pattern() {
            params.push(pattern);
        }
        if !params.is_empty() && self.consume_symbol("=>") {
            let body = self.parse_expr()?;
            let span = merge_span(pattern_span(&params[0]), expr_span(&body));
            return Some(Expr::Lambda {
                params,
                body: Box::new(body),
                span,
            });
        }
        self.pos = checkpoint;
        self.diagnostics.truncate(diag_checkpoint);
        self.parse_match_or_binary()
    }

    fn parse_match_or_binary(&mut self) -> Option<Expr> {
        let expr = self.parse_binary(0)?;
        if self.consume_symbol("?") {
            let mut arms = Vec::new();
            loop {
                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
                let pattern = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(expr_span(&expr)));
                let guard = if self.match_keyword("when") {
                    self.parse_guard_expr()
                } else {
                    None
                };
                self.expect_symbol("=>", "expected '=>' in match arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: expr_span(&expr),
                });
                let span = merge_span(pattern_span(&pattern), expr_span(&body));
                arms.push(MatchArm {
                    pattern,
                    guard,
                    body,
                    span,
                });
            }
            let span = merge_span(
                expr_span(&expr),
                arms.last()
                    .map(|arm| arm.span.clone())
                    .unwrap_or(expr_span(&expr)),
            );
            return Some(Expr::Match {
                scrutinee: Some(Box::new(expr)),
                arms,
                span,
            });
        }
        Some(expr)
    }

    fn parse_binary(&mut self, min_prec: u8) -> Option<Expr> {
        let mut left = self.parse_application()?;
        while let Some(op) = self.peek_symbol_text() {
            let prec = binary_prec(&op);
            if prec < min_prec || prec == 0 {
                break;
            }
            self.pos += 1;
            let right = self.parse_binary(prec + 1)?;
            let span = merge_span(expr_span(&left), expr_span(&right));
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }
        Some(left)
    }

    fn parse_guard_expr(&mut self) -> Option<Expr> {
        self.consume_newlines();
        self.parse_binary(0)
    }

    fn parse_application(&mut self) -> Option<Expr> {
        let mut expr = self.parse_postfix()?;
        let mut args = Vec::new();
        while self.is_expr_start() {
            let arg = self.parse_postfix()?;
            args.push(arg);
        }
        if args.is_empty() {
            return Some(expr);
        }
        let span = merge_span(expr_span(&expr), expr_span(args.last().unwrap()));
        expr = Expr::Call {
            func: Box::new(expr),
            args,
            span,
        };
        Some(expr)
    }

    fn parse_postfix(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.peek_symbol("(") {
                if let Some(span) = self.peek_span() {
                    if is_adjacent(&expr_span(&expr), &span) {
                        self.consume_symbol("(");
                        let mut args = Vec::new();
                        while !self.check_symbol(")") && self.pos < self.tokens.len() {
                            if let Some(arg) = self.parse_expr() {
                                args.push(arg);
                            }
                            if !self.consume_symbol(",") {
                                break;
                            }
                        }
                        let end = self.expect_symbol(")", "expected ')' to close call");
                        let span = merge_span(expr_span(&expr), end.unwrap_or(expr_span(&expr)));
                        expr = Expr::Call {
                            func: Box::new(expr),
                            args,
                            span,
                        };
                        continue;
                    }
                }
            }
            if self.peek_symbol("[") {
                if let Some(span) = self.peek_span() {
                    if is_adjacent(&expr_span(&expr), &span) {
                        self.consume_symbol("[");
                        self.consume_newlines();
                        let spread = self.consume_symbol("...");
                        let first = self.parse_expr().unwrap_or(Expr::Raw {
                            text: String::new(),
                            span: expr_span(&expr),
                        });
                        let first_span = expr_span(&first);
                        self.consume_newlines();

                        // `base[index]` (single expr) vs `base[ a, b, c ]` (bracket-list call)
                        if self.consume_symbol(",") {
                            let mut items = vec![ListItem {
                                expr: first,
                                spread,
                                span: first_span.clone(),
                            }];
                            self.consume_newlines();
                            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                                let spread = self.consume_symbol("...");
                                if let Some(item_expr) = self.parse_expr() {
                                    let span = expr_span(&item_expr);
                                    items.push(ListItem {
                                        expr: item_expr,
                                        spread,
                                        span,
                                    });
                                }
                                self.consume_newlines();
                                if !self.consume_symbol(",") {
                                    break;
                                }
                                self.consume_newlines();
                            }
                            let end = self.expect_symbol("]", "expected ']' to close bracket list");
                            let list_span = merge_span(
                                first_span.clone(),
                                end.unwrap_or_else(|| first_span.clone()),
                            );
                            let list = Expr::List {
                                items,
                                span: list_span.clone(),
                            };
                            let span = merge_span(expr_span(&expr), list_span);
                            expr = Expr::Call {
                                func: Box::new(expr),
                                args: vec![list],
                                span,
                            };
                        } else {
                            let end = self.expect_symbol("]", "expected ']' after index");
                            let span =
                                merge_span(expr_span(&expr), end.unwrap_or(expr_span(&expr)));
                            expr = Expr::Index {
                                base: Box::new(expr),
                                index: Box::new(first),
                                span,
                            };
                        }
                        continue;
                    }
                }
            }
            if self.consume_symbol(".") {
                if let Some(field) = self.consume_ident() {
                    let span = merge_span(expr_span(&expr), field.span.clone());
                    expr = Expr::FieldAccess {
                        base: Box::new(expr),
                        field,
                        span,
                    };
                    continue;
                }
            }
            break;
        }
        Some(expr)
    }

    fn is_expr_start(&self) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::Ident => {
                    if token.text == "then" || token.text == "else" || token.text == "or" {
                        return false;
                    }
                    return true;
                }
                TokenKind::Number | TokenKind::String | TokenKind::Sigil => return true,
                TokenKind::Symbol => {
                    return matches!(token.text.as_str(), "(" | "[" | "{" | "." | "-")
                }
                TokenKind::Newline => return false,
            }
        }
        self.peek_keyword("if")
            || self.peek_keyword("effect")
            || self.peek_keyword("generate")
            || self.peek_keyword("resource")
    }

    fn is_record_field_start(&self) -> bool {
        let Some(token) = self.tokens.get(self.pos) else {
            return false;
        };
        match token.kind {
            TokenKind::Ident => token
                .text
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_lowercase()),
            TokenKind::Symbol => token.text == "...",
            _ => false,
        }
    }

    fn is_pattern_start(&self) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::Ident | TokenKind::Number | TokenKind::String | TokenKind::Sigil => {
                    return true
                }
                TokenKind::Symbol => {
                    return matches!(token.text.as_str(), "(" | "[" | "{" | "-" | "~")
                }
                TokenKind::Newline => return false,
            }
        }
        false
    }

    fn parse_primary(&mut self) -> Option<Expr> {
        if self.consume_symbol("-") {
            let minus_span = self.previous_span();
            if let Some(number) = self.consume_number() {
                let (text, span) = self.consume_number_suffix(number, Some(minus_span));
                return Some(Expr::Literal(Literal::Number { text, span }));
            }
        }
        if let Some(expr) = self.parse_structured_sigil() {
            return Some(expr);
        }
        if self.consume_symbol("(") {
            if self.consume_symbol(")") {
                let span = self.previous_span();
                return Some(Expr::Tuple {
                    items: Vec::new(),
                    span,
                });
            }
            let expr = self.parse_expr()?;
            if self.consume_symbol(",") {
                let mut items = vec![expr];
                while !self.check_symbol(")") && self.pos < self.tokens.len() {
                    if let Some(item) = self.parse_expr() {
                        items.push(item);
                    }
                    if !self.consume_symbol(",") {
                        break;
                    }
                }
                let end = self.expect_symbol(")", "expected ')' to close tuple");
                let span = merge_span(expr_span(&items[0]), end.unwrap_or(expr_span(&items[0])));
                return Some(Expr::Tuple { items, span });
            }
            let _ = self.expect_symbol(")", "expected ')' to close group");
            return Some(expr);
        }

        if self.consume_symbol("[") {
            let mut items = Vec::new();
            self.consume_newlines();
            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                let spread = self.consume_symbol("...");
                if let Some(expr) = self.parse_expr() {
                    let span = expr_span(&expr);
                    items.push(ListItem { expr, spread, span });
                }
                let had_newline = self.peek_newline();
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
                if self.check_symbol("]") {
                    break;
                }
                if self.is_expr_start() {
                    if !had_newline {
                        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                        self.emit_diag("E1524", "expected ',' between list items", span);
                    }
                    continue;
                }
                break;
            }
            let end = self.expect_symbol("]", "expected ']' to close list");
            let span = merge_span(
                items
                    .first()
                    .map(|item| item.span.clone())
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Expr::List { items, span });
        }

        if self.peek_symbol("{") {
            let checkpoint = self.pos;
            let diag_checkpoint = self.diagnostics.len();
            self.pos += 1;
            self.consume_newlines();
            let is_record = self.parse_record_field().is_some();
            self.pos = checkpoint;
            self.diagnostics.truncate(diag_checkpoint);

            if is_record {
                self.consume_symbol("{");
                let mut fields = Vec::new();
                self.consume_newlines();
                while !self.check_symbol("}") && self.pos < self.tokens.len() {
                    if let Some(field) = self.parse_record_field() {
                        fields.push(field);
                        let had_newline = self.peek_newline();
                        self.consume_newlines();
                        if self.consume_symbol(",") {
                            self.consume_newlines();
                            continue;
                        }
                        if self.check_symbol("}") {
                            break;
                        }
                        if self.is_record_field_start() {
                            if !had_newline {
                                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                                self.emit_diag("E1525", "expected ',' between record fields", span);
                            }
                            continue;
                        }
                        continue;
                    }
                    self.pos += 1;
                }
                let end = self.expect_symbol("}", "expected '}' to close record");
                let span = merge_span(
                    fields
                        .first()
                        .map(|field| field.span.clone())
                        .unwrap_or(self.previous_span()),
                    end.unwrap_or(self.previous_span()),
                );
                return Some(Expr::Record { fields, span });
            }

            return Some(self.parse_block(BlockKind::Plain));
        }

        if self.consume_symbol(".") {
            if let Some(field) = self.consume_ident() {
                let span = merge_span(field.span.clone(), field.span.clone());
                return Some(Expr::FieldSection { field, span });
            }
        }

        if self.match_keyword("if") {
            let cond = self.parse_expr()?;
            self.expect_keyword("then", "expected 'then' in if expression");
            let then_branch = self.parse_expr()?;
            self.expect_keyword("else", "expected 'else' in if expression");
            let else_branch = self.parse_expr()?;
            let span = merge_span(expr_span(&cond), expr_span(&else_branch));
            return Some(Expr::If {
                cond: Box::new(cond),
                then_branch: Box::new(then_branch),
                else_branch: Box::new(else_branch),
                span,
            });
        }

        if self.match_keyword("effect") {
            return Some(self.parse_block(BlockKind::Effect));
        }
        if self.match_keyword("generate") {
            return Some(self.parse_block(BlockKind::Generate));
        }
        if self.match_keyword("resource") {
            return Some(self.parse_block(BlockKind::Resource));
        }
        if self.match_keyword("patch") {
            let start = self.previous_span();
            return Some(self.parse_patch_literal(start));
        }

        if let Some(number) = self.consume_number() {
            if let Some(dt) = self.try_parse_datetime(number.clone()) {
                return Some(Expr::Literal(dt));
            }
            let (text, span) = self.consume_number_suffix(number, None);
            return Some(Expr::Literal(Literal::Number { text, span }));
        }

        if let Some(string) = self.consume_string() {
            return Some(self.parse_text_literal_expr(string));
        }

        if let Some(sigil) = self.consume_sigil() {
            let span = sigil.span.clone();
            if let Some((tag, body, flags)) = parse_sigil_text(&sigil.text) {
                if tag == "u" && !is_probably_url(&body) {
                    self.emit_diag("E1510", "invalid url sigil", span.clone());
                }
                if (tag == "t" || tag == "dt") && !is_probably_datetime(&body) {
                    self.emit_diag("E1511", "invalid datetime sigil", span.clone());
                }
                if tag == "d" && !is_probably_date(&body) {
                    self.emit_diag("E1512", "invalid date sigil", span.clone());
                }
                if tag == "k" {
                    if let Err(msg) = crate::i18n::validate_key_text(&body) {
                        self.emit_diag(
                            "E1514",
                            &format!("invalid i18n key sigil: {msg}"),
                            span.clone(),
                        );
                    }
                }
                if tag == "m" {
                    if let Err(msg) = crate::i18n::parse_message_template(&body) {
                        self.emit_diag(
                            "E1515",
                            &format!("invalid i18n message sigil: {msg}"),
                            span.clone(),
                        );
                    }
                }
                return Some(Expr::Literal(Literal::Sigil {
                    tag,
                    body,
                    flags,
                    span,
                }));
            }
            self.emit_diag("E1513", "invalid sigil literal", span.clone());
            return Some(Expr::Literal(Literal::Sigil {
                tag: "?".to_string(),
                body: sigil.text,
                flags: String::new(),
                span,
            }));
        }

        if let Some(ident) = self.consume_ident() {
            if ident.name == "True" || ident.name == "False" {
                let value = ident.name == "True";
                return Some(Expr::Literal(Literal::Bool {
                    value,
                    span: ident.span.clone(),
                }));
            }
            return Some(Expr::Ident(ident));
        }

        None
    }

    fn parse_structured_sigil(&mut self) -> Option<Expr> {
        if !self.peek_symbol("~") {
            return None;
        }
        let checkpoint = self.pos;
        let start_span = self.peek_span().unwrap_or_else(|| self.previous_span());
        self.pos += 1;
        if self.consume_ident_text("map").is_some() {
            return self.parse_map_literal(start_span);
        }
        if self.consume_ident_text("set").is_some() {
            return self.parse_set_literal(start_span);
        }
        self.pos = checkpoint;
        None
    }

    fn parse_patch_literal(&mut self, start: Span) -> Expr {
        self.expect_symbol("{", "expected '{' to start patch literal");
        let mut fields = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            if let Some(field) = self.parse_record_field() {
                fields.push(field);
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close patch literal");
        let span = merge_span(start.clone(), end.unwrap_or(start));
        Expr::PatchLit { fields, span }
    }

    fn parse_map_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("{", "expected '{' to start map literal");
        let mut entries: Vec<(bool, Expr, Option<Expr>)> = Vec::new();
        self.consume_newlines();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            if self.consume_symbol("...") {
                if let Some(expr) = self.parse_expr() {
                    entries.push((true, expr, None));
                }
            } else if let Some(key) = self.parse_primary() {
                self.consume_newlines();
                self.expect_symbol("=>", "expected '=>' in map literal");
                let value = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: expr_span(&key),
                });
                entries.push((false, key, Some(value)));
            }
            let had_newline = self.peek_newline();
            self.consume_newlines();
            if self.consume_symbol(",") {
                self.consume_newlines();
                continue;
            }
            if self.check_symbol("}") {
                break;
            }
            if self.is_expr_start() {
                if !had_newline {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1526", "expected ',' between map entries", span);
                }
                continue;
            }
            break;
        }
        let end = self.expect_symbol("}", "expected '}' to close map literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );
        Some(self.build_map_literal_expr(entries, span))
    }

    fn parse_set_literal(&mut self, start_span: Span) -> Option<Expr> {
        self.expect_symbol("[", "expected '[' to start set literal");
        let mut entries: Vec<(bool, Expr)> = Vec::new();
        self.consume_newlines();
        while !self.check_symbol("]") && self.pos < self.tokens.len() {
            if self.consume_symbol("...") {
                if let Some(expr) = self.parse_expr() {
                    entries.push((true, expr));
                }
            } else if let Some(value) = self.parse_expr() {
                entries.push((false, value));
            }
            let had_newline = self.peek_newline();
            self.consume_newlines();
            if self.consume_symbol(",") {
                self.consume_newlines();
                continue;
            }
            if self.check_symbol("]") {
                break;
            }
            if self.is_expr_start() {
                if !had_newline {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1527", "expected ',' between set entries", span);
                }
                continue;
            }
            break;
        }
        let end = self.expect_symbol("]", "expected ']' to close set literal");
        let span = merge_span(
            start_span.clone(),
            end.unwrap_or_else(|| start_span.clone()),
        );
        Some(self.build_set_literal_expr(entries, span))
    }

    fn build_map_literal_expr(&self, entries: Vec<(bool, Expr, Option<Expr>)>, span: Span) -> Expr {
        let map_name = SpannedName {
            name: "Map".to_string(),
            span: span.clone(),
        };
        let empty = Expr::FieldAccess {
            base: Box::new(Expr::Ident(map_name.clone())),
            field: SpannedName {
                name: "empty".to_string(),
                span: span.clone(),
            },
            span: span.clone(),
        };
        let union_field = SpannedName {
            name: "union".to_string(),
            span: span.clone(),
        };
        let from_list_field = SpannedName {
            name: "fromList".to_string(),
            span: span.clone(),
        };
        let mut acc = empty;
        for (is_spread, key, value) in entries {
            let next = if is_spread {
                key
            } else {
                let value = value.unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: span.clone(),
                });
                let tuple_span = merge_span(expr_span(&key), expr_span(&value));
                let tuple = Expr::Tuple {
                    items: vec![key, value],
                    span: tuple_span.clone(),
                };
                let list = Expr::List {
                    items: vec![ListItem {
                        expr: tuple,
                        spread: false,
                        span: tuple_span,
                    }],
                    span: span.clone(),
                };
                Expr::Call {
                    func: Box::new(Expr::FieldAccess {
                        base: Box::new(Expr::Ident(map_name.clone())),
                        field: from_list_field.clone(),
                        span: span.clone(),
                    }),
                    args: vec![list],
                    span: span.clone(),
                }
            };
            acc = Expr::Call {
                func: Box::new(Expr::FieldAccess {
                    base: Box::new(Expr::Ident(map_name.clone())),
                    field: union_field.clone(),
                    span: span.clone(),
                }),
                args: vec![acc, next],
                span: span.clone(),
            };
        }
        acc
    }

    fn build_set_literal_expr(&self, entries: Vec<(bool, Expr)>, span: Span) -> Expr {
        let set_name = SpannedName {
            name: "Set".to_string(),
            span: span.clone(),
        };
        let empty = Expr::FieldAccess {
            base: Box::new(Expr::Ident(set_name.clone())),
            field: SpannedName {
                name: "empty".to_string(),
                span: span.clone(),
            },
            span: span.clone(),
        };
        let union_field = SpannedName {
            name: "union".to_string(),
            span: span.clone(),
        };
        let from_list_field = SpannedName {
            name: "fromList".to_string(),
            span: span.clone(),
        };
        let mut acc = empty;
        for (is_spread, value) in entries {
            let next = if is_spread {
                value
            } else {
                let list = Expr::List {
                    items: vec![ListItem {
                        expr: value,
                        spread: false,
                        span: span.clone(),
                    }],
                    span: span.clone(),
                };
                Expr::Call {
                    func: Box::new(Expr::FieldAccess {
                        base: Box::new(Expr::Ident(set_name.clone())),
                        field: from_list_field.clone(),
                        span: span.clone(),
                    }),
                    args: vec![list],
                    span: span.clone(),
                }
            };
            acc = Expr::Call {
                func: Box::new(Expr::FieldAccess {
                    base: Box::new(Expr::Ident(set_name.clone())),
                    field: union_field.clone(),
                    span: span.clone(),
                }),
                args: vec![acc, next],
                span: span.clone(),
            };
        }
        acc
    }

    fn parse_block(&mut self, kind: BlockKind) -> Expr {
        let start = self.previous_span();
        self.expect_symbol("{", "expected '{' to start block");
        let mut items = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            self.consume_newlines();
            if self.check_symbol("}") {
                break;
            }
            if self.match_keyword("loop") {
                let loop_start = self.previous_span();
                let _ = self.parse_pattern();
                self.expect_symbol("=", "expected '=' in loop binding");
                self.consume_newlines();
                let _ = self.parse_match_or_binary();
                self.expect_symbol("=>", "expected '=>' in loop binding");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: loop_start.clone(),
                });
                let span = merge_span(loop_start, expr_span(&body));
                items.push(BlockItem::Expr {
                    expr: Expr::Raw {
                        text: "loop".to_string(),
                        span: span.clone(),
                    },
                    span,
                });
                continue;
            }
            if self.match_keyword("yield") {
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: self.previous_span(),
                });
                let span = merge_span(self.previous_span(), expr_span(&expr));
                items.push(BlockItem::Yield { expr, span });
                continue;
            }
            if self.match_keyword("recurse") {
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: self.previous_span(),
                });
                let span = merge_span(self.previous_span(), expr_span(&expr));
                items.push(BlockItem::Recurse { expr, span });
                continue;
            }
            let checkpoint = self.pos;
            if let Some(pattern) = self.parse_pattern() {
                if self.consume_symbol("<-") {
                    let expr = self.parse_expr_without_result_or().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let expr = if matches!(kind, BlockKind::Effect) && self.peek_keyword("or") {
                        // Disambiguation:
                        // - `x <- eff or | NotFound m => ...` is effect-fallback (patterns match E)
                        // - `x <- (res or "boom")` is result-fallback (expression-level)
                        // - `x <- res or | Err _ => ...` is treated as result-fallback for ergonomics
                        let checkpoint = self.pos;
                        self.pos += 1; // consume `or` for lookahead
                        self.consume_newlines();
                        let mut looks_like_result_or = false;
                        if self.consume_symbol("|") {
                            self.consume_newlines();
                            if let Some(token) = self.tokens.get(self.pos) {
                                looks_like_result_or =
                                    token.kind == TokenKind::Ident && token.text == "Err";
                            }
                        }
                        self.pos = checkpoint;

                        let _ = self.match_keyword("or");
                        if looks_like_result_or {
                            self.parse_result_or_suffix(expr).unwrap_or(Expr::Raw {
                                text: String::new(),
                                span: self.previous_span(),
                            })
                        } else {
                            self.parse_effect_or_suffix(expr)
                        }
                    } else {
                        expr
                    };
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Bind {
                        pattern,
                        expr,
                        span,
                    });
                    continue;
                }
                if self.consume_symbol("->") {
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    items.push(BlockItem::Filter { expr, span });
                    continue;
                }
                if self.consume_symbol("=") {
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
                    let span = merge_span(pattern_span(&pattern), expr_span(&expr));
                    if matches!(kind, BlockKind::Effect) {
                        items.push(BlockItem::Let {
                            pattern,
                            expr,
                            span,
                        });
                    } else {
                        items.push(BlockItem::Bind {
                            pattern,
                            expr,
                            span,
                        });
                    }
                    continue;
                }
            }
            self.pos = checkpoint;
            if let Some(expr) = self.parse_expr() {
                let span = expr_span(&expr);
                items.push(BlockItem::Expr { expr, span });
                continue;
            }
            self.pos += 1;
        }
        let end = self.expect_symbol("}", "expected '}' to close block");
        let span = merge_span(start.clone(), end.unwrap_or(start));
        Expr::Block { kind, items, span }
    }

    fn parse_effect_or_suffix(&mut self, effect_expr: Expr) -> Expr {
        let or_span = self.previous_span();
        self.consume_newlines();

        // Parse either `or <expr>` or `or | pat => expr | ...` where patterns match the error value.
        let (patterns, fallback_expr) = if self.consume_symbol("|") {
            let mut arms = Vec::new();
            loop {
                let mut pat = self
                    .parse_pattern()
                    .unwrap_or(Pattern::Wildcard(or_span.clone()));
                // If someone wrote `Err ...` here, recover by stripping the outer `Err` and
                // still treat it as an error-pattern arm.
                if let Pattern::Constructor { name, args, .. } = &pat {
                    if name.name == "Err" && args.len() == 1 {
                        pat = args[0].clone();
                        self.emit_diag(
                            "E1532",
                            "effect `or` arms match the error value; omit the leading `Err`",
                            pattern_span(&pat),
                        );
                    }
                }

                self.expect_symbol("=>", "expected '=>' in effect or arm");
                let body = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: or_span.clone(),
                });
                arms.push((pat, body));

                self.consume_newlines();
                if !self.consume_symbol("|") {
                    break;
                }
            }
            (Some(arms), None)
        } else {
            let rhs = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: or_span.clone(),
            });
            (None, Some(rhs))
        };

        // Desugar to:
        //   effect {
        //     __res <- attempt effect_expr
        //     __res ?
        //       | Ok x => pure x
        //       | Err <pat> => pure <body>
        //       | Err e => fail e
        //   }
        //
        // This keeps error-handling explicit in core terms (attempt/?/pure/fail).
        let res_name = self.fresh_internal_name("or_res", or_span.clone());
        let res_pat = Pattern::Ident(res_name.clone());
        let attempt_call = self.build_call_expr(
            self.build_ident_expr("attempt", or_span.clone()),
            vec![effect_expr],
            or_span.clone(),
        );
        let bind_item = BlockItem::Bind {
            pattern: res_pat,
            expr: attempt_call,
            span: or_span.clone(),
        };

        let ok_value = self.fresh_internal_name("or_ok", or_span.clone());
        let ok_arm = MatchArm {
            pattern: self.build_ctor_pattern(
                "Ok",
                vec![Pattern::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            guard: None,
            body: self.build_call_expr(
                self.build_ident_expr("pure", ok_value.span.clone()),
                vec![Expr::Ident(ok_value.clone())],
                ok_value.span.clone(),
            ),
            span: ok_value.span.clone(),
        };

        let mut match_arms = vec![ok_arm];
        if let Some(rhs) = fallback_expr {
            let err_pat =
                self.build_ctor_pattern("Err", vec![Pattern::Wildcard(or_span.clone())], or_span.clone());
            let rhs_span = expr_span(&rhs);
            let body = self.build_call_expr(
                self.build_ident_expr("pure", rhs_span.clone()),
                vec![rhs],
                rhs_span,
            );
            match_arms.push(MatchArm {
                pattern: err_pat,
                guard: None,
                body,
                span: or_span.clone(),
            });
        } else if let Some(arms) = patterns {
            for (pat, body_expr) in arms {
                let err_pat = self.build_ctor_pattern("Err", vec![pat], or_span.clone());
                let body_span = expr_span(&body_expr);
                let body = self.build_call_expr(
                    self.build_ident_expr("pure", body_span.clone()),
                    vec![body_expr],
                    body_span,
                );
                match_arms.push(MatchArm {
                    pattern: err_pat,
                    guard: None,
                    body,
                    span: or_span.clone(),
                });
            }
        }

        let err_name = self.fresh_internal_name("or_err", or_span.clone());
        let err_pat = self.build_ctor_pattern("Err", vec![Pattern::Ident(err_name.clone())], or_span.clone());
        let err_body = self.build_call_expr(
            self.build_ident_expr("fail", or_span.clone()),
            vec![Expr::Ident(err_name)],
            or_span.clone(),
        );
        match_arms.push(MatchArm {
            pattern: err_pat,
            guard: None,
            body: err_body,
            span: or_span.clone(),
        });

        let match_expr = Expr::Match {
            scrutinee: Some(Box::new(Expr::Ident(res_name))),
            arms: match_arms,
            span: or_span.clone(),
        };

        let span = merge_span(or_span.clone(), or_span.clone());
        Expr::Block {
            kind: BlockKind::Effect,
            items: vec![bind_item, BlockItem::Expr { expr: match_expr, span }],
            span: or_span,
        }
    }

    fn parse_record_field(&mut self) -> Option<RecordField> {
        // Record spread: `{ ...base, field: value }`
        if self.consume_symbol("...") {
            let start_span = self.previous_span();
            let value = self.parse_expr().unwrap_or(Expr::Raw {
                text: String::new(),
                span: start_span.clone(),
            });
            let span = merge_span(start_span, expr_span(&value));
            return Some(RecordField {
                spread: true,
                path: Vec::new(),
                value,
                span,
            });
        }

        let start = self.pos;
        let mut path = Vec::new();
        if let Some(name) = self.consume_ident() {
            path.push(PathSegment::Field(name));
        } else {
            self.pos = start;
            return None;
        }
        loop {
            if self.consume_symbol(".") {
                if let Some(name) = self.consume_ident() {
                    path.push(PathSegment::Field(name));
                    continue;
                }
            }
            if self.consume_symbol("[") {
                let expr = self.parse_expr().unwrap_or(Expr::Raw {
                    text: String::new(),
                    span: self.previous_span(),
                });
                let end = self.expect_symbol("]", "expected ']' in record field path");
                path.push(PathSegment::Index(
                    expr,
                    end.unwrap_or(self.previous_span()),
                ));
                continue;
            }
            break;
        }

        if !self.consume_symbol(":") {
            self.pos = start;
            return None;
        }
        let value = self.parse_expr().unwrap_or(Expr::Raw {
            text: String::new(),
            span: self.previous_span(),
        });
        let span = merge_span(path_span(&path), expr_span(&value));
        Some(RecordField {
            spread: false,
            path,
            value,
            span,
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        if self.consume_symbol("-") {
            let minus_span = self.previous_span();
            if let Some(number) = self.consume_number() {
                let span = merge_span(minus_span, number.span.clone());
                return Some(Pattern::Literal(Literal::Number {
                    text: format!("-{}", number.text),
                    span,
                }));
            }
        }
        if let Some(ident) = self.consume_ident() {
            if ident.name == "_" {
                return Some(Pattern::Wildcard(ident.span));
            }
            if ident
                .name
                .chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
            {
                let mut args = Vec::new();
                while let Some(pattern) = self.parse_pattern() {
                    args.push(pattern);
                }
                let span = merge_span(
                    ident.span.clone(),
                    args.last().map(pattern_span).unwrap_or(ident.span.clone()),
                );
                return Some(Pattern::Constructor {
                    name: ident,
                    args,
                    span,
                });
            }
            return Some(Pattern::Ident(ident));
        }
        if self.consume_symbol("(") {
            if self.consume_symbol(")") {
                return Some(Pattern::Tuple {
                    items: Vec::new(),
                    span: self.previous_span(),
                });
            }
            let mut items = Vec::new();
            if let Some(pattern) = self.parse_pattern() {
                items.push(pattern);
            }
            if self.consume_symbol(",") {
                while !self.check_symbol(")") && self.pos < self.tokens.len() {
                    if let Some(pattern) = self.parse_pattern() {
                        items.push(pattern);
                    }
                    if !self.consume_symbol(",") {
                        break;
                    }
                }
                let end = self.expect_symbol(")", "expected ')' to close tuple pattern");
                let span = merge_span(
                    pattern_span(&items[0]),
                    end.unwrap_or(pattern_span(&items[0])),
                );
                return Some(Pattern::Tuple { items, span });
            }
            let end = self.expect_symbol(")", "expected ')' to close pattern");
            let _ = end;
            return items.into_iter().next();
        }
        if self.consume_symbol("[") {
            let mut items = Vec::new();
            let mut rest = None;
            while !self.check_symbol("]") && self.pos < self.tokens.len() {
                if self.consume_symbol("...") {
                    if let Some(pattern) = self.parse_pattern() {
                        rest = Some(Box::new(pattern));
                    }
                } else if let Some(pattern) = self.parse_pattern() {
                    items.push(pattern);
                }
                if !self.consume_symbol(",") {
                    break;
                }
            }
            let end = self.expect_symbol("]", "expected ']' to close list pattern");
            let span = merge_span(
                items
                    .first()
                    .map(pattern_span)
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Pattern::List { items, rest, span });
        }
        if self.consume_symbol("{") {
            let mut fields = Vec::new();
            while !self.check_symbol("}") && self.pos < self.tokens.len() {
                if let Some(field) = self.parse_record_pattern_field() {
                    fields.push(field);
                    continue;
                }
                self.pos += 1;
            }
            let end = self.expect_symbol("}", "expected '}' to close record pattern");
            let span = merge_span(
                fields
                    .first()
                    .map(|field| field.span.clone())
                    .unwrap_or(self.previous_span()),
                end.unwrap_or(self.previous_span()),
            );
            return Some(Pattern::Record { fields, span });
        }
        if let Some(number) = self.consume_number() {
            return Some(Pattern::Literal(Literal::Number {
                text: number.text,
                span: number.span,
            }));
        }
        if let Some(string) = self.consume_string() {
            return Some(Pattern::Literal(self.parse_text_literal_plain(string)));
        }
        if let Some(sigil) = self.consume_sigil() {
            if let Some((tag, body, flags)) = parse_sigil_text(&sigil.text) {
                return Some(Pattern::Literal(Literal::Sigil {
                    tag,
                    body,
                    flags,
                    span: sigil.span,
                }));
            }
            return Some(Pattern::Literal(Literal::Sigil {
                tag: "?".to_string(),
                body: sigil.text,
                flags: String::new(),
                span: sigil.span,
            }));
        }
        None
    }

    fn parse_record_pattern_field(&mut self) -> Option<RecordPatternField> {
        let mut path = Vec::new();
        let start = self.pos;
        if let Some(name) = self.consume_ident() {
            path.push(name);
        } else {
            self.pos = start;
            return None;
        }
        while self.consume_symbol(".") {
            if let Some(name) = self.consume_ident() {
                path.push(name);
            } else {
                break;
            }
        }
        let pattern = if self.consume_symbol("@") || self.consume_symbol(":") {
            self.parse_pattern()
                .unwrap_or(Pattern::Wildcard(self.previous_span()))
        } else {
            let last = path.last().cloned().unwrap();
            Pattern::Ident(last)
        };
        let span = merge_span(path.first().unwrap().span.clone(), pattern_span(&pattern));
        Some(RecordPatternField {
            path,
            pattern,
            span,
        })
    }

    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        let lhs = self.parse_type_and()?;
        if self.consume_symbol("->") {
            let result = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
                span: type_span(&lhs),
            });
            let span = merge_span(type_span(&lhs), type_span(&result));
            return Some(TypeExpr::Func {
                params: vec![lhs],
                result: Box::new(result),
                span,
            });
        }
        Some(lhs)
    }

    fn parse_type_and(&mut self) -> Option<TypeExpr> {
        let mut items = vec![self.parse_type_pipe()?];
        while self.consume_symbol("&") {
            let rhs = self.parse_type_pipe().unwrap_or(TypeExpr::Unknown {
                span: type_span(items.last().unwrap()),
            });
            items.push(rhs);
        }
        if items.len() == 1 {
            return Some(items.remove(0));
        }
        let span = merge_span(type_span(&items[0]), type_span(items.last().unwrap()));
        Some(TypeExpr::And { items, span })
    }

    fn parse_type_pipe(&mut self) -> Option<TypeExpr> {
        let mut lhs = self.parse_type_apply()?;
        while self.consume_symbol("|>") {
            let rhs = self.parse_type_apply().unwrap_or(TypeExpr::Unknown {
                span: type_span(&lhs),
            });
            lhs = self.apply_type_pipe(lhs, rhs);
        }
        Some(lhs)
    }

    fn apply_type_pipe(&mut self, left: TypeExpr, right: TypeExpr) -> TypeExpr {
        let span = merge_span(type_span(&left), type_span(&right));
        match right {
            TypeExpr::Apply { base, mut args, .. } => {
                args.push(left);
                TypeExpr::Apply { base, args, span }
            }
            other => TypeExpr::Apply {
                base: Box::new(other),
                args: vec![left],
                span,
            },
        }
    }

    fn parse_type_apply(&mut self) -> Option<TypeExpr> {
        let base = self.parse_type_atom()?;
        let mut args = Vec::new();
        while let Some(arg) = self.parse_type_atom() {
            args.push(arg);
        }
        if args.is_empty() {
            return Some(base);
        }
        let span = merge_span(type_span(&base), type_span(args.last().unwrap()));
        Some(TypeExpr::Apply {
            base: Box::new(base),
            args,
            span,
        })
    }

    fn parse_type_atom(&mut self) -> Option<TypeExpr> {
        if self.consume_symbol("(") {
            let mut items = Vec::new();
            if let Some(item) = self.parse_type_expr() {
                items.push(item);
                while self.consume_symbol(",") {
                    if let Some(item) = self.parse_type_expr() {
                        items.push(item);
                    }
                }
            }
            self.expect_symbol(")", "expected ')' to close type tuple");
            if items.len() == 1 {
                return Some(items.remove(0));
            }
            let span = merge_span(type_span(&items[0]), type_span(items.last().unwrap()));
            return Some(TypeExpr::Tuple { items, span });
        }
        if self.consume_symbol("{") {
            let mut fields = Vec::new();
            self.consume_newlines();
            while !self.check_symbol("}") && self.pos < self.tokens.len() {
                self.consume_newlines();
                if self.check_symbol("}") {
                    break;
                }
                if let Some(name) = self.consume_ident() {
                    self.consume_newlines();
                    self.expect_symbol(":", "expected ':' in record type");
                    self.consume_newlines();
                    if let Some(ty) = self.parse_type_expr() {
                        fields.push((name, ty));
                    }
                } else {
                    // Recovery: skip unexpected tokens inside record types.
                    self.pos += 1;
                    continue;
                }
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
                // Newline-separated fields are allowed (FieldSep includes Sep).
                if self.check_symbol("}") {
                    break;
                }
            }
            self.expect_symbol("}", "expected '}' to close record type");
            let span = fields
                .first()
                .map(|field| field.0.span.clone())
                .unwrap_or(self.previous_span());
            return Some(TypeExpr::Record { fields, span });
        }
        if self.consume_symbol("*") {
            let span = self.previous_span();
            return Some(TypeExpr::Star { span });
        }
        if let Some(name) = self.consume_ident() {
            return Some(TypeExpr::Name(name));
        }
        None
    }

    fn try_parse_datetime(&mut self, head: Token) -> Option<Literal> {
        let checkpoint = self.pos;
        if !self.consume_symbol("-") {
            return None;
        }
        let month = self.consume_number()?;
        self.expect_symbol("-", "expected '-' in datetime literal");
        let day = self.consume_number()?;
        let t_token = self.consume_ident()?;
        if !t_token.name.starts_with('T') {
            self.pos = checkpoint;
            return None;
        }
        let hour_text = t_token.name.trim_start_matches('T');
        let hour = if hour_text.is_empty() {
            self.consume_number()?
        } else {
            Token {
                kind: TokenKind::Number,
                text: hour_text.to_string(),
                span: t_token.span.clone(),
            }
        };
        self.expect_symbol(":", "expected ':' in datetime literal");
        let minute = self.consume_number()?;
        self.expect_symbol(":", "expected ':' in datetime literal");
        let second = self.consume_number()?;
        if self.consume_ident_text("Z").is_none() {
            self.pos = checkpoint;
            return None;
        }
        let text = format!(
            "{}-{}-{}T{}:{}:{}Z",
            head.text, month.text, day.text, hour.text, minute.text, second.text
        );
        let span = merge_span(head.span.clone(), second.span.clone());
        Some(Literal::DateTime { text, span })
    }

    fn parse_dotted_name(&mut self) -> Option<SpannedName> {
        let mut name = self.consume_ident()?;
        while self.consume_symbol(".") {
            if let Some(part) = self.consume_ident() {
                name.name.push('.');
                name.name.push_str(&part.name);
                name.span = merge_span(name.span.clone(), part.span.clone());
            } else {
                break;
            }
        }
        Some(name)
    }

    fn consume_ident_text(&mut self, expected: &str) -> Option<SpannedName> {
        let name = self.consume_ident()?;
        if name.name == expected {
            return Some(name);
        }
        self.pos -= 1;
        None
    }

    fn consume_name(&mut self) -> Option<SpannedName> {
        self.consume_newlines();
        if let Some(name) = self.consume_ident() {
            return Some(name);
        }
        if self.consume_symbol("(") {
            let op_token = self.consume_symbol_token()?;
            let end = self.expect_symbol(")", "expected ')' after operator name");
            let span = merge_span(op_token.span.clone(), end.unwrap_or(op_token.span.clone()));
            return Some(SpannedName {
                name: format!("({})", op_token.text),
                span,
            });
        }
        None
    }

    fn consume_ident(&mut self) -> Option<SpannedName> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Ident {
            return None;
        }
        self.pos += 1;
        Some(SpannedName {
            name: token.text.clone(),
            span: token.span.clone(),
        })
    }

    fn consume_number(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Number {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_string(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::String {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_sigil(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Sigil {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn parse_text_literal_plain(&mut self, token: Token) -> Literal {
        let span = token.span.clone();
        Literal::String {
            text: decode_text_literal(&token.text).unwrap_or_else(|| token.text.clone()),
            span,
        }
    }

    fn parse_text_literal_expr(&mut self, token: Token) -> Expr {
        let span = token.span.clone();
        let Some(inner) = strip_text_literal_quotes(&token.text) else {
            return Expr::Literal(Literal::String {
                text: token.text,
                span,
            });
        };

        let raw_chars: Vec<char> = inner.chars().collect();
        let mut parts: Vec<TextPart> = Vec::new();

        let mut text_buf = String::new();
        let mut text_start = 0usize;
        let mut i = 0usize;

        while i < raw_chars.len() {
            let ch = raw_chars[i];
            if ch == '\\' {
                if i + 1 >= raw_chars.len() {
                    self.emit_diag(
                        "E1520",
                        "unterminated escape sequence in text literal",
                        span.clone(),
                    );
                    text_buf.push('\\');
                    i += 1;
                    continue;
                }
                let esc = raw_chars[i + 1];
                match decode_escape(esc) {
                    Some(decoded) => text_buf.push(decoded),
                    None => {
                        let esc_span = span_in_text_literal(&token.span, i, i + 2);
                        self.emit_diag(
                            "E1521",
                            &format!("unknown escape sequence '\\{esc}'"),
                            esc_span,
                        );
                        text_buf.push(esc);
                    }
                }
                i += 2;
                continue;
            }

            if ch == '{' {
                if !text_buf.is_empty() {
                    let part_span = span_in_text_literal(&token.span, text_start, i);
                    parts.push(TextPart::Text {
                        text: std::mem::take(&mut text_buf),
                        span: part_span,
                    });
                }

                let open_index = i;
                let remainder: String = raw_chars[i + 1..].iter().collect();
                let Some(close_offset) = find_interpolation_close(&remainder) else {
                    let open_span = span_in_text_literal(&token.span, open_index, open_index + 1);
                    self.emit_diag("E1522", "unterminated text interpolation", open_span);
                    text_buf.push('{');
                    text_start = i;
                    i += 1;
                    continue;
                };

                let close_index = i + 1 + close_offset;
                let expr_raw: String = raw_chars[i + 1..close_index].iter().collect();
                let (expr_decoded, expr_raw_map) = decode_interpolation_source_with_map(&expr_raw);
                let expr_start_col = token.span.start.column + 1 + open_index + 1; // opening quote + '{'
                let expr_line = token.span.start.line;

                match self.parse_embedded_expr(
                    &expr_decoded,
                    &expr_raw_map,
                    expr_line,
                    expr_start_col,
                ) {
                    Some(expr) => {
                        let part_span =
                            span_in_text_literal(&token.span, open_index, close_index + 1);
                        parts.push(TextPart::Expr {
                            expr: Box::new(expr),
                            span: part_span,
                        });
                    }
                    None => {
                        let part_span =
                            span_in_text_literal(&token.span, open_index, close_index + 1);
                        parts.push(TextPart::Text {
                            text: format!("{{{expr_raw}}}"),
                            span: part_span,
                        });
                    }
                }

                i = close_index + 1;
                text_start = i;
                continue;
            }

            text_buf.push(ch);
            i += 1;
        }

        if !text_buf.is_empty() {
            let part_span = span_in_text_literal(&token.span, text_start, raw_chars.len());
            parts.push(TextPart::Text {
                text: text_buf,
                span: part_span,
            });
        }

        let has_expr = parts
            .iter()
            .any(|part| matches!(part, TextPart::Expr { .. }));
        if !has_expr {
            let mut out = String::new();
            for part in parts {
                if let TextPart::Text { text, .. } = part {
                    out.push_str(&text);
                }
            }
            return Expr::Literal(Literal::String { text: out, span });
        }

        Expr::TextInterpolate { parts, span }
    }

    fn parse_embedded_expr(
        &mut self,
        text: &str,
        raw_map: &[usize],
        line: usize,
        column: usize,
    ) -> Option<Expr> {
        let (cst_tokens, lex_diags) = lex(text);
        for diag in lex_diags {
            let mapped_span = map_span_columns(&diag.span, raw_map);
            self.diagnostics.push(FileDiagnostic {
                path: self.path.clone(),
                diagnostic: Diagnostic {
                    code: diag.code,
                    message: diag.message,
                    span: shift_span(&mapped_span, line - 1, column - 1),
                    labels: diag
                        .labels
                        .into_iter()
                        .map(|label| DiagnosticLabel {
                            message: label.message,
                            span: shift_span(
                                &map_span_columns(&label.span, raw_map),
                                line - 1,
                                column - 1,
                            ),
                        })
                        .collect(),
                },
            });
        }
        let mut tokens = filter_tokens(&cst_tokens);
        for token in &mut tokens {
            let mapped_span = map_span_columns(&token.span, raw_map);
            token.span = shift_span(&mapped_span, line - 1, column - 1);
        }

        let mut parser = Parser::new(tokens, Path::new(&self.path));
        let expr = parser.parse_expr();
        parser.consume_newlines();
        if parser.pos < parser.tokens.len() {
            let span = parser.peek_span().unwrap_or_else(|| parser.previous_span());
            parser.emit_diag("E1523", "unexpected tokens in text interpolation", span);
        }
        self.diagnostics.append(&mut parser.diagnostics);
        expr
    }

    fn consume_number_suffix(&mut self, number: Token, prefix: Option<Span>) -> (String, Span) {
        let mut text = number.text.clone();
        let mut span = number.span.clone();
        if let Some(prefix_span) = prefix {
            text = format!("-{text}");
            span = merge_span(prefix_span, span);
        }
        if let Some(suffix) = self.consume_adjacent_suffix(&number.span) {
            text.push_str(&suffix.text);
            span = merge_span(span, suffix.span);
        }
        (text, span)
    }

    fn consume_adjacent_suffix(&mut self, number_span: &Span) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if !is_adjacent(number_span, &token.span) {
            return None;
        }
        if token.kind == TokenKind::Ident || (token.kind == TokenKind::Symbol && token.text == "%")
        {
            self.pos += 1;
            return Some(token.clone());
        }
        None
    }

    fn consume_symbol_token(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Symbol || token.text == ")" {
            return None;
        }
        self.pos += 1;
        Some(token.clone())
    }

    fn consume_newlines(&mut self) {
        while self.peek_newline() {
            self.pos += 1;
        }
    }

    fn peek_newline(&self) -> bool {
        matches!(
            self.tokens.get(self.pos).map(|token| &token.kind),
            Some(TokenKind::Newline)
        )
    }

    fn peek_symbol_text(&self) -> Option<String> {
        let token = self.tokens.get(self.pos)?;
        if token.kind != TokenKind::Symbol {
            return None;
        }
        Some(token.text.clone())
    }

    fn consume_symbol(&mut self, symbol: &str) -> bool {
        let token = match self.tokens.get(self.pos) {
            Some(token) => token,
            None => return false,
        };
        if token.kind == TokenKind::Symbol && token.text == symbol {
            self.pos += 1;
            return true;
        }
        false
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            if token.kind == TokenKind::Ident && token.text == keyword {
                self.pos += 1;
                return true;
            }
        }
        false
    }

    fn expect_keyword(&mut self, keyword: &str, message: &str) {
        if !self.match_keyword(keyword) {
            let span = self.peek_span().unwrap_or_else(|| self.previous_span());
            self.emit_diag("E1500", message, span);
        }
    }

    fn expect_symbol(&mut self, symbol: &str, message: &str) -> Option<Span> {
        if self.consume_symbol(symbol) {
            return Some(self.previous_span());
        }
        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
        self.emit_diag("E1501", message, span.clone());
        None
    }

    fn check_symbol(&self, symbol: &str) -> bool {
        self.peek_symbol(symbol)
    }

    fn peek_symbol(&self, symbol: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            return token.kind == TokenKind::Symbol && token.text == symbol;
        }
        false
    }

    fn previous_span(&self) -> Span {
        if self.pos == 0 {
            return Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            };
        }
        if self.pos > self.tokens.len() {
            return self.tokens.last().map(|t| t.span.clone()).unwrap_or(Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            });
        }
        if self.pos >= self.tokens.len() {
            return self.tokens.last().map(|t| t.span.clone()).unwrap_or(Span {
                start: Position { line: 1, column: 1 },
                end: Position { line: 1, column: 1 },
            });
        }
        self.tokens[self.pos - 1].span.clone()
    }

    fn peek_span(&self) -> Option<Span> {
        self.tokens.get(self.pos).map(|token| token.span.clone())
    }

    fn emit_diag(&mut self, code: &str, message: &str, span: Span) {
        self.diagnostics.push(FileDiagnostic {
            path: self.path.clone(),
            diagnostic: Diagnostic {
                code: code.to_string(),
                message: message.to_string(),
                span,
                labels: Vec::new(),
            },
        });
    }

    fn recover_to_item(&mut self) {
        let start = self.pos;
        while self.pos < self.tokens.len() {
            if self.peek_symbol("}") {
                break;
            }
            if self.peek_keyword("export")
                || self.peek_keyword("use")
                || self.peek_keyword("class")
                || self.peek_keyword("instance")
                || self.peek_keyword("domain")
            {
                break;
            }
            self.pos += 1;
        }
        // Always advance at least one token to prevent caller loops
        if self.pos == start && self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn recover_to_module(&mut self) {
        while self.pos < self.tokens.len() {
            if self.peek_keyword("module") {
                break;
            }
            self.pos += 1;
        }
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            return token.kind == TokenKind::Ident && token.text == keyword;
        }
        false
    }
}

fn binary_prec(op: &str) -> u8 {
    match op {
        "<|" | "|>" => 1,
        "||" => 2,
        "&&" => 3,
        "==" | "!=" | "<" | ">" | "<=" | ">=" => 4,
        ".." => 5,
        "+" | "-" => 6,
        "*" | "/" | "%" => 7,
        _ => 0,
    }
}

fn merge_span(start: Span, end: Span) -> Span {
    Span {
        start: start.start,
        end: end.end,
    }
}

fn shift_span(span: &Span, line_offset: usize, col_offset: usize) -> Span {
    Span {
        start: Position {
            line: span.start.line + line_offset,
            column: span.start.column + col_offset,
        },
        end: Position {
            line: span.end.line + line_offset,
            column: span.end.column + col_offset,
        },
    }
}

fn strip_text_literal_quotes(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('"')?;
    Some(inner.strip_suffix('"').unwrap_or(inner))
}

fn decode_escape(ch: char) -> Option<char> {
    match ch {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '\\' => Some('\\'),
        '"' => Some('"'),
        '{' => Some('{'),
        '}' => Some('}'),
        _ => None,
    }
}

fn decode_text_literal(text: &str) -> Option<String> {
    let inner = strip_text_literal_quotes(text)?;
    let mut out = String::new();
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' && i + 1 < chars.len() {
            let esc = chars[i + 1];
            out.push(decode_escape(esc).unwrap_or(esc));
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    Some(out)
}

fn span_in_text_literal(token_span: &Span, start: usize, end: usize) -> Span {
    let line = token_span.start.line;
    let base_col = token_span.start.column + 1;
    let start_col = base_col + start;
    let end_col = if end > start {
        base_col + end - 1
    } else {
        start_col
    };
    Span {
        start: Position {
            line,
            column: start_col,
        },
        end: Position {
            line,
            column: end_col,
        },
    }
}

fn find_interpolation_close(remainder: &str) -> Option<usize> {
    let (decoded, raw_map) = decode_interpolation_source_with_map(remainder);
    let (tokens, _) = lex(&decoded);
    let mut depth = 0usize;
    for token in tokens {
        if token.kind != "symbol" {
            continue;
        }
        match token.text.as_str() {
            "{" => depth += 1,
            "}" => {
                if depth == 0 {
                    let decoded_index = decoded_char_index(
                        &decoded,
                        token.span.start.line,
                        token.span.start.column,
                    )?;
                    return raw_map.get(decoded_index).copied();
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn decode_interpolation_source_with_map(raw: &str) -> (String, Vec<usize>) {
    let raw_chars: Vec<char> = raw.chars().collect();
    let mut decoded = String::new();
    let mut raw_map = Vec::new();
    let mut i = 0usize;
    while i < raw_chars.len() {
        let ch = raw_chars[i];
        if ch == '\\' && i + 1 < raw_chars.len() {
            let esc = raw_chars[i + 1];
            if matches!(esc, '\\' | '"' | '{' | '}') {
                decoded.push(esc);
                raw_map.push(i + 1);
                i += 2;
                continue;
            }
        }
        decoded.push(ch);
        raw_map.push(i);
        i += 1;
    }
    (decoded, raw_map)
}

fn decoded_char_index(text: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let mut line_offsets = vec![0usize];
    let mut idx = 0usize;
    for ch in text.chars() {
        idx += 1;
        if ch == '\n' {
            line_offsets.push(idx);
        }
    }
    let line_start = *line_offsets.get(line - 1)?;
    Some(line_start + (column - 1))
}

fn map_span_columns(span: &Span, raw_map: &[usize]) -> Span {
    let start_idx = span.start.column.saturating_sub(1);
    let end_idx = span.end.column.saturating_sub(1);
    let start_raw = raw_map.get(start_idx).copied().unwrap_or(start_idx);
    let end_raw = raw_map.get(end_idx).copied().unwrap_or(end_idx);
    Span {
        start: Position {
            line: span.start.line,
            column: start_raw + 1,
        },
        end: Position {
            line: span.end.line,
            column: end_raw + 1,
        },
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => literal_span(literal),
        Expr::TextInterpolate { span, .. } => span.clone(),
        Expr::List { span, .. }
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
        | Expr::Block { span, .. } => span.clone(),
        Expr::Raw { span, .. } => span.clone(),
    }
}

fn pattern_span(pattern: &Pattern) -> Span {
    match pattern {
        Pattern::Wildcard(span) => span.clone(),
        Pattern::Ident(name) => name.span.clone(),
        Pattern::Literal(literal) => literal_span(literal),
        Pattern::Constructor { span, .. }
        | Pattern::Tuple { span, .. }
        | Pattern::List { span, .. }
        | Pattern::Record { span, .. } => span.clone(),
    }
}

fn type_span(ty: &TypeExpr) -> Span {
    match ty {
        TypeExpr::Name(name) => name.span.clone(),
        TypeExpr::And { span, .. }
        | TypeExpr::Apply { span, .. }
        | TypeExpr::Func { span, .. }
        | TypeExpr::Record { span, .. }
        | TypeExpr::Tuple { span, .. }
        | TypeExpr::Star { span }
        | TypeExpr::Unknown { span } => span.clone(),
    }
}

fn literal_span(literal: &Literal) -> Span {
    match literal {
        Literal::Number { span, .. }
        | Literal::String { span, .. }
        | Literal::Sigil { span, .. }
        | Literal::Bool { span, .. }
        | Literal::DateTime { span, .. } => span.clone(),
    }
}

fn parse_sigil_text(text: &str) -> Option<(String, String, String)> {
    let mut iter = text.chars();
    if iter.next()? != '~' {
        return None;
    }
    let mut tag = String::new();
    let mut open = None;
    for ch in iter.by_ref() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            tag.push(ch);
            continue;
        }
        open = Some(ch);
        break;
    }
    let open = open?;
    let close = match open {
        '/' => '/',
        '"' => '"',
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return None,
    };
    let mut body = String::new();
    let mut escaped = false;
    for ch in iter.by_ref() {
        if escaped {
            body.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            body.push(ch);
            escaped = true;
            continue;
        }
        if ch == close {
            break;
        }
        body.push(ch);
    }
    let flags: String = iter.take_while(|c| c.is_ascii_alphabetic()).collect();
    Some((tag, body, flags))
}

fn is_probably_url(text: &str) -> bool {
    let text = text.trim();
    let Some((scheme, rest)) = text.split_once("://") else {
        return false;
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return false;
    }
    !rest.is_empty() && !rest.starts_with('/')
}

fn is_probably_date(text: &str) -> bool {
    let text = text.trim();
    let parts: Vec<&str> = text.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn is_probably_datetime(text: &str) -> bool {
    let text = text.trim();
    let Some((date, time)) = text.split_once('T') else {
        return false;
    };
    if !is_probably_date(date) {
        return false;
    }
    let Some(time) = time.strip_suffix('Z') else {
        return false;
    };
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 2
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

fn path_span(path: &[PathSegment]) -> Span {
    match (path.first(), path.last()) {
        (Some(PathSegment::Field(first)), Some(PathSegment::Field(last))) => {
            merge_span(first.span.clone(), last.span.clone())
        }
        (Some(PathSegment::Field(first)), Some(PathSegment::Index(_, span))) => {
            merge_span(first.span.clone(), span.clone())
        }
        _ => Span {
            start: Position { line: 1, column: 1 },
            end: Position { line: 1, column: 1 },
        },
    }
}

fn is_adjacent(left: &Span, right: &Span) -> bool {
    left.end.line == right.start.line && left.end.column + 1 == right.start.column
}
