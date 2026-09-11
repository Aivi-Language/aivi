#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
snippet_mode=(--preserve-format)
for argument in "$@"; do
    if [[ "${argument}" == "--write" ]]; then
        snippet_mode=()
    fi
done
cargo run --quiet -p aivi-cli --bin aivi -- manual-snippets --root manual --todo manual/aivi-snippet-todo.json "${snippet_mode[@]}" "$@"
node tooling/check-stdlib-api.mjs
