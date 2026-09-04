#!/usr/bin/env bash
# Build peram-core race-car hot path → public/game/pkg
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CRATE="$ROOT/crates/peram-core"
OUT="$ROOT/public/game/pkg"

echo "==> peram-core wasm-pack (release, web target)"
mkdir -p "$OUT"
cd "$CRATE"
wasm-pack build --target web --release --out-dir "$OUT" --out-name peram_core
# wasm-pack writes .gitignore that ignores pkg — we want pkg in repo for serve-without-build
rm -f "$OUT/.gitignore"
echo "==> wrote $OUT"
ls -la "$OUT"
echo "==> engine smoke (shared crate tests)"
cargo test --quiet --manifest-path "$CRATE/Cargo.toml"
echo "OK build-wasm — host-agnostic peram-core ready (wasm pkg + rlib)"
