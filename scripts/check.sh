#!/bin/zsh
set -euo pipefail
PROJECT_ROOT="${0:A:h:h}"
cd "$PROJECT_ROOT"
cargo fmt --check
cargo test --locked --offline
cargo clippy --locked --offline --all-targets -- -D warnings
zsh -n scripts/*.sh
# Run on a logged-in Mac with a Metal GPU. This uses only synthetic features.
if [[ "${1:-}" == "--gpu" ]]; then
  cargo run --locked --offline --release -- --verify-render outputs/validation
fi
