#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ (RailroadGuideAIUpdate.cpp TrainTrack::incReference, releaseReference;
//   HackInternetAIUpdate.cpp isHacking; WorkerAIUpdate.cpp and DozerAIUpdate.cpp getRepairHealthPerSecond, getBoredTime;
//   JetAIUpdate.cpp friend_setAllowAirLoco, friend_addWaypointToGoalPath; SalvageCrateCollide.cpp doLevelGain; CreateCrateDie.cpp
//   testVeterancyLevel; Object.cpp getLastVictimID; SpecialPowerModule.cpp getPowerName)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::misc_object_accessors` against upstream's (#215).
//!
//! # Method
//!
//! The ten upstream functions are cut verbatim (build.rs, drafts_guilogic3) into `differential/wrap_draft_misc.cpp` (stand-ins in its
//! header). 20,000 random cases for each. The draft's `friend_set_allow_air_loco` is called the way its doc comment says to: with
//! `crate::logic::ai::jet_ai_update::ALLOW_AIR_LOCO`.
//!
//! Not covered: `ParticleUplinkCannonUpdate::createGroundHitParticleSystem` and `GhostObjectManager::updateOrphanedObjects`
//! (empty upstream; the drafts are empty), and the command source of `friend_addWaypointToGoalPath` (`CMD_FROM_AI`): the draft's
//! closure takes only the position, so the source is not observable from it (the wrapper's log records it; the test compares the position).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches except `friend_setAllowAirLoco`: upstream sets `m_flags |= (1 << f)` with `f` the `FlagType` index (ALLOW_AIR_LOCO = 1), and
//! the draft ORs its parameter as it is, documented as `jet_ai_update::ALLOW_AIR_LOCO`, which is that index (1, not the bit 2): I predict
//! the draft sets bit 0 where upstream sets bit 1, i.e. a mismatch in every case.
//!
//! # Results
//!
//! 2026-11-19: 20,000 cases per function. First run: 11 of 12 matched; **`friend_setAllowAirLoco` mismatched in 14,981 of 20,000** (prediction met): the draft ORed its
//! parameter as a bit mask while the constant it is documented to take (`jet_ai_update::ALLOW_AIR_LOCO` = 1) is the `FlagType` index, so it set bit 0 (`HAS_PENDING_COMMAND`)
//! where upstream (`1 << f`) sets bit 1. Fixed: the draft takes the index and shifts it. After the fix, **0 mismatches in all 12**. Planted bugs, 11 of 11 caught (mismatches
//! of 20,000): 15706 18,367 (inc by 2); 15707 11,810 (release tests before decrementing); 15708 5,701 (`>=` state); 15709 19,958 (bored time doubled, worker and dozer);
//! 15710 10,079 (clearing zeroes every flag); 15711 20,000 (two levels); 15712 7,544 (veterancy `>=`); 15713 13,381 (victim + 1); 15714 12,037 (name truncated); 15715 10,102
//! (repair rate `abs`, worker and dozer); 15716 20,000 (x and y swapped).

mod support;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::VeterancyLevel as V;
use rts_lockstep_engine::drafts::sweep_gui_logic::misc_object_accessors as m;
use rts_lockstep_engine::logic::ai::jet_ai_update::ALLOW_AIR_LOCO;
use std::ffi::CString;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_misc_track(start: i32, inp: *const i32, n: i32, out: *mut i32) -> i32;
    fn ref_misc_hack(state: i32) -> i32;
    fn ref_misc_float(kind: i32, bits: i32) -> i32;
    fn ref_misc_flags(flags: i32, allow: i32) -> i32;
    fn ref_misc_waypoint(x: i32, y: i32, z: i32, out: *mut i32) -> i32;
    fn ref_misc_levelgain() -> i32;
    fn ref_misc_vet(c: i32, o: i32) -> i32;
    fn ref_misc_victim(has: i32, v: i32) -> i32;
    fn ref_misc_power(name: *const std::os::raw::c_char, out: *mut std::os::raw::c_char) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

#[test]
fn misc_accessors() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_3C01);
    let names = ["track", "isHacking", "worker repair", "worker bored", "dozer repair", "dozer bored", "setAllowAirLoco", "addWaypoint", "doLevelGain", "testVeterancyLevel", "getLastVictimID", "getPowerName"];
    let mut cs: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    let levels = [V::Regular, V::Veteran, V::Elite, V::Heroic];
    for i in 0..20_000 {
        // TrainTrack: a start count and 1 to 12 inc/release operations
        let start = g.below(4) as i32 - 1;
        let ops: Vec<i32> = (0..1 + g.below(12)).map(|_| g.below(2) as i32).collect();
        let mut t = m::TrainTrackRefCount { ref_count: start };
        let mut want = Vec::new();
        for &o in &ops {
            if o == 0 {
                t.inc_reference();
            } else {
                want.push(t.release_reference() as i32);
            }
        }
        want.push(t.ref_count);
        let mut out = [0i32; 32];
        let k = unsafe { ref_misc_track(start, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr()) };
        cs[0].eq(want, out[..k as usize].to_vec(), || format!("case {i}: start {start} ops {ops:?}"));

        let (st, hid) = (g.below(7) as i32, 4);
        cs[1].eq(m::is_hacking(st as u32, hid) as i32, unsafe { ref_misc_hack(st) }, || format!("state {st}"));

        // the module data's floats: any bit pattern (NaN, infinities, zeros, denormals included)
        let bits = g.u32() as i32;
        let f = |b: i32| f32::from_bits(b as u32);
        let r = unsafe { (ref_misc_float(0, bits), ref_misc_float(1, bits), ref_misc_float(2, bits), ref_misc_float(3, bits)) };
        cs[2].eq(m::get_repair_health_per_second(f(bits)).to_bits() as i32, r.0, || format!("bits {bits:#x}"));
        cs[3].eq(m::get_bored_time(f(bits ^ 0x101)).to_bits() as i32, r.1, || format!("bits {bits:#x}"));
        cs[4].eq(m::get_repair_health_per_second(f(bits ^ 0x202)).to_bits() as i32, r.2, || format!("bits {bits:#x}"));
        cs[5].eq(m::get_bored_time(f(bits ^ 0x404)).to_bits() as i32, r.3, || format!("bits {bits:#x}"));

        let (flags, allow) = (g.u32() & if g.below(2) == 0 { 0x1ff } else { 0xffff_ffff }, g.below(2) != 0);
        let mut fl = flags;
        m::friend_set_allow_air_loco(&mut fl, ALLOW_AIR_LOCO, allow);
        cs[6].eq(fl as i32, unsafe { ref_misc_flags(flags as i32, allow as i32) }, || format!("flags {flags:#x} allow {allow}"));

        let (x, y, z) = (g.u32() as i32, g.u32() as i32, g.u32() as i32);
        let mut got = Vec::new();
        let pos = Coord3D { x: f32::from_bits(x as u32), y: f32::from_bits(y as u32), z: f32::from_bits(z as u32) };
        m::friend_add_waypoint_to_goal_path(|p: &Coord3D| got.extend([p.x.to_bits() as i32, p.y.to_bits() as i32, p.z.to_bits() as i32]), &pos);
        let mut out = [0i32; 8];
        let k = unsafe { ref_misc_waypoint(x, y, z, out.as_mut_ptr()) };
        // the log has the three coordinates and then the source (CMD_FROM_AI = 2)
        assert_eq!(k, 4);
        assert_eq!(out[3], 2);
        cs[7].eq(got, out[..3].to_vec(), || format!("pos {x:#x} {y:#x} {z:#x}"));

        let mut lv = 0;
        m::do_level_gain(|_, n| lv = n, 7);
        cs[8].eq(lv, unsafe { ref_misc_levelgain() }, || "level gain".to_string());

        let (c, o) = (g.below(4) as usize, g.below(4) as usize);
        cs[9].eq(m::test_veterancy_level(levels[c], levels[o]) as i32, unsafe { ref_misc_vet(c as i32, o as i32) }, || format!("{c} {o}"));

        let (has, v) = (g.below(3) != 0, g.below(5) as i32 - 1);
        cs[10].eq(m::get_last_victim_id(if has { Some(v) } else { None }), unsafe { ref_misc_victim(has as i32, v) }, || format!("{has} {v}"));

        let n = g.below(20) as usize;
        let name: Vec<u8> = (0..n).map(|_| b'A' + g.below(26) as u8).collect();
        let c = CString::new(name.clone()).expect("no NUL");
        let mut buf = vec![0 as std::os::raw::c_char; 64];
        let k = unsafe { ref_misc_power(c.as_ptr(), buf.as_mut_ptr()) };
        let cpp: Vec<u8> = buf[..k as usize].iter().map(|&b| b as u8).collect();
        cs[11].eq(m::get_power_name(&name), cpp, || format!("name {name:?}"));
    }
    if planted() {
        for c in cs {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in cs {
        c.finish();
    }
}
