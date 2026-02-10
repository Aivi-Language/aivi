use std::path::Path;

use crate::cst::CstToken;
use crate::diagnostics::{Diagnostic, DiagnosticLabel, FileDiagnostic, Position, Span};
use crate::lexer::{filter_tokens, lex, Token, TokenKind};

#[derive(Debug, Clone)]
pub struct SpannedName {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub module: SpannedName,
    pub items: Vec<SpannedName>,
    pub span: Span,
    pub wildcard: bool,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub decorators: Vec<SpannedName>,
    pub name: SpannedName,
    pub params: Vec<Pattern>,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeSig {
    pub decorators: Vec<SpannedName>,
    pub name: SpannedName,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: SpannedName,
    pub params: Vec<SpannedName>,
    pub constructors: Vec<TypeCtor>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub name: SpannedName,
    pub params: Vec<SpannedName>,
    pub aliased: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeCtor {
    pub name: SpannedName,
    pub args: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: SpannedName,
    pub params: Vec<TypeExpr>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassMember {
    pub name: SpannedName,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InstanceDecl {
    pub name: SpannedName,
    pub params: Vec<TypeExpr>,
    pub defs: Vec<Def>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DomainDecl {
    pub name: SpannedName,
    pub over: TypeExpr,
    pub items: Vec<DomainItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum DomainItem {
    TypeAlias(TypeDecl),
    TypeSig(TypeSig),
    Def(Def),
    LiteralDef(Def),
}

#[derive(Debug, Clone)]
pub enum ModuleItem {
    Def(Def),
    TypeSig(TypeSig),
    TypeDecl(TypeDecl),
    TypeAlias(TypeAlias),
    ClassDecl(ClassDecl),
    InstanceDecl(InstanceDecl),
    DomainDecl(DomainDecl),
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: SpannedName,
    pub exports: Vec<SpannedName>,
    pub uses: Vec<UseDecl>,
    pub items: Vec<ModuleItem>,
    pub annotations: Vec<SpannedName>,
    pub span: Span,
    pub path: String,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Name(SpannedName),
    Apply {
        base: Box<TypeExpr>,
        args: Vec<TypeExpr>,
        span: Span,
    },
    Func {
        params: Vec<TypeExpr>,
        result: Box<TypeExpr>,
        span: Span,
    },
    Record {
        fields: Vec<(SpannedName, TypeExpr)>,
        span: Span,
    },
    Tuple {
        items: Vec<TypeExpr>,
        span: Span,
    },
    Star {
        span: Span,
    },
    Unknown {
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number {
        text: String,
        span: Span,
    },
    String {
        text: String,
        span: Span,
    },
    Sigil {
        tag: String,
        body: String,
        flags: String,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    DateTime {
        text: String,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum TextPart {
    Text {
        text: String,
        span: Span,
    },
    Expr {
        expr: Box<Expr>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Expr {
    Ident(SpannedName),
    Literal(Literal),
    TextInterpolate {
        parts: Vec<TextPart>,
        span: Span,
    },
    List {
        items: Vec<ListItem>,
        span: Span,
    },
    Tuple {
        items: Vec<Expr>,
        span: Span,
    },
    Record {
        fields: Vec<RecordField>,
        span: Span,
    },
    FieldAccess {
        base: Box<Expr>,
        field: SpannedName,
        span: Span,
    },
    FieldSection {
        field: SpannedName,
        span: Span,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Lambda {
        params: Vec<Pattern>,
        body: Box<Expr>,
        span: Span,
    },
    Match {
        scrutinee: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: Span,
    },
    Binary {
        op: String,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Block {
        kind: BlockKind,
        items: Vec<BlockItem>,
        span: Span,
    },
    Raw {
        text: String,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ListItem {
    pub expr: Expr,
    pub spread: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RecordField {
    pub path: Vec<PathSegment>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Field(SpannedName),
    Index(Expr, Span),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum BlockKind {
    Plain,
    Effect,
    Generate,
    Resource,
}

#[derive(Debug, Clone)]
pub enum BlockItem {
    Bind {
        pattern: Pattern,
        expr: Expr,
        span: Span,
    },
    Filter {
        expr: Expr,
        span: Span,
    },
    Yield {
        expr: Expr,
        span: Span,
    },
    Recurse {
        expr: Expr,
        span: Span,
    },
    Expr {
        expr: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Span),
    Ident(SpannedName),
    Literal(Literal),
    Constructor {
        name: SpannedName,
        args: Vec<Pattern>,
        span: Span,
    },
    Tuple {
        items: Vec<Pattern>,
        span: Span,
    },
    List {
        items: Vec<Pattern>,
        rest: Option<Box<Pattern>>,
        span: Span,
    },
    Record {
        fields: Vec<RecordPatternField>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct RecordPatternField {
    pub path: Vec<SpannedName>,
    pub pattern: Pattern,
    pub span: Span,
}

pub fn parse_modules(path: &Path, content: &str) -> (Vec<Module>, Vec<FileDiagnostic>) {
    let (cst_tokens, lex_diags) = lex(content);
    let tokens = filter_tokens(&cst_tokens);
    let mut parser = Parser::new(tokens, path);
    let mut modules = parser.parse_modules();
    inject_prelude_imports(&mut modules);
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
            .any(|annotation| annotation.name == "no_prelude")
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

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<FileDiagnostic>,
    path: String,
}

impl Parser {
    fn new(tokens: Vec<Token>, path: &Path) -> Self {
        Self {
            tokens,
            pos: 0,
            diagnostics: Vec::new(),
            path: path.display().to_string(),
        }
    }

    fn parse_modules(&mut self) -> Vec<Module> {
        let mut modules = Vec::new();
        while self.pos < self.tokens.len() {
            let annotations = self.consume_annotations();
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

    fn consume_annotations(&mut self) -> Vec<SpannedName> {
        let mut annotations = Vec::new();
        loop {
            self.consume_newlines();
            if !self.consume_symbol("@") {
                break;
            }
            let Some(name) = self.consume_ident() else {
                self.emit_diag(
                    "E1503",
                    "expected decorator name after `@`",
                    self.previous_span(),
                );
                break;
            };
            if let Some(next) = self.tokens.get(self.pos) {
                if next.span.start.line == name.span.end.line {
                    self.emit_diag(
                        "E1504",
                        "decorators must be written on their own line (decorator arguments are not supported here)",
                        merge_span(name.span.clone(), next.span.clone()),
                    );
                }
            }
            annotations.push(name);
        }
        annotations
    }

    fn parse_module(&mut self, annotations: Vec<SpannedName>) -> Option<Module> {
        let module_kw = self.previous_span();
        let name = self.parse_dotted_name()?;
        self.expect_symbol("=", "expected '=' after module name");
        self.expect_symbol("{", "expected '{' to start module body");
        let mut exports = Vec::new();
        let mut uses = Vec::new();
        let mut items = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            self.consume_newlines();
            let decorators = self.consume_annotations();
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

            if let Some(item) = self.parse_type_or_def(decorators) {
                items.push(item);
                continue;
            }

            self.recover_to_item();
        }
        let end_span = self.expect_symbol("}", "expected '}' to close module body");
        let span = merge_span(module_kw.clone(), end_span.unwrap_or(module_kw));
        for annotation in &annotations {
            if annotation.name != "no_prelude" {
                self.emit_diag(
                    "E1506",
                    &format!("unknown module decorator `@{}`", annotation.name),
                    annotation.span.clone(),
                );
            }
        }
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
                if let Some(name) = self.consume_ident() {
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

    fn validate_item_decorators(&mut self, decorators: &[SpannedName]) {
        for decorator in decorators {
            if !matches!(
                decorator.name.as_str(),
                "static" | "inline" | "deprecated" | "mcp_tool" | "mcp_resource" | "test"
            ) {
                self.emit_diag(
                    "E1506",
                    &format!("unknown decorator `@{}`", decorator.name),
                    decorator.span.clone(),
                );
            }
        }
    }

    fn parse_type_or_def(&mut self, decorators: Vec<SpannedName>) -> Option<ModuleItem> {
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

    fn parse_type_sig(&mut self, decorators: Vec<SpannedName>) -> Option<TypeSig> {
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

    fn parse_def_or_type(&mut self, decorators: Vec<SpannedName>) -> Option<ModuleItem> {
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
        self.expect_symbol("=", "expected '=' in class declaration");
        self.expect_symbol("{", "expected '{' to start class body");
        let mut members = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            let member_name = match self.consume_ident() {
                Some(value) => value,
                None => {
                    self.pos += 1;
                    continue;
                }
            };
            self.expect_symbol(":", "expected ':' in class member");
            let ty = self.parse_type_expr().unwrap_or(TypeExpr::Unknown {
                span: member_name.span.clone(),
            });
            let span = merge_span(member_name.span.clone(), type_span(&ty));
            members.push(ClassMember {
                name: member_name,
                ty,
                span,
            });
        }
        let end = self.expect_symbol("}", "expected '}' to close class body");
        let span = merge_span(start, end.unwrap_or(name.span.clone()));
        Some(ClassDecl {
            name,
            params,
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
        self.expect_symbol("=", "expected '=' in instance declaration");
        self.expect_symbol("{", "expected '{' to start instance body");
        let mut defs = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
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
        self.expect_symbol("=", "expected '=' in domain declaration");
        self.expect_symbol("{", "expected '{' to start domain body");
        let mut items = Vec::new();
        while !self.check_symbol("}") && self.pos < self.tokens.len() {
            self.consume_newlines();
            if self.check_symbol("}") {
                break;
            }
            let decorators = self.consume_annotations();
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

    fn parse_literal_def(&mut self, decorators: Vec<SpannedName>) -> Option<Def> {
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

    fn parse_def(&mut self, decorators: Vec<SpannedName>) -> Option<Def> {
        let name = self.consume_name()?;
        let mut params = Vec::new();
        while !self.check_symbol("=") && self.pos < self.tokens.len() {
            if let Some(pattern) = self.parse_pattern() {
                params.push(pattern);
                continue;
            }
            break;
        }
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
                    self.parse_expr()
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
                    self.parse_expr()
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
                        let index = self.parse_expr().unwrap_or(Expr::Raw {
                            text: String::new(),
                            span: expr_span(&expr),
                        });
                        let end = self.expect_symbol("]", "expected ']' after index");
                        let span = merge_span(expr_span(&expr), end.unwrap_or(expr_span(&expr)));
                        expr = Expr::Index {
                            base: Box::new(expr),
                            index: Box::new(index),
                            span,
                        };
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
                    if token.text == "then" || token.text == "else" {
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

    fn is_pattern_start(&self) -> bool {
        if let Some(token) = self.tokens.get(self.pos) {
            match token.kind {
                TokenKind::Ident | TokenKind::Number | TokenKind::String | TokenKind::Sigil => {
                    return true
                }
                TokenKind::Symbol => return matches!(token.text.as_str(), "(" | "[" | "{" | "-"),
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
                self.consume_newlines();
                if self.consume_symbol(",") {
                    self.consume_newlines();
                    continue;
                }
                if self.check_symbol("]") {
                    break;
                }
                if self.is_expr_start() {
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
                while !self.check_symbol("}") && self.pos < self.tokens.len() {
                    if let Some(field) = self.parse_record_field() {
                        fields.push(field);
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
                    let expr = self.parse_expr().unwrap_or(Expr::Raw {
                        text: String::new(),
                        span: pattern_span(&pattern),
                    });
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

    fn parse_record_field(&mut self) -> Option<RecordField> {
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
        Some(RecordField { path, value, span })
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
        let lhs = self.parse_type_apply()?;
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
            while !self.check_symbol("}") && self.pos < self.tokens.len() {
                if let Some(name) = self.consume_ident() {
                    self.expect_symbol(":", "expected ':' in record type");
                    if let Some(ty) = self.parse_type_expr() {
                        fields.push((name, ty));
                    }
                }
                if !self.consume_symbol(",") {
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
                let expr_start_col =
                    token.span.start.column + 1 + open_index + 1; // opening quote + '{'
                let expr_line = token.span.start.line;

                match self.parse_embedded_expr(&expr_raw, expr_line, expr_start_col) {
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

        let has_expr = parts.iter().any(|part| matches!(part, TextPart::Expr { .. }));
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
        line: usize,
        column: usize,
    ) -> Option<Expr> {
        let (cst_tokens, lex_diags) = lex(text);
        for diag in lex_diags {
            self.diagnostics.push(FileDiagnostic {
                path: self.path.clone(),
                diagnostic: Diagnostic {
                    code: diag.code,
                    message: diag.message,
                    span: shift_span(&diag.span, line - 1, column - 1),
                    labels: diag
                        .labels
                        .into_iter()
                        .map(|label| DiagnosticLabel {
                            message: label.message,
                            span: shift_span(&label.span, line - 1, column - 1),
                        })
                        .collect(),
                },
            });
        }
        let mut tokens = filter_tokens(&cst_tokens);
        for token in &mut tokens {
            token.span = shift_span(&token.span, line - 1, column - 1);
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
    let (tokens, _) = lex(remainder);
    let mut depth = 0usize;
    for token in tokens {
        if token.kind != "symbol" {
            continue;
        }
        match token.text.as_str() {
            "{" => depth += 1,
            "}" => {
                if depth == 0 {
                    return Some(token.span.start.column.saturating_sub(1));
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Ident(name) => name.span.clone(),
        Expr::Literal(literal) => literal_span(literal),
        Expr::TextInterpolate { span, .. } => span.clone(),
        Expr::List { span, .. }
        | Expr::Tuple { span, .. }
        | Expr::Record { span, .. }
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
        TypeExpr::Apply { span, .. }
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
