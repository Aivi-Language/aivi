#!/bin/bash
# AIVI Specification HTML Builder
# Requires: pandoc, python3

set -e

SPEC_DIR="$(dirname "$0")"
OUTPUT_DIR="$SPEC_DIR/build"
mkdir -p "$OUTPUT_DIR/fonts"
cp "$SPEC_DIR/style.css" "$OUTPUT_DIR/"
cp "$SPEC_DIR/0xProto"/*.woff2 "$OUTPUT_DIR/fonts/"

function build_html() {
  local TITLE="$1"
  local FILENAME="$2"
  shift 2
  local FILES=("$@")
  
  local HTML_FILE="$OUTPUT_DIR/$FILENAME.html"
  local TEMP_SPEC_DIR="$OUTPUT_DIR/temp_md"
  mkdir -p "$TEMP_SPEC_DIR"

  echo "Building $TITLE..."

  local PROCESSED_FILES=()
  for f in "${FILES[@]}"; do
    local REL_PATH="${f#$SPEC_DIR/}"
    # Create a slug from the relative path: 02_syntax/01_bindings.md -> 02_syntax-01_bindings
    local SLUG=$(echo "$REL_PATH" | sed 's/\.md$//' | tr '/' '-')
    local TEMP_F="$TEMP_SPEC_DIR/$SLUG.md"
    
    # Add an anchor at the start of each file
    echo "<span id=\"$SLUG\"></span>" > "$TEMP_F"
    cat "$f" >> "$TEMP_F"
    PROCESSED_FILES+=("$TEMP_F")
  done

  echo "  Generating HTML..."
  pandoc "${PROCESSED_FILES[@]}" \
    --from commonmark_x \
    --to html5 \
    --standalone \
    --metadata title="$TITLE" \
    --metadata author="AIVI Project" \
    --metadata date="$(date +%Y-%m-%d)" \
    --highlight-style=tango \
    --css="style.css" \
    -o "$HTML_FILE"

  # Fix internal links via Python script
  echo "  Fixing internal links..."
  python3 "$SPEC_DIR/fix_links.py" "$HTML_FILE"

  # Apply AIVI syntax highlighting
  echo "  Applying syntax highlighting..."
  python3 "$SPEC_DIR/highlight.py" "$HTML_FILE"

  # Clean up temp files
  rm -rf "$TEMP_SPEC_DIR"

  echo "  ✓ HTML generated: $HTML_FILE"
}

# --- MAIN SPEC FILES ---
MAIN_FILES=(
  "$SPEC_DIR/README.md"
  "$SPEC_DIR/01_introduction.md"
  "$SPEC_DIR/02_syntax/00_grammar.md"
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
  "$SPEC_DIR/02_syntax/13_jsx_literals.md"
  "$SPEC_DIR/02_syntax/14_decorators.md"
  "$SPEC_DIR/02_syntax/15_resources.md"
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
  "$SPEC_DIR/05_stdlib/01_prelude.md"
  "$SPEC_DIR/05_stdlib/02_calendar.md"
  "$SPEC_DIR/05_stdlib/03_duration.md"
  "$SPEC_DIR/05_stdlib/04_color.md"
  "$SPEC_DIR/05_stdlib/05_vector.md"
  "$SPEC_DIR/05_stdlib/06_html.md"
  "$SPEC_DIR/05_stdlib/07_style.md"
  "$SPEC_DIR/05_stdlib/08_sqlite.md"
  "$SPEC_DIR/06_runtime/01_concurrency.md"
  "$SPEC_DIR/ideas/01_wasm_target.md"
  "$SPEC_DIR/ideas/02_liveview_frontend.md"
  "$SPEC_DIR/ideas/03_html_domains.md"
  "$SPEC_DIR/ideas/04_meta_domain.md"
  "$SPEC_DIR/ideas/05_tooling.md"
  "$SPEC_DIR/OPEN_QUESTIONS.md"
  "$SPEC_DIR/TODO.md"
)

# --- KERNEL FILES ---
KERNEL_FILES=(
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
)

build_html "AIVI Language Specification" "aivi-spec" "${MAIN_FILES[@]}"
build_html "AIVI Kernel Specification" "aivi-kernel" "${KERNEL_FILES[@]}"

# Create index.html for entry point
cp "$OUTPUT_DIR/aivi-spec.html" "$OUTPUT_DIR/index.html"

echo "Done!"
