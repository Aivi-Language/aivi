#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = aivi_fuzz::refresh_corpus_from_fixtures()?;
    println!(
        "refreshed fuzz corpora: {} parser seeds, {} decoder seeds",
        summary.parser_seed_count, summary.decoder_seed_count
    );
    Ok(())
}
