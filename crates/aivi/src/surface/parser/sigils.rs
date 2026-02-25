impl Parser {
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
        if self.consume_ident_text("mat").is_some() {
            return self.parse_mat_literal(start_span);
        }
        if self.consume_ident_text("path").is_some() {
            return self.parse_path_literal(start_span);
        }
        self.pos = checkpoint;
        None
    }

    fn parse_html_sigil(&mut self, sigil: &Token, body: &str) -> Expr {
        #[derive(Debug, Clone)]
        enum HtmlAttrValue {
            Bare,
            Text(String),
            Splice(Expr),
        }

        #[derive(Debug, Clone)]
        struct HtmlAttr {
            name: String,
            value: HtmlAttrValue,
        }

        #[derive(Debug, Clone)]
        enum HtmlNode {
            Element {
                tag: String,
                attrs: Vec<HtmlAttr>,
                children: Vec<HtmlNode>,
            },
            Text(String),
            Splice(Expr),
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

        // Compute the body offset inside the full sigil token (`~html~> ... <~html`).
        let body_start_offset = sigil
            .text
            .chars()
            .position(|ch| ch == '>')
            .map(|i| i + 1)
            .unwrap_or(0);

        let body_chars: Vec<char> = body.chars().collect();
        let mut i = 0usize;

        let mut nodes: Vec<HtmlNode> = Vec::new();
        let mut stack: Vec<(String, Vec<HtmlAttr>, Vec<HtmlNode>)> = Vec::new();

        let emit_html_diag = |this: &mut Parser, message: &str| {
            this.emit_diag("E1600", message, sigil.span.clone());
        };

        let push_node =
            |node: HtmlNode,
             nodes: &mut Vec<HtmlNode>,
             stack: &mut Vec<(String, Vec<HtmlAttr>, Vec<HtmlNode>)>| {
                if let Some((_tag, _attrs, children)) = stack.last_mut() {
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
                    emit_html_diag(self, "unterminated html splice (missing '}')");
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
                    push_node(HtmlNode::Splice(expr), &mut nodes, &mut stack);
                } else {
                    emit_html_diag(self, "invalid html splice expression");
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
                        emit_html_diag(self, "expected '>' to close html end tag");
                    }

                    if let Some((open_tag, open_attrs, open_children)) = stack.pop() {
                        if open_tag != name {
                            emit_html_diag(
                                self,
                                &format!("mismatched html end tag: expected </{open_tag}>"),
                            );
                        }
                        push_node(
                            HtmlNode::Element {
                                tag: open_tag,
                                attrs: open_attrs,
                                children: open_children,
                            },
                            &mut nodes,
                            &mut stack,
                        );
                    } else {
                        emit_html_diag(self, "unexpected html end tag");
                    }
                    continue;
                }

                // Start tag / self-close.
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
                    emit_html_diag(self, "expected tag name after '<'");
                }
                let tag: String = body_chars[start..i].iter().collect();
                let mut attrs: Vec<HtmlAttr> = Vec::new();

                loop {
                    while i < body_chars.len() && body_chars[i].is_whitespace() {
                        i += 1;
                    }
                    if i >= body_chars.len() {
                        emit_html_diag(self, "unterminated html tag");
                        break;
                    }
                    if body_chars[i] == '>' {
                        i += 1;
                        stack.push((tag.clone(), attrs, Vec::new()));
                        break;
                    }
                    if body_chars[i] == '/' && i + 1 < body_chars.len() && body_chars[i + 1] == '>'
                    {
                        i += 2;
                        push_node(
                            HtmlNode::Element {
                                tag: tag.clone(),
                                attrs,
                                children: Vec::new(),
                            },
                            &mut nodes,
                            &mut stack,
                        );
                        break;
                    }

                    // Attribute name.
                    let astart = i;
                    if i < body_chars.len() && is_name_start(body_chars[i]) {
                        i += 1;
                        while i < body_chars.len() && is_name_continue(body_chars[i]) {
                            i += 1;
                        }
                    } else {
                        emit_html_diag(self, "expected attribute name in html tag");
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
                            HtmlAttrValue::Bare
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
                                emit_html_diag(self, "unterminated quoted attribute value");
                            }
                            HtmlAttrValue::Text(text)
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
                                        Some(expr) => HtmlAttrValue::Splice(expr),
                                        None => HtmlAttrValue::Bare,
                                    }
                                }
                                None => {
                                    emit_html_diag(
                                        self,
                                        "unterminated attribute splice (missing '}')",
                                    );
                                    i += 1;
                                    HtmlAttrValue::Bare
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
                            HtmlAttrValue::Text(body_chars[vstart..i].iter().collect())
                        }
                    } else {
                        HtmlAttrValue::Bare
                    };

                    attrs.push(HtmlAttr { name, value });
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
                push_node(HtmlNode::Text(text), &mut nodes, &mut stack);
            }
        }

        // Close any unclosed tags.
        while let Some((open_tag, open_attrs, open_children)) = stack.pop() {
            emit_html_diag(self, &format!("unclosed html tag <{open_tag}>"));
            push_node(
                HtmlNode::Element {
                    tag: open_tag,
                    attrs: open_attrs,
                    children: open_children,
                },
                &mut nodes,
                &mut stack,
            );
        }

        // Lower parsed HTML nodes to `aivi.ui` constructors.
        fn lower_attr(_this: &mut Parser, attr: HtmlAttr, span: &Span) -> Option<Expr> {
            // Lower into the public `aivi.ui` helper functions (e.g. `vElement`, `vClass`).
            // Users are expected to `use aivi.ui` (or selectively import these helpers).
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
            let call1 = |fname: &str, arg: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![arg],
                span: span.clone(),
            };
            let call2 = |fname: &str, a: Expr, b: Expr| Expr::Call {
                func: Box::new(mk_ui(fname)),
                args: vec![a, b],
                span: span.clone(),
            };

            let name = attr.name;
            match (name.as_str(), attr.value) {
                ("class", HtmlAttrValue::Text(v)) => Some(call1("vClass", mk_string(&v))),
                ("id", HtmlAttrValue::Text(v)) => Some(call1("vId", mk_string(&v))),
                ("style", HtmlAttrValue::Splice(expr)) => Some(call1("vStyle", expr)),
                ("onClick", HtmlAttrValue::Splice(expr)) => Some(call1("vOnClick", expr)),
                ("onClickE", HtmlAttrValue::Splice(expr)) => Some(call1("vOnClickE", expr)),
                ("onInput", HtmlAttrValue::Splice(expr)) => Some(call1("vOnInput", expr)),
                ("onInputE", HtmlAttrValue::Splice(expr)) => Some(call1("vOnInputE", expr)),
                ("onKeyDown", HtmlAttrValue::Splice(expr)) => Some(call1("vOnKeyDown", expr)),
                ("onKeyUp", HtmlAttrValue::Splice(expr)) => Some(call1("vOnKeyUp", expr)),
                ("onPointerDown", HtmlAttrValue::Splice(expr)) => {
                    Some(call1("vOnPointerDown", expr))
                }
                ("onPointerUp", HtmlAttrValue::Splice(expr)) => Some(call1("vOnPointerUp", expr)),
                ("onPointerMove", HtmlAttrValue::Splice(expr)) => {
                    Some(call1("vOnPointerMove", expr))
                }
                ("onFocus", HtmlAttrValue::Splice(expr)) => Some(call1("vOnFocus", expr)),
                ("onBlur", HtmlAttrValue::Splice(expr)) => Some(call1("vOnBlur", expr)),
                ("key", _) => None, // handled separately
                (_other, HtmlAttrValue::Text(v)) => {
                    Some(call2("vAttr", mk_string(&name), mk_string(&v)))
                }
                (_other, HtmlAttrValue::Splice(expr)) => {
                    Some(call2("vAttr", mk_string(&name), expr))
                }
                (_other, HtmlAttrValue::Bare) => {
                    Some(call2("vAttr", mk_string(&name), mk_string("true")))
                }
            }
        }

        fn component_tag_expr(tag: &str, span: &Span) -> Option<Expr> {
            let mut segments = tag.split('.');
            let first = segments.next()?;
            if first.is_empty() || !first.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
                return None;
            }
            let is_ident_segment = |segment: &str| -> bool {
                let mut chars = segment.chars();
                let Some(head) = chars.next() else {
                    return false;
                };
                (head.is_ascii_alphabetic() || head == '_')
                    && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            };
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

        fn lower_node(this: &mut Parser, node: HtmlNode, span: &Span) -> Expr {
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

            match node {
                HtmlNode::Text(t) => Expr::Call {
                    func: Box::new(mk_ui("vText")),
                    args: vec![mk_string(&t)],
                    span: span.clone(),
                },
                HtmlNode::Splice(expr) => expr,
                HtmlNode::Element {
                    tag,
                    attrs,
                    children,
                } => {
                    let mut key_expr: Option<Expr> = None;
                    let mut lowered_attrs = Vec::new();
                    for attr in attrs {
                        if attr.name == "key" {
                            key_expr = Some(match attr.value {
                                HtmlAttrValue::Text(v) => mk_string(&v),
                                HtmlAttrValue::Splice(expr) => expr,
                                HtmlAttrValue::Bare => mk_string(""),
                            });
                            continue;
                        }
                        if let Some(expr) = lower_attr(this, attr, span) {
                            lowered_attrs.push(expr);
                        }
                    }

                    let lowered_children: Vec<Expr> = children
                        .into_iter()
                        .map(|child| lower_node(this, child, span))
                        .collect();

                    let attrs_expr = list(lowered_attrs);
                    let children_expr = list(lowered_children);
                    let element_expr = if let Some(component_expr) = component_tag_expr(&tag, span)
                    {
                        Expr::Call {
                            func: Box::new(component_expr),
                            args: vec![attrs_expr, children_expr],
                            span: span.clone(),
                        }
                    } else {
                        Expr::Call {
                            func: Box::new(mk_ui("vElement")),
                            args: vec![mk_string(&tag), attrs_expr, children_expr],
                            span: span.clone(),
                        }
                    };
                    if let Some(key_expr) = key_expr {
                        Expr::Call {
                            func: Box::new(mk_ui("vKeyed")),
                            args: vec![key_expr, element_expr],
                            span: span.clone(),
                        }
                    } else {
                        element_expr
                    }
                }
            }
        }

        let root_span = sigil.span.clone();
        if nodes.len() == 1 {
            return lower_node(self, nodes.remove(0), &root_span);
        }
        self.emit_diag(
            "E1601",
            "html sigil must have a single root element",
            root_span.clone(),
        );

        // Keep a synthetic wrapper for error recovery so downstream passes can continue.
        let wrapper = HtmlNode::Element {
            tag: "div".to_string(),
            attrs: Vec::new(),
            children: nodes,
        };
        lower_node(self, wrapper, &root_span)
    }

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
        }

        #[derive(Debug, Clone)]
        enum GtkNode {
            Element {
                tag: String,
                attrs: Vec<GtkAttr>,
                children: Vec<GtkNode>,
            },
            Text(String),
            Splice(Expr),
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
        let mut stack: Vec<(String, Vec<GtkAttr>, Vec<GtkNode>)> = Vec::new();

        let emit_gtk_diag = |this: &mut Parser, message: &str| {
            this.emit_diag("E1610", message, sigil.span.clone());
        };

        let push_node =
            |node: GtkNode,
             nodes: &mut Vec<GtkNode>,
             stack: &mut Vec<(String, Vec<GtkAttr>, Vec<GtkNode>)>| {
                if let Some((_tag, _attrs, children)) = stack.last_mut() {
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
                    push_node(GtkNode::Splice(expr), &mut nodes, &mut stack);
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

                    if let Some((open_tag, open_attrs, open_children)) = stack.pop() {
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
                        stack.push((tag.clone(), attrs, Vec::new()));
                        break;
                    }
                    if body_chars[i] == '/' && i + 1 < body_chars.len() && body_chars[i + 1] == '>'
                    {
                        i += 2;
                        push_node(
                            GtkNode::Element {
                                tag: tag.clone(),
                                attrs,
                                children: Vec::new(),
                            },
                            &mut nodes,
                            &mut stack,
                        );
                        break;
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

                    attrs.push(GtkAttr { name, value });
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
                push_node(GtkNode::Text(text), &mut nodes, &mut stack);
            }
        }

        // Close any unclosed tags.
        while let Some((open_tag, open_attrs, open_children)) = stack.pop() {
            emit_gtk_diag(self, &format!("unclosed gtk tag <{open_tag}>"));
            push_node(
                GtkNode::Element {
                    tag: open_tag,
                    attrs: open_attrs,
                    children: open_children,
                },
                &mut nodes,
                &mut stack,
            );
        }

        // Lower parsed GTK XML nodes to `aivi.ui.gtk4` helper constructors.
        fn lower_attr(attr: GtkAttr, span: &Span) -> Expr {
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
            let value_expr = match attr.value {
                GtkAttrValue::Text(v) => mk_string(&v),
                GtkAttrValue::Splice(expr) => expr,
                GtkAttrValue::Bare => mk_string("true"),
            };
            call2("gtkAttr", mk_string(&attr.name), value_expr)
        }

        fn component_tag_expr(tag: &str, span: &Span) -> Option<Expr> {
            let mut segments = tag.split('.');
            let first = segments.next()?;
            if first.is_empty() || !first.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()) {
                return None;
            }
            let is_ident_segment = |segment: &str| -> bool {
                let mut chars = segment.chars();
                let Some(head) = chars.next() else {
                    return false;
                };
                (head.is_ascii_alphabetic() || head == '_')
                    && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            };
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

        fn lower_children(this: &mut Parser, children: Vec<GtkNode>, span: &Span) -> Expr {
            let mut lowered_items: Vec<ListItem> = Vec::new();
            for child in children {
                let GtkNode::Element {
                    tag,
                    attrs,
                    children: each_children,
                } = child.clone()
                else {
                    lowered_items.push(ListItem {
                        expr: lower_node(this, child, span),
                        spread: false,
                        span: span.clone(),
                    });
                    continue;
                };
                if tag != "each" {
                    lowered_items.push(ListItem {
                        expr: lower_node(
                            this,
                            GtkNode::Element {
                                tag,
                                attrs,
                                children: each_children,
                            },
                            span,
                        ),
                        spread: false,
                        span: span.clone(),
                    });
                    continue;
                }

                let mut each_items: Option<Expr> = None;
                let mut each_binder: Option<SpannedName> = None;
                for attr in attrs {
                    if attr.name == "items" {
                        if let GtkAttrValue::Splice(expr) = attr.value {
                            each_items = Some(expr);
                        } else {
                            this.emit_diag(
                                "E1615",
                                "<each> `items` must be a splice expression: items={items}",
                                span.clone(),
                            );
                        }
                    } else if attr.name == "as" {
                        if let GtkAttrValue::Splice(Expr::Ident(name)) = attr.value {
                            each_binder = Some(name);
                        } else {
                            this.emit_diag(
                                "E1615",
                                "<each> `as` must be an identifier splice: as={item}",
                                span.clone(),
                            );
                        }
                    }
                }

                let Some(items_expr) = each_items else {
                    this.emit_diag(
                        "E1615",
                        "<each> requires `items={...}`",
                        span.clone(),
                    );
                    continue;
                };
                let Some(item_binder) = each_binder else {
                    this.emit_diag("E1615", "<each> requires `as={...}`", span.clone());
                    continue;
                };
                let mut each_children_iter = each_children.into_iter();
                let Some(each_template_node) = each_children_iter.next() else {
                    this.emit_diag(
                        "E1615",
                        "<each> requires exactly one child template node",
                        span.clone(),
                    );
                    continue;
                };
                if each_children_iter.next().is_some() {
                    this.emit_diag(
                        "E1615",
                        "<each> requires exactly one child template node",
                        span.clone(),
                    );
                    continue;
                }

                let lambda_expr = Expr::Lambda {
                    params: vec![Pattern::Ident(item_binder)],
                    body: Box::new(lower_node(this, each_template_node, span)),
                    span: span.clone(),
                };
                let mapped_expr = Expr::Call {
                    func: Box::new(Expr::Ident(SpannedName {
                        name: "each".into(),
                        span: span.clone(),
                    })),
                    args: vec![lambda_expr, items_expr],
                    span: span.clone(),
                };
                lowered_items.push(ListItem {
                    expr: mapped_expr,
                    spread: true,
                    span: span.clone(),
                });
            }
            Expr::List {
                items: lowered_items,
                span: span.clone(),
            }
        }

        fn lower_node(this: &mut Parser, node: GtkNode, span: &Span) -> Expr {
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
                GtkNode::Text(t) => Expr::Call {
                    func: Box::new(mk_ui("gtkTextNode")),
                    args: vec![mk_string(&t)],
                    span: span.clone(),
                },
                GtkNode::Splice(expr) => expr,
                GtkNode::Element {
                    tag,
                    attrs,
                    children,
                } => {
                    let mut lowered_attrs = Vec::new();
                    let attr_handler_text =
                        |attr_name: &str, value: GtkAttrValue| -> Option<String> {
                            match value {
                                GtkAttrValue::Text(v) => Some(v),
                                GtkAttrValue::Splice(expr) => compile_time_expr_text(&expr),
                                GtkAttrValue::Bare => {
                                    let _ = attr_name;
                                    None
                                }
                            }
                        };
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
                                        let Some(prop_value_text) =
                                            compile_time_expr_text(&field.value)
                                        else {
                                            this.emit_diag(
                                                "E1613",
                                                "props field values must be compile-time literals",
                                                field.span.clone(),
                                            );
                                            continue;
                                        };
                                        let prop_name = normalize_prop_name(&name.name);
                                        lowered_attrs.push(call2(
                                            "gtkAttr",
                                            mk_string(&format!("prop:{prop_name}")),
                                            mk_string(&prop_value_text),
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
                        if attr.name == "onClick" || attr.name == "onInput" {
                            let signal_name = if attr.name == "onClick" {
                                "clicked"
                            } else {
                                "changed"
                            };
                            let Some(handler) = attr_handler_text(&attr.name, attr.value) else {
                                this.emit_diag(
                                    "E1614",
                                    "signal handlers must be compile-time values",
                                    span.clone(),
                                );
                                continue;
                            };
                            lowered_attrs.push(call2(
                                "gtkAttr",
                                mk_string(&format!("signal:{signal_name}")),
                                mk_string(&handler),
                            ));
                            continue;
                        }
                        lowered_attrs.push(lower_attr(attr, span));
                    }

                    let mut kept_children = Vec::new();
                    for child in children {
                        let GtkNode::Element {
                            tag: child_tag,
                            attrs: child_attrs,
                            children: _,
                        } = &child
                        else {
                            kept_children.push(child);
                            continue;
                        };
                        if child_tag != "signal" {
                            kept_children.push(child);
                            continue;
                        }
                        let mut signal_name: Option<String> = None;
                        let mut signal_handler: Option<String> = None;
                        for attr in child_attrs.iter().cloned() {
                            if attr.name == "name" {
                                signal_name = attr_handler_text("name", attr.value);
                            } else if attr.name == "handler" || attr.name == "on" {
                                signal_handler = attr_handler_text(&attr.name, attr.value);
                            }
                        }
                        let Some(name) = signal_name else {
                            this.emit_diag(
                                "E1614",
                                "signal tag requires a compile-time `name` attribute",
                                span.clone(),
                            );
                            continue;
                        };
                        let Some(handler) = signal_handler else {
                            this.emit_diag(
                                "E1614",
                                "signal tag requires a compile-time `handler` or `on` attribute",
                                span.clone(),
                            );
                            continue;
                        };
                        lowered_attrs.push(call2(
                            "gtkAttr",
                            mk_string(&format!("signal:{name}")),
                            mk_string(&handler),
                        ));
                    }

                    let attrs_expr = list(lowered_attrs);
                    let children_expr = lower_children(this, kept_children, span);
                    if let Some(component_expr) = component_tag_expr(&tag, span) {
                        Expr::Call {
                            func: Box::new(component_expr),
                            args: vec![attrs_expr, children_expr],
                            span: span.clone(),
                        }
                    } else {
                        Expr::Call {
                            func: Box::new(mk_ui("gtkElement")),
                            args: vec![mk_string(&tag), attrs_expr, children_expr],
                            span: span.clone(),
                        }
                    }
                }
            }
        }

        let root_span = sigil.span.clone();
        if nodes.len() == 1 {
            let root = nodes.remove(0);
            if let GtkNode::Element { tag, .. } = &root {
                if tag == "each" {
                    self.emit_diag(
                        "E1615",
                        "<each> is only valid inside a GTK element",
                        root_span.clone(),
                    );
                }
            }
            return lower_node(self, root, &root_span);
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
            }],
            children: nodes,
        };
        lower_node(self, wrapper, &root_span)
    }
}
