#!/bin/bash
set -euo pipefail

cargo install --path crates/aivi
cd vscode
pnpm build

VSIX="$(ls -1t aivi-vscode-*.vsix | head -n 1)"
if command -v code >/dev/null 2>&1; then
  code --install-extension "$VSIX" --force >/dev/null
else
  EXT_DIR="$HOME/.vscode/extensions"
  VERSION="$(node -p "require('./package.json').version")"
  TARGET_DIR="$EXT_DIR/aivi.aivi-vscode-$VERSION"
  TMP_DIR="$(mktemp -d)"
  mkdir -p "$EXT_DIR"
  unzip -q "$VSIX" -d "$TMP_DIR"
  rm -rf "$EXT_DIR"/aivi.aivi-vscode-*
  mv "$TMP_DIR/extension" "$TARGET_DIR"
  rm -rf "$TMP_DIR"
fi

cd ../ui-client
pnpm build
cd ..

