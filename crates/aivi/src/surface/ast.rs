use crate::diagnostics::Span;

#[derive(Debug, Clone)]
pub struct SpannedName {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Decorator {
    pub name: SpannedName,
    pub arg: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct UseDecl {
    pub module: SpannedName,
    pub items: Vec<SpannedName>,
    pub span: Span,
    pub wildcard: bool,
    pub alias: Option<SpannedName>,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub params: Vec<Pattern>,
    pub expr: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeSig {
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub params: Vec<SpannedName>,
    pub constructors: Vec<TypeCtor>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAlias {
    pub decorators: Vec<Decorator>,
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
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub params: Vec<TypeExpr>,
    pub supers: Vec<TypeExpr>,
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
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub params: Vec<TypeExpr>,
    pub defs: Vec<Def>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DomainDecl {
    pub decorators: Vec<Decorator>,
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
    pub annotations: Vec<Decorator>,
    pub span: Span,
    pub path: String,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Name(SpannedName),
    And {
        items: Vec<TypeExpr>,
        span: Span,
    },
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
    Text { text: String, span: Span },
    Expr { expr: Box<Expr>, span: Span },
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
    PatchLit {
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
    pub spread: bool,
    pub path: Vec<PathSegment>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Field(SpannedName),
    Index(Expr, Span),
    All(Span),
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
    Let {
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
