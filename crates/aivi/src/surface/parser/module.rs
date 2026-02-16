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
        let mut reported_leading_tokens = false;
        while self.pos < self.tokens.len() {
            let annotations = self.consume_decorators();
            if self.peek_keyword("module") {
                self.pos += 1;
                let module_kw_span = self.previous_span();
                if let Some(module) = self.parse_module(annotations) {
                    if modules.is_empty() {
                        modules.push(module);
                    } else {
                        self.emit_diag(
                            "E1516",
                            "only one `module` declaration is allowed per file",
                            module_kw_span,
                        );
                    }
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
            } else if modules.is_empty() {
                // v0.1: the file must start with `module ...` (after optional module decorators).
                // Emit a single, clear diagnostic and then recover to the first `module`.
                if !reported_leading_tokens {
                    if let Some(token) = self.tokens.get(self.pos) {
                        if token.kind != TokenKind::Newline {
                            reported_leading_tokens = true;
                            self.emit_diag(
                                "E1519",
                                "`module` declaration must be the first item in the file",
                                token.span.clone(),
                            );
                            self.recover_to_module();
                            continue;
                        }
                    }
                }
                self.pos += 1;
            } else {
                self.pos += 1;
            }
        }
        // In v0.1 there must be exactly one module per file. When users are typing in an editor
        // it's easy to start with just definitions; emit a clear parse diagnostic instead of
        // returning an empty module set (which would otherwise suppress downstream checking).
        if modules.is_empty() {
            if let Some(first) = self.tokens.first() {
                self.emit_diag("E1517", "expected `module` declaration", first.span.clone());
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
        let mut legacy_braced_body = false;
        if self.consume_symbol("=") {
            self.emit_diag(
                "E1518",
                "braced module bodies were removed; use `module x.y.z` and put the module in its own file",
                self.previous_span(),
            );
            self.consume_newlines();
            if self.consume_symbol("{") {
                legacy_braced_body = true;
            }
        } else if self.consume_symbol("{") {
            self.emit_diag(
                "E1518",
                "braced module bodies were removed; use `module x.y.z` and put the module in its own file",
                self.previous_span(),
            );
            legacy_braced_body = true;
        }
        let mut exports = Vec::new();
        let mut uses = Vec::new();
        let mut items = Vec::new();
        loop {
            if self.pos >= self.tokens.len() {
                break;
            }
            let loop_start = self.pos;
            self.consume_newlines();
            if legacy_braced_body && self.check_symbol("}") {
                break;
            }
            if self.peek_keyword("module") {
                if legacy_braced_body {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag(
                        "E1540",
                        "nested `module` declarations are not supported; use one module per file with dot paths",
                        span,
                    );
                    self.pos += 1;
                    self.recover_to_item();
                    continue;
                }
                // Stop the current module body so the outer loop can see the next `module`.
                break;
            }
            let decorators = self.consume_decorators();
            self.validate_item_decorators(&decorators);
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
                if let Some(class_decl) = self.parse_class_decl(decorators) {
                    items.push(ModuleItem::ClassDecl(class_decl));
                }
                continue;
            }
            if self.match_keyword("instance") {
                if let Some(instance_decl) = self.parse_instance_decl(decorators) {
                    items.push(ModuleItem::InstanceDecl(instance_decl));
                }
                continue;
            }
            if self.match_keyword("domain") {
                if let Some(domain) = self.parse_domain_decl(decorators) {
                    items.push(ModuleItem::DomainDecl(domain));
                }
                continue;
            }

            if self.peek_keyword("type")
                && self.tokens.get(self.pos + 1).is_some_and(|tok| {
                    tok.kind == TokenKind::Ident
                        && tok
                            .text
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_uppercase())
                })
            {
                // Legacy syntax: `type T = ...` / `type T`.
                // `type` is not part of the language; recover by dropping it and parsing the
                // following type declaration/alias.
                let _ = self.match_keyword("type");
                let span = self.previous_span();
                self.emit_diag(
                    "E1542",
                    "`type` keyword is not part of AIVI syntax; write `Name = ...` (or `Name` for opaque types)",
                    span,
                );
                if let Some(item) = self.parse_type_decl_or_alias(decorators) {
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
        let end_span = if legacy_braced_body {
            self.expect_symbol("}", "expected '}' to close module body")
                .unwrap_or_else(|| module_kw.clone())
        } else {
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

    fn parse_export_list(&mut self) -> Vec<crate::surface::ExportItem> {
        let mut exports = Vec::new();
        loop {
            if self.match_keyword("domain") {
                if let Some(name) = self.consume_ident() {
                    exports.push(crate::surface::ExportItem {
                        kind: crate::surface::ScopeItemKind::Domain,
                        name,
                    });
                } else {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1500", "expected domain name after 'domain'", span);
                    break;
                }
            } else if let Some(name) = self.consume_ident() {
                exports.push(crate::surface::ExportItem {
                    kind: crate::surface::ScopeItemKind::Value,
                    name,
                });
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
        let alias = if self.match_keyword("as") {
            let as_span = self.previous_span();
            match self.consume_ident() {
                Some(name) => Some(name),
                None => {
                    self.emit_diag("E1500", "expected alias name after 'as'", as_span);
                    None
                }
            }
        } else {
            None
        };
        let mut items = Vec::new();
        let mut wildcard = true;
        if self.consume_symbol("(") {
            wildcard = false;
            while !self.check_symbol(")") && self.pos < self.tokens.len() {
                if self.match_keyword("domain") {
                    if let Some(name) = self.consume_ident() {
                        items.push(crate::surface::UseItem {
                            kind: crate::surface::ScopeItemKind::Domain,
                            name,
                        });
                    } else {
                        let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                        self.emit_diag("E1500", "expected domain name after 'domain'", span);
                        break;
                    }
                } else if let Some(name) = self.consume_ident() {
                    items.push(crate::surface::UseItem {
                        kind: crate::surface::ScopeItemKind::Value,
                        name,
                    });
                }
                if !self.consume_symbol(",") {
                    break;
                }
            }
            self.expect_symbol(")", "expected ')' to close import list");
        }
        let span = match &alias {
            Some(alias) => merge_span(start, alias.span.clone()),
            None => merge_span(start, module.span.clone()),
        };
        Some(UseDecl {
            module,
            items,
            span,
            wildcard,
            alias,
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
                "static"
                    | "inline"
                    | "deprecated"
                    | "test"
                    | "debug"
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
                    } else if !matches!(decorator.arg, Some(Expr::Literal(Literal::String { .. })))
                    {
                        let span = decorator
                            .arg
                            .as_ref()
                            .map(expr_span)
                            .unwrap_or_else(|| decorator.span.clone());
                        self.emit_diag(
                            "E1510",
                            "`@deprecated` expects a string literal argument",
                            span,
                        );
                    }
                }
                "debug" => {
                    // `@debug` supports an optional argument list (validated during module checks).
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

    fn reject_debug_decorators(&mut self, decorators: &[Decorator], item: &str) {
        for decorator in decorators {
            if decorator.name.name == "debug" {
                self.emit_diag(
                    "E1514",
                    &format!("`@debug` can only be applied to function definitions (not {item})"),
                    decorator.span.clone(),
                );
            }
        }
    }
}
