#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/DemoralizeSpecialPower.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::demoralize_special_power::DemoralizeSpecialPower` against `DemoralizeSpecialPower` (#215).
//!
//! # Method
//!
//! `doSpecialPowerAtLocation` and `doSpecialPowerAtObject` are cut verbatim (build.rs) into `differential/wrap_draft_demoralize.cpp`
//! on stand-ins for everything they call (listed there: the special power base, the object, its contain module and AI, the partition
//! scan and its filters, the FX list). The stand-ins and the draft's `DemoralizeWorld` record the same events as words: the base class
//! call (location, angle, options), the scan (range bits, location), each `setDemoralized` (object, duration) and the FX (location). The
//! event logs are compared in order. 100,000 cases from a fixed seed: the ranges from the generator's mixed floats (NaN, infinities,
//! negatives), the durations and the contain count from edge values (0, 1, `u32::MAX` so that the product wraps), contain module or
//! none, disabled or not, a NULL location or object, 0 to 6 scanned objects of which some have no AI, with and without an FX list.
//!
//! The upstream file is under `#ifdef ALLOW_DEMORALIZE`, which the shipped game does not define; it is compiled here as if defined.
//! The draft's field names, `Snapshot` and the INI keys are not covered (no C++ of the module data's parser or xfer is built).
//!
//! # Prediction (written before the first run, 2026-11-18)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-18: 100,000 cases (69,774 reached the scan), **0 mismatches** (prediction held). Planted bugs, 10 of 10 caught (of 100,000 cases): 11511 23,711;
//! 11512 29,213; 11513 18,163; 11514 11,709; 11515 34,927 (the first version of the test returned one position for every object, so 11515 was
//! invisible until the test gave the source its own); 11516 34,857; 11517 69,774; 11518 7,393; 11519 44,830; 11520 28,568.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11511 the duration cap omitted; 11512 the range cap omitted; 11513 the range bonus not multiplied by the count; 11514
//! the caps applied also with no contain module; 11515 `doSpecialPowerAtObject` uses the source's position; 11516 the FX played before the
//! scan; 11517 the base call after the scan; 11518 the duration product saturates instead of wrapping; 11519 only the first scanned object
//! demoralized; 11520 the range cap replaced by the base range.

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::drafts::logic_common::demoralize_special_power::{DemoralizeSpecialPower, DemoralizeSpecialPowerModuleData, DemoralizeWorld};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_demoralize_run(input: *const u32, out: *mut u32) -> i32;
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

struct World {
    log: Vec<u32>,
    disabled: bool,
    contain: Option<u32>,
    scanned: Vec<(i32, bool)>,
    target_pos: Coord3D,
}

fn push3(log: &mut Vec<u32>, c: &Coord3D) {
    log.extend([c.x.to_bits(), c.y.to_bits(), c.z.to_bits()]);
}

impl DemoralizeWorld for World {
    fn is_disabled(&self, obj: i32) -> bool {
        let _ = obj;
        self.disabled
    }
    fn special_power_base_do_at_location(&mut self, _obj: i32, loc: &Coord3D, angle: f32, command_options: u32) {
        self.log.push(1);
        push3(&mut self.log, loc);
        self.log.extend([angle.to_bits(), command_options]);
    }
    fn contain_count(&self, _obj: i32) -> Option<u32> {
        self.contain
    }
    fn iterate_enemy_neutral_infantry_in_range(&mut self, _source: i32, loc: &Coord3D, range: f32) -> Vec<i32> {
        self.log.push(2);
        self.log.push(range.to_bits());
        push3(&mut self.log, loc);
        self.scanned.iter().map(|(id, _)| *id).collect()
    }
    fn set_demoralized(&mut self, obj: i32, duration: u32) -> bool {
        match self.scanned.iter().find(|(id, _)| *id == obj) {
            Some((_, true)) => {
                self.log.extend([3, obj as u32, duration]);
                true
            }
            _ => false,
        }
    }
    fn do_fx_pos(&mut self, _fx_list: &[u8], loc: &Coord3D) {
        self.log.push(4);
        push3(&mut self.log, loc);
    }
    fn position(&self, obj: i32) -> Coord3D {
        // the target (object 2) has its own position; the source (object 1) is at the origin, as the stand-in's
        if obj == 2 { self.target_pos } else { Coord3D { x: 0.0, y: 0.0, z: 0.0 } }
    }
}

fn dur(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 => 1,
        2 => g.below(100),
        3 => u32::MAX,
        4 => u32::MAX - g.below(5),
        5 => 1 << g.below(32),
        _ => g.below(5000),
    }
}

fn cnt(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => 0,
        1 => 1,
        2 => 2 + g.below(10),
        3 => u32::MAX,
        4 => 0x8000_0000,
        5 => 1 << g.below(32),
        _ => g.below(100),
    }
}

fn rng(g: &mut Gen) -> f32 {
    match g.below(6) {
        0 => 0.0,
        1 => g.uniform(0.0, 300.0),
        2 => g.uniform(-50.0, 50.0),
        _ => g.mixed(),
    }
}

#[test]
fn demoralize_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_DE30);
    let mut c = Check::new("DemoralizeSpecialPower");
    let mut with_scan = 0u32;
    for case in 0..100_000 {
        let disabled = g.below(5) == 0;
        let contain = if g.below(4) == 0 { None } else { Some(cnt(&mut g)) };
        let (base_range, bonus_range, max_range) = (rng(&mut g), rng(&mut g), rng(&mut g));
        let (base_dur, bonus_dur, max_dur) = (dur(&mut g), dur(&mut g), dur(&mut g));
        let has_fx = g.below(2) == 0;
        let by_object = g.below(2) == 0;
        let loc_null = g.below(8) == 0;
        let obj_null = g.below(8) == 0;
        let loc = Coord3D { x: g.mixed(), y: g.mixed(), z: g.mixed() };
        let target = Coord3D { x: g.mixed(), y: g.mixed(), z: g.mixed() };
        let angle = g.mixed();
        let opts = g.u32();
        let n = g.below(7) as usize;
        let scanned: Vec<(i32, bool)> = (0..n).map(|i| (10 + i as i32, g.below(3) != 0)).collect();

        let mut input: Vec<u32> = vec![
            disabled as u32, contain.is_some() as u32, contain.unwrap_or(0), base_range.to_bits(), bonus_range.to_bits(), max_range.to_bits(),
            base_dur, bonus_dur, max_dur, has_fx as u32, by_object as u32, loc_null as u32, loc.x.to_bits(), loc.y.to_bits(), loc.z.to_bits(),
            angle.to_bits(), opts, target.x.to_bits(), target.y.to_bits(), target.z.to_bits(), obj_null as u32, n as u32,
        ];
        for (id, ai) in &scanned {
            input.push(*id as u32);
            input.push(*ai as u32);
        }
        let mut cpp = vec![0u32; 256];
        let len = unsafe { ref_demoralize_run(input.as_ptr(), cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let data = DemoralizeSpecialPowerModuleData {
            base_range,
            bonus_range_per_captured: bonus_range,
            max_range,
            base_duration_in_frames: base_dur,
            bonus_duration_per_captured_in_frames: bonus_dur,
            max_duration_in_frames: max_dur,
            fx_list: if has_fx { Some(b"FX".to_vec()) } else { None },
        };
        let mut p = DemoralizeSpecialPower::new(data);
        let mut w = World { log: Vec::new(), disabled, contain, scanned, target_pos: target };
        if !by_object {
            p.do_special_power_at_location(&mut w, 1, if loc_null { None } else { Some(&loc) }, angle, opts);
        } else {
            p.do_special_power_at_object(&mut w, 1, if obj_null { None } else { Some(2) }, angle, opts);
        }
        if w.log.iter().any(|&x| x == 2) {
            with_scan += 1;
        }
        c.eq(w.log, cpp, || format!("case {case}: input {input:08x?}"));
    }
    println!("(demoralize: {with_scan} cases reached the scan)");
    finish(c);
}
