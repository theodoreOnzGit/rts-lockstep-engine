#!/bin/bash
# The client's web build (issue #5): wasm32 release build, wasm-bindgen, and the page, into client/dist/.
# Needs: the wasm32-unknown-unknown target of the client's toolchain (client/rust-toolchain.toml) and wasm-bindgen-cli at
# the version in client/Cargo.lock. wasm-opt (binaryen) is used when present.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT/client"
TARGET_DIR=${CARGO_TARGET_DIR:-"$ROOT/client/target"}
cargo build --release --target wasm32-unknown-unknown --bin rts-client
OUT="$ROOT/client/dist"
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name rts_client "$TARGET_DIR/wasm32-unknown-unknown/release/rts-client.wasm"
if command -v wasm-opt > /dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o "$OUT/rts_client_bg.wasm" "$OUT/rts_client_bg.wasm"
fi
cp "$ROOT/client/web/index.html" "$OUT/"
ls -la "$OUT"
