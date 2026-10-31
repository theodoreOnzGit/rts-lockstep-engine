#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptActions.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ScriptActions` (`logic::world::script_actions`) against the C++, `p1w13-sonnet`, layer 11 (batches A1-A10).
//!
//! # Method
//!
//! The C++ side is `ScriptActions.cpp` and its header, compiled by `differential/wrap_actions.cpp` on the stand-ins of
//! `differential/shim_actions` (`sa_world.h` and its two parts), with the real `PartitionSolver.cpp`. The header is build.rs's copy
//! with one edit (the opaque `enum AudioAffect;` given `: int`, which clang requires), the file build.rs's copy with two (old MSVC's
//! `for` scope and its `'\0'` null pointer, see build.rs). The world is a configuration of 256 words and its own state: sixteen
//! objects and sixteen teams from the configuration, the objects and teams the factories make, the AI groups, the windows, the money;
//! eight players, team prototypes (with their reinforcement templates), waypoints and templates. Every call with an effect writes a
//! line (floats as their bits); the answers come from the configuration, the state, or a hash of a seed and the question.
//!
//! The Rust side is the module on [`ScriptActionsHost`], implemented here on the same world with the same functions.
//!
//! A program is the configuration, up to eight actions of the translated types (each with the parameters its execution reads,
//! sometimes too few, and with strings that name things that exist or not), and 1 to 40 operations: execute an action, change a
//! configuration word, `init`, or rebuild the world. Compared after each: the lines, the window, `m_suppressNewWindows` and the global
//! data the actions write. When the Rust side finds upstream undefined at an operation, the C++ runs the operations before it.
//!
//! # Results (2026-10-30, `ACTIONS_CASES=40000`, the default; batches A1-A10, all 344 types of `executeAction`)
//!
//! 40,000 programs, 719,958 operations compared (502,310 executions, every one of the 344 types executed), 7,564,577 output words:
//! **0 mismatches**. 5,483 programs were cut at an operation upstream does not define (their prefix compared): 3,422 an action with
//! too few parameters; 581 an uninitialised local (`doDisplayCinematicText` reading past its string, `getHumanPlayerMask`'s mask when
//! revealing or shrouding at a waypoint without a player, the position of `doSkirmishAttackNearestGroupWithValue` for any comparison
//! but `>=` and `>`, the capture's search); 291 a NULL object (`doCameoFlash`'s missing button, the unmanned actions without a
//! neutral player); 254 the local player missing; 210 a unit or team with no player (the garrison actions); 203 `doC3CameraShake`'s
//! waypoint missing (it is not checked); 181 a `Real` an `Int` cannot hold (camera times, hulk lifetimes); 178 a disbanded team's
//! player with no default team; 117 an `Int` overflow (`doCameoFlash`'s `30*seconds`); 46 an array index out of range. Every effect
//! in `REACH` occurs.
//!
//! The first 40,000-program run of A5-A10 had one mismatch, a defect of this test, not of the module: the C++ stand-in builds each
//! player's team list once, in `scene()`, and the Rust host read the prototype's owner from the live configuration, so a program that
//! changed a prototype word and then fired `SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST` saw different teams. The host now keeps the
//! owners `scene()` saw.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`, 40,000 programs: all 29 caught. 3868 needs the 40,000 (at 20,000 it was missed once all 344 types shared the
//! programs); 3881 needed a configuration bit (`cfg[0]` bit 11) that names the last player `ReplayObserver`, and 3882 the generator
//! to put half the objects on the first four teams. A plant must not change what the Rust side finds undefined, or the C++ runs past
//! it: the first 3882 (the highest id selected) crashed the C++ that way, as the selection is read later, so it moves the camera only.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3861 | the reinforcements' transports exit when told not to | 19 |
//! | 3862 | a team without a transport not moved to its goal | 26 |
//! | 3863 | the hunt refuses a special power that needs an object target | 1 |
//! | 3864 | wander and panic swapped | 136 |
//! | 3865 | the reinforcements' rows `2.25` radii apart | 23 |
//! | 3866 | the observer window not shown after the local defeat window | 199 |
//! | 3867 | the unsellable flag inverted | 50 |
//! | 3868 | the selection's mean height not divided | 2 |
//! | 3869 | a name that existed not transferred | 57 |
//! | 3870 | 0 transport slots read as "lots" when loading | 14 |
//! | 3871 | the hunt's message reworded | 3 |
//! | 3872 | bold read anywhere in the font string | 25 |
//! | 3873 | the merge's first member moved once | 65 |
//! | 3874 | the garrison search stops at the first member that cannot garrison | 13 |
//! | 3875 | a structure nobody entered not garrisoned | 4 |
//! | 3876 | the subtitle shown when the text does not exist | 88 |
//! | 3877 | the camera's time rounded | 17 |
//! | 3878 | the skirmish special power fired by every ready member, not the first | 8 |
//! | 3879 | a disbanded team's members made unrecruitable | 5 |
//! | 3880 | the projectile follows its path as a team | 25 |
//! | 3881 | the border switch leaves the observer's map revealed | 174 |
//! | 3882 | the forced selection's camera on the highest id | 2 |
//! | 3883 | the team moving to the nearest object skips a member without AI | 25 |
//! | 3884 | the most valuable target searched 1 further | 36 |
//! | 3885 | one member more uses the partial command button | 24 |
//! | 3886 | the global difficulty bonus flag inverted | 360 |
//! | 3887 | the face actions keep the waypoint queue | 264 |
//! | 3888 | the stopping distance never set | 69 |
//! | 3889 | the unmanned unit deselected without affecting the client | 266 |

use rts_lockstep_engine::common::base_type::{Coord2D, Coord3D};
use rts_lockstep_engine::common::disabled_types::{DisabledType, DISABLED_HELD, DISABLED_UNMANNED};
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::kind_of::{KindOfType, KINDOF_STRUCTURE};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::scripts::{action_type as at, parameter_type, Parameter, ScriptAction, MAX_PARMS};
use rts_lockstep_engine::logic::world::script_actions::*;

mod support;
use support::Gen;

const NCFG: usize = 256;

fn hh(seed: u32, a: u32, b: u32) -> u32 {
    let mut x = seed ^ a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x
}

/// `saNum`: the number after `prefix`, or -1.
fn num(s: &[u8], prefix: &str) -> i32 {
    let p = prefix.as_bytes();
    if !s.starts_with(p) || s.len() == p.len() {
        return -1;
    }
    let mut v = 0i32;
    for &c in &s[p.len()..] {
        if !c.is_ascii_digit() || v > 1000 {
            return -1;
        }
        v = v * 10 + i32::from(c - b'0');
    }
    v
}

fn strhash(s: &[u8]) -> u32 {
    let mut h = 2_166_136_261u32;
    for &c in s {
        h ^= u32::from(c);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

fn fb(r: f32) -> String {
    format!("{:08x}", r.to_bits())
}
fn st(s: &[u8]) -> String {
    String::from_utf8_lossy(s).into_owned()
}

#[derive(Clone, Debug)]
struct Obj {
    name: Vec<u8>,
    tmpl: Option<i32>,
    team: Option<i32>,
    pos: Coord3D,
    angle: f32,
    killed: bool,
    contained_by: Option<i32>,
    contained: Vec<i32>,
}

#[derive(Clone, Debug)]
struct Team {
    proto: Option<i32>,
    members: Vec<i32>,
    controller: Option<i32>,
}

/// The world a program runs on (the stand-ins' `g_cfg`, `g_world` and the rest).
struct Sim {
    cfg: [u32; NCFG],
    log: Vec<String>,
    objs: Vec<Obj>,
    teams: Vec<Team>,
    protos: Vec<Vec<i32>>,
    /// Each prototype's owning player as `scene()` saw it (the stand-ins build `Player::m_teams` once, there).
    proto_owner: [i32; 8],
    next_group: i32,
    next_window: i32,
    light_scale: f32,
    /// `TheCampaignManager` exists (set when the world is built).
    campaign: bool,
    /// `TheGlobalData->m_timeOfDay` is night (set when the world is built).
    night: bool,
    /// `TheGlobalData->m_clearAlpha`, `m_shroudAlpha` (set when the world is built).
    alphas: (u8, u8),
    /// Each player's money.
    money: Vec<u32>,
    /// Each AI group's members (as `getTeamAsAIGroup` set them).
    groups: Vec<Vec<ObjH>>,
    /// `TheWritableGlobalData->m_useFpsLimit`.
    use_fps: bool,
}

impl Sim {
    fn new(cfg: &[u32; NCFG]) -> Sim {
        let mut s = Sim { cfg: *cfg, log: Vec::new(), objs: Vec::new(), teams: Vec::new(), protos: Vec::new(), proto_owner: [-1; 8], next_group: 0, next_window: 0, light_scale: -1.0, campaign: true, night: false, alphas: (0, 0), money: Vec::new(), groups: Vec::new(), use_fps: true };
        s.scene();
        s
    }
    fn seed(&self) -> u32 {
        self.cfg[7]
    }
    fn ntemplates(&self) -> i32 {
        ((self.cfg[11] & 15) as i32).min(8)
    }
    fn nplayers(&self) -> i32 {
        ((self.cfg[4] & 15) as i32).min(8)
    }
    fn player(&self, p: i32) -> Option<i32> {
        (p >= 0 && p < self.nplayers()).then_some(p)
    }
    fn tw(&self, t: i32) -> u32 {
        self.cfg[24 + t as usize]
    }
    /// `scene()`: the world from the configuration.
    fn scene(&mut self) {
        self.objs.clear();
        self.teams.clear();
        self.protos = vec![Vec::new(); 8];
        for k in 0..8 {
            self.proto_owner[k] = (self.cfg[96 + k] & 15) as i32;
        }
        self.next_group = 0;
        self.groups.clear();
        self.next_window = 0;
        for _ in 0..16 {
            self.teams.push(Team { proto: None, members: Vec::new(), controller: None });
        }
        for k in 0..8 {
            for t in 0..16 {
                if (self.cfg[96 + k] >> (4 + t)) & 1 != 0 && self.teams[t].proto.is_none() {
                    self.teams[t].proto = Some(k as i32);
                    self.protos[k].push(t as i32);
                }
            }
        }
        for j in 0..16usize {
            let w0 = self.cfg[40 + 4 * j];
            let w3 = self.cfg[40 + 4 * j + 3];
            let t = ((w0 >> 12) & 7) as i32;
            let team = ((w0 >> 20) & 15) as i32;
            self.teams[team as usize].members.push(j as i32 + 1);
            self.objs.push(Obj {
                name: if w0 & 1 != 0 { format!("unit{j}").into_bytes() } else { Vec::new() },
                tmpl: (t < self.ntemplates()).then_some(t),
                team: Some(team),
                pos: Coord3D { x: (w3 & 255) as f32 * 1.5, y: ((w3 >> 8) & 255) as f32 * 1.5, z: 0.0 },
                angle: 0.0,
                killed: false,
                contained_by: None,
                contained: Vec::new(),
            });
        }
        self.light_scale = -1.0;
        self.use_fps = true;
        self.money = (0..8).map(|p| self.cfg[128 + 4 * p + 2]).collect();
        self.campaign = (self.cfg[0] >> 10) & 1 == 0;
        self.night = (self.cfg[15] >> 4) & 7 == 4;
        self.alphas = (((self.cfg[15] >> 8) & 255) as u8, ((self.cfg[15] >> 16) & 255) as u8);
    }
    fn o(&self, id: ObjH) -> &Obj {
        &self.objs[(id - 1) as usize]
    }
    /// `Object::w(k)`: the configuration's words for the first sixteen, zero for the made ones.
    fn ow(&self, id: ObjH, k: usize) -> u32 {
        if id <= 16 {
            self.cfg[40 + 4 * (id - 1) as usize + k]
        } else {
            0
        }
    }
    fn waypoint_exists(&self, k: i32) -> bool {
        (0..8).contains(&k) && (self.cfg[12] >> k) & 1 != 0
    }
    fn drawable(&self, id: ObjH) -> bool {
        id > 16 || (self.ow(id, 0) >> 5) & 1 != 0
    }
}

impl ScriptActionsHost for Sim {
    fn sa_win_create_from_script(&mut self, name: &[u8]) -> WindowH {
        self.next_window += 1;
        self.log.push(format!("window create {} {}", st(name), self.next_window));
        self.next_window
    }
    fn sa_win_destroy(&mut self, w: WindowH) {
        self.log.push(format!("window destroy {w}"));
    }
    fn sa_logic_close_windows(&mut self) {
        self.log.push("logic closewindows".into());
    }
    fn sa_find_object_by_id(&self, id: ObjectId) -> Option<ObjH> {
        (id >= 1 && id as usize <= self.objs.len()).then_some(id)
    }
    fn sa_ui_set_input_enabled(&mut self, enabled: bool) {
        self.log.push(format!("ui input {}", u8::from(enabled)));
    }
    fn sa_mouse_set_visibility(&mut self, visible: bool) {
        self.log.push(format!("mouse visible {}", u8::from(visible)));
    }
    fn sa_ui_deselect_all_drawables(&mut self) {
        self.log.push("ui deselectall".into());
    }
    fn sa_ui_clear_attack_move_to_mode(&mut self) {
        self.log.push("ui clearattackmove".into());
    }
    fn sa_ui_set_waypoint_mode(&mut self, on: bool) {
        self.log.push(format!("ui waypointmode {}", u8::from(on)));
    }
    fn sa_control_bar_delete_build_tooltip_layout(&mut self) {
        self.log.push("controlbar deletetooltip".into());
    }
    fn sa_look_at_reset_modes(&mut self) {
        self.log.push("lookat reset".into());
    }
    fn sa_has_campaign_manager(&self) -> bool {
        self.campaign
    }
    fn sa_campaign_set_victorious(&mut self, victorious: bool) {
        self.log.push(format!("campaign victorious {}", u8::from(victorious)));
    }
    fn sa_am_i_observer(&self) -> bool {
        (self.cfg[0] >> 9) & 1 != 0
    }
    fn sa_set_infantry_light_scale(&mut self, setting: f32) {
        self.light_scale = setting;
    }
    fn sa_is_night(&self) -> bool {
        self.night
    }

    fn sa_start_quick_end_game_timer(&mut self) {
        self.log.push("se quickendgame".into());
    }
    fn sa_start_end_game_timer(&mut self) {
        self.log.push("se endgame".into());
    }
    fn sa_start_close_window_timer(&mut self) {
        self.log.push("se closewindow".into());
    }
    fn sa_has_shown_mp_local_defeat_window(&self) -> bool {
        (self.cfg[0] >> 8) & 1 != 0
    }
    fn sa_mark_mp_local_defeat_window_shown(&mut self) {
        self.log.push("se markdefeatshown".into());
    }
    fn sa_append_debug_message(&mut self, message: &[u8], pause: bool) {
        self.log.push(format!("se debug {} {}", st(message), u8::from(pause)));
    }
    fn sa_unit_named(&self, name: &[u8]) -> Option<ObjH> {
        if name.is_empty() {
            return None;
        }
        self.objs.iter().position(|o| o.name == name).map(|i| i as i32 + 1)
    }
    fn sa_team_named(&self, name: &[u8]) -> Option<TeamH> {
        if name == b"<This Team>" {
            let t = self.cfg[5] & 255;
            return ((t as usize) < self.teams.len()).then_some(t as i32 + 1);
        }
        let k = num(name, "team");
        if !(0..8).contains(&k) {
            let k = num(name, "lone");
            return (k >= 0 && (k as usize) < self.teams.len() && self.teams[k as usize].proto.is_none()).then_some(k + 1);
        }
        self.protos[k as usize].first().map(|t| t + 1)
    }
    fn sa_did_unit_exist(&self, name: &[u8]) -> bool {
        let k = num(name, "unit");
        (0..32).contains(&k) && (self.cfg[10] >> k) & 1 != 0
    }
    fn sa_transfer_object_name(&mut self, name: &[u8], o: ObjH) {
        self.log.push(format!("se transfername {} {o}", st(name)));
    }
    fn sa_add_object_to_cache(&mut self, o: ObjH) {
        self.log.push(format!("se addcache {o}"));
    }
    fn sa_current_player(&self) -> Option<PlayerH> {
        self.player(((self.cfg[5] >> 8) & 15) as i32)
    }
    fn sa_player_from_name(&self, name: &[u8]) -> Option<PlayerH> {
        if name == b"<This Player>" {
            return self.sa_current_player();
        }
        if name == b"<This Player's Enemy>" {
            return self.player(((self.cfg[5] >> 12) & 15) as i32);
        }
        self.player(num(name, "player"))
    }

    fn sa_local_player(&self) -> Option<PlayerH> {
        self.player(((self.cfg[4] >> 4) & 15) as i32)
    }
    fn sa_player_is_observer(&self, p: PlayerH) -> bool {
        (self.cfg[128 + 4 * p as usize] >> 1) & 1 != 0
    }
    fn sa_player_build_specific_building(&mut self, p: PlayerH, b: &[u8]) {
        self.log.push(format!("player {p} building {}", st(b)));
    }
    fn sa_player_build_by_supplies(&mut self, p: PlayerH, cash: i32, b: &[u8]) {
        self.log.push(format!("player {p} bysupplies {cash} {}", st(b)));
    }
    fn sa_player_build_specific_building_nearest_team(&mut self, p: PlayerH, b: &[u8], t: TeamH) {
        self.log.push(format!("player {p} buildingnearest {} {t}", st(b)));
    }
    fn sa_player_build_upgrade(&mut self, p: PlayerH, u: &[u8]) {
        self.log.push(format!("player {p} upgrade {}", st(u)));
    }
    fn sa_player_build_base_defense(&mut self, p: PlayerH, flank: bool) {
        self.log.push(format!("player {p} basedefense {}", u8::from(flank)));
    }
    fn sa_player_build_base_defense_structure(&mut self, p: PlayerH, b: &[u8], flank: bool) {
        self.log.push(format!("player {p} basestructure {} {}", st(b), u8::from(flank)));
    }

    fn sa_audio_add_event(&mut self, e: &AudioEvent) {
        let mut l = format!("audio add {} {} {} {} {} {}", st(&e.name), u8::from(e.logical), e.player_index, e.object, u8::from(e.uninterruptable), u8::from(e.should_fade));
        if let Some(p) = e.position {
            l += &format!(" {} {} {}", fb(p.x), fb(p.y), fb(p.z));
        }
        self.log.push(l);
    }
    fn sa_waypoint_by_name(&self, name: &[u8]) -> Option<WaypointH> {
        let k = num(name, "wp");
        self.waypoint_exists(k).then_some(k)
    }
    fn sa_first_waypoint(&self) -> Option<WaypointH> {
        (0..8).find(|k| self.waypoint_exists(*k))
    }
    fn sa_waypoint_next(&self, w: WaypointH) -> Option<WaypointH> {
        ((self.cfg[12] >> (8 + w)) & 1 != 0 && self.waypoint_exists(w + 1)).then_some(w + 1)
    }
    fn sa_waypoint_name(&self, w: WaypointH) -> Vec<u8> {
        format!("wp{w}").into_bytes()
    }
    fn sa_waypoint_location(&self, w: WaypointH) -> Coord3D {
        let v = self.cfg[32 + w as usize];
        Coord3D { x: (v & 255) as f32 * 2.5, y: ((v >> 8) & 255) as f32 * 2.5, z: ((v >> 16) & 15) as f32 }
    }
    fn sa_ground_height(&self, x: f32, y: f32) -> f32 {
        (hh(self.seed(), x.to_bits(), y.to_bits()) % 16) as f32 * 0.5
    }
    fn sa_create_crater_in_terrain(&mut self, o: ObjH) {
        self.log.push(format!("terrain crater {o}"));
    }
    fn sa_pathfinder_add_object(&mut self, o: ObjH) {
        self.log.push(format!("pathfind add {o}"));
    }
    fn sa_oversize_the_terrain(&mut self, amount: i32) {
        self.log.push(format!("terrain oversize {amount}"));
    }

    fn sa_view_set_camera_lock(&mut self, id: ObjectId) {
        self.log.push(format!("view lock {id}"));
    }
    fn sa_view_snap_to_camera_lock(&mut self) {
        self.log.push("view snaplock".into());
    }
    fn sa_view_set_snap_mode(&mut self, t: i32, d: f32) {
        self.log.push(format!("view snapmode {t} {}", fb(d)));
    }
    fn sa_view_move_camera_to(&mut self, p: &Coord3D, frames: i32, shutter: i32, orient: bool, ei: f32, eo: f32) {
        self.log.push(format!("view moveto {} {} {} {frames} {shutter} {} {} {}", fb(p.x), fb(p.y), fb(p.z), u8::from(orient), fb(ei), fb(eo)));
    }
    fn sa_view_move_camera_along_waypoint_path(&mut self, w: WaypointH, frames: i32, shutter: i32, orient: bool, ei: f32, eo: f32) {
        self.log.push(format!("view alongpath {w} {frames} {shutter} {} {} {}", u8::from(orient), fb(ei), fb(eo)));
    }
    fn sa_view_zoom_camera(&mut self, z: f32, ms: i32, ei: f32, eo: f32) {
        self.log.push(format!("view zoom {} {ms} {} {}", fb(z), fb(ei), fb(eo)));
    }
    fn sa_view_pitch_camera(&mut self, p: f32, ms: i32, ei: f32, eo: f32) {
        self.log.push(format!("view pitch {} {ms} {} {}", fb(p), fb(ei), fb(eo)));
    }
    fn sa_view_rotate_camera(&mut self, r: f32, frames: i32, ei: f32, eo: f32) {
        self.log.push(format!("view rotate {} {frames} {} {}", fb(r), fb(ei), fb(eo)));
    }
    fn sa_view_rotate_camera_toward_object(&mut self, id: ObjectId, ms: i32, hold: i32, ei: f32, eo: f32) {
        self.log.push(format!("view rotateobj {id} {ms} {hold} {} {}", fb(ei), fb(eo)));
    }
    fn sa_view_rotate_camera_toward_position(&mut self, p: &Coord3D, ms: i32, ei: f32, eo: f32, reverse: bool) {
        self.log.push(format!("view rotatepos {} {} {} {ms} {} {} {}", fb(p.x), fb(p.y), fb(p.z), fb(ei), fb(eo), u8::from(reverse)));
    }
    fn sa_view_reset_camera(&mut self, p: &Coord3D, frames: i32, ei: f32, eo: f32) {
        self.log.push(format!("view reset {} {} {} {frames} {} {}", fb(p.x), fb(p.y), fb(p.z), fb(ei), fb(eo)));
    }
    fn sa_view_scroll_by(&mut self, d: &Coord2D) {
        self.log.push(format!("view scroll {} {}", fb(d.x), fb(d.y)));
    }
    fn sa_view_camera_mod_look_toward(&mut self, p: &Coord3D) {
        self.log.push(format!("view modlook {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_view_camera_mod_final_look_toward(&mut self, p: &Coord3D) {
        self.log.push(format!("view modfinallook {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_view_camera_mod_final_move_to(&mut self, p: &Coord3D) {
        self.log.push(format!("view modfinalmove {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_view_camera_mod_final_pitch(&mut self, p: f32, ei: f32, eo: f32) {
        self.log.push(format!("view modfinalpitch {} {} {}", fb(p), fb(ei), fb(eo)));
    }
    fn sa_view_camera_mod_final_zoom(&mut self, z: f32, ei: f32, eo: f32) {
        self.log.push(format!("view modfinalzoom {} {} {}", fb(z), fb(ei), fb(eo)));
    }
    fn sa_drawables(&self) -> Vec<(bool, Coord3D)> {
        (1..=self.objs.len() as i32)
            .filter(|id| self.drawable(*id))
            .map(|id| ((self.cfg[14] >> ((id - 1) & 31)) & 1 != 0, self.o(id).pos))
            .collect()
    }

    fn sa_team_damage_members(&mut self, t: TeamH, amount: f32) {
        self.log.push(format!("team {t} damage {}", fb(amount)));
    }
    fn sa_team_set_state(&mut self, t: TeamH, state: &[u8]) {
        self.log.push(format!("team {t} state {}", st(state)));
    }
    fn sa_team_set_active(&mut self, t: TeamH) {
        self.log.push(format!("team {t} active"));
    }
    fn sa_team_as_ai_group(&mut self, t: TeamH, g: GroupH) {
        let ids: String = self.teams[(t - 1) as usize].members.iter().map(|m| format!(" {m}")).collect();
        self.groups[(g - 1) as usize] = self.teams[(t - 1) as usize].members.clone();
        self.log.push(format!("team {t} asgroup {g}{ids}"));
    }
    fn sa_team_members(&self, t: TeamH) -> Vec<ObjH> {
        self.teams[(t - 1) as usize].members.clone()
    }
    fn sa_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH> {
        let k = num(name, "team");
        (0..8).contains(&k).then_some(k)
    }
    fn sa_proto_template_info(&self, p: ProtoH) -> TeamTemplateInfo {
        let v = self.cfg[104 + p as usize];
        let n = ((v >> 7) & 3) as usize;
        TeamTemplateInfo {
            start_reinforce_waypoint: format!("wp{}", v & 7).into_bytes(),
            transport_unit_type: if (v >> 3) & 1 != 0 { format!("tmpl{}", (v >> 4) & 7).into_bytes() } else { Vec::new() },
            units_info: (0..n).map(|k| (format!("tmpl{}", (v >> (9 + 5 * k)) & 7).into_bytes(), ((v >> (12 + 5 * k)) & 3) as i32)).collect(),
            team_starts_full: (v >> 24) & 1 != 0,
            transports_exit: (v >> 25) & 1 != 0,
        }
    }
    fn sa_create_inactive_team(&mut self, name: &[u8]) -> Option<TeamH> {
        self.make_team(name, "createinactive")
    }
    fn sa_create_team(&mut self, name: &[u8]) -> Option<TeamH> {
        self.make_team(name, "create")
    }

    fn sa_create_group(&mut self) -> Option<GroupH> {
        self.next_group += 1;
        self.groups.push(Vec::new());
        self.log.push(format!("ai creategroup {}", self.next_group));
        Some(self.next_group)
    }
    fn sa_group_move_to_position(&mut self, g: GroupH, p: &Coord3D, add: bool, cmd: i32) {
        self.log.push(format!("group {g} move {} {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z), u8::from(add)));
    }
    fn sa_group_attack_team(&mut self, g: GroupH, t: TeamH, max: i32, cmd: i32) {
        self.log.push(format!("group {g} attackteam {t} {max} {cmd}"));
    }
    fn sa_obj_has_ai(&self, o: ObjH) -> bool {
        match self.o(o).tmpl {
            Some(t) => (self.tw(t) >> 22) & 1 != 0,
            None => (self.ow(o, 0) >> 4) & 1 != 0,
        }
    }
    fn sa_ai_clear_waypoint_queue(&mut self, o: ObjH) {
        self.log.push(format!("ai {o} clearwaypoints"));
    }
    fn sa_ai_choose_locomotor_set(&mut self, o: ObjH, set: i32) {
        self.log.push(format!("ai {o} locoset {set}"));
    }
    fn sa_ai_move_to_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("ai {o} move {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_ai_move_to_and_evacuate(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("ai {o} moveevac {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_ai_move_to_and_evacuate_and_exit(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("ai {o} moveevacexit {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_ai_force_attack_object(&mut self, o: ObjH, v: ObjH, max: i32, cmd: i32) {
        self.log.push(format!("ai {o} forceattack {v} {max} {cmd}"));
    }
    fn sa_ai_set_is_recruitable(&mut self, o: ObjH, r: bool) {
        self.log.push(format!("ai {o} recruitable {}", u8::from(r)));
    }

    fn sa_find_template(&self, name: &[u8]) -> Option<TemplateH> {
        let k = num(name, "tmpl");
        (k >= 0 && k < self.ntemplates()).then_some(k)
    }
    fn sa_new_object(&mut self, t: TemplateH, team: Option<TeamH>) -> Option<ObjH> {
        let id = self.objs.len() as i32 + 1;
        self.objs.push(Obj {
            name: Vec::new(),
            tmpl: Some(t),
            team: team.map(|t| t - 1),
            pos: Coord3D { x: 0.0, y: 0.0, z: 0.0 },
            angle: 0.0,
            killed: false,
            contained_by: None,
            contained: Vec::new(),
        });
        if let Some(tm) = team {
            self.teams[(tm - 1) as usize].members.push(id);
        }
        self.log.push(format!("factory newobject {id} {t} {}", team.unwrap_or(0)));
        Some(id)
    }
    fn sa_obj_template(&self, o: ObjH) -> Option<TemplateH> {
        self.o(o).tmpl
    }
    fn sa_template_is_equivalent_to(&self, a: TemplateH, b: Option<TemplateH>) -> bool {
        b == Some(a)
    }
    fn sa_obj_is_effectively_dead(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 1) & 1 != 0 || self.o(o).killed
    }
    fn sa_obj_is_kind_of(&self, o: ObjH, k: KindOfType) -> bool {
        match self.o(o).tmpl {
            Some(t) => (self.cfg[160 + t as usize] >> (k as u32 & 31)) & 1 != 0,
            None => o <= 16 && (self.cfg[168 + (o - 1) as usize] >> (k as u32 & 31)) & 1 != 0,
        }
    }
    fn sa_obj_is_disabled_by_type(&self, o: ObjH, t: DisabledType) -> bool {
        if t == DISABLED_HELD {
            (self.ow(o, 0) >> 7) & 1 != 0
        } else if t == DISABLED_UNMANNED {
            (self.ow(o, 0) >> 8) & 1 != 0
        } else {
            false
        }
    }
    fn sa_obj_set_name(&mut self, o: ObjH, name: &[u8]) {
        self.objs[(o - 1) as usize].name = name.to_vec();
        self.log.push(format!("obj {o} setname {}", st(name)));
    }
    fn sa_obj_set_position(&mut self, o: ObjH, p: &Coord3D) {
        self.objs[(o - 1) as usize].pos = *p;
        self.log.push(format!("obj {o} setpos {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_obj_set_orientation(&mut self, o: ObjH, a: f32) {
        self.objs[(o - 1) as usize].angle = a;
        self.log.push(format!("obj {o} setangle {}", fb(a)));
    }
    fn sa_obj_major_radius(&self, o: ObjH) -> f32 {
        self.o(o).tmpl.map(|t| (t + 1) as f32 * 2.5).unwrap_or(1.0)
    }
    fn sa_obj_transport_slot_count(&self, o: ObjH) -> i32 {
        match self.o(o).tmpl {
            Some(t) => ((self.tw(t) >> 16) & 3) as i32,
            None => ((self.ow(o, 1) >> 16) & 3) as i32,
        }
    }
    fn sa_obj_is_contained(&self, o: ObjH) -> bool {
        self.o(o).contained_by.is_some()
    }
    fn sa_obj_leave_group(&mut self, o: ObjH) {
        self.log.push(format!("obj {o} leavegroup"));
    }
    fn sa_obj_set_script_status(&mut self, o: ObjH, bit: i32, v: bool) {
        self.log.push(format!("obj {o} scriptstatus {bit} {}", u8::from(v)));
    }
    fn sa_obj_has_body(&self, o: ObjH) -> bool {
        o > 16 || (self.ow(o, 0) >> 2) & 1 != 0
    }
    fn sa_body_set_indestructible(&mut self, o: ObjH, v: bool) {
        self.log.push(format!("body {o} indestructible {}", u8::from(v)));
    }
    fn sa_obj_is_selectable(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 9) & 1 != 0
    }
    fn sa_obj_set_selectable(&mut self, o: ObjH, v: bool) {
        self.log.push(format!("obj {o} selectable {}", u8::from(v)));
    }
    fn sa_obj_has_drawable(&self, o: ObjH) -> bool {
        self.drawable(o)
    }
    fn sa_drawable_enable_ambient_sound(&mut self, o: ObjH, e: bool) {
        self.log.push(format!("draw {o} ambient {}", u8::from(e)));
    }
    fn sa_obj_has_contain(&self, o: ObjH) -> bool {
        match self.o(o).tmpl {
            Some(t) => (self.tw(t) >> 21) & 1 != 0,
            None => (self.ow(o, 0) >> 3) & 1 != 0,
        }
    }
    fn sa_contain_max(&self, o: ObjH) -> u32 {
        match self.o(o).tmpl {
            Some(t) => (self.tw(t) >> 18) & 7,
            None => (self.ow(o, 1) >> 18) & 7,
        }
    }
    fn sa_contain_is_valid_container_for(&self, c: ObjH, o: ObjH, check: bool) -> bool {
        if self.sa_obj_is_kind_of(o, KINDOF_STRUCTURE) || hh(self.seed(), c as u32, 90 + o as u32) & 3 == 0 {
            return false;
        }
        !check || (self.o(c).contained.len() as u32) < self.sa_contain_max(c)
    }
    fn sa_contain_add(&mut self, c: ObjH, o: ObjH) {
        self.log.push(format!("contain {c} add {o}"));
        self.objs[(o - 1) as usize].contained_by = Some(c);
        self.objs[(c - 1) as usize].contained.push(o);
    }
    fn sa_obj_has_deliver_payload(&self, o: ObjH) -> bool {
        self.o(o).tmpl.is_some_and(|t| (self.tw(t) >> 23) & 1 != 0)
    }
    fn sa_deliver_payload_put_in_container_template(&self, o: ObjH) -> Option<TemplateH> {
        let t = self.o(o).tmpl.map(|t| ((self.tw(t) >> 24) & 7) as i32).unwrap_or(7);
        (t < self.ntemplates()).then_some(t)
    }
    fn sa_deliver_payload(&mut self, o: ObjH, p: &Coord3D) {
        self.log.push(format!("dp {o} deliver {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }

    // --- batch A2 ---
    fn sa_attack_info(&self, name: &[u8]) -> Vec<u8> {
        if hh(self.seed(), strhash(name), 70) & 1 != 0 {
            name.to_vec()
        } else {
            Vec::new()
        }
    }
    fn sa_team_set_attack_priority_name(&mut self, t: TeamH, name: &[u8]) {
        self.log.push(format!("team {t} attackpriority {}", st(name)));
    }
    fn sa_ai_set_attack_info(&mut self, o: ObjH, info: &[u8]) {
        self.log.push(format!("ai {o} attackinfo {}", st(info)));
    }
    fn sa_player_set_team_delay_seconds(&mut self, p: PlayerH, s: i32) {
        self.log.push(format!("player {p} teamdelay {s}"));
    }
    fn sa_ai_set_attitude(&mut self, o: ObjH, a: i32) {
        self.log.push(format!("ai {o} attitude {a}"));
    }
    fn sa_group_set_attitude(&mut self, g: GroupH, a: i32) {
        self.log.push(format!("group {g} attitude {a}"));
    }
    fn sa_obj_set_status(&mut self, o: ObjH, status: i32, v: bool) {
        self.log.push(format!("obj {o} status {:x} {}", 1u64 << status, u8::from(v)));
    }
    fn sa_trigger_named(&self, name: &[u8]) -> Option<TriggerH> {
        let k = num(name, "trig");
        ((0..8).contains(&k) && (self.cfg[13] >> k) & 1 != 0).then_some(k)
    }
    fn sa_ai_attack_area(&mut self, o: ObjH, trig: TriggerH, cmd: i32) {
        self.log.push(format!("ai {o} attackarea {trig} {cmd}"));
    }
    fn sa_ai_attack_team(&mut self, o: ObjH, t: TeamH, max: i32, cmd: i32) {
        self.log.push(format!("ai {o} attackteam {t} {max} {cmd}"));
    }
    fn sa_group_attack_area(&mut self, g: GroupH, trig: TriggerH, cmd: i32) {
        self.log.push(format!("group {g} attackarea {trig} {cmd}"));
    }
    fn sa_group_attack_object(&mut self, g: GroupH, v: ObjH, max: i32, cmd: i32) {
        self.log.push(format!("group {g} attackobj {v} {max} {cmd}"));
    }
    fn sa_ai_enter(&mut self, o: ObjH, t: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} enter {t} {cmd}"));
    }
    fn sa_group_enter(&mut self, g: GroupH, t: ObjH, cmd: i32) {
        self.log.push(format!("group {g} enter {t} {cmd}"));
    }
    fn sa_ai_evacuate(&mut self, o: ObjH, expose: bool, cmd: i32) {
        self.log.push(format!("ai {o} evacuate {} {cmd}", u8::from(expose)));
    }
    fn sa_group_evacuate(&mut self, g: GroupH, cmd: i32) {
        self.log.push(format!("group {g} evacuate {cmd}"));
    }
    fn sa_contain_set_evac_disposition(&mut self, o: ObjH, d: u32) {
        self.log.push(format!("contain {o} evacdisposition {}", d as i32));
    }
    fn sa_obj_position(&self, o: ObjH) -> Coord3D {
        self.o(o).pos
    }
    fn sa_closest_waypoint_on_path(&self, p: &Coord3D, label: &[u8]) -> Option<WaypointH> {
        let mut best: Option<(i32, f32)> = None;
        for k in 0..8 {
            if !self.waypoint_exists(k) {
                continue;
            }
            let w = self.cfg[32 + k as usize];
            let on = [20, 23, 26].iter().any(|s| format!("path{}", (w >> s) & 7).as_bytes() == label);
            if !on {
                continue;
            }
            let l = self.sa_waypoint_location(k);
            let d = (l.x - p.x) * (l.x - p.x) + (l.y - p.y) * (l.y - p.y);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((k, d));
            }
        }
        best.map(|b| b.0)
    }
    fn sa_ai_follow_waypoint_path(&mut self, o: ObjH, w: WaypointH, exact: bool, cmd: i32) {
        self.log.push(format!("ai {o} followpath{} {w} {cmd}", if exact { "exact" } else { "" }));
    }
    fn sa_group_follow_waypoint_path(&mut self, g: GroupH, w: WaypointH, as_team: bool, exact: bool, cmd: i32) {
        self.log.push(format!("group {g} followpath{}{} {w} {cmd}", if as_team { "team" } else { "" }, if exact { "exact" } else { "" }));
    }
    fn sa_skirmish_enemy_player(&self) -> Option<PlayerH> {
        self.player(((self.cfg[5] >> 12) & 15) as i32)
    }
    fn sa_player_mp_start_index(&self, p: PlayerH) -> i32 {
        ((self.cfg[128 + 4 * p as usize] >> 8) & 15) as i32
    }
    fn sa_player_check_bridges(&mut self, p: PlayerH, o: ObjH, w: WaypointH) {
        self.log.push(format!("player {p} checkbridges {o} {w}"));
    }
    fn sa_ai_guard_position(&mut self, o: ObjH, p: &Coord3D, mode: i32, cmd: i32) {
        self.log.push(format!("ai {o} guardpos {} {} {} {mode} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_group_guard_position(&mut self, g: GroupH, p: &Coord3D, mode: i32, cmd: i32) {
        self.log.push(format!("group {g} guardpos {} {} {} {mode} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_group_guard_object(&mut self, g: GroupH, o: ObjH, mode: i32, cmd: i32) {
        self.log.push(format!("group {g} guardobj {o} {mode} {cmd}"));
    }
    fn sa_group_guard_area(&mut self, g: GroupH, trig: TriggerH, mode: i32, cmd: i32) {
        self.log.push(format!("group {g} guardarea {trig} {mode} {cmd}"));
    }
    fn sa_ai_hunt(&mut self, o: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} hunt {cmd}"));
    }
    fn sa_group_hunt(&mut self, g: GroupH, cmd: i32) {
        self.log.push(format!("group {g} hunt {cmd}"));
    }
    fn sa_find_command_button(&self, name: &[u8]) -> Option<ButtonH> {
        let k = num(name, "btn");
        (k >= 0 && k < ((self.cfg[11] >> 8) & 15) as i32).then_some(k)
    }
    fn sa_button_command_type(&self, b: ButtonH) -> i32 {
        (hh(self.seed(), b as u32, 80) % 30) as i32
    }
    fn sa_button_has_special_power(&self, b: ButtonH) -> bool {
        hh(self.seed(), b as u32, 82) & 1 != 0 && (self.cfg[11] >> 4) & 15 > 0
    }
    fn sa_button_options(&self, b: ButtonH) -> u32 {
        hh(self.seed(), b as u32, 81) & (7 | 32)
    }
    fn sa_obj_command_set_string(&self, o: ObjH) -> Vec<u8> {
        format!("cs{}", self.o(o).tmpl.unwrap_or(9)).into_bytes()
    }
    fn sa_find_command_set(&self, name: &[u8]) -> Option<CommandSetH> {
        let k = num(name, "cs");
        (0..=8).contains(&k).then_some(k)
    }
    fn sa_command_set_button(&self, s: CommandSetH, i: i32) -> Option<ButtonH> {
        let n = (self.cfg[11] >> 8) & 15;
        let h = hh(self.seed(), s as u32, 84 + i as u32);
        (n > 0 && h & 1 != 0).then(|| ((h >> 4) % n) as i32)
    }
    fn sa_template_name(&self, t: TemplateH) -> Vec<u8> {
        format!("tmpl{t}").into_bytes()
    }
    fn sa_obj_has_command_button_hunt(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 51) & 1 != 0
    }
    fn sa_command_button_hunt_set(&mut self, o: ObjH, ability: &[u8]) {
        self.log.push(format!("cbhunt {o} {}", st(ability)));
    }
    fn sa_player_set_units_should_hunt(&mut self, p: PlayerH, hunt: bool, cmd: i32) {
        self.log.push(format!("player {p} hunt {} {cmd}", u8::from(hunt)));
    }
    fn sa_player_sell_everything(&mut self, p: PlayerH) {
        self.log.push(format!("player {p} selleverything"));
    }
    fn sa_player_set_can_build_base(&mut self, p: PlayerH, can: bool) {
        self.log.push(format!("player {p} canbuildbase {}", u8::from(can)));
    }
    fn sa_player_set_can_build_units(&mut self, p: PlayerH, can: bool) {
        self.log.push(format!("player {p} canbuildunits {}", u8::from(can)));
    }
    fn sa_player_set_objects_enabled(&mut self, p: PlayerH, name: &[u8], enable: bool) {
        self.log.push(format!("player {p} objectsenabled {} {}", st(name), u8::from(enable)));
    }
    fn sa_player_repair_structure(&mut self, p: PlayerH, id: ObjectId) {
        self.log.push(format!("player {p} repair {id}"));
    }

    // --- batch A3 ---
    fn sa_proto_controlling_player(&self, p: ProtoH) -> Option<PlayerH> {
        self.player((self.cfg[96 + p as usize] & 15) as i32)
    }
    fn sa_player_build_specific_team(&mut self, p: PlayerH, proto: ProtoH) {
        self.log.push(format!("player {p} buildteam {proto}"));
    }
    fn sa_player_recruit_specific_team(&mut self, p: PlayerH, proto: ProtoH, radius: f32) {
        self.log.push(format!("player {p} recruit {proto} {}", fb(radius)));
    }
    fn sa_obj_attempt_damage(&mut self, o: ObjH, source: ObjectId, dt: i32, death: i32, amount: f32) {
        self.log.push(format!("obj {o} damage {source} {dt} {death} {} 0", fb(amount)));
    }
    fn sa_logic_destroy_object(&mut self, o: ObjH) {
        self.log.push(format!("logic destroy {o}"));
    }
    fn sa_team_delete(&mut self, t: TeamH, ignore_dead: bool) {
        self.log.push(format!("team {t} delete {}", u8::from(ignore_dead)));
    }
    fn sa_ai_wander(&mut self, o: ObjH, w: WaypointH, cmd: i32) {
        self.log.push(format!("ai {o} wander {w} {cmd}"));
    }
    fn sa_ai_wander_in_place(&mut self, o: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} wanderinplace {cmd}"));
    }
    fn sa_ai_panic(&mut self, o: ObjH, w: WaypointH, cmd: i32) {
        self.log.push(format!("ai {o} panic {w} {cmd}"));
    }
    fn sa_team_prototype(&self, t: TeamH) -> Option<ProtoH> {
        self.teams[(t - 1) as usize].proto
    }
    fn sa_proto_change_priority(&mut self, p: ProtoH, up: bool) {
        self.log.push(format!("proto {p} priority{}", if up { "success" } else { "failure" }));
    }
    fn sa_proto_production_priority(&self, p: ProtoH) -> i32 {
        ((self.cfg[104 + p as usize] >> 26) & 15) as i32
    }
    fn sa_obj_kill(&mut self, o: ObjH) {
        self.objs[(o - 1) as usize].killed = true;
        self.log.push(format!("obj {o} kill 11 0"));
    }
    fn sa_team_kill(&mut self, t: TeamH) {
        self.log.push(format!("team {t} kill"));
    }
    fn sa_player_kill(&mut self, p: PlayerH) {
        self.log.push(format!("player {p} kill"));
    }
    fn sa_ui_message(&mut self, text: &[u8]) {
        self.log.push(format!("ui message {}", st(text)));
    }
    fn sa_ui_popup_message(&mut self, m: &[u8], x: i32, y: i32, w: i32, pause: bool, pm: bool) {
        self.log.push(format!("ui popup {} {x} {y} {w} {} {}", st(m), u8::from(pause), u8::from(pm)));
    }
    fn sa_game_text_fetch(&self, label: &[u8]) -> (Vec<u16>, bool) {
        let h = hh(self.seed(), strhash(label), 140);
        let mut s: Vec<u8> = if h & 2 != 0 { b"*".to_vec() } else { b"T:".to_vec() };
        s.extend_from_slice(label);
        (s.iter().map(|c| u16::from(*c)).collect(), h & 1 != 0)
    }
    fn sa_display_set_cinematic_text(&mut self, text: &[u8]) {
        self.log.push(format!("display cinematic {}", st(text)));
    }
    fn sa_display_set_cinematic_font(&mut self, name: &[u8], size: i32, bold: bool) {
        self.log.push(format!("display font {} {size} {}", st(name), u8::from(bold)));
    }
    fn sa_adjust_font_size(&self, size: i32) -> i32 {
        size + 1
    }
    fn sa_display_set_cinematic_text_frames(&mut self, frames: i32) {
        self.log.push(format!("display cinematicframes {frames}"));
    }
    fn sa_button_set_flash_count(&mut self, b: ButtonH, n: i32) {
        self.log.push(format!("button {b} flashcount {n}"));
    }
    fn sa_control_bar_set_flash(&mut self, on: bool) {
        self.log.push(format!("controlbar flash {}", u8::from(on)));
    }
    fn sa_obj_set_custom_indicator_color(&mut self, o: ObjH, c: i32) {
        self.log.push(format!("obj {o} customcolor {c}"));
    }
    fn sa_obj_indicator_color(&self, o: ObjH) -> i32 {
        0x100 + o
    }
    fn sa_obj_night_indicator_color(&self, o: ObjH) -> i32 {
        0x200 + o
    }
    fn sa_drawable_set_flash_color(&mut self, o: ObjH, c: i32) {
        self.log.push(format!("draw {o} flashcolor {c}"));
    }
    fn sa_drawable_set_flash_count(&mut self, o: ObjH, n: i32) {
        self.log.push(format!("draw {o} flashcount {n}"));
    }
    fn sa_drawable_set_indicator_color(&mut self, o: ObjH, c: i32) {
        self.log.push(format!("draw {o} indicator {c}"));
    }
    fn sa_team_has_any_objects(&self, t: TeamH) -> bool {
        if t <= 16 {
            self.cfg[64 + (t - 1) as usize] & 1 != 0
        } else {
            false
        }
    }
    fn sa_display_play_movie(&mut self, name: &[u8]) {
        self.log.push(format!("display movie {}", st(name)));
    }
    fn sa_ui_play_movie(&mut self, name: &[u8]) {
        self.log.push(format!("ui movie {}", st(name)));
    }
    fn sa_ui_military_subtitle(&mut self, label: &[u8], d: i32) {
        self.log.push(format!("ui subtitle {} {d}", st(label)));
    }
    fn sa_player_transfer_assets_from(&mut self, d: PlayerH, s: PlayerH) {
        self.log.push(format!("player {d} transferfrom {s}"));
    }
    fn sa_player_default_team(&self, p: PlayerH) -> Option<TeamH> {
        let t = self.cfg[128 + 4 * p as usize + 1] & 255;
        ((t as usize) < self.teams.len()).then_some(t as i32 + 1)
    }
    fn sa_obj_set_team(&mut self, o: ObjH, t: Option<TeamH>) {
        if let Some(old) = self.objs[(o - 1) as usize].team {
            let old = old as usize;
            if let Some(i) = self.teams[old].members.iter().position(|m| *m == o) {
                self.teams[old].members.remove(i);
            }
        }
        self.objs[(o - 1) as usize].team = t.map(|t| t - 1);
        if let Some(t) = t {
            self.teams[(t - 1) as usize].members.push(o);
        }
        self.log.push(format!("obj {o} setteam {}", t.unwrap_or(0)));
    }
    fn sa_obj_update_upgrade_modules(&mut self, o: ObjH) {
        self.log.push(format!("obj {o} updateupgrades"));
    }
    fn sa_player_set_list_in_score_screen(&mut self, p: PlayerH, b: bool) {
        self.log.push(format!("player {p} scorescreen {}", u8::from(b)));
    }
    fn sa_logic_enable_scoring(&mut self, b: bool) {
        self.log.push(format!("logic scoring {}", u8::from(b)));
    }
    fn sa_player_set_player_relationship(&mut self, p: PlayerH, other: PlayerH, r: i32) {
        self.log.push(format!("player {p} playerrel {other} {r}"));
    }
    fn sa_radar_create_event(&mut self, p: &Coord3D, t: i32) {
        self.log.push(format!("radar event {} {} {} {t}", fb(p.x), fb(p.y), fb(p.z)));
    }

    // --- batch A4 ---
    fn sa_team_has_any_units(&self, t: TeamH) -> bool {
        if t <= 16 {
            (self.cfg[64 + (t - 1) as usize] >> 1) & 1 != 0
        } else {
            false
        }
    }
    fn sa_team_estimate_position(&self, t: TeamH) -> Option<Coord3D> {
        self.teams[(t - 1) as usize].members.first().map(|m| self.o(*m).pos)
    }
    fn sa_radar_hide(&mut self, hide: bool) {
        self.log.push(format!("radar hide {}", u8::from(hide)));
    }
    fn sa_view_set_view_filter(&mut self, f: i32) -> bool {
        self.log.push(format!("view filter {f}"));
        hh(self.seed(), 150, f as u32) & 3 != 0
    }
    fn sa_view_set_view_filter_mode(&mut self, m: i32) -> bool {
        self.log.push(format!("view filtermode {m}"));
        hh(self.seed(), 151, m as u32) & 3 != 0
    }
    fn sa_view_set_view_filter_pos(&mut self, p: &Coord3D) {
        self.log.push(format!("view filterpos {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_view_look_at(&mut self, p: &Coord3D) {
        self.log.push(format!("view lookat {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_player_count(&self) -> i32 {
        self.nplayers()
    }
    fn sa_nth_player(&self, i: i32) -> Option<PlayerH> {
        self.player(i)
    }
    fn sa_player_is_human(&self, p: PlayerH) -> bool {
        self.cfg[128 + 4 * p as usize] & 1 == 0
    }
    fn sa_player_mask(&self, p: PlayerH) -> u32 {
        1 << p
    }
    fn sa_partition_shroud(&mut self, cover: bool, undo: bool, x: f32, y: f32, r: f32, mask: u32) {
        let what = match (cover, undo) {
            (false, false) => "shroudreveal",
            (false, true) => "undoshroudreveal",
            (true, false) => "shroudcover",
            (true, true) => "undoshroudcover",
        };
        self.log.push(format!("partition {what} {} {} {} {mask:x}", fb(x), fb(y), fb(r)));
    }
    fn sa_partition_reveal_map(&mut self, i: i32) {
        self.log.push(format!("partition revealmap {i}"));
    }
    fn sa_partition_reveal_map_permanently(&mut self, i: i32, undo: bool) {
        self.log.push(format!("partition {}revealmapperm {i}", if undo { "undo" } else { "" }));
    }
    fn sa_partition_shroud_map(&mut self, i: i32) {
        self.log.push(format!("partition shroudmap {i}"));
    }
    fn sa_team_set_recruitable(&mut self, t: TeamH, r: bool) {
        self.log.push(format!("team {t} recruitable {}", u8::from(r)));
    }
    fn sa_find_team(&self, name: &[u8]) -> Option<TeamH> {
        let k = num(name, "team");
        if !(0..8).contains(&k) {
            return None;
        }
        self.protos[k as usize].first().map(|t| t + 1)
    }
    fn sa_global_alpha(&self, shroud: bool) -> u8 {
        if shroud {
            self.alphas.1
        } else {
            self.alphas.0
        }
    }
    fn sa_display_set_border_shroud_level(&mut self, level: u8) {
        self.log.push(format!("display bordershroud {level}"));
    }
    fn sa_player_set_units_should_idle_or_resume(&mut self, p: PlayerH, idle: bool) {
        self.log.push(format!("player {p} idleorresume {}", u8::from(idle)));
    }
    fn sa_audio_pause_ambient(&mut self, pause: bool) {
        self.log.push(format!("audio pauseambient {}", u8::from(pause)));
    }
    fn sa_audio_remove_event_handle(&mut self, h: i32) {
        self.log.push(format!("audio removehandle {h}"));
    }
    fn sa_set_current_track_name(&mut self, name: &[u8]) {
        self.log.push(format!("se track {}", st(name)));
    }
    fn sa_contain_player_who_entered(&self, o: ObjH) -> u32 {
        let p = hh(self.seed(), o as u32, 92) % 9;
        if p == 8 {
            0
        } else {
            1 << p
        }
    }
    fn sa_contain_count(&self, o: ObjH) -> u32 {
        self.o(o).contained.len() as u32
    }
    fn sa_team_controlling_player(&self, t: TeamH) -> Option<PlayerH> {
        let tm = &self.teams[(t - 1) as usize];
        if tm.controller.is_some() {
            return tm.controller;
        }
        match tm.proto {
            Some(k) => self.player((self.cfg[96 + k as usize] & 15) as i32),
            None => self.player(((if t <= 16 { self.cfg[64 + (t - 1) as usize] } else { 4 } >> 8) & 15) as i32),
        }
    }
    fn sa_contain_remove_all_contained(&mut self, o: ObjH, expose: bool) {
        self.log.push(format!("contain {o} removeall {}", u8::from(expose)));
        let c = std::mem::take(&mut self.objs[(o - 1) as usize].contained);
        for m in c {
            self.objs[(m - 1) as usize].contained_by = None;
        }
    }
    fn sa_partition_garrison_candidates(&mut self, leader: ObjH, money_hacker: bool) -> Vec<ObjH> {
        let range: f32 = 100_000.0 * 10.0;
        self.log.push(format!("partition iterateobj {leader} {} 1 1", fb(range)));
        // filters: [internet centres (kind 99) or garrisonable, same map]
        let salt = 2000 + if money_hacker { (1 + 8 * 99) * 7 } else { 0 };
        (0..self.objs.len()).filter(|i| hh(self.seed() ^ self.cfg[16], *i as u32, salt) & 1 != 0).map(|i| i as i32 + 1).collect()
    }
    fn sa_ai_exit(&mut self, o: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} exit 0 {cmd}"));
    }

    // --- batch A5 ---
    fn sa_partition_unit_garrison_candidates(&mut self, unit: ObjH, money_hacker: bool) -> Vec<ObjH> {
        let range: f32 = 100_000.0 * 10.0;
        self.log.push(format!("partition iterateobj {unit} {} 1 1", fb(range)));
        // filters: [structures (kind 7), same map, internet centres (kind 99) or not]
        let s = (1 + 8 * 7) * 49;
        let salt = 2000 + if money_hacker { s + 1 + 8 * 99 } else { s };
        (0..self.objs.len()).filter(|i| hh(self.seed() ^ self.cfg[16], *i as u32, salt) & 1 != 0).map(|i| i as i32 + 1).collect()
    }
    fn sa_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH> {
        if o <= 16 {
            self.player(((self.cfg[40 + 4 * (o - 1) as usize] >> 16) & 15) as i32)
        } else {
            self.o(o).team.and_then(|t| self.sa_team_controlling_player(t + 1))
        }
    }
    fn sa_obj_team(&self, o: ObjH) -> Option<TeamH> {
        self.o(o).team.map(|t| t + 1)
    }
    fn sa_obj_set_disabled(&mut self, o: ObjH, t: DisabledType) {
        self.log.push(format!("obj {o} disable {t}"));
    }
    fn sa_obj_set_disabled_until(&mut self, o: ObjH, t: DisabledType, f: u32) {
        self.log.push(format!("obj {o} disableuntil {t} {f}"));
    }
    fn sa_logic_deselect_object(&mut self, o: ObjH, mask: u32, affect: bool) {
        self.log.push(format!("logic deselect {o} {mask:x} {}", u8::from(affect)));
    }
    fn sa_neutral_player(&self) -> Option<PlayerH> {
        self.player(((self.cfg[4] >> 8) & 15) as i32)
    }
    fn sa_obj_has_sticky_bomb_update(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 52) & 1 != 0
    }
    fn sa_obj_random_offset_on_perimeter(&mut self, o: ObjH) -> Coord3D {
        Coord3D { x: self.sa_obj_major_radius(o), y: 0.0, z: 0.0 }
    }
    fn sa_obj_transform_vector(&self, o: ObjH, v: &Coord3D) -> Coord3D {
        let ob = self.o(o);
        let (c, s) = (f64::from(ob.angle).cos() as f32, f64::from(ob.angle).sin() as f32);
        Coord3D { x: c * v.x - s * v.y + ob.pos.x, y: s * v.x + c * v.y + ob.pos.y, z: v.z + ob.pos.z }
    }
    fn sa_sticky_bomb_init(&mut self, bomb: ObjH, target: ObjH, p: &Coord3D) {
        self.log.push(format!("sticky {bomb} init {target} 0 {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_player_garrison_all(&mut self, p: PlayerH, garrison: bool, cmd: i32) {
        self.log.push(format!("player {p} {}garrisonall {cmd}", if garrison { "" } else { "un" }));
    }
    fn sa_control_bar_show(&mut self, hide: bool, immediate: bool) {
        self.log.push(format!("controlbar {} {}", if hide { "hide" } else { "show" }, u8::from(immediate)));
    }
    fn sa_display_enable_letter_box(&mut self, e: bool) {
        self.log.push(format!("display letterbox {}", u8::from(e)));
    }
    fn sa_view_set_fade_parameters(&mut self, frames: i32, dir: i32) {
        self.log.push(format!("view fade {frames} {dir}"));
    }
    fn sa_view_filter_type(&self) -> i32 {
        (self.cfg[15] & 3) as i32
    }
    fn sa_sky_box_set(&mut self, draw: bool) {
        self.log.push(format!("skybox {}", u8::from(draw)));
    }
    fn sa_snow_set_visible(&mut self, v: bool) {
        self.log.push(format!("snow {}", u8::from(v)));
    }
    fn sa_freeze_time(&mut self, freeze: bool) {
        self.log.push(if freeze { "se freeze".into() } else { "se unfreeze".into() });
    }
    fn sa_ai_has_cur_locomotor(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 40) & 1 != 0
    }
    fn sa_locomotor_set_close_enough_dist(&mut self, o: ObjH, d: f32) {
        self.log.push(format!("loco {o} closeenough {}", fb(d)));
    }
    fn sa_ui_set_superweapon_display_enabled(&mut self, e: bool) {
        self.log.push(format!("ui superweapondisplay {}", u8::from(e)));
    }
    fn sa_ui_object_superweapon_display(&mut self, o: ObjH, hide: bool) {
        self.log.push(format!("ui {}superweapon {o}", if hide { "hide" } else { "show" }));
    }
    fn sa_audio_set_volume(&mut self, v: f32, affect: i32) {
        self.log.push(format!("audio volume {} {affect}", fb(v)));
    }
    fn sa_team_set_controlling_player(&mut self, t: TeamH, p: PlayerH) {
        self.teams[(t - 1) as usize].controller = Some(p);
        self.log.push(format!("team {t} controller {p}"));
    }
    fn sa_money_count(&self, p: PlayerH) -> u32 {
        self.money[p as usize]
    }
    fn sa_money_withdraw(&mut self, p: PlayerH, a: u32) {
        let a = a.min(self.money[p as usize]);
        self.money[p as usize] -= a;
        self.log.push(format!("money {p} withdraw {a}"));
    }
    fn sa_money_deposit(&mut self, p: PlayerH, a: u32) {
        self.money[p as usize] = self.money[p as usize].wrapping_add(a);
        self.log.push(format!("money {p} deposit {a}"));
    }
    fn sa_ui_add_named_timer(&mut self, name: &[u8], text: &[u16], countdown: bool) {
        let t: String = text.iter().map(|c| *c as u8 as char).collect();
        self.log.push(format!("ui addtimer {} {t} {}", st(name), u8::from(countdown)));
    }
    fn sa_ui_remove_named_timer(&mut self, name: &[u8]) {
        self.log.push(format!("ui removetimer {}", st(name)));
    }
    fn sa_ui_show_named_timer_display(&mut self, show: bool) {
        self.log.push(format!("ui showtimers {}", u8::from(show)));
    }
    fn sa_find_special_power(&self, name: &[u8]) -> Option<SpecialPowerH> {
        let k = num(name, "power");
        (k >= 0 && k < ((self.cfg[11] >> 4) & 15) as i32).then_some(k)
    }
    fn sa_obj_has_special_power_module(&self, o: ObjH, p: SpecialPowerH) -> bool {
        hh(self.seed(), o as u32, 410 + p as u32) & 1 != 0
    }
    fn sa_spm_pause_countdown(&mut self, o: ObjH, p: SpecialPowerH, pause: bool) {
        self.log.push(format!("spm {o} {p} pause {}", u8::from(pause)));
    }
    fn sa_spm_ready_frame(&self, o: ObjH, p: SpecialPowerH) -> u32 {
        (hh(self.seed(), o as u32, 400 + p as u32) >> 8) % 2000
    }
    fn sa_spm_set_ready_frame(&mut self, o: ObjH, p: SpecialPowerH, f: u32) {
        self.log.push(format!("spm {o} {p} readyframe {f}"));
    }
    fn sa_spm_do_at_location(&mut self, o: ObjH, p: SpecialPowerH, l: &Coord3D, angle: f32, opts: u32) {
        self.log.push(format!("spm {o} {p} atloc {} {} {} {} {opts}", fb(l.x), fb(l.y), fb(l.z), fb(angle)));
    }
    fn sa_frame(&self) -> u32 {
        self.cfg[1]
    }

    // --- batch A6 ---
    fn sa_player_teams(&self, p: PlayerH) -> Vec<Vec<TeamH>> {
        (0..8).filter(|&k| self.proto_owner[k] == p).map(|k| self.protos[k].iter().map(|t| t + 1).collect()).collect()
    }
    fn sa_special_power_radius_cursor(&self, sp: SpecialPowerH) -> f32 {
        (sp * 10 + 5) as f32
    }
    fn sa_special_power_type(&self, sp: SpecialPowerH) -> i32 {
        sp + 1
    }
    fn sa_spm_is_ready(&self, o: ObjH, sp: SpecialPowerH) -> bool {
        hh(self.seed(), o as u32, 400 + sp as u32) & 2 != 0
    }
    fn sa_spm_reference_template(&self, o: ObjH, sp: SpecialPowerH) -> Option<TemplateH> {
        let t = ((hh(self.seed(), o as u32, 400 + sp as u32) >> 4) & 7) as i32;
        (t < self.ntemplates()).then_some(t)
    }
    fn sa_spm_do_at_object(&mut self, o: ObjH, sp: SpecialPowerH, target: ObjH, opts: u32) {
        self.log.push(format!("spm {o} {sp} atobj {target} {opts}"));
    }
    fn sa_player_compute_superweapon_target(&mut self, p: PlayerH, sp: SpecialPowerH, index: i32, weight: f32) -> Option<Coord3D> {
        self.log.push(format!("player {p} superweapontarget {sp} {index} {}", fb(weight)));
        (hh(self.seed(), p as u32, 110 + sp as u32) & 1 != 0).then_some(Coord3D { x: 50.0 + p as f32, y: 60.0 + sp as f32, z: 0.0 })
    }
    fn sa_player_closest_construction_zone(&mut self, p: PlayerH, t: TemplateH) -> Option<Coord3D> {
        (hh(self.seed(), p as u32, 100 + t as u32) & 1 != 0).then_some(Coord3D { x: p as f32 * 7.0, y: t as f32 * 3.0, z: 0.0 })
    }
    fn sa_button_name(&self, b: ButtonH) -> Vec<u8> {
        format!("btn{b}").into_bytes()
    }
    fn sa_obj_do_command_button(&mut self, o: ObjH, b: ButtonH, cmd: i32) {
        self.log.push(format!("obj {o} button {b} {cmd}"));
    }
    fn sa_obj_do_command_button_at_object(&mut self, o: ObjH, b: ButtonH, t: ObjH, cmd: i32) {
        self.log.push(format!("obj {o} buttonobj {b} {t} {cmd}"));
    }
    fn sa_obj_do_command_button_at_position(&mut self, o: ObjH, b: ButtonH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("obj {o} buttonpos {b} {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_obj_do_command_button_using_waypoints(&mut self, o: ObjH, b: ButtonH, w: WaypointH, cmd: i32) {
        self.log.push(format!("obj {o} buttonpath {b} {w} {cmd}"));
    }
    fn sa_group_do_command_button(&mut self, g: GroupH, b: ButtonH, cmd: i32) {
        self.log.push(format!("group {g} button {b} {cmd}"));
    }
    fn sa_group_do_command_button_at_object(&mut self, g: GroupH, b: ButtonH, t: ObjH, cmd: i32) {
        self.log.push(format!("group {g} buttonobj {b} {t} {cmd}"));
    }
    fn sa_group_do_command_button_at_position(&mut self, g: GroupH, b: ButtonH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("group {g} buttonpos {b} {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_radar_refresh_terrain(&mut self) {
        self.log.push("radar refresh".into());
    }
    fn sa_view_set_default_view(&mut self, p: f32, a: f32, m: f32) {
        self.log.push(format!("view default {} {} {}", fb(p), fb(a), fb(m)));
    }
    fn sa_ai_idle(&mut self, o: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} idle {cmd}"));
    }
    fn sa_group_idle(&mut self, g: GroupH, cmd: i32) {
        self.log.push(format!("group {g} idle {cmd}"));
    }
    fn sa_team_name(&self, t: TeamH) -> Vec<u8> {
        match self.teams[(t - 1) as usize].proto {
            Some(k) => format!("team{k}").into_bytes(),
            None => format!("lone{}", t - 1).into_bytes(),
        }
    }
    fn sa_team_set_override_team_relationship(&mut self, t: TeamH, o: TeamH, r: i32) {
        self.log.push(format!("team {t} overrideteam {o} {r}"));
    }
    fn sa_team_remove_override_team_relationship(&mut self, t: TeamH, o: TeamH) {
        self.log.push(format!("team {t} removeoverrideteam {o}"));
    }
    fn sa_team_set_override_player_relationship(&mut self, t: TeamH, p: PlayerH, r: i32) {
        self.log.push(format!("team {t} overrideplayer {p} {r}"));
    }
    fn sa_team_remove_override_player_relationship(&mut self, t: TeamH, p: PlayerH) {
        self.log.push(format!("team {t} removeoverrideplayer {p}"));
    }
    fn sa_player_with_name_key(&self, name: &[u8]) -> Option<PlayerH> {
        let key = |s: &[u8]| strhash(s) & 0x7fff_ffff;
        if (self.cfg[0] >> 11) & 1 != 0 && key(name) == key(b"ReplayObserver") && self.nplayers() > 0 {
            return Some(self.nplayers() - 1);
        }
        (0..self.nplayers()).find(|i| key(format!("player{i}").as_bytes()) == key(name))
    }
    fn sa_player_set_team_relationship(&mut self, p: PlayerH, t: TeamH, r: i32) {
        self.log.push(format!("player {p} teamrel {t} {r}"));
    }
    fn sa_player_remove_team_relationship(&mut self, p: PlayerH, t: TeamH) {
        self.log.push(format!("player {p} removeteamrel {t}"));
    }
    fn sa_find_script(&self, name: &[u8]) -> bool {
        num(name, "script") >= 0
    }
    fn sa_append_sequential_script(&mut self, team: Option<TeamH>, object: ObjectId, script: &[u8], times: i32) {
        self.log.push(format!("se appendseq {} {object} {} {times}", team.unwrap_or(0), st(script)));
    }
    fn sa_remove_all_sequential_scripts_obj(&mut self, o: ObjH) {
        self.log.push(format!("se removeseqobj {o}"));
    }
    fn sa_remove_all_sequential_scripts_team(&mut self, t: TeamH) {
        self.log.push(format!("se removeseqteam {t}"));
    }
    fn sa_set_sequential_timer_obj(&mut self, o: ObjH, f: i32) {
        self.log.push(format!("se seqtimerobj {o} {f}"));
    }
    fn sa_set_sequential_timer_team(&mut self, t: TeamH, f: i32) {
        self.log.push(format!("se seqtimerteam {t} {f}"));
    }
    fn sa_obj_has_waypoint_following_weapon(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 60) & 1 != 0
    }
    fn sa_weapon_force_fire(&mut self, o: ObjH, p: &Coord3D) -> Option<ObjH> {
        self.log.push(format!("weapon {o} forcefire {} {} {}", fb(p.x), fb(p.y), fb(p.z)));
        (hh(self.seed(), o as u32, 61) & 1 != 0).then_some(o)
    }

    // --- batch A7 ---
    fn sa_water_handle(&self, name: &[u8]) -> Option<i32> {
        let k = num(name, "water");
        (0..4).contains(&k).then_some(k)
    }
    fn sa_set_water_height(&mut self, w: i32, ht: f32, dmg: f32, force: bool) {
        self.log.push(format!("terrain waterheight {w} {} {} {}", fb(ht), fb(dmg), u8::from(force)));
    }
    fn sa_change_water_height_over_time(&mut self, w: i32, ht: f32, t: f32, dmg: f32) {
        self.log.push(format!("terrain waterover {w} {} {} {}", fb(ht), fb(t), fb(dmg)));
    }
    fn sa_set_active_boundary(&mut self, b: i32) {
        self.log.push(format!("terrain boundary {b}"));
    }
    fn sa_refresh_shroud_for_local_player(&mut self) {
        self.log.push("partition refreshshroud".into());
    }
    fn sa_ui_select_drawable(&mut self, o: ObjH) {
        self.log.push(format!("ui select {o}"));
    }
    fn sa_view_position(&self) -> Coord3D {
        Coord3D { x: 100.0, y: 200.0, z: 0.0 }
    }
    fn sa_view_shake(&mut self, p: &Coord3D, t: i32) {
        self.log.push(format!("view shake {} {} {} {t}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_contained(&self, o: ObjH) -> Vec<ObjH> {
        self.o(o).contained.clone()
    }
    fn sa_logic_set_buildable_status_override(&mut self, t: TemplateH, s: i32) {
        self.log.push(format!("logic buildable {t} {s}"));
    }
    fn sa_obj_has_cave(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 50) & 1 != 0
    }
    fn sa_cave_try_set_index(&mut self, o: ObjH, i: i32) {
        self.log.push(format!("cave {o} index {i}"));
    }
    fn sa_obj_has_warehouse(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 53) & 1 != 0
    }
    fn sa_warehouse_set_cash_value(&mut self, o: ObjH, v: i32) {
        self.log.push(format!("warehouse {o} cash {v}"));
    }
    fn sa_audio_set_event_enabled(&mut self, n: &[u8], e: bool) {
        self.log.push(format!("audio enabled {} {}", st(n), u8::from(e)));
    }
    fn sa_audio_remove_disabled_events(&mut self) {
        self.log.push("audio removedisabled".into());
    }
    fn sa_audio_remove_event_named(&mut self, n: &[u8]) {
        self.log.push(format!("audio remove {}", st(n)));
    }
    fn sa_audio_set_event_volume_override(&mut self, n: &[u8], v: f32) {
        self.log.push(format!("audio volumeoverride {} {}", st(n), fb(v)));
    }
    fn sa_set_topple_direction(&mut self, n: &[u8], d: &Coord3D) {
        self.log.push(format!("se topple {} {} {} {}", st(n), fb(d.x), fb(d.y), fb(d.z)));
    }
    fn sa_radar_force_on(&mut self, on: bool) {
        self.log.push(format!("radar forceon {}", u8::from(on)));
    }

    // --- batch A8 ---
    fn sa_partition_closest_of_template(&self, p: &Coord3D, t: TemplateH, _trig: TriggerH, _obj: ObjH) -> Option<(ObjH, f32)> {
        let salt = 3000 + (3 + 8 * t as u32) * 49;
        let o = (0..self.objs.len()).find(|i| hh(self.seed() ^ self.cfg[16], *i as u32, salt) & 1 != 0)? as i32 + 1;
        Some((o, (hh(self.seed(), o as u32, p.x.to_bits()) % 1000) as f32))
    }
    fn sa_object_types(&self, name: &[u8]) -> Option<Vec<Vec<u8>>> {
        let k = num(name, "list");
        if !(0..4).contains(&k) {
            return None;
        }
        Some((0..9u32).filter(|j| hh(self.seed(), k as u32, 120 + j) & 1 != 0).map(|j| format!("tmpl{j}").into_bytes()).collect())
    }
    fn sa_ai_move_to_object(&mut self, o: ObjH, t: Option<ObjH>, cmd: i32) {
        self.log.push(format!("ai {o} moveobj {} {cmd}", t.unwrap_or(0)));
    }
    fn sa_find_upgrade(&self, name: &[u8]) -> Option<i32> {
        let k = num(name, "upg");
        (0..4).contains(&k).then_some(k)
    }
    fn sa_obj_give_upgrade(&mut self, o: ObjH, u: i32) {
        self.log.push(format!("obj {o} giveupgrade {u}"));
    }
    fn sa_group_center(&self, g: GroupH) -> Option<Coord3D> {
        let m = &self.groups[(g - 1) as usize];
        if m.is_empty() {
            return None;
        }
        let mut c = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        for o in m {
            let p = self.o(*o).pos;
            c.x += p.x;
            c.y += p.y;
            c.z += p.z;
        }
        let n = m.len() as f32;
        Some(Coord3D { x: c.x / n, y: c.y / n, z: c.z / n })
    }
    fn sa_partition_nearest_group_with_value(&mut self, index: i32, src: &Coord3D, value: i32, greater: bool) -> Option<Coord3D> {
        self.log.push(format!("partition nearestgroup {index} 1 1 {} {} {value} {}", fb(src.x), fb(src.y), u8::from(greater)));
        (hh(self.seed(), index as u32, 130u32.wrapping_add(value as u32)) & 1 != 0).then_some(Coord3D { x: src.x + 10.0, y: src.y - 10.0, z: 0.0 })
    }
    fn sa_group_attack_move_to_position(&mut self, g: GroupH, p: &Coord3D, max: i32, cmd: i32) {
        self.log.push(format!("group {g} attackmove {} {} {} {max} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_button_special_power_id(&self, b: ButtonH) -> Option<u32> {
        let n = (self.cfg[11] >> 4) & 15;
        (hh(self.seed(), b as u32, 82) & 1 != 0 && n > 0).then(|| (b as u32 % n) + 1)
    }
    fn sa_group_special_power_source(&self, g: GroupH, id: u32) -> Option<ObjH> {
        self.groups[(g - 1) as usize].iter().copied().find(|o| hh(self.seed(), *o as u32, 600 + id) & 1 != 0)
    }
    fn sa_group_command_button_source(&self, g: GroupH, t: i32) -> Option<ObjH> {
        self.groups[(g - 1) as usize].iter().copied().find(|o| hh(self.seed(), *o as u32, 700 + t as u32) & 1 != 0)
    }
    fn sa_partition_most_valuable_target(&mut self, p: &Coord3D, range: f32) -> Option<ObjH> {
        self.log.push(format!("partition iterate {} {} {} 0 4", fb(p.x), fb(p.y), fb(range)));
        (0..self.objs.len()).find(|i| hh(self.seed() ^ self.cfg[16], *i as u32, 1000 + 98) & 1 != 0).map(|i| i as i32 + 1)
    }

    // --- batch A9 ---
    fn sa_button_is_valid_to_use_on(&self, b: ButtonH, src: ObjH, target: Option<ObjH>, _cmd: i32) -> bool {
        hh(self.seed(), (b * 64 + src) as u32, 83 + target.unwrap_or(0) as u32) & 1 != 0
    }
    fn sa_partition_closest_for_button(&self, p: &Coord3D, search: ButtonSearch) -> Option<(ObjH, f32)> {
        // the filters' salt: the player's affiliation (2), then as the stand-ins' kinds (a kind-of filter 1 + 8 * its bit, a thing
        // filter 3 + 8 * its template, the others 0)
        let salt: u32 = match search {
            ButtonSearch::Enemy | ButtonSearch::Unmanned => 98,
            ButtonSearch::GarrisonedBuilding => (2 * 7 + 57) * 343,
            ButtonSearch::KindOf(k) => (14 + 1 + 8 * k as u32) * 49,
            ButtonSearch::Building => (14 + 57) * 49,
            ButtonSearch::BuildingClass(k) => ((14 + 57) * 7 + 1 + 8 * k as u32) * 49,
            ButtonSearch::Template(t) => (14 + 3 + 8 * t as u32) * 49,
        };
        let o = (0..self.objs.len()).find(|i| hh(self.seed() ^ self.cfg[16], *i as u32, 3000 + salt) & 1 != 0)? as i32 + 1;
        Some((o, (hh(self.seed(), o as u32, p.x.to_bits()) % 1000) as f32))
    }
    fn sa_group_do_command_button_at_object_opt(&mut self, g: GroupH, b: ButtonH, t: Option<ObjH>, cmd: i32) {
        self.log.push(format!("group {g} buttonobj {b} {} {cmd}", t.unwrap_or(0)));
    }

    // --- batch A10 ---
    fn sa_view_camera_mod_freeze(&mut self, time: bool) {
        self.log.push(format!("view freeze{}", if time { "time" } else { "angle" }));
    }
    fn sa_view_camera_mod_final_time_multiplier(&mut self, m: i32) {
        self.log.push(format!("view modtimemult {m}"));
    }
    fn sa_view_camera_mod_rolling_average(&mut self, n: i32) {
        self.log.push(format!("view modrolling {n}"));
    }
    fn sa_view_set_time_multiplier(&mut self, m: i32) {
        self.log.push(format!("view timemult {m}"));
    }
    fn sa_audio_pause(&mut self, pause: bool, affect: i32) {
        self.log.push(format!("audio {} {affect}", if pause { "pause" } else { "resume" }));
    }
    fn sa_global_fps_limit(&self) -> i32 {
        30
    }
    fn sa_engine_set_fps_limit(&mut self, fps: i32) {
        self.log.push(format!("engine fps {fps}"));
    }
    fn sa_set_use_fps_limit(&mut self, u: bool) {
        self.use_fps = u;
    }
    fn sa_player_add_skill_points(&mut self, p: PlayerH, d: i32) {
        self.log.push(format!("player {p} addskill {d}"));
    }
    fn sa_player_rank_level(&self, p: PlayerH) -> i32 {
        ((self.cfg[128 + 4 * p as usize] >> 12) & 7) as i32
    }
    fn sa_player_set_rank_level(&mut self, p: PlayerH, l: i32) {
        self.log.push(format!("player {p} ranklevel {l}"));
    }
    fn sa_logic_set_rank_level_limit(&mut self, l: i32) {
        self.log.push(format!("logic ranklimit {l}"));
    }
    fn sa_science_from_name(&self, name: &[u8]) -> Option<i32> {
        let k = num(name, "sci");
        (0..6).contains(&k).then_some(k)
    }
    fn sa_player_grant_science(&mut self, p: PlayerH, s: i32) {
        self.log.push(format!("player {p} grantscience {s}"));
    }
    fn sa_player_purchase_science(&mut self, p: PlayerH, s: i32) {
        self.log.push(format!("player {p} purchasescience {s}"));
    }
    fn sa_player_science_availability_from_string(&self, _p: PlayerH, name: &[u8]) -> Option<i32> {
        match name {
            b"Available" => Some(0),
            b"Disabled" => Some(1),
            b"Hidden" => Some(2),
            _ => None,
        }
    }
    fn sa_player_set_science_availability(&mut self, p: PlayerH, s: i32, a: i32) {
        self.log.push(format!("player {p} scienceavail {s} {a}"));
    }
    fn sa_group_set_emoticon(&mut self, g: GroupH, n: &[u8], f: i32) {
        self.log.push(format!("group {g} emoticon {} {f}", st(n)));
    }
    fn sa_drawable_set_emoticon(&mut self, o: ObjH, n: &[u8], f: i32) {
        self.log.push(format!("draw {o} emoticon {} {f}", st(n)));
    }
    fn sa_object_type_list_maintenance(&mut self, l: &[u8], t: &[u8], add: bool) {
        self.log.push(format!("se objlist {} {} {}", st(l), st(t), u8::from(add)));
    }
    fn sa_named_map_reveal(&mut self, look: &[u8], w: &[u8], r: f32, p: &[u8]) {
        self.log.push(format!("se createreveal {} {} {} {}", st(look), st(w), fb(r), st(p)));
        self.log.push(format!("se doreveal {}", st(look)));
    }
    fn sa_undo_named_map_reveal(&mut self, look: &[u8]) {
        self.log.push(format!("se undoreveal {}", st(look)));
        self.log.push(format!("se removereveal {}", st(look)));
    }
    fn sa_eva_set_enabled(&mut self, e: bool) {
        self.log.push(format!("eva {}", u8::from(e)));
    }
    fn sa_logic_set_option(&mut self, which: i32, e: bool) {
        let n = ["behindbuilding", "drawicon", "dynamiclod"][which as usize];
        self.log.push(format!("logic {n} {}", u8::from(e)));
    }
    fn sa_logic_set_hulk_max_lifetime_override(&mut self, f: i32) {
        self.log.push(format!("logic hulk {f}"));
    }
    fn sa_view_camera_slave_mode(&mut self, slave: Option<(&[u8], &[u8])>) {
        match slave {
            Some((t, b)) => self.log.push(format!("view slave {} {}", st(t), st(b))),
            None => self.log.push("view unslave".into()),
        }
    }
    fn sa_view_add_camera_shake(&mut self, p: &Coord3D, r: f32, d: f32, a: f32) {
        self.log.push(format!("view addshake {} {} {} {} {} {}", fb(p.x), fb(p.y), fb(p.z), fb(r), fb(d), fb(a)));
    }
    fn sa_ai_face_object(&mut self, o: ObjH, t: ObjH, cmd: i32) {
        self.log.push(format!("ai {o} faceobj {t} {cmd}"));
    }
    fn sa_ai_face_position(&mut self, o: ObjH, p: &Coord3D, cmd: i32) {
        self.log.push(format!("ai {o} facepos {} {} {} {cmd}", fb(p.x), fb(p.y), fb(p.z)));
    }
    fn sa_player_set_skillset(&mut self, p: PlayerH, s: i32) {
        self.log.push(format!("player {p} skillset {s}"));
    }
    fn sa_player_guard_supply_center(&mut self, p: PlayerH, t: TeamH, s: i32) {
        self.log.push(format!("player {p} guardsupply {t} {s}"));
    }
    fn sa_ai_guard_tunnel_network(&mut self, o: ObjH, m: i32, cmd: i32) {
        self.log.push(format!("ai {o} guardtunnel {m} {cmd}"));
    }
    fn sa_template_command_set_string(&self, t: TemplateH) -> Vec<u8> {
        format!("cs{t}").into_bytes()
    }
    fn sa_logic_set_control_bar_override(&mut self, cs: &[u8], slot: i32, b: Option<ButtonH>) {
        self.log.push(format!("logic controlbaroverride {} {slot} {}", st(cs), b.unwrap_or(-1)));
    }
    fn sa_player_set_skill_points_modifier(&mut self, p: PlayerH, m: f32) {
        self.log.push(format!("player {p} skillmod {}", fb(m)));
    }
    fn sa_view_set_guard_band_bias(&mut self, x: f32, y: f32) {
        self.log.push(format!("view guardband {} {}", fb(x), fb(y)));
    }
    fn sa_all_objects(&self) -> Vec<ObjH> {
        (1..=self.objs.len() as i32).collect()
    }
    fn sa_obj_set_receiving_difficulty_bonus(&mut self, o: ObjH, b: bool) {
        self.log.push(format!("obj {o} difficultybonus {}", u8::from(b)));
    }
    fn sa_set_objects_should_receive_difficulty_bonus(&mut self, b: bool) {
        self.log.push(format!("se difficultybonus {}", u8::from(b)));
    }
    fn sa_set_choose_victim_always_uses_normal(&mut self, b: bool) {
        self.log.push(format!("se victimnormal {}", u8::from(b)));
    }
    fn sa_obj_has_railroad(&self, o: ObjH) -> bool {
        hh(self.seed(), o as u32, 54) & 1 != 0
    }
    fn sa_railroad_set_held(&mut self, o: ObjH, h: bool) {
        self.log.push(format!("railroad {o} held {}", u8::from(h)));
    }
}

impl Sim {
    /// `TeamFactory::make`: a new instance of the prototype the name names.
    fn make_team(&mut self, name: &[u8], how: &str) -> Option<TeamH> {
        let k = num(name, "team");
        if !(0..8).contains(&k) {
            self.log.push(format!("teamfactory {how} {} none", st(name)));
            return None;
        }
        let id = self.teams.len() as i32 + 1;
        self.teams.push(Team { proto: Some(k), members: Vec::new(), controller: None });
        self.protos[k as usize].push(id - 1);
        self.log.push(format!("teamfactory {how} {} {id}", st(name)));
        Some(id)
    }
}

/// An action as the program gives it.
#[derive(Clone, Debug)]
struct A {
    ty: i32,
    parms: Vec<Parameter>,
}

fn build_action(a: &A) -> ScriptAction {
    let mut sa = ScriptAction::new_for_read();
    sa.action_type = a.ty;
    sa.num_parms = a.parms.len() as i32;
    for (i, p) in a.parms.iter().enumerate().take(MAX_PARMS) {
        sa.parms[i] = Some(p.clone());
    }
    sa
}

fn pack_str(v: &mut Vec<u32>, s: &[u8]) {
    v.push(s.len() as u32);
    for ch in s.chunks(4) {
        let mut w = 0u32;
        for (k, b) in ch.iter().enumerate() {
            w |= u32::from(*b) << (8 * k);
        }
        v.push(w);
    }
}

fn encode_actions(acts: &[A]) -> Vec<u32> {
    let mut v = vec![acts.len() as u32];
    for a in acts {
        v.push(a.ty as u32);
        v.push(a.parms.len() as u32);
        for p in &a.parms {
            v.push(p.param_type as u32);
            v.push(p.int as u32);
            v.push(p.real.to_bits());
            v.extend([p.coord.x.to_bits(), p.coord.y.to_bits(), p.coord.z.to_bits()]);
            pack_str(&mut v, &p.string);
        }
    }
    v
}

fn put_str(out: &mut Vec<u32>, s: &str) {
    pack_str(out, s.as_bytes());
}

/// The Rust run: the output, and the operation at which upstream is undefined (the output stops before it).
fn rust_run(cfg: &[u32; NCFG], acts: &[A], ops: &[[u32; 3]]) -> (Vec<u32>, Option<(usize, String)>) {
    let mut sim = Sim::new(cfg);
    let actions: Vec<ScriptAction> = acts.iter().map(build_action).collect();
    let mut sa = ScriptActions::new();
    sa.init(&mut sim);
    let mut out = Vec::new();
    for (i, o) in ops.iter().enumerate() {
        sim.log.clear();
        if o[0] == 1 && (o[1] as usize) < NCFG {
            sim.cfg[o[1] as usize] = o[2];
        }
        if o[0] == 3 {
            sim.scene();
        }
        if o[0] == 0 && (o[1] as usize) < actions.len() {
            if let Err(e) = sa.execute_action(&mut sim, &actions[o[1] as usize]) {
                return (out, Some((i, format!("{} {e:?}", actions[o[1] as usize].action_type))));
            }
        }
        if o[0] == 2 {
            sa.init(&mut sim);
        }
        out.push(sim.log.len() as u32);
        for l in &sim.log {
            put_str(&mut out, l);
        }
        out.push(u32::from(sa.suppress_new_windows));
        out.push(sa.message_window.unwrap_or(0) as u32);
        out.push(sim.light_scale.to_bits());
        out.push(30);
        out.push(u32::from(sim.use_fps));
    }
    (out, None)
}

fn cpp_run(cfg: &[u32; NCFG], acts: &[A], ops: &[[u32; 3]]) -> Vec<u32> {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let enc = encode_actions(acts);
    let mut out = vec![0u32; 1 << 20];
    let mut len = 0i32;
    let st = unsafe { cpp::ref_actions_run(cfg.as_ptr(), enc.as_ptr(), flat.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32, &mut len) };
    assert_eq!(st, 0, "the output fits");
    out.truncate(len as usize);
    out
}

/// The parameters each translated type's execution reads: T team, U unit, W waypoint, S state, P player, B building or upgrade,
/// N sound or message, M thing, R a real, E an ease or time (a real), I an int, C a coordinate, F an object panel flag, X camera time
/// (a real, sometimes huge).
fn kinds(ty: i32) -> Option<&'static str> {
    use at::*;
    Some(match ty {
        DEBUG_MESSAGE_BOX | DEBUG_STRING | PLAY_SOUND_EFFECT => "N",
        DAMAGE_MEMBERS_OF_TEAM => "TR",
        MOVE_TEAM_TO => "TW",
        MOVE_NAMED_UNIT_TO => "UW",
        TEAM_SET_STATE => "TS",
        CREATE_REINFORCEMENT_TEAM => "TW",
        SKIRMISH_BUILD_BUILDING | SKIRMISH_BUILD_STRUCTURE_FRONT | SKIRMISH_BUILD_STRUCTURE_FLANK => "B",
        AI_PLAYER_BUILD_SUPPLY_CENTER => "PBI",
        AI_PLAYER_BUILD_TYPE_NEAREST_TEAM => "PBT",
        AI_PLAYER_BUILD_UPGRADE => "PB",
        SKIRMISH_BUILD_BASE_DEFENSE_FRONT | SKIRMISH_BUILD_BASE_DEFENSE_FLANK | CAMERA_STOP_FOLLOW | MOVE_CAMERA_TO_SELECTION
        | RESET_INFANTRY_LIGHTING_OVERRIDE | QUICKVICTORY | VICTORY | DEFEAT | LOCALDEFEAT | DEBUG_CRASH_BOX => "",
        PLAY_SOUND_EFFECT_AT => "NW",
        MOVE_CAMERA_TO | MOVE_CAMERA_ALONG_WAYPOINT_PATH => "WXXEE",
        SETUP_CAMERA => "WRRW",
        ZOOM_CAMERA | PITCH_CAMERA | ROTATE_CAMERA => "RXEE",
        CAMERA_FOLLOW_NAMED => "UI",
        OVERSIZE_TERRAIN => "I",
        CAMERA_MOD_LOOK_TOWARD | CAMERA_MOD_FINAL_LOOK_TOWARD => "W",
        CAMERA_LOOK_TOWARD_OBJECT => "UXXEE",
        CAMERA_LOOK_TOWARD_WAYPOINT => "WXEEI",
        RESET_CAMERA => "WXEE",
        SET_INFANTRY_LIGHTING_OVERRIDE => "R",
        CREATE_OBJECT => "MTCR",
        UNIT_SPAWN_NAMED_LOCATION_ORIENTATION => "UMTCR",
        TEAM_ATTACK_TEAM => "TT",
        NAMED_ATTACK_NAMED => "UU",
        CREATE_NAMED_ON_TEAM_AT_WAYPOINT => "UMTW",
        CREATE_UNNAMED_ON_TEAM_AT_WAYPOINT => "MTW",
        ENABLE_OBJECT_SOUND | DISABLE_OBJECT_SOUND => "U",
        // batch A2
        TEAM_FOLLOW_WAYPOINTS | TEAM_FOLLOW_WAYPOINTS_EXACT => "TLI",
        NAMED_FOLLOW_WAYPOINTS | NAMED_FOLLOW_WAYPOINTS_EXACT => "UL",
        SKIRMISH_FOLLOW_APPROACH_PATH => "TQI",
        SKIRMISH_MOVE_TO_APPROACH_PATH => "TQ",
        NAMED_APPLY_ATTACK_PRIORITY_SET => "UA",
        TEAM_APPLY_ATTACK_PRIORITY_SET => "TA",
        SET_BASE_CONSTRUCTION_SPEED => "PI",
        NAMED_SET_ATTITUDE | NAMED_SET_REPULSOR | NAMED_SET_EVAC_LEFT_OR_RIGHT => "UI",
        TEAM_SET_ATTITUDE | TEAM_SET_REPULSOR => "TI",
        NAMED_ATTACK_AREA => "UG",
        NAMED_ATTACK_TEAM => "UT",
        TEAM_ATTACK_AREA | TEAM_GUARD_AREA => "TG",
        TEAM_ATTACK_NAMED | TEAM_ENTER_NAMED | TEAM_GUARD_OBJECT => "TU",
        TEAM_LOAD_TRANSPORTS | TEAM_EXIT_ALL | TEAM_GUARD | TEAM_HUNT => "T",
        NAMED_ENTER_NAMED => "UU",
        NAMED_EXIT_ALL | NAMED_GUARD | NAMED_HUNT => "U",
        TEAM_GUARD_POSITION => "TW",
        TEAM_HUNT_WITH_COMMAND_BUTTON => "TK",
        PLAYER_HUNT | PLAYER_SELL_EVERYTHING | PLAYER_DISABLE_BASE_CONSTRUCTION | PLAYER_DISABLE_UNIT_CONSTRUCTION
        | PLAYER_ENABLE_BASE_CONSTRUCTION | PLAYER_ENABLE_UNIT_CONSTRUCTION => "P",
        PLAYER_DISABLE_FACTORIES | PLAYER_ENABLE_FACTORIES => "PB",
        PLAYER_REPAIR_NAMED_STRUCTURE => "PU",
        CAMERA_MOVE_HOME => "",
        // batch A3
        RECRUIT_TEAM => "TR",
        BUILD_TEAM | TEAM_DELETE | TEAM_DELETE_LIVING | TEAM_WANDER_IN_PLACE | TEAM_INCREASE_PRIORITY | TEAM_DECREASE_PRIORITY | TEAM_KILL => "T",
        NAMED_DAMAGE | NAMED_FLASH | NAMED_FLASH_WHITE | NAMED_CUSTOM_COLOR => "UI",
        NAMED_DELETE | NAMED_KILL => "U",
        TEAM_WANDER | TEAM_PANIC => "TL",
        PLAYER_KILL | PLAYER_EXCLUDE_FROM_SCORE_SCREEN => "P",
        DISPLAY_TEXT | MOVIE_PLAY_FULLSCREEN | MOVIE_PLAY_RADAR => "N",
        DISPLAY_CINEMATIC_TEXT => "NYI",
        INGAME_POPUP_MESSAGE => "NIIII",
        CAMEO_FLASH => "KI",
        TEAM_FLASH | TEAM_FLASH_WHITE => "TI",
        SOUND_PLAY_NAMED => "NU",
        SPEECH_PLAY => "NI",
        PLAYER_TRANSFER_OWNERSHIP_PLAYER => "PP",
        NAMED_TRANSFER_OWNERSHIP_PLAYER => "UP",
        ENABLE_SCORING | DISABLE_SCORING => "",
        PLAYER_RELATES_PLAYER => "PPI",
        RADAR_CREATE_EVENT => "CI",
        // batch A4
        OBJECT_CREATE_RADAR_EVENT => "UI",
        TEAM_CREATE_RADAR_EVENT | TEAM_AVAILABLE_FOR_RECRUITMENT => "TI",
        RADAR_DISABLE | RADAR_ENABLE | IDLE_ALL_UNITS | RESUME_SUPPLY_TRUCKING | DISABLE_INPUT | ENABLE_INPUT | DISABLE_BORDER_SHROUD
        | ENABLE_BORDER_SHROUD | SOUND_AMBIENT_PAUSE | SOUND_AMBIENT_RESUME => "",
        MAP_REVEAL_AT_WAYPOINT | MAP_SHROUD_AT_WAYPOINT => "WRP",
        MAP_REVEAL_ALL | MAP_REVEAL_ALL_PERM | MAP_REVEAL_ALL_UNDO_PERM | MAP_SHROUD_ALL => "P",
        TEAM_COLLECT_NEARBY_FOR_TEAM | TEAM_GARRISON_NEAREST_BUILDING | TEAM_EXIT_ALL_BUILDINGS => "T",
        TEAM_MERGE_INTO_TEAM => "TT",
        MUSIC_SET_TRACK => "NII",
        TEAM_GARRISON_SPECIFIC_BUILDING => "TU",
        EXIT_SPECIFIC_BUILDING => "U",
        CAMERA_MOTION_BLUR => "II",
        CAMERA_MOTION_BLUR_JUMP => "WI",
        // batch A5
        NAMED_SET_STEALTH_ENABLED | SHOW_MILITARY_CAPTION | NAMED_SET_HELD => "UI",
        TEAM_SET_STEALTH_ENABLED => "TI",
        NAMED_SET_UNMANNED_STATUS | NAMED_GARRISON_NEAREST_BUILDING | NAMED_EXIT_BUILDING | NAMED_HIDE_SPECIAL_POWER_DISPLAY
        | NAMED_SHOW_SPECIAL_POWER_DISPLAY => "U",
        TEAM_SET_UNMANNED_STATUS => "T",
        NAMED_SET_BOOBYTRAPPED => "MU",
        TEAM_SET_BOOBYTRAPPED => "MT",
        NAMED_GARRISON_SPECIFIC_BUILDING => "UU",
        PLAYER_GARRISON_ALL_BUILDINGS | PLAYER_EXIT_ALL_BUILDINGS => "P",
        CAMERA_LETTERBOX_BEGIN | CAMERA_LETTERBOX_END | DRAW_SKYBOX_BEGIN | DRAW_SKYBOX_END | FREEZE_TIME | UNFREEZE_TIME
        | DISABLE_SPECIAL_POWER_DISPLAY | ENABLE_SPECIAL_POWER_DISPLAY | DISABLE_COUNTDOWN_TIMER_DISPLAY | ENABLE_COUNTDOWN_TIMER_DISPLAY => "",
        CAMERA_BW_MODE_BEGIN | CAMERA_BW_MODE_END | SHOW_WEATHER => "I",
        CAMERA_SET_AUDIBLE_DISTANCE | MUSIC_SET_VOLUME | SOUND_SET_VOLUME | SPEECH_SET_VOLUME => "R",
        SET_STOPPING_DISTANCE => "TR",
        NAMED_SET_STOPPING_DISTANCE => "UR",
        TEAM_TRANSFER_TO_PLAYER => "TP",
        PLAYER_SET_MONEY | PLAYER_GIVE_MONEY => "PI",
        DISPLAY_COUNTDOWN_TIMER | DISPLAY_COUNTER => "NN",
        HIDE_COUNTDOWN_TIMER | HIDE_COUNTER => "N",
        NAMED_STOP_SPECIAL_POWER_COUNTDOWN | NAMED_START_SPECIAL_POWER_COUNTDOWN => "UO",
        NAMED_SET_SPECIAL_POWER_COUNTDOWN | NAMED_ADD_SPECIAL_POWER_COUNTDOWN => "UOI",
        NAMED_FIRE_SPECIAL_POWER_AT_WAYPOINT => "UOW",
        // batch A6
        SKIRMISH_FIRE_SPECIAL_POWER_AT_MOST_COST => "PO",
        NAMED_FIRE_SPECIAL_POWER_AT_NAMED => "UOU",
        REFRESH_RADAR | CAMERA_STOP_TETHER_NAMED => "",
        NAMED_STOP | UNIT_STOP_SEQUENTIAL_SCRIPT => "U",
        TEAM_STOP | TEAM_STOP_AND_DISBAND | TEAM_REMOVE_ALL_OVERRIDE_RELATIONS | TEAM_STOP_SEQUENTIAL_SCRIPT => "T",
        CAMERA_TETHER_NAMED => "UIR",
        CAMERA_SET_DEFAULT => "RRR",
        TEAM_SET_OVERRIDE_RELATION_TO_TEAM => "TTI",
        TEAM_REMOVE_OVERRIDE_RELATION_TO_TEAM => "TT",
        TEAM_SET_OVERRIDE_RELATION_TO_PLAYER => "TPI",
        TEAM_REMOVE_OVERRIDE_RELATION_TO_PLAYER => "TP",
        PLAYER_SET_OVERRIDE_RELATION_TO_TEAM => "PTI",
        PLAYER_REMOVE_OVERRIDE_RELATION_TO_TEAM => "PT",
        NAMED_FIRE_WEAPON_FOLLOWING_WAYPOINT_PATH => "UL",
        NAMED_USE_COMMANDBUTTON_ABILITY => "UK",
        NAMED_USE_COMMANDBUTTON_ABILITY_ON_NAMED => "UKU",
        NAMED_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT => "UKW",
        NAMED_USE_COMMANDBUTTON_ABILITY_USING_WAYPOINT_PATH => "UKL",
        TEAM_USE_COMMANDBUTTON_ABILITY => "TK",
        TEAM_USE_COMMANDBUTTON_ABILITY_ON_NAMED => "TKU",
        TEAM_USE_COMMANDBUTTON_ABILITY_AT_WAYPOINT => "TKW",
        UNIT_EXECUTE_SEQUENTIAL_SCRIPT => "US",
        UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING => "USI",
        TEAM_EXECUTE_SEQUENTIAL_SCRIPT => "TS",
        TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING => "TSI",
        UNIT_GUARD_FOR_FRAMECOUNT | UNIT_IDLE_FOR_FRAMECOUNT => "UI",
        TEAM_GUARD_FOR_FRAMECOUNT | TEAM_IDLE_FOR_FRAMECOUNT => "TI",
        // batch A7
        WATER_CHANGE_HEIGHT => "VR",
        WATER_CHANGE_HEIGHT_OVER_TIME => "VRRR",
        MAP_SWITCH_BORDER | SCREEN_SHAKE => "I",
        OBJECT_FORCE_SELECT => "TMIN",
        UNIT_DESTROY_ALL_CONTAINED => "U",
        RADAR_FORCE_ENABLE | RADAR_REVERT_TO_NORMAL | SOUND_ENABLE_ALL | SOUND_REMOVE_ALL_DISABLED | AUDIO_RESTORE_VOLUME_ALL_TYPE => "",
        TECHTREE_MODIFY_BUILDABILITY_OBJECT => "MI",
        SET_CAVE_INDEX | WAREHOUSE_SET_VALUE => "UI",
        SOUND_DISABLE_TYPE | SOUND_ENABLE_TYPE | SOUND_REMOVE_TYPE | AUDIO_RESTORE_VOLUME_TYPE => "N",
        AUDIO_OVERRIDE_VOLUME_TYPE => "NR",
        NAMED_SET_TOPPLE_DIRECTION => "UC",
        // batch A8
        UNIT_MOVE_TOWARDS_NEAREST_OBJECT_TYPE => "UJG",
        TEAM_MOVE_TOWARDS_NEAREST_OBJECT_TYPE => "TJG",
        NAMED_RECEIVE_UPGRADE => "UZ",
        SKIRMISH_ATTACK_NEAREST_GROUP_WITH_VALUE => "TcI",
        SKIRMISH_PERFORM_COMMANDBUTTON_ON_MOST_VALUABLE_OBJECT => "TKRI",
        // batch A9
        TEAM_SPIN_FOR_FRAMECOUNT => "TI",
        TEAM_ALL_USE_COMMANDBUTTON_ON_NAMED => "TKU",
        TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_UNIT | TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_GARRISONED_BUILDING
        | TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING => "TK",
        TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_KINDOF | TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_ENEMY_BUILDING_CLASS => "TKk",
        TEAM_ALL_USE_COMMANDBUTTON_ON_NEAREST_OBJECTTYPE => "TKJ",
        TEAM_PARTIAL_USE_COMMANDBUTTON => "RTK",
        TEAM_CAPTURE_NEAREST_UNOWNED_FACTION_UNIT => "T",
        PLAYER_CREATE_TEAM_FROM_CAPTURED_UNITS => "PT",
        // batch A10
        CAMERA_MOD_FREEZE_TIME | CAMERA_MOD_FREEZE_ANGLE | SUSPEND_BACKGROUND_SOUNDS | RESUME_BACKGROUND_SOUNDS | CAMERA_MOTION_BLUR_END_FOLLOW
        | SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL | SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_PARTIAL | TEAM_WAIT_FOR_NOT_CONTAINED_ALL
        | TEAM_WAIT_FOR_NOT_CONTAINED_PARTIAL | CAMERA_DISABLE_SLAVE_MODE | DELETE_ALL_UNMANNED => "",
        CAMERA_MOD_SET_FINAL_ZOOM | CAMERA_MOD_SET_FINAL_PITCH => "REE",
        CAMERA_MOD_SET_FINAL_SPEED_MULTIPLIER | CAMERA_MOD_SET_ROLLING_AVERAGE | SET_VISUAL_SPEED_MULTIPLIER | CAMERA_MOTION_BLUR_FOLLOW
        | SET_FPS_LIMIT | PLAYER_SET_RANKLEVELLIMIT | EVA_SET_ENABLED_DISABLED | OPTIONS_SET_OCCLUSION_MODE | OPTIONS_SET_DRAWICON_UI_MODE
        | OPTIONS_SET_PARTICLE_CAP_MODE | OBJECT_ALLOW_BONUSES | CHOOSE_VICTIM_ALWAYS_USES_NORMAL => "I",
        PLAYER_ADD_SKILLPOINTS | PLAYER_ADD_RANKLEVEL | PLAYER_SET_RANKLEVEL | PLAYER_SELECT_SKILLSET => "PI",
        PLAYER_GRANT_SCIENCE | PLAYER_PURCHASE_SCIENCE => "Ps",
        PLAYER_SCIENCE_AVAILABILITY => "Psa",
        TEAM_SET_EMOTICON => "TNX",
        NAMED_SET_EMOTICON => "UNX",
        OBJECTLIST_ADDOBJECTTYPE | OBJECTLIST_REMOVEOBJECTTYPE => "JM",
        MAP_REVEAL_PERMANENTLY_AT_WAYPOINT => "WRPN",
        MAP_UNDO_REVEAL_PERMANENTLY_AT_WAYPOINT => "N",
        CAMERA_ENABLE_SLAVE_MODE => "MN",
        CAMERA_ADD_SHAKER_AT => "WRRR",
        SCRIPTING_OVERRIDE_HULK_LIFETIME => "X",
        NAMED_FACE_NAMED => "UU",
        NAMED_FACE_WAYPOINT => "UW",
        TEAM_FACE_NAMED => "TU",
        TEAM_FACE_WAYPOINT => "TW",
        UNIT_AFFECT_OBJECT_PANEL_FLAGS => "UFI",
        TEAM_AFFECT_OBJECT_PANEL_FLAGS => "TFI",
        COMMANDBAR_REMOVE_BUTTON_OBJECTTYPE => "KM",
        COMMANDBAR_ADD_BUTTON_OBJECTTYPE_SLOT => "KMI",
        PLAYER_AFFECT_RECEIVING_EXPERIENCE => "PR",
        TEAM_GUARD_SUPPLY_CENTER => "TI",
        TEAM_GUARD_IN_TUNNEL_NETWORK => "T",
        RESIZE_VIEW_GUARDBAND => "RR",
        SET_TRAIN_HELD => "UI",
        _ => "",
    })
}

fn sv(x: String) -> Vec<u8> {
    x.into_bytes()
}

fn gen_parm(g: &mut Gen, k: char) -> Parameter {
    let mut p = Parameter::new(if k == 'C' { parameter_type::COORD3D } else { 0 }, 0);
    match k {
        'T' => {
            p.string = match g.below(12) {
                0 | 1 => sv("<This Team>".into()),
                2 => sv(format!("lone{}", g.below(18))),
                3 => sv("nobody".into()),
                _ => sv(format!("team{}", g.below(9))),
            }
        }
        'U' => p.string = if g.below(10) == 0 { Vec::new() } else { sv(format!("unit{}", g.below(20))) },
        'W' => p.string = sv(format!("wp{}", g.below(9))),
        'S' => p.string = if g.below(2) == 0 { sv(format!("state{}", g.below(4))) } else { sv(format!("script{}", g.below(3))) },
        'P' => {
            p.string = match g.below(8) {
                0 => sv("<This Player>".into()),
                1 => sv("<This Player's Enemy>".into()),
                _ => sv(format!("player{}", g.below(9))),
            }
        }
        'B' => p.string = sv(format!("bld{}", g.below(4))),
        'N' => p.string = sv(format!("snd{}", g.below(4))),
        'M' => p.string = sv(format!("tmpl{}", g.below(9))),
        'R' => p.real = [0.0f32, 0.5, 1.0, 2.5, -1.0, 0.25, 100.0][g.below(7) as usize],
        'E' => p.real = g.below(8) as f32 * 0.125,
        'X' => {
            p.real = match g.below(30) {
                0 => 3.0e6,
                1 => f32::NAN,
                2..=14 => g.below(40) as f32 * 0.3707,
                _ => g.below(40) as f32 * 0.37,
            }
        }
        'I' => p.int = if g.below(8) == 0 { g.i32() } else { g.below(4) as i32 },
        'C' => {
            p.coord = Coord3D { x: g.below(100) as f32 * 0.5, y: g.below(100) as f32 * 0.5, z: g.below(4) as f32 };
            if g.below(10) == 0 {
                p.param_type = parameter_type::REAL; // not a COORD3D: getCoord3D gives zero
            }
        }
        'F' => p.string = OBJECT_FLAGS_NAMES[g.below(7) as usize].to_vec(),
        'L' => p.string = sv(format!("path{}", g.below(9))),
        'Q' => p.string = if g.below(8) == 0 { sv("x".into()) } else { sv("path".into()) },
        'A' => p.string = sv(format!("set{}", g.below(4))),
        'G' => p.string = sv(format!("trig{}", g.below(9))),
        'O' => p.string = sv(format!("power{}", g.below(8))),
        'V' => p.string = sv(format!("water{}", g.below(5))),
        'J' => p.string = if g.below(3) == 0 { sv(format!("list{}", g.below(5))) } else { sv(format!("tmpl{}", g.below(9))) },
        'Z' => p.string = sv(format!("upg{}", g.below(5))),
        'c' => p.int = g.below(6) as i32,
        'k' => p.int = [7, 8, 18, 24, 65, 99, 100][g.below(7) as usize],
        's' => p.string = if g.below(6) == 0 { sv("bad".into()) } else { sv(format!("sci{}", g.below(7))) },
        'a' => p.string = sv(["Available", "Disabled", "Hidden", "Other"][g.below(4) as usize].into()),
        'K' => p.string = sv(format!("btn{}", g.below(10))),
        'Y' => {
            p.string = match g.below(9) {
                0 => sv("Arial - Size: 12".into()),
                1 => sv("Arial - Size:12 [Bold]".into()),
                2 => sv("Times-Size:9".into()),
                3 => sv("Courier".into()),
                4 => sv("A b: c".into()),
                5 => sv("x:1".into()),
                6 => vec![b'a'; 300],
                7 => sv(format!("F{} - Size:{} [Bold]", g.below(9), g.below(40))),
                8 if g.below(2) == 0 => sv(format!("G{} [Bold] - Size:{}", g.below(9), g.below(40))),
                _ => sv(format!("Font{} - Size:{}", g.below(9), g.below(40))),
            }
        }
        _ => {}
    }
    p
}

/// The types of the translated batches.
fn translated() -> Vec<i32> {
    (0..at::NUM_ITEMS).filter(|t| kinds(*t).is_some()).collect()
}

fn gen_program(g: &mut Gen, types: &[i32]) -> ([u32; NCFG], Vec<A>, Vec<[u32; 3]>) {
    let mut cfg = [0u32; NCFG];
    for w in cfg.iter_mut() {
        *w = g.u32();
    }
    let np = 1 + g.below(6);
    cfg[0] &= 0xfff;
    cfg[1] = g.below(1000);
    cfg[4] = np | (if g.below(10) == 0 { 15 } else { g.below(np) }) << 4 | g.below(np + 1) << 8;
    cfg[5] = (if g.below(5) == 0 { 255 } else { g.below(16) }) | g.below(np + 1) << 8 | g.below(np + 1) << 12;
    cfg[11] = (3 + g.below(6)) | g.below(6) << 4 | g.below(8) << 8;
    cfg[12] = (g.u32() & 0xff) | (g.u32() & 0xff) << 8;
    for t in 0..8usize {
        // templates: kinds in bits 0-15, slots 16-17, contain max 18-20, contain 21, AI 22, deliver payload 23, put-in container 24-26
        cfg[24 + t] = (g.u32() & 0x0000_ffff) | g.below(4) << 16 | g.below(8) << 18 | u32::from(g.below(2) == 0) << 21
            | u32::from(g.below(4) != 0) << 22 | u32::from(g.below(4) == 0) << 23 | g.below(8) << 24;
    }
    for j in 0..16usize {
        let mut w0 = g.u32();
        if g.below(5) != 0 {
            w0 |= 1;
        }
        if g.below(3) != 0 {
            w0 &= !2;
        }
        if g.below(2) == 0 {
            // most of the objects on the first four teams, so teams have several members (of one template, too)
            w0 &= !(0xc << 20);
        }
        cfg[40 + 4 * j] = w0;
    }
    for k in 0..8usize {
        cfg[96 + k] = (if g.below(6) == 0 { 15 } else { g.below(np) }) | (g.u32() & g.u32() & 0xffff) << 4;
        // reinforcement info: waypoint 0-2, transport 3 (type 4-6), units 7-8, each (type, max), starts full 24, exit 25
        cfg[104 + k] = g.u32() & 0x03ff_ffff;
    }
    let na = 1 + g.below(8) as usize;
    let mut acts = Vec::new();
    for _ in 0..na {
        let ty = types[g.below(types.len() as u32) as usize];
        let mut parms: Vec<Parameter> = kinds(ty).unwrap().chars().map(|k| gen_parm(g, k)).collect();
        if g.below(30) == 0 && !parms.is_empty() {
            let keep = g.below(parms.len() as u32) as usize;
            parms.truncate(keep);
        }
        acts.push(A { ty, parms });
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=69 => [0, g.below(na as u32), 0],
            70..=71 => [2, 0, 0],
            72..=73 => [3, 0, 0],
            _ => {
                let i = match g.below(5) {
                    0 => g.below(16),
                    1 => 24 + g.below(16),
                    2 => 160 + g.below(24),
                    _ => 40 + g.below(80),
                };
                [1, i, g.u32()]
            }
        };
        ops.push(o);
    }
    (cfg, acts, ops)
}

/// Effects every run must show (so that the paths that make them are compared).
const REACH: [&str; 24] = [
    "window create Menus/ObserverQuit.wnd",
    "window create Menus/Victorious.wnd",
    "window create Menus/LocalDefeat.wnd",
    "teamfactory createinactive",
    "factory newobject",
    " add ",
    "dp ",
    "moveevacexit",
    " moveevac ",
    "se transfername",
    "se addcache",
    "terrain crater",
    "view modfinalmove",
    "group ",
    "followpathteamexact",
    "checkbridges",
    "cbhunt",
    "display font",
    "ui subtitle",
    "partition iterateobj",
    "setteam",
    " panic ",
    "removehandle",
    "view filterpos",
];

#[test]
fn script_actions_match_upstream() {
    let cases: usize = std::env::var("ACTIONS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(40_000);
    let types = translated();
    let mut g = Gen::new(0x5AC7_0001);
    let (mut cut, mut ops_run, mut execs, mut lines, mut mismatches) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut by_type = std::collections::BTreeMap::<i32, u64>::new();
    let mut cut_why = std::collections::BTreeMap::<String, u64>::new();
    let mut reached = std::collections::BTreeMap::<&str, u64>::new();
    for case in 0..cases {
        let (cfg, acts, ops) = gen_program(&mut g, &types);
        let (r, undefined_at) = rust_run(&cfg, &acts, &ops);
        let n = undefined_at.as_ref().map(|u| u.0).unwrap_or(ops.len());
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            let (who, e) = why.split_once(' ').unwrap_or((why.as_str(), ""));
            let variant = e.rsplit("Undefined(").next().and_then(|r| r.split(')').next()).unwrap_or(e);
            *cut_why.entry(format!("{variant} (type {who})")).or_insert(0) += 1;
        }
        let c = cpp_run(&cfg, &acts, &ops[..n]);
        ops_run += n as u64;
        for o in &ops[..n] {
            if o[0] == 0 {
                execs += 1;
                *by_type.entry(acts[o[1] as usize].ty).or_insert(0) += 1;
            }
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let at = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
                let text = |v: &[u32]| String::from_utf8_lossy(&v.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()).replace('\0', ".");
                println!(
                    "case {case}: acts {:?}\n ops {:?}\n  first difference at word {at}:\n  rust {}\n  c++  {}",
                    acts.iter().map(|a| (a.ty, a.parms.iter().map(|p| (st(&p.string), p.int, p.real)).collect::<Vec<_>>())).collect::<Vec<_>>(),
                    &ops[..n],
                    text(&r[at.saturating_sub(16)..(at + 40).min(r.len())]),
                    text(&c[at.saturating_sub(16)..(at + 40).min(c.len())])
                );
            }
        } else {
            lines += r.len() as u64;
            let s = String::from_utf8_lossy(&r.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>()).to_string();
            for w in REACH {
                *reached.entry(w).or_insert(0) += s.matches(w).count() as u64;
            }
        }
    }
    let missing: Vec<i32> = types.iter().copied().filter(|t| !by_type.contains_key(t)).collect();
    println!(
        "A: {cases} programs ({cut} cut at an undefined operation), {ops_run} ops ({execs} executions of {} types), {lines} output words, {mismatches} mismatches; never executed: {missing:?}",
        types.len()
    );
    println!("cut at: {cut_why:?}");
    println!("effects: {reached:?}");
    assert_eq!(mismatches, 0);
    for w in REACH {
        assert!(reached.get(w).copied().unwrap_or(0) > 0, "the generator must reach `{w}`");
    }
    assert!(missing.is_empty(), "every translated type is executed");
}
