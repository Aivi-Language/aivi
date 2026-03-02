#!/usr/bin/env bash
# Fetches GObject Introspection (GIR) files for GTK4 and libadwaita.
# Outputs to assets/gir/ in the repo root.
#
# Usage:
#   ./scripts/fetch-gir.sh          # auto-detect: copy local or download
#   ./scripts/fetch-gir.sh --local  # force copy from /usr/share/gir-1.0
#   ./scripts/fetch-gir.sh --remote # force download from GNOME GitLab
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$REPO_ROOT/assets/gir"
mkdir -p "$OUT_DIR"

LOCAL_GIR="/usr/share/gir-1.0"

FILES=(
  "Gtk-4.0.gir"
  "Adw-1.gir"
)

mode="${1:-auto}"

copy_local() {
  local name="$1"
  if [[ -f "$LOCAL_GIR/$name" ]]; then
    echo "Copying $name from $LOCAL_GIR"
    cp "$LOCAL_GIR/$name" "$OUT_DIR/$name"
    return 0
  fi
  return 1
}

download_file() {
  local name="$1"
  # gtk-rs/gir-files maintains up-to-date GIR files for all GNOME libraries
  local url="https://raw.githubusercontent.com/gtk-rs/gir-files/master/$name"
  echo "Downloading $name from $url"
  curl -fsSL --retry 3 -o "$OUT_DIR/$name" "$url"
}

for name in "${FILES[@]}"; do
  case "$mode" in
    --local)
      copy_local "$name" || { echo "ERROR: $name not found in $LOCAL_GIR"; exit 1; }
      ;;
    --remote)
      download_file "$name"
      ;;
    auto|*)
      copy_local "$name" 2>/dev/null || download_file "$name"
      ;;
  esac
done

echo ""
echo "GIR files saved to $OUT_DIR:"
ls -lh "$OUT_DIR"/*.gir
