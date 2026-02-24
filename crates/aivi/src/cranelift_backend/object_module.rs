//! Object module construction for AOT compilation.
//!
//! Produces a native object file (.o) that can be linked with the AIVI runtime
//! library to create a standalone executable.

use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_object::{ObjectBuilder, ObjectModule};

use super::runtime_helpers::runtime_helper_symbols;

/// Create an `ObjectModule` targeting the host platform.
///
/// Runtime helper symbols are declared as imports (they'll be resolved at link time).
pub(crate) fn create_object_module(output_name: &str) -> Result<ObjectModule, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("is_pic", "true")
        .map_err(|e| format!("set is_pic: {e}"))?;
    flag_builder
        .set("opt_level", "speed")
        .map_err(|e| format!("set opt_level: {e}"))?;

    let isa_builder =
        isa::lookup(target_lexicon::Triple::host()).map_err(|e| format!("isa lookup: {e}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| format!("isa finish: {e}"))?;

    let builder = ObjectBuilder::new(isa, output_name, cranelift_module::default_libcall_names())
        .map_err(|e| format!("object builder: {e}"))?;

    let module = ObjectModule::new(builder);

    // Note: unlike JITModule, we don't pre-register runtime helper symbol addresses.
    // For AOT, they'll be resolved at link time against the runtime static library.
    // We just need to verify the symbols exist (they're declared as Linkage::Import
    // by declare_helpers).
    let _ = runtime_helper_symbols(); // ensure the helpers are compiled in

    Ok(module)
}
