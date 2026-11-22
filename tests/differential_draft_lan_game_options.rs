// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, #231, group lan7) of the draft `src/drafts/ui/lan_game_options_menu.rs` (the file's statics, `getNextSelectablePlayer`,
//! `getFirstSelectablePlayer`, `playerTooltip`, `StartPressed`, `LANEnableStartButton`, the `handle*Selection` functions, `lanUpdateSlotList`, `InitLanGameGadgets`,
//! `DeinitLanGameGadgets`, `LanGameOptionsMenuInit` / `Shutdown` / `Update` / `Input` / `System`, `updateGameOptions`, `PostToLanGameOptions`) against upstream
//! `LanGameOptionsMenu.cpp`, cut verbatim by build.rs (`drafts_lan7`) into the program `LAN7_OPT_EXE` (differential/wrap_draft_lan7_opts.cpp lists the cuts, the nine
//! edits and the stand-ins), which also holds upstream's real window manager and gadgets (combo box, list box, text entry, static text, check box, push button).
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 300: one run is under a minute; fixed seed). A scene: the menu's parent window and its 35 named children (the eleven single gadgets, per
//! slot a player / colour / faction / team combo box, an accept button and a start-position button; a combo box is eight windows), a game (host or client, local slot, 8
//! slots of random state), the map cache, the preferences, the settings. Then 6 to 24 operations: Init, Update, Shutdown, direct calls (`StartPressed`, `playerTooltip`,
//! `lanUpdateSlotList`, `updateGameOptions`, `PostToLanGameOptions`, `LANEnableStartButton`, `InitLanGameGadgets`, `DeinitLanGameGadgets`), state changes (slots, the map,
//! cash, superweapon limit, preferences, the globals of LANAPICallbacks.cpp), combo box selections (which send `GCM_SELECTED` through the real gadget), the system callback
//! called directly (every control, the chat), the input callback (Escape). After each operation both programs print the stand-in calls (`lan ...`, `game ...`, `gui ...`,
//! `shell ...`, `mouse ...`), the messages, the dump of every window and layout and the statics, the game, the tooltips, the accept colours, the transition and animate models
//! and every combo box's item data; the texts must be identical. A case stops after the first operation that frees windows (Back with a select-map layout, a pop).
//!
//! Stand-ins (logged on both sides): TheLAN, the game (the Rust host holds the VERIFIED `common::game_info::GameInfo`, the C++ a transcription of it), the map cache,
//! GUIUtil.cpp's helpers (their entries are a fixed list on both sides: `gui_util.rs` is verified on its own), the preferences, the shell, the transition handler.
//!
//! Excluded inputs (upstream's undefined behaviour or an input the program cannot reach): any control NULL (the scene always has all 35), a game that is not in a game or
//! has no local slot (`getLANSlot(-1)`), `GetMyGame() == NULL` except where upstream tests it, messages once the gadgets are torn down (Deinit) until the next Init, a player
//! name or map name that is not ASCII (`%hs`).
//!
//! # Prediction (before the first run)
//!
//! Draft defects found on reading, before a run: (1) `LanSlotInfo.user` holds the LANPlayer's `UnicodeString`s as bytes (`getName()`, `getLogin()`, `getHost()` are wide):
//! fixed to `Vec<u16>`. Expected after that: a few mismatches from my generator and stand-ins, then 0.
//!
//! Draft defects found by the run (and fixed): (2) `InitLanGameGadgets` assigned `comboBoxPlayerID[i]`, `comboBoxPlayer[i]`, the colour / faction / team ids and the accept
//! button AFTER the `GadgetComboBoxSetSelectedPos` calls whose `GCM_SELECTED` reaches the menu's system callback; upstream assigns each first, so the callback saw the
//! combo box (the first run: 20 of 2,946 outputs, from the first Init: C++ `game reset accepted` where the draft did nothing). Not draft defects, mine: the shell's pushed
//! screen, `GuiHost::layout` unlogged, the stack bookkeeping (each found as a mismatch in the harness, fixed there).
//!
//! # Planted bugs (`L9_PLANT=<id>`)
//!
//! | id | planted bug |
//! |---|---|
//! | 24661 | colour bound `<=` the number of colours |
//! | 24662 | the colour -1 (random) is also checked for availability |
//! | 24663 | leaving the observer template does not reset colour, team and start position |
//! | 24664 | a negative start position is checked for availability |
//! | 24665 | the team selection does not reset the accepted flags |
//! | 24666 | the host's re-send updates the slot list before sending the options |
//! | 24667 | StartPressed: too many players is `<=` the map's number of players |
//! | 24668 | StartPressed counts observers as users |
//! | 24669 | StartPressed starts the timer when the countdown is 0 |
//! | 24670 | updateGameOptions cuts the map name at `/` instead of `\\` |
//! | 24671 | updateGameOptions always re-positions the start spots |
//! | 24672 | Init hides the accept button of slot 1, not slot 0 |
//! | 24673 | Init of the host disables its own combo boxes (`start = 0`) |
//! | 24674 | a player combo box change repopulates the faction box for AI to AI too (`\|\|` for `^`) |
//! | 24675 | the client's Accept button calls `EnableAcceptControls(FALSE)` |
//! | 24676 | Escape acts on the key-down |
//! | 24677 | `getNextSelectablePlayer` accepts an observer local slot |
//! | 24678 | Shutdown does not reset the game start timer |
//! | 24679 | the player tooltip swaps login and host |
//! | 24680 | PostToLanGameOptions leaves slot 7's start position |
//! | 24681 | trim keeps the leading white space |
//!
//! # Results
//!
//! 2026-11-22, `DRAFT_CASES=300` (the default, about 45 s a run): **0 mismatches over 300 sequences, 48,353 outputs compared**, max ULP 0. The prediction held for (1) (fixed on
//! reading); (2) was a miss (not predicted). Planted bugs: see the table in the report on #215 (mismatches of 300 sequences; 21 of 21 caught; 24674 and 24677 were missed at 0
//! until the generator got two composites: an AI slot changed to another AI through its combo box, and an observer host clicking a start position).

mod support;
mod ui4_common;

use rts_lockstep_engine::client::gadget as gad;
use rts_lockstep_engine::client::gadget_combo_box as cb;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GameFont, WinId, WinInstanceData};
use rts_lockstep_engine::client::window_transitions::TransitionGroup;
use rts_lockstep_engine::common::game_info::{slot_state, GameInfo, GameInfoHost, MapMeta};
use rts_lockstep_engine::common::ini::game_make_color;
use rts_lockstep_engine::common::money::Money;
use rts_lockstep_engine::drafts::ui::lan_game_options_menu::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"LanGameOptionsMenuSystem";
const INP: &[u8] = b"LanGameOptionsMenuInput";
const GROUP: &[u8] = b"LanGameOptionsFade";
const FILE: &str = "LanGameOptionsMenu.wnd";
const MAX: usize = 8;

fn hexb(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn hexs(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

fn label(s: &str) -> Vec<u16> {
    format!("T:{s}").encode_utf16().collect()
}

fn cat(mut a: Vec<u16>, parts: &[&[u16]]) -> Vec<u16> {
    for p in parts {
        a.push(u16::from(b'|'));
        a.extend_from_slice(p);
    }
    a
}

/// What `GameInfo.cpp` reaches (the Rust host of the verified `GameInfo`).
#[derive(Default)]
struct Gh {
    /// lower case map name -> (numPlayers, crc)
    cache: BTreeMap<Vec<u8>, (i32, u32)>,
}

impl GameInfoHost for Gh {
    fn game_text(&mut self, l: &str) -> Vec<u16> {
        label(l)
    }
    fn tick_count(&mut self) -> u32 {
        0
    }
    fn default_starting_cash(&self) -> Money {
        Money::new()
    }
    fn net_crc_interval(&self) -> i32 {
        100
    }
    fn has_map_cache(&self) -> bool {
        true
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<MapMeta> {
        self.cache.get(lower).map(|&(n, c)| MapMeta { num_players: n, crc: c })
    }
    fn file_exists(&mut self, _p: &[u8]) -> bool {
        false
    }
    fn real_map_path_to_portable(&self, p: &[u8]) -> Vec<u8> {
        p.to_vec()
    }
    fn portable_map_path_to_real(&self, p: &[u8]) -> Vec<u8> {
        p.to_vec()
    }
    fn has_multiplayer_settings(&self) -> bool {
        true
    }
    fn num_colors(&self) -> i32 {
        0
    }
    fn show_random_player_template(&self) -> bool {
        false
    }
    fn show_random_color(&self) -> bool {
        false
    }
    fn show_random_start_pos(&self) -> bool {
        false
    }
    fn observer_color(&self) -> Result<i32, rts_lockstep_engine::common::ini::IniError> {
        Ok(0)
    }
    fn player_template_count(&self) -> i32 {
        0
    }
    fn player_template_display_name(&self, _n: i32) -> Result<Vec<u16>, rts_lockstep_engine::common::ini::IniError> {
        Ok(Vec::new())
    }
    fn record_income(&mut self, _i: i32) {}
}

struct Host {
    log: String,
    keys: KeyTable,
    shell: Shell,
    o: LanGameOptions,
    game: GameInfo,
    gh: Gh,
    cache: BTreeMap<Vec<u8>, (bool, u32, u32, i32, Vec<u16>)>,
    exists: bool,
    local_ip: u32,
    my_name: Vec<u16>,
    users: Vec<(Vec<u16>, Vec<u16>)>,
    next_screen: Option<Vec<u8>>,
    shutting: bool,
    pushed: bool,
    slots_enabled: bool,
    pref: (i32, i32, Vec<u8>, u32, bool),
    num_colors: i32,
    countdown: i32,
    min_players: i32,
    tips: BTreeMap<WinId, &'static str>,
    lay_members: Vec<usize>,
    layout: usize,
    stack: bool,
    ub: bool,
}

impl Host {
    fn new() -> Host {
        let mut gh = Gh::default();
        let mut game = GameInfo::new(&mut gh);
        game.map_name = Vec::new();
        Host {
            log: String::new(),
            keys: KeyTable::default(),
            shell: Shell::new(),
            o: LanGameOptions::default(),
            game,
            gh,
            cache: BTreeMap::new(),
            exists: true,
            local_ip: 0,
            my_name: Vec::new(),
            users: vec![(Vec::new(), Vec::new()); MAX],
            next_screen: None,
            shutting: false,
            pushed: false,
            slots_enabled: false,
            pref: (0, 0, Vec::new(), 0, false),
            num_colors: 0,
            countdown: 0,
            min_players: 0,
            tips: BTreeMap::new(),
            lay_members: Vec::new(),
            layout: 0,
            stack: false,
            ub: false,
        }
    }
    fn logln(&mut self, s: &str) {
        self.log.push_str(s);
        self.log.push('\n');
    }
    fn slot_idx(&self, i: usize) -> bool {
        i < MAX
    }
    fn fill(&mut self, wm: &mut GameWindowManager, combo: Option<WinId>, prefix: &str, data: &[i32]) {
        let Some(c) = combo else { return };
        cb::reset(wm, self, c);
        for (k, d) in data.iter().enumerate() {
            let idx = cb::add_entry(wm, self, c, &format!("{prefix}{k}").encode_utf16().collect::<Vec<u16>>(), -1);
            cb::set_item_data(wm, self, c, idx, *d as u32);
        }
    }
    fn set_cash(&mut self, v: u32) {
        let mut m = Money::new();
        m.deposit(v, false, &mut Vec::new());
        self.game.starting_cash = m;
    }
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            lan_game_options_menu_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            lan_game_options_menu_input(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn layout(&mut self, _wm: &mut GameWindowManager, _func: &[u8], layout: usize) {
        let _ = writeln!(self.log, "layout init {layout}");
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn block_input_left_up(&mut self) {
        self.log.push_str("block\n");
    }
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        match filename {
            b"pushed" => {
                let _ = writeln!(self.log, "shell complete {} {}", self.layout, u8::from(self.next_screen.is_some()));
                Some(self.layout)
            }
            b"Menus/LanMapSelectMenu.wnd" => {
                let m = self.lay_members.clone();
                let l = make_layout(wm, &m);
                wm.layouts[l].init = Some(b"init".to_vec());
                self.logln("layout Menus/LanMapSelectMenu.wnd");
                Some(l)
            }
            _ => {
                // (a screen the shell pushes: the program's `Shell::push` makes a layout the same way)
                let l = make_layout(wm, &[]);
                let _ = writeln!(self.log, "layout {}", String::from_utf8_lossy(filename));
                Some(l)
            }
        }
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, func: &[u8], layout: usize, user: Option<bool>) {
        match func {
            b"init" => {
                let _ = writeln!(self.log, "layout init {layout}");
            }
            _ => {
                self.log.push_str("shell popimmediate\n");
                let _ = writeln!(self.log, "shell shutdown pi {} {layout}", u8::from(user == Some(true)));
            }
        }
    }
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.key(name)
    }
}

const TIPS: [(&[u8], &str); 5] = [(b"MapSelectorTooltip", "map"), (b"gameAcceptTooltip", "accept"), (b"playerTooltip", "player"), (b"playerTemplateComboBoxTooltip", "tplcombo"), (b"playerTemplateListBoxTooltip", "tpllist")];

impl LanGameOptionsHost for Host {
    fn lan_game_options(&mut self) -> &mut LanGameOptions {
        &mut self.o
    }
    fn fetch_text(&self, label_: &[u8]) -> Vec<u16> {
        label(&String::from_utf8_lossy(label_))
    }
    fn lan_next_screen_take(&mut self) -> Option<Vec<u8>> {
        self.next_screen.take()
    }
    fn lan_next_screen_is_set(&self) -> bool {
        self.next_screen.is_some()
    }
    fn lan_is_shutting_down(&self) -> bool {
        self.shutting
    }
    fn set_lan_is_shutting_down(&mut self, v: bool) {
        self.shutting = v;
    }
    fn lan_button_pushed(&self) -> bool {
        self.pushed
    }
    fn set_lan_button_pushed(&mut self, v: bool) {
        self.pushed = v;
    }
    fn lan_my_game_exists(&self) -> bool {
        self.exists
    }
    fn lan_am_i_host(&self) -> bool {
        self.exists && self.game.in_game && self.game.slots[0].is_human() && self.game.slots[0].ip == self.local_ip
    }
    fn lan_game_in_progress(&self) -> bool {
        self.game.in_progress
    }
    fn lan_local_slot_num(&self) -> i32 {
        if !self.game.in_game {
            return -1;
        }
        (0..MAX).find(|&i| self.game.slots[i].is_human() && self.game.slots[i].ip == self.local_ip).map_or(-1, |i| i as i32)
    }
    fn lan_slot(&self, i: usize) -> Option<LanSlotInfo> {
        if !self.slot_idx(i) {
            return None;
        }
        let s = &self.game.slots[i];
        Some(LanSlotInfo {
            start_pos: s.start_pos,
            color: s.color,
            player_template: s.player_template,
            team_number: s.team_number,
            state: s.state,
            is_ai: s.is_ai(),
            is_human: s.is_human(),
            is_open: s.is_open(),
            is_occupied: s.is_occupied(),
            is_accepted: s.is_accepted,
            is_local_player: s.is_human() && s.ip == self.local_ip,
            has_map: s.has_map,
            name: s.name.clone(),
            user: s.is_human().then(|| (s.name.clone(), self.users[i].0.clone(), self.users[i].1.clone())),
        })
    }
    fn lan_my_name(&self) -> Vec<u16> {
        self.my_name.clone()
    }
    fn lan_player_name(&self, i: usize) -> Vec<u16> {
        self.game.slots[i].name.clone()
    }
    fn lan_game_map(&self) -> Vec<u8> {
        self.game.map_name.clone()
    }
    fn lan_slot_set_color(&mut self, i: usize, color: i32) {
        self.game.slots[i].color = color;
    }
    fn lan_slot_set_player_template(&mut self, i: usize, t: i32) {
        self.game.slots[i].set_player_template(t);
    }
    fn lan_slot_set_start_pos(&mut self, i: usize, p: i32) {
        self.game.slots[i].start_pos = p;
    }
    fn lan_slot_set_team_number(&mut self, i: usize, t: i32) {
        self.game.slots[i].team_number = t;
    }
    fn lan_slot_set_state(&mut self, i: usize, s: i32) {
        self.game.slots[i].set_state(s, &[], 0, &mut self.gh);
    }
    fn lan_slot_set_accept(&mut self, i: usize) {
        self.game.slots[i].set_accept();
    }
    fn lan_slot_set_nat_behavior_simple(&mut self, i: usize) {
        self.game.slots[i].nat_behavior = 1;
    }
    fn lan_game_reset_accepted(&mut self) {
        self.logln("game reset accepted");
        self.game.reset_accepted();
    }
    fn lan_game_set_starting_cash(&mut self, cash: u32) {
        self.set_cash(cash);
        let _ = writeln!(self.log, "game cash {cash}");
    }
    fn lan_game_starting_cash(&self) -> u32 {
        self.game.starting_cash.count_money()
    }
    fn lan_game_set_superweapon_restriction(&mut self, v: i32) {
        self.game.superweapon_restriction = v as u16;
        let _ = writeln!(self.log, "game sw {}", v as u16);
    }
    fn lan_game_superweapon_restriction(&self) -> i32 {
        i32::from(self.game.superweapon_restriction)
    }
    fn lan_game_set_map(&mut self, map: &[u8]) {
        self.game.map_name = map.to_vec();
        let _ = writeln!(self.log, "game setmap {}", hexb(map));
    }
    fn lan_game_reset_map_flags_for_recheck(&mut self) {
        let (c, s) = (self.game.map_crc, self.game.map_size);
        let _ = writeln!(self.log, "game crc {c}");
        self.ub |= self.game.set_map_crc(c, &self.gh).is_err();
        let _ = writeln!(self.log, "game size {s}");
        self.ub |= self.game.set_map_size(s, &self.gh).is_err();
    }
    fn lan_game_set_map_availability_slot0(&mut self, v: bool) {
        self.game.slots[0].set_map_availability(v);
    }
    fn lan_game_set_map_crc_and_size(&mut self, crc: u32, size: u32) {
        let _ = writeln!(self.log, "game crc {crc}");
        self.ub |= self.game.set_map_crc(crc, &self.gh).is_err();
        let _ = writeln!(self.log, "game size {size}");
        self.ub |= self.game.set_map_size(size, &self.gh).is_err();
    }
    fn lan_game_adjust_slots_for_map(&mut self) {
        self.logln("game adjust slots");
        self.ub |= self.game.adjust_slots_for_map(&mut self.gh).is_err();
    }
    fn lan_request_game_options(&mut self, options: &[u8], now: bool) {
        let _ = writeln!(self.log, "lan options {} {} 0", hexb(options), u8::from(now));
    }
    fn generate_game_options_string(&mut self) -> Vec<u8> {
        let g = &self.game;
        let mut o = format!("G;{};{};{}", String::from_utf8_lossy(&g.map_name), g.starting_cash.count_money(), g.superweapon_restriction);
        for s in &g.slots {
            let _ = write!(o, ";{},{},{},{},{},{}", s.state, s.color, s.start_pos, s.player_template, s.team_number, u8::from(s.is_accepted));
        }
        o.into_bytes()
    }
    fn lan_request_game_announce(&mut self) {
        self.logln("lan announce");
    }
    fn lan_request_game_leave(&mut self) {
        self.logln("lan leave");
    }
    fn lan_request_has_map(&mut self) {
        self.logln("lan hasmap");
    }
    fn lan_request_accept(&mut self) {
        self.logln("lan accept");
    }
    fn lan_request_game_start(&mut self) {
        self.logln("lan start");
    }
    fn lan_request_game_start_timer(&mut self, seconds: i32) {
        let _ = writeln!(self.log, "lan starttimer {seconds}");
    }
    fn lan_request_chat(&mut self, text_: &[u16], emote: bool) {
        let _ = write!(self.log, "lan chat {}", if emote { 1 } else { 0 });
        text(&mut self.log, text_);
        self.log.push('\n');
    }
    fn lan_on_chat_system(&mut self, text_: &[u16]) {
        self.log.push_str("lan onchat");
        text(&mut self.log, &"SYSTEM".encode_utf16().collect::<Vec<u16>>());
        let _ = write!(self.log, " {}", self.local_ip);
        text(&mut self.log, text_);
        self.log.push_str(" 2\n");
    }
    fn lan_on_player_leave(&mut self, name: &[u16]) {
        self.log.push_str("lan playerleave");
        text(&mut self.log, name);
        self.log.push('\n');
    }
    fn lan_reset_game_start_timer(&mut self) {
        self.logln("lan resettimer");
    }
    fn lan_exists(&self) -> bool {
        true
    }
    fn map_cache_update(&mut self) {
        self.logln("mapcache update");
    }
    fn map_cache_find_map(&self, map: &[u8]) -> Option<(i32, Vec<u16>, bool)> {
        self.cache.get(&map.to_ascii_lowercase()).map(|m| (m.3, m.4.clone(), m.0))
    }
    fn map_cache_find(&self, lower: &[u8]) -> Option<(u32, u32)> {
        self.cache.get(lower).map(|m| (m.1, m.2))
    }
    fn would_map_transfer(&self, map: &[u8]) -> bool {
        map.len() >= 4 && map[..4].eq_ignore_ascii_case(b"usr/")
    }
    fn net_min_players(&self) -> i32 {
        self.min_players
    }
    fn multiplayer_num_colors(&self) -> i32 {
        self.num_colors
    }
    fn multiplayer_start_countdown_timer_seconds(&self) -> i32 {
        self.countdown
    }
    fn lan_prefs_preferred_color(&mut self) -> i32 {
        self.pref.0
    }
    fn lan_prefs_preferred_faction(&mut self) -> i32 {
        self.pref.1
    }
    fn lan_prefs_preferred_map(&mut self) -> Vec<u8> {
        self.pref.2.clone()
    }
    fn lan_prefs_starting_cash(&mut self) -> u32 {
        self.pref.3
    }
    fn lan_prefs_superweapon_restricted(&mut self) -> bool {
        self.pref.4
    }
    fn gui_util_populate_starting_cash(&mut self, wm: &mut GameWindowManager, combo: Option<WinId>) {
        let _ = writeln!(self.log, "gui populate cash {}", idx(combo));
        self.fill(wm, combo, "cash", &[1000, 2000, 3000, 4000]);
    }
    fn gui_util_populate_color(&mut self, wm: &mut GameWindowManager, i: usize) {
        let _ = writeln!(self.log, "gui populate color {i} 0");
        let c = self.o.combo_box_color[i];
        self.fill(wm, c, "c", &[-1, 0, 1, 2, 3]);
    }
    fn gui_util_populate_player_template(&mut self, wm: &mut GameWindowManager, i: usize, allow_observers: bool) {
        let _ = writeln!(self.log, "gui populate template {i} {}", u8::from(allow_observers));
        let c = self.o.combo_box_player_template[i];
        let d: &[i32] = if allow_observers { &[-1, 0, 1, 2, -2] } else { &[-1, 0, 1, 2] };
        self.fill(wm, c, "t", d);
    }
    fn gui_util_populate_team(&mut self, wm: &mut GameWindowManager, i: usize) {
        let _ = writeln!(self.log, "gui populate team {i} 0");
        let c = self.o.combo_box_team[i];
        self.fill(wm, c, "m", &[-1, 0, 1, 2, 3]);
    }
    fn gui_util_update_slot_list(&mut self, _wm: &mut GameWindowManager) {
        let _ = writeln!(self.log, "gui updateslotlist {}", u8::from(self.exists));
    }
    fn gui_util_enable_accept_controls(&mut self, _wm: &mut GameWindowManager, enable: bool) {
        let _ = writeln!(self.log, "gui enableaccept {} {} -1", u8::from(enable), u8::from(self.exists));
    }
    fn slot_list_updates_enabled(&self) -> bool {
        self.slots_enabled
    }
    fn enable_slot_list_updates(&mut self, v: bool) {
        self.slots_enabled = v;
    }
    fn position_start_spots_for_game(&mut self, _wm: &mut GameWindowManager) {
        let _ = write!(self.log, "gui positionspots {}", u8::from(self.exists));
        for b in self.o.button_map_start_position {
            let _ = write!(self.log, " {}", idx(b));
        }
        let _ = writeln!(self.log, " win {}", idx(self.o.window_map));
    }
    fn update_map_start_spots(&mut self, _wm: &mut GameWindowManager) {
        let _ = write!(self.log, "gui updatemapspots {} 0", u8::from(self.exists));
        for b in self.o.button_map_start_position {
            let _ = write!(self.log, " {}", idx(b));
        }
        self.log.push('\n');
    }
    fn win_set_tooltip_func(&mut self, _wm: &mut GameWindowManager, w: WinId, func: &[u8]) {
        let n = TIPS.iter().find(|t| t.0 == func).map_or("other", |t| t.1);
        self.tips.insert(w, n);
    }
    fn button_set_enabled_color_accept_true(&mut self, wm: &mut GameWindowManager, w: WinId) {
        wm.tree.windows[w].inst.enabled_draw_data[0].color = game_make_color(0, 255, 0, 255);
    }
    fn chat_system_color(&self) -> i32 {
        game_make_color(255, 0, 255, 255)
    }
    fn mouse_reset_for_shutdown(&mut self) {
        self.logln("mouse cursor 1");
        self.log.push_str("mouse text");
        text(&mut self.log, &[]);
        self.log.push('\n');
    }
    fn mouse_set_cursor_tooltip(&mut self, t: &[u16]) {
        self.log.push_str("mouse tip");
        text(&mut self.log, t);
        self.log.push('\n');
    }
    fn format_one_int(&self, fmt: &[u16], a: i32) -> Vec<u16> {
        cat(fmt.to_vec(), &[&a.to_string().encode_utf16().collect::<Vec<u16>>()])
    }
    fn format_two_text(&self, fmt: &[u16], a: &[u16], b: &[u16]) -> Vec<u16> {
        cat(fmt.to_vec(), &[a, b])
    }
    fn format_tooltip_lan_player(&self, fmt: &[u16], name: &[u16], login: &[u16], host: &[u16]) -> Vec<u16> {
        cat(fmt.to_vec(), &[name, login, host])
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// the scene and the operations
// ---------------------------------------------------------------------------------------------------------------------

const MAPS: [&str; 6] = ["maps/a/a.map", "usr/b.map", "USR/C.MAP", "maps\\d\\d.map", "zz", "Maps/A/A.map"];
const CACHE: [&str; 4] = ["maps/a/a.map", "usr/b.map", "usr/c.map", "maps\\d\\d.map"];
const NAMES: [&str; 6] = ["41,42", "e9,4e2d", "61", "41,100,42", "4a,6f,65", "ffe9"];
const TEXTS: [&str; 6] = ["68,69", "20,68,69,20", "-", "20,20", "e9,9,41", "61"];

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

struct Scene {
    ops: Vec<String>,
    /// window index of: parent, back, start, emote, selmap, chat, mapdisp, chk, cash, map, list
    singles: [usize; 11],
    /// the combo boxes by kind (0 player, 1 colour, 2 faction, 3 team) and slot
    combos: [[usize; MAX]; 4],
    accept: [usize; MAX],
    spos: [usize; MAX],
    cash: usize,
    local: usize,
    host: bool,
}

fn gen_game(g: &mut Gen, ops: &mut Vec<String>, local: usize) {
    ops.push("@ lg localip 100".into());
    ops.push("@ lg ingame 1".into());
    ops.push(format!("@ lg myname {}", pick(g, &NAMES)));
    for i in 0..MAX {
        ops.push(slot_op(g, i, local));
    }
    ops.push(format!("@ lg map {}", pick(g, &MAPS)));
    ops.push(format!("@ lg cash {}", pick(g, &[1000u32, 2000, 3000, 4000, 1500])));
    ops.push(format!("@ lg sw {}", g.below(3)));
    ops.push(format!("@ lg crc {}", pick(g, &[7u32, 1007, 2007])));
    ops.push(format!("@ lg size {}", g.below(3) * 100));
    for _ in 0..g.below(5) {
        ops.push(cache_op(g));
    }
    for i in 0..MAX {
        if g.below(2) == 0 {
            ops.push(format!("@ lg user {i} {} {}", pick(g, &NAMES), pick(g, &NAMES)));
        }
    }
    state_ops(g, ops);
}

fn cache_op(g: &mut Gen) -> String {
    format!("@ lg mapmeta {} {} {} {} {} {}", pick(g, &CACHE), g.below(2), g.below(3) * 1000 + 7, g.below(4) * 100, 1 + g.below(8), pick(g, &NAMES))
}

fn state_ops(g: &mut Gen, ops: &mut Vec<String>) {
    ops.push(format!("@ lo pref {} {} {} {} {}", g.below(5) as i32 - 1, g.below(4) as i32 - 1, pick(g, &["-", "maps/a/a.map", "Usr/B.map", "ZZ", "maps\\d\\d.map"]), pick(g, &[1000u32, 2000, 3000, 4000, 1500]), g.below(2)));
    ops.push(format!("@ lo settings {} {} {}", g.below(6), g.below(3) * 5, g.below(4)));
}

fn slot_op(g: &mut Gen, i: usize, local: usize) -> String {
    let (state, ip) = if i == local {
        (slot_state::PLAYER, 100)
    } else if i == 0 {
        (slot_state::PLAYER, 200)
    } else {
        match g.below(7) {
            0 | 1 => (slot_state::OPEN, 0),
            2 => (slot_state::CLOSED, 0),
            3 => (slot_state::EASY_AI, 0),
            4 => (slot_state::MED_AI, 0),
            5 => (slot_state::BRUTAL_AI, 0),
            _ => (slot_state::PLAYER, 300 + i as i32),
        }
    };
    let color = g.below(5) as i32 - 1;
    let start = g.below(9) as i32 - 1;
    let tpl = pick(g, &[-2i32, -1, 0, 1, 2]);
    let team = g.below(5) as i32 - 1;
    format!("@ lg slot {i} {state} {color} {start} {tpl} {team} {} {} {ip} {}", g.below(2), g.below(2), pick(g, &NAMES))
}

fn gen_scene(g: &mut Gen) -> Scene {
    let en = win_status::ENABLED;
    let mut ops = Vec::new();
    let mut names: Vec<(usize, String)> = Vec::new();
    let mut next = 0usize;
    ops.push(format!("@ wu -1 0 0 0 640 480 {en} {}", gws::USER_WINDOW));
    names.push((0, format!("{FILE}:LanGameOptionsMenuParent")));
    next += 1;
    let mut s = [0usize; 11];
    s[0] = 0;
    let mut push = |ops: &mut Vec<String>, names: &mut Vec<(usize, String)>, next: &mut usize, line: String, n: &str, width: usize| -> usize {
        let at = *next;
        ops.push(line);
        names.push((at, format!("{FILE}:{n}")));
        *next += width;
        at
    };
    let btn = |style: u32| format!("W 0 1 5 5 30 10 {en} {style} 0");
    s[1] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), "ButtonBack", 1);
    s[2] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), "ButtonStart", 1);
    s[3] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), "ButtonEmote", 1);
    s[4] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), "ButtonSelectMap", 1);
    s[5] = push(&mut ops, &mut names, &mut next, format!("W 0 9 5 40 80 12 {en} {} 0 40 0 0 0 0 _", gws::ENTRY_FIELD), "TextEntryChat", 1);
    s[6] = push(&mut ops, &mut names, &mut next, format!("W 0 11 5 60 80 12 {en} {} 0", gws::STATIC_TEXT), "TextEntryMapDisplay", 1);
    s[7] = push(&mut ops, &mut names, &mut next, format!("W 0 2 5 70 20 12 {en} {} 0", gws::CHECK_BOX), "CheckboxLimitSuperweapons", 1);
    s[9] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), "MapWindow", 1);
    s[10] = push(&mut ops, &mut names, &mut next, format!("W 0 6 20 20 100 80 {en} {} 0 8 1 0 0 0 0 0 0", gws::SCROLL_LISTBOX), "ListboxChatWindowLanGame", 1);
    let combo = |x: usize| format!("W 0 4 {} 50 120 20 {en} {} 0 0 5 64 0 0 20 1 0", 400 + x, gws::COMBO_BOX);
    s[8] = push(&mut ops, &mut names, &mut next, combo(0), "ComboBoxStartingCash", 8);
    let mut combos = [[0usize; MAX]; 4];
    let mut accept = [0usize; MAX];
    let mut spos = [0usize; MAX];
    for i in 0..MAX {
        for (k, n) in ["ComboBoxPlayer", "ComboBoxColor", "ComboBoxPlayerTemplate", "ComboBoxTeam"].iter().enumerate() {
            combos[k][i] = push(&mut ops, &mut names, &mut next, combo(10 * (k + 1) + i), &format!("{n}{i}"), 8);
        }
        accept[i] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), &format!("ButtonAccept{i}"), 1);
        spos[i] = push(&mut ops, &mut names, &mut next, btn(gws::PUSH_BUTTON), &format!("ButtonMapStartPosition{i}"), 1);
    }
    // a window the select-map layout will hold
    ops.push(format!("@ wu -1 0 300 300 50 50 {en} {}", gws::USER_WINDOW));
    let lay_win = next;
    for (i, n) in &names {
        ops.push(format!("@ nm {i} {n}"));
    }
    ops.push("@ lo layout 1 0".into());
    ops.push(format!("@ lay 1 {lay_win}"));
    let host = g.below(3) != 0;
    let local = if host { 0 } else { 1 + g.below(7) as usize };
    gen_game(g, &mut ops, local);
    Scene { ops, singles: s, combos, accept, spos, cash: s[8], local, host }
}

fn win_any(sc: &Scene, g: &mut Gen) -> usize {
    match g.below(6) {
        0 => sc.singles[g.below(11) as usize],
        1 => sc.accept[g.below(8) as usize],
        2 => sc.spos[g.below(8) as usize],
        _ => sc.combos[g.below(4) as usize][g.below(8) as usize],
    }
}

/// One generated step (possibly several operations). `valid`: the gadgets are set (Init done, not torn down).
fn gen_step(g: &mut Gen, sc: &Scene, valid: &mut bool, progress: &mut bool, out: &mut Vec<String>) {
    let parent = 0;
    let slot = g.below(8) as usize;
    let sys = |msg: u32, ctl: usize, d2: u32| format!("@ sys 0 {parent} {msg} 1 {ctl} {d2}");
    let r = g.below(100);
    if !*valid {
        match r {
            0..=39 => {
                out.push("@ lo init".into());
                // (a game in progress: Init pops back to the lobby and sets no gadget)
                if !*progress {
                    *valid = true;
                }
            }
            40..=49 => {
                out.push("@ lo igg".into());
                *valid = true;
            }
            50..=64 => out.push("@ lo update".into()),
            65..=74 => out.push(format!("@ lo shutdown {}", g.below(2))),
            75..=79 => out.push("@ lo dgg".into()),
            80..=84 => out.push("@ lo lus".into()),
            85..=87 => out.push("@ lo pos".into()),
            88..=91 => out.push(format!("@ lo slots {}", g.below(2))),
            92..=95 => out.push(format!("@ lo globals {} {} {}", pick(g, &["-", "Menus/Lobby.wnd"]), g.below(2), g.below(2))),
            _ => state_ops(g, out),
        }
        return;
    }
    match r {
        0..=5 => {
            out.push("@ lo init".into());
            if !*progress {
                    *valid = true;
                }
        }
        6..=8 => out.push("@ lo update".into()),
        9..=11 => {
            out.push(format!("@ lo shutdown {}", g.below(2)));
            *valid = false;
        }
        12..=13 => {
            out.push("@ lo dgg".into());
            *valid = false;
        }
        14..=15 => out.push("@ lo igg".into()),
        // GCM_SELECTED through the gadget
        16..=33 => {
            let k = g.below(4) as usize;
            let i = g.below(8) as usize;
            let pos = if k == 0 && i == sc.local { 0 } else { g.below(4) };
            out.push(format!("@ lo cbsel {} {pos}", sc.combos[k][i]));
        }
        34..=36 => out.push(format!("@ lo cbsel {} {}", sc.cash, g.below(4))),
        // GCM_SELECTED sent directly (the message the gadget would send)
        37..=40 => out.push(sys(ggm::GCM_SELECTED, win_any(sc, g), 0)),
        41..=44 => out.push(sys(ggm::GBM_SELECTED, sc.spos[slot], 0)),
        45..=46 => out.push(sys(ggm::GBM_SELECTED_RIGHT, sc.spos[slot], 0)),
        47..=48 => out.push(sys(ggm::GBM_SELECTED, sc.singles[2], 0)),
        49..=50 => out.push(sys(ggm::GBM_SELECTED, sc.singles[7], 0)),
        51 => out.push(sys(ggm::GBM_SELECTED, sc.singles[4], 0)),
        52 => out.push(sys(ggm::GBM_SELECTED, sc.singles[1], 0)),
        53..=55 => {
            out.push(format!("@ lo text {} {}", sc.singles[5], pick(g, &TEXTS)));
            out.push(sys(ggm::GBM_SELECTED, sc.singles[3], 0));
        }
        56..=58 => {
            out.push(format!("@ lo text {} {}", sc.singles[5], pick(g, &TEXTS)));
            out.push(sys(ggm::GEM_EDIT_DONE, sc.singles[5], 0));
        }
        59..=60 => out.push(sys(ggm::GEM_EDIT_DONE, win_any(sc, g), 0)),
        61..=62 => {
            let msg = pick(g, &[gwm::CREATE, gwm::DESTROY, gwm::INPUT_FOCUS, 99]);
            // (INPUT_FOCUS with mData1 TRUE writes through mData2: not sent)
            let d1 = if msg == gwm::INPUT_FOCUS { 2 * g.below(2) } else { g.below(3) };
            out.push(format!("@ sys 0 {parent} {msg} 0 {d1} 0"));
        }
        63..=66 => {
            let key = pick(g, &[1u32, 1, 1, 2, 57]);
            let state = pick(g, &[1u32, 2, 3, 0, 0x101]);
            out.push(format!("@ inp 0 {parent} {} 0 {key} {state}", gwm::CHAR));
        }
        // direct calls
        67..=69 => out.push("@ lo start".into()),
        70..=71 => out.push(format!("@ lo tip {}", if g.below(2) == 0 { cb_edit(sc, g) } else { win_any(sc, g) })),
        72..=73 => out.push("@ lo lus".into()),
        74..=75 => out.push("@ lo ugo".into()),
        76..=77 => out.push(format!("@ lo post {}", g.below(3))),
        78 => out.push(format!("@ lo ens {}", g.below(2))),
        79 => out.push("@ lo pos".into()),
        // state
        80..=86 => {
            let i = g.below(8) as usize;
            out.push(slot_op(g, i, sc.local));
        }
        87 => out.push(format!("@ lg map {}", pick(g, &MAPS))),
        88 => out.push(format!("@ lg cash {}", pick(g, &[1000u32, 2000, 3000, 4000, 1500]))),
        89 => out.push(format!("@ lg sw {}", g.below(3))),
        90 => out.push(cache_op(g)),
        91 => out.push(format!("@ lo globals {} {} {}", pick(g, &["-", "Menus/Lobby.wnd"]), 0, g.below(2))),
        92 => out.push(format!("@ lo slots {}", g.below(2))),
        93 => out.push(format!("@ lo chk {} {}", sc.singles[7], g.below(2))),
        94 => out.push(format!("@ lo msl {}", g.below(2))),
        // composites aimed at the AI -> AI change of a player combo box and at the start position of an observer host
        96 if g.below(2) == 0 => {
            let i = 1 + g.below(7) as usize;
            if i != sc.local {
                out.push(format!("@ lg slot {i} {} -1 -1 -1 -1 1 1 0 {}", pick(g, &[2, 3, 4]), pick(g, &NAMES)));
                out.push(format!("@ lo cbsel {} {}", sc.combos[0][i], pick(g, &[1u32, 2, 3])));
            }
        }
        97 if g.below(2) == 0 => {
            let l = sc.local;
            out.push(format!("@ lg slot {l} 5 {} -1 {} {} 1 1 100 {}", g.below(5) as i32 - 1, pick(g, &[-2i32, -2, -1, 0]), g.below(5) as i32 - 1, pick(g, &NAMES)));
            out.push(sys(ggm::GBM_SELECTED, sc.spos[slot], 0));
        }
        96 if g.below(3) == 0 => out.push(format!("@ lo stack {}", g.below(2))),
        95 => {
            let b = g.below(2);
            *progress = b != 0;
            out.push(format!("@ lg inprogress {b}"));
        }
        96 => state_ops(g, out),
        97 => {
            out.push(format!("H {} {}", win_any(sc, g), g.below(2)));
        }
        98 => out.push(format!("E {} {}", win_any(sc, g), g.below(2))),
        _ => out.push(format!("@ lo cbadd {} {} {}", sc.combos[g.below(4) as usize][g.below(8) as usize], pick(g, &NAMES), g.below(5) as i32 - 2)),
    }
}

fn cb_edit(sc: &Scene, g: &mut Gen) -> usize {
    // the edit box of a player combo box is its window + 2 (drop-down button, edit box, list box, ...)
    sc.combos[0][g.below(8) as usize] + 2
}

fn combo_create(wm: &mut GameWindowManager, h: &mut Host, v: &[&str]) {
    let n: Vec<i64> = v[1..].iter().filter(|s| s.parse::<i64>().is_ok()).map(|s| s.parse().unwrap()).collect();
    let (parent, x, y, ww, hh) = (n[0] as i32, n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32);
    let (status, style, visual) = (n[6] as u32, n[7] as u32, n[8] != 0);
    let f: Vec<i32> = n[9..17].iter().map(|&x| x as i32).collect();
    let d = ComboBoxData { is_editable: f[0] != 0, max_display: f[1], max_chars: f[2], ascii_only: f[3] != 0, letters_and_numbers_only: f[4] != 0 };
    let mut t = cb::combo_template(&d);
    t.listbox.list_length = f[5] as i16;
    t.listbox.scroll_bar = f[6] != 0;
    let mut inst = WinInstanceData { style, ..WinInstanceData::new() };
    inst.font = (f[7] != 0).then(|| GameFont { name: b"F".to_vec(), point_size: f[7], bold: false });
    let p = (parent >= 0).then_some(parent as usize);
    cb::gogo_gadget_combo_box(wm, h, p, status, x, y, ww, hh, inst, t, visual);
}

fn num(v: &[&str], i: usize) -> i64 {
    v[i].parse().unwrap_or_else(|_| panic!("number at {i} in {v:?}"))
}

/// The `@ lg` and `@ lo` operations; false when `op` is none of them.
fn menu_op(wm: &mut GameWindowManager, h: &mut Host, v: &[&str]) -> bool {
    if v[0] != "@" || !matches!(v[1], "lg" | "lo") {
        return false;
    }
    let u = |i: usize| hexs(v[i]);
    if v[1] == "lg" {
        match v[2] {
            "exists" => h.exists = num(v, 3) != 0,
            "ingame" => h.game.in_game = num(v, 3) != 0,
            "inprogress" => h.game.in_progress = num(v, 3) != 0,
            "localip" => {
                h.local_ip = num(v, 3) as u32;
                h.game.local_ip = h.local_ip;
            }
            "myname" => h.my_name = u(3),
            "map" => h.game.map_name = if v[3] == "-" { Vec::new() } else { v[3].as_bytes().to_vec() },
            "cash" => h.set_cash(num(v, 3) as u32),
            "sw" => h.game.superweapon_restriction = num(v, 3) as u16,
            "crc" => h.game.map_crc = num(v, 3) as u32,
            "size" => h.game.map_size = num(v, 3) as u32,
            "slot" => {
                let i = num(v, 3) as usize;
                let s = &mut h.game.slots[i];
                s.state = num(v, 4) as i32;
                s.color = num(v, 5) as i32;
                s.start_pos = num(v, 6) as i32;
                s.player_template = num(v, 7) as i32;
                s.team_number = num(v, 8) as i32;
                s.is_accepted = num(v, 9) != 0;
                s.has_map = num(v, 10) != 0;
                s.ip = num(v, 11) as u32;
                s.name = u(12);
            }
            "user" => {
                let i = num(v, 3) as usize;
                h.users[i] = (u(4), u(5));
            }
            "mapmeta" => {
                let key = v[3].as_bytes().to_vec();
                h.cache.insert(key.clone(), (num(v, 4) != 0, num(v, 5) as u32, num(v, 6) as u32, num(v, 7) as i32, u(8)));
                h.gh.cache.insert(key, (num(v, 7) as i32, num(v, 5) as u32));
            }
            other => panic!("@ lg {other}"),
        }
        return true;
    }
    match v[2] {
        "layout" => {
            let l = make_layout(wm, &[0]);
            wm.layouts[l].shutdown = Some(b"shutdown".to_vec());
            h.layout = l;
            h.logln("layout LanGameOptionsMenu.wnd");
        }
        "init" => {
            let l = h.layout;
            lan_game_options_menu_init(wm, h, l);
        }
        "shutdown" => {
            // (the pending push is this layout's own, as group ui5's program models it)
            h.shell.pending_push = true;
            h.shell.pending_push_name = b"pushed".to_vec();
            let l = h.layout;
            lan_game_options_menu_shutdown(wm, h, l, num(v, 3) != 0);
        }
        "update" => {
            h.shell.pending_push = true;
            h.shell.pending_push_name = b"pushed".to_vec();
            let l = h.layout;
            lan_game_options_menu_update(wm, h, l);
            h.shell.pending_push = false;
            h.shell.pending_push_name.clear();
        }
        "igg" => init_lan_game_gadgets(wm, h),
        "dgg" => deinit_lan_game_gadgets(h),
        "tip" => player_tooltip(wm, h, Some(num(v, 3) as usize)),
        "start" => start_pressed(wm, h),
        "ens" => lan_enable_start_button(wm, h, num(v, 3) != 0),
        "lus" => lan_update_slot_list(wm, h),
        "ugo" => update_game_options(wm, h),
        "pos" => lan_position_start_spots(wm, h),
        "post" => {
            let n = num(v, 3);
            if n == 0 {
                post_to_lan_game_options(wm, h, PostToLanGameType::SendGameOpts);
            } else if n == 1 {
                post_to_lan_game_options(wm, h, PostToLanGameType::MapBack);
            }
        }
        "msl" => {
            let l = if num(v, 3) != 0 { Some(h.layout) } else { None };
            h.o.map_select_layout = l;
        }
        "stack" => h.stack = num(v, 3) != 0,
        "globals" => {
            h.next_screen = (v[3] != "-").then(|| v[3].as_bytes().to_vec());
            h.shutting = num(v, 4) != 0;
            h.pushed = num(v, 5) != 0;
        }
        "slots" => h.slots_enabled = num(v, 3) != 0,
        "pref" => h.pref = (num(v, 3) as i32, num(v, 4) as i32, if v[5] == "-" { Vec::new() } else { v[5].as_bytes().to_vec() }, num(v, 6) as u32, num(v, 7) != 0),
        "settings" => {
            h.num_colors = num(v, 3) as i32;
            h.countdown = num(v, 4) as i32;
            h.min_players = num(v, 5) as i32;
        }
        "cbadd" => {
            let c = num(v, 3) as usize;
            let i = cb::add_entry(wm, h, c, &u(4), -1);
            cb::set_item_data(wm, h, c, i, num(v, 5) as i32 as u32);
        }
        "cbsel" => cb::set_selected_pos(wm, h, num(v, 3) as usize, num(v, 4) as i32, false),
        "chk" => gad::check_box_set_checked(wm, h, num(v, 3) as usize, num(v, 4) != 0),
        "text" => rts_lockstep_engine::client::gadget_text_entry::set_text(wm, h, Some(num(v, 3) as usize), &u(4)),
        other => panic!("@ lo {other}"),
    }
    true
}

fn sys_op(wm: &mut GameWindowManager, h: &mut Host, v: &[&str]) {
    let (win, msg, kind, d1, d2) = (num(v, 3) as usize, num(v, 4) as u32, num(v, 5), num(v, 6) as u32, num(v, 7) as u32);
    let a = if kind == 1 { win_data(Some(d1 as usize)) } else { d1 };
    let r = if v[1] == "sys" { lan_game_options_menu_system(wm, h, win, msg, a, d2) } else { lan_game_options_menu_input(wm, h, win, msg, a, d2) };
    let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
}

fn statics(h: &Host, wm: &GameWindowManager) -> String {
    let o = &h.o;
    let mut s = String::new();
    let _ = write!(s, "lo ids {} {} {} {} {} {} {} {} {} {} {}", o.parent_lan_game_options_id, o.text_entry_chat_id, o.text_entry_map_display_id, o.button_back_id, o.button_start_id, o.button_emote_id, o.button_select_map_id, o.checkbox_limit_superweapons_id, o.combo_box_starting_cash_id, o.window_map_id, o.listbox_chat_window_lan_game_id);
    let ids = |s: &mut String, n: &str, a: &[i32; MAX]| {
        let _ = write!(s, " {n}");
        for x in a {
            let _ = write!(s, " {x}");
        }
    };
    ids(&mut s, "pl", &o.combo_box_player_id);
    ids(&mut s, "acc", &o.button_accept_id);
    ids(&mut s, "col", &o.combo_box_color_id);
    ids(&mut s, "tpl", &o.combo_box_player_template_id);
    ids(&mut s, "team", &o.combo_box_team_id);
    ids(&mut s, "sp", &o.button_map_start_position_id);
    let _ = write!(s, "\nlo wins {} {} {} {} {} {} {} {} {} {} {}", idx(o.parent_lan_game_options), idx(o.button_back), idx(o.button_start), idx(o.button_select_map), idx(o.button_emote), idx(o.text_entry_chat), idx(o.text_entry_map_display), idx(o.checkbox_limit_superweapons), idx(o.combo_box_starting_cash), idx(o.window_map), idx(o.listbox_chat_window_lan_game));
    let wins = |s: &mut String, n: &str, a: &[Option<WinId>; MAX]| {
        let _ = write!(s, " {n}");
        for x in a {
            let _ = write!(s, " {}", idx(*x));
        }
    };
    wins(&mut s, "pl", &o.combo_box_player);
    wins(&mut s, "acc", &o.button_accept);
    wins(&mut s, "col", &o.combo_box_color);
    wins(&mut s, "tpl", &o.combo_box_player_template);
    wins(&mut s, "team", &o.combo_box_team);
    wins(&mut s, "sp", &o.button_map_start_position);
    let msl = o.map_select_layout.filter(|&l| !wm.layouts[l].deleted).map_or(-1, |l| l as i64);
    let next = h.next_screen.as_ref().map_or("-".to_string(), |n| String::from_utf8_lossy(n).into_owned());
    let _ = write!(s, "\nlo flags init {} msl {msl} shutting {} pushed {} next {next} slots {}\n", u8::from(o.s_is_initing), u8::from(h.shutting), u8::from(h.pushed), u8::from(h.slots_enabled));
    let g = &h.game;
    let _ = write!(s, "lo game exists {} ingame {} progress {} ip {}/{} map {} crc {} size {} cash {} sw {} name", u8::from(h.exists), u8::from(g.in_game), u8::from(g.in_progress), h.local_ip, g.local_ip, hexb(&g.map_name), g.map_crc, g.map_size, g.starting_cash.count_money(), g.superweapon_restriction);
    text(&mut s, &h.my_name);
    s.push('\n');
    for (i, sl) in g.slots.iter().enumerate() {
        let _ = write!(s, "lo slot {i} {} {} {} {} {} {} {} {} {}", sl.state, sl.color, sl.start_pos, sl.player_template, sl.team_number, u8::from(sl.is_accepted), u8::from(sl.has_map), sl.ip, sl.nat_behavior);
        text(&mut s, &sl.name);
        text(&mut s, &h.users[i].0);
        text(&mut s, &h.users[i].1);
        s.push('\n');
    }
    s.push_str("lo tips");
    for (w, n) in &h.tips {
        if !wm.ext[*w].freed {
            let _ = write!(s, " {w}:{n}");
        }
    }
    s.push_str("\nlo colors");
    let green = game_make_color(0, 255, 0, 255);
    for (i, w) in wm.tree.windows.iter().enumerate() {
        if !wm.ext[i].freed && w.inst.enabled_draw_data[0].color == green {
            let _ = write!(s, " {i}");
        }
    }
    let t = &wm.transitions;
    let _ = writeln!(s, "\nlo trans cur {} pend {} dir {} anim nu {} rev {}", u8::from(t.current.is_some()), u8::from(t.pending.is_some()), t.groups[0].direction_multiplier, u8::from(h.shell.animate.needs_update), u8::from(h.shell.animate.reverse));
    s
}

#[test]
fn lan_game_options_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1A_07);
    let mut chk = Check::new("LanGameOptionsMenu");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    for case in 0..cases(300) as u64 {
        let sc = gen_scene(&mut g);
        let mut ops = sc.ops.clone();
        let mut valid = false;
        let mut progress = false;
        let _ = (sc.host, sc.cash);
        for _ in 0..6 + g.below(19) {
            let mut step = Vec::new();
            gen_step(&mut g, &sc, &mut valid, &mut progress, &mut step);
            ops.extend(step);
        }
        run_case(&mut chk, case, ops, &menu_funcs);
    }
    finish(chk);
}

fn run_case(chk: &mut Check, case: u64, ops: Vec<String>, menu_funcs: &[(&[u8], &[u8])]) {
    let mut wm = GameWindowManager::new((800, 600));
    wm.transitions.groups.push(TransitionGroup { name: GROUP.to_vec(), ..Default::default() });
    let mut h = Host::new();
    let mut chunks = Vec::new();
    let mut run = ops.len();
    for (i, op) in ops.iter().enumerate() {
        let v: Vec<&str> = op.split(' ').collect();
        let freed_before = wm.ext.iter().filter(|e| e.freed).count() + wm.layouts.iter().filter(|l| l.deleted).count();
        // the shell's stack is the case's `@ lo stack` (the host's `pending_push` hack links the layout)
        h.shell.screen_stack = if h.stack { vec![h.layout] } else { Vec::new() };
        let had_stack = h.stack;
        if v[0] == "W" && v[2] == "4" {
            combo_create(&mut wm, &mut h, &v);
        } else if v[0] == "W" && v[2] == "2" {
            let n: Vec<i64> = v[1..].iter().map(|s| s.parse().unwrap()).collect();
            let p = (n[0] >= 0).then_some(n[0] as usize);
            let inst = WinInstanceData { style: n[7] as u32, ..WinInstanceData::new() };
            wm.gogo_gadget_look(&mut h, p, n[6] as u32, n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32, inst, rts_lockstep_engine::client::window_script::GadgetData::None, n[8] != 0);
        } else if v[0] == "@" && v[1] == "lay" {
            h.lay_members = v[3..].iter().map(|s| s.parse().unwrap()).collect();
        } else if menu_op(&mut wm, &mut h, &v) {
        } else if v[0] == "@" && matches!(v[1], "sys" | "inp") {
            sys_op(&mut wm, &mut h, &v);
        } else {
            let mut keys = std::mem::take(&mut h.keys);
            let ok = scene_op(&mut wm, &mut h, &mut keys, menu_funcs, op);
            h.keys = keys;
            assert!(ok, "unknown op {op}");
        }
        if wm.undefined.is_some() || h.ub {
            if std::env::var("LAN7_DEBUG").is_ok() {
                eprintln!("UNDEFINED case {case} op {i} `{op}`: {:?}", wm.undefined);
            }
            run = i;
            break;
        }
        if had_stack && h.shell.screen_stack.is_empty() {
            h.stack = false;
        }
        let log = std::mem::take(&mut h.log);
        let mut st = dump(&wm);
        st.push_str(&dump_layouts(&wm));
        st.push_str(&dump_static_texts(&wm));
        st.push_str(&statics(&h, &wm));
        st.push_str(&items(&mut wm, &mut h));
        chunks.push(format!("#{i}\n{log}{st}"));
        if wm.ext.iter().filter(|e| e.freed).count() + wm.layouts.iter().filter(|l| l.deleted).count() != freed_before {
            run = i + 1;
            break;
        }
    }
    let ops: Vec<String> = ops.into_iter().take(run).collect();
    if std::env::var("LAN7_DUMP_CASE").ok().and_then(|v| v.parse::<u64>().ok()) == Some(case) {
        let _ = std::fs::write("/tmp/claude-0/-home-user/3cefbd62-dbac-57be-8b5b-f778561c06c9/scratchpad/case7.txt", ops.join("\n") + "\n");
    }
    compare_case(env!("LAN7_OPT_EXE"), chk, case, &ops, &chunks);
}

/// The combo boxes' item data: the program's `lo items` line is the last of its dump.
fn items(wm: &mut GameWindowManager, h: &mut Host) -> String {
    let mut s = String::from("lo items");
    for i in 0..wm.tree.windows.len() {
        if wm.ext[i].freed || wm.tree.windows[i].inst.style & gws::COMBO_BOX == 0 {
            continue;
        }
        let _ = write!(s, " {i}:");
        let n = cb::get_length(wm, i);
        for k in 0..n {
            if k > 0 {
                s.push(',');
            }
            let _ = write!(s, "{}", cb::get_item_data(wm, h, i, k) as i32);
        }
    }
    s.push('\n');
    s
}
