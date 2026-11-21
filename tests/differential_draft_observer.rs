// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/control_bar_observer.rs` (`init_observer_controls`, `control_bar_observer_system`,
//! `populate_observer_list`, `populate_observer_info_window`) against upstream `ControlBarObserver.cpp`, cut verbatim by build.rs (`drafts_ui3b`;
//! `differential/wrap_draft_observer.cpp`, whose stand-ins, the window manager's functions cut verbatim from GameWindowManager.cpp / GameWindow.cpp among them, and exclusions
//! are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). A window tree with every window the bar needs (the info window and the list window, under the list window its eight player
//! buttons and eight texts, the seven info texts, the flag and the portrait) plus up to 6 decoys with the same ids at random places (so a lookup from the list window, which scans the
//! list window, its children and then its later siblings, may find a different window than a lookup from the top), each with random hidden, enabled bits; 0 to 6 players (observer or
//! not, human or not, a colour, image tokens, a name of 0 to 4 units, kills, losses) put in the game slots as `player0` to `player7` (repeats allowed), 8 slots (team -1 to 7, AI or not),
//! multiplayer or not; 1 to 12 steps of `initObserverControls`, `ControlBarObserverSystem` (create, mouse enter and leave, `GBM_SELECTED` and `GBM_SELECTED_RIGHT` on a random window,
//! others), `populateObserverList`, `populateObserverInfoWindow`, a change of the player being looked at and of the multiplayer flag. The last three kinds are used only after an
//! init (upstream dereferences the windows `initObserverControls` finds). After every step the calls logged (static texts, button images, tooltips, text colours, the flag image), the
//! result, every window's status word and button data, the player looked at and all the statics are compared. The draft runs on the verified `GameWindowManager`.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The likeliest miss: the draft's `fill_button` order against upstream's (it does the text colour and hide after `winSetStatus`, upstream computes the slot between).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21241 | the buttons are looked up from the top of the window list, not from the list window |
//! | 21242 | Cancel does not repopulate the list |
//! | 21243 | Cancel shows the info window and hides the list |
//! | 21244 | a player button does not populate the info window |
//! | 21245 | the team shown is the team number, not number + 1 |
//! | 21246 | team 0 of an AI is shown as `Team:AI` |
//! | 21247 | the single-player list keeps filling after the first human |
//! | 21248 | the building count leaves out the destroy-score buildings |
//! | 21249 | the kills shown are the losses |
//! | 21250 | the info window is populated only while it is hidden |
//! | 21251 | the info window's hidden test reads `isHidden` (the window or an ancestor), not `winIsHidden` (the draft's original defect, reinstated) |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences** after the fixes noted here, max ULP 0 (nothing is floating point in the comparison, which is of integer words). The first run crashed (SIGSEGV) in the C++, a harness input error: a decoy window with the list window's id made `ObserverPlayerListWindow` a decoy, whose children do not hold the buttons, and upstream dereferences the NULL it then finds (excluded: no decoy has that id any more). The second run showed **127 mismatches**, all in `populateObserverInfoWindow`: the draft read `ObserverPlayerInfoWindow->winIsHidden()` (the window's own hidden bit) as `GameWindowManager::isHidden` (the window or an ancestor hidden). **Draft fixed**, then **0 mismatches**. Prediction (0) missed once.
//! Planted bugs (of 20,000): 21241 7,617; 21242 28; 21243 33; 21244 387; 21245 5,899; 21246 1,410; 21247 2,393; 21248 1,251; 21249 1,245; 21250 11,769; 21251 127 (21251 is the draft's original defect, reinstated). **11 of 11 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{ggm, gwm, win_data, GameWindowManager, GuiHost, MsgHandled};
use rts_lockstep_engine::client::window_script::WinId;
use rts_lockstep_engine::drafts::ui::control_bar_observer::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_obs_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Pl {
    observer: bool,
    human: bool,
    color: i32,
    enabled: u32,
    flag: u32,
    name: Vec<u16>,
    killed: i32,
    lost: i32,
}

struct Host {
    log: Vec<u32>,
    obs: ControlBarObserver,
    players: Vec<Pl>,
    slot_player: [u32; 8],
    slots: [(i32, bool); 8],
    multi: bool,
    look: Option<usize>,
    gdata: Vec<u32>,
}

impl Host {
    fn text(&mut self, wm: &GameWindowManager, w: WinId, text: &[u16]) {
        self.log.extend([3, wm.tree.windows[w].id as u32, text.len() as u32]);
        self.log.extend(text.iter().map(|&u| u32::from(u)));
    }
}

impl GuiHost for Host {}

fn fetch(label: &[u8]) -> Vec<u16> {
    vec![(fnv(label) & 0xFFFF) as u16, label.len() as u16]
}

impl ObserverHost for Host {
    fn observer(&mut self) -> &mut ControlBarObserver {
        &mut self.obs
    }
    fn name_to_key(&mut self, name: &[u8]) -> i32 {
        key_of(name)
    }
    fn is_multiplayer(&self) -> bool {
        self.multi
    }
    fn find_player_with_name_key(&mut self, name: &[u8]) -> Option<usize> {
        let i = (name[6] - b'0') as usize;
        assert!(name.starts_with(b"player") && name.len() == 7);
        (self.slot_player[i] != 0).then(|| self.slot_player[i] as usize - 1)
    }
    fn get_nth_player(&self, i: usize) -> Option<usize> {
        (i < self.players.len()).then_some(i)
    }
    fn is_player_observer(&self, p: usize) -> bool {
        self.players[p].observer
    }
    fn is_player_human(&self, p: usize) -> bool {
        self.players[p].human
    }
    fn gadget_button_set_data_player(&mut self, _wm: &mut GameWindowManager, w: WinId, p: usize) {
        self.gdata[w] = p as u32 + 1;
    }
    fn gadget_button_get_data_player(&self, _wm: &GameWindowManager, w: WinId) -> Option<usize> {
        (self.gdata[w] != 0).then(|| self.gdata[w] as usize - 1)
    }
    fn gadget_button_set_enabled_image_player(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize) {
        self.log.extend([5, wm.tree.windows[w].id as u32, self.players[p].enabled]);
    }
    fn win_set_tooltip_player_name(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize) {
        let name = self.players[p].name.clone();
        self.log.extend([6, wm.tree.windows[w].id as u32, name.len() as u32]);
        self.log.extend(name.iter().map(|&u| u32::from(u)));
    }
    fn player_color(&self, p: usize) -> i32 {
        self.players[p].color
    }
    fn make_color(&self, r: u8, g: u8, b: u8, a: u8) -> i32 {
        ((u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)) as i32
    }
    fn win_set_enabled_text_colors(&mut self, wm: &mut GameWindowManager, w: WinId, color: i32, border: i32) {
        self.log.extend([7, wm.tree.windows[w].id as u32, color as u32, border as u32]);
    }
    fn gadget_static_text_set_text(&mut self, wm: &mut GameWindowManager, w: WinId, text: &[u16]) {
        self.text(wm, w, text);
    }
    fn player_display_name(&self, p: usize) -> Vec<u16> {
        self.players[p].name.clone()
    }
    fn slot_team_number(&self, i: usize) -> i32 {
        self.slots[i].0
    }
    fn slot_is_ai(&self, i: usize) -> bool {
        self.slots[i].1
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        fetch(label)
    }
    fn format_obs_player_label(&self, name: &[u16], team: &[u16]) -> Vec<u16> {
        let mut v = fetch(b"CONTROLBAR:ObsPlayerLabel");
        v.extend_from_slice(name);
        v.push(0xFFFF);
        v.extend_from_slice(team);
        v.push(0xFFFF);
        v
    }
    fn observer_look_at_player(&self) -> Option<usize> {
        self.look
    }
    fn set_observer_look_at_player(&mut self, p: Option<usize>) {
        self.look = p;
    }
    fn count_objects(&self, p: usize, set: &[&str], clear: &[&str]) -> i32 {
        let bit = |names: &[&str]| {
            names.iter().fold(0u32, |acc, n| {
                acc | 1
                    << match *n {
                        "SCORE" => 0,
                        "STRUCTURE" => 1,
                        "SCORE_CREATE" => 2,
                        "SCORE_DESTROY" => 3,
                        _ => panic!("an unknown kind-of"),
                    }
            })
        };
        let mut h: u32 = 2166136261;
        h ^= p as u32;
        h = h.wrapping_mul(16777619);
        h ^= bit(set);
        h = h.wrapping_mul(16777619);
        h ^= bit(clear);
        h = h.wrapping_mul(16777619);
        ((h >> 5) % 90) as i32
    }
    fn total_units_destroyed(&self, p: usize) -> i32 {
        self.players[p].killed
    }
    fn total_units_lost(&self, p: usize) -> i32 {
        self.players[p].lost
    }
    fn win_set_flag_image(&mut self, wm: &mut GameWindowManager, w: WinId, p: usize) {
        self.log.extend([8, wm.tree.windows[w].id as u32, self.players[p].flag]);
    }
}

#[test]
fn observer_bar_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_0D);
    let mut chk = Check::new("ControlBar observer");
    let info_key = key_of(b"ControlBar.wnd:ObserverPlayerInfoWindow");
    let list_key = key_of(b"ControlBar.wnd:ObserverPlayerListWindow");
    let button_keys: Vec<i32> = (0..8).map(|i| key_of(format!("ControlBar.wnd:ButtonPlayer{i}").as_bytes())).collect();
    let text_keys: Vec<i32> = (0..8).map(|i| key_of(format!("ControlBar.wnd:StaticTextPlayer{i}").as_bytes())).collect();
    let info_text_keys: Vec<i32> = ["StaticTextNumberOfUnits", "StaticTextNumberOfBuildings", "StaticTextNumberOfUnitsKilled", "StaticTextNumberOfUnitsLost", "StaticTextPlayerName", "WinFlag", "WinGeneralPortrait"]
        .iter()
        .map(|n| key_of(format!("ControlBar.wnd:{n}").as_bytes()))
        .collect();
    let cancel_key = key_of(b"ControlBar.wnd:ButtonCancel");
    let mut pool: Vec<i32> = button_keys.clone();
    pool.extend(text_keys.iter().copied());
    pool.extend(info_text_keys.iter().copied());
    // (no decoy has the list window's id: a lookup from a decoy list window would not reach the buttons, and upstream dereferences the NULL it finds)
    pool.extend([info_key, cancel_key, 7001, 7002]);
    for _ in 0..cases(20_000) {
        // the tree: roots in a random order, the list window's children after it, then decoys
        let mut roots: Vec<i32> = vec![info_key, list_key];
        roots.extend(info_text_keys.iter().copied());
        for i in (1..roots.len()).rev() {
            let j = g.below(i as u32 + 1) as usize;
            roots.swap(i, j);
        }
        let mut ts: Vec<TreeSpec> = Vec::new();
        let mut list_idx = 0usize;
        let spec = |g: &mut Gen, id: i32| WinSpec { id, hidden: g.below(3) == 0, enabled: g.below(3) != 0, no_focus: false, extra: 0 };
        for &id in &roots {
            let s = spec(&mut g, id);
            ts.push(TreeSpec { spec: s, parent: None, unit_system: false });
            if id == list_key {
                list_idx = ts.len() - 1;
                let mut kids: Vec<i32> = button_keys.iter().chain(text_keys.iter()).copied().collect();
                for i in (1..kids.len()).rev() {
                    let j = g.below(i as u32 + 1) as usize;
                    kids.swap(i, j);
                }
                for k in kids {
                    let s = spec(&mut g, k);
                    ts.push(TreeSpec { spec: s, parent: Some(list_idx), unit_system: false });
                }
            }
        }
        let ndecoy = g.below(7);
        for _ in 0..ndecoy {
            let id = pool[g.below(pool.len() as u32) as usize];
            let s = spec(&mut g, id);
            let parent = if g.below(2) == 0 { None } else { Some(g.below(ts.len() as u32) as usize) };
            ts.push(TreeSpec { spec: s, parent, unit_system: false });
        }
        // (the ids of required windows are never changed; a decoy shares an id with one of them)
        let n = ts.len();
        let (mut wm, scene) = make_tree_scene(&ts, b"observer");
        let mut input = scene;
        let np = g.below(7) as usize;
        input.push(np as u32);
        let mut players = Vec::new();
        for _ in 0..np {
            let nl = g.below(5) as usize;
            let p = Pl { observer: g.below(4) == 0, human: g.below(2) == 0, color: g.u32() as i32, enabled: 1 + g.below(500), flag: 1 + g.below(500), name: (0..nl).map(|_| 1 + g.below(0xFFFE) as u16).collect(), killed: g.below(200) as i32 - 20, lost: g.below(200) as i32 - 20 };
            input.extend([u32::from(p.observer), u32::from(p.human), p.color as u32, p.enabled, p.flag, nl as u32]);
            input.extend(p.name.iter().map(|&u| u32::from(u)));
            input.extend([p.killed as u32, p.lost as u32]);
            players.push(p);
        }
        let mut slot_player = [0u32; 8];
        for s in slot_player.iter_mut() {
            *s = if np == 0 || g.below(4) == 0 { 0 } else { 1 + g.below(np as u32) };
        }
        input.extend(slot_player.iter().copied());
        let mut slots = [(0, false); 8];
        for s in slots.iter_mut() {
            *s = (g.below(9) as i32 - 1, g.below(2) == 0);
            input.extend([s.0 as u32, u32::from(s.1)]);
        }
        let multi = g.below(2) == 0;
        input.push(u32::from(multi));
        let mut host = Host { log: Vec::new(), obs: ControlBarObserver::default(), players, slot_player, slots, multi, look: None, gdata: vec![0; n] };
        let steps = 1 + g.below(12);
        let mut step_words: Vec<u32> = Vec::new();
        let mut rust = Vec::new();
        let mut inited = false;
        for _ in 0..steps {
            let kind = match g.below(9) {
                0 | 1 => 0,
                2 | 3 => 1,
                4 | 5 => 2,
                6 | 7 => 3,
                _ => 4 + g.below(2),
            };
            // populate and set-look-at only after an init
            let kind = if !inited && matches!(kind, 2 | 3) { 0 } else { kind };
            let (a, b, c) = match kind {
                1 => {
                    let msg = match g.below(6) {
                        0 => gwm::CREATE,
                        1 => ggm::GBM_MOUSE_ENTERING,
                        2 => ggm::GBM_MOUSE_LEAVING,
                        3 | 4 => [ggm::GBM_SELECTED, ggm::GBM_SELECTED_RIGHT][g.below(2) as usize],
                        _ => g.below(40),
                    };
                    let sel = msg == ggm::GBM_SELECTED || msg == ggm::GBM_SELECTED_RIGHT;
                    // the control: a player button, the cancel button (if present), or any window
                    let c = if sel { 1 + g.below(n as u32) } else { g.below(5) };
                    (g.below(n as u32), msg, c)
                }
                4 => (if np == 0 { 0 } else { g.below(np as u32 + 1) }, 0, 0),
                5 => (g.below(2), 0, 0),
                _ => (0, 0, 0),
            };
            let kind = if kind == 4 && np == 0 { 5 } else { kind };
            step_words.extend([kind, a, b, c]);
            host.log.clear();
            let mut res: i32 = -1;
            match kind {
                0 => {
                    init_observer_controls(&mut wm, &mut host);
                    inited = true;
                }
                1 => {
                    let d1 = if b == ggm::GBM_SELECTED || b == ggm::GBM_SELECTED_RIGHT { win_data(Some(c as usize - 1)) } else { c };
                    res = i32::from(control_bar_observer_system(&mut wm, &mut host, a as usize, b, d1, 0) == MsgHandled::Handled);
                }
                2 => populate_observer_list(&mut wm, &mut host),
                3 => populate_observer_info_window(&mut wm, &mut host),
                4 => host.look = (a != 0).then(|| a as usize - 1),
                _ => host.multi = a != 0,
            }
            rust.extend(host.log.iter().copied());
            rust.extend([100, res as u32]);
            put_state_tree(&wm, n, &mut rust);
            let o = &host.obs;
            let idx = |w: Option<WinId>| w.map_or(0, |w| w as u32 + 1);
            rust.push(host.look.map_or(0, |p| p as u32 + 1));
            rust.extend([idx(o.observer_player_info_window), idx(o.observer_player_list_window), o.button_cancel_id as u32]);
            for i in 0..8 {
                rust.extend([o.button_player_id[i] as u32, idx(o.button_player[i]), o.static_text_player_id[i] as u32, idx(o.static_text_player[i])]);
            }
            rust.extend([idx(o.win_flag), idx(o.win_general_portrait), idx(o.static_text_number_of_units), idx(o.static_text_number_of_buildings)]);
            rust.extend([idx(o.static_text_number_of_units_killed), idx(o.static_text_number_of_units_lost), idx(o.static_text_player_name)]);
            rust.extend(host.gdata.iter().copied());
        }
        input.push(steps);
        input.extend(step_words);
        trace(&input);
        let mut buf = vec![0u32; 1 << 17];
        let len = unsafe { ref_ui3b_obs_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        let _ = list_idx;
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
