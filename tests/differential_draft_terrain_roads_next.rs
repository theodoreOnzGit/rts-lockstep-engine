// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/terrain_roads.rs`
//! (`next_road`, `next_bridge`) against upstream `TerrainRoads.cpp`'s `TerrainRoadCollection::nextRoad` and `nextBridge`,
//! cut verbatim by build.rs (`drafts_ui2`; `differential/wrap_draft_terrain_roads_next.cpp` lists the stand-ins).
//!
//! # Method
//!
//! 5,000 sequences (fixed seed, `DRAFT_CASES` overrides) of 0 to 20 `newRoad` / `newBridge` calls, against the verified
//! `TerrainRoadCollection::new_road` / `new_bridge` (which give each type its id from 1, over roads and bridges). The C++ walks
//! its list from the head (`m_roadList`, `m_bridgeList`: upstream's lists put a new type at the head) with `nextRoad` /
//! `nextBridge`; the Rust walks from the last index (the draft's convention for the head) with `next_road` / `next_bridge`.
//! The id sequences of both lists are compared; this also checks the draft's claim that the head is the last index and that
//! `friend_getNext` is the previous index.
//!
//! **Stand-in (stated):** the C++ `newRoad` / `newBridge` are reduced to the "link to list" lines (the default-road and
//! default-bridge fields they copy do not change the list order). The `DEBUG_ASSERTCRASH`es are compiled out upstream in
//! release; the draft's `debug_assert!` is only run on roads passed as roads and bridges as bridges here.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. Plants 15478 to 15481 all caught in most sequences with 3 or more roads or bridges.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 15478 | `next_road` walks forward through the oldest-first vector (the wrong end) |
//! | 15479 | `next_road` stops one road early (`road >= 2`) |
//! | 15480 | `next_bridge` skips a bridge (steps by two) |
//! | 15481 | `next_bridge` consults the road list's length and ends the walk |
//!
//! # Results (2026-11-19)
//!
//! 5,000 sequences: **0 mismatches** on the first run. Planted bugs (of 5,000): 15478 4,013; 15479 4,013; 15480 4,031; 15481 3,653. **4 of 4 caught.**
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::common::terrain_roads::TerrainRoadCollection;
use rts_lockstep_engine::drafts::sweep_common_client::terrain_roads::*;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_tr_run(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(5000)
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
fn road_and_bridge_lists_walk_as_upstream() {
    let mut g = Gen::new(0x215_2_04);
    let mut chk = Check::new("TerrainRoadCollection::nextRoad / nextBridge");
    for _ in 0..cases() {
        let n = g.below(21) as usize;
        let steps: Vec<i32> = (0..n).map(|_| g.below(2) as i32).collect();
        let mut c = TerrainRoadCollection::new();
        for (k, &s) in steps.iter().enumerate() {
            let name = format!("R{k}");
            if s == 0 { c.new_road(name.as_bytes()); } else { c.new_bridge(name.as_bytes()); }
        }
        let mut rust: Vec<i32> = Vec::new();
        let mut cur = c.roads.len().checked_sub(1);
        while let Some(i) = cur {
            rust.push(c.roads[i].id as i32);
            cur = next_road(&c, i);
        }
        rust.push(0);
        let mut cur = c.bridges.len().checked_sub(1);
        while let Some(i) = cur {
            rust.push(c.bridges[i].id as i32);
            cur = next_bridge(&c, i);
        }
        let mut out = vec![0i32; 64];
        let k = unsafe { ref_u2_tr_run(steps.as_ptr(), n as i32, out.as_mut_ptr(), 64) } as usize;
        out.truncate(k);
        chk.eq(rust, out, || format!("{steps:?}"));
    }
    finish(chk);
}
