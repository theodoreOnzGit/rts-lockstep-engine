// SPDX-License-Identifier: GPL-3.0-only
//! Phase 1 gate (a), the native side: prints the scripted game's per-frame logic CRCs, one per line in hex
//! (`logic::world::gate_scenario`; `scripts/gate_a.sh` compares them with the wasm build's). The frame count is the first
//! argument (default 300).
//!
//! CLI glue: reached by `scripts/gate_a.sh`, not by a test (the game itself is tested in `gate_scenario`).

fn main() {
    let frames = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(300);
    for c in rts_lockstep_engine::logic::world::gate_scenario::run_gate_scenario(frames) {
        println!("{c:08x}");
    }
}
