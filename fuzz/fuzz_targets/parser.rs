#![no_main]

use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    // Avoid pathological allocations in the harness itself; libFuzzer will still mutate below this.
    if data.len() > 64 * 1024 {
        return;
    }
    let src = String::from_utf8_lossy(data);
    let (tokens, _lex_diags) = aivi::lex_cst(&src);
    let _ = aivi::parse_modules_from_tokens(Path::new("fuzz.aivi"), &tokens);
});

