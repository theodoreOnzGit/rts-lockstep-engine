#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarCommand.cpp (populateInvDataCallback,
//   resetBuildQueueData, calculateVeterancyOverlayForThing)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::control_bar_command` against upstream's (#215).
//!
//! # Method
//!
//! The three functions are cut verbatim (build.rs, drafts_guilogic3) into `differential/wrap_draft_cbcmd.cpp` (stand-ins listed in
//! its header). 20,000 random cases for each:
//!
//! - `populateInvDataCallback`: a button list of 1 to 16 non-NULL controls, a `maxIndex` (the last valid control, and sometimes smaller
//!   or larger than the list), a start index (0 to maxIndex + 1) and 1 to 6 objects each with a button image (or none) and a veterancy
//!   overlay (or none). Compared: the log of the gadget calls (which control, which image, enabled), the final index and the whole
//!   `m_containData`.
//! - `resetBuildQueueData`: nine entries with random control, type and a 32-bit value in the union (set through the pointer member).
//!   Compared: every field of every entry after.
//! - `calculateVeterancyOverlayForThing`: 0 to 6 modules, of which some are not `VeterancyGainCreate` and some have no data (the draft's
//!   `modules` is the filtered list), science `-1` (`SCIENCE_INVALID`) or 0 to 63, a mask of the sciences the player has, starting
//!   levels `LEVEL_REGULAR` to `LEVEL_HEROIC`, with and without a template and a local player, rank icons present or `NULL`.
//!
//! Excluded: a NULL control with a current index inside the list (upstream dereferences it: `control->winEnable`), and a control list
//! shorter than `maxIndex + 1` with a current index past its end (upstream reads past the array).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! (filled in after the run)

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::control_bar_command as cb;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_cb_populate(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_cb_reset(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_cb_vet(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn call(f: unsafe extern "C" fn(*const i32, i32, *mut i32, i32) -> i32, inp: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 1024];
    let n = unsafe { f(inp.as_ptr(), inp.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0);
    out.truncate(n as usize);
    out
}

fn image(id: i32) -> Option<Vec<u8>> {
    if id == 0 {
        None
    } else {
        Some(id.to_le_bytes().to_vec())
    }
}

fn image_id(i: &Option<Vec<u8>>) -> i32 {
    i.as_ref().map_or(0, |v| i32::from_le_bytes([v[0], v[1], v[2], v[3]]))
}

struct Host {
    button: i32,
    overlay: i32,
    log: Vec<i32>,
}

impl cb::PopulateInvHost for Host {
    fn object_button_image(&self, _obj: i32) -> Option<Vec<u8>> {
        image(self.button)
    }
    fn veterancy_overlay_for_object(&self, _obj: i32) -> Option<Vec<u8>> {
        image(self.overlay)
    }
    fn button_set_enabled_image(&mut self, control: u32, img: Option<Vec<u8>>) {
        self.log.extend([1, control as i32, image_id(&img)]);
    }
    fn button_draw_overlay_image(&mut self, control: u32, img: Option<Vec<u8>>) {
        self.log.extend([2, control as i32, image_id(&img)]);
    }
    fn win_enable(&mut self, control: u32, enable: bool) {
        self.log.extend([3, control as i32, enable as i32]);
    }
}

fn populate_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let nc = 1 + g.below(16) as usize;
    let controls: Vec<u32> = (0..nc).map(|_| 1 + g.below(500)).collect();
    let max_index = match g.below(4) {
        0 => nc as i32 - 1,
        1 => g.below(nc as u32) as i32,
        _ => nc as i32 - 1 - g.below(2) as i32,
    }
    .max(0);
    // current index: inside the control list (the defined inputs), or past maxIndex
    let start = g.below(max_index as u32 + 2) as i32;
    let no = 1 + g.below(6) as usize;
    let mut inp = vec![max_index, start, nc as i32];
    inp.extend(controls.iter().map(|&c| c as i32));
    inp.push(no as i32);
    let mut data = cb::PopulateInvButtonData { controls: controls.clone(), curr_index: start, max_index };
    let mut contain = [cb::ContainEntry::default(); 16];
    let mut host = Host { button: 0, overlay: 0, log: Vec::new() };
    for _ in 0..no {
        let (obj, b, o) = (1 + g.below(300) as i32, if g.below(4) == 0 { 0 } else { 1 + g.below(50) as i32 }, if g.below(3) == 0 { 0 } else { 51 + g.below(50) as i32 });
        inp.extend([obj, b, o]);
        host.button = b;
        host.overlay = o;
        // excluded: an index with no control (upstream reads past the list)
        if data.curr_index <= data.max_index && (data.curr_index as usize) < data.controls.len() || data.curr_index > data.max_index {
            cb::populate_inv_data_callback(&mut host, &mut contain, obj, &mut data);
        } else {
            return populate_case(g);
        }
    }
    let mut want = host.log;
    want.push(-1);
    want.push(data.curr_index);
    for c in contain.iter() {
        want.push(c.control.map_or(0, |x| x as i32));
        want.push(c.object_id);
    }
    (inp, want)
}

fn reset_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let mut inp = Vec::new();
    let mut q = [cb::QueueEntry::default(); cb::MAX_BUILD_QUEUE_BUTTONS];
    for e in q.iter_mut() {
        let control = if g.below(3) == 0 { 0 } else { 1 + g.below(400) as i32 };
        let ty = g.below(3) as i32;
        let p = if g.below(3) == 0 { 0 } else { g.u32() as i32 };
        inp.extend([control, ty, p]);
        e.control = if control == 0 { None } else { Some(control as u32) };
        e.ty = match ty {
            0 => cb::ProductionType::Invalid,
            1 => cb::ProductionType::Unit,
            _ => cb::ProductionType::Upgrade,
        };
        // the union: written through the pointer member, read back through both
        e.production_id = p as u32;
        e.upgrade_to_research = if p == 0 { None } else { Some(p as u32) };
    }
    cb::reset_build_queue_data(&mut q);
    let mut want = Vec::new();
    for e in q.iter() {
        want.push(e.control.map_or(0, |x| x as i32));
        want.push(match e.ty {
            cb::ProductionType::Invalid => 0,
            cb::ProductionType::Unit => 1,
            cb::ProductionType::Upgrade => 2,
        });
        want.push(e.production_id as i32);
        want.push(e.upgrade_to_research.map_or(0, |x| x as i32));
    }
    (inp, want)
}

fn vet_case(g: &mut Gen) -> (Vec<i32>, Vec<i32>) {
    let have_t = g.below(10) != 0;
    let have_p = g.below(8) != 0;
    let nm = g.below(7) as usize;
    let mask = g.u64();
    let mut inp = vec![have_t as i32, have_p as i32, nm as i32];
    let mut mods: Vec<(Option<i32>, cb::VeterancyLevelRank)> = Vec::new();
    for _ in 0..nm {
        let is_v = g.below(5) != 0;
        let has_d = g.below(6) != 0;
        let sci = if g.below(3) == 0 { -1 } else { g.below(64) as i32 };
        let lvl = g.below(4) as i32;
        inp.extend([is_v as i32, has_d as i32, sci, lvl]);
        if is_v && has_d {
            mods.push((if sci == -1 { None } else { Some(sci) }, [cb::VeterancyLevelRank::Regular, cb::VeterancyLevelRank::Veteran, cb::VeterancyLevelRank::Elite, cb::VeterancyLevelRank::Heroic][lvl as usize]));
        }
    }
    inp.extend([mask as u32 as i32, (mask >> 32) as u32 as i32]);
    let icons: Vec<i32> = (0..3).map(|i| if g.below(6) == 0 { 0 } else { 10 + i * 10 + g.below(5) as i32 }).collect();
    inp.extend(icons.iter().copied());
    let r = cb::calculate_veterancy_overlay_for_thing(have_t, have_p, &mods, |s| (mask >> s) & 1 != 0, image(icons[0]), image(icons[1]), image(icons[2]));
    (inp, vec![image_id(&r)])
}

#[test]
fn control_bar_command() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_CB11);
    let mut pop = Check::new("ControlBar::populateInvDataCallback");
    let mut rst = Check::new("ControlBar::resetBuildQueueData");
    let mut vet = Check::new("ControlBar::calculateVeterancyOverlayForThing");
    for i in 0..20_000 {
        let (inp, want) = populate_case(&mut g);
        pop.eq(want, call(ref_cb_populate, &inp), || format!("case {i}: {inp:?}"));
        let (inp, want) = reset_case(&mut g);
        rst.eq(want, call(ref_cb_reset, &inp), || format!("case {i}: {inp:?}"));
        let (inp, want) = vet_case(&mut g);
        vet.eq(want, call(ref_cb_vet, &inp), || format!("case {i}: {inp:?}"));
    }
    if planted() {
        for c in [pop, rst, vet] {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for c in [pop, rst, vet] {
        c.finish();
    }
}
