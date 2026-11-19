// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/particle_sys.rs`
//! (`find_parent_template`) against upstream `ParticleSys.cpp`'s `ParticleSystemManager::findParentTemplate`, cut verbatim by
//! build.rs (`drafts_ui2`; `differential/wrap_draft_find_parent.cpp` lists the stand-ins).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides): 0 to 8 templates (names from a pool of 10 with case variants, "FX"
//! and "fx", so names repeat and a repeated name updates the template; a slave name from the pool or empty) in a
//! `ParticleSystemManager` (`new_template`), then 1 to 8 queries (a name from the pool, or empty; `parentNum` from -1 to 4).
//! The answer to each (the template's name, or none) is compared.
//!
//! **Undefined order, stated:** upstream's `TemplateMap` is a `hash_map`, whose iteration order is unspecified, so which
//! template is the n-th parent of a name is unspecified upstream when more than one has it. The draft orders by name
//! (`BTreeMap`, byte order); the C++ stand-in is a `std::map` in the same order, so this test compares the counting logic
//! (the empty-name guard, the count down on matches only, the exact comparison) and not the hash order.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. Plants 15482 to 15486 all caught; 15482 (no empty-name guard) the fewest, needing an empty query and a
//! template with an empty slave name.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 15482 | no guard against an empty name |
//! | 15483 | the templates are walked in reverse order |
//! | 15484 | the slave name compares without case |
//! | 15485 | the count goes down for every template, not only the matches |
//! | 15486 | the count is tested after the decrement (the n+1-th parent) |
//!
//! # Results (2026-11-19)
//!
//! 20,000 sequences: **0 mismatches** on the first run. Planted bugs (of 20,000): 15482 1,810; 15483 1,232; 15484 2,988; 15485 4,356; 15486 5,992. **5 of 5 caught**; 15483 (reverse order) the fewest, not 15482 as predicted.
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::client::particle_sys::ParticleSystemManager;
use rts_lockstep_engine::drafts::sweep_common_client::particle_sys::find_parent_template;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_fp_run(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

const NAMES: [&str; 10] = ["FX", "fx", "Fx1", "Smoke", "smoke", "Fire", "FireA", "Fire2", "A", "B"];

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Check) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    println!("DIFF {} compared {} mismatches {} (sequences)", chk.name, chk.compared, chk.mismatches);
    if planted {
        assert!(chk.mismatches > 0, "plant not caught");
    } else {
        chk.finish();
    }
}

#[test]
fn find_parent_template_matches_upstream() {
    let mut g = Gen::new(0x215_2_05);
    let mut chk = Check::new("ParticleSystemManager::findParentTemplate");
    for _ in 0..cases() {
        let mut m = ParticleSystemManager::default();
        let mut input: Vec<i32> = Vec::new();
        let nt = g.below(9) as i32;
        input.push(nt);
        for _ in 0..nt {
            let name = g.below(10) as i32;
            // few distinct slave names, so several templates share a parent
            let slave = if g.below(4) == 0 { -1 } else { [0, 1, 3, 4, 5, 8][g.below(6) as usize] };
            input.extend([name, slave]);
            m.new_template(NAMES[name as usize].as_bytes()).info.slave_system_name = if slave < 0 { Vec::new() } else { NAMES[slave as usize].as_bytes().to_vec() };
        }
        let nq = 1 + g.below(8) as i32;
        input.push(nq);
        let mut rust = Vec::new();
        for _ in 0..nq {
            let name = if g.below(6) == 0 { -1 } else { [0, 1, 3, 4, 5, 8, 9][g.below(7) as usize] };
            let pn = g.below(6) as i32 - 1;
            input.extend([name, pn]);
            let q: &[u8] = if name < 0 { b"" } else { NAMES[name as usize].as_bytes() };
            rust.push(match find_parent_template(&m, q, pn) {
                None => -1,
                Some(k) => NAMES.iter().position(|n| n.as_bytes() == k.as_slice()).unwrap() as i32,
            });
        }
        let mut out = vec![0i32; 16];
        let k = unsafe { ref_u2_fp_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), 16) } as usize;
        out.truncate(k);
        chk.eq(rust, out, || format!("{input:?}"));
    }
    finish(chk);
}
