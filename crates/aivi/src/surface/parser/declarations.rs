const AUTO_FORWARD_DECORATOR: &str = "__auto_forward";

impl Parser {
    fn parse_type_or_def(&mut self, decorators: Vec<Decorator>) -> Option<ModuleItem> {
        let checkpoint = self.pos;
        if self.consume_name().is_some() {
            self.consume_newlines();
            if self.check_symbol(":") {
                self.pos = checkpoint;
                return self.parse_type_sig(decorators).map(ModuleItem::TypeSig);
            }
            if self.check_symbol("=") || self.is_pattern_start() {
                self.pos = checkpoint;
                return self.parse_def_or_type(decorators);
            }
            // Opaque type declarations can be written as a standalone `UpperIdent` on its own
            // line. Treat those as type declarations, not expression statements.
            let terminator = self.tokens.get(self.pos).is_none_or(|tok| {
                tok.kind == TokenKind::Newline || (tok.kind == TokenKind::Symbol && tok.text == "}")
            });
            if terminator {
                self.pos = checkpoint;
                let name = self.consume_name()?;
                if name
                    .name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                {
                    self.pos = checkpoint;
                    return self.parse_type_decl_or_alias(decorators);
                }
            }
            self.pos = checkpoint;
        }
        None
    }

    fn parse_type_sig(&mut self, decorators: Vec<Decorator>) -> Option<TypeSig> {
        self.reject_debug_decorators(&decorators, "type signatures");
        let name = self.consume_name()?;
        let start = name.span.clone();
        self.consume_newlines();
        self.expect_symbol(":", "expected ':' for type signature");
        let ty = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: start.clone(),
        });
        let span = merge_span(start, type_span(&ty));

        // `name : Type` is a standalone item; `name : Type = expr` is not valid syntax.
        // If there are more tokens on the same line, emit a targeted diagnostic and
        // skip the rest of the line to avoid cascading errors.
        if let Some(next) = self.tokens.get(self.pos) {
            let same_line = next.span.start.line == span.end.line;
            let allowed_terminator = next.kind == TokenKind::Newline
                || (next.kind == TokenKind::Symbol && next.text == "}");
            if same_line && !allowed_terminator {
                let next_span = next.span.clone();
                let line = next.span.start.line;
                self.emit_diag(
                    "E1528",
                    "type signatures must be written on their own line (write `name = ...` on the next line)",
                    merge_span(span.clone(), next_span.clone()),
                );
                while self.pos < self.tokens.len() {
                    let tok = &self.tokens[self.pos];
                    if tok.kind == TokenKind::Newline
                        || (tok.kind == TokenKind::Symbol && tok.text == "}")
                        || tok.span.start.line != line
                    {
                        break;
                    }
                    self.pos += 1;
                }
            }
        }
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
            self.pos = checkpoint;
            return self.parse_type_decl_or_alias(decorators);
        }
        self.pos = checkpoint;
        self.parse_def(decorators).map(ModuleItem::Def)
    }

    fn parse_type_decl_or_alias(&mut self, decorators: Vec<Decorator>) -> Option<ModuleItem> {
        let checkpoint = self.pos;
        let diag_checkpoint = self.diagnostics.len();
        if let Some(decl) = self.parse_type_decl(decorators.clone()) {
            if !decl.constructors.is_empty() {
                return Some(ModuleItem::TypeDecl(decl));
            }
        }
        self.pos = checkpoint;
        if let Some(alias) = self.parse_type_alias(decorators.clone()) {
            if self.check_symbol("=>") {
                self.pos = checkpoint;
                self.diagnostics.truncate(diag_checkpoint);
                return self.parse_def(Vec::new()).map(ModuleItem::Def);
            }
            if alias
                .decorators
                .iter()
                .any(|decorator| decorator.name.name == AUTO_FORWARD_DECORATOR)
            {
                let ctor_span = merge_span(alias.name.span.clone(), type_span(&alias.aliased));
                let constructor = TypeCtor {
                    name: alias.name.clone(),
                    args: vec![alias.aliased.clone()],
                    span: ctor_span,
                };
                return Some(ModuleItem::TypeDecl(TypeDecl {
                    decorators: alias.decorators,
                    name: alias.name,
                    params: alias.params,
                    constructors: vec![constructor],
                    span: alias.span,
                }));
            }
            return Some(ModuleItem::TypeAlias(alias));
        }
        self.pos = checkpoint;
        if let Some(opaque) = self.parse_opaque_type_decl(decorators) {
            return Some(ModuleItem::TypeDecl(opaque));
        }
        self.diagnostics.truncate(diag_checkpoint);
        None
    }

    fn parse_opaque_type_decl(&mut self, decorators: Vec<Decorator>) -> Option<TypeDecl> {
        self.reject_debug_decorators(&decorators, "type declarations");
        let name = self.consume_ident()?;
        if !name
            .name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
        {
            return None;
        }
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        // `UpperIdent [Params]` on its own line declares an opaque type.
        let end = params
            .last()
            .map(|p| p.span.clone())
            .unwrap_or_else(|| name.span.clone());
        let span = merge_span(name.span.clone(), end);
        Some(TypeDecl {
            decorators,
            name,
            params,
            constructors: Vec::new(),
            span,
        })
    }

    fn parse_type_decl(&mut self, decorators: Vec<Decorator>) -> Option<TypeDecl> {
        self.reject_debug_decorators(&decorators, "type declarations");
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        if !self.check_symbol("=") {
            return None;
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
                // If the next non-newline token looks like it starts a new *top-level* item,
                // assume the type declaration ends here (and thus has no constructor bars).
                //
                // Constructor lists are typically indented, and the first constructor may omit a
                // leading `|` (e.g. `Attr msg =\n  Class Text\n  | Id Text`), so we only stop the
                // scan when the next token is at column 1 (or EOF) and isn't a `|`.
                let mut lookahead = scan + 1;
                while lookahead < self.tokens.len()
                    && self.tokens[lookahead].kind == TokenKind::Newline
                {
                    lookahead += 1;
                }
                if lookahead >= self.tokens.len() {
                    break;
                }
                let next = &self.tokens[lookahead];
                let next_is_bar = next.kind == TokenKind::Symbol && next.text == "|";
                if !next_is_bar && next.span.start.column == 1 {
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
        // Constructors may be written inline:
        //   Msg = Inc | Dec
        // or in a multi-line style:
        //   Msg =
        //     | Inc
        //     | Dec
        //
        // Accept an optional leading `|` and allow newlines between ctors.
        self.consume_newlines();
        let _ = self.consume_symbol("|");
        self.consume_newlines();
        while let Some(ctor_name) = self.consume_ident() {
            let mut args = Vec::new();
            while !self.check_symbol("|") && !self.check_symbol("}") && self.pos < self.tokens.len()
            {
                // Constructor arguments are a sequence of *type atoms* so that
                // multi-argument constructors like `Element Text (List A) (List B)`
                // don't get parsed as a single type application `Text (List A) (List B)`.
                if let Some(ty) = self.parse_type_atom() {
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
            self.consume_newlines();
            if !self.consume_symbol("|") {
                break;
            }
            self.consume_newlines();
        }
        let span = merge_span(
            name.span.clone(),
            ctors
                .last()
                .map(|ctor| ctor.span.clone())
                .unwrap_or(name.span.clone()),
        );
        Some(TypeDecl {
            decorators,
            name,
            params,
            constructors: ctors,
            span,
        })
    }

    fn parse_type_alias(&mut self, mut decorators: Vec<Decorator>) -> Option<TypeAlias> {
        self.reject_debug_decorators(&decorators, "type aliases");
        let name = self.consume_ident()?;
        let mut params = Vec::new();
        while let Some(param) = self.consume_ident() {
            params.push(param);
        }
        if !self.check_symbol("=") {
            return None;
        }
        self.expect_symbol("=", "expected '=' in type alias");
        let aliased = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
            span: name.span.clone(),
        });
        let mut end_span = type_span(&aliased);
        if self.consume_symbol("!") {
            let bang_span = self.previous_span();
            decorators.push(Decorator {
                name: SpannedName {
                    name: AUTO_FORWARD_DECORATOR.to_string(),
                    span: bang_span.clone(),
                },
                arg: None,
                span: bang_span.clone(),
            });
            end_span = bang_span;
        }
        let span = merge_span(name.span.clone(), end_span);
        Some(TypeAlias {
            decorators,
            name,
            params,
            aliased,
            span,
        })
    }

    fn parse_class_decl(&mut self, decorators: Vec<Decorator>) -> Option<ClassDecl> {
        self.reject_debug_decorators(&decorators, "class declarations");
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

        // Syntax:
        //   class Name (Var *) = [Super1, Super2, ...] [given (A: Eq, ...)] { ... }
        //
        // Examples:
        //   class Functor (F *) = { map: (A -> B) -> F A -> F B }
        //   class Monad (M *) = Applicative, Chain { bind: M A -> (A -> M B) -> M B }
        //   class Collection (C *) = given (A: Eq) { elem: A -> C A -> Bool }

        fn peek_is_given_constraints(parser: &Parser) -> bool {
            if !parser.peek_keyword("given") {
                return false;
            }
            parser
                .tokens
                .get(parser.pos + 1)
                .is_some_and(|tok| tok.kind == TokenKind::Symbol && tok.text == "(")
        }

        // Parse optional comma-separated superclass list.
        // Superclasses are type names/applications; we must not consume `{` (the member body).
        let mut raw_supers = Vec::new();
        while !self.check_symbol("{")
            && !peek_is_given_constraints(self)
            && self.pos < self.tokens.len()
        {
            // Parse a single superclass: a name optionally followed by parenthesized args.
            let Some(name) = self.consume_ident() else {
                break;
            };
            raw_supers.push(TypeExpr::Name(name));
            self.consume_newlines();
            if !self.consume_symbol(",") {
                break;
            }
            self.consume_newlines();
        }

        // Parse optional `given (...)` constraint clause.
        let mut constraints = Vec::new();
        self.consume_newlines();
        if peek_is_given_constraints(self) {
            let given_span = self.consume_ident_text("given").expect("infallible").span;
            self.expect_symbol("(", "expected '(' after 'given' in class constraints");
            self.consume_newlines();
            while self.pos < self.tokens.len() && !self.check_symbol(")") {
                self.consume_newlines();
                let var = match self.consume_ident() {
                    Some(var) => var,
                    None => break,
                };
                self.consume_newlines();
                self.expect_symbol(":", "expected ':' in class type-variable constraint");
                self.consume_newlines();
                let class = self.consume_ident().unwrap_or(SpannedName {
                    name: String::new(),
                    span: var.span.clone(),
                });
                let span = merge_span(var.span.clone(), class.span.clone());
                constraints.push(crate::surface::TypeVarConstraint { var, class, span });
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
            }
            let end = self.expect_symbol(")", "expected ')' to close class constraints");
            if let Some(end) = end {
                let _ = merge_span(given_span, end);
            }
        }

        // Parse optional member record (`{ ... }`).
        let mut members = Vec::new();
        self.consume_newlines();
        if self.check_symbol("{") {
            if let Some(TypeExpr::Record { fields, .. }) = self.parse_type_atom() {
                for (field_name, field_ty) in fields {
                    let span = merge_span(field_name.span.clone(), type_span(&field_ty));
                    members.push(ClassMember {
                        name: field_name,
                        ty: field_ty,
                        span,
                    });
                }
            } else {
                self.expect_symbol("{", "expected '{' to start class member set");
            }
        }

        let supers = if params.is_empty() {
            raw_supers
        } else {
            raw_supers
                .into_iter()
                .map(|super_ty| match super_ty {
                    TypeExpr::Name(name) => {
                        let base = TypeExpr::Name(name.clone());
                        let args = params.clone();
                        let span =
                            merge_span(name.span, type_span(args.last().expect("non-empty")));
                        TypeExpr::Apply {
                            base: Box::new(base),
                            args,
                            span,
                        }
                    }
                    other => other,
                })
                .collect()
        };

        let span = merge_span(start, self.previous_span());
        Some(ClassDecl {
            decorators,
            name,
            params,
            constraints,
            supers,
            members,
            span,
        })
    }

    fn parse_instance_decl(&mut self, decorators: Vec<Decorator>) -> Option<InstanceDecl> {
        self.reject_debug_decorators(&decorators, "instance declarations");
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
            decorators,
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

    fn parse_domain_decl(&mut self, decorators: Vec<Decorator>) -> Option<DomainDecl> {
        self.reject_debug_decorators(&decorators, "domain declarations");
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
                let _ = self.match_keyword("type");
                let span = self.previous_span();
                self.emit_diag(
                    "E1542",
                    "`type` keyword is not part of AIVI syntax; write `Name = ...` inside domains",
                    span,
                );
                if let Some(type_decl) = self.parse_domain_type_decl(decorators.clone()) {
                    items.push(DomainItem::TypeAlias(type_decl));
                    continue;
                }
            } else if self.tokens.get(self.pos).is_some_and(|tok| {
                tok.kind == TokenKind::Ident
                    && tok
                        .text
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_uppercase())
            }) {
                // Domain-local type declarations (e.g. `Delta = ...`) start with `UpperIdent`.
                // Only treat it as a type declaration when an `=` appears on the same line.
                let mut scan = self.pos;
                let mut saw_eq = false;
                while scan < self.tokens.len() {
                    let tok = &self.tokens[scan];
                    if tok.kind == TokenKind::Newline
                        || (tok.kind == TokenKind::Symbol && tok.text == "}")
                    {
                        break;
                    }
                    if tok.kind == TokenKind::Symbol && tok.text == "=" {
                        saw_eq = true;
                        break;
                    }
                    scan += 1;
                }
                if saw_eq {
                    let checkpoint = self.pos;
                    if let Some(type_decl) = self.parse_domain_type_decl(decorators.clone()) {
                        items.push(DomainItem::TypeAlias(type_decl));
                        continue;
                    }
                    self.pos = checkpoint;
                }
            }
            let checkpoint = self.pos;
            if let Some(sig) = self.parse_literal_type_sig(decorators.clone()) {
                items.push(DomainItem::TypeSig(sig));
                continue;
            }
            self.pos = checkpoint;
            let checkpoint_after = self.pos;
            if self.consume_name().is_some() {
                if self.check_symbol(":") {
                    self.pos = checkpoint_after;
                    if let Some(sig) = self.parse_type_sig(decorators) {
                        items.push(DomainItem::TypeSig(sig));
                    }
                    continue;
                }
                self.pos = checkpoint_after;
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
            decorators,
            name,
            over,
            items,
            span,
        })
    }

    /// Parse a `machine Name = { [Source] -> Target : event { fields } }` declaration.
    ///
    /// Grammar:
    ///   MachineDecl       := "machine" UpperIdent "=" "{" { MachineTransition } "}"
    ///   MachineTransition := [ UpperIdent ] "->" UpperIdent ":" lowerIdent "{" { FieldDecl } "}"
    pub(crate) fn parse_machine_decl(&mut self, decorators: Vec<Decorator>) -> Option<MachineDecl> {
        self.reject_debug_decorators(&decorators, "machine declarations");
        let start = self.previous_span();
        let name = self.consume_ident()?;
        self.consume_newlines();
        self.expect_symbol("=", "expected '=' in machine declaration");
        self.consume_newlines();
        self.expect_symbol("{", "expected '{' to start machine body");

        let mut states = Vec::new();
        let mut transitions = Vec::new();
        let mut seen_states = std::collections::HashSet::new();

        loop {
            self.consume_newlines();
            if self.check_symbol("}") || self.pos >= self.tokens.len() {
                break;
            }

            let trans_start = self.peek_span().unwrap_or_else(|| self.previous_span());

            // Parse: [SourceState] "->" TargetState ":" eventName "{" { fields } "}"
            // If no source state before "->", this is the initial transition.
            let source: Option<SpannedName> = if self.check_symbol("->") {
                // Initial transition (no source)
                None
            } else if let Some(src) = self.consume_ident() {
                Some(src)
            } else {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag("E1550", "expected state name or '->' in machine body", span);
                self.pos += 1;
                continue;
            };

            self.consume_newlines();
            if !self.consume_symbol("->") {
                let span = self.peek_span().unwrap_or_else(|| self.previous_span());
                self.emit_diag("E1551", "expected '->' in machine transition", span);
                self.recover_to_item();
                continue;
            }

            self.consume_newlines();
            let Some(target) = self.consume_ident() else {
                self.emit_diag(
                    "E1552",
                    "expected target state name after '->'",
                    self.previous_span(),
                );
                self.recover_to_item();
                continue;
            };

            self.consume_newlines();
            self.expect_symbol(":", "expected ':' after target state in machine transition");
            self.consume_newlines();

            let Some(event_name) = self.consume_ident() else {
                self.emit_diag(
                    "E1553",
                    "expected transition name after ':'",
                    self.previous_span(),
                );
                self.recover_to_item();
                continue;
            };

            self.consume_newlines();

            // Parse optional payload: "{" { fieldName ":" TypeExpr } "}"
            let mut payload = Vec::new();
            if self.consume_symbol("{") {
                loop {
                    self.consume_newlines();
                    if self.check_symbol("}") || self.pos >= self.tokens.len() {
                        break;
                    }
                    if let Some(field_name) = self.consume_ident() {
                        self.consume_newlines();
                        self.expect_symbol(":", "expected ':' after field name in payload");
                        self.consume_newlines();
                        if let Some(ty) = self.parse_type_expr() {
                            payload.push((field_name, ty));
                        }
                    } else {
                        break;
                    }
                }
                self.expect_symbol("}", "expected '}' to close transition payload");
            }

            let trans_end = self.previous_span();

            // Collect inferred states
            if let Some(ref src) = source {
                if seen_states.insert(src.name.clone()) {
                    states.push(MachineState {
                        name: src.clone(),
                        fields: Vec::new(),
                        span: src.span.clone(),
                    });
                }
            }
            if seen_states.insert(target.name.clone()) {
                states.push(MachineState {
                    name: target.clone(),
                    fields: Vec::new(),
                    span: target.span.clone(),
                });
            }

            // For initial transition, use an empty source name to indicate no source
            let source_name = source.unwrap_or_else(|| SpannedName {
                name: String::new(),
                span: trans_start.clone(),
            });

            transitions.push(MachineTransition {
                source: source_name,
                target,
                name: event_name,
                payload,
                span: merge_span(trans_start, trans_end),
            });
        }
        let end = self.expect_symbol("}", "expected '}' to close machine body");
        let span = merge_span(start, end.unwrap_or(name.span.clone()));
        Some(MachineDecl {
            decorators,
            name,
            states,
            transitions,
            span,
        })
    }
}
