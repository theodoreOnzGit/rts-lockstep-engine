#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (populatePurchaseScience, updateContextPurchaseScience,
//   hideCommunicator, setControlBarSchemeByName, populateSpecialPowerShortcut, hasAnyShortcutSelection, updateSpecialPowerShortcut,
//   drawSpecialPowerShortcutMultiplierText)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::control_bar` against upstream's (#215).
//!
//! # Method
//!
//! Eight functions of `ControlBar.cpp` are cut verbatim (build.rs, `drafts_guilogic4`) into `differential/wrap_draft_cbar.cpp`, built on upstream's own
//! UnicodeString.cpp and AsciiString.cpp; the control bar's other members, the windows, the player, the command sets and buttons, the science store, the game text
//! and the window manager are stand-ins (listed in its header) that **log every call that changes something or asks something with an effect**: `hide 23 1`,
//! `setstatus 5 1000000`, `statictext 66 12`, `progress 67 40`, `setcmd 24 B7`, `avail B3 12 24 -1`, `copyimages B2 B9`, `showshortcut`, `animate 1`, ... The draft's
//! [`ControlBarWindowsHost`] is implemented over the same world and writes the same lines; the whole logs are compared, and after each case the state of all 71 windows
//! (hidden, status bits, the command a button holds). Pure queries (a set's button, a player's science) are answered from tables and not logged.
//!
//! 20,000 random worlds: a player with a template (set names, possibly empty), 8 command sets of 18 slots over up to 40 buttons (options with `NEED_UPGRADE`,
//! `NEED_SPECIAL_POWER_SCIENCE`, `SCRIPT_ONLY`; commands purchase science, select all units of a type, special power, other; 0 to 3 sciences; special power template
//! with a required science or none; upgrade template; thing template), the sciences -1..7 with each of disabled, hidden, held, prerequisites, root prerequisites, a cost,
//! points, skill points, rank, upgrades, objects found by type and thing, ready counts, a shortcut bar of 0 to 11 buttons (some without windows past the used ones), hidden
//! windows, a master window and shortcut parent that may be missing, the three named windows of the purchase screen that may be missing. Then 1 to 8 operations from the
//! eight functions, in any order.
//!
//! Excluded inputs (undefined upstream): a `getSkillPointsLevelUp()` equal to `getSkillPointsLevelDown()` (division by zero); skill values past 10^6 (the product by 100
//! overflows `Int`); a rank 8 purchase button with no science (`getScienceVec()[0]`); a "select all units of a type" command with no thing template; a missing window among
//! the first `m_currentlyUsedSpecialPowersButtons` shortcut buttons or their parents; `populateSpecialPowerShortcut` with a master window shown and no shortcut parent;
//! `updateContextPurchaseScience` with no local player. `m_specialPowerShortcutButtons == NULL` is never true (it is an array in ControlBar.h): the draft's
//! `shortcut_buttons_exist` is always true here.
//!
//! Not tested: `friend_add_to_list`, `new_command_button`, `new_command_set`, `new_command_set_override` (the real `CommandSet` and `CommandButton` classes are the
//! whole INI-driven command model; the draft's `CommandSetRec` is a name and a list), `get_marker_pos`, `update_purchase_science`, `update_command_marker_image`,
//! `draw_transition_handler` (empty or a field read), `process_context_sensitive_button_transition` (the verified `control_bar_command_processing`, tested in
//! `differential_draft_cbtrans.rs`).
//!
//! # Prediction (before the first run, 2026-11-20)
//!
//! From reading upstream, three mismatches: (1) `populateSpecialPowerShortcut`: the `bestIndex` search for the enhanced image sits in the `else` of "the player has the
//! required science" (when `getRequiredScience() != SCIENCE_INVALID`); the draft puts it in the `else` of `required != SCIENCE_INVALID` instead; (2)
//! `updateSpecialPowerShortcut`: `findMostReadyShortcutSpecialPowerForThing` is asked only when `findAnyExistingObjectWithThingTemplate` found an object; the draft asks
//! it either way; (3) `m_contextParent[CP_PURCHASE_SCIENCE] == NULL`: `winGetWindowFromId(NULL, ...)` searches every window upstream; the draft does nothing, and skips the
//! closing `updateContextPurchaseScience()`. Everything else: 0.
//!
//! # Results
//!
//! 2026-11-20: first run, 20,000 worlds, **2,158 mismatches**. Cause by cause (the draft fixed in control_bar.rs, the run repeated): (3) the missing parent screen, and
//! (4) not found by reading, `populateSpecialPowerShortcut` returns BEFORE hiding the buttons when the player has no template, the draft returned after (`!player->getPlayerTemplate()`
//! was missing from its first test): together 1,774 of them; then 384 remained, and with (1) the `bestIndex` search moved into the right branch and (2) the lookup nested
//! as upstream has it, **0 mismatches** over 20,000 worlds (about 11,000 operations of each of the eight kinds). Predictions (1), (2), (3) met; (4) was a miss of the
//! prediction. Planted bugs, 15 of 15 caught (mismatches of 20,000): 15905 350; 15906 275; 15907 1,088; 15908 2,287; 15909 2,287; 15910 7,208; 15911 1,870; 15912 2,293;
//! 15913 2,058; 15914 1,604; 15915 916; 15916 861; 15917 2,909; 15918 3,416; 15919 33 (the least: it needs a science chain with a gap before a held one).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 15905 rank 1 reads `SCIENCE_INVALID` for a button with no science (rank 3's way); 15906 the purchase test is `cost < points`; 15907 the `!hasScience`
//! guard of enabling is missing; 15908 the always-colour status is set when the science is NOT held; 15909 the root prerequisites test is inverted; 15910 the progress is
//! `(skill - down) / (up - down) * 100`; 15911 the title is `SCIENCE:Rank<rank+1>`; 15912 the script-only test reads `NEED_UPGRADE`; 15913 `hasAnyShortcutSelection` looks at
//! hidden buttons too; 15914 the multiplier text shows a count of 1; 15915 `COMMAND_CANT_AFFORD` sets the not-ready status; 15916 the upgrade test is inverted; 15917 the
//! shortcut button's parent is not enabled; 15918 the always-colour status is not cleared; 15919 the science chain search goes on after a gap.

mod support;

use rts_lockstep_engine::client::control_bar::gui;
use rts_lockstep_engine::drafts::sweep_gui_logic::control_bar::{self as cb, CommandAvailability, CommandButtonInfo, ControlBarWindowsHost};
use std::cell::RefCell;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_cbar_run(world: *const i64, nw: i32, ops: *const i64, nops: i32, log: *mut std::os::raw::c_char, logcap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const N_WIN: usize = 71;

#[derive(Clone)]
struct Button {
    options: u32,
    kind: u8, // 0 purchase science, 1 select all of a type, 2 special power, 3 other
    sciences: Vec<i32>,
    sp: Option<(i32, i32)>,
    upgrade: Option<u32>,
    thing: Option<i32>,
}

#[derive(Clone)]
struct World {
    n_used: usize,
    buttons_null: [bool; 11],
    parents_null: [bool; 11],
    ctx_purchase: bool,
    master: bool,
    shortcut_parent: bool,
    anim_finished: bool,
    communicator: bool,
    game_ending: bool,
    player: bool,
    template: bool,
    local: bool,
    active: bool,
    named: [bool; 3],
    hidden: Vec<bool>,
    shortcut_data: [Option<usize>; 11],
    purchase_names: [Option<usize>; 3],
    shortcut_name: Option<usize>,
    sets: Vec<Option<Vec<Option<usize>>>>,
    buttons: Vec<Button>,
    sci: [(bool, bool, bool, bool, bool, i32); 9], // held, disabled, hidden, prereqs, root prereqs, cost
    points: i32,
    skill: i32,
    down: i32,
    up: i32,
    rank: i32,
    upgrades: [bool; 8],
    find_type: [i32; 4],
    find_any: [i32; 5],
    find_thing: [i32; 5],
    has_any_shortcut: bool,
    count: [i32; 4],
    salt: i32,
}

fn coin(g: &mut Gen, percent: u32) -> bool {
    g.below(100) < percent
}

fn gen_world(g: &mut Gen) -> World {
    let nb = 12 + g.below(29) as usize;
    let half = nb / 2; // buttons 0..half may have no science, half.. always have
    let mut buttons = Vec::new();
    for b in 0..nb {
        let mut options = g.u32() & 0x00F7_FF3F; // not the three bits below
        for bit in [0x40u32, 0x80, 0x80000] {
            if coin(g, 35) {
                options |= bit;
            }
        }
        let kind = g.below(4) as u8;
        let ns = if b >= half { 1 + g.below(3) } else { g.below(4) } as usize;
        let mut sciences: Vec<i32> = (0..ns).map(|_| g.below(9) as i32 - 1).collect();
        if coin(g, 40) {
            sciences.sort(); // increasing, as an upgrade chain
        }
        let sp = coin(g, 60).then(|| (g.below(4) as i32, if coin(g, 40) { -1 } else { g.below(8) as i32 }));
        let upgrade = coin(g, 40).then(|| g.below(8));
        let thing = if kind == 1 || coin(g, 40) { Some(1 + g.below(4) as i32) } else { None };
        buttons.push(Button { options, kind, sciences, sp, upgrade, thing });
    }
    let n_used = g.below(12) as usize;
    let mut buttons_null = [false; 11];
    let mut parents_null = [false; 11];
    for i in n_used..11 {
        buttons_null[i] = coin(g, 30);
        parents_null[i] = coin(g, 30);
    }
    let mut hidden: Vec<bool> = (0..N_WIN).map(|_| coin(g, 40)).collect();
    hidden[45] = coin(g, 25); // the master window is mostly shown
    let pn: [Option<usize>; 3] = [0, 1, 2].map(|_| coin(g, 85).then(|| g.below(8) as usize));
    let mut sets: Vec<Option<Vec<Option<usize>>>> = Vec::new();
    for s in 0..8 {
        if coin(g, 80) {
            let mut slots: Vec<Option<usize>> = (0..18).map(|_| coin(g, 60).then(|| g.below(nb as u32) as usize)).collect();
            if pn[2] == Some(s) {
                // the rank 8 set: its four purchase slots hold buttons with a science (upstream reads getScienceVec()[0])
                for slot in slots.iter_mut().take(4) {
                    if let Some(b) = *slot {
                        if buttons[b].sciences.is_empty() {
                            *slot = Some(half + g.below((nb - half) as u32) as usize);
                        }
                    }
                }
            }
            sets.push(Some(slots));
        } else {
            sets.push(None);
        }
    }
    let down = g.below(2_000_001) as i32 - 1_000_000;
    let mut up = g.below(2_000_001) as i32 - 1_000_000;
    if up == down {
        up += 1;
    }
    let objs = |g: &mut Gen| if coin(g, 50) { 1 + g.below(63) as i32 } else { 0 };
    World {
        n_used,
        buttons_null,
        parents_null,
        ctx_purchase: coin(g, 85),
        master: coin(g, 80),
        shortcut_parent: coin(g, 92),
        anim_finished: coin(g, 50),
        communicator: coin(g, 70),
        game_ending: coin(g, 8),
        player: coin(g, 92),
        template: coin(g, 92),
        local: coin(g, 85),
        active: coin(g, 80),
        named: [coin(g, 80), coin(g, 80), coin(g, 80)],
        hidden,
        shortcut_data: [0; 11].map(|_| coin(g, 70).then(|| g.below(nb as u32) as usize)),
        purchase_names: pn,
        shortcut_name: coin(g, 85).then(|| g.below(8) as usize),
        sets,
        buttons,
        sci: [0; 9].map(|_| (coin(g, 50), coin(g, 15), coin(g, 15), coin(g, 60), coin(g, 70), g.below(6) as i32)),
        points: g.below(8) as i32,
        skill: g.below(2_000_001) as i32 - 1_000_000,
        down,
        up,
        rank: g.below(9) as i32,
        upgrades: [0; 8].map(|_| coin(g, 50)),
        find_type: [0; 4].map(|_| objs(g)),
        find_any: [0; 5].map(|_| objs(g)),
        find_thing: [0; 5].map(|_| objs(g)),
        has_any_shortcut: coin(g, 50),
        count: [0; 4].map(|_| g.below(5) as i32),
        salt: g.below(6) as i32,
    }
}

/// The numbers the wrapper reads, in its order.
fn serialize(w: &World) -> Vec<i64> {
    let mut v: Vec<i64> = Vec::new();
    let b = |x: bool| x as i64;
    v.push(w.n_used as i64);
    v.extend(w.buttons_null.iter().map(|&x| b(x)));
    v.extend(w.parents_null.iter().map(|&x| b(x)));
    v.extend([b(w.ctx_purchase), b(w.master), b(w.shortcut_parent), b(w.anim_finished), b(w.communicator), b(w.game_ending), b(w.player), b(w.template), b(w.local), b(w.active), b(w.player)]);
    v.extend(w.named.iter().map(|&x| b(x)));
    v.extend(w.hidden.iter().map(|&x| b(x)));
    v.extend(w.shortcut_data.iter().map(|x| x.map_or(-1, |i| i as i64)));
    v.extend(w.purchase_names.iter().map(|x| x.map_or(-1, |i| i as i64)));
    v.push(w.shortcut_name.map_or(-1, |i| i as i64));
    for s in &w.sets {
        match s {
            Some(slots) => {
                v.push(1);
                v.extend(slots.iter().map(|x| x.map_or(-1, |i| i as i64)));
            }
            None => {
                v.push(0);
                v.extend([-1i64; 18]);
            }
        }
    }
    v.push(w.buttons.len() as i64);
    for bt in &w.buttons {
        v.extend([bt.options as i64, bt.kind as i64, bt.sciences.len() as i64]);
        v.extend(bt.sciences.iter().map(|&s| s as i64));
        v.extend([b(bt.sp.is_some()), bt.sp.map_or(0, |p| p.0 as i64), bt.sp.map_or(0, |p| p.1 as i64)]);
        v.extend([b(bt.upgrade.is_some()), bt.upgrade.unwrap_or(0) as i64]);
        v.extend([b(bt.thing.is_some()), bt.thing.unwrap_or(0) as i64]);
    }
    for s in &w.sci {
        v.extend([b(s.0), b(s.1), b(s.2), b(s.3), b(s.4), s.5 as i64]);
    }
    v.extend([w.points, w.skill, w.down, w.up, w.rank].map(|x| x as i64));
    v.extend(w.upgrades.iter().map(|&x| b(x)));
    v.extend(w.find_type.iter().map(|&x| x as i64));
    v.extend(w.find_any.iter().map(|&x| x as i64));
    v.extend(w.find_thing.iter().map(|&x| x as i64));
    v.push(b(w.has_any_shortcut));
    v.extend(w.count.iter().map(|&x| x as i64));
    v.push(w.salt as i64);
    v
}

fn text(t: &[u16]) -> String {
    t.iter().map(|&c| c as u8 as char).collect()
}

fn key_of(name: &[u8]) -> u32 {
    match name {
        b"GeneralsExpPoints.wnd:StaticTextRankPointsAvailable" => 1,
        b"GeneralsExpPoints.wnd:ProgressBarExperience" => 2,
        b"GeneralsExpPoints.wnd:StaticTextTitle" => 3,
        _ => 99,
    }
}

struct Host {
    w: World,
    hidden: Vec<bool>,
    status: Vec<u32>,
    data: Vec<Option<usize>>,
    log: RefCell<Vec<String>>,
}

impl Host {
    fn new(w: World) -> Host {
        let mut data = vec![None; N_WIN];
        for i in 0..11 {
            if !w.buttons_null[i] {
                data[23 + i] = w.shortcut_data[i];
            }
        }
        Host { hidden: w.hidden.clone(), status: vec![0; N_WIN], data, log: RefCell::new(Vec::new()), w }
    }
    fn l(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
    fn info(&self, b: usize) -> CommandButtonInfo {
        let bt = &self.w.buttons[b];
        CommandButtonInfo {
            name: format!("B{b}").into_bytes(),
            options: bt.options,
            command_type: match bt.kind {
                0 => gui::PURCHASE_SCIENCE,
                1 => gui::SELECT_ALL_UNITS_OF_TYPE,
                2 => gui::SPECIAL_POWER,
                _ => 12,
            },
            science_vec: bt.sciences.clone(),
            special_power: bt.sp,
            upgrade_mask: bt.upgrade.map(|k| 1u64 << k),
            thing_template: bt.thing.map(|t| t as u16),
        }
    }
    fn set_of(&self, name: &[u8]) -> Option<&Vec<Option<usize>>> {
        let s = std::str::from_utf8(name).ok()?.strip_prefix("Set")?;
        let k: usize = s.parse().ok()?;
        self.w.sets.get(k)?.as_ref()
    }
    fn set_name(&self, k: Option<usize>) -> Vec<u8> {
        k.map_or(Vec::new(), |k| format!("Set{k}").into_bytes())
    }
}

fn sci(s: i32) -> usize {
    (s + 1) as usize
}

impl ControlBarWindowsHost for Host {
    fn win_hide(&mut self, win: u32, hide: bool) {
        self.hidden[win as usize] = hide;
        self.l(format!("hide {win} {}", hide as i32));
    }
    fn win_enable(&mut self, win: u32, enable: bool) {
        self.l(format!("enable {win} {}", enable as i32));
    }
    fn win_is_hidden(&self, win: u32) -> bool {
        self.hidden[win as usize]
    }
    fn win_set_status(&mut self, win: u32, bits: u32) {
        self.status[win as usize] |= bits;
        self.l(format!("setstatus {win} {bits:x}"));
    }
    fn win_clear_status(&mut self, win: u32, bits: u32) {
        self.status[win as usize] &= !bits;
        self.l(format!("clearstatus {win} {bits:x}"));
    }
    fn win_get_window_from_name(&self, parent: Option<u32>, name: &[u8]) -> Option<u32> {
        let key = key_of(name);
        self.l(format!("findwin {key} parent {}", parent.map_or(-1, |p| p as i32)));
        (key <= 3 && self.w.named[key as usize - 1]).then_some(65 + key)
    }
    fn static_text_set_text(&mut self, win: u32, t: &[u16]) {
        self.l(format!("statictext {win} {}", text(t)));
    }
    fn progress_bar_set_progress(&mut self, win: u32, progress: i32) {
        self.l(format!("progress {win} {progress}"));
    }
    fn button_data(&self, win: u32) -> Option<CommandButtonInfo> {
        self.data[win as usize].map(|b| self.info(b))
    }
    fn button_set_text(&mut self, win: u32, t: &[u16]) {
        self.l(format!("buttontext {win} {}", text(t)));
    }
    fn button_set_alt_sound(&mut self, win: u32, name: &[u8]) {
        self.l(format!("altsound {win} {}", String::from_utf8_lossy(name)));
    }
    fn set_control_command(&mut self, win: u32, command_button: &CommandButtonInfo) {
        let n = String::from_utf8_lossy(&command_button.name).into_owned();
        self.data[win as usize] = Some(n[1..].parse().expect("B<n>"));
        self.l(format!("setcmd {win} {n}"));
    }
    fn context_parent_purchase_science(&self) -> Option<u32> {
        self.w.ctx_purchase.then_some(70)
    }
    fn context_parent_master(&self) -> Option<u32> {
        self.w.master.then_some(45)
    }
    fn science_purchase_window(&self, rank: u32, i: usize) -> u32 {
        (match rank {
            1 => 0,
            3 => 4,
            _ => 19,
        } + i) as u32
    }
    fn shortcut_button(&self, i: usize) -> Option<u32> {
        (!self.w.buttons_null[i]).then_some(23 + i as u32)
    }
    fn shortcut_button_parent(&self, i: usize) -> Option<u32> {
        (!self.w.parents_null[i]).then_some(34 + i as u32)
    }
    fn shortcut_parent(&self) -> Option<u32> {
        self.w.shortcut_parent.then_some(46)
    }
    fn shortcut_buttons_exist(&self) -> bool {
        true
    }
    fn currently_used_special_powers_buttons(&self) -> usize {
        self.w.n_used
    }
    fn command_window(&self, i: usize) -> u32 {
        47 + i as u32
    }
    fn shortcut_display_string_exists(&self, _i: usize) -> bool {
        false
    }
    fn show_special_power_shortcut(&mut self) {
        self.l("showshortcut".to_string());
    }
    fn hide_special_power_shortcut(&mut self) {
        self.l("hideshortcut".to_string());
    }
    fn animate_special_power_shortcut(&mut self, forward: bool) {
        self.l(format!("animate {}", forward as i32));
    }
    fn gen_shortcuts_animation_finished(&self) -> bool {
        self.w.anim_finished
    }
    fn command_set_button(&self, set_name: &[u8], i: usize) -> Option<Option<CommandButtonInfo>> {
        let set = self.set_of(set_name)?;
        Some(set.get(i).copied().flatten().map(|b| self.info(b)))
    }
    fn get_command_availability(&mut self, button: &CommandButtonInfo, obj: Option<i32>, win: u32, apply_to_win: Option<u32>) -> CommandAvailability {
        let sum: i32 = button.name.iter().map(|&c| c as i32).sum();
        let (o, wn, a) = (obj.unwrap_or(0), win as i32, apply_to_win.map_or(-1, |x| x as i32));
        self.l(format!("avail {} {o} {wn} {a}", String::from_utf8_lossy(&button.name)));
        [CommandAvailability::Restricted, CommandAvailability::Available, CommandAvailability::Active, CommandAvailability::Hidden, CommandAvailability::NotReady, CommandAvailability::CantAfford][((sum + 7 * o + 3 * (wn + 1) + 5 * (a + 1) + self.w.salt) % 6) as usize]
    }
    fn script_engine_is_game_ending(&self) -> bool {
        self.w.game_ending
    }
    fn player_exists(&self) -> bool {
        self.w.player
    }
    fn purchase_science_command_set_names(&self) -> Option<[Vec<u8>; 3]> {
        self.w.template.then(|| [self.set_name(self.w.purchase_names[0]), self.set_name(self.w.purchase_names[1]), self.set_name(self.w.purchase_names[2])])
    }
    fn special_power_shortcut_command_set_name(&self) -> Option<Vec<u8>> {
        self.w.template.then(|| self.set_name(self.w.shortcut_name))
    }
    fn player_is_local(&self) -> bool {
        self.w.local
    }
    fn player_is_active(&self) -> bool {
        self.w.active
    }
    fn player_has_science(&self, science: i32) -> bool {
        self.w.sci[sci(science)].0
    }
    fn player_is_science_disabled(&self, science: i32) -> bool {
        self.w.sci[sci(science)].1
    }
    fn player_is_science_hidden(&self, science: i32) -> bool {
        self.w.sci[sci(science)].2
    }
    fn player_has_upgrade_complete(&self, mask: u64) -> bool {
        (0..8).find(|&k| mask == 1u64 << k).is_some_and(|k| self.w.upgrades[k])
    }
    fn player_science_purchase_points(&self) -> i32 {
        self.w.points
    }
    fn player_skill_points(&self) -> i32 {
        self.w.skill
    }
    fn player_skill_points_level_down(&self) -> i32 {
        self.w.down
    }
    fn player_skill_points_level_up(&self) -> i32 {
        self.w.up
    }
    fn player_rank_level(&self) -> i32 {
        self.w.rank
    }
    fn science_player_has_prereqs(&self, science: i32) -> bool {
        self.w.sci[sci(science)].3
    }
    fn science_purchase_cost(&self, science: i32) -> i32 {
        self.w.sci[sci(science)].5
    }
    fn science_player_has_root_prereqs(&self, science: i32) -> bool {
        self.w.sci[sci(science)].4
    }
    fn game_text_fetch(&self, label: &[u8]) -> Vec<u16> {
        let mut v: Vec<u16> = "T:".encode_utf16().collect();
        v.extend(label.iter().map(|&c| c as u16));
        v
    }
    fn find_most_ready_shortcut_special_power_of_type(&self, special_power_type: i32) -> Option<i32> {
        let o = self.w.find_type[special_power_type as usize];
        (o != 0).then_some(o)
    }
    fn find_any_existing_object_with_thing_template(&self, template: u16) -> Option<i32> {
        let o = self.w.find_any[template as usize];
        (o != 0).then_some(o)
    }
    fn find_most_ready_shortcut_special_power_for_thing(&self, template: u16) -> Option<i32> {
        let o = self.w.find_thing[template as usize];
        (o != 0).then_some(o)
    }
    fn has_any_shortcut_special_power(&self) -> bool {
        self.w.has_any_shortcut
    }
    fn count_ready_shortcut_special_powers_of_type(&self, special_power_type: i32) -> i32 {
        self.w.count[special_power_type as usize]
    }
    fn object_command_set_string(&self, obj: i32) -> Vec<u8> {
        format!("Set{}", obj % 8).into_bytes()
    }
    fn copy_images_and_text_from(&mut self, to: &CommandButtonInfo, from: &CommandButtonInfo) {
        let (t, f) = (String::from_utf8_lossy(&to.name).into_owned(), String::from_utf8_lossy(&from.name).into_owned());
        self.l(format!("copyimages {t} {f}"));
        self.l(format!("copytext {t} {f}"));
    }
}

fn run_rust(w: &World, ops: &[i64]) -> Vec<String> {
    let mut h = Host::new(w.clone());
    for p in ops.chunks(2) {
        let (kind, a) = (p[0], p[1]);
        match kind {
            0 => cb::populate_purchase_science(&mut h, |h| cb::update_context_purchase_science(h)),
            1 => cb::update_context_purchase_science(&mut h),
            2 => cb::populate_special_power_shortcut(&mut h, |h| cb::update_special_power_shortcut(h)),
            3 => {
                let r = cb::has_any_shortcut_selection(&h);
                h.l(format!("has {}", r as i32));
            }
            4 => cb::update_special_power_shortcut(&mut h),
            5 => cb::draw_special_power_shortcut_multiplier_text(&mut h),
            6 => cb::hide_communicator(&mut h, w.communicator.then_some(65), a != 0),
            _ => {
                let l = RefCell::new(Vec::new());
                cb::set_control_bar_scheme_by_name(a != 0, || l.borrow_mut().push("scheme Scheme".to_string()), || l.borrow_mut().push("stage 0".to_string()));
                for s in l.into_inner() {
                    h.l(s);
                }
            }
        }
    }
    for i in 0..N_WIN {
        h.l(format!("state {i} hidden {} status {:x} data {}", h.hidden[i] as i32, h.status[i], h.data[i].map_or("-".to_string(), |b| format!("B{b}"))));
    }
    h.log.into_inner()
}

#[test]
fn control_bar() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_CBA2);
    let mut check = Check::new("ControlBar (purchase science, shortcut bar): call logs and window state");
    let mut buf = vec![0 as std::os::raw::c_char; 1 << 20];
    let mut by_op = [(0u64, 0u64); 8];
    for case in 0..20_000 {
        let w = gen_world(&mut g);
        let mut ops: Vec<i64> = Vec::new();
        for _ in 0..1 + g.below(8) {
            let kind = g.below(8) as i64;
            let ok = match kind {
                1 => w.player,
                2 => w.shortcut_parent,
                _ => true,
            };
            if ok {
                ops.extend([kind, g.below(2) as i64]);
            }
        }
        if ops.is_empty() {
            ops.extend([3, 0]);
        }
        let world = serialize(&w);
        let n = unsafe { ref_cbar_run(world.as_ptr(), world.len() as i32, ops.as_ptr(), ops.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(n >= 0, "log buffer too small");
        let cpp: Vec<String> = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) }.to_string_lossy().lines().map(|s| s.to_string()).collect();
        let rust = run_rust(&w, &ops);
        let before = check.mismatches;
        check.eq(rust.clone(), cpp.clone(), || {
            let at = rust.iter().zip(cpp.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(cpp.len()));
            format!("case {case} ops {ops:?}; first differing line {at}: rust {:?} c++ {:?}", rust.get(at), cpp.get(at))
        });
        if check.mismatches > before {
            for p in ops.chunks(2) {
                by_op[p[0] as usize].1 += 1;
            }
        }
        for p in ops.chunks(2) {
            by_op[p[0] as usize].0 += 1;
        }
    }
    println!("operations run / in cases that mismatched, by kind [purchase, ctx, popShortcut, has, updShortcut, draw, hideComm, scheme]: {by_op:?}");
    if planted() {
        check.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    }
    check.finish();
}
