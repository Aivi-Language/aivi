#!/bin/bash
cargo install --path crates/aivi
cd vscode
pnpm build
cd ../ui-client
pnpm build
cd ..


