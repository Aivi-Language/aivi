mod lower;

pub use lower::lower_modules_to_arena;

use crate::diagnostics::Span;
use crate::intern::{ExprId, PatternId, Symbol, TypeExprId};
use crate::surface::{ScopeItemKind, SpannedName};

#[derive(Debug, Clone)]
pub struct SpannedSymbol {
    pub symbol: Symbol,
    pub span: Span,
}

impl From<&SpannedName> for SpannedSymbol {
    fn from(value: &SpannedName) -> Self {
        Self {
            symbol: value.symbol(),
            span: value.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ArenaLiteral {
    Number {
        text: Symbol,
        span: Span,
    },
    String {
        text: Symbol,
        span: Span,
    },
    Sigil {
        tag: Symbol,
        body: Symbol,
        flags: Symbol,
        span: Span,
    },
    Bool {
        value: bool,
        span: Span,
    },
    DateTime {
        text: Symbol,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum ArenaTextPart {
    Text { text: Symbol, span: Span },
    Expr { expr: ExprId, span: Span },
}

#[derive(Debug, Clone)]
pub enum ArenaExpr {
    Ident(SpannedSymbol),
    Literal(ArenaLiteral),
    UnaryNeg {
        expr: ExprId,
        span: Span,
    },
    Suffixed {
        base: ExprId,
        suffix: SpannedSymbol,
        span: Span,
    },
    TextInterpolate {
        parts: Vec<ArenaTextPart>,
        span: Span,
    },
    List {
        items: Vec<ArenaListItem>,
        span: Span,
    },
    Tuple {
        items: Vec<ExprId>,
        span: Span,
    },
    Record {
        fields: Vec<ArenaRecordField>,
        span: Span,
    },
    PatchLit {
        fields: Vec<ArenaRecordField>,
        span: Span,
    },
    FieldAccess {
        base: ExprId,
        field: SpannedSymbol,
        span: Span,
    },
    FieldSection {
        field: SpannedSymbol,
        span: Span,
    },
    Index {
        base: ExprId,
        index: ExprId,
        span: Span,
    },
    Call {
        func: ExprId,
        args: Vec<ExprId>,
        span: Span,
    },
    Lambda {
        params: Vec<PatternId>,
        body: ExprId,
        span: Span,
    },
    Match {
        scrutinee: Option<ExprId>,
        arms: Vec<ArenaMatchArm>,
        span: Span,
    },
    If {
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
        span: Span,
    },
    Binary {
        op: Symbol,
        left: ExprId,
        right: ExprId,
        span: Span,
    },
    Flow {
        root: ExprId,
        lines: Vec<ArenaFlowLine>,
        span: Span,
    },
    Block {
        kind: ArenaBlockKind,
        items: Vec<ArenaBlockItem>,
        span: Span,
    },
    Raw {
        text: Symbol,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ArenaListItem {
    pub expr: ExprId,
    pub spread: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaRecordField {
    pub spread: bool,
    pub path: Vec<ArenaPathSegment>,
    pub value: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaPathSegment {
    Field(SpannedSymbol),
    Index(ExprId, Span),
    All(Span),
}

#[derive(Debug, Clone)]
pub struct ArenaMatchArm {
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub guard_negated: bool,
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaFlowBinding {
    pub name: SpannedSymbol,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaFlowModifier {
    Timeout {
        duration: ExprId,
        span: Span,
    },
    Delay {
        duration: ExprId,
        span: Span,
    },
    Concurrent {
        limit: ExprId,
        span: Span,
    },
    Retry {
        attempts: u32,
        interval: ExprId,
        exponential: bool,
        span: Span,
    },
    Cleanup {
        expr: ExprId,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaFlowStepKind {
    Flow,
    Tap,
    Attempt,
    FanOut,
    Applicative,
}

#[derive(Debug, Clone)]
pub struct ArenaFlowStep {
    pub kind: ArenaFlowStepKind,
    pub expr: ExprId,
    pub modifiers: Vec<ArenaFlowModifier>,
    pub binding: Option<ArenaFlowBinding>,
    pub subflow: Vec<ArenaFlowLine>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaFlowGuard {
    pub predicate: ExprId,
    pub fail_expr: Option<ExprId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaFlowArm {
    pub pattern: PatternId,
    pub guard: Option<ExprId>,
    pub guard_negated: bool,
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaFlowAnchor {
    pub name: SpannedSymbol,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaFlowLine {
    Step(ArenaFlowStep),
    Guard(ArenaFlowGuard),
    Branch(ArenaFlowArm),
    Recover(ArenaFlowArm),
    Anchor(ArenaFlowAnchor),
}

#[derive(Debug, Clone)]
pub enum ArenaBlockKind {
    Plain,
    Do { monad: SpannedSymbol },
    Generate,
    Managed,
}

#[derive(Debug, Clone)]
pub enum ArenaBlockItem {
    Bind {
        pattern: PatternId,
        expr: ExprId,
        span: Span,
    },
    Let {
        pattern: PatternId,
        expr: ExprId,
        span: Span,
    },
    Filter {
        expr: ExprId,
        span: Span,
    },
    Yield {
        expr: ExprId,
        span: Span,
    },
    Recurse {
        expr: ExprId,
        span: Span,
    },
    Expr {
        expr: ExprId,
        span: Span,
    },
    When {
        cond: ExprId,
        effect: ExprId,
        span: Span,
    },
    Unless {
        cond: ExprId,
        effect: ExprId,
        span: Span,
    },
    Given {
        cond: ExprId,
        fail_expr: ExprId,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub enum ArenaPattern {
    Wildcard(Span),
    Ident(SpannedSymbol),
    SubjectIdent(SpannedSymbol),
    Literal(ArenaLiteral),
    At {
        name: SpannedSymbol,
        pattern: PatternId,
        subject: bool,
        span: Span,
    },
    Constructor {
        name: SpannedSymbol,
        args: Vec<PatternId>,
        span: Span,
    },
    Tuple {
        items: Vec<PatternId>,
        span: Span,
    },
    List {
        items: Vec<PatternId>,
        rest: Option<PatternId>,
        span: Span,
    },
    Record {
        fields: Vec<ArenaRecordPatternField>,
        span: Span,
    },
}

#[derive(Debug, Clone)]
pub struct ArenaRecordPatternField {
    pub path: Vec<SpannedSymbol>,
    pub pattern: PatternId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaRecordTypeField {
    Named { name: SpannedSymbol, ty: TypeExprId },
    Spread { ty: TypeExprId, span: Span },
}

#[derive(Debug, Clone)]
pub enum ArenaTypeExpr {
    Name(SpannedSymbol),
    And {
        items: Vec<TypeExprId>,
        span: Span,
    },
    Apply {
        base: TypeExprId,
        args: Vec<TypeExprId>,
        span: Span,
    },
    Func {
        params: Vec<TypeExprId>,
        result: TypeExprId,
        span: Span,
    },
    Record {
        fields: Vec<ArenaRecordTypeField>,
        span: Span,
    },
    Tuple {
        items: Vec<TypeExprId>,
        span: Span,
    },
    Star {
        span: Span,
    },
    Unknown {
        span: Span,
    },
}

#[derive(Debug, Default, Clone)]
pub struct AstArena {
    pub exprs: Vec<ArenaExpr>,
    pub patterns: Vec<ArenaPattern>,
    pub type_exprs: Vec<ArenaTypeExpr>,
}

impl AstArena {
    pub fn alloc_expr(&mut self, expr: ArenaExpr) -> ExprId {
        let id = ExprId::new(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    pub fn alloc_pattern(&mut self, pattern: ArenaPattern) -> PatternId {
        let id = PatternId::new(self.patterns.len() as u32);
        self.patterns.push(pattern);
        id
    }

    pub fn alloc_type_expr(&mut self, ty: ArenaTypeExpr) -> TypeExprId {
        let id = TypeExprId::new(self.type_exprs.len() as u32);
        self.type_exprs.push(ty);
        id
    }

    pub fn expr(&self, id: ExprId) -> &ArenaExpr {
        &self.exprs[id.as_u32() as usize]
    }

    pub fn pattern(&self, id: PatternId) -> &ArenaPattern {
        &self.patterns[id.as_u32() as usize]
    }

    pub fn type_expr(&self, id: TypeExprId) -> &ArenaTypeExpr {
        &self.type_exprs[id.as_u32() as usize]
    }
}

#[derive(Debug, Clone)]
pub struct ArenaDef {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<PatternId>,
    pub expr: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaDecorator {
    pub name: SpannedSymbol,
    pub arg: Option<ExprId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeSig {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub ty: TypeExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeDecl {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<SpannedSymbol>,
    pub constructors: Vec<ArenaTypeCtor>,
    pub opaque: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeAlias {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<SpannedSymbol>,
    pub aliased: TypeExprId,
    pub opaque: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeCtor {
    pub name: SpannedSymbol,
    pub args: Vec<TypeExprId>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaClassDecl {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<TypeExprId>,
    pub constraints: Vec<ArenaTypeVarConstraint>,
    pub supers: Vec<TypeExprId>,
    pub members: Vec<ArenaClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeVarConstraint {
    pub var: SpannedSymbol,
    pub class: SpannedSymbol,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaClassMember {
    pub name: SpannedSymbol,
    pub ty: TypeExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaInstanceDecl {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<TypeExprId>,
    pub defs: Vec<ArenaDef>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaDomainDecl {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub over: TypeExprId,
    pub items: Vec<ArenaDomainItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaDomainItem {
    TypeAlias(ArenaTypeDecl),
    TypeSig(ArenaTypeSig),
    Def(ArenaDef),
    LiteralDef(ArenaDef),
}

#[derive(Debug, Clone)]
pub enum ArenaModuleItem {
    Def(ArenaDef),
    TypeSig(ArenaTypeSig),
    TypeDecl(ArenaTypeDecl),
    TypeAlias(ArenaTypeAlias),
    ClassDecl(ArenaClassDecl),
    InstanceDecl(ArenaInstanceDecl),
    DomainDecl(ArenaDomainDecl),
}

#[derive(Debug, Clone)]
pub struct ArenaModule {
    pub name: SpannedSymbol,
    pub exports: Vec<ArenaScopeItem>,
    pub uses: Vec<ArenaUseDecl>,
    pub items: Vec<ArenaModuleItem>,
    pub annotations: Vec<ArenaDecorator>,
    pub span: Span,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ArenaScopeItem {
    pub kind: ScopeItemKind,
    pub name: SpannedSymbol,
    pub alias: Option<SpannedSymbol>,
}

#[derive(Debug, Clone)]
pub struct ArenaUseDecl {
    pub module: SpannedSymbol,
    pub items: Vec<ArenaScopeItem>,
    pub span: Span,
    pub wildcard: bool,
    pub hiding: bool,
    pub alias: Option<SpannedSymbol>,
}
