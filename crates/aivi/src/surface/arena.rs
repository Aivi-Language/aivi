use crate::diagnostics::Span;
use crate::intern::{ExprId, PatternId, Symbol, TypeExprId};
use crate::surface::{
    BlockItem, BlockKind, ClassDecl, ClassMember, Decorator, Def, DomainDecl, DomainItem,
    ExportItem, Expr, InstanceDecl, ListItem, Literal, MachineDecl, MachineState,
    MachineTransition, MatchArm, Module, ModuleItem, PathSegment, Pattern, RecordField,
    RecordPatternField, ScopeItemKind, SpannedName, TextPart, TypeAlias, TypeCtor, TypeDecl,
    TypeExpr, TypeSig, TypeVarConstraint, UseDecl, UseItem,
};

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
    pub body: ExprId,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ArenaBlockKind {
    Plain,
    Do { monad: SpannedSymbol },
    Generate,
    Resource,
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
    On {
        transition: ExprId,
        handler: ExprId,
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
        fields: Vec<(SpannedSymbol, TypeExprId)>,
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
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaTypeAlias {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub params: Vec<SpannedSymbol>,
    pub aliased: TypeExprId,
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
pub struct ArenaMachineDecl {
    pub decorators: Vec<ArenaDecorator>,
    pub name: SpannedSymbol,
    pub states: Vec<ArenaMachineState>,
    pub transitions: Vec<ArenaMachineTransition>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaMachineState {
    pub name: SpannedSymbol,
    pub fields: Vec<(SpannedSymbol, TypeExprId)>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ArenaMachineTransition {
    pub source: SpannedSymbol,
    pub target: SpannedSymbol,
    pub name: SpannedSymbol,
    pub payload: Vec<(SpannedSymbol, TypeExprId)>,
    pub span: Span,
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
    MachineDecl(ArenaMachineDecl),
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
}

#[derive(Debug, Clone)]
pub struct ArenaUseDecl {
    pub module: SpannedSymbol,
    pub items: Vec<ArenaScopeItem>,
    pub span: Span,
    pub wildcard: bool,
    pub alias: Option<SpannedSymbol>,
}

pub fn lower_modules_to_arena(modules: &[Module]) -> (AstArena, Vec<ArenaModule>) {
    let mut builder = ArenaBuilder::default();
    let lowered = modules.iter().map(|m| builder.lower_module(m)).collect();
    (builder.arena, lowered)
}

#[derive(Default)]
struct ArenaBuilder {
    arena: AstArena,
}

impl ArenaBuilder {
    fn lower_module(&mut self, module: &Module) -> ArenaModule {
        let mut items = Vec::new();
        for item in &module.items {
            match item {
                ModuleItem::Def(def) => items.push(ArenaModuleItem::Def(self.lower_def(def))),
                ModuleItem::TypeSig(sig) => {
                    items.push(ArenaModuleItem::TypeSig(self.lower_type_sig(sig)))
                }
                ModuleItem::TypeDecl(ty) => {
                    items.push(ArenaModuleItem::TypeDecl(self.lower_type_decl(ty)))
                }
                ModuleItem::TypeAlias(alias) => {
                    items.push(ArenaModuleItem::TypeAlias(self.lower_type_alias(alias)))
                }
                ModuleItem::ClassDecl(class_decl) => items.push(ArenaModuleItem::ClassDecl(
                    self.lower_class_decl(class_decl),
                )),
                ModuleItem::InstanceDecl(instance_decl) => items.push(
                    ArenaModuleItem::InstanceDecl(self.lower_instance_decl(instance_decl)),
                ),
                ModuleItem::DomainDecl(domain_decl) => items.push(ArenaModuleItem::DomainDecl(
                    self.lower_domain_decl(domain_decl),
                )),
                ModuleItem::MachineDecl(machine_decl) => items.push(ArenaModuleItem::MachineDecl(
                    self.lower_machine_decl(machine_decl),
                )),
            }
        }
        ArenaModule {
            name: SpannedSymbol::from(&module.name),
            exports: module
                .exports
                .iter()
                .map(|x| self.lower_export_item(x))
                .collect(),
            uses: module.uses.iter().map(|x| self.lower_use_decl(x)).collect(),
            items,
            annotations: module
                .annotations
                .iter()
                .map(|x| self.lower_decorator(x))
                .collect(),
            span: module.span.clone(),
            path: module.path.clone(),
        }
    }

    fn lower_def(&mut self, def: &Def) -> ArenaDef {
        ArenaDef {
            decorators: def
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&def.name),
            params: def.params.iter().map(|p| self.lower_pattern(p)).collect(),
            expr: self.lower_expr(&def.expr),
            span: def.span.clone(),
        }
    }

    fn lower_type_sig(&mut self, sig: &TypeSig) -> ArenaTypeSig {
        ArenaTypeSig {
            decorators: sig
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&sig.name),
            ty: self.lower_type_expr(&sig.ty),
            span: sig.span.clone(),
        }
    }

    fn lower_type_ctor(&mut self, ctor: &TypeCtor) -> ArenaTypeCtor {
        ArenaTypeCtor {
            name: SpannedSymbol::from(&ctor.name),
            args: ctor.args.iter().map(|t| self.lower_type_expr(t)).collect(),
            span: ctor.span.clone(),
        }
    }

    fn lower_type_decl(&mut self, ty: &TypeDecl) -> ArenaTypeDecl {
        ArenaTypeDecl {
            decorators: ty
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&ty.name),
            params: ty.params.iter().map(SpannedSymbol::from).collect(),
            constructors: ty
                .constructors
                .iter()
                .map(|c| self.lower_type_ctor(c))
                .collect(),
            span: ty.span.clone(),
        }
    }

    fn lower_type_alias(&mut self, alias: &TypeAlias) -> ArenaTypeAlias {
        ArenaTypeAlias {
            decorators: alias
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&alias.name),
            params: alias.params.iter().map(SpannedSymbol::from).collect(),
            aliased: self.lower_type_expr(&alias.aliased),
            span: alias.span.clone(),
        }
    }

    fn lower_type_var_constraint(
        &mut self,
        constraint: &TypeVarConstraint,
    ) -> ArenaTypeVarConstraint {
        ArenaTypeVarConstraint {
            var: SpannedSymbol::from(&constraint.var),
            class: SpannedSymbol::from(&constraint.class),
            span: constraint.span.clone(),
        }
    }

    fn lower_class_member(&mut self, member: &ClassMember) -> ArenaClassMember {
        ArenaClassMember {
            name: SpannedSymbol::from(&member.name),
            ty: self.lower_type_expr(&member.ty),
            span: member.span.clone(),
        }
    }

    fn lower_class_decl(&mut self, class_decl: &ClassDecl) -> ArenaClassDecl {
        ArenaClassDecl {
            decorators: class_decl
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&class_decl.name),
            params: class_decl
                .params
                .iter()
                .map(|t| self.lower_type_expr(t))
                .collect(),
            constraints: class_decl
                .constraints
                .iter()
                .map(|c| self.lower_type_var_constraint(c))
                .collect(),
            supers: class_decl
                .supers
                .iter()
                .map(|t| self.lower_type_expr(t))
                .collect(),
            members: class_decl
                .members
                .iter()
                .map(|m| self.lower_class_member(m))
                .collect(),
            span: class_decl.span.clone(),
        }
    }

    fn lower_instance_decl(&mut self, instance_decl: &InstanceDecl) -> ArenaInstanceDecl {
        ArenaInstanceDecl {
            decorators: instance_decl
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&instance_decl.name),
            params: instance_decl
                .params
                .iter()
                .map(|t| self.lower_type_expr(t))
                .collect(),
            defs: instance_decl
                .defs
                .iter()
                .map(|d| self.lower_def(d))
                .collect(),
            span: instance_decl.span.clone(),
        }
    }

    fn lower_domain_item(&mut self, item: &DomainItem) -> ArenaDomainItem {
        match item {
            DomainItem::TypeAlias(ty) => ArenaDomainItem::TypeAlias(self.lower_type_decl(ty)),
            DomainItem::TypeSig(sig) => ArenaDomainItem::TypeSig(self.lower_type_sig(sig)),
            DomainItem::Def(def) => ArenaDomainItem::Def(self.lower_def(def)),
            DomainItem::LiteralDef(def) => ArenaDomainItem::LiteralDef(self.lower_def(def)),
        }
    }

    fn lower_domain_decl(&mut self, domain_decl: &DomainDecl) -> ArenaDomainDecl {
        ArenaDomainDecl {
            decorators: domain_decl
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&domain_decl.name),
            over: self.lower_type_expr(&domain_decl.over),
            items: domain_decl
                .items
                .iter()
                .map(|item| self.lower_domain_item(item))
                .collect(),
            span: domain_decl.span.clone(),
        }
    }

    fn lower_machine_state(&mut self, state: &MachineState) -> ArenaMachineState {
        ArenaMachineState {
            name: SpannedSymbol::from(&state.name),
            fields: state
                .fields
                .iter()
                .map(|(name, ty)| (SpannedSymbol::from(name), self.lower_type_expr(ty)))
                .collect(),
            span: state.span.clone(),
        }
    }

    fn lower_machine_transition(
        &mut self,
        transition: &MachineTransition,
    ) -> ArenaMachineTransition {
        ArenaMachineTransition {
            source: SpannedSymbol::from(&transition.source),
            target: SpannedSymbol::from(&transition.target),
            name: SpannedSymbol::from(&transition.name),
            payload: transition
                .payload
                .iter()
                .map(|(name, ty)| (SpannedSymbol::from(name), self.lower_type_expr(ty)))
                .collect(),
            span: transition.span.clone(),
        }
    }

    fn lower_machine_decl(&mut self, machine_decl: &MachineDecl) -> ArenaMachineDecl {
        ArenaMachineDecl {
            decorators: machine_decl
                .decorators
                .iter()
                .map(|d| self.lower_decorator(d))
                .collect(),
            name: SpannedSymbol::from(&machine_decl.name),
            states: machine_decl
                .states
                .iter()
                .map(|state| self.lower_machine_state(state))
                .collect(),
            transitions: machine_decl
                .transitions
                .iter()
                .map(|transition| self.lower_machine_transition(transition))
                .collect(),
            span: machine_decl.span.clone(),
        }
    }

    fn lower_decorator(&mut self, decorator: &Decorator) -> ArenaDecorator {
        ArenaDecorator {
            name: SpannedSymbol::from(&decorator.name),
            arg: decorator.arg.as_ref().map(|arg| self.lower_expr(arg)),
            span: decorator.span.clone(),
        }
    }

    fn lower_export_item(&mut self, item: &ExportItem) -> ArenaScopeItem {
        ArenaScopeItem {
            kind: item.kind,
            name: SpannedSymbol::from(&item.name),
        }
    }

    fn lower_use_item(&mut self, item: &UseItem) -> ArenaScopeItem {
        ArenaScopeItem {
            kind: item.kind,
            name: SpannedSymbol::from(&item.name),
        }
    }

    fn lower_use_decl(&mut self, decl: &UseDecl) -> ArenaUseDecl {
        ArenaUseDecl {
            module: SpannedSymbol::from(&decl.module),
            items: decl.items.iter().map(|x| self.lower_use_item(x)).collect(),
            span: decl.span.clone(),
            wildcard: decl.wildcard,
            alias: decl.alias.as_ref().map(SpannedSymbol::from),
        }
    }

    fn lower_literal(&mut self, literal: &Literal) -> ArenaLiteral {
        match literal {
            Literal::Number { text, span } => ArenaLiteral::Number {
                text: Symbol::intern(text),
                span: span.clone(),
            },
            Literal::String { text, span } => ArenaLiteral::String {
                text: Symbol::intern(text),
                span: span.clone(),
            },
            Literal::Sigil {
                tag,
                body,
                flags,
                span,
            } => ArenaLiteral::Sigil {
                tag: Symbol::intern(tag),
                body: Symbol::intern(body),
                flags: Symbol::intern(flags),
                span: span.clone(),
            },
            Literal::Bool { value, span } => ArenaLiteral::Bool {
                value: *value,
                span: span.clone(),
            },
            Literal::DateTime { text, span } => ArenaLiteral::DateTime {
                text: Symbol::intern(text),
                span: span.clone(),
            },
        }
    }

    fn lower_text_part(&mut self, part: &TextPart) -> ArenaTextPart {
        match part {
            TextPart::Text { text, span } => ArenaTextPart::Text {
                text: Symbol::intern(text),
                span: span.clone(),
            },
            TextPart::Expr { expr, span } => ArenaTextPart::Expr {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> ExprId {
        let lowered = match expr {
            Expr::Ident(name) => ArenaExpr::Ident(SpannedSymbol::from(name)),
            Expr::Literal(lit) => ArenaExpr::Literal(self.lower_literal(lit)),
            Expr::UnaryNeg { expr, span } => ArenaExpr::UnaryNeg {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            Expr::Suffixed { base, suffix, span } => ArenaExpr::Suffixed {
                base: self.lower_expr(base),
                suffix: SpannedSymbol::from(suffix),
                span: span.clone(),
            },
            Expr::TextInterpolate { parts, span } => ArenaExpr::TextInterpolate {
                parts: parts.iter().map(|p| self.lower_text_part(p)).collect(),
                span: span.clone(),
            },
            Expr::List { items, span } => ArenaExpr::List {
                items: items.iter().map(|x| self.lower_list_item(x)).collect(),
                span: span.clone(),
            },
            Expr::Tuple { items, span } => ArenaExpr::Tuple {
                items: items.iter().map(|x| self.lower_expr(x)).collect(),
                span: span.clone(),
            },
            Expr::Record { fields, span } => ArenaExpr::Record {
                fields: fields.iter().map(|x| self.lower_record_field(x)).collect(),
                span: span.clone(),
            },
            Expr::PatchLit { fields, span } => ArenaExpr::PatchLit {
                fields: fields.iter().map(|x| self.lower_record_field(x)).collect(),
                span: span.clone(),
            },
            Expr::FieldAccess { base, field, span } => ArenaExpr::FieldAccess {
                base: self.lower_expr(base),
                field: SpannedSymbol::from(field),
                span: span.clone(),
            },
            Expr::FieldSection { field, span } => ArenaExpr::FieldSection {
                field: SpannedSymbol::from(field),
                span: span.clone(),
            },
            Expr::Index { base, index, span } => ArenaExpr::Index {
                base: self.lower_expr(base),
                index: self.lower_expr(index),
                span: span.clone(),
            },
            Expr::Call { func, args, span } => ArenaExpr::Call {
                func: self.lower_expr(func),
                args: args.iter().map(|x| self.lower_expr(x)).collect(),
                span: span.clone(),
            },
            Expr::Lambda { params, body, span } => ArenaExpr::Lambda {
                params: params.iter().map(|p| self.lower_pattern(p)).collect(),
                body: self.lower_expr(body),
                span: span.clone(),
            },
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => ArenaExpr::Match {
                scrutinee: scrutinee.as_ref().map(|x| self.lower_expr(x)),
                arms: arms.iter().map(|x| self.lower_match_arm(x)).collect(),
                span: span.clone(),
            },
            Expr::If {
                cond,
                then_branch,
                else_branch,
                span,
            } => ArenaExpr::If {
                cond: self.lower_expr(cond),
                then_branch: self.lower_expr(then_branch),
                else_branch: self.lower_expr(else_branch),
                span: span.clone(),
            },
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => ArenaExpr::Binary {
                op: Symbol::intern(op),
                left: self.lower_expr(left),
                right: self.lower_expr(right),
                span: span.clone(),
            },
            Expr::Block { kind, items, span } => ArenaExpr::Block {
                kind: self.lower_block_kind(kind),
                items: items.iter().map(|x| self.lower_block_item(x)).collect(),
                span: span.clone(),
            },
            Expr::Raw { text, span } => ArenaExpr::Raw {
                text: Symbol::intern(text),
                span: span.clone(),
            },
            Expr::Mock { body, .. } => {
                // ArenaExpr has no Mock variant; lower the body only.
                return self.lower_expr(body);
            }
        };
        self.arena.alloc_expr(lowered)
    }

    fn lower_match_arm(&mut self, arm: &MatchArm) -> ArenaMatchArm {
        ArenaMatchArm {
            pattern: self.lower_pattern(&arm.pattern),
            guard: arm.guard.as_ref().map(|g| self.lower_expr(g)),
            body: self.lower_expr(&arm.body),
            span: arm.span.clone(),
        }
    }

    fn lower_block_kind(&mut self, kind: &BlockKind) -> ArenaBlockKind {
        match kind {
            BlockKind::Plain => ArenaBlockKind::Plain,
            BlockKind::Do { monad } => ArenaBlockKind::Do {
                monad: SpannedSymbol::from(monad),
            },
            BlockKind::Generate => ArenaBlockKind::Generate,
            BlockKind::Resource => ArenaBlockKind::Resource,
        }
    }

    fn lower_block_item(&mut self, item: &BlockItem) -> ArenaBlockItem {
        match item {
            BlockItem::Bind {
                pattern,
                expr,
                span,
            } => ArenaBlockItem::Bind {
                pattern: self.lower_pattern(pattern),
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::Let {
                pattern,
                expr,
                span,
            } => ArenaBlockItem::Let {
                pattern: self.lower_pattern(pattern),
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::Filter { expr, span } => ArenaBlockItem::Filter {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::Yield { expr, span } => ArenaBlockItem::Yield {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::Recurse { expr, span } => ArenaBlockItem::Recurse {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::Expr { expr, span } => ArenaBlockItem::Expr {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
            BlockItem::When { cond, effect, span } => ArenaBlockItem::When {
                cond: self.lower_expr(cond),
                effect: self.lower_expr(effect),
                span: span.clone(),
            },
            BlockItem::Unless { cond, effect, span } => ArenaBlockItem::Unless {
                cond: self.lower_expr(cond),
                effect: self.lower_expr(effect),
                span: span.clone(),
            },
            BlockItem::Given {
                cond,
                fail_expr,
                span,
            } => ArenaBlockItem::Given {
                cond: self.lower_expr(cond),
                fail_expr: self.lower_expr(fail_expr),
                span: span.clone(),
            },
            BlockItem::On {
                transition,
                handler,
                span,
            } => ArenaBlockItem::On {
                transition: self.lower_expr(transition),
                handler: self.lower_expr(handler),
                span: span.clone(),
            },
        }
    }

    fn lower_list_item(&mut self, item: &ListItem) -> ArenaListItem {
        ArenaListItem {
            expr: self.lower_expr(&item.expr),
            spread: item.spread,
            span: item.span.clone(),
        }
    }

    fn lower_path_segment(&mut self, segment: &PathSegment) -> ArenaPathSegment {
        match segment {
            PathSegment::Field(name) => ArenaPathSegment::Field(SpannedSymbol::from(name)),
            PathSegment::Index(expr, span) => {
                ArenaPathSegment::Index(self.lower_expr(expr), span.clone())
            }
            PathSegment::All(span) => ArenaPathSegment::All(span.clone()),
        }
    }

    fn lower_record_field(&mut self, field: &RecordField) -> ArenaRecordField {
        ArenaRecordField {
            spread: field.spread,
            path: field
                .path
                .iter()
                .map(|x| self.lower_path_segment(x))
                .collect(),
            value: self.lower_expr(&field.value),
            span: field.span.clone(),
        }
    }

    fn lower_pattern(&mut self, pattern: &Pattern) -> PatternId {
        let lowered = match pattern {
            Pattern::Wildcard(span) => ArenaPattern::Wildcard(span.clone()),
            Pattern::Ident(name) => ArenaPattern::Ident(SpannedSymbol::from(name)),
            Pattern::SubjectIdent(name) => ArenaPattern::SubjectIdent(SpannedSymbol::from(name)),
            Pattern::Literal(lit) => ArenaPattern::Literal(self.lower_literal(lit)),
            Pattern::At {
                name,
                pattern,
                subject,
                span,
            } => ArenaPattern::At {
                name: SpannedSymbol::from(name),
                pattern: self.lower_pattern(pattern),
                subject: *subject,
                span: span.clone(),
            },
            Pattern::Constructor { name, args, span } => ArenaPattern::Constructor {
                name: SpannedSymbol::from(name),
                args: args.iter().map(|x| self.lower_pattern(x)).collect(),
                span: span.clone(),
            },
            Pattern::Tuple { items, span } => ArenaPattern::Tuple {
                items: items.iter().map(|x| self.lower_pattern(x)).collect(),
                span: span.clone(),
            },
            Pattern::List { items, rest, span } => ArenaPattern::List {
                items: items.iter().map(|x| self.lower_pattern(x)).collect(),
                rest: rest.as_ref().map(|r| self.lower_pattern(r)),
                span: span.clone(),
            },
            Pattern::Record { fields, span } => ArenaPattern::Record {
                fields: fields
                    .iter()
                    .map(|x| self.lower_record_pattern_field(x))
                    .collect(),
                span: span.clone(),
            },
        };
        self.arena.alloc_pattern(lowered)
    }

    fn lower_record_pattern_field(
        &mut self,
        field: &RecordPatternField,
    ) -> ArenaRecordPatternField {
        ArenaRecordPatternField {
            path: field.path.iter().map(SpannedSymbol::from).collect(),
            pattern: self.lower_pattern(&field.pattern),
            span: field.span.clone(),
        }
    }

    fn lower_type_expr(&mut self, ty: &TypeExpr) -> TypeExprId {
        let lowered = match ty {
            TypeExpr::Name(name) => ArenaTypeExpr::Name(SpannedSymbol::from(name)),
            TypeExpr::And { items, span } => ArenaTypeExpr::And {
                items: items.iter().map(|x| self.lower_type_expr(x)).collect(),
                span: span.clone(),
            },
            TypeExpr::Apply { base, args, span } => ArenaTypeExpr::Apply {
                base: self.lower_type_expr(base),
                args: args.iter().map(|x| self.lower_type_expr(x)).collect(),
                span: span.clone(),
            },
            TypeExpr::Func {
                params,
                result,
                span,
            } => ArenaTypeExpr::Func {
                params: params.iter().map(|x| self.lower_type_expr(x)).collect(),
                result: self.lower_type_expr(result),
                span: span.clone(),
            },
            TypeExpr::Record { fields, span } => ArenaTypeExpr::Record {
                fields: fields
                    .iter()
                    .map(|(name, item)| (SpannedSymbol::from(name), self.lower_type_expr(item)))
                    .collect(),
                span: span.clone(),
            },
            TypeExpr::Tuple { items, span } => ArenaTypeExpr::Tuple {
                items: items.iter().map(|x| self.lower_type_expr(x)).collect(),
                span: span.clone(),
            },
            TypeExpr::Star { span } => ArenaTypeExpr::Star { span: span.clone() },
            TypeExpr::Unknown { span } => ArenaTypeExpr::Unknown { span: span.clone() },
        };
        self.arena.alloc_type_expr(lowered)
    }
}
