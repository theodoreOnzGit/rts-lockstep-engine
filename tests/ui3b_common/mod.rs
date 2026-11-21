// SPDX-License-Identifier: GPL-3.0-only
//! Shared pieces of the ui3b differential tests (#215): the scene of test windows the C++ stand-ins mirror
//! (differential/shim_ui3b/ui3b_stubs.h), the name keys, and the dump of the windows' state.
#![allow(dead_code)]

use rts_lockstep_engine::client::window_manager::{gwm, GameWindowManager, MsgHandled, SystemFunc};
use rts_lockstep_engine::client::window_script::{win_status, WinId};

/// `NameKeyGenerator::nameToKey` stand-in (the wrappers' `keyOf`): FNV-1a of the name, 31 bits.
pub fn fnv(s: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &c in s {
        h ^= u32::from(c);
        h = h.wrapping_mul(16777619);
    }
    h
}
pub fn key_of(s: &[u8]) -> i32 {
    (fnv(s) & 0x7FFF_FFFF) as i32
}

/// The name of the system callback the scene's windows have: it answers `GWM_INPUT_FOCUS` (see [`focus_system`]).
pub const SYSTEM: &[u8] = b"ui3b";

pub struct NoHost;
impl rts_lockstep_engine::client::window_manager::GuiHost for NoHost {}

/// One window of a scene.
#[derive(Clone, Copy, Debug)]
pub struct WinSpec {
    pub id: i32,
    pub hidden: bool,
    pub enabled: bool,
    pub no_focus: bool,
    /// Other `WIN_STATUS_*` bits the window has (never `HIDDEN`, `ENABLED`, `NO_FOCUS` or `DESTROYED`).
    pub extra: u32,
}

impl WinSpec {
    /// The window's whole status word.
    pub fn status(&self) -> u32 {
        (if self.hidden { win_status::HIDDEN } else { 0 }) | (if self.enabled { win_status::ENABLED } else { 0 }) | (if self.no_focus { win_status::NO_FOCUS } else { 0 }) | self.extra
    }
}

/// Creates the windows of `specs` (top-level, in the window list's order the C++ table is given) with the host system callback.
/// Returns the manager and the windows in list order (`order[i]` is table entry `i`).
pub fn make_scene(specs: &[WinSpec]) -> (GameWindowManager, Vec<WinId>) {
    let mut wm = GameWindowManager::new((800, 600));
    let mut h = NoHost;
    let mut created = Vec::new();
    for s in specs.iter().rev() {
        // win_create puts a window at the head of the list: create in reverse so that the list order is `specs`' order
        let w = wm.win_create(&mut h, None, 0, 0, 0, 10, 10, SystemFunc::Default, None);
        wm.tree.windows[w].id = s.id;
        wm.tree.windows[w].status = s.status();
        wm.ext[w].system = SystemFunc::Host(SYSTEM.to_vec());
        created.push(w);
    }
    let order = wm.window_list.clone();
    assert_eq!(order.len(), specs.len());
    for (i, &w) in order.iter().enumerate() {
        assert_eq!(wm.tree.windows[w].id, specs[i].id, "the window list's order");
    }
    (wm, order)
}

/// The host's reply to `GWM_INPUT_FOCUS` (the wrappers' `winSetFocus` logs `[900, id, d1]` and the window wants the focus).
pub fn focus_system(log: &mut Vec<u32>, wm: &mut GameWindowManager, win: WinId, msg: u32, d1: u32) -> MsgHandled {
    if msg == gwm::INPUT_FOCUS {
        log.push(900);
        log.push(wm.tree.windows[win].id as u32);
        log.push(d1);
        if d1 == 1 {
            wm.focus_reply = true;
        }
        return MsgHandled::Handled;
    }
    MsgHandled::Ignored
}

/// Appends every window's status word and the focused window (table index + 1, 0 for none).
pub fn put_state(wm: &GameWindowManager, order: &[WinId], out: &mut Vec<u32>) {
    for &w in order {
        out.push(wm.tree.windows[w].status);
    }
    out.push(wm.keyboard_focus.map_or(0, |f| order.iter().position(|&x| x == f).unwrap() as u32 + 1));
}

pub fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

pub fn cases(default: usize) -> usize {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Ends a test: a planted run prints the `MEASURED` line (the mismatch count N is the plant caught) and fails on purpose; a normal run
/// prints the `DIFF` line and fails on any mismatch.
pub fn finish(c: super::support::Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

/// A scene's words for the C++: `[nwin, nwin x [id, status word], focus index + 1]`, and sets the manager's initial focus.
pub fn put_scene(wm: &mut GameWindowManager, order: &[WinId], specs: &[WinSpec], focus: u32, input: &mut Vec<u32>) {
    input.push(specs.len() as u32);
    for s in specs {
        input.extend([s.id as u32, s.status()]);
    }
    input.push(focus);
    wm.keyboard_focus = if focus == 0 { None } else { Some(order[focus as usize - 1]) };
}

/// A random scene of `n` windows whose ids come from `ids` (duplicates possible), random hidden, enabled and no-focus bits.
pub fn random_specs(g: &mut super::support::Gen, n: usize, ids: &[i32]) -> Vec<WinSpec> {
    (0..n).map(|_| WinSpec { id: ids[g.below(ids.len() as u32) as usize], hidden: g.below(3) == 0, enabled: g.below(3) != 0, no_focus: g.below(5) == 0, extra: 0 }).collect()
}

/// Where two word lists first differ, with their neighbours (for the first mismatches printed).
pub fn diff_note(rust: &[u32], cpp: &[u32]) -> String {
    let n = rust.len().min(cpp.len());
    let i = (0..n).find(|&i| rust[i] != cpp[i]).unwrap_or(n);
    let lo = i.saturating_sub(12);
    format!("first difference at word {i} (rust {} words, c++ {} words): rust {:?} vs c++ {:?};", rust.len(), cpp.len(), &rust[lo..(i + 8).min(rust.len())], &cpp[lo..(i + 8).min(cpp.len())])
}
