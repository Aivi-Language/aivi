#!/bin/bash
# AIVI Specification PDF Builder
# Requires: pandoc, wkhtmltopdf (or weasyprint)

set -e

SPEC_DIR="$(dirname "$0")"
OUTPUT_DIR="$SPEC_DIR/build"
OUTPUT_FILE="$OUTPUT_DIR/aivi-spec.pdf"

mkdir -p "$OUTPUT_DIR"

echo "Building AIVI Language Specification PDF..."

# Collect all markdown files in order
FILES=(
  "$SPEC_DIR/README.md"
  "$SPEC_DIR/01_introduction.md"
  
  # Syntax
  "$SPEC_DIR/02_syntax/01_bindings.md"
  "$SPEC_DIR/02_syntax/02_functions.md"
  "$SPEC_DIR/02_syntax/03_types.md"
  "$SPEC_DIR/02_syntax/04_predicates.md"
  "$SPEC_DIR/02_syntax/05_patching.md"
  "$SPEC_DIR/02_syntax/06_domains.md"
  "$SPEC_DIR/02_syntax/07_generators.md"
  "$SPEC_DIR/02_syntax/08_pattern_matching.md"
  "$SPEC_DIR/02_syntax/09_effects.md"
  "$SPEC_DIR/02_syntax/10_modules.md"
  "$SPEC_DIR/02_syntax/11_domain_definition.md"
  "$SPEC_DIR/02_syntax/12_external_sources.md"
  
  # Kernel
  "$SPEC_DIR/03_kernel/01_core_terms.md"
  "$SPEC_DIR/03_kernel/02_types.md"
  "$SPEC_DIR/03_kernel/03_records.md"
  "$SPEC_DIR/03_kernel/04_patterns.md"
  "$SPEC_DIR/03_kernel/05_predicates.md"
  "$SPEC_DIR/03_kernel/06_traversals.md"
  "$SPEC_DIR/03_kernel/07_generators.md"
  "$SPEC_DIR/03_kernel/08_effects.md"
  "$SPEC_DIR/03_kernel/09_classes.md"
  "$SPEC_DIR/03_kernel/10_domains.md"
  "$SPEC_DIR/03_kernel/11_patching.md"
  "$SPEC_DIR/03_kernel/12_minimality.md"
  
  # Desugaring
  "$SPEC_DIR/04_desugaring/01_bindings.md"
  "$SPEC_DIR/04_desugaring/02_functions.md"
  "$SPEC_DIR/04_desugaring/03_records.md"
  "$SPEC_DIR/04_desugaring/04_patterns.md"
  "$SPEC_DIR/04_desugaring/05_predicates.md"
  "$SPEC_DIR/04_desugaring/06_generators.md"
  "$SPEC_DIR/04_desugaring/07_effects.md"
  "$SPEC_DIR/04_desugaring/08_classes.md"
  "$SPEC_DIR/04_desugaring/09_domains.md"
  "$SPEC_DIR/04_desugaring/10_patching.md"
  
  # Standard Library
  "$SPEC_DIR/05_stdlib/01_prelude.md"
  "$SPEC_DIR/05_stdlib/02_calendar.md"
  "$SPEC_DIR/05_stdlib/03_duration.md"
  "$SPEC_DIR/05_stdlib/04_color.md"
  "$SPEC_DIR/05_stdlib/05_vector.md"
  "$SPEC_DIR/05_stdlib/06_html.md"
  "$SPEC_DIR/05_stdlib/07_style.md"
  "$SPEC_DIR/05_stdlib/08_sqlite.md"
  
  # Ideas
  "$SPEC_DIR/ideas/01_wasm_target.md"
  "$SPEC_DIR/ideas/02_liveview_frontend.md"
  "$SPEC_DIR/ideas/03_html_domains.md"
  "$SPEC_DIR/ideas/04_meta_domain.md"
  "$SPEC_DIR/ideas/05_tooling.md"
  
  # Appendix
  "$SPEC_DIR/OPEN_QUESTIONS.md"
  "$SPEC_DIR/TODO.md"
)

# Check for dependencies
if command -v pandoc &> /dev/null; then
  echo "Using pandoc..."
  
  pandoc "${FILES[@]}" \
    --from markdown+smart+pipe_tables+fenced_code_blocks \
    --to pdf \
    --pdf-engine=wkhtmltopdf \
    --toc \
    --toc-depth=3 \
    --metadata title="AIVI Language Specification" \
    --metadata author="AIVI Project" \
    --metadata date="$(date +%Y-%m-%d)" \
    --highlight-style=tango \
    --css="$SPEC_DIR/style.css" \
    -V geometry:margin=1in \
    -V fontsize=11pt \
    -V colorlinks=true \
    -V linkcolor=blue \
    -V urlcolor=blue \
    -o "$OUTPUT_FILE"
    
elif command -v weasyprint &> /dev/null; then
  echo "Using weasyprint..."
  
  # First convert to HTML, then to PDF
  TEMP_HTML="$OUTPUT_DIR/temp.html"
  
  pandoc "${FILES[@]}" \
    --from markdown+smart+pipe_tables+fenced_code_blocks \
    --to html5 \
    --toc \
    --toc-depth=3 \
    --standalone \
    --metadata title="AIVI Language Specification" \
    --highlight-style=tango \
    -o "$TEMP_HTML"
  
  weasyprint "$TEMP_HTML" "$OUTPUT_FILE"
  rm "$TEMP_HTML"
  
else
  echo "Error: Neither pandoc nor weasyprint found."
  echo "Install pandoc: https://pandoc.org/installing.html"
  echo "Or weasyprint: pip install weasyprint"
  exit 1
fi

echo "✓ PDF generated: $OUTPUT_FILE"
