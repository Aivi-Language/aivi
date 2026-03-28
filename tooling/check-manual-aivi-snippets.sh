#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
cargo run --quiet -p aivi-cli --bin aivi -- manual-snippets --root manual --todo manual/aivi-snippet-todo.json --write "$@"
