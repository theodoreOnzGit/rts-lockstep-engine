// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/control_bar_multi_select.rs` (`reset_common_command_data`,
//! `add_common_commands`, `populate_multi_select`, `update_context_multi_select`) against upstream `ControlBarMultiSelect.cpp`'s
//! `ControlBar::resetCommonCommandData`, `addCommonCommands`, `populateMultiSelect` and `updateContextMultiSelect`, cut verbatim by build.rs
//! (`drafts_ui3b`; `differential/wrap_draft_multi_select.cpp`, whose stand-ins and the one exclusion are listed there; the enums `CommandOption` and
//! `CommandAvailability` are cut from ControlBar.h too, so the draft's `OK_FOR_MULTI_SELECT` and `CHECK_LIKE` values are checked).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). 18 command windows (a random status word each: hidden, enabled, `NOT_READY`,
//! `ALWAYS_COLOR`; present or not; a random button command or none), 2 to 6 command buttons (options drawn from OK_FOR_MULTI_SELECT, CHECK_LIKE,
//! others; attack move or not), 1 to 4 command sets of 18 slots (mostly filled, a few empty), 1 to 6 objects (ignored in the GUI or not, sold or not, a
//! command set or none, a portrait image or none), a random selection (1 to 6 objects, repeats included), a random initial common command per slot;
//! 1 to 10 steps of `populateMultiSelect`, `updateContextMultiSelect`, `addCommonCommands(drawable, first)`, `resetCommonCommandData` and a new selection.
//! `getCommandAvailability` answers by a hash of (command, object, window), logged with its arguments. After every step the calls logged (overlay clears,
//! commands set, availability asked, check-like marks, the portrait object), every window's status word and button data and the 18 common commands are
//! compared. The draft runs on the verified `GameWindowManager` with the same windows. EXCLUDED: a selected drawable without an object (upstream
//! dereferences NULL).
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The draft's enum `CommandAvailability` has a different order from the C++'s but is mapped by name, not number.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21131 | `resetCommonCommandData` does not clear the common commands |
//! | 21132 | the first drawable's commands are shown but not enabled |
//! | 21133 | the first drawable keeps commands that are CHECK_LIKE, not OK_FOR_MULTI_SELECT |
//! | 21134 | a drawable with no command set does not hide the windows |
//! | 21135 | attack move needs both buttons to have it (`&&` for `||`) |
//! | 21136 | a differing command is removed even when it is attack move |
//! | 21137 | a sold object's commands are added |
//! | 21138 | the portrait object is the last selected, not the first |
//! | 21139 | a command that cannot be afforded is marked NOT_READY |
//! | 21140 | a command is enabled when 2 or more objects can do it (`> 1`) |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). No draft change was needed. Prediction held.
//! Planted bugs (of 20,000): 21131 13,778; 21132 6,820; 21133 7,277; 21134 1,923; 21135 4,483; 21136 2,320; 21137 2,763; 21138 3,892; 21139 7,650; 21140 5,402. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::control_bar_command::CommandAvailability;
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::drafts::ui::control_bar_multi_select::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_msel_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn avail_hash(cmd: u32, obj: u32, win: u32) -> u32 {
    let mut h: u32 = 2166136261;
    h ^= cmd;
    h = h.wrapping_mul(16777619);
    h ^= obj;
    h = h.wrapping_mul(16777619);
    h ^= win;
    h = h.wrapping_mul(16777619);
    (h >> 7) % 6
}

struct Cmd {
    options: u32,
    attack_move: bool,
}
struct ObjM {
    ignored: bool,
    sold: bool,
    set: u32,
    portrait: u32,
}

struct Host {
    log: Vec<u32>,
    order: Vec<usize>,
    slots: Vec<Option<usize>>,
    gdata: Vec<u32>,
    common: [Option<usize>; 18],
    cmds: Vec<Cmd>,
    sets: Vec<[u32; 18]>,
    objs: Vec<ObjM>,
    selected: Vec<u32>,
}

impl Host {
    fn idx(&self, w: usize) -> usize {
        self.order.iter().position(|&x| x == w).unwrap()
    }
}

impl GuiHost for Host {}

impl MultiSelectHost for Host {
    fn command_window(&self, i: usize) -> Option<usize> {
        self.slots[i]
    }
    fn common_command(&self, i: usize) -> Option<usize> {
        self.common[i]
    }
    fn set_common_command(&mut self, i: usize, c: Option<usize>) {
        self.common[i] = c;
    }
    fn gadget_button_clear_overlay(&mut self, wm: &mut GameWindowManager, w: usize) {
        self.log.extend([5, wm.tree.windows[w].id as u32]);
    }
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>) {
        let t = command.map_or(0, |c| c as u32);
        self.log.extend([6, wm.tree.windows[w].id as u32, t]);
        let i = self.idx(w);
        self.gdata[i] = t;
    }
    fn drawable_object(&self, draw: u32) -> Option<u32> {
        Some(draw)
    }
    fn is_ignored_in_gui(&self, obj: u32) -> bool {
        self.objs[obj as usize].ignored
    }
    fn command_set_buttons(&self, obj: u32) -> Option<Vec<Option<usize>>> {
        let s = self.objs[obj as usize].set;
        (s != 0).then(|| self.sets[s as usize - 1].iter().map(|&t| (t != 0).then_some(t as usize)).collect())
    }
    fn command_options(&self, c: usize) -> u32 {
        self.cmds[c - 1].options
    }
    fn command_is_attack_move(&self, c: usize) -> bool {
        self.cmds[c - 1].attack_move
    }
    fn selected_drawables(&self) -> Vec<u32> {
        self.selected.clone()
    }
    fn is_sold(&self, obj: u32) -> bool {
        self.objs[obj as usize].sold
    }
    fn drawable_portrait_image(&self, draw: u32) -> usize {
        self.objs[draw as usize].portrait as usize
    }
    fn set_portrait_by_object(&mut self, obj: Option<u32>) {
        self.log.extend([7, obj.unwrap_or(0xFFFF_FFFF)]);
    }
    fn gadget_button_command(&self, _wm: &GameWindowManager, w: usize) -> Option<usize> {
        let t = self.gdata[self.idx(w)];
        (t != 0).then_some(t as usize)
    }
    fn get_command_availability(&mut self, wm: &mut GameWindowManager, command: usize, obj: u32, w: usize) -> CommandAvailability {
        let id = wm.tree.windows[w].id as u32;
        self.log.extend([3, command as u32, obj, id]);
        match avail_hash(command as u32, obj, id) {
            0 => CommandAvailability::Restricted,
            1 => CommandAvailability::Available,
            2 => CommandAvailability::Active,
            3 => CommandAvailability::Hidden,
            4 => CommandAvailability::NotReady,
            _ => CommandAvailability::CantAfford,
        }
    }
    fn gadget_check_like_button_set_visual_check(&mut self, wm: &mut GameWindowManager, w: usize, checked: bool) {
        self.log.extend([4, wm.tree.windows[w].id as u32, u32::from(checked)]);
    }
}

#[test]
fn multi_select_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_07);
    let mut chk = Check::new("ControlBar multi select");
    let extras = [0x0040_0000u32, 0x0100_0000, 0x80];
    let option_bits = [0x100u32, 0x400, 0x40, 0x2000, 0x1_0000];
    for _ in 0..cases(20_000) {
        let specs: Vec<WinSpec> = (0..18)
            .map(|i| {
                let mut extra = 0;
                for &e in &extras {
                    if g.below(4) == 0 {
                        extra |= e;
                    }
                }
                WinSpec { id: i + 1, hidden: g.below(3) == 0, enabled: g.below(2) == 0, no_focus: false, extra }
            })
            .collect();
        let (mut wm, order) = make_scene(&specs);
        let mut input = Vec::new();
        put_scene(&mut wm, &order, &specs, 0, &mut input);
        let mask = if g.below(3) == 0 { g.below(1 << 18) } else { (1 << 18) - 1 };
        input.push(mask);
        let ncmd = 2 + g.below(5) as usize;
        let gdata: Vec<u32> = (0..18).map(|_| if g.below(4) == 0 { 0 } else { 1 + g.below(ncmd as u32) }).collect();
        input.extend(gdata.iter().copied());
        let mut common = [None; 18];
        for c in common.iter_mut() {
            if g.below(3) != 0 {
                *c = Some(1 + g.below(ncmd as u32) as usize);
            }
        }
        input.extend(common.iter().map(|c| c.map_or(0, |t| t as u32)));
        input.push(ncmd as u32);
        let mut cmds = Vec::new();
        for _ in 0..ncmd {
            let mut options = 0;
            for &b in &option_bits {
                if g.below(2) == 0 {
                    options |= b;
                }
            }
            let c = Cmd { options, attack_move: g.below(4) == 0 };
            input.extend([c.options, u32::from(c.attack_move)]);
            cmds.push(c);
        }
        let nset = 1 + g.below(4) as usize;
        input.push(nset as u32);
        let mut sets = Vec::new();
        for _ in 0..nset {
            let fill = g.below(4);
            let mut slots = [0u32; 18];
            for s in slots.iter_mut() {
                *s = if fill == 0 { 0 } else if g.below(4) == 0 { 0 } else { 1 + g.below(ncmd as u32) };
            }
            input.extend(slots.iter().copied());
            sets.push(slots);
        }
        let nobj = 1 + g.below(6) as usize;
        input.push(nobj as u32);
        let mut objs = Vec::new();
        for _ in 0..nobj {
            let o = ObjM { ignored: g.below(5) == 0, sold: g.below(5) == 0, set: if g.below(6) == 0 { 0 } else { 1 + g.below(nset as u32) }, portrait: g.below(4) };
            input.extend([u32::from(o.ignored), u32::from(o.sold), o.set, o.portrait]);
            objs.push(o);
        }
        let nsel = 1 + g.below(6);
        let selected: Vec<u32> = (0..nsel).map(|_| g.below(nobj as u32)).collect();
        input.push(nsel);
        input.extend(selected.iter().copied());
        let slots: Vec<Option<usize>> = (0..18).map(|i| (mask >> i & 1 == 1).then_some(order[i])).collect();
        let mut host = Host { log: Vec::new(), order: order.clone(), slots, gdata, common, cmds, sets, objs, selected };
        let steps = 1 + g.below(10);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let kind = g.below(6);
            let (kind, a, b) = match kind {
                0 => (0, 0, 0),
                1 | 5 => (1, 0, 0),
                2 => (2, g.below(nobj as u32), g.below(2)),
                3 => (3, 0, 0),
                _ => (4, g.below(nobj as u32), 1 + g.below(6)),
            };
            input.extend([kind, a, b]);
            host.log.clear();
            match kind {
                0 => populate_multi_select(&mut wm, &mut host),
                1 => update_context_multi_select(&mut wm, &mut host),
                2 => add_common_commands(&mut wm, &mut host, a, b != 0),
                3 => reset_common_command_data(&mut wm, &mut host),
                _ => host.selected = (0..b).map(|i| (a + i) % nobj as u32).collect(),
            }
            rust.extend(host.log.iter().copied());
            rust.push(100);
            put_state(&wm, &order, &mut rust);
            rust.extend(host.gdata.iter().copied());
            rust.extend(host.common.iter().map(|c| c.map_or(0, |t| t as u32)));
        }
        let mut buf = vec![0u32; 1 << 16];
        let len = unsafe { ref_ui3b_msel_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
