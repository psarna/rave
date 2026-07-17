#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
wasm-pack build --release --target web --out-dir web/pkg
echo "Built web/pkg. Serve the repository root, then open /."
