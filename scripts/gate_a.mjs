// SPDX-License-Identifier: GPL-3.0-only
// Phase 1 gate (a), the wasm side's runner: loads the gate_wasm module (no imports), runs the scripted game for the given number
// of frames and prints its per-frame logic CRCs, one per line in hex, as examples/gate_native.rs does.
// Usage: node scripts/gate_a.mjs <module.wasm> <frames>
import { readFileSync } from 'node:fs';
const [, , path, framesArg] = process.argv;
const frames = Number(framesArg ?? 300);
const { instance } = await WebAssembly.instantiate(readFileSync(path), {});
const n = instance.exports.gate_run(frames);
for (let i = 0; i < n; i++) {
  console.log((instance.exports.gate_crc(i) >>> 0).toString(16).padStart(8, '0'));
}
