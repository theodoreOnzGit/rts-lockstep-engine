#!/bin/bash
# SPDX-License-Identifier: GPL-3.0-only
# Phase 1 gate (a): the scripted game's per-frame logic CRCs, native and wasm, must be identical (SCOPE.md, section 5).
# Builds examples/gate_native.rs (native) and examples/gate_wasm.rs (wasm32-unknown-unknown, release), runs the wasm under Node
# (scripts/gate_a.mjs), and compares the two lists. Usage: scripts/gate_a.sh [frames] (default 300). Exit status 0: identical.
set -euo pipefail
frames="${1:-300}"
cd "$(dirname "$0")/.."
target="${CARGO_TARGET_DIR:-target}"
cargo build --release --example gate_native >&2
cargo build --release --target wasm32-unknown-unknown --example gate_wasm >&2
native="$(mktemp)"; wasm="$(mktemp)"
trap 'rm -f "$native" "$wasm"' EXIT
"$target/release/examples/gate_native" "$frames" > "$native"
node scripts/gate_a.mjs "$target/wasm32-unknown-unknown/release/examples/gate_wasm.wasm" "$frames" > "$wasm"
n_native=$(wc -l < "$native"); n_wasm=$(wc -l < "$wasm")
echo "gate (a): $frames frames asked; native $n_native CRCs, wasm $n_wasm CRCs"
if cmp -s "$native" "$wasm" && [ "$n_native" -eq "$frames" ]; then
  echo "gate (a): PASS, identical CRCs at every frame (last $(tail -1 "$native"))"
else
  first=$(diff <(nl -ba "$native") <(nl -ba "$wasm") | grep -m1 '^<' | awk '{print $2}' || true)
  echo "gate (a): FAIL (first differing frame: ${first:-none, the lengths differ})"
  exit 1
fi
