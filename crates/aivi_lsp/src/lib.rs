mod backend;
mod completion;
mod diagnostics;
mod doc_index;
mod document_symbols;
mod navigation;
mod semantic_tokens;
mod server;
mod signature;
mod state;
mod strict;
mod workspace;

#[cfg(test)]
mod repro_lsp;
#[cfg(test)]
mod tests;

pub use server::run;
