#![forbid(unsafe_code)]

//! GTK bridge foundations for the AIVI widget runtime.
//!
//! This crate intentionally stops at a strongly typed widget plan. It lowers HIR markup into a
//! stable widget/control graph with explicit property setters, event hookups, child operations,
//! and keyed collection management, while deferring actual GTK object creation to later runtime
//! milestones.

pub mod lower;
pub mod plan;

pub use lower::{
    LoweringError, LoweringOptions, lower_markup_expr, lower_markup_expr_with_options,
    lower_markup_root, lower_markup_root_with_options,
};
pub use plan::*;
