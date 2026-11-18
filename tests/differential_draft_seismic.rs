#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Terrain/TerrainVisual.cpp (DomeStyleSeismicFilter)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::terrain_visual` against `DomeStyleSeismicFilter` (#215).
//!
//! # Method
//!
//! `filterCallback` and `applyGravityCallback` are cut verbatim (build.rs) into `differential/wrap_draft_seismic.cpp`, with the
//! classes they use (`SeismicSimulationFilterBase`, `SeismicSimulationNode`, `WorldHeightMapInterfaceClass`, `MAP_XY_FACTOR`) cut
//! verbatim from TerrainVisual.h and MapObject.h. The height map is a stand-in on both sides: a hash of (x, y, salt) for a
//! bilinear sample, and every `setSeismicZVelocity` remembered and returned by later samples of that cell. Every call (the border
//! size, each sample with its coordinates, each set with its coordinates and value bits) is logged, and the two logs, the
//! status code and the gravity results are compared whole. 20,000 filter cases (null map; life 0, 1 to 14, 15, negatives, large;
//! radius 0 to 40; centres and borders negative and positive; magnitudes from `mixed`: zero, subnormal, huge, infinite, NaN) and
//! 20,000 gravity inputs. `sqrt` and `cos` (the `double` ones upstream calls on `float` arguments) are the interposed petir
//! functions on both sides (tests/support).
//!
//! Excluded input: a negative radius (`UnsignedInt workspaceWidth = radius*2` then allocates a huge array in C++, undefined; the
//! draft's `vec!` would abort). The node's `m_life` is an `UnsignedInt` upstream; the filter reads it as `Int`, as the draft's
//! `i32` does, so the generator's negative lives stand for values at or above 2^31.
//!
//! # Prediction (before the first run, 2026-11-18)
//!
//! 0 mismatches. The one place I expect trouble is `Real distance = sqrt( sqr(x) + sqr(y) )`: `sqr(Int)` is an `Int`, `sqrt` of it
//! is the `double` one, narrowed to `Real`, as the draft does.
//!
//! # Results
//!
//! 2026-11-18: **0 mismatches**, first run, no draft fix needed: 20,000 `filterCallback` sequences (each compares the status code and the whole
//! call log: the length, then word for word; 40,000 comparisons) and 20,000 `applyGravityCallback` inputs (max ULP 0). Prediction met.
//! The only edit outside the draft: one mechanical edit to the upstream cut (build.rs, asserted once): `filterCallback` reads the first
//! loop's `x` in its second loop (VC6's pre-standard `for` scope), so the copy declares `x` before the loop. Planted bugs, 11 of 11 caught
//! (mismatching comparisons of 40,000, 20,000 for gravity): 12261 3,836; 12262 9,257; 12263 3,240; 12264 4,901; 12265 20,362; 12266 8,228;
//! 12267 5 (only a NaN velocity separates the clamps); 12268 10,181; 12269 10,181; 12270 17,480 (of 20,000 gravity inputs); 12271 4,014.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 12261 `life <= 15`; 12262 `magnitude / radius`; 12263 `distance <= radius`; 12264 `PI/2` in `double`; 12265 no
//! `x != 0` guard (double stamp); 12266 the `(radius - x, radius - y)` stamp samples `centerX + x`; 12267 the clamp as
//! `!(v <= 9)` (NaN becomes 9); 12268 the final stuffing loop with x and y swapped; 12269 the workspace stride `radius` for the
//! first stamp; 12270 gravity `+ 1.5`; 12271 no `life == 0` guard.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::terrain_visual::{
    dome_style_apply_gravity_callback, dome_style_filter_callback, SeismicHeightMap, SeismicSimStatusCode, SeismicSimulationNode,
};
use std::collections::HashMap;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_seismic_filter(null_map: i32, cx: i32, cy: i32, radius: i32, life: i32, mag_bits: i32, border: i32, salt: u32, out: *mut i32, cap: i32) -> i32;
    fn ref_seismic_gravity(velocity_bits: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn hash_sample(x: i32, y: i32, salt: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ salt.wrapping_mul(83492791);
    h ^= h >> 15;
    h = h.wrapping_mul(2246822519);
    h ^= h >> 13;
    (h & 0xFFFFF) as i32 as f32 / 65536.0 - 8.0
}

struct Map {
    border: i32,
    salt: u32,
    over: HashMap<(i32, i32), f32>,
    log: Vec<i32>,
}

impl SeismicHeightMap for Map {
    fn get_border_size(&self) -> i32 {
        // logged through a RefCell-free trick: the border call is logged by the caller's order, see `run_rust`
        self.border
    }
    fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32 {
        match self.over.get(&(x, y)) {
            Some(v) => *v,
            None => hash_sample(x, y, self.salt),
        }
    }
    fn set_seismic_z_velocity(&mut self, x: i32, y: i32, value: f32) {
        self.log.push(2);
        self.log.push(x);
        self.log.push(y);
        self.log.push(value.to_bits() as i32);
        self.over.insert((x, y), value);
    }
}

/// The sample calls cannot be logged through `&self`; a wrapper map with interior mutability does it.
struct LogMap {
    inner: std::cell::RefCell<Map>,
}

struct LogRef<'a>(&'a LogMap);

impl SeismicHeightMap for LogRef<'_> {
    fn get_border_size(&self) -> i32 {
        let m = self.0.inner.borrow();
        drop(m);
        let mut m = self.0.inner.borrow_mut();
        m.log.push(3);
        m.border
    }
    fn get_bilinear_sample_seismic_z_velocity(&self, x: i32, y: i32) -> f32 {
        let mut m = self.0.inner.borrow_mut();
        m.log.push(1);
        m.log.push(x);
        m.log.push(y);
        m.get_bilinear_sample_seismic_z_velocity(x, y)
    }
    fn set_seismic_z_velocity(&mut self, x: i32, y: i32, value: f32) {
        self.0.inner.borrow_mut().set_seismic_z_velocity(x, y, value);
    }
}

fn status_code(s: SeismicSimStatusCode) -> i32 {
    match s {
        SeismicSimStatusCode::Invalid => 0,
        SeismicSimStatusCode::Active => 1,
        SeismicSimStatusCode::ZeroEnergy => 2,
    }
}

fn run_rust(null_map: bool, node: &SeismicSimulationNode, border: i32, salt: u32) -> Vec<i32> {
    let lm = LogMap { inner: std::cell::RefCell::new(Map { border, salt, over: HashMap::new(), log: Vec::new() }) };
    let mut r = LogRef(&lm);
    let status = if null_map { dome_style_filter_callback::<LogRef>(None, node) } else { dome_style_filter_callback(Some(&mut r), node) };
    let mut out = vec![status_code(status)];
    out.extend(lm.inner.borrow().log.iter().copied());
    out
}

fn small(g: &mut Gen, lo: i32, hi: i32) -> i32 {
    lo + g.below((hi - lo + 1) as u32) as i32
}

fn life(g: &mut Gen) -> i32 {
    match g.below(10) {
        0 => 0,
        1 => 15,
        2 => 14,
        3 => 1,
        4 => small(g, -20, -1),
        5 => small(g, 16, 40),
        6 => i32::MIN + g.below(3) as i32,
        7 => i32::MAX - g.below(3) as i32,
        _ => small(g, 1, 14),
    }
}

#[test]
fn filter_callback() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5E15);
    let mut c = Check::new("DomeStyleSeismicFilter::filterCallback");
    let mut buf = vec![0i32; 4_000_000];
    for case in 0..20_000 {
        let null_map = g.below(40) == 0;
        let radius = match g.below(8) {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => small(&mut g, 20, 40),
            _ => small(&mut g, 1, 12),
        };
        let l = life(&mut g);
        let mag = match g.below(6) {
            0 => g.uniform(-30.0, 30.0),
            1 => 20.0,
            2 => g.mixed(),
            _ => g.uniform(0.0, 40.0),
        };
        let (cx, cy) = (small(&mut g, -50, 300), small(&mut g, -50, 300));
        let border = small(&mut g, -3, 40);
        let salt = g.u32();
        let node = SeismicSimulationNode { center: (cx, cy), radius, life: l, magnitude: mag };
        let rust = run_rust(null_map, &node, border, salt);
        let n = unsafe { ref_seismic_filter(null_map as i32, cx, cy, radius, l, mag.to_bits() as i32, border, salt, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n > 0, "the output buffer is too small");
        let cpp = &buf[..n as usize];
        c.eq(rust.len(), cpp.len(), || format!("case {case} node {node:?} border {border}: log length"));
        let ok = rust == cpp;
        c.eq(ok, true, || {
            let first = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b);
            format!("case {case} node {node:?} border {border} salt {salt}: logs differ (status rust {} c++ {}, first difference at word {first:?})", rust[0], cpp[0])
        });
    }
    finish(c);
}

#[test]
fn apply_gravity_callback() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_6A41);
    let mut c = Check::new("DomeStyleSeismicFilter::applyGravityCallback");
    for _ in 0..20_000 {
        let v = match g.below(6) {
            0 => g.uniform(-20.0, 20.0),
            1 => [0.0f32, -0.0, 1.5, -1.5, 9.0, f32::INFINITY, f32::NEG_INFINITY][g.below(7) as usize],
            _ => g.mixed(),
        };
        let cpp = f32::from_bits(unsafe { ref_seismic_gravity(v.to_bits() as i32) } as u32);
        c.f32(dome_style_apply_gravity_callback(v), cpp, || format!("velocity {v:e}"));
    }
    finish(c);
}
