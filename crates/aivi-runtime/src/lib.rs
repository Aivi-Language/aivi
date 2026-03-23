#![forbid(unsafe_code)]

//! Runtime and scheduler foundations for the AIVI execution engine.

pub mod graph;
pub mod scheduler;

pub use graph::{
    DerivedHandle, DerivedSpec, GraphBuildError, InputHandle, OwnerHandle, OwnerSpec, SignalGraph,
    SignalGraphBuilder, SignalHandle, SignalKind, SignalSpec, TopologyBatch,
};
pub use scheduler::{
    DependencyValue, DependencyValues, DerivedNodeEvaluator, DroppedPublication, Generation,
    Publication, PublicationDropReason, PublicationStamp, Scheduler, SchedulerAccessError,
    SchedulerMessage, TickOutcome,
};
