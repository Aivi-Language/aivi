struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<FileDiagnostic>,
    path: String,
    gensym: u32,
    /// When set, plain `{ ... }` blocks inside a `loop` body are promoted to
    /// the given block kind so that keywords like `recurse` are recognised.
    loop_block_kind: Option<BlockKind>,
}

impl Parser {
    fn new(tokens: Vec<Token>, path: &Path) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            path: path.display().to_string(),
            gensym: 0,
            loop_block_kind: None,
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
                name: name.into(),
                span: span.clone(),
            },
            args,
            span,
        }
    }

    fn build_ident_expr(&self, name: &str, span: Span) -> Expr {
        Expr::Ident(SpannedName {
            name: name.into(),
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
                if let Some((item, export_item)) =
                    self.parse_export_prefixed_item(decorators.clone())
                {
                    items.push(item);
                    exports.push(export_item);
                    continue;
                }
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
                uses.extend(self.parse_use_decls());
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
            if self.match_keyword("opaque") {
                if let Some(item) = self.parse_type_decl_or_alias_opaque(decorators) {
                    items.push(item);
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

    fn parse_export_prefixed_item(
        &mut self,
        decorators: Vec<Decorator>,
    ) -> Option<(ModuleItem, crate::surface::ExportItem)> {
        let checkpoint = self.pos;
        let diag_checkpoint = self.diagnostics.len();

        let parsed_item = if self.match_keyword("class") {
            self.parse_class_decl(decorators.clone())
                .map(ModuleItem::ClassDecl)
        } else if self.match_keyword("instance") {
            self.parse_instance_decl(decorators.clone())
                .map(ModuleItem::InstanceDecl)
        } else if self.peek_keyword("domain") {
            // Check if this is `export domain DomainName over ...` (a domain declaration)
            // vs. `export domain DomainName` (an export-list entry).
            // Only parse as a declaration if `over` keyword follows the name.
            let looks_like_decl = self.tokens.get(self.pos + 1).is_some_and(|tok| {
                tok.kind == TokenKind::Ident
            }) && self.tokens.get(self.pos + 2).is_some_and(|tok| {
                tok.kind == TokenKind::Ident && tok.text == "over"
            });
            if looks_like_decl {
                let _ = self.match_keyword("domain");
                self.parse_domain_decl(decorators.clone())
                    .map(ModuleItem::DomainDecl)
            } else {
                // Not a domain declaration — let parse_export_list handle `domain Name` syntax.
                None
            }
        } else if self.match_keyword("opaque") {
            self.parse_type_decl_or_alias_opaque(decorators.clone())
        } else if self.peek_keyword("type")
            && self.tokens.get(self.pos + 1).is_some_and(|tok| {
                tok.kind == TokenKind::Ident
                    && tok
                        .text
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_uppercase())
            })
        {
            // Legacy syntax: `export type T = ...`.
            let _ = self.match_keyword("type");
            let span = self.previous_span();
            self.emit_diag(
                "E1542",
                "`type` keyword is not part of AIVI syntax; write `export Name = ...`",
                span,
            );
            self.parse_type_decl_or_alias(decorators)
        } else if self.looks_like_export_prefixed_type_or_def() {
            self.parse_type_or_def(decorators)
        } else {
            None
        };

        let Some(item) = parsed_item else {
            self.pos = checkpoint;
            self.diagnostics.truncate(diag_checkpoint);
            return None;
        };

        let export_item = match &item {
            ModuleItem::Def(def) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: def.name.clone(),
            },
            ModuleItem::TypeSig(sig) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: sig.name.clone(),
            },
            ModuleItem::TypeDecl(ty) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: ty.name.clone(),
            },
            ModuleItem::TypeAlias(alias) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: alias.name.clone(),
            },
            ModuleItem::ClassDecl(class_decl) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: class_decl.name.clone(),
            },
            ModuleItem::InstanceDecl(instance_decl) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Value,
                name: instance_decl.name.clone(),
            },
            ModuleItem::DomainDecl(domain_decl) => crate::surface::ExportItem {
                kind: crate::surface::ScopeItemKind::Domain,
                name: domain_decl.name.clone(),
            },
        };
        Some((item, export_item))
    }

    fn looks_like_export_prefixed_type_or_def(&self) -> bool {
        let Some(first) = self.tokens.get(self.pos) else {
            return false;
        };
        match first.kind {
            TokenKind::Ident => {
                if matches!(
                    first.text.as_str(),
                    "module" | "export" | "use" | "class" | "instance" | "domain"
                ) {
                    return false;
                }
            }
            TokenKind::Symbol => {
                if first.text != "(" {
                    return false;
                }
            }
            _ => return false,
        }
        let mut scan = self.pos;
        while let Some(tok) = self.tokens.get(scan) {
            if tok.kind == TokenKind::Newline || (tok.kind == TokenKind::Symbol && tok.text == "}")
            {
                break;
            }
            if tok.kind == TokenKind::Symbol && (tok.text == ":" || tok.text == "=") {
                return true;
            }
            scan += 1;
        }
        false
    }

    fn parse_export_list(&mut self) -> Vec<crate::surface::ExportItem> {
        let mut exports = Vec::new();
        loop {
            // A blank line (two or more consecutive newlines) ends the export
            // list. Single newlines are treated as line continuations.
            if self.at_blank_line() {
                break;
            }
            self.consume_newlines();
            // Stop when the next token looks like a definition (ident followed by
            // `=`, `:`, or `(`), a keyword, or end-of-file — not an export name.
            if self.looks_like_definition_start() {
                break;
            }
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
            // Commas and newlines both separate export items.
            self.consume_symbol(",");
        }
        exports
    }

    /// Returns `true` when a blank line separates the last consumed token from
    /// the next one.  The lexer synthesises one `Newline` per line-change, so a
    /// blank line shows up as a gap of >1 between the previous real token's
    /// line and the `Newline` token's span (which carries the *next* real
    /// token's position).
    fn at_blank_line(&self) -> bool {
        if !matches!(
            self.tokens.get(self.pos).map(|t| &t.kind),
            Some(TokenKind::Newline)
        ) {
            return false;
        }
        // Find the previous non-newline token's ending line.
        let prev_line = (0..self.pos)
            .rev()
            .find_map(|i| {
                let t = &self.tokens[i];
                if t.kind != TokenKind::Newline {
                    Some(t.span.end.line)
                } else {
                    None
                }
            })
            .unwrap_or(0);
        // The Newline token's span points at the next real token's position.
        let next_line = self.tokens[self.pos].span.start.line;
        next_line > prev_line + 1
    }

    /// Returns `true` when the current position looks like the beginning of a
    /// definition or other module-level item rather than an export-list name.
    fn looks_like_definition_start(&self) -> bool {
        let Some(tok) = self.tokens.get(self.pos) else {
            return true;
        };
        if tok.kind == TokenKind::Ident {
            match tok.text.as_str() {
                // Keywords that unambiguously start module items.
                "module" | "export" | "use" | "class" | "instance" => {
                    return true;
                }
                // `domain Name` is a valid export-list entry, but
                // `domain Name over ...` / `domain Name =` is a declaration.
                "domain" => {
                    // Look past `domain Name` to see if it's a declaration.
                    if let Some(name_tok) = self.tokens.get(self.pos + 1) {
                        if name_tok.kind == TokenKind::Ident {
                            if let Some(after) = self.tokens.get(self.pos + 2) {
                                if after.kind == TokenKind::Ident
                                    && after.text.as_str() == "over"
                                {
                                    return true;
                                }
                                if after.kind == TokenKind::Symbol
                                    && after.text.as_str() == "="
                                {
                                    return true;
                                }
                            }
                        }
                    }
                    return false;
                }
                _ => {}
            }
            // An identifier followed by `=`, `:`, or `(` is a definition, not an export name.
            if let Some(next) = self.tokens.get(self.pos + 1) {
                if next.kind == TokenKind::Symbol
                    && matches!(next.text.as_str(), "=" | ":" | "(")
                {
                    return true;
                }
            }
        }
        // Decorators (`@...`) start a new item.
        if tok.kind == TokenKind::Symbol && tok.text == "@" {
            return true;
        }
        false
    }

    fn parse_use_decls(&mut self) -> Vec<UseDecl> {
        let start = self.previous_span();
        let Some(module) = self.parse_dotted_name() else {
            return Vec::new();
        };
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
        if self.consume_symbol("(") {
            // Disambiguate grouped vs selective import by lookahead:
            // after `(`, skip newlines; if we see a lowercase non-keyword ident
            // followed (after optional newlines) by `(`, it's a grouped import.
            let checkpoint = self.pos;
            self.consume_newlines();
            let is_grouped = self.is_grouped_import_start();
            self.pos = checkpoint;

            if is_grouped {
                return self.parse_grouped_import_items(&module, &start);
            }

            // Selective import (existing logic)
            let items = self.parse_use_item_list();
            self.expect_symbol(")", "expected ')' to close import list");
            let span = merge_span(start, self.previous_span());
            return vec![UseDecl {
                module,
                items,
                span,
                wildcard: false,
                alias,
            }];
        }
        // Wildcard import (no parens)
        let span = match &alias {
            Some(alias) => merge_span(start, alias.span.clone()),
            None => merge_span(start, module.span.clone()),
        };
        vec![UseDecl {
            module,
            items: Vec::new(),
            span,
            wildcard: true,
            alias,
        }]
    }

    /// Parse the comma/newline-separated list of import items inside `(...)`.
    /// Assumes the opening `(` has already been consumed. Does NOT consume the
    /// closing `)`.
    fn parse_use_item_list(&mut self) -> Vec<crate::surface::UseItem> {
        let mut items = Vec::new();
        self.consume_newlines();
        while !self.check_symbol(")") && self.pos < self.tokens.len() {
            if self.match_keyword("domain") {
                if let Some(name) = self.consume_ident() {
                    items.push(crate::surface::UseItem {
                        kind: crate::surface::ScopeItemKind::Domain,
                        name,
                        alias: None,
                    });
                } else {
                    let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                    self.emit_diag("E1500", "expected domain name after 'domain'", span);
                    break;
                }
            } else if let Some(name) = self.consume_ident() {
                let alias = if self.match_keyword("as") {
                    let as_span = self.previous_span();
                    match self.consume_ident() {
                        Some(a) => Some(a),
                        None => {
                            self.emit_diag(
                                "E1500",
                                "expected alias name after 'as'",
                                as_span,
                            );
                            None
                        }
                    }
                } else {
                    None
                };
                items.push(crate::surface::UseItem {
                    kind: crate::surface::ScopeItemKind::Value,
                    name,
                    alias,
                });
            }
            let pos_before = self.pos;
            self.consume_newlines();
            self.consume_symbol(",");
            self.consume_newlines();
            if self.pos == pos_before && !self.check_symbol(")") {
                break;
            }
        }
        items
    }

    /// Lookahead check: does the current position (after newlines have been
    /// skipped) look like the start of a grouped import sub-item, i.e.
    /// `lowerIdent (` ?
    fn is_grouped_import_start(&self) -> bool {
        let mut pos = self.pos;
        // Skip newlines
        while pos < self.tokens.len() && self.tokens[pos].kind == TokenKind::Newline {
            pos += 1;
        }
        if pos >= self.tokens.len() {
            return false;
        }
        let tok = &self.tokens[pos];
        // Must be a lowercase, non-keyword ident
        if tok.kind != TokenKind::Ident {
            return false;
        }
        let is_lower = tok.text.chars().next().is_some_and(|c| c.is_ascii_lowercase());
        if !is_lower {
            return false;
        }
        if crate::syntax::KEYWORDS_ALL.contains(&tok.text.as_str()) {
            return false;
        }
        // Skip to next non-newline token
        let mut next_pos = pos + 1;
        while next_pos < self.tokens.len() && self.tokens[next_pos].kind == TokenKind::Newline {
            next_pos += 1;
        }
        if next_pos >= self.tokens.len() {
            return false;
        }
        let next_tok = &self.tokens[next_pos];
        next_tok.kind == TokenKind::Symbol && next_tok.text == "("
    }

    /// Parse grouped import items: `use foo.bar ( sub1 (...) sub2 (...) )`.
    /// Assumes the opening `(` has already been consumed.
    fn parse_grouped_import_items(
        &mut self,
        parent_module: &SpannedName,
        start: &Span,
    ) -> Vec<UseDecl> {
        let mut decls = Vec::new();
        self.consume_newlines();
        while !self.check_symbol(")") && self.pos < self.tokens.len() {
            let Some(sub_module) = self.consume_ident() else {
                break;
            };
            if !self.consume_symbol("(") {
                self.emit_diag(
                    "E1500",
                    "expected '(' after sub-module name in grouped import",
                    sub_module.span.clone(),
                );
                break;
            }
            let items = self.parse_use_item_list();
            self.expect_symbol(")", "expected ')' to close sub-module import list");

            let full_name = format!("{}.{}", parent_module.name, sub_module.name);
            let module_span = merge_span(parent_module.span.clone(), sub_module.span.clone());
            let decl_span = merge_span(start.clone(), self.previous_span());

            decls.push(UseDecl {
                module: SpannedName {
                    name: full_name,
                    span: module_span,
                },
                items,
                span: decl_span,
                wildcard: false,
                alias: None,
            });

            let pos_before = self.pos;
            self.consume_newlines();
            self.consume_symbol(",");
            self.consume_newlines();
            if self.pos == pos_before && !self.check_symbol(")") {
                break;
            }
        }
        self.expect_symbol(")", "expected ')' to close grouped import");
        decls
    }

    fn validate_module_decorators(&mut self, decorators: &[Decorator]) {
        for decorator in decorators {
            if !matches!(decorator.name.name.as_str(), "no_prelude" | "test") {
                self.emit_diag(
                    "E1506",
                    &format!("unknown module decorator `@{}`", decorator.name.name),
                    decorator.span.clone(),
                );
                continue;
            }
            if decorator.name.name == "no_prelude" && decorator.arg.is_some() {
                self.emit_diag(
                    "E1512",
                    "`@no_prelude` does not take an argument",
                    decorator.span.clone(),
                );
            }
            if decorator.name.name == "test" && decorator.arg.is_some() {
                self.emit_diag(
                    "E1512",
                    "`@test` on a module does not take an argument",
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
                "static" | "deprecated" | "test" | "debug" | "native"
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
                "test" => {
                    if decorator.arg.is_none() {
                        self.emit_diag(
                            "E1511",
                            "`@test` expects a description string (e.g. `@test \"adds two numbers\"`)",
                            decorator.span.clone(),
                        );
                    } else if !matches!(decorator.arg, Some(Expr::Literal(Literal::String { .. })))
                    {
                        let span = decorator
                            .arg
                            .as_ref()
                            .map(expr_span)
                            .unwrap_or_else(|| decorator.span.clone());
                        self.emit_diag("E1510", "`@test` expects a string literal argument", span);
                    }
                }
                "native" => {
                    if decorator.arg.is_none() {
                        self.emit_diag(
                            "E1511",
                            "`@native` expects a target string (e.g. `@native \"gtk4.appRun\"`)",
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
                            "`@native` expects a string literal argument",
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

#[cfg(test)]
mod use_separator_tests {
    use crate::surface::parse_modules;
    use std::path::Path;

    fn parse_use(src: &str) -> Vec<crate::surface::UseDecl> {
        let (modules, diags) = parse_modules(Path::new("test.aivi"), src);
        assert!(diags.iter().all(|d| d.diagnostic.severity != crate::DiagnosticSeverity::Error), "parse errors: {diags:?}");
        modules.into_iter().flat_map(|m| m.uses).collect()
    }

    #[test]
    fn newline_separator_no_commas() {
        let uses = parse_use("module test\nuse aivi.text (\n  length\n  toUpper\n)\n");
        let text_use = uses.iter().find(|u| u.module.name == "aivi.text").expect("aivi.text use not found");
        assert_eq!(text_use.items.len(), 2);
    }

    #[test]
    fn mixed_comma_and_newline_separators() {
        let uses = parse_use("module test\nuse aivi.text (\n  length,\n  toUpper\n)\n");
        let text_use = uses.iter().find(|u| u.module.name == "aivi.text").expect("aivi.text use not found");
        assert_eq!(text_use.items.len(), 2);
    }

    #[test]
    fn grouped_import_desugars_to_flat() {
        let uses = parse_use(
            "module test\nuse aivi.text (\n  utils (toUpper, toLower)\n  format (padLeft as pad)\n)\n",
        );
        // +1 for the injected prelude import
        let non_prelude: Vec<_> = uses.iter().filter(|u| u.module.name != "aivi.prelude").collect();
        assert_eq!(non_prelude.len(), 2);
        let u1 = non_prelude.iter().find(|u| u.module.name == "aivi.text.utils").expect("aivi.text.utils not found");
        assert_eq!(u1.items.len(), 2);
        assert_eq!(u1.items[0].name.name, "toUpper");
        assert_eq!(u1.items[1].name.name, "toLower");
        let u2 = non_prelude.iter().find(|u| u.module.name == "aivi.text.format").expect("aivi.text.format not found");
        assert_eq!(u2.items.len(), 1);
        assert_eq!(u2.items[0].name.name, "padLeft");
        assert_eq!(u2.items[0].alias.as_ref().expect("alias should be present").name, "pad");
    }

    #[test]
    fn grouped_import_single_submodule() {
        let uses = parse_use(
            "module test\nuse aivi.core (\n  math (add)\n)\n",
        );
        let non_prelude: Vec<_> = uses.iter().filter(|u| u.module.name != "aivi.prelude").collect();
        assert_eq!(non_prelude.len(), 1);
        assert_eq!(non_prelude[0].module.name, "aivi.core.math");
        assert_eq!(non_prelude[0].items.len(), 1);
    }

    #[test]
    fn selective_import_still_works() {
        let uses = parse_use("module test\nuse aivi.text (toUpper, toLower)\n");
        let text_use = uses.iter().find(|u| u.module.name == "aivi.text").expect("aivi.text use not found");
        assert_eq!(text_use.items.len(), 2);
    }
}
