use serde::{Deserialize, Serialize};

use crate::surface::{BlockItem, BlockKind, Def, DomainItem, Expr, Module, ModuleItem, Pattern};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirProgram {
    pub modules: Vec<HirModule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirModule {
    pub name: String,
    pub defs: Vec<HirDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirDef {
    pub name: String,
    pub expr: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "kind")]
pub enum HirExpr {
    Var {
        id: u32,
        name: String,
    },
    LitNumber {
        id: u32,
        text: String,
    },
    LitString {
        id: u32,
        text: String,
    },
    LitSigil {
        id: u32,
        tag: String,
        body: String,
        flags: String,
    },
    LitBool {
        id: u32,
        value: bool,
    },
    LitDateTime {
        id: u32,
        text: String,
    },
    Lambda {
        id: u32,
        param: String,
        body: Box<HirExpr>,
    },
    App {
        id: u32,
        func: Box<HirExpr>,
        arg: Box<HirExpr>,
    },
    Call {
        id: u32,
        func: Box<HirExpr>,
        args: Vec<HirExpr>,
    },
    List {
        id: u32,
        items: Vec<HirListItem>,
    },
    Tuple {
        id: u32,
        items: Vec<HirExpr>,
    },
    Record {
        id: u32,
        fields: Vec<HirRecordField>,
    },
    Patch {
        id: u32,
        target: Box<HirExpr>,
        fields: Vec<HirRecordField>,
    },
    FieldAccess {
        id: u32,
        base: Box<HirExpr>,
        field: String,
    },
    Index {
        id: u32,
        base: Box<HirExpr>,
        index: Box<HirExpr>,
    },
    Match {
        id: u32,
        scrutinee: Box<HirExpr>,
        arms: Vec<HirMatchArm>,
    },
    If {
        id: u32,
        cond: Box<HirExpr>,
        then_branch: Box<HirExpr>,
        else_branch: Box<HirExpr>,
    },
    Binary {
        id: u32,
        op: String,
        left: Box<HirExpr>,
        right: Box<HirExpr>,
    },
    Block {
        id: u32,
        block_kind: HirBlockKind,
        items: Vec<HirBlockItem>,
    },
    Raw {
        id: u32,
        text: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirListItem {
    pub expr: HirExpr,
    pub spread: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirRecordField {
    pub path: Vec<HirPathSegment>,
    pub value: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirPathSegment {
    Field(String),
    Index(HirExpr),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirMatchArm {
    pub pattern: HirPattern,
    pub guard: Option<HirExpr>,
    pub body: HirExpr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirPattern {
    Wildcard {
        id: u32,
    },
    Var {
        id: u32,
        name: String,
    },
    Literal {
        id: u32,
        value: HirLiteral,
    },
    Constructor {
        id: u32,
        name: String,
        args: Vec<HirPattern>,
    },
    Tuple {
        id: u32,
        items: Vec<HirPattern>,
    },
    List {
        id: u32,
        items: Vec<HirPattern>,
        rest: Option<Box<HirPattern>>,
    },
    Record {
        id: u32,
        fields: Vec<HirRecordPatternField>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HirRecordPatternField {
    pub path: Vec<String>,
    pub pattern: HirPattern,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirLiteral {
    Number(String),
    String(String),
    Sigil {
        tag: String,
        body: String,
        flags: String,
    },
    Bool(bool),
    DateTime(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirBlockKind {
    Plain,
    Effect,
    Generate,
    Resource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum HirBlockItem {
    Bind { pattern: HirPattern, expr: HirExpr },
    Filter { expr: HirExpr },
    Yield { expr: HirExpr },
    Recurse { expr: HirExpr },
    Expr { expr: HirExpr },
}

pub fn desugar_modules(modules: &[Module]) -> HirProgram {
    let mut id_gen = IdGen::default();
    let mut hir_modules = Vec::new();
    for module in modules {
        let defs = collect_defs(module)
            .into_iter()
            .map(|(name, expr)| HirDef {
                name,
                expr: lower_expr(expr, &mut id_gen),
            })
            .collect();
        hir_modules.push(HirModule {
            name: module.name.name.clone(),
            defs,
        });
    }
    HirProgram {
        modules: hir_modules,
    }
}

fn collect_defs(module: &Module) -> Vec<(String, Expr)> {
    let mut defs = Vec::new();
    for item in &module.items {
        match item {
            ModuleItem::Def(def) => defs.push((def.name.name.clone(), def_expr(def))),
            ModuleItem::InstanceDecl(instance) => {
                for def in &instance.defs {
                    defs.push((def.name.name.clone(), def_expr(def)));
                }
            }
            ModuleItem::DomainDecl(domain) => {
                for domain_item in &domain.items {
                    match domain_item {
                        DomainItem::Def(def) | DomainItem::LiteralDef(def) => {
                            defs.push((def.name.name.clone(), def_expr(def)));
                        }
                        DomainItem::TypeAlias(_) | DomainItem::TypeSig(_) => {}
                    }
                }
            }
            ModuleItem::TypeAlias(_) => {}
            _ => {}
        }
    }
    defs
}

fn def_expr(def: &Def) -> Expr {
    if def.params.is_empty() {
        def.expr.clone()
    } else {
        Expr::Lambda {
            params: def.params.clone(),
            body: Box::new(def.expr.clone()),
            span: def.span.clone(),
        }
    }
}

fn lower_expr(expr: Expr, id_gen: &mut IdGen) -> HirExpr {
    if let Expr::Binary {
        op, left, right, ..
    } = &expr
    {
        if op == "<|" && matches!(**right, Expr::Record { .. }) && !contains_hole(left) {
            return lower_expr_inner(expr, id_gen);
        }
    }
    if contains_hole(&expr) {
        let (rewritten, params) = replace_holes(expr);
        let mut hir = lower_expr_inner(rewritten, id_gen);
        for param in params.into_iter().rev() {
            hir = HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(hir),
            };
        }
        return hir;
    }
    lower_expr_inner(expr, id_gen)
}

fn lower_expr_inner(expr: Expr, id_gen: &mut IdGen) -> HirExpr {
    match expr {
        Expr::Ident(name) => HirExpr::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Expr::Literal(literal) => match literal {
            crate::surface::Literal::Number { text, .. } => {
                fn split_suffixed(text: &str) -> Option<(String, String)> {
                    let mut chars = text.chars().peekable();
                    let mut number = String::new();
                    if matches!(chars.peek(), Some('-')) {
                        number.push('-');
                        chars.next();
                    }
                    let mut saw_digit = false;
                    let mut saw_dot = false;
                    while let Some(&ch) = chars.peek() {
                        if ch.is_ascii_digit() {
                            saw_digit = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        if ch == '.' && !saw_dot {
                            saw_dot = true;
                            number.push(ch);
                            chars.next();
                            continue;
                        }
                        break;
                    }
                    if !saw_digit {
                        return None;
                    }
                    let suffix: String = chars.collect();
                    if suffix.is_empty() {
                        return None;
                    }
                    if !suffix
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                    {
                        return None;
                    }
                    Some((number, suffix))
                }

                if let Some((number, suffix)) = split_suffixed(&text) {
                    let template_name = format!("1{suffix}");
                    return HirExpr::App {
                        id: id_gen.next(),
                        func: Box::new(HirExpr::Var {
                            id: id_gen.next(),
                            name: template_name,
                        }),
                        arg: Box::new(HirExpr::LitNumber {
                            id: id_gen.next(),
                            text: number,
                        }),
                    };
                }

                HirExpr::LitNumber {
                    id: id_gen.next(),
                    text,
                }
            }
            crate::surface::Literal::String { text, .. } => HirExpr::LitString {
                id: id_gen.next(),
                text,
            },
            crate::surface::Literal::Sigil {
                tag, body, flags, ..
            } => HirExpr::LitSigil {
                id: id_gen.next(),
                tag,
                body,
                flags,
            },
            crate::surface::Literal::Bool { value, .. } => HirExpr::LitBool {
                id: id_gen.next(),
                value,
            },
            crate::surface::Literal::DateTime { text, .. } => HirExpr::LitDateTime {
                id: id_gen.next(),
                text,
            },
        },
        Expr::List { items, .. } => HirExpr::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| HirListItem {
                    expr: lower_expr(item.expr, id_gen),
                    spread: item.spread,
                })
                .collect(),
        },
        Expr::Tuple { items, .. } => HirExpr::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_expr(item, id_gen))
                .collect(),
        },
        Expr::Record { fields, .. } => HirExpr::Record {
            id: id_gen.next(),
            fields: fields
                .into_iter()
                .map(|field| HirRecordField {
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                HirPathSegment::Field(name.name)
                            }
                            crate::surface::PathSegment::Index(expr, _) => {
                                HirPathSegment::Index(lower_expr(expr, id_gen))
                            }
                        })
                        .collect(),
                    value: lower_expr(field.value, id_gen),
                })
                .collect(),
        },
        Expr::FieldAccess { base, field, .. } => HirExpr::FieldAccess {
            id: id_gen.next(),
            base: Box::new(lower_expr(*base, id_gen)),
            field: field.name,
        },
        Expr::FieldSection { field, .. } => {
            let param = "_arg0".to_string();
            let var = HirExpr::Var {
                id: id_gen.next(),
                name: param.clone(),
            };
            let body = HirExpr::FieldAccess {
                id: id_gen.next(),
                base: Box::new(var),
                field: field.name,
            };
            HirExpr::Lambda {
                id: id_gen.next(),
                param,
                body: Box::new(body),
            }
        }
        Expr::Index { base, index, .. } => HirExpr::Index {
            id: id_gen.next(),
            base: Box::new(lower_expr(*base, id_gen)),
            index: Box::new(lower_expr(*index, id_gen)),
        },
        Expr::Call { func, args, .. } => HirExpr::Call {
            id: id_gen.next(),
            func: Box::new(lower_expr(*func, id_gen)),
            args: args
                .into_iter()
                .map(|arg| lower_expr(arg, id_gen))
                .collect(),
        },
        Expr::Lambda { params, body, .. } => lower_lambda(params, *body, id_gen),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            let scrutinee = if let Some(scrutinee) = scrutinee {
                lower_expr(*scrutinee, id_gen)
            } else {
                let param = "_arg0".to_string();
                let var = HirExpr::Var {
                    id: id_gen.next(),
                    name: param.clone(),
                };
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(var),
                    arms: arms
                        .into_iter()
                        .map(|arm| HirMatchArm {
                            pattern: lower_pattern(arm.pattern, id_gen),
                            guard: arm.guard.map(|guard| lower_expr(guard, id_gen)),
                            body: lower_expr(arm.body, id_gen),
                        })
                        .collect(),
                };
                return HirExpr::Lambda {
                    id: id_gen.next(),
                    param,
                    body: Box::new(match_expr),
                };
            };
            HirExpr::Match {
                id: id_gen.next(),
                scrutinee: Box::new(scrutinee),
                arms: arms
                    .into_iter()
                    .map(|arm| HirMatchArm {
                        pattern: lower_pattern(arm.pattern, id_gen),
                        guard: arm.guard.map(|guard| lower_expr(guard, id_gen)),
                        body: lower_expr(arm.body, id_gen),
                    })
                    .collect(),
            }
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => HirExpr::If {
            id: id_gen.next(),
            cond: Box::new(lower_expr(*cond, id_gen)),
            then_branch: Box::new(lower_expr(*then_branch, id_gen)),
            else_branch: Box::new(lower_expr(*else_branch, id_gen)),
        },
        Expr::Binary {
            op, left, right, ..
        } => {
            if op == "|>" {
                let left = lower_expr(*left, id_gen);
                let right = lower_expr(*right, id_gen);
                return HirExpr::App {
                    id: id_gen.next(),
                    func: Box::new(right),
                    arg: Box::new(left),
                };
            }
            if op == "<|" {
                if let Expr::Record { fields, .. } = *right.clone() {
                    return HirExpr::Patch {
                        id: id_gen.next(),
                        target: Box::new(lower_expr(*left, id_gen)),
                        fields: fields
                            .into_iter()
                            .map(|field| HirRecordField {
                                path: field
                                    .path
                                    .into_iter()
                                    .map(|segment| match segment {
                                        crate::surface::PathSegment::Field(name) => {
                                            HirPathSegment::Field(name.name)
                                        }
                                        crate::surface::PathSegment::Index(expr, _) => {
                                            HirPathSegment::Index(lower_expr(expr, id_gen))
                                        }
                                    })
                                    .collect(),
                                value: lower_expr(field.value, id_gen),
                            })
                            .collect(),
                    };
                }
            }
            HirExpr::Binary {
                id: id_gen.next(),
                op,
                left: Box::new(lower_expr(*left, id_gen)),
                right: Box::new(lower_expr(*right, id_gen)),
            }
        }
        Expr::Block { kind, items, .. } => HirExpr::Block {
            id: id_gen.next(),
            block_kind: lower_block_kind(kind),
            items: items
                .into_iter()
                .map(|item| lower_block_item(item, id_gen))
                .collect(),
        },
        Expr::Raw { text, .. } => HirExpr::Raw {
            id: id_gen.next(),
            text,
        },
    }
}

fn lower_lambda(params: Vec<Pattern>, body: Expr, id_gen: &mut IdGen) -> HirExpr {
    let mut acc = lower_expr(body, id_gen);
    for (index, param) in params.into_iter().rev().enumerate() {
        match param {
            Pattern::Ident(name) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: name.name,
                    body: Box::new(acc),
                };
            }
            Pattern::Wildcard(_) => {
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: format!("_arg{}", index),
                    body: Box::new(acc),
                };
            }
            other => {
                let param_name = format!("_arg{}", index);
                let match_expr = HirExpr::Match {
                    id: id_gen.next(),
                    scrutinee: Box::new(HirExpr::Var {
                        id: id_gen.next(),
                        name: param_name.clone(),
                    }),
                    arms: vec![HirMatchArm {
                        pattern: lower_pattern(other, id_gen),
                        guard: None,
                        body: acc,
                    }],
                };
                acc = HirExpr::Lambda {
                    id: id_gen.next(),
                    param: param_name,
                    body: Box::new(match_expr),
                };
            }
        }
    }
    acc
}

fn lower_block_kind(kind: BlockKind) -> HirBlockKind {
    match kind {
        BlockKind::Plain => HirBlockKind::Plain,
        BlockKind::Effect => HirBlockKind::Effect,
        BlockKind::Generate => HirBlockKind::Generate,
        BlockKind::Resource => HirBlockKind::Resource,
    }
}

fn lower_block_item(item: BlockItem, id_gen: &mut IdGen) -> HirBlockItem {
    match item {
        BlockItem::Bind { pattern, expr, .. } => HirBlockItem::Bind {
            pattern: lower_pattern(pattern, id_gen),
            expr: lower_expr(expr, id_gen),
        },
        BlockItem::Filter { expr, .. } => HirBlockItem::Filter {
            expr: lower_expr(expr, id_gen),
        },
        BlockItem::Yield { expr, .. } => HirBlockItem::Yield {
            expr: lower_expr(expr, id_gen),
        },
        BlockItem::Recurse { expr, .. } => HirBlockItem::Recurse {
            expr: lower_expr(expr, id_gen),
        },
        BlockItem::Expr { expr, .. } => HirBlockItem::Expr {
            expr: lower_expr(expr, id_gen),
        },
    }
}

fn lower_pattern(pattern: Pattern, id_gen: &mut IdGen) -> HirPattern {
    match pattern {
        Pattern::Wildcard(_) => HirPattern::Wildcard { id: id_gen.next() },
        Pattern::Ident(name) => HirPattern::Var {
            id: id_gen.next(),
            name: name.name,
        },
        Pattern::Literal(literal) => HirPattern::Literal {
            id: id_gen.next(),
            value: match literal {
                crate::surface::Literal::Number { text, .. } => HirLiteral::Number(text),
                crate::surface::Literal::String { text, .. } => HirLiteral::String(text),
                crate::surface::Literal::Sigil {
                    tag, body, flags, ..
                } => HirLiteral::Sigil { tag, body, flags },
                crate::surface::Literal::Bool { value, .. } => HirLiteral::Bool(value),
                crate::surface::Literal::DateTime { text, .. } => HirLiteral::DateTime(text),
            },
        },
        Pattern::Constructor { name, args, .. } => HirPattern::Constructor {
            id: id_gen.next(),
            name: name.name,
            args: args
                .into_iter()
                .map(|arg| lower_pattern(arg, id_gen))
                .collect(),
        },
        Pattern::Tuple { items, .. } => HirPattern::Tuple {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
        },
        Pattern::List { items, rest, .. } => HirPattern::List {
            id: id_gen.next(),
            items: items
                .into_iter()
                .map(|item| lower_pattern(item, id_gen))
                .collect(),
            rest: rest.map(|rest| Box::new(lower_pattern(*rest, id_gen))),
        },
        Pattern::Record { fields, .. } => HirPattern::Record {
            id: id_gen.next(),
            fields: fields
                .into_iter()
                .map(|field| HirRecordPatternField {
                    path: field.path.into_iter().map(|name| name.name).collect(),
                    pattern: lower_pattern(field.pattern, id_gen),
                })
                .collect(),
        },
    }
}

fn contains_hole(expr: &Expr) -> bool {
    match expr {
        Expr::Ident(name) => name.name == "_",
        Expr::Literal(_) => false,
        Expr::List { items, .. } => items.iter().any(|item| contains_hole(&item.expr)),
        Expr::Tuple { items, .. } => items.iter().any(contains_hole),
        Expr::Record { fields, .. } => fields.iter().any(|field| {
            field
                .path
                .iter()
                .any(|segment| matches!(segment, crate::surface::PathSegment::Index(expr, _) if contains_hole(expr)))
                || contains_hole(&field.value)
        }),
        Expr::FieldAccess { base, .. } => contains_hole(base),
        Expr::FieldSection { .. } => true,
        Expr::Index { base, index, .. } => contains_hole(base) || contains_hole(index),
        Expr::Call { func, args, .. } => {
            contains_hole(func) || args.iter().any(contains_hole)
        }
        Expr::Lambda { .. } => false,
        Expr::Match { scrutinee, arms, .. } => {
            scrutinee.as_deref().is_some_and(contains_hole)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(contains_hole)
                        || contains_hole(&arm.body)
                })
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => contains_hole(cond) || contains_hole(then_branch) || contains_hole(else_branch),
        Expr::Binary { left, right, .. } => contains_hole(left) || contains_hole(right),
        Expr::Block { items, .. } => items.iter().any(|item| match item {
            BlockItem::Bind { expr, .. } => contains_hole(expr),
            BlockItem::Filter { expr, .. }
            | BlockItem::Yield { expr, .. }
            | BlockItem::Recurse { expr, .. }
            | BlockItem::Expr { expr, .. } => contains_hole(expr),
        }),
        Expr::Raw { .. } => false,
    }
}

fn replace_holes(expr: Expr) -> (Expr, Vec<String>) {
    let mut counter = 0;
    let mut params = Vec::new();
    let rewritten = replace_holes_inner(expr, &mut counter, &mut params);
    (rewritten, params)
}

fn replace_holes_inner(expr: Expr, counter: &mut u32, params: &mut Vec<String>) -> Expr {
    match expr {
        Expr::Ident(name) if name.name == "_" => {
            let param = format!("_arg{}", counter);
            *counter += 1;
            params.push(param.clone());
            Expr::Ident(crate::surface::SpannedName {
                name: param,
                span: name.span,
            })
        }
        Expr::Ident(_) | Expr::Literal(_) | Expr::Raw { .. } => expr,
        Expr::List { items, span } => Expr::List {
            items: items
                .into_iter()
                .map(|item| crate::surface::ListItem {
                    expr: replace_holes_inner(item.expr, counter, params),
                    spread: item.spread,
                    span: item.span,
                })
                .collect(),
            span,
        },
        Expr::Tuple { items, span } => Expr::Tuple {
            items: items
                .into_iter()
                .map(|item| replace_holes_inner(item, counter, params))
                .collect(),
            span,
        },
        Expr::Record { fields, span } => Expr::Record {
            fields: fields
                .into_iter()
                .map(|field| crate::surface::RecordField {
                    path: field
                        .path
                        .into_iter()
                        .map(|segment| match segment {
                            crate::surface::PathSegment::Field(name) => {
                                crate::surface::PathSegment::Field(name)
                            }
                            crate::surface::PathSegment::Index(expr, span) => {
                                crate::surface::PathSegment::Index(
                                    replace_holes_inner(expr, counter, params),
                                    span,
                                )
                            }
                        })
                        .collect(),
                    value: replace_holes_inner(field.value, counter, params),
                    span: field.span,
                })
                .collect(),
            span,
        },
        Expr::FieldAccess { base, field, span } => Expr::FieldAccess {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            field,
            span,
        },
        Expr::FieldSection { .. } => expr,
        Expr::Index { base, index, span } => Expr::Index {
            base: Box::new(replace_holes_inner(*base, counter, params)),
            index: Box::new(replace_holes_inner(*index, counter, params)),
            span,
        },
        Expr::Call { func, args, span } => Expr::Call {
            func: Box::new(replace_holes_inner(*func, counter, params)),
            args: args
                .into_iter()
                .map(|arg| replace_holes_inner(arg, counter, params))
                .collect(),
            span,
        },
        Expr::Lambda {
            params: lambda_params,
            body,
            span,
        } => Expr::Lambda {
            params: lambda_params,
            body: Box::new(replace_holes_inner(*body, counter, params)),
            span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: scrutinee.map(|expr| Box::new(replace_holes_inner(*expr, counter, params))),
            arms: arms
                .into_iter()
                .map(|arm| crate::surface::MatchArm {
                    pattern: arm.pattern,
                    guard: arm
                        .guard
                        .map(|guard| replace_holes_inner(guard, counter, params)),
                    body: replace_holes_inner(arm.body, counter, params),
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
            cond: Box::new(replace_holes_inner(*cond, counter, params)),
            then_branch: Box::new(replace_holes_inner(*then_branch, counter, params)),
            else_branch: Box::new(replace_holes_inner(*else_branch, counter, params)),
            span,
        },
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(replace_holes_inner(*left, counter, params)),
            right: Box::new(replace_holes_inner(*right, counter, params)),
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
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Filter { expr, span } => BlockItem::Filter {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Yield { expr, span } => BlockItem::Yield {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Recurse { expr, span } => BlockItem::Recurse {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                    BlockItem::Expr { expr, span } => BlockItem::Expr {
                        expr: replace_holes_inner(expr, counter, params),
                        span,
                    },
                })
                .collect(),
            span,
        },
    }
}

#[derive(Default)]
struct IdGen {
    next: u32,
}

impl IdGen {
    fn next(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }
}
