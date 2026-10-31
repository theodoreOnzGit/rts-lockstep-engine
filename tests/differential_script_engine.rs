#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptEngine.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ScriptEngine` at run time and its save and load (`logic::world::script_engine`) against the C++, `p1w13-sonnet`, layer 11
//! batches E2 and E4.
//!
//! # Method
//!
//! The C++ side is `ScriptEngine.cpp` and the real `ScriptEngine.h`, compiled by `differential/wrap_engine.cpp` on the stand-ins of
//! `differential/shim_engine/se_world.h` (build.rs's copy of the file has three mechanical edits, all about the script debugger
//! and particle editor DLLs: see build.rs). The world is the configuration of 256 words the wrapper documents and its own state:
//! eight players, eight team prototypes and teams, sixteen objects, templates, waypoints, trigger areas from a hash; the scripts
//! of up to eight sides, decoded from words; `TheScriptConditions` and `TheScriptActions` are stand-ins that answer from a hash,
//! keep a condition's `m_customData` count (so a condition's change is seen) and, for the sequential script, unit and team stop,
//! frame count and delete actions, call back into the engine as the real ones do. Every call with an effect writes a line, and
//! after each operation the whole state is written: the engine's members and every script's changing fields.
//!
//! The Rust side is the module on [`ScriptEngineHost`], implemented here on the same world with the same functions, the scripts
//! the crate's `Script`, `ScriptGroup` and `ScriptList` decoded from the same words.
//!
//! A program is the configuration, the scripts (names, flags, delays, or-terms of conditions, actions and false actions, of the
//! engine's own types and others, with parameters that name things that exist or not), and 1 to 40 operations: an update, frames
//! passing, a configuration word, running scripts for a team or an object, the notifications and their questions, the named
//! object cache, named reveals, topple directions, object type lists, the lookups, reset, newMap, `friend_executeAction`,
//! `evaluateConditions`, the timers and flags, the music track, object counts, sequential scripts appended, removed and timed,
//! the named cache, `executeScript`, a save, a save and load round trip (the bytes as they are, or damaged). The C++ `Xfer` is an
//! in-memory stand-in with `XferSave`'s and `XferLoad`'s formats; the Rust side uses the crate's `XferSave` and `XferLoad`. When the Rust side finds upstream undefined at an operation, the C++ runs the operations before it.
//!
//! # Results (2026-10-31, `ENGINE_CASES=20000`, the default; batches E2 and E4)
//!
//! 20,000 programs, 330,759 operations compared (68,054 updates; every operation kind 9,700 times or more, saves and save-load
//! round trips about 10,000 each), 716,430 log lines: **0 mismatches**. A round trip saves, resets, and loads the bytes, as they
//! are or with a byte flipped or the end cut off (a load that throws ends the program on both sides, with "load failed"); a save
//! is compared byte for byte. 5,064 programs were cut at an operation upstream does not define (their prefix compared): 2,679 a
//! counter, flag or timer index a parameter caches outside `0..256`, or a notification's player index outside the 16; 1,241 a
//! missing parameter; 497 `setToppleDirection` with no direction for a new name; 364 `getObjectCount` of a player that does not
//! exist; 153 a random or millisecond timer's `Real` an `Int` cannot hold; 55 a load into a state upstream cannot use
//! ([`LOAD_UNDEFINED`]); 35 the local player missing (`loadPostProcess` with a music track, the challenge's `teamThePlayer`); 40 a
//! counter's `Int` overflow. (No program met an action adding or removing a sequential script under the progress loop's iterator:
//! that path is modelled from the vector's growth policy, not seen; and none called a subroutine 1,000 deep in this run.)
//!
//! Defects of the fixture found on the way, all in the stand-ins or the test: an object without a thing template (real objects
//! always have one: `addObjectToCache` formats its name); the Rust test's thread stack, too small for the 1,000 nested subroutine
//! calls both sides make before the Rust side calls it an overflow (the test runs in a thread with a 1 GiB stack); the C++ log's
//! 512-byte lines, which cut the saves short.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`, 20,000 programs: all 19 caught. A plant must only make the Rust side do more, or change state and lines: one
//! that makes it skip work the C++ does can hide upstream's undefined behaviour from it, and the unplanted C++ then crashes (the
//! first 3902, 3907, 3913, 3924 and 3926, which changed one-shot clearing, sequential waits, empty or-terms, the save's format and
//! the music track's playing, did; replaced).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3901 | `setFade` with no increase frames does not update the fade at once | 1,209 |
//! | 3902 | a transferred name never carries the old object's colour | 54 |
//! | 3905 | a singleton team returned when inactive | 6 |
//! | 3907 | completed videos noted at the front | 16 |
//! | 3908 | an appended sequential script keeps its instruction | 271 |
//! | 3909 | a power or upgrade question ignores the source object | 2 |
//! | 3911 | `<Local Player>` names no player | 3 |
//! | 3913 | object counts stored one more | 3,326 |
//! | 3914 | the fade's decrease divides by its frames, not frames + 1 | 8,761 |
//! | 3915 | attack priorities set on the template, not its final override | 35 |
//! | 3916 | a named reveal twice its radius | 1 |
//! | 3917 | a named reveal never removed | 26 |
//! | 3921 | a loaded attack priority set on the template, not its final override | 28 |
//! | 3922 | a load without a fade does not start the fade in | 801 |
//! | 3923 | named objects saved without their ids | 4,290 |
//! | 3924 | a loaded named reveal twice its radius | 147 |
//! | 3925 | the breeze's version not loaded | 53 |
//! | 3926 | the music track played for the next player index | 48 |

use std::collections::BTreeMap;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::ini::{IniError, Undefined};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::scripts::{action_type as at, condition_type as ct, parameter_type as pt, Condition, OrCondition, Parameter, Script, ScriptAction, ScriptGroup, ScriptList, MAX_PARMS};
use rts_lockstep_engine::logic::world::script_engine::*;

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

const SPECIAL: [&str; 18] = [
    "",
    "<none>",
    "<This Team>",
    "<This Object>",
    "<This Player>",
    "<Local Player>",
    "ThePlayer",
    "teamThePlayer",
    "<This Player's Enemy>",
    "[Skirmish]MyInnerPerimeter",
    "[Skirmish]MyOuterPerimeter",
    "[Skirmish]EnemyInnerPerimeter",
    "[Skirmish]EnemyOuterPerimeter",
    "USA Team is Building0",
    "USA Team is Building3",
    "GLA Inf Team is Building15",
    "China Air Team Is Building7",
    "USA Team is Building",
];

/// `seStr(id)`.
fn se_str(id: u32) -> Vec<u8> {
    let (c, k) = (id / 100, id % 100);
    match c {
        0 => SPECIAL.get(k as usize).copied().unwrap_or("").as_bytes().to_vec(),
        1 => format!("script{k}").into_bytes(),
        2 => format!("team{k}").into_bytes(),
        3 => format!("unit{k}").into_bytes(),
        4 => format!("player{k}").into_bytes(),
        5 => format!("c{k}").into_bytes(),
        6 => format!("tmpl{k}").into_bytes(),
        7 => format!("wp{k}").into_bytes(),
        8 => format!("list{k}").into_bytes(),
        9 => if k < 50 { format!("InnerPerimeter{k}") } else { format!("OuterPerimeter{}", k - 50) }.into_bytes(),
        _ => format!("snd{k}").into_bytes(),
    }
}

/// The scripts from their words (`readSides`).
struct Rd<'a> {
    w: &'a [u32],
    at: usize,
}
impl Rd<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w.get(self.at).copied().unwrap_or(0);
        if self.at < self.w.len() {
            self.at += 1;
        }
        v
    }
    fn parms(&mut self) -> (i32, [Option<Parameter>; MAX_PARMS]) {
        let n = (self.u() % (MAX_PARMS as u32 + 1)) as i32;
        let mut parms: [Option<Parameter>; MAX_PARMS] = Default::default();
        for p in parms.iter_mut().take(n as usize) {
            let ty = self.u() as i32;
            let mut q = Parameter::new(ty, self.u() as i32);
            q.real = f32::from_bits(self.u());
            q.string = se_str(self.u());
            *p = Some(q);
        }
        (n, parms)
    }
    fn actions(&mut self) -> Vec<ScriptAction> {
        let n = self.u() % 64;
        (0..n)
            .map(|_| {
                let mut a = ScriptAction::new_for_read();
                a.action_type = self.u() as i32;
                (a.num_parms, a.parms) = self.parms();
                a
            })
            .collect()
    }
    fn script(&mut self) -> Script {
        let mut s = Script::new();
        s.script_name = se_str(self.u());
        let f = self.u();
        s.is_active = f & 1 != 0;
        s.is_one_shot = f >> 1 & 1 != 0;
        s.is_subroutine = f >> 2 & 1 != 0;
        s.easy = f >> 3 & 1 != 0;
        s.normal = f >> 4 & 1 != 0;
        s.hard = f >> 5 & 1 != 0;
        s.delay_evaluation_seconds = self.u() as i32;
        s.frame_to_evaluate_at = 0;
        s.cur_time = 0.0;
        let n_or = self.u() % 64;
        s.conditions = (0..n_or)
            .map(|_| {
                let nc = self.u() % 64;
                OrCondition {
                    conditions: (0..nc)
                        .map(|_| {
                            let mut c = Condition::new_for_read();
                            c.condition_type = self.u() as i32;
                            c.custom_data = 0;
                            (c.num_parms, c.parms) = self.parms();
                            c
                        })
                        .collect(),
                }
            })
            .collect();
        s.actions = self.actions();
        s.actions_false = self.actions();
        s
    }
    fn scripts(&mut self) -> Vec<Script> {
        let n = self.u() % 64;
        (0..n).map(|_| self.script()).collect()
    }
}
fn read_sides(w: &[u32]) -> Vec<Option<ScriptList>> {
    let mut r = Rd { w, at: 0 };
    (0..8)
        .map(|_| {
            if r.u() & 1 == 0 {
                return None;
            }
            let mut l = ScriptList::new();
            l.scripts = r.scripts();
            let ng = r.u() % 64;
            l.groups = (0..ng)
                .map(|_| {
                    let group_name = se_str(r.u());
                    let is_group_active = r.u() & 1 != 0;
                    let is_group_subroutine = r.u() & 1 != 0;
                    ScriptGroup { group_name, is_group_active, is_group_subroutine, has_warnings: false, scripts: r.scripts() }
                })
                .collect();
            Some(l)
        })
        .collect()
}

/// The world (the stand-ins' `g_cfg` and state).
struct Sim {
    cfg: [u32; NCFG],
    frame: u32,
    rng: u32,
    log: Vec<String>,
    sides: Vec<Option<ScriptList>>,
    dead: [bool; 16],
    names: Vec<Vec<u8>>,
    colors: Vec<(bool, i32)>,
    groups: Vec<i32>,
    stop: bool,
}

impl Sim {
    fn seed(&self) -> u32 {
        self.cfg[7]
    }
    fn nplayers(&self) -> i32 {
        ((self.cfg[4] & 15) as i32).min(8)
    }
    fn player(&self, i: i32) -> Option<PlayerH> {
        (0..self.nplayers()).contains(&i).then_some(i)
    }
    fn pw(&self, p: PlayerH) -> u32 {
        self.cfg[16 + 2 * p as usize]
    }
    fn team(&self, t: i32) -> Option<TeamH> {
        ((0..8).contains(&t) && self.cfg[40 + t as usize] >> 1 & 1 != 0).then_some(t + 1)
    }
    fn tw(&self, t: TeamH) -> u32 {
        self.cfg[40 + (t - 1) as usize]
    }
    fn obj(&self, o: i32) -> Option<ObjH> {
        ((0..16).contains(&o) && self.cfg[64 + o as usize] & 1 != 0 && !self.dead[o as usize]).then_some(o + 1)
    }
    fn ow(&self, o: ObjH) -> u32 {
        self.cfg[64 + (o - 1) as usize]
    }
    fn ntemplates(&self) -> i32 {
        (self.cfg[11] & 7) as i32 + 1
    }
    fn template_named(&self, name: &[u8]) -> Option<TemplateH> {
        let k = num(name, "tmpl");
        (k >= 0 && k < self.ntemplates()).then_some(k)
    }
}

fn pstr(p: Option<&Parameter>) -> Vec<u8> {
    p.map(|p| p.string.clone()).unwrap_or_default()
}
fn pint(p: Option<&Parameter>) -> i32 {
    p.map_or(0, |p| p.int)
}

impl ScriptEngineHost for Sim {
    fn se_restore_fps_limit(&mut self) {
        self.log.push("fps 30".into());
    }
    fn se_frame(&self) -> u32 {
        self.frame
    }
    fn se_random_int(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let span = (hi as u32).wrapping_sub(lo as u32).wrapping_add(1);
        let v = if hi < lo || span == 0 { lo } else { (lo as u32).wrapping_add((self.rng >> 8) % span) as i32 };
        self.log.push(format!("rand {lo} {hi} {v}"));
        v
    }
    fn se_append_clear_game_data(&mut self) {
        self.log.push("message 7".into());
    }
    fn se_audio_length_ms(&mut self, name: &[u8]) -> f32 {
        (strhash(name) % 4000) as f32 * 0.75
    }
    fn se_is_challenge_campaign(&self) -> bool {
        self.cfg[0] & 1 != 0 && self.cfg[0] & 2 != 0
    }
    fn se_name_key(&mut self, name: &[u8]) -> NameKeyType {
        NameKeyType(strhash(name) & 0x7fff_ffff)
    }
    fn se_actions_reset(&mut self) {
        self.log.push("actions reset".into());
    }
    fn se_conditions_reset(&mut self) {
        self.log.push("conditions reset".into());
    }
    fn se_actions_update(&mut self) {
        self.log.push("actions update".into());
    }
    fn se_conditions_update(&mut self) {
        self.log.push("conditions update".into());
    }
    fn se_actions_close_windows(&mut self) {
        self.log.push("actions closewindows 0".into());
    }
    fn se_execute_action(&mut self, e: &mut ScriptEngine, a: &ScriptAction) -> Result<(), IniError> {
        let mut line = format!("act {}", a.action_type);
        for i in 0..a.num_parms {
            let p = a.get_parameter(i).expect("parameter");
            line += &format!(" {}:{}", p.int, st(&p.string));
        }
        self.log.push(line);
        let p = |i: i32| a.get_parameter(i);
        match a.action_type {
            at::UNIT_EXECUTE_SEQUENTIAL_SCRIPT | at::UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING | at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT | at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING => {
                let team = matches!(a.action_type, at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT | at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING);
                let looping = matches!(a.action_type, at::UNIT_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING | at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING);
                let Some(s) = e.find_script(self, &pstr(p(1))) else { return Ok(()) };
                let mut seq = SequentialScript::new(s);
                if team {
                    let Some(t) = e.get_team_named(self, &pstr(p(0)))? else { return Ok(()) };
                    seq.team_to_exec_on = Some(t);
                } else {
                    let Some(o) = e.get_unit_named(&pstr(p(0))) else { return Ok(()) };
                    seq.object_id = o;
                }
                seq.times_to_loop = if looping { pint(p(2)) } else { 0 };
                e.append_sequential_script(&seq);
            }
            at::UNIT_STOP_SEQUENTIAL_SCRIPT => {
                if let Some(o) = e.get_unit_named(&pstr(p(0))) {
                    e.remove_all_sequential_scripts_of_object(o);
                }
            }
            at::TEAM_STOP_SEQUENTIAL_SCRIPT => {
                if let Some(t) = e.get_team_named(self, &pstr(p(0)))? {
                    e.notify_of_team_destruction(t);
                }
            }
            at::UNIT_GUARD_FOR_FRAMECOUNT => {
                if let Some(o) = e.get_unit_named(&pstr(p(0))) {
                    e.set_sequential_timer_object(o, pint(p(1)));
                }
            }
            at::TEAM_GUARD_FOR_FRAMECOUNT => {
                if let Some(t) = e.get_team_named(self, &pstr(p(0)))? {
                    e.set_sequential_timer_team(t, pint(p(1)));
                }
            }
            at::NAMED_DELETE => {
                if let Some(o) = e.get_unit_named(&pstr(p(0))) {
                    e.notify_of_object_destruction(self, o);
                    self.dead[(o - 1) as usize] = true;
                    self.log.push(format!("obj {o} deleted"));
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn se_evaluate_condition(&mut self, _e: &mut ScriptEngine, c: &mut Condition) -> Result<bool, IniError> {
        let v = hh(self.seed(), (c.condition_type as u32).wrapping_mul(7).wrapping_add(c.custom_data as u32), self.frame) & 1 != 0;
        c.custom_data += 1;
        self.log.push(format!("cond {} {}", c.condition_type, u8::from(v)));
        Ok(v)
    }
    fn se_skirmish_command_button_is_ready(&mut self, _e: &mut ScriptEngine, p1: Option<&Parameter>, p2: Option<&Parameter>, all: bool) -> Result<bool, IniError> {
        let v = hh(self.seed(), 800u32.wrapping_add(pint(p1) as u32).wrapping_add(pint(p2) as u32), self.frame) & 1 != 0;
        self.log.push(format!("buttonready {} {} {} {}", st(&pstr(p1)), st(&pstr(p2)), u8::from(all), u8::from(v)));
        Ok(v)
    }
    fn se_team_is_contained(&mut self, _e: &mut ScriptEngine, p0: Option<&Parameter>, all: bool) -> Result<bool, IniError> {
        let v = hh(self.seed(), 900u32.wrapping_add(pint(p0) as u32), self.frame) & 1 != 0;
        self.log.push(format!("contained {} {} {}", st(&pstr(p0)), u8::from(all), u8::from(v)));
        Ok(v)
    }
    fn se_num_sides(&self) -> i32 {
        ((self.cfg[5] & 15) as i32).min(8)
    }
    fn se_script_list(&self, side: usize) -> Option<&ScriptList> {
        self.sides.get(side)?.as_ref()
    }
    fn se_script_list_mut(&mut self, side: usize) -> Option<&mut ScriptList> {
        self.sides.get_mut(side)?.as_mut()
    }
    fn se_reset_script_lists(&mut self) {
        self.log.push("scriptlist reset".into());
    }
    fn se_nth_player(&self, i: i32) -> Option<PlayerH> {
        self.player(i)
    }
    fn se_player_count(&self) -> i32 {
        self.nplayers()
    }
    fn se_local_player(&self) -> Option<PlayerH> {
        self.player(((self.cfg[4] >> 4) & 15) as i32)
    }
    fn se_find_player_with_name_key(&self, key: NameKeyType) -> Option<PlayerH> {
        (0..self.nplayers()).find(|p| strhash(format!("player{p}").as_bytes()) & 0x7fff_ffff == key.0)
    }
    fn se_update_team_states(&mut self) {
        self.log.push("playerlist updateteamstates".into());
    }
    fn se_player_current_enemy(&self, p: PlayerH) -> Option<PlayerH> {
        self.player(((self.pw(p) >> 12) & 15) as i32)
    }
    fn se_player_is_human(&self, p: PlayerH) -> bool {
        self.pw(p) & 1 != 0
    }
    fn se_player_name_key(&self, p: PlayerH) -> NameKeyType {
        NameKeyType(strhash(format!("player{p}").as_bytes()) & 0x7fff_ffff)
    }
    fn se_player_mp_start_index(&self, p: PlayerH) -> i32 {
        ((self.pw(p) >> 8) & 15) as i32
    }
    fn se_player_mask(&self, p: PlayerH) -> u32 {
        1 << p
    }
    fn se_player_difficulty(&self, p: PlayerH) -> i32 {
        (((self.pw(p) >> 1) & 3) % 3) as i32
    }
    fn se_player_is_active(&self, p: PlayerH) -> bool {
        (self.pw(p) >> 4) & 1 != 0
    }
    fn se_player_is_skirmish_ai(&self, p: PlayerH) -> bool {
        (self.pw(p) >> 5) & 1 != 0
    }
    fn se_player_default_team(&self, p: PlayerH) -> Option<TeamH> {
        self.team(((self.pw(p) >> 16) & 15) as i32)
    }
    fn se_find_team_prototype(&self, name: &[u8]) -> Option<ProtoH> {
        let k = num(name, "team");
        ((0..8).contains(&k) && self.cfg[32 + k as usize] >> 31 & 1 != 0).then_some(k)
    }
    fn se_proto_is_singleton(&self, p: ProtoH) -> bool {
        self.cfg[32 + p as usize] & 1 != 0
    }
    fn se_proto_max_instances(&self, p: ProtoH) -> i32 {
        ((self.cfg[32 + p as usize] >> 1) & 7) as i32
    }
    fn se_proto_instances(&self, p: ProtoH) -> Vec<TeamH> {
        (0..8).filter_map(|t| self.team(t)).filter(|&t| ((self.tw(t) >> 8) & 7) as i32 == p).collect()
    }
    fn se_team_is_active(&self, t: TeamH) -> bool {
        self.tw(t) & 1 != 0
    }
    fn se_team_name(&self, t: TeamH) -> Vec<u8> {
        format!("team{}", (self.tw(t) >> 8) & 7).into_bytes()
    }
    fn se_team_controlling_player(&self, t: TeamH) -> Option<PlayerH> {
        self.player(((self.tw(t) >> 4) & 15) as i32)
    }
    fn se_team_as_new_ai_group(&mut self, t: TeamH) -> GroupH {
        self.groups.push(t);
        let g = self.groups.len() as i32;
        self.log.push(format!("ai creategroup {g}"));
        self.log.push(format!("team {t} asgroup {g}"));
        g
    }
    fn se_group_is_idle(&self, g: GroupH) -> bool {
        hh(self.seed(), 300 + self.groups[(g - 1) as usize] as u32, self.frame) & 1 != 0
    }
    fn se_group_is_ai_dead(&self, g: GroupH) -> bool {
        hh(self.seed(), 400 + self.groups[(g - 1) as usize] as u32, self.frame) & 7 == 0
    }
    fn se_find_object(&self, id: ObjectId) -> Option<ObjH> {
        self.obj(id - 1)
    }
    fn se_objects(&self) -> Vec<ObjH> {
        (0..16).filter_map(|o| self.obj(o)).collect()
    }
    fn se_obj_name(&self, o: ObjH) -> Vec<u8> {
        self.names[(o - 1) as usize].clone()
    }
    fn se_obj_set_name(&mut self, o: ObjH, name: &[u8]) {
        self.names[(o - 1) as usize] = name.to_vec();
        self.log.push(format!("obj {o} name {}", st(name)));
    }
    fn se_obj_has_custom_indicator_color(&self, o: ObjH) -> bool {
        self.colors[(o - 1) as usize].0
    }
    fn se_obj_indicator_color(&self, o: ObjH) -> i32 {
        self.colors[(o - 1) as usize].1
    }
    fn se_obj_set_custom_indicator_color(&mut self, o: ObjH, color: i32) {
        self.colors[(o - 1) as usize] = (true, color);
        self.log.push(format!("obj {o} color {color}"));
    }
    fn se_obj_remove_custom_indicator_color(&mut self, o: ObjH) {
        self.colors[(o - 1) as usize].0 = false;
        self.log.push(format!("obj {o} nocolor"));
    }
    fn se_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH> {
        self.player(((self.ow(o) >> 4) & 15) as i32)
    }
    fn se_obj_has_ai(&self, o: ObjH) -> bool {
        (self.ow(o) >> 1) & 1 != 0
    }
    fn se_ai_is_idle(&self, o: ObjH) -> bool {
        hh(self.seed(), 200 + o as u32, self.frame) & 1 != 0
    }
    fn se_obj_is_effectively_dead(&self, o: ObjH) -> bool {
        hh(self.seed(), 600 + o as u32, self.frame) & 7 == 0
    }
    fn se_trigger_area_by_name(&self, name: &[u8]) -> Option<TriggerH> {
        let h = strhash(name) ^ self.seed();
        (!h.is_multiple_of(3)).then_some((h & 0xffff) as i32)
    }
    fn se_waypoint_location(&self, name: &[u8]) -> Option<Coord3D> {
        let k = num(name, "wp");
        if !(0..8).contains(&k) || (self.cfg[12] >> k) & 1 == 0 {
            return None;
        }
        Some(Coord3D { x: (hh(self.seed(), 700, k as u32) % 1000) as f32 * 0.5, y: (hh(self.seed(), 701, k as u32) % 1000) as f32 * 0.5, z: 0.0 })
    }
    fn se_do_shroud_reveal(&mut self, x: f32, y: f32, r: f32, mask: u32) {
        self.log.push(format!("reveal {} {} {} {mask}", fb(x), fb(y), fb(r)));
    }
    fn se_undo_shroud_reveal(&mut self, x: f32, y: f32, r: f32, mask: u32) {
        self.log.push(format!("unreveal {} {} {} {mask}", fb(x), fb(y), fb(r)));
    }
    fn se_find_template(&self, name: &[u8]) -> Option<TemplateH> {
        self.template_named(name)
    }
    fn se_templates(&self) -> Vec<TemplateH> {
        (0..self.ntemplates()).collect()
    }
    fn se_template_is_kind_of(&self, t: TemplateH, kind: i32) -> Result<bool, IniError> {
        Ok(hh(self.seed(), 500 + t as u32, kind as u32) & 1 != 0)
    }
    fn se_template_final_override(&self, t: TemplateH) -> TemplateH {
        (t * 5 + 1) % self.ntemplates()
    }
    fn se_team_exists(&self, id: TeamH) -> bool {
        self.team(id - 1).is_some()
    }
    fn se_template_name(&self, t: TemplateH) -> Vec<u8> {
        format!("tmpl{t}").into_bytes()
    }
    fn se_science_name(&self, s: i32) -> Vec<u8> {
        format!("sci{s}").into_bytes()
    }
    fn se_science_from_name(&self, name: &[u8]) -> Option<i32> {
        let k = num(name, "sci");
        (0..8).contains(&k).then_some(k)
    }
    fn se_actions_enable_difficulty_bonuses(&mut self, enable: bool) {
        self.log.push(format!("actions bonuses {}", u8::from(enable)));
    }
    fn se_audio_add_event(&mut self, name: &[u8], player: i32) {
        self.log.push(format!("audio add {} {player}", st(name)));
    }
}

/// `dumpState`.
fn dump(e: &ScriptEngine, h: &Sim) -> String {
    let mut s = format!("state c{}", e.num_counters);
    for c in &e.counters[..e.num_counters.max(0) as usize] {
        s += &format!(" {}={}{}", st(&c.name), c.value, if c.is_countdown_timer { "t" } else { "" });
    }
    s += &format!(" f{}", e.num_flags);
    for f in &e.flags[..e.num_flags.max(0) as usize] {
        s += &format!(" {}={}", st(&f.name), u8::from(f.value));
    }
    s += &format!(" p{}", e.num_attack_info);
    for a in &e.attack_priority_info[..e.num_attack_info.max(0) as usize] {
        s += &format!(" {}/{}", st(&a.name), a.default_priority);
        for (t, p) in a.priority_map.iter().flatten() {
            s += &format!(",{t}:{p}");
        }
    }
    let id = |x: Option<i32>| x.unwrap_or(0);
    s += &format!(
        " timers {} {} who {} {} {} {} {} first {}",
        e.end_game_timer,
        e.close_window_timer,
        id(e.calling_team),
        id(e.calling_object),
        id(e.condition_team),
        id(e.condition_object),
        e.current_player.map_or(0, |p| p + 1),
        u8::from(e.first_update)
    );
    s += &format!(
        " fade {} {} {} {} {} {} {} {}",
        e.fade,
        fb(e.min_fade),
        fb(e.max_fade),
        fb(e.cur_fade_value),
        e.cur_fade_frame,
        e.fade_frames_increase,
        e.fade_frames_hold,
        e.fade_frames_decrease
    );
    let b = &e.breeze_info;
    s += &format!(
        " breeze {} {} {} {} {} {} {} {}",
        fb(b.direction),
        fb(b.direction_vec.x),
        fb(b.direction_vec.y),
        fb(b.intensity),
        fb(b.lean),
        fb(b.randomness),
        b.breeze_period,
        b.breeze_version
    );
    s += &format!(" track {}", st(&e.current_track_name));
    s += &format!(
        " changed {} diff {} freeze {} bonus {} victim {} mpdefeat {}",
        e.frame_object_count_changed,
        e.game_difficulty,
        u8::from(e.freeze_by_script),
        u8::from(e.objects_should_receive_difficulty_bonus),
        u8::from(e.choose_victim_always_uses_normal),
        u8::from(e.shown_mp_local_defeat_window)
    );
    s += " names";
    for (n, o) in &e.named_objects {
        s += &format!(" {}:{}", st(n), id(*o));
    }
    s += " notes";
    for v in &e.completed_video {
        s += &format!(" v:{}", st(v));
    }
    for (n, f) in &e.testing_speech {
        s += &format!(" s:{}:{f}", st(n));
    }
    for (n, f) in &e.testing_audio {
        s += &format!(" a:{}:{f}", st(n));
    }
    for u in &e.ui_interactions {
        s += &format!(" u:{}", st(u));
    }
    for (what, l) in [("t", &e.triggered_special_powers), ("m", &e.midway_special_powers), ("d", &e.finished_special_powers), ("g", &e.completed_upgrades)] {
        for (p, list) in l.iter().enumerate() {
            for (n, src) in list {
                s += &format!(" {what}{p}:{}:{src}", st(n));
            }
        }
    }
    for (p, l) in e.acquired_sciences.iter().enumerate() {
        for sc in l {
            s += &format!(" sci{p}:{sc}");
        }
    }
    for (n, d) in &e.topple_directions {
        s += &format!(" top:{}:{}:{}:{}", st(n), fb(d.x), fb(d.y), fb(d.z));
    }
    for r in &e.named_reveals {
        s += &format!(" rv:{}:{}:{}:{}", st(&r.reveal_name), st(&r.waypoint_name), fb(r.radius_to_reveal), st(&r.player_name));
    }
    for t in &e.all_object_type_lists {
        s += &format!(" ot:{}", st(t.list_name().str()));
        for k in 0..t.list_size() as i32 {
            s += &format!(",{}", st(t.get_nth_in_list(k).str()));
        }
    }
    for (p, m) in e.object_counts.iter().enumerate() {
        for (n, c) in m {
            s += &format!(" n{p}:{}:{c}", st(n));
        }
    }
    s += " seq";
    for chain in &e.sequential_scripts {
        s += " {";
        for q in chain {
            let name = ScriptEngine::script(h, q.script).map(|s| st(&s.script_name)).unwrap_or_default();
            s += &format!(" {}/{}/{name}/{}/{}/{}/{}", id(q.team_to_exec_on), q.object_id, q.current_instruction, q.times_to_loop, q.frames_to_wait, u8::from(q.dont_advance_instruction));
        }
        s += " }";
    }
    s += " scripts";
    fn script(s: &mut String, sc: &Script) {
        *s += &format!(" [{} {} {} {} {}", st(&sc.script_name), u8::from(sc.is_active), sc.frame_to_evaluate_at, st(&sc.condition_team_name), fb(sc.cur_time));
        let ints = |n: i32, parms: &[Option<Parameter>]| parms[..n as usize].iter().map(|p| format!(" {}", p.as_ref().expect("parameter").int)).collect::<String>();
        for or in &sc.conditions {
            for c in &or.conditions {
                *s += &format!(" c{}{}", c.custom_data, ints(c.num_parms, &c.parms));
            }
        }
        for a in &sc.actions {
            *s += &format!(" a{}", ints(a.num_parms, &a.parms));
        }
        for a in &sc.actions_false {
            *s += &format!(" f{}", ints(a.num_parms, &a.parms));
        }
        *s += "]";
    }
    for (i, l) in h.sides.iter().enumerate() {
        let Some(l) = l else { continue };
        s += &format!(" side{i}");
        for sc in &l.scripts {
            script(&mut s, sc);
        }
        for g in &l.groups {
            s += &format!(" g{}:{}", st(&g.group_name), u8::from(g.is_group_active));
            for sc in &g.scripts {
                script(&mut s, sc);
            }
        }
    }
    s
}

/// One operation, as `runOp`.
fn run_op(e: &mut ScriptEngine, h: &mut Sim, op: [u32; 3]) -> Result<(), IniError> {
    let (a, b) = (op[1], op[2]);
    match op[0] {
        0 => e.update(h)?,
        1 => h.frame += a % 7,
        2 => h.cfg[(a % NCFG as u32) as usize] = b,
        3 => {
            let t = h.team(b as i32 - 1);
            e.run_script(h, &se_str(a), t)?
        }
        4 => {
            let o = h.obj(b as i32 - 1);
            e.run_object_script(h, &se_str(a), o)?
        }
        5 => {
            let player = ((b >> 8) & 15) as i32 + if (b >> 30) & 1 != 0 { 16 } else { 0 };
            let name = se_str(1000 + (b & 7));
            let src = ((b >> 16) & 3) as ObjectId;
            match a % 8 {
                0 => e.notify_of_completed_video(&name),
                k @ 1..=4 => e.notify_of_power((k - 1) as usize, player, &name, src)?,
                5 => e.notify_of_acquired_science(player, (b & 7) as i32)?,
                6 => e.signal_ui_interact(&se_str(500 + (b & 7))),
                _ => e.notify_of_object_creation_or_destruction(h),
            }
        }
        6 => {
            let player = ((b >> 8) & 15) as i32 - if (b >> 30) & 1 != 0 { 3 } else { 0 };
            let name = se_str(1000 + (b & 7));
            let rm = (b >> 4) & 1 != 0;
            let src = ((b >> 16) & 3) as ObjectId;
            let v = match a % 10 {
                0 => e.is_video_complete(&name, rm),
                1 => e.is_sound_complete(h, &name, rm, false),
                2 => e.is_sound_complete(h, &name, rm, true),
                k @ 3..=6 => e.is_power_done((k - 3) as usize, player, &name, rm, src),
                7 => e.is_science_acquired(player, (b & 7) as i32, rm),
                8 => e.get_unit_named(&se_str(b & 0xfff)).is_some(),
                _ => e.did_unit_exist(&se_str(b & 0xfff)),
            };
            h.log.push(format!("query {} {}", a % 10, u8::from(v)));
        }
        7 => {
            let o = h.obj((b % 16) as i32);
            match a % 5 {
                0 => {
                    if let Some(o) = o {
                        e.add_object_to_cache(h, o);
                    }
                }
                1 => {
                    if let Some(o) = o {
                        e.remove_object_from_cache(o);
                    }
                }
                2 => {
                    if let Some(o) = o {
                        e.transfer_object_name(h, &se_str((b >> 8) & 0xfff), o);
                    }
                }
                3 => {
                    if let Some(o) = o {
                        e.notify_of_object_destruction(h, o);
                    }
                }
                _ => {
                    if let Some(o) = o {
                        h.se_obj_set_name(o, &se_str((b >> 8) & 0xfff));
                    }
                }
            }
        }
        8 => {
            let name = se_str(800 + ((a >> 4) & 3));
            match a % 4 {
                0 => {
                    let r = f32::from_bits((b & 0xff) << 20 | 0x4000_0000);
                    e.create_named_map_reveal(&name, &se_str(700 + ((b >> 8) & 15)), r, &se_str((b >> 12) & 0x3ff));
                }
                1 => e.do_named_map_reveal(h, &name),
                2 => e.undo_named_map_reveal(h, &name),
                _ => e.remove_named_map_reveal(&name),
            }
        }
        9 => {
            if a % 2 == 0 {
                let d = Coord3D { x: (b & 15) as i32 as f32 - 7.0, y: ((b >> 4) & 15) as i32 as f32 - 7.0, z: ((b >> 8) & 3) as f32 };
                e.set_topple_direction(&se_str(300 + ((a >> 4) & 15)), if (b >> 12) & 1 != 0 { None } else { Some(d) })?;
            } else if let Some(o) = h.obj((b % 16) as i32) {
                if (b >> 4) & 1 != 0 {
                    let mut d2 = rts_lockstep_engine::common::base_type::Coord2D { x: 3.0, y: -1.0 };
                    e.adjust_topple_direction_2d(h, o, &mut d2);
                    h.log.push(format!("topple2 {} {}", fb(d2.x), fb(d2.y)));
                } else {
                    let mut d = Coord3D { x: 1.0, y: 2.0, z: 0.5 };
                    e.adjust_topple_direction(h, o, &mut d);
                    h.log.push(format!("topple {} {} {}", fb(d.x), fb(d.y), fb(d.z)));
                }
            }
        }
        10 => e.do_object_type_list_maintenance(&se_str(800 + (a & 3)), &se_str(600 + (b & 7)), (b >> 3) & 1 != 0),
        11 => {
            let name = se_str(b & 0xfff);
            let line = match a % 8 {
                0 => format!("teamnamed {}", e.get_team_named(h, &name)?.unwrap_or(0)),
                1 => format!("playerfrom {}", e.get_player_from_ascii_string(h, &name).map_or(0, |p| p + 1)),
                2 => format!("trigger {}", e.get_qualified_trigger_area_by_name(h, &name).unwrap_or(-1)),
                3 => format!("enemy {}", e.get_skirmish_enemy_player(h).map_or(0, |p| p + 1)),
                4 => format!("current {}", e.get_current_player().map_or(0, |p| p + 1)),
                5 => format!("counter {}", e.get_counter(&name).map_or(-99999, |c| c.value)),
                6 => {
                    let i = e.get_attack_info(&name);
                    let t = h.template_named(&se_str(600 + ((b >> 12) & 7))).map(|t| h.se_template_final_override(t));
                    format!("attack {} {}", st(&i.name), i.get_priority(t))
                }
                _ => format!("objecttypes {}", e.get_object_types(&name).map_or(-1, |i| e.all_object_type_lists[i].list_size() as i32)),
            };
            h.log.push(line);
        }
        12 => e.reset(h),
        13 => e.new_map(h)?,
        14 => {
            if let Some(r) = e.find_script(h, &se_str(a)) {
                let t = h.team((b >> 1) as i32 - 1);
                e.friend_execute_action(h, r, b & 1 == 0, t)?;
            }
        }
        15 => {
            if let Some(r) = e.find_script(h, &se_str(a)) {
                let t = h.team((b & 15) as i32 - 1);
                let p = h.player(((b >> 4) & 15) as i32 - 1);
                let v = e.evaluate_conditions(h, r, t, p)?;
                h.log.push(format!("evaluate {}", u8::from(v)));
            }
        }
        16 => match a % 10 {
            0 => e.start_end_game_timer(),
            1 => e.start_quick_end_game_timer(),
            2 => e.start_close_window_timer(),
            3 => e.do_freeze_time(),
            4 => e.do_unfreeze_time(),
            5 => e.set_global_difficulty((b % 3) as i32),
            6 => e.clear_team_flags(),
            7 => e.clear_flag(&se_str(b & 0xfff)),
            8 => e.mark_mp_local_defeat_window_shown(),
            _ => e.current_track_name = se_str(1000 + (b & 7)),
        },
        17 => {
            let (p, name) = ((b & 7) as i32, se_str(600 + ((b >> 4) & 7)));
            if a & 1 != 0 {
                e.set_object_count(p, &name, ((b >> 8) & 0xff) as i32 - 100)?;
            } else {
                let c = e.get_object_count(h, p, &name)?;
                h.log.push(format!("objectcount {c}"));
            }
        }
        18 => {
            if let Some(r) = e.find_script(h, &se_str(a)) {
                let mut seq = SequentialScript::new(r);
                if b & 1 != 0 {
                    seq.team_to_exec_on = h.team(((b >> 1) & 7) as i32);
                } else {
                    seq.object_id = ((b >> 1) & 15) as i32 + 1;
                }
                seq.times_to_loop = ((b >> 8) & 3) as i32 - 1;
                seq.frames_to_wait = ((b >> 12) & 3) as i32 - 1;
                e.append_sequential_script(&seq);
            }
        }
        19 => match a % 5 {
            0 => {
                if let Some(t) = h.team((b & 7) as i32) {
                    e.notify_of_team_destruction(t);
                }
            }
            1 => {
                if let Some(o) = h.obj((b & 15) as i32) {
                    e.remove_all_sequential_scripts_of_object(o);
                }
            }
            2 => {
                if let Some(o) = h.obj((b & 15) as i32) {
                    e.set_sequential_timer_object(o, ((b >> 8) & 7) as i32 - 1);
                }
            }
            3 => {
                if let Some(t) = h.team((b & 7) as i32) {
                    e.set_sequential_timer_team(t, ((b >> 8) & 7) as i32 - 1);
                }
            }
            _ => e.create_named_cache(h),
        },
        21 => {
            let mut x = XferSave::new();
            x.open(b"engine").expect("open");
            e.xfer(h, &mut x).expect("a save does not fail");
            let hex: String = x.bytes().iter().map(|b| format!("{b:02x}")).collect();
            h.log.push(format!("save {hex}"));
        }
        22 => {
            let mut x = XferSave::new();
            x.open(b"engine").expect("open");
            e.xfer(h, &mut x).expect("a save does not fail");
            let mut bytes = x.bytes().to_vec();
            if a & 1 != 0 && !bytes.is_empty() {
                let n = bytes.len();
                bytes[b as usize % n] ^= ((b >> 8) | 1) as u8;
            }
            if a & 2 != 0 {
                bytes.truncate(bytes.len() * (b & 7) as usize / 8);
            }
            e.reset(h);
            let mut l = XferLoad::new(bytes);
            l.open(b"engine").expect("open");
            match e.xfer(h, &mut l) {
                Ok(()) => e.load_post_process(h)?,
                Err(s) if s == LOAD_UNDEFINED => return Err(IniError::undefined(Undefined::NullObject)),
                Err(_s) => {
                    if std::env::var("ENGINE_DEBUG").is_ok() {
                        eprintln!("load failed: {_s:?} at {:?}", l.position());
                    }
                    h.log.push("load failed".into());
                    h.stop = true;
                }
            }
        }
        _ => {
            if let Some(r) = e.find_script(h, &se_str(a)) {
                e.execute_script(h, r)?;
            }
        }
    }
    Ok(())
}

/// The Rust run: the log, and the operation found undefined (with why).
fn rust_run(cfg: &[u32; NCFG], scripts: &[u32], ops: &[[u32; 3]]) -> (String, Option<(usize, String)>) {
    let mut h = Sim {
        cfg: *cfg,
        frame: 0,
        rng: cfg[7],
        log: Vec::new(),
        sides: read_sides(scripts),
        dead: [false; 16],
        names: (0..16).map(|o| if (cfg[64 + o] >> 12) & 1 != 0 { format!("unit{}", (cfg[64 + o] >> 8) & 15).into_bytes() } else { Vec::new() }).collect(),
        colors: (0..16).map(|o| (hh(cfg[7], 100, o) & 1 != 0, (hh(cfg[7], 101, o) & 0xff_ffff) as i32)).collect(),
        groups: Vec::new(),
        stop: false,
    };
    let mut e = ScriptEngine::new();
    // init: TheScriptActions->init(), TheScriptConditions->init(), the templates (batch E1), reset()
    h.log.push("actions init".into());
    h.log.push("conditions init".into());
    e.reset(&mut h);
    let d = dump(&e, &h);
    h.log.push(d);
    let mut undefined_at = None;
    for (i, op) in ops.iter().enumerate() {
        let before = h.log.len();
        if let Err(err) = run_op(&mut e, &mut h, *op) {
            h.log.truncate(before);
            undefined_at = Some((i, format!("op {} {:?}", op[0], err)));
            break;
        }
        if h.stop {
            break;
        }
        let d = dump(&e, &h);
        h.log.push(d);
    }
    let mut s = String::new();
    for l in &h.log {
        s += l;
        s.push('\n');
    }
    (s, undefined_at)
}

fn cpp_run(cfg: &[u32; NCFG], scripts: &[u32], ops: &[[u32; 3]]) -> String {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut cap = 1 << 16;
    loop {
        let mut out = vec![0u8; cap];
        let n = unsafe { cpp::ref_engine_run(cfg.as_ptr(), scripts.as_ptr(), scripts.len() as i32, flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), cap as i32) } as usize;
        if n <= cap {
            out.truncate(n);
            return String::from_utf8_lossy(&out).into_owned();
        }
        cap = n;
    }
}

/// The engine's own condition and action types, and the stand-ins' active ones.
const ENGINE_CONDITIONS: [i32; 5] = [ct::CONDITION_FALSE, ct::CONDITION_TRUE, ct::COUNTER, ct::FLAG, ct::TIMER_EXPIRED];
const ENGINE_ACTIONS: [i32; 31] = [
    at::SET_COUNTER,
    at::SET_TREE_SWAY,
    at::INCREMENT_COUNTER,
    at::DECREMENT_COUNTER,
    at::SET_FLAG,
    at::STOP_TIMER,
    at::RESTART_TIMER,
    at::SET_TIMER,
    at::SET_MILLISECOND_TIMER,
    at::SET_RANDOM_TIMER,
    at::SET_RANDOM_MSEC_TIMER,
    at::ADD_TO_MSEC_TIMER,
    at::SUB_FROM_MSEC_TIMER,
    at::ENABLE_SCRIPT,
    at::DISABLE_SCRIPT,
    at::CALL_SUBROUTINE,
    at::CAMERA_FADE_ADD,
    at::CAMERA_FADE_SUBTRACT,
    at::CAMERA_FADE_SATURATE,
    at::CAMERA_FADE_MULTIPLY,
    at::SET_ATTACK_PRIORITY_THING,
    at::SET_ATTACK_PRIORITY_KIND_OF,
    at::SET_DEFAULT_ATTACK_PRIORITY,
    at::NO_OP,
    at::UNIT_EXECUTE_SEQUENTIAL_SCRIPT,
    at::TEAM_EXECUTE_SEQUENTIAL_SCRIPT_LOOPING,
    at::UNIT_STOP_SEQUENTIAL_SCRIPT,
    at::TEAM_STOP_SEQUENTIAL_SCRIPT,
    at::UNIT_GUARD_FOR_FRAMECOUNT,
    at::NAMED_DELETE,
    at::SKIRMISH_WAIT_FOR_COMMANDBUTTON_AVAILABLE_ALL,
];

fn gen_string(g: &mut Gen) -> u32 {
    match g.below(12) {
        0 => g.below(SPECIAL.len() as u32),
        1..=3 => 100 + g.below(6),
        4 | 5 => 200 + g.below(8),
        6 | 7 => 300 + g.below(10),
        8 => 400 + g.below(6),
        9 => 500 + g.below(6),
        10 => 600 + g.below(8),
        _ => [700 + g.below(8), 800 + g.below(4), 1000 + g.below(8), 900 + g.below(100)][g.below(4) as usize],
    }
}

fn gen_parms(g: &mut Gen, w: &mut Vec<u32>, n: u32) {
    w.push(n);
    for _ in 0..n {
        w.push(if g.below(5) == 0 { pt::TEAM as u32 } else { g.below(12) });
        w.push(match g.below(16) {
            0..=7 => 0,
            8 | 9 => g.below(8),
            10 | 11 => g.below(5),
            12 => g.below(300),
            13 | 14 => g.below(1000).wrapping_sub(500),
            _ => g.u32(),
        });
        w.push([0.0f32, 0.5, 1.0, 2.5, -1.0, 100.0, 1.0e10, 0.033][g.below(8) as usize].to_bits());
        w.push(gen_string(g));
    }
}

fn gen_script(g: &mut Gen, w: &mut Vec<u32>) {
    w.push(100 + g.below(8));
    w.push(g.u32() | if g.below(3) != 0 { 0x39 } else { 0 });
    w.push(if g.below(3) == 0 { g.below(4) } else { 0 });
    let n_or = g.below(3);
    w.push(n_or);
    for _ in 0..n_or {
        let nc = g.below(3);
        w.push(nc);
        for _ in 0..nc {
            w.push(if g.below(2) == 0 { ENGINE_CONDITIONS[g.below(5) as usize] as u32 } else { 5 + g.below(104) });
            let n = if g.below(10) == 0 { g.below(3) } else { 3 };
            gen_parms(g, w, n);
        }
    }
    for _ in 0..2 {
        let na = g.below(4);
        w.push(na);
        for _ in 0..na {
            w.push(if g.below(4) != 0 { ENGINE_ACTIONS[g.below(ENGINE_ACTIONS.len() as u32) as usize] as u32 } else { g.below(344) });
            let n = if g.below(10) == 0 { g.below(5) } else { 5 };
            gen_parms(g, w, n);
        }
    }
}

fn gen_program(g: &mut Gen) -> ([u32; NCFG], Vec<u32>, Vec<[u32; 3]>) {
    let mut cfg = [0u32; NCFG];
    for w in cfg.iter_mut() {
        *w = g.u32();
    }
    cfg[0] &= 3;
    cfg[4] = (2 + g.below(7)) | g.below(9) << 4;
    cfg[5] = 1 + g.below(4);
    cfg[11] = g.below(8);
    for p in 0..8 {
        cfg[16 + 2 * p] = (cfg[16 + 2 * p] & 0x000f_ff3f) | g.below(9) << 12 | g.below(9) << 16;
    }
    for k in 0..8 {
        cfg[32 + k] = (cfg[32 + k] & 0xf) | u32::from(g.below(4) != 0) << 31;
    }
    for t in 0..8 {
        cfg[40 + t] = (cfg[40 + t] & 0x7f3) | u32::from(g.below(5) != 0) << 1 | g.below(8) << 4 | g.below(4) << 8;
    }
    for o in 0..16 {
        cfg[64 + o] = (cfg[64 + o] & 0x7_0ff3) | u32::from(g.below(4) != 0) | u32::from(g.below(3) != 0) << 12 | g.below(10) << 8;
    }
    let mut w = Vec::new();
    for _ in 0..8 {
        if g.below(4) == 0 {
            w.push(0);
            continue;
        }
        w.push(1);
        let ns = g.below(4);
        w.push(ns);
        for _ in 0..ns {
            gen_script(g, &mut w);
        }
        let ng = g.below(3);
        w.push(ng);
        for _ in 0..ng {
            w.push(100 + g.below(8));
            w.push(g.below(4).min(1));
            w.push(g.below(2));
            let ns = g.below(3);
            w.push(ns);
            for _ in 0..ns {
                gen_script(g, &mut w);
            }
        }
    }
    let n = 1 + g.below(40);
    let mut ops = Vec::new();
    if g.below(2) == 0 {
        ops.push([13, 0, 0]);
    }
    for _ in 0..n {
        let k = match g.below(32) {
            0..=6 => 0,
            7..=9 => 1,
            k => k - 8,
        };
        let a = if matches!(k, 3 | 4 | 14 | 15 | 18 | 20 | 21) { 100 + g.below(8) } else { g.u32() };
        let b = if matches!(k, 3 | 4 | 14 | 15) { g.below(10) } else if k == 2 { [g.u32(), g.u32() & 0x8000_0003][g.below(2) as usize] } else { g.u32() & 0x4fff_ffff };
        let mut b = b;
        if g.below(8) != 0 {
            b &= !(1 << 30);
        }
        if k == 9 && g.below(4) != 0 {
            b &= !(1 << 12);
        }
        if k == 17 && g.below(4) != 0 {
            b &= !6;
        }
        // names that exist, more often than chance gives them
        match k {
            5 | 6 if g.below(2) == 0 => b = (b & !0x0f07 & !(3 << 16)) | g.below(4) << 8 | g.below(2) | g.below(3) << 16,
            7 if g.below(2) == 0 => b = (b & !(0xfff << 8)) | (300 + g.below(10)) << 8,
            8 if g.below(2) == 0 => b = (b & !(0x3ff << 12) & !(0xf << 8)) | [400 + g.below(6), 4, 5, 8][g.below(4) as usize] << 12 | g.below(8) << 8,
            11 if g.below(2) == 0 => b = (b & !0xfff) | gen_string(g),
            _ => {}
        }
        let a = if k == 2 { [0, 4, 5, 11, 12, 32 + g.below(8), 40 + g.below(8), 64 + g.below(16)][g.below(8) as usize] } else { a };
        ops.push([k, a, b]);
    }
    (cfg, w, ops)
}

#[test]
fn script_engine_matches_upstream() {
    // subroutines that call each other without end go MAX_CALL_DEPTH deep before the Rust side calls it a stack overflow, and
    // the C++ runs the same calls up to there: a thread with a stack for that
    std::thread::Builder::new().stack_size(1 << 30).spawn(run).expect("thread").join().expect("the test");
}

fn run() {
    let cases: usize = std::env::var("ENGINE_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x5E_0002);
    let (mut mismatches, mut cut, mut ops_run, mut lines) = (0usize, 0usize, 0usize, 0usize);
    let mut cut_why: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_op = [0usize; 24];
    for case in 0..cases {
        let (cfg, scripts, ops) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &scripts, &ops);
        let n = undefined_at.as_ref().map_or(ops.len(), |u| u.0);
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            let variant = why.rsplit("Undefined(").next().and_then(|r| r.split(')').next()).unwrap_or(why);
            *cut_why.entry(format!("{variant} ({})", why.split(' ').take(2).collect::<Vec<_>>().join(" "))).or_insert(0) += 1;
        }
        if std::env::var("ENGINE_TRACE").is_ok() {
            eprintln!("case {case}: {:?}", &ops[..n]);
        }
        let c = cpp_run(&cfg, &scripts, &ops[..n]);
        ops_run += n;
        for o in &ops[..n] {
            by_op[o[0] as usize] += 1;
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let (rl, cl): (Vec<&str>, Vec<&str>) = (r.lines().collect(), c.lines().collect());
                let at = rl.iter().zip(&cl).position(|(a, b)| a != b).unwrap_or(rl.len().min(cl.len()));
                println!("case {case}: ops {:?}\n  first difference at line {at}:\n  rust {:?}\n  c++  {:?}", &ops[..n], rl.get(at), cl.get(at));
            }
        } else {
            lines += r.lines().count();
        }
    }
    println!("E2: {cases} programs ({cut} cut at an undefined operation), {ops_run} operations, {lines} lines, {mismatches} mismatches; by kind {by_op:?}");
    println!("cut at: {cut_why:?}");
    assert_eq!(mismatches, 0);
}
