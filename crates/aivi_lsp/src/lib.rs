mod backend;
mod completion;
mod diagnostics;
mod document_symbols;
mod navigation;
mod semantic_tokens;
mod server;
mod signature;
mod state;
mod workspace;

#[cfg(test)]
mod tests;

pub use server::run;
