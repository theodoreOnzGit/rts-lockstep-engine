// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/control_bar_structure_inventory.rs` (`populate_button_proc`,
//! `populate_structure_inventory`, `update_context_structure_inventory`) against upstream `ControlBarStructureInventory.cpp`'s
//! `ControlBar::populateButtonProc`, `populateStructureInventory` and `updateContextStructureInventory`, cut verbatim by build.rs (`drafts_ui3b`;
//! `differential/wrap_draft_structure_inventory.cpp`, whose stand-ins and exclusions are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A control bar of 18 command windows (a random status word each: hidden, enabled, the
//! `ALWAYS_COLOR` and `NOT_READY` bits and other bits), windows 12 to 17 present or not; the three commands (`Command_Evacuate`, `Command_Stop`,
//! `Command_StructureExit`) defined or not; a hot key manager or none; 1 to 5 objects (a contain module or none, locally controlled or not,
//! relationship neutral or not, a drawable or none, a contain maximum from -2 to 14, a count from -1 to 14 that need not agree with the list of 0 to 12
//! contained objects, ids and image tokens random); 1 to 10 steps of `populateStructureInventory(object)`, `updateContextStructureInventory()`, a change
//! of an object's count, of the selection, of the last recorded count. After every step the calls logged (reset, hot keys, `setControlCommand`, the
//! button image and the overlay), every window's status word, the 18 contain-data entries and the last recorded count are compared.
//! The draft runs on the verified `GameWindowManager` with the same windows. EXCLUDED (upstream indexes out of bounds or dereferences NULL, the draft
//! skips): a contained list longer than the windows it reaches, a missing window among the buttons the loop reaches.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches: line for line. The likeliest miss: the order of the status calls (`winSetStatus` ALWAYS_COLOR before `winClearStatus` NOT_READY is
//! not observable) or the contain-data after a longer list than 10 (buttons 10 and 11 are the stop and evacuate windows).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21111 | the buttons are hidden first only up to `MAX_STRUCTURE_INVENTORY_BUTTONS`, not `MAX_COMMANDS_PER_SET` |
//! | 21112 | the evacuate button is given the `Command_Stop` command |
//! | 21113 | the surplus buttons are hidden from `i > max` (off by one) |
//! | 21114 | stop and evacuate are enabled for a count above 1, not for any |
//! | 21115 | the stop button is not shown again after the loop |
//! | 21116 | a contained object's veterancy overlay is cleared, not shown |
//! | 21117 | the contain data's index is one past the button's |
//! | 21118 | the last recorded count is not saved |
//! | 21119 | the deselect fires when the building is not ours OR the relationship is not neutral |
//! | 21120 | the buttons are repopulated only when the recorded count is lower than the contain count |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). The first run showed 12,302 mismatches, all in the contain data: the wrapper's `static m_containData` (a static array upstream) kept the previous case's entries; the wrapper now zeroes it at the start of every case (a harness fix, not a draft defect), then **0 mismatches**. No draft change was needed.
//! Planted bugs (of 20,000): 21111 13,939; 21112 13,535; 21113 9,480; 21114 1,961; 21115 13,955; 21116 13,192; 21117 13,192; 21118 12,930; 21119 7,791; 21120 4,824. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::control_bar_structure_inventory::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_sinv_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct Obj {
    id: u32,
    button: u32,
    overlay: u32,
    has_contain: bool,
    local: bool,
    rel: bool,
    has_draw: bool,
    max: i32,
    count: i32,
    contained: Vec<u32>,
}

struct Host {
    log: Vec<u32>,
    objs: Vec<Obj>,
    defined: [bool; 3],
    hotkey: bool,
    slots: Vec<Option<WinId>>,
    contain: [ContainEntry; 18],
    last: i32,
    selected: u32,
}

impl GuiHost for Host {}

impl StructureInventoryHost for Host {
    fn command_window(&self, i: usize) -> Option<WinId> {
        self.slots.get(i).copied().flatten()
    }
    fn reset_contain_data(&mut self) {
        self.log.push(1);
        self.contain = [ContainEntry::default(); 18];
    }
    fn set_contain_data(&mut self, i: usize, entry: ContainEntry) {
        self.contain[i] = entry;
    }
    fn hot_key_manager_reset(&mut self) {
        if self.hotkey {
            self.log.push(2);
        }
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        let k = match name {
            b"Command_Evacuate" => 0,
            b"Command_Stop" => 1,
            b"Command_StructureExit" => 2,
            _ => panic!("an unknown command name"),
        };
        self.defined[k].then_some(k + 1)
    }
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: WinId, command: Option<usize>) {
        self.log.extend([4, wm.tree.windows[w].id as u32, command.map_or(0, |c| c as u32)]);
    }
    fn gadget_button_set_enabled_image_of_template(&mut self, wm: &mut GameWindowManager, w: WinId, obj: u32) {
        self.log.extend([5, wm.tree.windows[w].id as u32, self.objs[obj as usize].button]);
    }
    fn gadget_button_draw_overlay_for_object(&mut self, wm: &mut GameWindowManager, w: WinId, obj: Option<u32>) {
        self.log.extend([6, wm.tree.windows[w].id as u32, obj.map_or(0, |o| self.objs[o as usize].overlay)]);
    }
    fn has_contain(&self, building: u32) -> bool {
        self.objs[building as usize].has_contain
    }
    fn contain_max(&self, building: u32) -> i32 {
        self.objs[building as usize].max
    }
    fn contain_count(&self, building: u32) -> i32 {
        self.objs[building as usize].count
    }
    fn iterate_contained(&self, building: u32) -> Vec<u32> {
        self.objs[building as usize].contained.clone()
    }
    fn object_id(&self, obj: u32) -> u32 {
        self.objs[obj as usize].id
    }
    fn last_recorded_inventory_count(&self) -> i32 {
        self.last
    }
    fn set_last_recorded_inventory_count(&mut self, n: i32) {
        self.last = n;
    }
    fn current_selected_object(&self) -> Option<u32> {
        Some(self.selected)
    }
    fn is_locally_controlled(&self, obj: u32) -> bool {
        self.objs[obj as usize].local
    }
    fn local_relationship_to_object_not_neutral(&self, obj: u32) -> bool {
        self.objs[obj as usize].rel
    }
    fn deselect_drawable_of_object(&mut self, obj: u32) {
        if self.objs[obj as usize].has_draw {
            self.log.extend([7, obj]);
        }
    }
}

#[test]
fn structure_inventory_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_06);
    let mut chk = Check::new("ControlBar structure inventory");
    let extras = [0x0100_0000u32, 0x0040_0000, 0x80, 0x1000, 0x0200_0000, 0x0000_0001];
    for _ in 0..cases(20_000) {
        let specs: Vec<WinSpec> = (0..18)
            .map(|i| {
                let mut extra = 0;
                for &e in &extras {
                    if g.below(4) == 0 {
                        extra |= e;
                    }
                }
                WinSpec { id: i + 1, hidden: g.below(2) == 0, enabled: g.below(2) == 0, no_focus: false, extra }
            })
            .collect();
        let (mut wm, order) = make_scene(&specs);
        let mut input = Vec::new();
        put_scene(&mut wm, &order, &specs, 0, &mut input);
        let present = if g.below(3) == 0 { g.below(64) } else { 63 };
        let first_absent = (0..6).find(|k| present >> k & 1 == 0).map_or(18, |k| 12 + k as usize);
        input.push(present);
        let defined = [g.below(6) != 0, g.below(6) != 0, g.below(6) != 0];
        input.push(u32::from(defined[0]) | u32::from(defined[1]) << 1 | u32::from(defined[2]) << 2);
        let hotkey = g.below(2) == 0;
        input.push(u32::from(hotkey));
        let nobj = 1 + g.below(5) as usize;
        input.push(nobj as u32);
        let mut objs = Vec::new();
        for _ in 0..nobj {
            let ncont = g.below(first_absent.min(12) as u32 + 1) as usize;
            let o = Obj {
                id: g.u32() | 1,
                button: 1 + g.below(1000),
                overlay: 1 + g.below(1000),
                has_contain: g.below(8) != 0,
                local: g.below(2) == 0,
                rel: g.below(2) == 0,
                has_draw: g.below(5) != 0,
                max: g.below(17) as i32 - 2,
                count: if g.below(3) == 0 { g.below(16) as i32 - 1 } else { ncont as i32 },
                contained: (0..ncont).map(|_| g.below(nobj as u32)).collect(),
            };
            input.extend([o.id, o.button, o.overlay, u32::from(o.has_contain) | u32::from(o.local) << 1 | u32::from(o.rel) << 2 | u32::from(o.has_draw) << 3, o.max as u32, o.count as u32, o.contained.len() as u32]);
            input.extend(o.contained.iter().copied());
            objs.push(o);
        }
        let selected = g.below(nobj as u32);
        let last = if g.below(2) == 0 { objs[selected as usize].count } else { g.below(16) as i32 - 1 };
        input.extend([selected, last as u32]);
        let slots: Vec<Option<WinId>> = (0..18).map(|i| if i < 12 || present >> (i - 12) & 1 == 1 { Some(order[i]) } else { None }).collect();
        let mut host = Host { log: Vec::new(), objs, defined, hotkey, slots, contain: [ContainEntry::default(); 18], last, selected };
        let steps = 1 + g.below(10);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let kind = g.below(6);
            let (kind, a, b) = match kind {
                0 | 1 => (kind, g.below(nobj as u32), 0),
                2 => (2, g.below(nobj as u32), g.below(16).wrapping_sub(1)),
                3 => (3, g.below(nobj as u32), 0),
                4 => (4, 0, g.below(16).wrapping_sub(1)),
                _ => (1, 0, 0),
            };
            input.extend([kind, a, b]);
            host.log.clear();
            match kind {
                0 => populate_structure_inventory(&mut wm, &mut host, a),
                1 => update_context_structure_inventory(&mut wm, &mut host),
                2 => host.objs[a as usize].count = b as i32,
                3 => host.selected = a,
                _ => host.last = b as i32,
            }
            rust.extend(host.log.iter().copied());
            rust.push(100);
            put_state(&wm, &order, &mut rust);
            for e in &host.contain {
                rust.push(e.control.map_or(0, |w| wm.tree.windows[w].id as u32));
                rust.push(e.object_id);
            }
            rust.push(host.last as u32);
        }
        let mut buf = vec![0u32; 1 << 16];
        let len = unsafe { ref_ui3b_sinv_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
