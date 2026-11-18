#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarOCLTimer.cpp
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBarUnderConstruction.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::ui::control_bar_ocl_timer` and `control_bar_under_construction` against upstream's (#215).
//!
//! # Method
//!
//! The six `ControlBar` methods (`updateOCLTimerTextDisplay`, `populateOCLTimer`, `updateContextOCLTimer`,
//! `updateConstructionTextDisplay`, `populateUnderConstruction`, `updateContextUnderConstruction`) are cut verbatim by build.rs
//! into `differential/wrap_draft_ctl_timers.cpp`, which lists its stand-ins (the name keys, the window table, the text, the
//! objects and the control bar's own helpers; each logs what the method asks of it). The Rust side runs the same sequence on the
//! draft with a mock host (this file) and a real `GameWindowManager` holding the same windows, and logs the same words. Each
//! sequence: 1 to 6 windows in a random order (the six named windows and decoys, duplicates included, so the parent argument of
//! the lookup matters), 1 to 4 objects (tech building, auto rally point, exit interface, under construction, frames left, the two
//! percents; frames at the second boundaries 29/30/59/60/599/600, 0, and random up to `u32::MAX`; percents at the
//! rounding edges `k/100`, 0.005, 0.995 and random), and 1 to 24 steps of the six calls and of changes to the objects. After
//! every step the calls logged and the displayed seconds, displayed percent and every window's status bits are compared.
//!
//! Excluded (upstream dereferences NULL, or the input is outside `Int`): a missing sell or cancel window and no selected object
//! (a parent chosen so the sell and cancel buttons are found); a percent whose `percent * 100` is NaN or outside `Int`
//! (the float-to-`Int` conversion is undefined in C++, saturating in Rust) in the OCL timer's progress; the draft's `as i32` and
//! `cvttss2si` agree for the rest. The OCL timer's `Int seconds = totalSeconds - minutes * 60` is the same wrapping arithmetic
//! on both sides for the whole `u32` range (the release profile here has no overflow checks: `Cargo.toml` has no
//! `[profile.release]`). The game's own text formatting, the window manager's windowHiding and the gadget messages are the host's
//! and not under test; nor are `ControlBar::findCommandButton`, `setControlCommand`, `showRallyPoint`, `setPortraitByObject`
//! and `evaluateContextUI`, which are logged.
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! 0 mismatches over 20,000 sequences: the drafts are line-for-line. The least sure: `(percent * 100.0) as i32` against C++'s
//! `Real * Int` converted to `Int` (both single precision products, truncated).
//!
//! # Results
//!
//! (filled in below after the run)
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose): 12661 padding up to and including 10 seconds; 12662
//! progress rounded; 12663 progress computed in `f64`; 12664 frames to seconds rounded up; 12665 the timer redrawn only when the
//! seconds went up; 12666 a rally-point building counts as a tech building in the sell branch; 12667 the sell branch looks up
//! `Command_SetRallyPoint`; 12668 the displayed seconds not recorded; 12669 the rally branch for any auto rally point building;
//! 12670 the rally point not shown; 12671 the displayed construct percent not recorded; 12672 no return after
//! `evaluateContextUI`; 12673 the construct text redrawn only when the percent rose; 12674 no overlay states on the cancel
//! button; 12675 the construction rally point not shown; 12676 portrait before the text (under construction); 12677 the OCL
//! context update after the portrait.

mod support;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost, SystemFunc};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::control_bar_ocl_timer::{populate_ocl_timer, update_context_ocl_timer, update_ocl_timer_text_display, OclTimerHost};
use rts_lockstep_engine::drafts::ui::control_bar_under_construction::{populate_under_construction, update_construction_text_display, update_context_under_construction, UnderConstructionHost};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_ctl_timers_seq(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

/// The stand-in `nameToKey` (the wrapper's `keyOf`): FNV-1a, 31 bits.
fn fnv(s: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &c in s {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    h
}
fn key_of(s: &[u8]) -> i32 {
    (fnv(s) & 0x7FFF_FFFF) as i32
}
fn put_i32(v: &mut Vec<u16>, x: i32) {
    let u = x as u32;
    v.push((u & 0xFFFF) as u16);
    v.push((u >> 16) as u16);
}
fn put_f64(v: &mut Vec<u16>, x: f64) {
    let u = x.to_bits();
    for i in 0..4 {
        v.push(((u >> (16 * i)) & 0xFFFF) as u16);
    }
}

#[derive(Clone, Debug)]
struct Obj {
    tech: bool,
    auto_rally: bool,
    exit: bool,
    under_construction: bool,
    frames: u32,
    ocl_pct: f32,
    cons_pct: f32,
}

struct Host {
    log: Vec<u32>,
    parent_ocl: Option<WinId>,
    parent_uc: Option<WinId>,
    buttons: bool,
    objs: Vec<Obj>,
    selected: usize,
    disp_secs: u32,
    disp_pct: f32,
}

impl GuiHost for Host {}

impl Host {
    fn token(wm: &GameWindowManager, w: WinId) -> u32 {
        wm.window_list.iter().position(|&x| x == w).expect("a top-level window") as u32 + 1
    }
    fn button(&self, name: &[u8]) -> Option<usize> {
        if !self.buttons {
            return None;
        }
        Some(match name {
            b"Command_Sell" => 1,
            b"Command_SetRallyPoint" => 2,
            _ => 3,
        })
    }
    fn set_text(&mut self, wm: &GameWindowManager, w: WinId, text: &[u16]) {
        self.log.push(1);
        self.log.push(Self::token(wm, w));
        self.log.push(text.len() as u32);
        self.log.extend(text.iter().map(|&x| u32::from(x)));
    }
    fn set_command(&mut self, wm: &GameWindowManager, w: WinId, command: Option<usize>) {
        self.log.push(3);
        self.log.push(Self::token(wm, w));
        self.log.push(command.map_or(0, |c| c as u32));
    }
    fn rally(&mut self, obj: u32) -> bool {
        let has = self.objs[obj as usize].exit;
        if has {
            self.log.push(4);
        }
        has
    }
}

impl OclTimerHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn context_parent_ocl_timer(&self) -> Option<usize> {
        self.parent_ocl
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        vec![(fnv(label) & 0xFFFF) as u16]
    }
    fn format_minutes_seconds(&self, fmt: &[u16], minutes: i32, seconds: i32) -> Vec<u16> {
        let mut v = fmt.to_vec();
        put_i32(&mut v, minutes);
        put_i32(&mut v, seconds);
        v
    }
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: usize, text: &[u16]) {
        self.set_text(wm, w, text);
    }
    fn gadget_progress_bar_set_progress(&mut self, wm: &mut GameWindowManager, w: usize, progress: i32) {
        self.log.push(2);
        self.log.push(Self::token(wm, w));
        self.log.push(progress as u32);
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        self.button(name)
    }
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>) {
        self.set_command(wm, w, command);
    }
    fn is_tech_building(&self, obj: u32) -> bool {
        self.objs[obj as usize].tech
    }
    fn is_auto_rallypoint(&self, obj: u32) -> bool {
        self.objs[obj as usize].auto_rally
    }
    fn show_rally_point_of_exit(&mut self, obj: u32) -> bool {
        self.rally(obj)
    }
    fn set_portrait_by_object(&mut self, obj: u32) {
        self.log.push(5);
        self.log.push(obj);
    }
    fn current_selected_object(&self) -> Option<u32> {
        Some(self.selected as u32)
    }
    fn ocl_update_remaining_frames(&self, obj: u32) -> u32 {
        self.objs[obj as usize].frames
    }
    fn ocl_update_countdown_percent(&self, obj: u32) -> f32 {
        self.objs[obj as usize].ocl_pct
    }
    fn displayed_ocl_timer_seconds(&self) -> u32 {
        self.disp_secs
    }
    fn set_displayed_ocl_timer_seconds(&mut self, s: u32) {
        self.disp_secs = s;
    }
}

impl UnderConstructionHost for Host {
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn context_parent_under_construction(&self) -> Option<usize> {
        self.parent_uc
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        vec![(fnv(label) & 0xFFFF) as u16]
    }
    fn format_percent(&self, fmt: &[u16], percent: f32) -> Vec<u16> {
        let mut v = fmt.to_vec();
        put_f64(&mut v, f64::from(percent));
        v
    }
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: usize, text: &[u16]) {
        self.set_text(wm, w, text);
    }
    fn find_command_button(&self, name: &[u8]) -> Option<usize> {
        self.button(name)
    }
    fn set_control_command(&mut self, wm: &mut GameWindowManager, w: usize, command: Option<usize>) {
        self.set_command(wm, w, command);
    }
    fn set_portrait_by_object(&mut self, obj: u32) {
        self.log.push(5);
        self.log.push(obj);
    }
    fn construction_percent(&self, obj: u32) -> f32 {
        self.objs[obj as usize].cons_pct
    }
    fn show_rally_point_of_exit(&mut self, obj: u32) -> bool {
        self.rally(obj)
    }
    fn current_selected_object(&self) -> Option<u32> {
        Some(self.selected as u32)
    }
    fn is_under_construction(&self, obj: u32) -> bool {
        self.objs[obj as usize].under_construction
    }
    fn displayed_construct_percent(&self) -> f32 {
        self.disp_pct
    }
    fn set_displayed_construct_percent(&mut self, p: f32) {
        self.disp_pct = p;
    }
    fn evaluate_context_ui(&mut self, _wm: &mut GameWindowManager) {
        self.log.push(6);
    }
}

const NAMES: [&[u8]; 5] = [
    b"ControlBar.wnd:OCLTimerStaticText",
    b"ControlBar.wnd:OCLTimerProgressBar",
    b"ControlBar.wnd:OCLTimerSellButton",
    b"ControlBar.wnd:UnderConstructionDesc",
    b"ControlBar.wnd:ButtonCancelConstruction",
];

/// A percent in `Real * 100` -> `Int` range: the rounding edges and random.
fn pct_in_range(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => 0.0,
        1 => 1.0,
        2 => g.below(101) as f32 / 100.0,
        3 => [0.005f32, 0.995, 0.5, 0.29, 0.57, 0.58, 0.07][g.below(7) as usize],
        4 => g.uniform(-30.0, 30.0),
        _ => g.uniform(0.0, 1.0),
    }
}
/// A construction percent: anything (it is only printed), NaN and infinities included.
fn pct_any(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => f32::from_bits(0x7FC0_0000),
        1 => g.mixed(),
        2 => 0.0,
        3 => 100.0,
        4 => g.below(101) as f32,
        _ => g.uniform(0.0, 100.0),
    }
}
fn frames(g: &mut Gen) -> u32 {
    match g.below(12) {
        0 => 0,
        1 => [29u32, 30, 31, 59, 60, 61, 89, 90][g.below(8) as usize],
        2 => [599u32, 600, 601, 1799, 1800, 1801, 1770, 1769][g.below(8) as usize],
        3 => g.u32(),
        4 => u32::MAX - g.below(100),
        5 => 30 * (g.below(5000) + 1) + g.below(2) * 29,
        6 => 30 * 60 * g.below(100) + 30 * (g.below(3) * 9 + 1),
        _ => g.below(20_000),
    }
}
fn seconds(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => g.below(12),
        1 => [59u32, 60, 61, 69, 70, 599, 600, 601, 3599, 3600, 3601][g.below(11) as usize],
        2 => g.u32(),
        3 => u32::MAX - g.below(70),
        4 => 60 * g.below(1000) + g.below(61),
        _ => g.below(700),
    }
}

struct Case {
    input: Vec<u32>,
    rust: Vec<u32>,
}

fn make_case(g: &mut Gen) -> Case {
    // windows
    let nwin = 1 + g.below(6) as usize;
    let mut kinds: Vec<i32> = Vec::new();
    // sell (2) and cancel (4) at least once
    kinds.push(2);
    kinds.push(4);
    while kinds.len() < nwin.max(2) {
        kinds.push(g.below(6) as i32); // 5 = a decoy
    }
    for i in (1..kinds.len()).rev() {
        let j = g.below(i as u32 + 1) as usize;
        kinds.swap(i, j);
    }
    let mut host = Host { log: Vec::new(), parent_ocl: None, parent_uc: None, buttons: g.below(8) != 0, objs: Vec::new(), selected: 0, disp_secs: 0, disp_pct: 0.0 };
    let mut wm = GameWindowManager::new((800, 600));
    let mut created: Vec<WinId> = Vec::new();
    for &k in &kinds {
        let w = wm.win_create(&mut host, None, 0, 0, 0, 10, 10, SystemFunc::Default, None);
        wm.tree.windows[w].id = if k == 5 { 5000 + g.below(1000) as i32 } else { key_of(NAMES[k as usize]) };
        wm.tree.windows[w].status = 0;
        created.push(w);
    }
    let order: Vec<WinId> = wm.window_list.clone();
    // parents: a window at which the sell (or cancel) button is still found, or none
    let parent = |g: &mut Gen, key: i32| -> Option<WinId> {
        if g.below(3) == 0 {
            return None;
        }
        let p = order[g.below(order.len() as u32) as usize];
        if wm.win_get_window_from_id(Some(p), key).is_some() {
            Some(p)
        } else {
            None
        }
    };
    host.parent_ocl = parent(g, key_of(NAMES[2]));
    host.parent_uc = parent(g, key_of(NAMES[4]));

    // objects
    let nobj = 1 + g.below(4) as usize;
    for _ in 0..nobj {
        host.objs.push(Obj {
            tech: g.below(2) == 0,
            auto_rally: g.below(2) == 0,
            exit: g.below(2) == 0,
            under_construction: g.below(2) == 0,
            frames: frames(g),
            ocl_pct: pct_in_range(g),
            cons_pct: pct_any(g),
        });
    }
    host.selected = g.below(nobj as u32) as usize;
    host.disp_secs = if g.below(3) == 0 { host.objs[host.selected].frames / 30 } else { seconds(g) };
    host.disp_pct = if g.below(3) == 0 { host.objs[host.selected].cons_pct } else { pct_any(g) };

    let mut input: Vec<u32> = vec![order.len() as u32];
    for &w in &order {
        input.push(wm.tree.windows[w].id as u32);
    }
    let idx = |p: Option<WinId>| p.map_or(0xFFFF_FFFF, |w| order.iter().position(|&x| x == w).unwrap() as u32);
    input.push(idx(host.parent_ocl));
    input.push(idx(host.parent_uc));
    input.push(u32::from(host.buttons));
    input.push(nobj as u32);
    for o in &host.objs {
        input.push(u32::from(o.tech) | u32::from(o.auto_rally) << 1 | u32::from(o.exit) << 2 | u32::from(o.under_construction) << 3);
        input.push(o.frames);
        input.push(o.ocl_pct.to_bits());
        input.push(o.cons_pct.to_bits());
    }
    input.push(host.selected as u32);
    input.push(host.disp_secs);
    input.push(host.disp_pct.to_bits());

    let nsteps = 1 + g.below(24) as usize;
    input.push(nsteps as u32);
    let mut rust: Vec<u32> = Vec::new();
    for _ in 0..nsteps {
        let kind = g.below(11);
        let obj = g.below(nobj as u32);
        let opt_obj = if g.below(10) == 0 { None } else { Some(obj) };
        let (a, b): (u32, u32) = match kind {
            0 => {
                let s = if g.below(3) == 0 { host.objs[g.below(nobj as u32) as usize].frames / 30 } else { seconds(g) };
                (s, pct_in_range(g).to_bits())
            }
            1 | 4 => (opt_obj.unwrap_or(0xFFFF_FFFF), 0),
            2 | 5 => (0, 0),
            3 => (obj, 0),
            6 => (obj, frames(g)),
            7 => (obj, pct_in_range(g).to_bits()),
            8 => (obj, pct_any(g).to_bits()),
            9 => (obj, g.below(2)),
            _ => (obj, 0),
        };
        input.push(kind);
        input.push(a);
        input.push(b);
        host.log.clear();
        match kind {
            0 => update_ocl_timer_text_display(&mut wm, &mut host, a, f32::from_bits(b)),
            1 => populate_ocl_timer(&mut wm, &mut host, (a != 0xFFFF_FFFF).then_some(a)),
            2 => update_context_ocl_timer(&mut wm, &mut host),
            3 => update_construction_text_display(&mut wm, &mut host, a),
            4 => populate_under_construction(&mut wm, &mut host, (a != 0xFFFF_FFFF).then_some(a)),
            5 => update_context_under_construction(&mut wm, &mut host),
            6 => host.objs[a as usize].frames = b,
            7 => host.objs[a as usize].ocl_pct = f32::from_bits(b),
            8 => host.objs[a as usize].cons_pct = f32::from_bits(b),
            9 => host.objs[a as usize].under_construction = b != 0,
            _ => host.selected = a as usize,
        }
        rust.extend(host.log.iter().copied());
        rust.push(100);
        rust.push(host.disp_secs);
        rust.push(host.disp_pct.to_bits());
        for &w in &order {
            rust.push(wm.tree.windows[w].status);
        }
    }
    let _ = created;
    Case { input, rust }
}

#[test]
fn ocl_timer_and_under_construction() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n: usize = std::env::var("CTL_TIMERS_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x215_C71A);
    let mut check = Check::new("ControlBar OCL timer + under construction");
    let mut buf = vec![0u32; 1 << 16];
    for _ in 0..n {
        let c = make_case(&mut g);
        let len = unsafe { ref_ctl_timers_seq(c.input.as_ptr(), c.input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        check.eq(c.rust, cpp, || format!("input {:?}", c.input));
    }
    if planted() {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        check.finish();
    }
}
