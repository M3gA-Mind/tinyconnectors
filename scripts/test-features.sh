#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 scripts/test-e2e-version.py
python3 scripts/test-script-contracts.py
cargo test --locked --workspace
cargo test --locked --workspace --all-features

cargo run -p tinyconnectors --example basic

set -euo pipefail
"$(dirname "${BASH_SOURCE[0]}")/../.github/scripts/check-storage-engine-trees.sh"
case "${RUNNER_OS:-$(uname -s)}" in
  Windows|MINGW*|MSYS*|CYGWIN*) ;;
  *) python3 scripts/test-package-layout.py ;;
esac
