#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogic.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/GameLogic.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameLogic`'s frame (`logic::world::game_logic_update`) against the C++, layer 12 batch L1.
//!
//! # Method
//!
//! The C++ side is five pieces of `GameLogic.cpp` build.rs cuts verbatim (`processCommandList` to `deselectObject`;
//! `update`; `getCRC`; `isGamePaused` to `getObjectCount`; the buildable and control-bar overrides), compiled by
//! `differential/wrap_game_logic_frame.cpp` in a namespace of stand-ins that log every call: a `GameLogic` with the
//! members the code uses, objects 1-6 (disabled, mass-selectable, with a drawable, as set), players 0-15, the AI's
//! groups, every subsystem `update` calls, the network, the in-game UI, mouse and audio, the load screen, and a
//! command list whose messages a stand-in dispatcher caches CRCs from (the dispatcher itself is batch D1's). The CRCs
//! are the Xfer build's real `XferCRC` and `XferDeepCRC` (a file), over snapshots whose `crc` xfers a value. The
//! Rust side runs the same on [`game_logic_update`] and its neighbours with a host that logs the same lines.
//!
//! A program configures the subsystems (a frozen view or script, a movie, a recorder and whether it is multiplayer
//! or playing back, a game info and its CRC interval, `REPLAY_CRC_INTERVAL`, a stats collector, a network and
//! whether it saw a mismatch, a load screen, the UI's input and the mouse), the game mode, a pending new game, the
//! command list (`MSG_LOGIC_CRC` from any player, `MSG_CLEAR_GAME_DATA`, others), the network's connected slots and
//! players, and the objects' flags; it runs frames, takes CRCs (cached, light, deep), selects and deselects objects
//! for player masks, pauses and resumes, reports load progress, tests the time-outs, counts objects, and sets and
//! finds overrides; and it dumps the logic's state.
//!
//! An operation the Rust side reports as undefined ends the Rust run there; the C++ runs only the operations before
//! it (a null `TheRecorder`, `TheGameInfo` or `TheNetwork` dereferenced, a CRC interval of 0).
//!
//! # Results (2026-10-31, `GLFRAME_CASES=20000`, the default)
//!
//! 20,000 programs, 326,879 operations, 1,319,097 log lines: **0 mismatches**. 3,496 programs cut at an undefined
//! operation: 1,595 at a CRC interval of 0, 1,901 at a null `TheRecorder`, `TheGameInfo` or `TheNetwork`. Paths
//! reached (log lines): 13,898 CRCs sent, 6,240 deep CRCs, 612 CRC mismatches found, 1,205 frozen frames unfrozen by
//! `MSG_CLEAR_GAME_DATA`, 1,501 new games started, 19,000 disabled objects checked, 5,171 pauses, 1,789 forced starts,
//! 9,444 control-bar and 6,884 buildable overrides found, 1,461 selections and 1,552 deselections reaching the UI.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 8 of 8 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 4001 | a frozen frame runs on | 5,037 programs |
//! | 4002 | the solo CRC taken every frame | from program 1 on, until the C++ divides by the zero interval the plant hid from the Rust side (the run dies) |
//! | 4003 | disabled objects not re-checked | 3,574 programs |
//! | 4004 | too few CRCs not a mismatch | 368 programs |
//! | 4005 | the light CRC without the random seed | 11,336 programs |
//! | 4006 | pausing the music with the rest | 2,797 programs |
//! | 4007 | a time-out that has not elapsed counts | 1,728 programs |
//! | 4008 | the control-bar key ignores the slot | 250 programs |

use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::message_stream::MessageType;
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic_dispatch::DispatchState;
use rts_lockstep_engine::logic::world::game_logic_update::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;

struct Gen(u64);

impl Gen {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn chance(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
}

type Op = [u32; 6];
type Cfg = [u32; 13];

const NAMES: [&[u8]; 6] = [b"Alpha", b"beta", b"", b"Gamma_Set", b"alpha", b"x"];
const MSG_LOGIC_CRC: i32 = 1095;
const MSG_CLEAR_GAME_DATA: i32 = 27;

struct Host {
    flags: u32,
    mode: i32,
    crc_interval: i32,
    replay_interval: i32,
    players: u32,
    local_player: i32,
    seed_crc: u32,
    time: u32,
    frame: u32,
    start_new_game: bool,
    in_update: bool,
    objects: Vec<u32>,
    obj_disabled: u32,
    obj_mass: u32,
    obj_drawable: u32,
    pl_value: u32,
    ai_value: u32,
    pm_value: u32,
    connected: u32,
    num_players: i32,
    next_group: u32,
    /// The command list: type, player, CRC.
    commands: Vec<(i32, i32, u32)>,
    log: String,
}

impl Host {
    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }
    fn flag(&self, bit: u32) -> bool {
        self.flags >> bit & 1 != 0
    }
}

impl GameLogicHost for Host {
    fn u_frame(&self) -> u32 {
        self.frame
    }
    fn u_advance_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }
    fn u_start_new_game_pending(&self) -> bool {
        self.start_new_game
    }
    fn u_set_start_new_game_pending(&mut self, on: bool) {
        self.start_new_game = on;
    }
    fn u_is_in_update(&self) -> bool {
        self.in_update
    }
    fn u_set_in_update(&mut self, on: bool) {
        self.in_update = on;
    }
    fn u_game_mode(&self) -> i32 {
        self.mode
    }
    fn u_start_new_game(&mut self, loading_save_game: bool) {
        self.line(format!("startnewgame {}", i32::from(loading_save_game)));
    }
    fn u_objects(&self) -> Vec<u32> {
        self.objects.clone()
    }
    fn u_object_is_disabled(&self, id: u32) -> bool {
        self.obj_disabled >> id & 1 != 0
    }
    fn u_check_disabled_status(&mut self, id: u32) {
        self.line(format!("checkdisabled {id}"));
    }
    fn u_object_is_mass_selectable(&self, id: u32) -> bool {
        self.obj_mass >> id & 1 != 0
    }
    fn u_object_has_drawable(&self, id: u32) -> bool {
        self.obj_drawable >> id & 1 != 0
    }
    fn u_run_sleepy_updates(&mut self) {
        self.line("sleepy".into());
    }
    fn u_process_destroy_list(&mut self) {
        self.line("destroylist".into());
    }
    fn u_command_list_contains(&self, t: MessageType) -> bool {
        self.commands.iter().any(|c| c.0 == t.0)
    }
    fn u_command_list(&self) -> Vec<u32> {
        (0..self.commands.len() as u32).collect()
    }
    fn u_dispatch(&mut self, state: &mut DispatchState, msg: u32) -> Result<(), IniError> {
        let (t, p, crc) = self.commands[msg as usize];
        self.line(format!("dispatch {t} player {p}"));
        if t == MSG_LOGIC_CRC {
            if p == self.local_player {
                state.should_validate_crcs = true;
            }
            state.cached_crcs.insert(p, crc);
        }
        Ok(())
    }
    fn u_command_list_reset(&mut self) {
        self.commands.clear();
        self.line("commandlist reset".into());
    }
    fn u_send_logic_crc(&mut self, crc: u32, playback: bool) {
        self.line(format!("append {MSG_LOGIC_CRC}"));
        self.line(format!("arg int {}", crc as i32));
        self.line(format!("arg bool {}", i32::from(playback)));
    }
    fn u_set_fp_mode(&mut self) {
        self.line("fpmode".into());
    }
    fn u_movie_playing(&self) -> bool {
        self.flag(4)
    }
    fn u_client_set_frame(&mut self, frame: u32) {
        self.line(format!("client frame {frame}"));
    }
    fn u_script_engine_update(&mut self) {
        self.line("script update".into());
    }
    fn u_view_time_frozen(&self) -> bool {
        self.flag(0)
    }
    fn u_view_camera_movement_finished(&self) -> bool {
        self.flag(1)
    }
    fn u_script_time_frozen_debug(&self) -> bool {
        self.flag(2)
    }
    fn u_script_time_frozen_script(&self) -> bool {
        self.flag(3)
    }
    fn u_script_force_unfreeze_time(&mut self) {
        self.line("unfreeze".into());
    }
    fn u_terrain_update(&mut self) {
        self.line("terrain update".into());
    }
    fn u_has_recorder(&self) -> bool {
        self.flag(5)
    }
    fn u_recorder_is_multiplayer(&self) -> bool {
        self.flag(6)
    }
    fn u_recorder_playback(&self) -> bool {
        self.flag(7)
    }
    fn u_recorder_update(&mut self) {
        self.line("recorder update".into());
    }
    fn u_game_info_crc_interval(&self) -> Option<i32> {
        self.flag(8).then_some(self.crc_interval)
    }
    fn u_replay_crc_interval(&self) -> i32 {
        self.replay_interval
    }
    fn u_stats_collector_update(&mut self) -> bool {
        if self.flag(9) {
            self.line("stats update".into());
        }
        self.flag(9)
    }
    fn u_ai_update(&mut self) {
        self.line("ai update".into());
    }
    fn u_build_assistant_update(&mut self) {
        self.line("buildassistant update".into());
    }
    fn u_partition_update(&mut self) {
        self.line("partition update".into());
    }
    fn u_weapon_store_update(&mut self) {
        self.line("weaponstore update".into());
    }
    fn u_locomotor_store_update(&mut self) {
        self.line("locomotorstore update".into());
    }
    fn u_victory_conditions_update(&mut self) {
        self.line("victory update".into());
    }
    fn u_crc_object(&mut self, id: u32, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut v = id.wrapping_mul(2_654_435_761);
        xfer.xfer_unsigned_int(&mut v)
    }
    fn u_random_seed_crc(&self) -> u32 {
        self.seed_crc
    }
    fn u_crc_partition_manager(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut v = self.pm_value;
        xfer.xfer_unsigned_int(&mut v)
    }
    fn u_crc_player_list(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut v = self.pl_value;
        xfer.xfer_unsigned_int(&mut v)
    }
    fn u_crc_ai(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut v = self.ai_value;
        xfer.xfer_unsigned_int(&mut v)
    }
    fn u_crc_game_state(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        // SNAPSHOT_DEEPCRC_LOGICONLY is 1
        let mut v = 0xC0FF_EE00u32 + 1;
        xfer.xfer_unsigned_int(&mut v)?;
        self.line("gamestate crc 1".into());
        Ok(())
    }
    fn u_has_network(&self) -> bool {
        self.flag(10)
    }
    fn u_network_saw_crc_mismatch(&self) -> bool {
        self.flag(11)
    }
    fn u_network_set_saw_crc_mismatch(&mut self) {
        self.flags |= 1 << 11;
        self.line("network mismatch".into());
    }
    fn u_network_player_connected(&self, slot: i32) -> bool {
        self.connected >> slot & 1 != 0
    }
    fn u_network_num_players(&self) -> i32 {
        self.num_players
    }
    fn u_time_get_time(&mut self) -> i32 {
        let t = self.time;
        self.time = self.time.wrapping_add(13);
        self.line(format!("time {}", t as i32));
        t as i32
    }
    fn u_load_screen_process_progress(&mut self, player: i32, percentage: i32) -> bool {
        if self.flag(12) {
            self.line(format!("loadscreen {player} {percentage}"));
        }
        self.flag(12)
    }
    fn u_each_player_from_mask(&self, mask: &mut u16) -> Option<i32> {
        for i in 0..16 {
            let m = 1u16 << i;
            if self.players >> i & 1 != 0 && m & *mask != 0 {
                *mask &= !m;
                return Some(i);
            }
        }
        *mask = 0;
        None
    }
    fn u_create_group(&mut self) -> u32 {
        let g = self.next_group;
        self.next_group += 1;
        self.line(format!("create group {g}"));
        g
    }
    fn u_destroy_group(&mut self, group: u32) {
        self.line(format!("destroy group {group}"));
    }
    fn u_group_add(&mut self, group: u32, id: u32) {
        self.line(format!("group {group} add {id}"));
    }
    fn u_group_remove(&mut self, group: u32, id: u32) -> bool {
        let deleted = (group + id).is_multiple_of(3);
        self.line(format!("group {group} remove {id} -> {}", i32::from(deleted)));
        deleted
    }
    fn u_player_set_selected_group(&mut self, player: i32, group: Option<u32>) {
        match group {
            Some(g) => self.line(format!("player {player} select group {g}")),
            None => self.line(format!("player {player} select none")),
        }
    }
    fn u_player_add_group_to_selection(&mut self, player: i32, group: u32) {
        self.line(format!("player {player} add group {group}"));
    }
    fn u_player_selection_as_group(&mut self, player: i32, group: u32) {
        self.line(format!("player {player} selection into group {group}"));
    }
    fn u_ui_select_drawable(&mut self, id: u32) {
        self.line(format!("ui select {id}"));
    }
    fn u_ui_deselect_drawable(&mut self, id: u32) {
        self.line(format!("ui deselect {id}"));
    }
    fn u_ui_input_enabled(&self) -> bool {
        self.flag(13)
    }
    fn u_ui_set_input_enabled(&mut self, on: bool) {
        self.line(format!("ui input {}", i32::from(on)));
    }
    fn u_mouse_visible(&self) -> bool {
        self.flag(14)
    }
    fn u_mouse_set_visible(&mut self, on: bool) {
        self.line(format!("mouse visible {}", i32::from(on)));
    }
    fn u_mouse_set_arrow_cursor(&mut self) {
        self.line("mouse cursor 1".into());
    }
    fn u_audio_pause(&mut self, affect: i32) {
        self.line(format!("audio pause {affect}"));
    }
    fn u_audio_resume(&mut self, affect: i32) {
        self.line(format!("audio resume {affect}"));
    }
}

fn dump_state(h: &mut Host, s: &GameLogicState, d: &DispatchState) {
    let crcs: String = d.cached_crcs.iter().map(|(p, c)| format!(" {p}:{c:X}")).collect();
    let pc: String = s.progress_complete.iter().map(|&c| format!(" {}", i32::from(c))).collect();
    let pt: String = s.progress_complete_timeout.iter().map(|t| format!(" {t}")).collect();
    let line = format!(
        "state frame {} crc {:X} new {} update {} paused {} input {} mouse {} force {} validate {} crcs{crcs} complete{pc} timeout{pt}",
        h.frame,
        s.crc,
        i32::from(h.start_new_game),
        i32::from(h.in_update),
        i32::from(s.game_paused),
        i32::from(s.input_enabled_memory),
        i32::from(s.mouse_visible_memory),
        i32::from(s.force_game_start_by_timeout),
        i32::from(d.should_validate_crcs)
    );
    h.line(line);
}

struct Cut(String);

fn cut(e: IniError) -> Cut {
    Cut(format!("{e:?}"))
}

fn op(s: &mut GameLogicState, d: &mut DispatchState, h: &mut Host, last_deep: &mut Vec<u8>, w: &Op) -> Result<(), Cut> {
    let (a, b, c, dd) = (w[1], w[2], w[3], w[4]);
    match w[0] {
        0 => {
            game_logic_update(s, d, h).map_err(cut)?;
            h.line("updated".into());
        }
        1 => {
            h.flags = a;
            h.mode = (b % 7) as i32;
            h.crc_interval = c as i32;
            h.replay_interval = dd as i32;
            h.line("config".into());
        }
        2 => {
            h.start_new_game = a & 1 != 0;
            h.line(format!("pending {}", a & 1));
        }
        3 => {
            if h.commands.len() >= 64 {
                h.line("full".into());
            } else {
                let t = match a % 3 {
                    0 => MSG_LOGIC_CRC,
                    1 => MSG_CLEAR_GAME_DATA,
                    _ => 1000 + (a % 50) as i32,
                };
                let p = (b % 10) as i32;
                h.commands.push((t, p, c));
                h.line(format!("push {t} {p}"));
            }
        }
        4 => {
            let deep = b & 1 != 0;
            let crc = if deep {
                let mut bytes = Vec::new();
                let r = get_crc(s, (a % 2) as i32, Some(&mut bytes), h).expect("crc");
                if a % 2 == CRC_RECALC as u32 {
                    *last_deep = bytes;
                }
                r
            } else {
                get_crc(s, (a % 2) as i32, None, h).expect("crc")
            };
            h.line(format!("crc {crc:X}"));
            if deep {
                let hex: String = last_deep.iter().map(|b| format!("{b:02x}")).collect();
                h.line(format!("deep {hex}"));
            }
        }
        5 => {
            let id = (a % 7 != 0).then_some(a % 7);
            select_object(id, b & 1 != 0, c as u16, dd & 1 != 0, h);
            h.line("selected".into());
        }
        6 => {
            let id = (a % 7 != 0).then_some(a % 7);
            deselect_object(id, c as u16, dd & 1 != 0, h);
            h.line("deselected".into());
        }
        7 => {
            set_game_paused(s, a & 1 != 0, b & 1 != 0, h);
            h.line(format!("paused {}", i32::from(s.game_paused)));
        }
        8 => {
            process_progress(s, a as i32 - 2, (b % 101) as i32, h);
            h.line("progress".into());
        }
        9 => {
            process_progress_complete(s, a as i32 - 2, h);
            h.line("complete".into());
        }
        10 => {
            let r = is_progress_complete(s, h);
            h.line(format!("iscomplete {}", i32::from(r)));
        }
        11 => {
            test_time_out(s, h);
            h.line(format!("tested {}", i32::from(s.force_game_start_by_timeout)));
        }
        12 => {
            time_out_game_start(s);
            h.line("forced".into());
        }
        13 => {
            init_time_out_values(s, h).map_err(cut)?;
            h.line("inited".into());
        }
        14 => {
            let n = object_count(h);
            h.line(format!("count {n}"));
        }
        15 => {
            let tt = (a % 7 != 6).then(|| NAMES[(a % 7 % 6) as usize]);
            if b & 1 != 0 {
                set_buildable_status_override(s, tt, (c % 5) as i32);
                h.line("setbuildable".into());
            } else {
                let r = find_buildable_status_override(s, tt);
                h.line(format!("findbuildable {} {}", i32::from(r.is_some()), r.unwrap_or(99)));
            }
        }
        16 => {
            let cs = NAMES[(a % 6) as usize];
            let slot = if dd == 48 { -48 } else { (c % 16) as i32 - 2 };
            if b & 1 != 0 {
                let v = b >> 1 & 3;
                set_control_bar_override(s, cs, slot, (v != 0).then_some(v)).map_err(cut)?;
                h.line("setbar".into());
            } else {
                let (found, id) = match find_control_bar_override(s, cs, slot).map_err(cut)? {
                    None => (0, 9),
                    Some(None) => (1, 0),
                    Some(Some(v)) => (1, v),
                };
                h.line(format!("findbar {found} {id}"));
            }
        }
        17 => {
            h.connected = a & 0xff;
            h.num_players = (b % 9) as i32;
            h.line("network".into());
        }
        18 => {
            h.obj_disabled = a & 0x7e;
            h.obj_mass = b & 0x7e;
            h.obj_drawable = c & 0x7e;
            h.line("objects".into());
        }
        19 => dump_state(h, s, d),
        _ => h.line("op?".into()),
    }
    Ok(())
}

fn rust_run(cfg: &Cfg, ops: &[Op]) -> (String, Option<(usize, String)>) {
    let start = cfg[9] % 7;
    let mut h = Host {
        flags: cfg[0],
        mode: (cfg[1] % 7) as i32,
        crc_interval: cfg[2] as i32,
        replay_interval: cfg[3] as i32,
        players: cfg[4] & 0xffff,
        local_player: (cfg[5] % 10) as i32,
        seed_crc: cfg[6],
        time: cfg[7],
        frame: cfg[8],
        start_new_game: false,
        in_update: false,
        objects: if start == 0 { Vec::new() } else { (7 - start..=6).collect() },
        obj_disabled: 0,
        obj_mass: 0,
        obj_drawable: 0,
        pl_value: cfg[10],
        ai_value: cfg[11],
        pm_value: cfg[12],
        connected: 0xff,
        num_players: 2,
        next_group: 100,
        commands: Vec::new(),
        log: String::new(),
    };
    let mut s = GameLogicState::default();
    let mut d = DispatchState::default();
    let mut last_deep = Vec::new();
    for (i, w) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(Cut(why)) = op(&mut s, &mut d, &mut h, &mut last_deep, w) {
            h.log.truncate(before);
            return (h.log, Some((i, why)));
        }
    }
    (h.log, None)
}

fn cpp_run(cfg: &Cfg, ops: &[Op]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_glframe_run(cfg.as_ptr(), flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

/// The subsystem flags, mostly a working game: a recorder, a game info, a network.
fn flags(g: &mut Gen) -> u32 {
    let mut f = g.next() & 0x7fff;
    if !g.chance(8) {
        f |= 1 << 5;
    }
    if !g.chance(8) {
        f |= 1 << 8;
    }
    if !g.chance(4) {
        f |= 1 << 10;
    }
    if !g.chance(3) {
        // mostly not frozen
        f &= !0b1101;
    }
    f
}

fn interval(g: &mut Gen) -> u32 {
    match g.below(12) {
        0 => 0,
        1 => g.next(),
        2 => (-(g.below(5) as i32)) as u32,
        _ => 1 + g.below(5),
    }
}

fn gen_program(g: &mut Gen) -> (Cfg, Vec<Op>) {
    let cfg: Cfg = [
        flags(g),
        if g.chance(3) { g.below(7) } else { [1, 5, 2, 0][g.below(4) as usize] },
        interval(g),
        interval(g),
        g.next(),
        g.below(10),
        g.next(),
        if g.chance(4) { g.next() } else { g.below(100_000) },
        if g.chance(4) { g.next() } else { g.below(30) },
        g.below(7),
        g.next(),
        g.next(),
        g.next(),
    ];
    let mut ops: Vec<Op> = Vec::new();
    for _ in 0..3 + g.below(30) {
        let k = match g.below(100) {
            0..=19 => 0,
            20..=24 => 1,
            25..=27 => 2,
            28..=41 => 3,
            42..=47 => 4,
            48..=52 => 5,
            53..=56 => 6,
            57..=60 => 7,
            61..=63 => 8,
            64..=66 => 9,
            67..=68 => 10,
            69..=71 => 11,
            72 => 12,
            73..=74 => 13,
            75 => 14,
            76..=80 => 15,
            81..=86 => 16,
            87..=89 => 17,
            90..=93 => 18,
            _ => 19,
        };
        // a set, then a find of the same key
        if (k == 15 || k == 16) && g.chance(2) {
            let (x, c, d) = (g.below(7), g.below(if k == 16 { 4 } else { 10 }), if g.chance(10) { 48 } else { 0 });
            ops.push([k, x, 1 | g.below(4) << 1, c, d, 0]);
            if g.chance(3) {
                ops.push([k, g.below(7), 1 | g.below(4) << 1, c, d, 0]);
            }
            ops.push([k, x, 0, c, d, 0]);
            continue;
        }
        let o: Op = match k {
            1 => [k, flags(g), if g.chance(3) { g.below(7) } else { [1, 5, 2, 0][g.below(4) as usize] }, interval(g), interval(g), 0],
            3 => [k, if g.chance(2) { 0 } else { g.below(150) }, g.below(10), if g.chance(2) { 0x1234 } else { g.next() }, 0, 0],
            4 => [k, if g.chance(4) { 0 } else { 1 }, u32::from(g.chance(3)), 0, 0, 0],
            5 | 6 => [k, g.below(7), g.below(2), if g.chance(3) { g.next() } else { g.below(16) }, g.below(2), 0],
            8 | 9 => [k, g.below(12), g.below(200), 0, 0, 0],
            15 => [k, g.below(7), g.below(2), g.below(10), 0, 0],
            16 => [k, g.below(6), g.below(8), g.below(16), if g.chance(10) { 48 } else { 0 }, 0],
            _ => [k, g.next(), g.next(), g.next(), g.next(), 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[test]
fn game_logic_frame_matches_upstream() {
    let cases: usize = std::env::var("GLFRAME_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x474c_4631_cafe_0001);
    let (mut mismatches, mut cut_n, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_op = [0usize; 20];
    let mut marks = BTreeMap::<&str, usize>::new();
    for case in 0..cases {
        let (cfg, ops) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &ops);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut_n += 1;
            *cut_why.entry(why.clone()).or_insert(0) += 1;
        }
        let c = cpp_run(&cfg, &ops[..n]);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: cfg {cfg:?} ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
            for l in r.lines() {
                for m in ["append 1095", "network mismatch", "unfreeze", "startnewgame", "deep ", "checkdisabled", "audio pause", "tested 1", "findbar 1", "findbuildable 1", "ui select", "ui deselect", "player"] {
                    if l.starts_with(m) {
                        *marks.entry(m).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    let mut out = String::new();
    let _ = write!(out, "L1: {cases} programs ({cut_n} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("{out}");
    println!("reached: {marks:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
