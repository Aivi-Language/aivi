pub mod cg_type {
    pub use aivi_core::cg_type::*;
}

include!("rust_ir/lowering.rs");
include!("rust_ir/unbound_vars.rs");
