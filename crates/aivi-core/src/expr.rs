use aivi_base::SourceSpan;
use aivi_hir::{
    BigIntLiteral, BinaryOperator, BindingId as HirBindingId, BuiltinTerm, DecimalLiteral,
    DomainMemberHandle, FloatLiteral, IntegerLiteral, IntrinsicValue, ItemId as HirItemId,
    PipeTransformMode, SuffixedIntegerLiteral, SumConstructorHandle, UnaryOperator,
};

use crate::{ids::ExprId, ty::Type};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub span: SourceSpan,
    pub ty: Type,
    pub kind: ExprKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    AmbientSubject,
    OptionSome {
        payload: ExprId,
    },
    OptionNone,
    Reference(Reference),
    Integer(IntegerLiteral),
    Float(FloatLiteral),
    Decimal(DecimalLiteral),
    BigInt(BigIntLiteral),
    SuffixedInteger(SuffixedIntegerLiteral),
    Text(TextLiteral),
    Tuple(Vec<ExprId>),
    List(Vec<ExprId>),
    Map(Vec<MapEntry>),
    Set(Vec<ExprId>),
    Record(Vec<RecordExprField>),
    Projection {
        base: ProjectionBase,
        path: Vec<Box<str>>,
    },
    Apply {
        callee: ExprId,
        arguments: Vec<ExprId>,
    },
    Unary {
        operator: UnaryOperator,
        expr: ExprId,
    },
    Binary {
        left: ExprId,
        operator: BinaryOperator,
        right: ExprId,
    },
    Pipe(PipeExpr),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    Local(HirBindingId),
    Item(crate::ItemId),
    HirItem(HirItemId),
    SumConstructor(SumConstructorHandle),
    DomainMember(DomainMemberHandle),
    ExecutableEvidence(ExecutableClassMember),
    BuiltinClassMember(BuiltinClassMemberIntrinsic),
    Builtin(BuiltinTerm),
    IntrinsicValue(IntrinsicValue),
}

pub type ExecutableEvidence = crate::ItemId;
pub type ExecutableClassMember = ExecutableEvidence;

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinClassMemberIntrinsic {
    StructuralEq,
    DerivedStructuralEq(std::sync::Arc<aivi_hir::EqualityShape>),
    Compare {
        subject: BuiltinOrdSubject,
        ordering_item: HirItemId,
    },
    Append(BuiltinAppendCarrier),
    Empty(BuiltinAppendCarrier),
    Map(BuiltinFunctorCarrier),
    Bimap(BuiltinBifunctorCarrier),
    Pure(BuiltinApplicativeCarrier),
    Apply(BuiltinApplyCarrier),
    Chain(BuiltinMonadCarrier),
    Join(BuiltinMonadCarrier),
    Reduce(BuiltinFoldableCarrier),
    /// Callable ABI: Applicative `pure`, superclass `apply` and `map`, then
    /// the mapper and source carrier. The result constructor stays abstract.
    Traverse {
        traversable: BuiltinTraversableCarrier,
    },
    /// Partial evaluation of `Traverse` after its three evidence arguments
    /// prove one standard eager dictionary. Callable ABI: mapper, source.
    TraverseCollected {
        traversable: BuiltinTraversableCarrier,
        applicative: BuiltinCollectedApplicativeCarrier,
    },
    FilterMap(BuiltinFilterableCarrier),
}

/// Standard dictionaries whose traversal admits a linear eager collector.
/// Task and authored dictionaries retain their explicit callable operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinCollectedApplicativeCarrier {
    List,
    Option,
    Result,
    Validation,
    Signal,
}

impl BuiltinCollectedApplicativeCarrier {
    pub fn from_evidence(
        pure: BuiltinClassMemberIntrinsic,
        apply: BuiltinClassMemberIntrinsic,
        map: BuiltinClassMemberIntrinsic,
    ) -> Option<Self> {
        match (pure, apply, map) {
            (
                BuiltinClassMemberIntrinsic::Pure(BuiltinApplicativeCarrier::List),
                BuiltinClassMemberIntrinsic::Apply(BuiltinApplyCarrier::List),
                BuiltinClassMemberIntrinsic::Map(BuiltinFunctorCarrier::List),
            ) => Some(Self::List),
            (
                BuiltinClassMemberIntrinsic::Pure(BuiltinApplicativeCarrier::Option),
                BuiltinClassMemberIntrinsic::Apply(BuiltinApplyCarrier::Option),
                BuiltinClassMemberIntrinsic::Map(BuiltinFunctorCarrier::Option),
            ) => Some(Self::Option),
            (
                BuiltinClassMemberIntrinsic::Pure(BuiltinApplicativeCarrier::Result),
                BuiltinClassMemberIntrinsic::Apply(BuiltinApplyCarrier::Result),
                BuiltinClassMemberIntrinsic::Map(BuiltinFunctorCarrier::Result),
            ) => Some(Self::Result),
            (
                BuiltinClassMemberIntrinsic::Pure(BuiltinApplicativeCarrier::Validation),
                BuiltinClassMemberIntrinsic::Apply(BuiltinApplyCarrier::Validation),
                BuiltinClassMemberIntrinsic::Map(BuiltinFunctorCarrier::Validation),
            ) => Some(Self::Validation),
            (
                BuiltinClassMemberIntrinsic::Pure(BuiltinApplicativeCarrier::Signal),
                BuiltinClassMemberIntrinsic::Apply(BuiltinApplyCarrier::Signal),
                BuiltinClassMemberIntrinsic::Map(BuiltinFunctorCarrier::Signal),
            ) => Some(Self::Signal),
            _ => None,
        }
    }

    pub fn builtin(self) -> BuiltinApplicativeCarrier {
        match self {
            Self::List => BuiltinApplicativeCarrier::List,
            Self::Option => BuiltinApplicativeCarrier::Option,
            Self::Result => BuiltinApplicativeCarrier::Result,
            Self::Validation => BuiltinApplicativeCarrier::Validation,
            Self::Signal => BuiltinApplicativeCarrier::Signal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinFunctorCarrier {
    List,
    Option,
    Result,
    Validation,
    Signal,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinBifunctorCarrier {
    Result,
    Validation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinApplicativeCarrier {
    List,
    Option,
    Result,
    Validation,
    Signal,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinApplyCarrier {
    List,
    Option,
    Result,
    Validation,
    Signal,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinMonadCarrier {
    List,
    Option,
    Result,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinFoldableCarrier {
    List,
    Option,
    Result,
    Validation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinTraversableCarrier {
    List,
    Option,
    Result,
    Validation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinFilterableCarrier {
    List,
    Option,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinAppendCarrier {
    Text,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BuiltinOrdSubject {
    Int,
    Float,
    Decimal,
    BigInt,
    Bool,
    Text,
    Ordering,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionBase {
    AmbientSubject,
    Expr(ExprId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLiteral {
    pub segments: Vec<TextSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextSegment {
    Fragment { raw: Box<str>, span: SourceSpan },
    Interpolation { expr: ExprId, span: SourceSpan },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordExprField {
    pub label: Box<str>,
    pub value: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapEntry {
    pub key: ExprId,
    pub value: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeExpr {
    pub head: ExprId,
    pub stages: Vec<PipeStage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub span: SourceSpan,
    pub kind: PatternKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternKind {
    Wildcard,
    Binding(PatternBinding),
    Integer(IntegerLiteral),
    Text(Box<str>),
    Tuple(Vec<Pattern>),
    List {
        elements: Vec<Pattern>,
        rest: Option<Box<Pattern>>,
    },
    Record(Vec<RecordPatternField>),
    Constructor {
        callee: PatternConstructor,
        arguments: Vec<Pattern>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternBinding {
    pub binding: HirBindingId,
    pub name: Box<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordPatternField {
    pub label: Box<str>,
    pub pattern: Pattern,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternConstructor {
    pub display: Box<str>,
    pub reference: Reference,
    pub field_types: Option<Vec<Type>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeStage {
    pub span: SourceSpan,
    pub subject_memo: Option<HirBindingId>,
    pub result_memo: Option<HirBindingId>,
    pub input_subject: Type,
    pub result_subject: Type,
    pub kind: PipeStageKind,
}

impl PipeStage {
    pub const fn supports_memos(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeCaseArm {
    pub span: SourceSpan,
    pub pattern: Pattern,
    pub body: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeTruthyFalsyStage {
    pub truthy: PipeTruthyFalsyBranch,
    pub falsy: PipeTruthyFalsyBranch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipeTruthyFalsyBranch {
    pub span: SourceSpan,
    pub constructor: BuiltinTerm,
    pub payload_subject: Option<Type>,
    pub result_type: Type,
    pub body: ExprId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipeStageKind {
    Transform {
        mode: PipeTransformMode,
        expr: ExprId,
    },
    Tap {
        expr: ExprId,
    },
    Debug {
        label: Box<str>,
    },
    Gate {
        predicate: ExprId,
        emits_negative_update: bool,
    },
    Case {
        arms: Vec<PipeCaseArm>,
    },
    TruthyFalsy(Box<PipeTruthyFalsyStage>),
    FanOut {
        map_expr: ExprId,
    },
}
