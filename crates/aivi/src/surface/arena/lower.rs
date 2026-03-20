use crate::intern::{ExprId, PatternId, Symbol, TypeExprId};
use crate::surface::{
    BlockItem, BlockKind, ClassDecl, ClassMember, Decorator, Def, DomainDecl, DomainItem,
    ExportItem, Expr, FlowArm, FlowBinding, FlowGuard, FlowLine, FlowModifier, FlowStep,
    FlowStepKind, InstanceDecl, ListItem, Literal, MatchArm, Module, ModuleItem, PathSegment,
    Pattern, RecordField, RecordPatternField, RecordTypeField, TextPart, TypeAlias, TypeCtor,
    TypeDecl, TypeExpr, TypeSig, TypeVarConstraint, UseDecl, UseItem,
};

use super::*;

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
            opaque: ty.opaque,
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
            opaque: alias.opaque,
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
            alias: None,
        }
    }

    fn lower_use_item(&mut self, item: &UseItem) -> ArenaScopeItem {
        ArenaScopeItem {
            kind: item.kind,
            name: SpannedSymbol::from(&item.name),
            alias: item.alias.as_ref().map(SpannedSymbol::from),
        }
    }

    fn lower_use_decl(&mut self, decl: &UseDecl) -> ArenaUseDecl {
        ArenaUseDecl {
            module: SpannedSymbol::from(&decl.module),
            items: decl.items.iter().map(|x| self.lower_use_item(x)).collect(),
            span: decl.span.clone(),
            wildcard: decl.wildcard,
            hiding: decl.hiding,
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
            Expr::Flow { root, lines, span } => ArenaExpr::Flow {
                root: self.lower_expr(root),
                lines: lines
                    .iter()
                    .map(|line| self.lower_flow_line(line))
                    .collect(),
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

    fn lower_flow_binding(&mut self, binding: &FlowBinding) -> ArenaFlowBinding {
        ArenaFlowBinding {
            name: SpannedSymbol::from(&binding.name),
            span: binding.span.clone(),
        }
    }

    fn lower_flow_modifier(&mut self, modifier: &FlowModifier) -> ArenaFlowModifier {
        match modifier {
            FlowModifier::Timeout { duration, span } => ArenaFlowModifier::Timeout {
                duration: self.lower_expr(duration),
                span: span.clone(),
            },
            FlowModifier::Delay { duration, span } => ArenaFlowModifier::Delay {
                duration: self.lower_expr(duration),
                span: span.clone(),
            },
            FlowModifier::Concurrent { limit, span } => ArenaFlowModifier::Concurrent {
                limit: self.lower_expr(limit),
                span: span.clone(),
            },
            FlowModifier::Retry {
                attempts,
                interval,
                exponential,
                span,
            } => ArenaFlowModifier::Retry {
                attempts: *attempts,
                interval: self.lower_expr(interval),
                exponential: *exponential,
                span: span.clone(),
            },
            FlowModifier::Cleanup { expr, span } => ArenaFlowModifier::Cleanup {
                expr: self.lower_expr(expr),
                span: span.clone(),
            },
        }
    }

    fn lower_flow_step_kind(&mut self, kind: FlowStepKind) -> ArenaFlowStepKind {
        match kind {
            FlowStepKind::Flow => ArenaFlowStepKind::Flow,
            FlowStepKind::Tap => ArenaFlowStepKind::Tap,
            FlowStepKind::Attempt => ArenaFlowStepKind::Attempt,
            FlowStepKind::FanOut => ArenaFlowStepKind::FanOut,
            FlowStepKind::Applicative => ArenaFlowStepKind::Applicative,
        }
    }

    fn lower_flow_step(&mut self, step: &FlowStep) -> ArenaFlowStep {
        ArenaFlowStep {
            kind: self.lower_flow_step_kind(step.kind),
            expr: self.lower_expr(&step.expr),
            modifiers: step
                .modifiers
                .iter()
                .map(|modifier| self.lower_flow_modifier(modifier))
                .collect(),
            binding: step
                .binding
                .as_ref()
                .map(|binding| self.lower_flow_binding(binding)),
            subflow: step
                .subflow
                .iter()
                .map(|line| self.lower_flow_line(line))
                .collect(),
            span: step.span.clone(),
        }
    }

    fn lower_flow_guard(&mut self, guard: &FlowGuard) -> ArenaFlowGuard {
        ArenaFlowGuard {
            predicate: self.lower_expr(&guard.predicate),
            fail_expr: guard.fail_expr.as_ref().map(|expr| self.lower_expr(expr)),
            span: guard.span.clone(),
        }
    }

    fn lower_flow_arm(&mut self, arm: &FlowArm) -> ArenaFlowArm {
        ArenaFlowArm {
            pattern: self.lower_pattern(&arm.pattern),
            guard: arm.guard.as_ref().map(|expr| self.lower_expr(expr)),
            guard_negated: arm.guard_negated,
            body: self.lower_expr(&arm.body),
            span: arm.span.clone(),
        }
    }

    fn lower_flow_line(&mut self, line: &FlowLine) -> ArenaFlowLine {
        match line {
            FlowLine::Step(step) => ArenaFlowLine::Step(self.lower_flow_step(step)),
            FlowLine::Guard(guard) => ArenaFlowLine::Guard(self.lower_flow_guard(guard)),
            FlowLine::Branch(arm) => ArenaFlowLine::Branch(self.lower_flow_arm(arm)),
            FlowLine::Recover(arm) => ArenaFlowLine::Recover(self.lower_flow_arm(arm)),
            FlowLine::Anchor(anchor) => ArenaFlowLine::Anchor(ArenaFlowAnchor {
                name: SpannedSymbol::from(&anchor.name),
                span: anchor.span.clone(),
            }),
        }
    }

    fn lower_match_arm(&mut self, arm: &MatchArm) -> ArenaMatchArm {
        ArenaMatchArm {
            pattern: self.lower_pattern(&arm.pattern),
            guard: arm.guard.as_ref().map(|g| self.lower_expr(g)),
            guard_negated: arm.guard_negated,
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
            BlockKind::Managed => ArenaBlockKind::Managed,
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
                    .map(|field| match field {
                        RecordTypeField::Named { name, ty } => ArenaRecordTypeField::Named {
                            name: SpannedSymbol::from(name),
                            ty: self.lower_type_expr(ty),
                        },
                        RecordTypeField::Spread { ty, span } => ArenaRecordTypeField::Spread {
                            ty: self.lower_type_expr(ty),
                            span: span.clone(),
                        },
                    })
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
