use crate::diagnostics::Span;

#[derive(Debug, Clone)]
pub struct SpannedName {
    pub name: String,
    pub span: Span,
}

impl SpannedName {
    #[inline]
    pub fn symbol(&self) -> crate::intern::Symbol {
        crate::intern::Symbol::intern(self.name.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeItemKind {
    Value,
    Domain,
}

#[derive(Debug, Clone)]
pub struct UseItem {
    pub kind: ScopeItemKind,
    pub name: SpannedName,
}

#[derive(Debug, Clone)]
pub struct ExportItem {
    pub kind: ScopeItemKind,
    pub name: SpannedName,
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
    pub items: Vec<UseItem>,
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
    pub constraints: Vec<TypeVarConstraint>,
    pub supers: Vec<TypeExpr>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeVarConstraint {
    pub var: SpannedName,
    pub class: SpannedName,
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

/// State machine declaration (Change 7)
#[derive(Debug, Clone)]
pub struct MachineDecl {
    pub decorators: Vec<Decorator>,
    pub name: SpannedName,
    pub states: Vec<MachineState>,
    pub transitions: Vec<MachineTransition>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MachineState {
    pub name: SpannedName,
    pub fields: Vec<(SpannedName, TypeExpr)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MachineTransition {
    pub source: SpannedName,
    pub target: SpannedName,
    pub name: SpannedName,
    pub payload: Vec<(SpannedName, TypeExpr)>,
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
    MachineDecl(MachineDecl),
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: SpannedName,
    pub exports: Vec<ExportItem>,
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
    /// Prefix unary negation: `-expr` (when not a negative numeric literal like `-1`).
    ///
    /// This is kept as a distinct node so elaboration can pick the right numeric zero
    /// (Int vs Float) without relying on implicit numeric promotions.
    UnaryNeg {
        expr: Box<Expr>,
        span: Span,
    },
    /// Postfix domain literal application: `(expr)suffix`.
    ///
    /// This is the generalization of numeric suffix literals like `10px`.
    /// It elaborates as applying the in-scope literal template `1{suffix}` to `expr`.
    Suffixed {
        base: Box<Expr>,
        suffix: SpannedName,
        span: Span,
    },
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
    Do { monad: SpannedName },
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
    /// `when cond <- eff` — conditional effect (Change 6)
    When {
        cond: Expr,
        effect: Expr,
        span: Span,
    },
    /// `unless cond <- eff` — negated conditional effect
    Unless {
        cond: Expr,
        effect: Expr,
        span: Span,
    },
    /// `given cond or failExpr` — precondition guard (Change 8)
    Given {
        cond: Expr,
        fail_expr: Expr,
        span: Span,
    },
    /// `on Transition => effect` — transition event wiring (Change 7)
    On {
        transition: Expr,
        handler: Expr,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Span),
    Ident(SpannedName),
    /// A value binder that is also marked as the subject for deconstructor heads (`|>` / `?`).
    ///
    /// This binds exactly like `Ident`, but is used by surface sugar like:
    ///   `f = { name! } |> ...`   and   `f = { name! } ? | ...`
    SubjectIdent(SpannedName),
    Literal(Literal),
    /// Whole-value binding: `x@p` binds `x` to the matched value while also matching `p`.
    ///
    /// This is distinct from record-pattern field syntax like `{ a.b@{x} }` where `@` separates a
    /// record field path from its subpattern.
    At {
        name: SpannedName,
        pattern: Box<Pattern>,
        subject: bool,
        span: Span,
    },
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
