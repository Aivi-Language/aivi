impl Parser {
    fn parse_gtk_sigil(&mut self, sigil: &Token, body: &str) -> Expr {
        #[derive(Debug, Clone)]
        enum GtkAttrValue {
            Bare,
            Text(String),
            Splice(Expr),
        }

        #[derive(Debug, Clone)]
        struct GtkAttr {
            name: String,
            value: GtkAttrValue,
            span: Span,
        }

        #[derive(Debug, Clone)]
        enum GtkNode {
            Element {
                tag: String,
                attrs: Vec<GtkAttr>,
                children: Vec<GtkNode>,
                span: Span,
            },
            FunctionCall {
                tag: String,
                args: Vec<Expr>,
                span: Span,
            },
            Text {
                text: String,
                span: Span,
            },
            Splice {
                expr: Expr,
                span: Span,
            },
        }

        fn is_name_start(ch: char) -> bool {
            ch.is_ascii_alphabetic()
        }

        fn is_name_continue(ch: char) -> bool {
            ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.')
        }

        fn pos_at_char_offset(start: &Position, text: &str, offset: usize) -> (usize, usize) {
            let mut line = start.line;
            let mut col = start.column;
            for ch in text.chars().take(offset) {
                if ch == '\n' {
                    line += 1;
                    col = 1;
                } else {
                    col += 1;
                }
            }
            (line, col)
        }

        fn span_at_char_offsets(
            start: &Position,
            text: &str,
            start_offset: usize,
            end_offset_exclusive: usize,
        ) -> Span {
            let (start_line, start_column) = pos_at_char_offset(start, text, start_offset);
            let end_offset = if end_offset_exclusive > start_offset {
                end_offset_exclusive - 1
            } else {
                start_offset
            };
            let (end_line, end_column) = pos_at_char_offset(start, text, end_offset);
            Span {
                start: Position {
                    line: start_line,
                    column: start_column,
                },
                end: Position {
                    line: end_line,
                    column: end_column,
                },
            }
        }

        fn gtk_node_span(node: &GtkNode) -> &Span {
            match node {
                GtkNode::Element { span, .. }
                | GtkNode::FunctionCall { span, .. }
                | GtkNode::Text { span, .. }
                | GtkNode::Splice { span, .. } => span,
            }
        }

        fn normalize_prop_name(name: &str) -> String {
            let mut out = String::new();
            for ch in name.chars() {
                if ch.is_ascii_uppercase() {
                    if !out.is_empty() {
                        out.push('-');
                    }
                    out.push(ch.to_ascii_lowercase());
                } else if ch == '_' {
                    out.push('-');
                } else {
                    out.push(ch);
                }
            }
            out
        }

        fn compile_time_expr_text(expr: &Expr) -> Option<String> {
            match expr {
                Expr::Ident(name) => Some(name.name.clone()),
                Expr::Literal(Literal::Number { text, .. }) => Some(text.clone()),
                Expr::Literal(Literal::String { text, .. }) => Some(text.clone()),
                Expr::Literal(Literal::Bool { value, .. }) => Some(value.to_string()),
                Expr::Literal(Literal::DateTime { text, .. }) => Some(text.clone()),
                Expr::UnaryNeg { expr, .. } => {
                    let inner = compile_time_expr_text(expr)?;
                    if inner.starts_with('-') {
                        Some(inner)
                    } else {
                        Some(format!("-{inner}"))
                    }
                }
                Expr::Suffixed { base, suffix, .. } => {
                    Some(format!("{}{}", compile_time_expr_text(base)?, suffix.name))
                }
                Expr::FieldAccess { base, field, .. } => {
                    Some(format!("{}.{}", compile_time_expr_text(base)?, field.name))
                }
                Expr::Call { func, args, .. } => {
                    let func_text = compile_time_expr_text(func)?;
                    let arg_texts = args
                        .iter()
                        .map(compile_time_expr_text)
                        .collect::<Option<Vec<_>>>()?;
                    Some(format!("{func_text}({})", arg_texts.join(", ")))
                }
                _ => None,
            }
        }

        fn static_value_expr_text(expr: &Expr) -> Option<String> {
            match expr {
                Expr::Literal(_) | Expr::UnaryNeg { .. } | Expr::Suffixed { .. } => {
                    compile_time_expr_text(expr)
                }
                _ => None,
            }
        }

        fn gtk_element_class_name(tag: &str, attrs: &[GtkAttr]) -> Option<String> {
            if tag.starts_with("Gtk") || tag.starts_with("Adw") || tag.starts_with("Gsk") {
                return Some(tag.to_string());
            }
            attrs.iter().find_map(|attr| {
                if attr.name != "class" {
                    return None;
                }
                match &attr.value {
                    GtkAttrValue::Text(value) => Some(value.clone()),
                    GtkAttrValue::Splice(expr) => compile_time_expr_text(expr),
                    GtkAttrValue::Bare => None,
                }
            })
        }

        fn gtk_signal_sugar_name(class_name: Option<&str>, attr_name: &str) -> Option<&'static str> {
            match attr_name {
                "onClick" => Some("clicked"),
                "onInput" => Some("changed"),
                "onActivate" => Some("activate"),
                "onToggle" => match class_name {
                    Some("GtkSwitch") => Some("notify::active"),
                    _ => Some("toggled"),
                },
                "onValueChanged" => Some("value-changed"),
                "onKeyPress" => Some("key-pressed"),
                "onFocusIn" => Some("focus-enter"),
                "onFocusOut" => Some("focus-leave"),
                "onSelect" => match class_name {
                    Some("GtkDropDown") => Some("notify::selected"),
                    _ => None,
                },
                "onClosed" => match class_name {
                    Some(name) if name.starts_with("Adw") && name.ends_with("Dialog") => {
                        Some("closed")
                    }
                    _ => None,
                },
                "onShowSidebarChanged" => Some("notify::show-sidebar"),
                _ => None,
            }
        }

        // Compute the body offset inside the full sigil token (`~<gtk> ... </gtk>`).
        let body_start_offset = sigil
            .text
            .chars()
            .position(|ch| ch == '>')
            .map(|i| i + 1)
            .unwrap_or(0);

        let body_chars: Vec<char> = body.chars().collect();
        let mut i = 0usize;

        let mut nodes: Vec<GtkNode> = Vec::new();
        let mut stack: Vec<(String, Vec<GtkAttr>, Vec<GtkNode>, Span)> = Vec::new();

        let emit_gtk_diag = |this: &mut Parser, message: &str| {
            this.emit_diag("E1610", message, sigil.span.clone());
        };

        let push_node =
            |node: GtkNode,
             nodes: &mut Vec<GtkNode>,
             stack: &mut Vec<(String, Vec<GtkAttr>, Vec<GtkNode>, Span)>| {
                if let Some((_tag, _attrs, children, _span)) = stack.last_mut() {
                    children.push(node);
                } else {
                    nodes.push(node);
                }
            };

        while i < body_chars.len() {
            let ch = body_chars[i];

            if ch == '{' {
                let remainder: String = body_chars[i + 1..].iter().collect();
                let Some(close_offset) = find_interpolation_close(&remainder) else {
                    emit_gtk_diag(self, "unterminated gtk splice (missing '}')");
                    i += 1;
                    continue;
                };
                let close_index = i + 1 + close_offset;
                let expr_raw: String = body_chars[i + 1..close_index].iter().collect();
                let (expr_decoded, expr_raw_map) = decode_interpolation_source_with_map(&expr_raw);

                let expr_start_offset = body_start_offset + (i + 1);
                let (expr_line, expr_col) =
                    pos_at_char_offset(&sigil.span.start, &sigil.text, expr_start_offset);
                let expr =
                    self.parse_embedded_expr(&expr_decoded, &expr_raw_map, expr_line, expr_col);
                if let Some(expr) = expr {
                    push_node(
                        GtkNode::Splice {
                            expr,
                            span: span_at_char_offsets(
                                &sigil.span.start,
                                &sigil.text,
                                body_start_offset + i,
                                body_start_offset + close_index + 1,
                            ),
                        },
                        &mut nodes,
                        &mut stack,
                    );
                } else {
                    emit_gtk_diag(self, "invalid gtk splice expression");
                }

                i = close_index + 1;
                continue;
            }

            if ch == '<' {
                // Closing tag.
                if i + 1 < body_chars.len() && body_chars[i + 1] == '/' {
                    i += 2;
                    while i < body_chars.len() && body_chars[i].is_whitespace() {
                        i += 1;
                    }
                    let start = i;
                    if i < body_chars.len() && is_name_start(body_chars[i]) {
                        i += 1;
                        while i < body_chars.len() && is_name_continue(body_chars[i]) {
                            i += 1;
                        }
                    }
                    let name: String = body_chars[start..i].iter().collect();
                    while i < body_chars.len() && body_chars[i].is_whitespace() {
                        i += 1;
                    }
                    if i < body_chars.len() && body_chars[i] == '>' {
                        i += 1;
                    } else {
                        emit_gtk_diag(self, "expected '>' to close gtk end tag");
                    }

                    if let Some((open_tag, open_attrs, open_children, open_span)) = stack.pop() {
                        if open_tag != name {
                            emit_gtk_diag(
                                self,
                                &format!("mismatched gtk end tag: expected </{open_tag}>"),
                            );
                        }
                        push_node(
                            GtkNode::Element {
                                tag: open_tag,
                                attrs: open_attrs,
                                children: open_children,
                                span: open_span,
                            },
                            &mut nodes,
                            &mut stack,
                        );
                    } else {
                        emit_gtk_diag(self, "unexpected gtk end tag");
                    }
                    continue;
                }

                // Start tag / self-close.
                let tag_start = i;
                i += 1;
                while i < body_chars.len() && body_chars[i].is_whitespace() {
                    i += 1;
                }
                let start = i;
                if i < body_chars.len() && is_name_start(body_chars[i]) {
                    i += 1;
                    while i < body_chars.len() && is_name_continue(body_chars[i]) {
                        i += 1;
                    }
                } else {
                    emit_gtk_diag(self, "expected tag name after '<'");
                }
                let tag: String = body_chars[start..i].iter().collect();
                let mut attrs: Vec<GtkAttr> = Vec::new();
                let mut positional_args: Vec<Expr> = Vec::new();

                loop {
                    while i < body_chars.len() && body_chars[i].is_whitespace() {
                        i += 1;
                    }
                    if i >= body_chars.len() {
                        emit_gtk_diag(self, "unterminated gtk tag");
                        break;
                    }
                    if body_chars[i] == '>' {
                        i += 1;
                        let tag_span = span_at_char_offsets(
                            &sigil.span.start,
                            &sigil.text,
                            body_start_offset + tag_start,
                            body_start_offset + i,
                        );
                        if positional_args.is_empty() {
                            stack.push((tag.clone(), attrs, Vec::new(), tag_span));
                        } else {
                            self.emit_diag(
                                "E1617",
                                "GTK function call sugar must use a self-closing tag: <FunctionName arg0 ... />",
                                sigil.span.clone(),
                            );
                            push_node(
                                GtkNode::FunctionCall {
                                    tag: tag.clone(),
                                    args: positional_args,
                                    span: tag_span,
                                },
                                &mut nodes,
                                &mut stack,
                            );
                        }
                        break;
                    }
                    if body_chars[i] == '/' && i + 1 < body_chars.len() && body_chars[i + 1] == '>'
                    {
                        i += 2;
                        let tag_span = span_at_char_offsets(
                            &sigil.span.start,
                            &sigil.text,
                            body_start_offset + tag_start,
                            body_start_offset + i,
                        );
                        if positional_args.is_empty()
                            && attrs.is_empty()
                            && function_call_tag_expr(&tag, &sigil.span).is_some()
                        {
                            push_node(
                                GtkNode::FunctionCall {
                                    tag: tag.clone(),
                                    args: Vec::new(),
                                    span: tag_span,
                                },
                                &mut nodes,
                                &mut stack,
                            );
                        } else if positional_args.is_empty() {
                            push_node(
                                GtkNode::Element {
                                    tag: tag.clone(),
                                    attrs,
                                    children: Vec::new(),
                                    span: tag_span,
                                },
                                &mut nodes,
                                &mut stack,
                            );
                        } else {
                            push_node(
                                GtkNode::FunctionCall {
                                    tag: tag.clone(),
                                    args: positional_args,
                                    span: tag_span,
                                },
                                &mut nodes,
                                &mut stack,
                            );
                        }
                        break;
                    }

                    let attr_like = gtk_attr_name_end(&body_chars, i)
                        .map(|name_end| {
                            let mut look = name_end;
                            while look < body_chars.len() && body_chars[look].is_whitespace() {
                                look += 1;
                            }
                            look < body_chars.len() && body_chars[look] == '='
                        })
                        .unwrap_or(false);

                    if function_call_tag_expr(&tag, &sigil.span).is_some() && !attr_like {
                        if !attrs.is_empty() {
                            self.emit_diag(
                                "E1617",
                                "GTK function call sugar cannot mix positional arguments with attributes",
                                sigil.span.clone(),
                            );
                            if let Some((_, next_i)) = parse_function_call_arg(
                                self,
                                sigil,
                                body_start_offset,
                                &body_chars,
                                i,
                            ) {
                                i = next_i;
                            } else {
                                i += 1;
                            }
                            continue;
                        }

                        if let Some((expr, next_i)) =
                            parse_function_call_arg(self, sigil, body_start_offset, &body_chars, i)
                        {
                            positional_args.push(expr);
                            i = next_i;
                            continue;
                        }

                        self.emit_diag(
                            "E1617",
                            "invalid GTK function-call argument",
                            sigil.span.clone(),
                        );
                        i += 1;
                        continue;
                    }

                    // Attribute name.
                    let astart = i;
                    if i < body_chars.len() && is_name_start(body_chars[i]) {
                        i += 1;
                        while i < body_chars.len() && is_name_continue(body_chars[i]) {
                            i += 1;
                        }
                    } else {
                        emit_gtk_diag(self, "expected attribute name in gtk tag");
                        i += 1;
                        continue;
                    }
                    let name: String = body_chars[astart..i].iter().collect();
                    while i < body_chars.len() && body_chars[i].is_whitespace() {
                        i += 1;
                    }
                    let value = if i < body_chars.len() && body_chars[i] == '=' {
                        i += 1;
                        while i < body_chars.len() && body_chars[i].is_whitespace() {
                            i += 1;
                        }
                        if i >= body_chars.len() {
                            GtkAttrValue::Bare
                        } else if body_chars[i] == '"' || body_chars[i] == '\'' {
                            let quote = body_chars[i];
                            i += 1;
                            let vstart = i;
                            while i < body_chars.len() {
                                if body_chars[i] == '\\' && i + 1 < body_chars.len() {
                                    i += 2;
                                    continue;
                                }
                                if body_chars[i] == quote {
                                    break;
                                }
                                i += 1;
                            }
                            let text: String = body_chars[vstart..i].iter().collect();
                            if i < body_chars.len() && body_chars[i] == quote {
                                i += 1;
                            } else {
                                emit_gtk_diag(self, "unterminated quoted attribute value");
                            }
                            GtkAttrValue::Text(text)
                        } else if body_chars[i] == '{' {
                            let remainder: String = body_chars[i + 1..].iter().collect();
                            match find_interpolation_close(&remainder) {
                                Some(close_offset) => {
                                    let close_index = i + 1 + close_offset;
                                    let expr_raw: String =
                                        body_chars[i + 1..close_index].iter().collect();
                                    let (expr_decoded, expr_raw_map) =
                                        decode_interpolation_source_with_map(&expr_raw);

                                    let expr_start_offset = body_start_offset + (i + 1);
                                    let (expr_line, expr_col) = pos_at_char_offset(
                                        &sigil.span.start,
                                        &sigil.text,
                                        expr_start_offset,
                                    );
                                    let expr = self.parse_embedded_expr(
                                        &expr_decoded,
                                        &expr_raw_map,
                                        expr_line,
                                        expr_col,
                                    );
                                    i = close_index + 1;
                                    match expr {
                                        Some(expr) => GtkAttrValue::Splice(expr),
                                        None => GtkAttrValue::Bare,
                                    }
                                }
                                None => {
                                    emit_gtk_diag(
                                        self,
                                        "unterminated attribute splice (missing '}')",
                                    );
                                    i += 1;
                                    GtkAttrValue::Bare
                                }
                            }
                        } else {
                            // Unquoted attribute value.
                            let vstart = i;
                            while i < body_chars.len()
                                && !body_chars[i].is_whitespace()
                                && body_chars[i] != '>'
                            {
                                if body_chars[i] == '/'
                                    && i + 1 < body_chars.len()
                                    && body_chars[i + 1] == '>'
                                {
                                    break;
                                }
                                i += 1;
                            }
                            GtkAttrValue::Text(body_chars[vstart..i].iter().collect())
                        }
                    } else {
                        GtkAttrValue::Bare
                    };

                    attrs.push(GtkAttr {
                        name,
                        value,
                        span: span_at_char_offsets(
                            &sigil.span.start,
                            &sigil.text,
                            body_start_offset + astart,
                            body_start_offset + i,
                        ),
                    });
                }
                continue;
            }

            // Text node.
            let start = i;
            while i < body_chars.len() && body_chars[i] != '<' && body_chars[i] != '{' {
                i += 1;
            }
            let text: String = body_chars[start..i].iter().collect();
            if !text.trim().is_empty() {
                push_node(
                    GtkNode::Text {
                        text,
                        span: span_at_char_offsets(
                            &sigil.span.start,
                            &sigil.text,
                            body_start_offset + start,
                            body_start_offset + i,
                        ),
                    },
                    &mut nodes,
                    &mut stack,
                );
            }
        }

        // Close any unclosed tags.
        while let Some((open_tag, open_attrs, open_children, open_span)) = stack.pop() {
            emit_gtk_diag(self, &format!("unclosed gtk tag <{open_tag}>"));
            push_node(
                GtkNode::Element {
                    tag: open_tag,
                    attrs: open_attrs,
                    children: open_children,
                    span: open_span,
                },
                &mut nodes,
                &mut stack,
            );
        }

        // Lower parsed GTK XML nodes to `aivi.ui.gtk4` helper constructors.
        fn lower_raw_attr(attr: GtkAttr) -> Expr {
            let span = attr.span.clone();
            let mk_ui = |name: &str| {
                Expr::Ident(SpannedName {
                    name: name.into(),
                    span: span.clone(),
                })
            };
            let mk_string = |value: &str| {
                Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                })
            };
            let call2 = |fname: &str, a: Expr, b: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![a, b],
                span: span.clone(),
            };
            match attr.name.as_str() {
                "id" => match attr.value {
                    GtkAttrValue::Text(v) => Expr::Call {
                        func: Box::new(mk_ui("gtkIdAttr")),
                        args: vec![mk_string(&v)],
                        span: span.clone(),
                    },
                    GtkAttrValue::Splice(expr) => {
                        if let Some(text) = static_value_expr_text(&expr) {
                            Expr::Call {
                                func: Box::new(mk_ui("gtkIdAttr")),
                                args: vec![mk_string(&text)],
                                span: span.clone(),
                            }
                        } else {
                            call2("gtkBoundAttr", mk_string("id"), expr)
                        }
                    }
                    GtkAttrValue::Bare => Expr::Call {
                        func: Box::new(mk_ui("gtkIdAttr")),
                        args: vec![mk_string("true")],
                        span: span.clone(),
                    },
                },
                "ref" => match attr.value {
                    GtkAttrValue::Text(v) => Expr::Call {
                        func: Box::new(mk_ui("gtkRefAttr")),
                        args: vec![mk_string(&v)],
                        span: span.clone(),
                    },
                    GtkAttrValue::Splice(expr) => {
                        if let Some(text) = static_value_expr_text(&expr) {
                            Expr::Call {
                                func: Box::new(mk_ui("gtkRefAttr")),
                                args: vec![mk_string(&text)],
                                span: span.clone(),
                            }
                        } else {
                            call2("gtkBoundAttr", mk_string("ref"), expr)
                        }
                    }
                    GtkAttrValue::Bare => Expr::Call {
                        func: Box::new(mk_ui("gtkRefAttr")),
                        args: vec![mk_string("true")],
                        span: span.clone(),
                    },
                },
                _ => match attr.value {
                    GtkAttrValue::Text(v) => {
                        call2("gtkStaticAttr", mk_string(&attr.name), mk_string(&v))
                    }
                    GtkAttrValue::Splice(expr) => {
                        if let Some(text) = static_value_expr_text(&expr) {
                            call2("gtkStaticAttr", mk_string(&attr.name), mk_string(&text))
                        } else {
                            call2("gtkBoundAttr", mk_string(&attr.name), expr)
                        }
                    }
                    GtkAttrValue::Bare => {
                        call2("gtkStaticAttr", mk_string(&attr.name), mk_string("true"))
                    }
                },
            }
        }

        fn lower_prop_attr(name: &str, value: GtkAttrValue, span: &Span) -> Expr {
            let mk_ui = |fname: &str| {
                Expr::Ident(SpannedName {
                    name: fname.into(),
                    span: span.clone(),
                })
            };
            let mk_string = |value: &str| {
                Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                })
            };
            let call2 = |fname: &str, a: Expr, b: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![a, b],
                span: span.clone(),
            };
            match value {
                GtkAttrValue::Text(v) => call2("gtkStaticProp", mk_string(name), mk_string(&v)),
                GtkAttrValue::Splice(expr) => {
                    if let Some(text) = static_value_expr_text(&expr) {
                        call2("gtkStaticProp", mk_string(name), mk_string(&text))
                    } else {
                        call2("gtkBoundProp", mk_string(name), expr)
                    }
                }
                GtkAttrValue::Bare => call2("gtkStaticProp", mk_string(name), mk_string("true")),
            }
        }

        fn lower_event_attr(
            this: &mut Parser,
            signal_name: &str,
            source_name: &str,
            value: GtkAttrValue,
            span: &Span,
        ) -> Option<Expr> {
            let mk_ui = |fname: &str| {
                Expr::Ident(SpannedName {
                    name: fname.into(),
                    span: span.clone(),
                })
            };
            let mk_string = |value: &str| {
                Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                })
            };
            let handler_expr = match value {
                GtkAttrValue::Splice(expr) => expr,
                GtkAttrValue::Text(v) => mk_string(&v),
                GtkAttrValue::Bare => {
                    this.emit_diag(
                        "E1614",
                        &format!("`{source_name}` handler requires a value expression"),
                        span.clone(),
                    );
                    return None;
                }
            };
            Some(Expr::Call {
                func: Box::new(mk_ui("gtkEventSugarAttr")),
                args: vec![mk_string(signal_name), mk_string(source_name), handler_expr],
                span: span.clone(),
            })
        }

        fn lower_source_meta(module_path: &str, span: &Span) -> Vec<Expr> {
            let mk_ui = |name: &str| {
                Expr::Ident(SpannedName {
                    name: name.into(),
                    span: span.clone(),
                })
            };
            let mk_string = |value: &str| {
                Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                })
            };
            let call2 = |fname: &str, a: Expr, b: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![a, b],
                span: span.clone(),
            };
            vec![
                call2(
                    "gtkStaticAttr",
                    mk_string("aivi-source-path"),
                    mk_string(module_path),
                ),
                call2(
                    "gtkStaticAttr",
                    mk_string("aivi-source-start-line"),
                    mk_string(&span.start.line.to_string()),
                ),
                call2(
                    "gtkStaticAttr",
                    mk_string("aivi-source-start-column"),
                    mk_string(&span.start.column.to_string()),
                ),
                call2(
                    "gtkStaticAttr",
                    mk_string("aivi-source-end-line"),
                    mk_string(&span.end.line.to_string()),
                ),
                call2(
                    "gtkStaticAttr",
                    mk_string("aivi-source-end-column"),
                    mk_string(&span.end.column.to_string()),
                ),
            ]
        }

        fn gtk_attr_name_end(chars: &[char], start: usize) -> Option<usize> {
            if start >= chars.len() || !is_name_start(chars[start]) {
                return None;
            }
            let mut i = start + 1;
            while i < chars.len() && is_name_continue(chars[i]) {
                i += 1;
            }
            Some(i)
        }

        fn is_ident_segment(segment: &str) -> bool {
            let mut chars = segment.chars();
            let Some(head) = chars.next() else {
                return false;
            };
            (head.is_ascii_alphabetic() || head == '_')
                && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        }

        fn function_call_tag_expr(tag: &str, span: &Span) -> Option<Expr> {
            if tag.contains('.')
                || tag.starts_with("Gtk")
                || tag.starts_with("Adw")
                || tag.starts_with("Gsk")
                || !is_ident_segment(tag)
            {
                return None;
            }
            let mut chars = tag.chars();
            let head = chars.next()?;
            if !head.is_ascii_uppercase() {
                return None;
            }
            Some(Expr::Ident(SpannedName {
                name: format!("{}{}", head.to_ascii_lowercase(), chars.collect::<String>()),
                span: span.clone(),
            }))
        }

        fn component_tag_expr(tag: &str, span: &Span) -> Option<Expr> {
            let mut segments = tag.split('.');
            let first = segments.next()?;
            if first.is_empty()
                || !first
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
            {
                return None;
            }
            if !is_ident_segment(first) {
                return None;
            }
            let mut expr = Expr::Ident(SpannedName {
                name: first.to_string(),
                span: span.clone(),
            });
            for segment in segments {
                if !is_ident_segment(segment) {
                    return None;
                }
                expr = Expr::FieldAccess {
                    base: Box::new(expr),
                    field: SpannedName {
                        name: segment.to_string(),
                        span: span.clone(),
                    },
                    span: span.clone(),
                };
            }
            Some(expr)
        }

        fn parse_function_call_arg(
            this: &mut Parser,
            sigil: &Token,
            body_start_offset: usize,
            body_chars: &[char],
            start: usize,
        ) -> Option<(Expr, usize)> {
            let mut i = start;
            let mut brace_depth = 0isize;
            let mut paren_depth = 0isize;
            let mut bracket_depth = 0isize;
            let mut in_quote: Option<char> = None;

            while i < body_chars.len() {
                let ch = body_chars[i];
                if let Some(quote) = in_quote {
                    if quote != '`' && ch == '\\' && i + 1 < body_chars.len() {
                        i += 2;
                        continue;
                    }
                    if ch == quote {
                        in_quote = None;
                    }
                    i += 1;
                    continue;
                }

                match ch {
                    '"' | '\'' | '`' => in_quote = Some(ch),
                    '{' => brace_depth += 1,
                    '}' => {
                        if brace_depth == 0 {
                            break;
                        }
                        brace_depth -= 1;
                    }
                    '(' => paren_depth += 1,
                    ')' => {
                        if paren_depth == 0 {
                            break;
                        }
                        paren_depth -= 1;
                    }
                    '[' => bracket_depth += 1,
                    ']' => {
                        if bracket_depth == 0 {
                            break;
                        }
                        bracket_depth -= 1;
                    }
                    '>' if brace_depth == 0 && paren_depth == 0 && bracket_depth == 0 => {
                        break;
                    }
                    '/' if brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0
                        && i + 1 < body_chars.len()
                        && body_chars[i + 1] == '>' =>
                    {
                        break;
                    }
                    _ if ch.is_whitespace()
                        && brace_depth == 0
                        && paren_depth == 0
                        && bracket_depth == 0 =>
                    {
                        break;
                    }
                    _ => {}
                }

                i += 1;
            }

            if i == start {
                return None;
            }

            let expr_raw: String = body_chars[start..i].iter().collect();
            let (expr_decoded, expr_raw_map) = decode_interpolation_source_with_map(&expr_raw);
            let expr_start_offset = body_start_offset + start;
            let (expr_line, expr_col) =
                pos_at_char_offset(&sigil.span.start, &sigil.text, expr_start_offset);
            let expr =
                this.parse_embedded_expr(&expr_decoded, &expr_raw_map, expr_line, expr_col)?;
            Some((expr, i))
        }

        fn lower_children(
            this: &mut Parser,
            module_path: &str,
            children: Vec<GtkNode>,
            span: &Span,
        ) -> Expr {
            let mut lowered_items: Vec<ListItem> = Vec::new();
            for child in children {
                let child_span = gtk_node_span(&child).clone();
                let GtkNode::Element {
                    tag,
                    attrs,
                    children: each_children,
                    ..
                } = child.clone()
                else {
                    lowered_items.push(ListItem {
                        expr: lower_node(this, module_path, child),
                        spread: false,
                        span: child_span,
                    });
                    continue;
                };
                if tag != "each" && tag != "show" {
                    lowered_items.push(ListItem {
                        expr: lower_node(
                            this,
                            module_path,
                            GtkNode::Element {
                                tag,
                                attrs,
                                children: each_children,
                                span: child_span.clone(),
                            },
                        ),
                        spread: false,
                        span: child_span,
                    });
                    continue;
                }

                if tag == "show" {
                    let mut when_expr: Option<Expr> = None;
                    for attr in attrs {
                        if attr.name == "when" {
                            if let GtkAttrValue::Splice(expr) = attr.value {
                                when_expr = Some(expr);
                            } else {
                                this.emit_diag(
                                    "E1615",
                                    "<show> `when` must be a splice expression: when={visible}",
                                    child_span.clone(),
                                );
                            }
                        }
                    }
                    let Some(when_expr) = when_expr else {
                        this.emit_diag("E1615", "<show> requires `when={...}`", child_span.clone());
                        continue;
                    };
                    let mut show_children_iter = each_children.into_iter();
                    let Some(show_child) = show_children_iter.next() else {
                        this.emit_diag(
                            "E1615",
                            "<show> requires exactly one child node",
                            child_span.clone(),
                        );
                        continue;
                    };
                    if show_children_iter.next().is_some() {
                        this.emit_diag(
                            "E1615",
                            "<show> requires exactly one child node",
                            child_span.clone(),
                        );
                        continue;
                    }
                    lowered_items.push(ListItem {
                        expr: Expr::Call {
                            func: Box::new(Expr::Ident(SpannedName {
                                name: "gtkShow".into(),
                                span: child_span.clone(),
                            })),
                            args: vec![when_expr, lower_node(this, module_path, show_child)],
                            span: child_span.clone(),
                        },
                        spread: false,
                        span: child_span,
                    });
                    continue;
                }

                let mut each_items: Option<Expr> = None;
                let mut each_binder: Option<SpannedName> = None;
                let mut each_key: Option<Expr> = None;
                for attr in attrs {
                    if attr.name == "items" {
                        if let GtkAttrValue::Splice(expr) = attr.value {
                            each_items = Some(expr);
                        } else {
                            this.emit_diag(
                                "E1615",
                                "<each> `items` must be a splice expression: items={items}",
                                child_span.clone(),
                            );
                        }
                    } else if attr.name == "as" {
                        if let GtkAttrValue::Splice(Expr::Ident(name)) = attr.value {
                            each_binder = Some(name);
                        } else {
                            this.emit_diag(
                                "E1615",
                                "<each> `as` must be an identifier splice: as={item}",
                                child_span.clone(),
                            );
                        }
                    } else if attr.name == "key" {
                        if let GtkAttrValue::Splice(expr) = attr.value {
                            each_key = Some(expr);
                        } else {
                            this.emit_diag(
                                "E1615",
                                "<each> `key` must be a splice expression: key={item => item.id}",
                                child_span.clone(),
                            );
                        }
                    }
                }

                let Some(items_expr) = each_items else {
                    this.emit_diag("E1615", "<each> requires `items={...}`", child_span.clone());
                    continue;
                };
                let Some(item_binder) = each_binder else {
                    this.emit_diag("E1615", "<each> requires `as={...}`", child_span.clone());
                    continue;
                };
                let mut each_children_iter = each_children.into_iter();
                let Some(each_template_node) = each_children_iter.next() else {
                    this.emit_diag(
                        "E1615",
                        "<each> requires exactly one child template node",
                        child_span.clone(),
                    );
                    continue;
                };
                if each_children_iter.next().is_some() {
                    this.emit_diag(
                        "E1615",
                        "<each> requires exactly one child template node",
                        child_span.clone(),
                    );
                    continue;
                }

                let lambda_expr = Expr::Lambda {
                    params: vec![Pattern::Ident(item_binder)],
                    body: Box::new(lower_node(this, module_path, each_template_node)),
                    span: child_span.clone(),
                };
                let func_name = if each_key.is_some() {
                    "gtkEachKeyed"
                } else {
                    "gtkEach"
                };
                let mut args = vec![items_expr];
                if let Some(key_expr) = each_key {
                    args.push(key_expr);
                }
                args.push(lambda_expr);
                lowered_items.push(ListItem {
                    expr: Expr::Call {
                        func: Box::new(Expr::Ident(SpannedName {
                            name: func_name.into(),
                            span: child_span.clone(),
                        })),
                        args,
                        span: child_span.clone(),
                    },
                    spread: false,
                    span: child_span,
                });
            }
            Expr::List {
                items: lowered_items,
                span: span.clone(),
            }
        }

        fn lower_node(this: &mut Parser, module_path: &str, node: GtkNode) -> Expr {
            let span = gtk_node_span(&node).clone();
            let mk_ui = |name: &str| {
                Expr::Ident(SpannedName {
                    name: name.into(),
                    span: span.clone(),
                })
            };
            let mk_string = |value: &str| {
                Expr::Literal(Literal::String {
                    text: value.to_string(),
                    span: span.clone(),
                })
            };
            let list = |items: Vec<Expr>| Expr::List {
                items: items
                    .into_iter()
                    .map(|expr| ListItem {
                        expr,
                        spread: false,
                        span: span.clone(),
                    })
                    .collect(),
                span: span.clone(),
            };
            let call2 = |fname: &str, a: Expr, b: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![a, b],
                span: span.clone(),
            };

            match node {
                GtkNode::Text { text, .. } => Expr::Call {
                    func: Box::new(mk_ui("gtkTextNode")),
                    args: vec![mk_string(&text)],
                    span: span.clone(),
                },
                GtkNode::FunctionCall { tag, args, .. } => {
                    let Some(func) = function_call_tag_expr(&tag, &span) else {
                        return Expr::Ident(SpannedName {
                            name: tag,
                            span: span.clone(),
                        });
                    };
                    let args = if args.is_empty() {
                        vec![Expr::Ident(SpannedName {
                            name: "Unit".into(),
                            span: span.clone(),
                        })]
                    } else {
                        args
                    };
                    Expr::Call {
                        func: Box::new(func),
                        args,
                        span: span.clone(),
                    }
                }
                GtkNode::Splice { expr, .. } => expr,
                GtkNode::Element {
                    tag,
                    attrs,
                    children,
                    ..
                } => {
                    // GTK widget shorthand: tags starting with Gtk/Adw/Gsk are
                    // lowered as `<object class="WidgetName">` where all
                    // non-signal attributes become props automatically.
                    let is_gtk_shorthand =
                        tag.starts_with("Gtk") || tag.starts_with("Adw") || tag.starts_with("Gsk");

                    if is_gtk_shorthand {
                        // Rewrite: <GtkButton label="Hello" onClick={handler}>
                        // → <object class="GtkButton" props={{ label: "Hello" }} onClick={handler}>
                        let mut lowered_attrs = lower_source_meta(module_path, &span);
                        lowered_attrs.push(call2(
                            "gtkStaticAttr",
                            mk_string("class"),
                            mk_string(&tag),
                        ));
                        for attr in attrs {
                            // Signal sugar (onClick, onInput, etc.)
                            let signal_name_opt =
                                gtk_signal_sugar_name(Some(tag.as_str()), attr.name.as_str());
                            if let Some(signal_name) = signal_name_opt {
                                if let Some(lowered) = lower_event_attr(
                                    this,
                                    signal_name,
                                    &attr.name,
                                    attr.value,
                                    &attr.span,
                                ) {
                                    lowered_attrs.push(lowered);
                                }
                                continue;
                            }

                            // Skip standard XML attrs that are handled separately
                            if attr.name == "id" || attr.name == "ref" {
                                lowered_attrs.push(lower_raw_attr(attr));
                                continue;
                            }

                            // Everything else is a prop: normalize and emit as prop:name
                            let prop_name = normalize_prop_name(&attr.name);
                            lowered_attrs.push(lower_prop_attr(&prop_name, attr.value, &attr.span));
                        }

                        // Process children same as <object>
                        let mut kept_children = Vec::new();
                        for child in children {
                            let (child_tag, child_has_type) = match &child {
                                GtkNode::Element { tag, attrs, .. } => {
                                    (tag.clone(), attrs.iter().any(|a| a.name == "type"))
                                }
                                _ => {
                                    kept_children.push(child);
                                    continue;
                                }
                            };
                            if child_tag == "signal" {
                                let mut signal_name = None::<String>;
                                let mut signal_handler = None::<Expr>;
                                if let GtkNode::Element { ref attrs, .. } = child {
                                    for attr in attrs {
                                        if attr.name == "name" {
                                            signal_name = match &attr.value {
                                                GtkAttrValue::Text(v) => Some(v.clone()),
                                                GtkAttrValue::Splice(expr) => {
                                                    compile_time_expr_text(expr)
                                                }
                                                GtkAttrValue::Bare => None,
                                            };
                                        } else if attr.name == "handler" || attr.name == "on" {
                                            signal_handler = match &attr.value {
                                                GtkAttrValue::Splice(expr) => Some(expr.clone()),
                                                GtkAttrValue::Text(v) => Some(mk_string(v)),
                                                GtkAttrValue::Bare => None,
                                            };
                                        }
                                    }
                                }
                                let Some(name) = signal_name else {
                                    let child_span = gtk_node_span(&child).clone();
                                    this.emit_diag(
                                        "E1614",
                                        "signal tag requires a compile-time `name` attribute",
                                        child_span,
                                    );
                                    continue;
                                };
                                let Some(handler) = signal_handler else {
                                    let child_span = gtk_node_span(&child).clone();
                                    this.emit_diag(
                                        "E1614",
                                        "signal tag requires a `handler` or `on` value expression",
                                        child_span,
                                    );
                                    continue;
                                };
                                lowered_attrs.push(call2(
                                    "gtkEventAttr",
                                    mk_string(&name),
                                    handler,
                                ));
                                continue;
                            }
                            if child_tag == "child" {
                                if child_has_type {
                                    kept_children.push(child);
                                } else {
                                    this.emit_diag(
                                        "E1616",
                                        "bare <child> wrapper is not allowed; nest elements directly inside the parent",
                                        span.clone(),
                                    );
                                    if let GtkNode::Element {
                                        children: inner, ..
                                    } = child
                                    {
                                        kept_children.extend(inner);
                                    }
                                }
                                continue;
                            }
                            kept_children.push(child);
                        }

                        let attrs_expr = list(lowered_attrs);
                        let children_expr = lower_children(this, module_path, kept_children, &span);
                        return Expr::Call {
                            func: Box::new(mk_ui("gtkElement")),
                            args: vec![mk_string("object"), attrs_expr, children_expr],
                            span: span.clone(),
                        };
                    }

                    // Component tags (uppercase / dotted) get record-based
                    // lowering: attrs become record fields, children become
                    // a `children` field. No signal sugar or props
                    // normalization — the component function owns its API.
                    if let Some(component_expr) = component_tag_expr(&tag, &span) {
                        let mut fields: Vec<RecordField> = Vec::new();
                        for attr in attrs {
                            let value_expr = match attr.value {
                                GtkAttrValue::Text(v) => mk_string(&v),
                                GtkAttrValue::Splice(expr) => expr,
                                GtkAttrValue::Bare => Expr::Literal(Literal::Bool {
                                    value: true,
                                    span: span.clone(),
                                }),
                            };
                            fields.push(RecordField {
                                spread: false,
                                path: vec![PathSegment::Field(SpannedName {
                                    name: attr.name,
                                    span: span.clone(),
                                })],
                                value: value_expr,
                                span: span.clone(),
                            });
                        }
                        if !children.is_empty() {
                            let children_expr = lower_children(this, module_path, children, &span);
                            fields.push(RecordField {
                                spread: false,
                                path: vec![PathSegment::Field(SpannedName {
                                    name: "children".to_string(),
                                    span: span.clone(),
                                })],
                                value: children_expr,
                                span: span.clone(),
                            });
                        }
                        let record = Expr::Record {
                            fields,
                            span: span.clone(),
                        };
                        return Expr::Call {
                            func: Box::new(component_expr),
                            args: vec![record],
                            span: span.clone(),
                        };
                    }

                    // Built-in GTK element lowering (lowercase tags).
                    let mut lowered_attrs = lower_source_meta(module_path, &span);
                    let sugar_class_name = gtk_element_class_name(&tag, &attrs);
                    for attr in attrs {
                        if attr.name == "props" {
                            match attr.value {
                                GtkAttrValue::Splice(expr) => {
                                    let Expr::Record { fields, .. } = expr else {
                                        this.emit_diag(
                                            "E1612",
                                            "props expects a compile-time record literal: props={ { ... } }",
                                            span.clone(),
                                        );
                                        continue;
                                    };
                                    for field in fields {
                                        if field.spread {
                                            this.emit_diag(
                                                "E1612",
                                                "props does not allow spread fields",
                                                field.span.clone(),
                                            );
                                            continue;
                                        }
                                        let Some(PathSegment::Field(name)) = field.path.first()
                                        else {
                                            this.emit_diag(
                                                "E1612",
                                                "props fields must be simple names",
                                                field.span.clone(),
                                            );
                                            continue;
                                        };
                                        if field.path.len() != 1 {
                                            this.emit_diag(
                                                "E1612",
                                                "props fields must be simple names",
                                                field.span.clone(),
                                            );
                                            continue;
                                        }
                                        let prop_name = normalize_prop_name(&name.name);
                                        lowered_attrs.push(lower_prop_attr(
                                            &prop_name,
                                            GtkAttrValue::Splice(field.value),
                                            &span,
                                        ));
                                    }
                                }
                                _ => {
                                    this.emit_diag(
                                        "E1612",
                                        "props expects a compile-time record literal: props={ { ... } }",
                                        span.clone(),
                                    );
                                }
                            }
                            continue;
                        }
                        let signal_name_opt = gtk_signal_sugar_name(
                            sugar_class_name.as_deref(),
                            attr.name.as_str(),
                        );
                        if let Some(signal_name) = signal_name_opt {
                            if let Some(lowered) = lower_event_attr(
                                this,
                                signal_name,
                                &attr.name,
                                attr.value,
                                &attr.span,
                            ) {
                                lowered_attrs.push(lowered);
                            }
                            continue;
                        }
                        lowered_attrs.push(lower_raw_attr(attr));
                    }

                    let mut kept_children = Vec::new();
                    for child in children {
                        let (child_tag, child_has_type) = match &child {
                            GtkNode::Element { tag, attrs, .. } => {
                                (tag.clone(), attrs.iter().any(|a| a.name == "type"))
                            }
                            _ => {
                                kept_children.push(child);
                                continue;
                            }
                        };
                        if child_tag == "signal" {
                            let mut signal_name = None::<String>;
                            let mut signal_handler = None::<Expr>;
                            if let GtkNode::Element { ref attrs, .. } = child {
                                for attr in attrs {
                                    if attr.name == "name" {
                                        signal_name = match &attr.value {
                                            GtkAttrValue::Text(v) => Some(v.clone()),
                                            GtkAttrValue::Splice(expr) => {
                                                compile_time_expr_text(expr)
                                            }
                                            GtkAttrValue::Bare => None,
                                        };
                                    } else if attr.name == "handler" || attr.name == "on" {
                                        signal_handler = match &attr.value {
                                            GtkAttrValue::Splice(expr) => Some(expr.clone()),
                                            GtkAttrValue::Text(v) => Some(mk_string(v)),
                                            GtkAttrValue::Bare => None,
                                        };
                                    }
                                }
                            }
                            let Some(name) = signal_name else {
                                let child_span = gtk_node_span(&child).clone();
                                this.emit_diag(
                                    "E1614",
                                    "signal tag requires a compile-time `name` attribute",
                                    child_span,
                                );
                                continue;
                            };
                            let Some(handler) = signal_handler else {
                                let child_span = gtk_node_span(&child).clone();
                                this.emit_diag(
                                    "E1614",
                                    "signal tag requires a `handler` or `on` value expression",
                                    child_span,
                                );
                                continue;
                            };
                            lowered_attrs.push(call2("gtkEventAttr", mk_string(&name), handler));
                            continue;
                        }
                        if child_tag == "child" {
                            if child_has_type {
                                kept_children.push(child);
                            } else {
                                this.emit_diag(
                                    "E1616",
                                    "bare <child> wrapper is not allowed; nest <object> elements directly inside the parent",
                                    span.clone(),
                                );
                                if let GtkNode::Element {
                                    children: inner, ..
                                } = child
                                {
                                    kept_children.extend(inner);
                                }
                            }
                            continue;
                        }
                        kept_children.push(child);
                    }

                    let attrs_expr = list(lowered_attrs);
                    // <property> children are text content: wrap splices with gtkTextNode
                    let children_expr = if tag == "property" {
                        let wrapped: Vec<GtkNode> = kept_children
                            .into_iter()
                            .map(|child| {
                                let child_span = gtk_node_span(&child).clone();
                                match child {
                                    GtkNode::Splice { expr, .. } => {
                                        if let Some(text) = static_value_expr_text(&expr) {
                                            GtkNode::Text {
                                                text,
                                                span: child_span,
                                            }
                                        } else {
                                            GtkNode::Splice {
                                                expr: Expr::Call {
                                                    func: Box::new(mk_ui("gtkBoundText")),
                                                    args: vec![expr],
                                                    span: child_span.clone(),
                                                },
                                                span: child_span,
                                            }
                                        }
                                    }
                                    other => other,
                                }
                            })
                            .collect();
                        lower_children(this, module_path, wrapped, &span)
                    } else {
                        lower_children(this, module_path, kept_children, &span)
                    };
                    Expr::Call {
                        func: Box::new(mk_ui("gtkElement")),
                        args: vec![mk_string(&tag), attrs_expr, children_expr],
                        span: span.clone(),
                    }
                }
            }
        }

        let root_span = sigil.span.clone();
        let source_path = self.path.clone();
        if nodes.len() == 1 {
            let root = nodes.remove(0);
            if let GtkNode::Element { tag, .. } = &root {
                if tag == "each" || tag == "show" {
                    self.emit_diag(
                        "E1615",
                        &format!("<{tag}> is only valid inside a GTK element"),
                        root_span.clone(),
                    );
                }
            }
            return lower_node(self, &source_path, root);
        }
        self.emit_diag(
            "E1611",
            "gtk sigil must have a single root element",
            root_span.clone(),
        );

        // Keep a synthetic wrapper for error recovery so downstream passes can continue.
        let wrapper = GtkNode::Element {
            tag: "object".to_string(),
            attrs: vec![GtkAttr {
                name: "class".to_string(),
                value: GtkAttrValue::Text("GtkBox".to_string()),
                span: root_span.clone(),
            }],
            children: nodes,
            span: root_span.clone(),
        };
        lower_node(self, &source_path, wrapper)
    }
}
