#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/System/GameLogicDispatch.cpp (logicMessageDispatcher)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `GameLogic::logicMessageDispatcher` (`logic::world::game_logic_dispatch`) against the C++, layer 12 batch D1.
//!
//! # Method
//!
//! The C++ side is the function and the file's statics and helpers, cut verbatim by build.rs (one for-scope edit in
//! `MSG_SELF_DESTRUCT`) and compiled by `differential/wrap_dispatch.cpp` inside a namespace of stand-ins: twelve objects
//! (whether each exists, has an AI, an exit, production, a drawable, is selected, locally controlled, under
//! construction, reconstructing, its owner, its template), up to six players (defeated, beacon template, beacon count,
//! observer, relationships, selections), the AI's groups, the network's slots, the recorder's mode, the replay camera; the
//! messages are the message stream build's real `GameMessage`. Every call that acts logs a line; the questions answer from
//! the world. The Rust side dispatches the same messages through a [`DispatchHost`] that keeps the same world and writes
//! the same lines. A program is a world and a run of messages: every handled type and others, the issuing player valid
//! or not, arguments as the client sends them or of other kinds, counts and values.
//!
//! A message the Rust side reports as undefined (the issuing player or another `NULL` dereferenced, an empty group's
//! first object read, the network's CRC array indexed past `MAX_SLOTS`) ends the Rust run before it; the C++ dispatches
//! only the messages before it.
//!
//! # Results (2026-10-31, `DISPATCH_CASES=20000`, the default)
//!
//! 20,000 programs, 91,660 messages dispatched (every type the release build handles, and others), 481,097 log
//! lines: **0 mismatches**. 7,197 programs cut at an undefined message: a `NULL` dereferenced (most often the issuing
//! player missing for a network command, which builds the selection's group from it; a beacon removed from objects with
//! no controlling player or a player template with no beacon), or a `Bool` argument whose byte is neither 0 nor 1.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 8 of 8 caught.
//!
//! | id | planted bug | caught by |
//! |---|---|---|
//! | 3971 | a group stripped of others' objects is kept (`removeAnyObjectsNotOwnedByPlayer`'s answer ignored) | 5,053 programs |
//! | 3972 | `MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION` does not fall into `MSG_DO_ATTACK_OBJECT` | 101 programs |
//! | 3973 | force-attacking the ground never locks the primary weapon | 165 programs |
//! | 3974 | `MSG_CANCEL_UNIT_CREATE` without production does not return (the group is destroyed) | 146 programs |
//! | 3975 | a self-destruct with no living ally does not kill the player | 475 programs |
//! | 3976 | every selected object starts a new group | 514 programs |
//! | 3977 | one beacon more than the limit allowed | 68 programs |
//! | 3978 | `MSG_EXIT` without the controlling player test | 141 programs |

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{IniError, Undefined};
use rts_lockstep_engine::common::message_stream::*;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic_dispatch::*;
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

#[derive(Clone, Copy, Default)]
struct ObjW {
    exists: bool,
    has_ai: bool,
    local: bool,
    draw_sel: bool,
    has_exit: bool,
    has_prod: bool,
    under_cons: bool,
    recons: bool,
    has_draw: bool,
    owner: i32,
    beacon_of: i32,
}

#[derive(Clone, Copy, Default)]
struct PlW {
    defeated: bool,
    beacon_t: i32,
    count: i32,
    observer: bool,
}

struct Host {
    log: String,
    obj: [ObjW; 13],
    pl: [PlW; 6],
    rel: [[i32; 6]; 6],
    sel: [Vec<u32>; 6],
    player_count: i32,
    local: i32,
    max_beacons: i32,
    fps: i32,
    in_game: bool,
    stats: bool,
    playback: bool,
    cam_follows: bool,
    cam_finished: bool,
    mouse_moved: bool,
    network: bool,
    net_connected: u32,
    net_slot: [i32; 8],
    bits: u32,
    groups: BTreeMap<u32, Vec<u32>>,
    next_group: u32,
    next_object: u32,
}

fn hexf(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

fn coord(p: &Coord3D) -> String {
    format!(" {} {} {}", hexf(p.x), hexf(p.y), hexf(p.z))
}

impl Host {
    fn w(&self, bit: i32) -> bool {
        (self.bits >> (bit & 31)) & 1 != 0
    }
    fn line(&mut self, s: String) {
        self.log.push_str(&s);
        self.log.push('\n');
    }
    fn template_of(&self, obj: u32) -> i32 {
        let b = self.obj[obj as usize].beacon_of;
        if b >= 0 {
            b
        } else {
            63
        }
    }
}

impl DispatchHost for Host {
    fn d_is_in_game(&self) -> bool {
        self.in_game
    }
    fn d_player_exists(&self, i: i32) -> bool {
        i >= 0 && i < self.player_count
    }
    fn d_player_count(&self) -> i32 {
        self.player_count
    }
    fn d_local_player(&self) -> Option<i32> {
        (self.local >= 0 && self.local < self.player_count).then_some(self.local)
    }
    fn d_find_object(&self, id: u32) -> bool {
        (1..=12).contains(&id) && self.obj[id as usize].exists
    }
    fn d_object_position(&self, obj: u32) -> Coord3D {
        Coord3D { x: obj as f32 * 10.0, y: obj as f32 * 20.0, z: 0.0 }
    }
    fn d_controlling_player(&self, obj: u32) -> Option<i32> {
        let o = self.obj[obj as usize].owner;
        (o >= 0 && o < self.player_count).then_some(o)
    }
    fn d_create_group(&mut self) -> u32 {
        let g = self.next_group;
        self.next_group += 1;
        self.groups.insert(g, Vec::new());
        self.line(format!("creategroup {g}"));
        g
    }
    fn d_destroy_group(&mut self, g: u32) {
        self.line(format!("destroygroup {g}"));
        self.groups.remove(&g);
    }
    fn d_selection_into_group(&mut self, player: i32, g: u32) {
        let s = self.sel[player as usize].clone();
        self.groups.entry(g).or_default().extend(s);
        self.line(format!("selection {player} into {g}"));
    }
    fn d_group_is_empty(&self, g: u32) -> bool {
        self.groups.get(&g).is_none_or(|v| v.is_empty())
    }
    fn d_group_remove_not_owned(&mut self, g: u32, player: i32) -> bool {
        let mut v = self.groups.get(&g).cloned().unwrap_or_default();
        let mut i = 0;
        while i < v.len() {
            if self.obj[v[i] as usize].owner != player {
                self.line(format!("groupremove {g} {}", v[i]));
                v.remove(i);
                if v.is_empty() {
                    self.line(format!("group {g} emptied"));
                    self.groups.insert(g, v);
                    return true;
                }
            } else {
                i += 1;
            }
        }
        self.groups.insert(g, v);
        false
    }
    fn d_group_ids(&self, g: u32) -> Vec<u32> {
        self.groups.get(&g).cloned().unwrap_or_default()
    }
    fn d_group_add(&mut self, g: u32, obj: u32) {
        self.groups.entry(g).or_default().push(obj);
        self.line(format!("groupadd {g} {obj}"));
    }
    fn d_group(&mut self, g: u32, call: GroupCall) -> Result<bool, IniError> {
        use GroupCall::*;
        let mut r = false;
        let l = match call {
            SetWeaponLock { slot, lock } => {
                r = self.w(g as i32 + slot);
                format!("group {g} weaponlock {slot} {lock}: {}", i32::from(r))
            }
            ReleaseWeaponLock { lock } => format!("group {g} releaselock {lock}"),
            AttackPosition { pos, max_shots } => match pos {
                Some(p) => format!("group {g} attackpos {max_shots}{}", coord(&p)),
                None => format!("group {g} attackpos {max_shots} none"),
            },
            AttackObject { target, max_shots } => format!("group {g} attackobj {target} {max_shots}"),
            ForceAttackObject { target, max_shots } => format!("group {g} forceattackobj {target} {max_shots}"),
            AttackMoveTo { pos, max_shots } => format!("group {g} attackmove {max_shots}{}", coord(&pos)),
            MoveTo { pos, add_waypoint } => format!("group {g} move {}{}", i32::from(add_waypoint), coord(&pos)),
            CombatDrop { target, pos } => format!("group {g} combatdrop {}{}", target.unwrap_or(0), coord(&pos)),
            MineClearingDetail => format!("group {g} mineclearing"),
            SpecialPower { id, options } => format!("group {g} sp {id} {options}"),
            SpecialPowerAtLocation { id, pos, angle, object_in_way, options } => {
                format!("group {g} sploc {id} {} {} {options}{}", hexf(angle), object_in_way.unwrap_or(0), coord(&pos))
            }
            SpecialPowerAtObject { id, target, options } => format!("group {g} spobj {id} {target} {options}"),
            OverrideSpecialPowerDestination { sp_type, pos } => format!("group {g} override {sp_type}{}", coord(&pos)),
            GuardPosition { pos, mode } => format!("group {g} guardpos {mode}{}", coord(&pos)),
            GuardObject { obj, mode } => format!("group {g} guardobj {obj} {mode}"),
            Idle => format!("group {g} idle"),
            Scatter => format!("group {g} scatter"),
            CreateFormation => format!("group {g} formation"),
            Cheer => format!("group {g} cheer"),
            Enter { obj } => format!("group {g} enter {obj}"),
            Evacuate => format!("group {g} evacuate"),
            ExecuteRailedTransport => format!("group {g} railed"),
            HackInternet => format!("group {g} hack"),
            GetRepaired { obj } => format!("group {g} getrepaired {obj}"),
            Dock { obj } => format!("group {g} dock {obj}"),
            GetHealed { obj } => format!("group {g} gethealed {obj}"),
            Repair { obj } => format!("group {g} repair {obj}"),
            ResumeConstruction { obj } => format!("group {g} resume {obj}"),
            IsIdle => {
                r = self.w(g as i32 + 7);
                format!("group {g} isidle: {}", i32::from(r))
            }
            QueueUpgrade { key } => format!("group {g} queueupgrade {key}"),
            Sell => format!("group {g} sell"),
            ToggleOvercharge => format!("group {g} overcharge"),
        };
        self.line(l);
        Ok(r)
    }
    fn d_ai_execute_waypoint_queue(&mut self, obj: u32) {
        if self.obj[obj as usize].has_ai {
            self.line(format!("waypointqueue {obj}"));
        }
    }
    fn d_release_weapon_lock(&mut self, obj: u32, lock: i32) {
        self.line(format!("releaselock {obj} {lock}"));
    }
    fn d_ai_exit(&mut self, obj: u32, container: u32) {
        if self.obj[obj as usize].has_ai {
            self.line(format!("aiexit {obj} {container}"));
        }
    }
    fn d_is_locally_controlled(&self, obj: u32) -> bool {
        self.obj[obj as usize].local
    }
    fn d_rally_path_exists(&mut self, _obj: u32, pos: &Coord3D) -> bool {
        (pos.x as i32) & 3 != 0
    }
    fn d_drawable_selected(&self, obj: u32) -> bool {
        let o = &self.obj[obj as usize];
        o.has_draw && o.draw_sel
    }
    fn d_set_rally_point(&mut self, obj: u32, pos: &Coord3D) {
        if self.obj[obj as usize].has_exit {
            self.line(format!("rally {obj}{}", coord(pos)));
        }
    }
    fn d_has_production(&self, obj: u32) -> bool {
        self.obj[obj as usize].has_prod
    }
    fn d_cancel_upgrade(&mut self, obj: u32, key: i32) {
        self.line(format!("cancelupgrade {obj} {key}"));
    }
    fn d_queue_create_unit(&mut self, obj: u32, t: i32, p: i32) {
        self.line(format!("queueunit {obj} {t} {p}"));
    }
    fn d_cancel_unit_create(&mut self, obj: u32, p: i32) {
        self.line(format!("cancelunit {obj} {p}"));
    }
    fn d_test_status(&self, obj: u32, status: StatusAsked) -> bool {
        let o = &self.obj[obj as usize];
        match status {
            StatusAsked::UnderConstruction => o.under_cons,
            StatusAsked::Reconstructing => o.recons,
        }
    }
    fn d_refund_build_cost(&mut self, player: i32, obj: u32) {
        let amount = (self.template_of(obj) * 100 + player) as u32;
        self.line(format!("deposit {player} {amount}"));
    }
    fn d_kill(&mut self, obj: u32) {
        self.line(format!("kill {obj}"));
    }
    fn d_destroy_object(&mut self, obj: u32) {
        self.line(format!("destroyobject {obj}"));
    }
    fn d_select_object(&mut self, obj: u32, new: bool, player: i32) {
        self.line(format!("select {obj} {} {:X}", i32::from(new), 1u32 << player));
    }
    fn d_deselect_object(&mut self, obj: u32, player: i32) {
        self.line(format!("deselect {obj} {:X}", 1u32 << player));
    }
    fn d_clear_selected_group(&mut self, player: i32) {
        self.line(format!("clearselection {player}"));
    }
    fn d_upgrade_exists(&self, key: i32) -> bool {
        self.w(key)
    }
    fn d_template_exists(&self, id: i32) -> bool {
        (0..32).contains(&id) && self.w(id + 3)
    }
    fn d_build_object_now(&mut self, c: u32, t: i32, loc: &Coord3D, angle: f32) {
        let owner = self.d_controlling_player(c).unwrap_or(-1);
        self.line(format!("build {c} {t} {} {owner}{}", hexf(angle), coord(loc)));
    }
    fn d_build_object_line_now(&mut self, c: u32, t: i32, start: &Coord3D, end: &Coord3D, angle: f32) {
        let owner = self.d_controlling_player(c).unwrap_or(-1);
        self.line(format!("buildline {c} {t} {} {owner}{}", hexf(angle), coord(start)));
        self.line(format!(" to{}", coord(end)));
    }
    fn d_clamp_to_terrain(&self, p: &Coord3D) -> Coord3D {
        let inside = p.x >= 0.0 && p.y >= 0.0 && p.x <= 1000.0 && p.y <= 800.0;
        if inside {
            return *p;
        }
        let mut c = *p;
        // findClosestEdgePoint's clamp (a NaN stays NaN, as clamp leaves it)
        c.x = c.x.clamp(0.0, 1000.0);
        c.y = c.y.clamp(0.0, 800.0);
        c
    }
    fn d_beacon_template(&self, player: i32) -> Result<Option<i32>, IniError> {
        let t = self.pl[player as usize].beacon_t;
        Ok((0..32).contains(&t).then_some(t))
    }
    fn d_player_defeated(&self, player: i32) -> bool {
        self.pl[player as usize].defeated
    }
    fn d_count_objects_of_template(&self, player: i32, _t: i32) -> i32 {
        self.pl[player as usize].count
    }
    fn d_max_beacons(&self) -> i32 {
        self.max_beacons
    }
    fn d_new_beacon(&mut self, t: i32, player: i32, pos: &Coord3D) -> Result<u32, IniError> {
        let id = self.next_object;
        self.next_object += 1;
        self.line(format!("newobject {id} template {t} team {player}"));
        self.line(format!("setposition {id}{}", coord(pos)));
        self.line(format!("setproducer {id}"));
        Ok(id)
    }
    fn d_relationship(&self, a: i32, b: i32) -> i32 {
        self.rel[a as usize][b as usize]
    }
    fn d_local_is_observer(&self) -> Result<bool, IniError> {
        match self.d_local_player() {
            Some(l) => Ok(self.pl[l as usize].observer),
            None => Err(IniError::undefined(Undefined::NullObject)),
        }
    }
    fn d_is_controllers_beacon(&self, obj: u32) -> Result<bool, IniError> {
        let Some(owner) = self.d_controlling_player(obj) else {
            return Err(IniError::undefined(Undefined::NullObject));
        };
        let t = self.pl[owner as usize].beacon_t;
        if !(0..32).contains(&t) {
            return Err(IniError::undefined(Undefined::NullObject));
        }
        Ok(self.template_of(obj) == t)
    }
    fn d_has_drawable(&self, obj: u32) -> bool {
        obj >= 20 || self.obj[obj as usize].has_draw
    }
    fn d_set_retaliation(&mut self, p: i32, e: bool) {
        self.line(format!("retaliation {p} {}", i32::from(e)));
    }
    fn d_transfer_assets(&mut self, to: i32, from: i32) {
        self.line(format!("transfer {to} from {from}"));
    }
    fn d_kill_player(&mut self, p: i32) {
        self.line(format!("killplayer {p}"));
    }
    fn d_team_message(&mut self, kind: i32, p: i32, n: i32, _msg: &GameMessage) {
        self.line(format!("team {kind} {p} {n}"));
    }
    fn d_purchase_science(&mut self, p: i32, s: i32) {
        self.line(format!("science {p} {s}"));
    }
    fn d_prepare_new_game(&mut self, m: i32, d: i32, r: i32) {
        self.line(format!("preparenewgame {m} {d} {r}"));
    }
    fn d_start_new_game(&mut self) {
        self.line("startnewgame".into());
    }
    fn d_clear_game_data(&mut self) {
        self.line("cleargamedata".into());
    }
    fn d_global_fps_limit(&self) -> i32 {
        self.fps
    }
    fn d_has_stats_collector(&self) -> bool {
        self.stats
    }
    fn d_has_in_game_ui(&self) -> bool {
        true
    }
    fn d_recorder_playback(&self) -> bool {
        self.playback
    }
    fn d_has_recorder(&self) -> bool {
        true
    }
    fn d_recorder_handle_crc(&mut self, crc: u32, p: i32, from: bool) -> Result<(), IniError> {
        self.line(format!("recordercrc {crc:X} {p} {}", i32::from(from)));
        Ok(())
    }
    fn d_replay_camera_follows(&self, player: i32) -> bool {
        let look = ((self.bits >> 28) & 7) as i32;
        let look = if look < self.player_count { look } else { -1 };
        self.cam_follows && look == player
    }
    fn d_camera_movement_finished(&self) -> bool {
        self.cam_finished
    }
    fn d_mouse_moved_recently(&self) -> bool {
        self.mouse_moved
    }
    fn d_has_network(&self) -> bool {
        self.network
    }
    fn d_network_slot_is(&self, slot: i32, player: i32) -> bool {
        self.net_slot[slot as usize] == player
    }
    fn d_network_connected(&self, slot: i32) -> bool {
        (self.net_connected >> slot) & 1 != 0
    }
    fn d_is_local_player(&self, p: i32) -> bool {
        p == self.local
    }
    fn d_client(&mut self, call: ClientCall) {
        let l = match call {
            ClientCall::CollectMsgStats { ty } => format!("stats {}", ty.0),
            ClientCall::UiMessage { label } => format!("ui {label}"),
            ClientCall::Audio { name, pos, player, object } => {
                let head = format!("audio {name} player {} obj {}", player.unwrap_or(-1), object.unwrap_or(0));
                match pos {
                    Some(p) => format!("{head}{}", coord(&p)),
                    None => format!("{head} nopos"),
                }
            }
            ClientCall::MarkUiDirty => "markuidirty".into(),
            ClientCall::RadarInformation { pos } => format!("radar{}", coord(&pos)),
            ClientCall::EvaBeaconDetected => "eva beacon".into(),
            ClientCall::HideBeacon { obj } => format!("hidebeacon {obj}"),
            ClientCall::BeaconCaption { obj, text } => {
                let t: String = text.iter().map(|c| format!(" {c:04x}")).collect();
                format!("caption {obj}{t}")
            }
            ClientCall::ClearPopupMessage => "clearpopup".into(),
            ClientCall::FramesPerSecondLimit { fps } => format!("fps {fps}"),
            ClientCall::ReplayCameraLocation { pos, angle, pitch, zoom } => format!("camera {} {} {}{}", hexf(angle), hexf(pitch), hexf(zoom), coord(&pos)),
            ClientCall::ReplayMouse { cursor, pos } => format!("mouse {cursor} {} {}", pos.x, pos.y),
            ClientCall::DeselectAllDrawables => "deselectall".into(),
            ClientCall::SelectDrawable { obj } => format!("selectdrawable {obj}"),
        };
        self.line(l);
    }
}

type World = [u32; 38];

fn host_from(w: &World) -> Host {
    let f = w[0];
    let mut obj = [ObjW::default(); 13];
    for (i, o) in obj.iter_mut().enumerate().skip(1) {
        let b = w[6 + i];
        *o = ObjW {
            exists: b & 1 != 0,
            has_ai: b >> 1 & 1 != 0,
            local: b >> 2 & 1 != 0,
            draw_sel: b >> 3 & 1 != 0,
            has_exit: b >> 4 & 1 != 0,
            has_prod: b >> 5 & 1 != 0,
            under_cons: b >> 6 & 1 != 0,
            recons: b >> 7 & 1 != 0,
            has_draw: b >> 8 & 1 != 0,
            owner: ((b >> 9) & 7) as i32 - 1,
            beacon_of: ((b >> 12) & 63) as i32 - 1,
        };
    }
    let mut pl = [PlW::default(); 6];
    let mut rel = [[0i32; 6]; 6];
    let mut sel: [Vec<u32>; 6] = Default::default();
    for p in 0..6 {
        let b = w[19 + p];
        pl[p] = PlW { defeated: b & 1 != 0, beacon_t: ((b >> 1) & 63) as i32 - 1, count: ((b >> 7) & 7) as i32, observer: b >> 10 & 1 != 0 };
        for (q, r) in rel[p].iter_mut().enumerate() {
            *r = ((w[25 + p] >> (2 * q)) & 3) as i32;
        }
        sel[p] = (0..12).filter(|i| (w[31 + p] >> i) & 1 != 0).map(|i| i + 1).collect();
    }
    let mut net_slot = [0i32; 8];
    for (s, n) in net_slot.iter_mut().enumerate() {
        *n = ((w[37] >> (4 * s)) & 15) as i32 - 1;
    }
    Host {
        log: String::new(),
        obj,
        pl,
        rel,
        sel,
        player_count: w[2] as i32,
        local: w[3] as i32,
        max_beacons: w[4] as i32,
        fps: w[5] as i32,
        in_game: f & 1 != 0,
        stats: f >> 1 & 1 != 0,
        playback: f >> 4 & 1 != 0,
        cam_follows: f >> 5 & 1 != 0,
        cam_finished: f >> 6 & 1 != 0,
        mouse_moved: f >> 7 & 1 != 0,
        network: f >> 8 & 1 != 0,
        net_connected: w[6],
        net_slot,
        bits: w[1],
        groups: BTreeMap::new(),
        next_group: 100,
        next_object: 20,
    }
}

/// A message as both sides build it: type, player, and per argument its type and four words.
#[derive(Clone, Debug)]
struct Msg {
    ty: i32,
    player: i32,
    args: Vec<(i32, [u32; 4])>,
}

fn end_line(s: &DispatchState) -> String {
    let crcs: String = s.cached_crcs.iter().map(|c| format!(" {c:X}")).collect();
    format!("end plan {} validate {} crcs{crcs}\n", i32::from(s.build_plan), i32::from(s.should_validate_crcs))
}

fn rust_run(w: &World, msgs: &[Msg]) -> (String, Option<(usize, String)>) {
    let mut h = host_from(w);
    let mut st = DispatchState::default();
    let mut arena = Messages::new();
    let mut cut = None;
    for (i, m) in msgs.iter().enumerate() {
        let id = arena.new_message(MessageType(m.ty), 0);
        let gm = arena.message_mut(id).expect("new");
        gm.set_player_index(m.player);
        for (t, words) in &m.args {
            gm.append_argument(GameMessageArgument {
                data: ArgumentData(*words),
                ty: ArgumentDataType::from_i32(*t).unwrap_or(ArgumentDataType::Unknown),
            });
        }
        let gm = arena.message(id).expect("made").clone();
        let before = h.log.len();
        let saved = st.clone();
        let _ = writeln!(h.log, "msg {}", m.ty);
        if let Err(e) = logic_message_dispatcher(&mut st, &gm, &mut h) {
            h.log.truncate(before);
            st = saved;
            cut = Some((i, format!("{e:?}")));
            break;
        }
        arena.delete_message(id).expect("delete");
    }
    h.log.push_str(&end_line(&st));
    (h.log, cut)
}

fn cpp_run(w: &World, msgs: &[Msg]) -> String {
    let mut flat = Vec::new();
    for m in msgs {
        flat.push(m.ty as u32);
        flat.push(m.player as u32);
        flat.push(m.args.len() as u32);
        for (t, words) in &m.args {
            flat.push(*t as u32);
            flat.extend_from_slice(words);
        }
    }
    let mut cap = 1 << 18;
    loop {
        let mut out = vec![0u8; cap];
        let len = unsafe { cpp::ref_dispatch_run(w.as_ptr(), w.len() as i32, flat.as_ptr(), msgs.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if len <= cap {
            out.truncate(len);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = len;
    }
}

fn gen_world(g: &mut Gen) -> World {
    let mut w = [0u32; 38];
    w[0] = g.next() & 0x1ff;
    if g.chance(4) {
        w[0] |= 1;
    }
    w[1] = g.next();
    w[2] = g.below(7);
    w[3] = if g.chance(5) { 9 } else { g.below(6) };
    w[4] = g.below(4);
    w[5] = g.below(100);
    w[6] = g.next() & 0xff;
    for i in 1..=12 {
        let mut b = g.next() & 0x1ff;
        if !g.chance(4) {
            b |= 1;
        }
        b |= g.below(8) << 9;
        b |= (if g.chance(3) { g.below(5) } else { 0 }) << 12;
        w[6 + i] = b;
    }
    for p in 0..6 {
        let bt = if g.chance(4) { 0 } else { 1 + g.below(4) };
        w[19 + p] = u32::from(g.chance(4)) | (bt << 1) | (g.below(5) << 7) | (u32::from(g.chance(4)) << 10);
        w[25 + p] = g.next() & 0xfff;
        w[31 + p] = if g.chance(5) { 0 } else { g.next() & 0xfff };
    }
    w[37] = g.next();
    w
}

fn loc(g: &mut Gen) -> [u32; 4] {
    let x = g.below(1300) as f32 - 150.0 + if g.chance(2) { 0.5 } else { 0.0 };
    let y = g.below(1000) as f32 - 100.0;
    let z = g.below(50) as f32;
    [x.to_bits(), y.to_bits(), z.to_bits(), 0]
}

fn int(v: u32) -> (i32, [u32; 4]) {
    (0, [v, 0, 0, 0])
}
fn obj(g: &mut Gen) -> (i32, [u32; 4]) {
    (3, [g.below(15), 0, 0, 0])
}

const TYPES: [MessageType; 52] = [
    MSG_NEW_GAME,
    MSG_CLEAR_GAME_DATA,
    MSG_META_BEGIN_PATH_BUILD,
    MSG_META_END_PATH_BUILD,
    MSG_SET_RALLY_POINT,
    MSG_DO_WEAPON,
    MSG_COMBATDROP_AT_OBJECT,
    MSG_COMBATDROP_AT_LOCATION,
    MSG_DO_WEAPON_AT_OBJECT,
    MSG_SWITCH_WEAPONS,
    MSG_SET_MINE_CLEARING_DETAIL,
    MSG_ENABLE_RETALIATION_MODE,
    MSG_DO_WEAPON_AT_LOCATION,
    MSG_DO_SPECIAL_POWER,
    MSG_DO_SPECIAL_POWER_AT_LOCATION,
    MSG_DO_SPECIAL_POWER_AT_OBJECT,
    MSG_DO_ATTACKMOVETO,
    MSG_DO_FORCEMOVETO,
    MSG_DO_SALVAGE,
    MSG_DO_MOVETO,
    MSG_ADD_WAYPOINT,
    MSG_DO_GUARD_POSITION,
    MSG_DO_GUARD_OBJECT,
    MSG_DO_STOP,
    MSG_DO_SCATTER,
    MSG_CREATE_FORMATION,
    MSG_CLEAR_INGAME_POPUP_MESSAGE,
    MSG_DO_CHEER,
    MSG_ENTER,
    MSG_EXIT,
    MSG_EVACUATE,
    MSG_EXECUTE_RAILED_TRANSPORT,
    MSG_INTERNET_HACK,
    MSG_GET_REPAIRED,
    MSG_DOCK,
    MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION,
    MSG_DO_ATTACK_OBJECT,
    MSG_DO_FORCE_ATTACK_OBJECT,
    MSG_DO_FORCE_ATTACK_GROUND,
    MSG_QUEUE_UPGRADE,
    MSG_CANCEL_UPGRADE,
    MSG_QUEUE_UNIT_CREATE,
    MSG_CANCEL_UNIT_CREATE,
    MSG_DOZER_CONSTRUCT,
    MSG_DOZER_CONSTRUCT_LINE,
    MSG_DOZER_CANCEL_CONSTRUCT,
    MSG_CREATE_SELECTED_GROUP,
    MSG_REMOVE_FROM_SELECTED_GROUP,
    MSG_PLACE_BEACON,
    MSG_REMOVE_BEACON,
    MSG_SET_BEACON_TEXT,
    MSG_SELF_DESTRUCT,
];

const MORE: [MessageType; 14] = [
    MSG_GET_HEALED,
    MSG_DO_REPAIR,
    MSG_RESUME_CONSTRUCTION,
    MSG_SELL,
    MSG_TOGGLE_OVERCHARGE,
    MSG_CREATE_SELECTED_GROUP_NO_SOUND,
    MSG_DESTROY_SELECTED_GROUP,
    MSG_SELECTED_GROUP_COMMAND,
    MSG_SET_REPLAY_CAMERA,
    MSG_CREATE_TEAM3,
    MSG_SELECT_TEAM0,
    MSG_ADD_TEAM9,
    MSG_LOGIC_CRC,
    MSG_PURCHASE_SCIENCE,
];

fn gen_msg(g: &mut Gen, w: &World) -> Msg {
    let ty = match g.below(20) {
        0 => MessageType(g.below(2100) as i32),
        1..=4 => MORE[g.below(MORE.len() as u32) as usize],
        _ => TYPES[g.below(TYPES.len() as u32) as usize],
    };
    let player = if g.chance(8) { g.below(9) as i32 - 1 } else { (g.below(w[2].max(1))) as i32 };
    let t = ty;
    let mut args: Vec<(i32, [u32; 4])> = if t == MSG_NEW_GAME {
        (0..g.below(5)).map(|_| int(g.below(1200))).collect()
    } else if t == MSG_SET_RALLY_POINT {
        vec![obj(g), (6, loc(g))]
    } else if t == MSG_DO_WEAPON || t == MSG_SWITCH_WEAPONS {
        vec![int(g.below(3)), int(g.below(5))]
    } else if t == MSG_COMBATDROP_AT_OBJECT || t == MSG_DO_GUARD_OBJECT || t == MSG_GET_REPAIRED || t == MSG_DOCK || t == MSG_GET_HEALED || t == MSG_DO_REPAIR || t == MSG_RESUME_CONSTRUCTION || t == MSG_DO_ATTACK_OBJECT || t == MSG_DO_FORCE_ATTACK_OBJECT || t == MSG_EXIT {
        vec![obj(g), int(g.below(3))]
    } else if t == MSG_DO_WEAPON_AT_OBJECT {
        vec![int(g.below(3)), obj(g), int(g.below(5))]
    } else if t == MSG_ENABLE_RETALIATION_MODE {
        vec![int(g.below(8).wrapping_sub(1)), (2, [g.below(2), 0, 0, 0])]
    } else if t == MSG_DO_WEAPON_AT_LOCATION {
        vec![int(g.below(3)), (6, loc(g)), int(g.below(5))]
    } else if t == MSG_DO_SPECIAL_POWER {
        vec![int(g.below(40)), int(g.below(16)), obj(g)]
    } else if t == MSG_DO_SPECIAL_POWER_AT_LOCATION {
        let a = (g.below(628) as f32 / 100.0).to_bits();
        vec![int(g.below(40)), (6, loc(g)), (1, [a, 0, 0, 0]), obj(g), int(g.below(16)), obj(g)]
    } else if t == MSG_DO_SPECIAL_POWER_AT_OBJECT {
        vec![int(g.below(40)), obj(g), int(g.below(16)), obj(g)]
    } else if t == MSG_DO_SPECIAL_POWER_OVERRIDE_DESTINATION {
        // the location's x is read as an object id by the fall-through: small whole values reach real objects
        let mut l = loc(g);
        if g.chance(2) {
            l[0] = g.below(15);
        }
        vec![(6, l), int(g.below(12)), obj(g)]
    } else if t == MSG_ENTER {
        vec![obj(g), obj(g)]
    } else if [MSG_COMBATDROP_AT_LOCATION, MSG_DO_ATTACKMOVETO, MSG_DO_FORCEMOVETO, MSG_DO_SALVAGE, MSG_DO_MOVETO, MSG_ADD_WAYPOINT, MSG_DO_FORCE_ATTACK_GROUND, MSG_PLACE_BEACON].contains(&t) {
        vec![(6, loc(g))]
    } else if t == MSG_DO_GUARD_POSITION {
        vec![(6, loc(g)), int(g.below(3))]
    } else if t == MSG_QUEUE_UPGRADE {
        vec![obj(g), int(g.below(40))]
    } else if t == MSG_CANCEL_UPGRADE || t == MSG_CANCEL_UNIT_CREATE {
        vec![int(g.below(40))]
    } else if t == MSG_QUEUE_UNIT_CREATE {
        vec![int(g.below(36)), int(g.below(9))]
    } else if t == MSG_DOZER_CONSTRUCT || t == MSG_DOZER_CONSTRUCT_LINE {
        let a = (g.below(628) as f32 / 100.0).to_bits();
        vec![int(g.below(36)), (6, loc(g)), (1, [a, 0, 0, 0]), (6, loc(g))]
    } else if t == MSG_CREATE_SELECTED_GROUP || t == MSG_CREATE_SELECTED_GROUP_NO_SOUND {
        let mut v = vec![(2, [g.below(2), 0, 0, 0])];
        for _ in 0..g.below(5) {
            v.push(obj(g));
        }
        v
    } else if t == MSG_REMOVE_FROM_SELECTED_GROUP {
        (0..g.below(5)).map(|_| obj(g)).collect()
    } else if t == MSG_SET_BEACON_TEXT {
        (0..g.below(6)).map(|_| (10, [if g.chance(6) { 0 } else { 0x41 + g.below(40) }, 0, 0, 0])).collect()
    } else if t == MSG_SELF_DESTRUCT {
        vec![(2, [g.below(2), 0, 0, 0])]
    } else if t == MSG_SET_REPLAY_CAMERA {
        let f = |g: &mut Gen| (1, [(g.below(1000) as f32 / 10.0).to_bits(), 0, 0, 0]);
        vec![(6, loc(g)), f(g), f(g), f(g), int(g.below(10)), (7, [g.below(800), g.below(600), 0, 0])]
    } else if t == MSG_LOGIC_CRC {
        vec![int(g.next()), (2, [g.below(2), 0, 0, 0])]
    } else if t == MSG_PURCHASE_SCIENCE {
        vec![int(g.below(10).wrapping_sub(1))]
    } else {
        (0..g.below(3)).map(|_| (g.below(11) as i32, [g.next(), g.next(), 0, 0])).collect()
    };
    // sometimes an argument of another kind, or one short
    if g.chance(15) && !args.is_empty() {
        let i = g.below(args.len() as u32) as usize;
        args[i].0 = g.below(11) as i32;
        args[i].1[0] = g.below(20);
    }
    if g.chance(20) {
        args.pop();
    }
    Msg { ty: ty.0, player, args }
}

#[test]
fn dispatch_matches_upstream() {
    let cases: usize = std::env::var("DISPATCH_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen(0x4449_5350_0001_7777);
    let (mut mismatches, mut cut, mut msgs_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why = BTreeMap::<String, usize>::new();
    let mut by_type = BTreeMap::<i32, usize>::new();
    for case in 0..cases {
        let w = gen_world(&mut g);
        let msgs: Vec<Msg> = (0..1 + g.below(12)).map(|_| gen_msg(&mut g, &w)).collect();
        let (r, undefined_at) = rust_run(&w, &msgs);
        let n = undefined_at.as_ref().map_or(msgs.len(), |u| u.0);
        if let Some((i, why)) = &undefined_at {
            cut += 1;
            *cut_why.entry(format!("{why} (type {})", msgs[*i].ty)).or_insert(0) += 1;
        }
        let c = cpp_run(&w, &msgs[..n]);
        msgs_run += n;
        for m in &msgs[..n] {
            *by_type.entry(m.ty).or_insert(0) += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!(
                    "case {case}: world {w:?}\n msgs {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}\n  before: {:?}",
                    &msgs[..n],
                    rl.get(at),
                    cl.get(at),
                    &rl[at.saturating_sub(4)..at]
                );
            }
        } else {
            lines += r.lines().count();
        }
    }
    println!("D1: {cases} programs ({cut} cut at an undefined message), {msgs_run} messages, {lines} lines, {mismatches} mismatches; {} types", by_type.len());
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
