// SPDX-License-Identifier: GPL-3.0-only
//! Phase 1 gate (a), the wasm side: a `wasm32-unknown-unknown` module (`crate-type = ["cdylib"]`, no imports) that runs the
//! scripted game (`logic::world::gate_scenario`) and exports its per-frame logic CRCs: `gate_run(frames)` runs it and answers
//! how many CRCs it took, `gate_crc(i)` answers the `i`th. `scripts/gate_a.mjs` loads it under Node.
//!
//! FFI glue: reached by `scripts/gate_a.sh`, not by a test (the game itself is tested in `gate_scenario`).

use std::sync::Mutex;

static CRCS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// Runs the game for `frames` frames; the number of CRCs taken.
#[no_mangle]
pub extern "C" fn gate_run(frames: u32) -> u32 {
    let crcs = rts_lockstep_engine::logic::world::gate_scenario::run_gate_scenario(frames);
    let n = crcs.len() as u32;
    *CRCS.lock().unwrap_or_else(|e| e.into_inner()) = crcs;
    n
}

/// The `i`th CRC of the last run (0 past the end).
#[no_mangle]
pub extern "C" fn gate_crc(i: u32) -> u32 {
    CRCS.lock().unwrap_or_else(|e| e.into_inner()).get(i as usize).copied().unwrap_or(0)
}
