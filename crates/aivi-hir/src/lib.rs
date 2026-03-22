#![forbid(unsafe_code)]

//! Milestone 2 HIR boundary with typed IDs, module-owned arenas, and structural validation.

pub mod arena;
mod hir;
mod ids;
mod lower;
mod sequence;
mod validate;

pub use arena::{Arena, ArenaId, ArenaOverflow};
pub use hir::{
    ApplicativeCluster, BinaryOperator, Binding, BindingKind, BindingPattern, BuiltinTerm,
    BuiltinType, CaseControl, ClassItem, ClassMember, ClusterFinalizer, ClusterPresentation,
    ControlNode, ControlNodeKind, Decorator, DecoratorCall, DecoratorPayload, DomainItem,
    DomainMember, DomainMemberKind, EachControl, EmptyControl, ExportItem, Expr, ExprKind,
    FragmentControl, FunctionItem, FunctionParameter, ImportBinding, InstanceItem, InstanceMember,
    IntegerLiteral, Item, ItemHeader, ItemKind, LiteralSuffixResolution, MarkupAttribute,
    MarkupAttributeValue, MarkupElement, MarkupNode, MarkupNodeKind, MatchControl, Module,
    ModuleArenas, Name, NameError, NamePath, NamePathError, Pattern, PatternKind, PipeExpr,
    PipeStage, PipeStageKind, ProjectionBase, RecordExpr, RecordExprField, RecordFieldSurface,
    RecordPatternField, RegexLiteral, ResolutionState, RootItemError, ShowControl, SignalItem,
    SourceDecorator, SourceMetadata, SuffixedIntegerLiteral, TermReference, TermResolution,
    TextFragment, TextInterpolation, TextLiteral, TextSegment, TypeField, TypeItem, TypeItemBody,
    TypeKind, TypeNode, TypeParameter, TypeReference, TypeResolution, TypeVariant, UnaryOperator,
    UseItem, ValueItem, WithControl,
};
pub use ids::{
    BindingId, ClusterId, ControlNodeId, DecoratorId, ExprId, ImportId, ItemId, MarkupNodeId,
    PatternId, TypeId, TypeParameterId,
};
pub use lower::{LoweringResult, lower_module};
pub use sequence::{AtLeastTwo, NonEmpty, SequenceError};
pub use validate::{ValidationMode, ValidationReport, validate_module};
