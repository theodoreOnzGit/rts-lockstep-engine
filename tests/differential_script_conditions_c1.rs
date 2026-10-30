#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/ScriptEngine/ScriptConditions.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ScriptConditions` (`logic::world::script_conditions`) against the C++, `p1w13-sonnet`, layer 11 batch C1.
//!
//! # Method
//!
//! The C++ side is `ScriptConditions.cpp` and its header, compiled by `differential/wrap_conditions.cpp` on the stand-ins of
//! `differential/shim_conditions` (`sc_world.h`). The file is build.rs's copy with one mechanical edit (`typename` before
//! `std::vector<T>::iterator` in `rts::sum`). The stand-ins answer from a configuration of 160 words: up to eight players (their
//! objects, build facilities, money, buildings, power, sciences, side, supplies, start position, attackers), eight team prototypes
//! (owner, instances, the instance `getTeamNamed` gives) and sixteen teams (objects, units, created, entered or exited, state, owner,
//! members), sixteen objects (named or not, dead, body and health, last damage and its source, owner, contain, AI and completed
//! waypoint, status, held, unmanned, disabled, captured, kinds, position), eight trigger areas, the templates, special powers, sciences,
//! command buttons and object-type lists the stores know, the selection, the frames, the camera and the victory flags; the rest (inside
//! an area, entered, shroud, special power modules, the partition's searches, the script engine's completion queries) is a hash of a
//! seed and the question. The script engine's completion queries, the object counts set, and the player's supply questions write a line.
//!
//! The Rust side is the module on [`ScriptConditionsHost`], implemented here on the same words with the same functions.
//!
//! A program is the configuration, up to eight conditions of random types (every type, each with the parameters its evaluation reads,
//! sometimes too few), and 1 to 60 operations: evaluate a condition, change a configuration word (the frames, a bit of any word), or
//! `init`. Compared after each: the result, the lines, the condition's custom data and frame and every parameter's `Int` (the cached
//! player masks and the special power's next frame), and the transports `UNIT_EMPTIED` has seen. When the Rust side finds upstream
//! undefined at an operation, the C++ runs only the operations before it, and that prefix is compared.
//!
//! # Results (2026-10-30, `CONDITIONS_CASES=20000`, the default)
//!
//! 20,000 programs, 567,137 operations compared (379,813 evaluations, every one of the 109 condition types evaluated; 3,752 direct
//! `evaluateTeamIsContained` and 500 `evaluateSkirmishCommandButtonIsReady` calls), 3,500,775 output words: **0 mismatches**. 1,857
//! programs were cut at an operation upstream does not define, and their prefix compared: 1,784 a condition with too few parameters
//! (NULL dereferenced; the generator truncates 1 condition in 30), 41 `TYPE_SIGHTED` seeing an object of the player with no template,
//! 16 `NAMED_SELECTED` with a selected drawable that has no object, 14 `ENEMY_SIGHTED` with a relation that is none of the three
//! (the filter's uninitialised `Int`), 2 `UNIT_HEALTH` of a unit whose initial health is 0.
//!
//! The answers the stand-ins give are their own (a hash where the real answer belongs to another module); what is verified is
//! `ScriptConditions.cpp`'s logic on them: which questions it asks, in which order and with which arguments, how it combines the answers,
//! what it caches in the condition and the parameters, and its effects (the completion queries, the object counts set, the transports).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`, 20,000 programs: 14 of 14 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 3841 | `playerFromParam` caches the mask of `<This Player's Enemy>` | 2,194 |
//! | 3842 | `NAMED_TOTALLY_DEAD` inverted for a unit that is gone | 121 |
//! | 3843 | the kind-in-area count caches its frame in the custom frame (as the other counts do) | 403 |
//! | 3844 | an attack with no source player is never by the side named | 24 |
//! | 3845 | `ENEMY_SIGHTED`'s enemy relation searches allies | 11 |
//! | 3846 | the skirmish special power's next frame not lowered to a module's ready frame | 23 |
//! | 3847 | the tech building's cached "no" asked again | 24 |
//! | 3848 | `evaluateTeamIsContained` of an empty team answers `allContained` | 13 |
//! | 3849 | `UNIT_EMPTIED` true whatever frame it last saw the transport | 10 |
//! | 3850 | the credits compared signed | 35 |
//! | 3851 | the health percentage truncated, not rounded | 16 |
//! | 3852 | `TEAM_HAS_UNITS` skips the calling team's name | 7 |
//! | 3853 | the value-in-area cache kept while the object-count frame goes back | 20 |
//! | 3854 | the source player mask of the last damage ignored | 8 |

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::base_type::{Coord3D, ICoord3D};
use rts_lockstep_engine::common::disabled_types::{DisabledType, DISABLED_HELD, DISABLED_UNMANNED};
use rts_lockstep_engine::common::game_common::ObjectShroudStatus;
use rts_lockstep_engine::common::game_type::ObjectId;
use rts_lockstep_engine::common::ini::IniError;
use rts_lockstep_engine::common::kind_of::{KindOfMaskType, KindOfType, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_STRUCTURE};
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::science::{ScienceType, SCIENCE_INVALID};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::object_types::ObjectTypes;
use rts_lockstep_engine::logic::scripts::{condition_type as ct, Condition, Parameter, MAX_PARMS};
use rts_lockstep_engine::logic::world::script_conditions::*;

mod support;
use support::Gen;

const NCFG: usize = 160;

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

/// `scNum`: the number after `prefix`, or -1.
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

fn name(prefix: &str, n: i32) -> AsciiString {
    AsciiString::try_from_bytes(format!("{prefix}{n}").as_bytes()).unwrap()
}

/// The configuration a program runs on, and the script engine's object counts.
struct Sim {
    cfg: [u32; NCFG],
    log: Vec<String>,
    counts: Vec<(i32, Vec<u8>, i32)>,
}

impl Sim {
    fn seed_a(&self) -> u32 {
        self.cfg[7]
    }
    fn seed_b(&self) -> u32 {
        self.cfg[8]
    }
    fn seed_c(&self) -> u32 {
        self.cfg[9]
    }
    fn nplayers(&self) -> i32 {
        ((self.cfg[4] & 15) as i32).min(8)
    }
    fn pw(&self, p: i32, k: usize) -> u32 {
        self.cfg[16 + 4 * p as usize + k]
    }
    fn ow(&self, o: ObjH, k: usize) -> u32 {
        self.cfg[80 + 4 * (o - 1) as usize + k]
    }
    fn tw(&self, t: TeamH) -> u32 {
        self.cfg[64 + t as usize]
    }
    fn player_or_none(&self, p: i32) -> Option<i32> {
        (p < self.nplayers()).then_some(p)
    }
    /// `scene()`'s prototypes: each one's owner and instances (a team in two prototypes stays in the first).
    fn protos(&self) -> Vec<(i32, Vec<TeamH>)> {
        let mut taken = 0u32;
        (0..8)
            .map(|k| {
                let w = self.cfg[48 + k];
                let mut inst = Vec::new();
                for t in 0..16u32 {
                    if (w >> (4 + t)) & 1 != 0 && (taken >> t) & 1 == 0 {
                        taken |= 1 << t;
                        inst.push(t);
                    }
                }
                ((w & 15) as i32, inst)
            })
            .collect()
    }
    fn exists(&self, j: i32) -> bool {
        self.cfg[80 + 4 * j as usize] & 1 != 0
    }
}

impl ScriptConditionsHost for Sim {
    fn sc_player_from_mask(&self, mask: u32) -> Option<PlayerH> {
        (0..self.nplayers()).find(|p| 1u32 << p == mask)
    }
    fn sc_neutral_player(&self) -> Option<PlayerH> {
        self.player_or_none(((self.cfg[4] >> 8) & 15) as i32)
    }
    fn sc_player_mask(&self, p: PlayerH) -> u32 {
        1 << p
    }
    fn sc_player_has_any_objects(&self, p: PlayerH) -> bool {
        self.pw(p, 0) & 1 != 0
    }
    fn sc_player_has_any_build_facility(&self, p: PlayerH) -> bool {
        (self.pw(p, 0) >> 1) & 1 != 0
    }
    fn sc_player_money(&self, p: PlayerH) -> u32 {
        self.pw(p, 1)
    }
    fn sc_player_count_buildings(&self, p: PlayerH) -> i32 {
        (self.pw(p, 2) & 255) as i32
    }
    fn sc_player_count_objects(&self, p: PlayerH, must: &KindOfMaskType, _must_not: &KindOfMaskType) -> i32 {
        if must.test(KINDOF_MP_COUNT_FOR_VICTORY) && must.test(KINDOF_STRUCTURE) {
            ((self.pw(p, 2) >> 8) & 255) as i32
        } else {
            -1
        }
    }
    fn sc_player_has_sufficient_power(&self, p: PlayerH) -> bool {
        (self.pw(p, 0) >> 2) & 1 != 0
    }
    fn sc_player_energy_supply_ratio(&self, p: PlayerH) -> f32 {
        (self.pw(p, 2) >> 24) as f32 / 100.0f32
    }
    fn sc_player_energy_production(&self, p: PlayerH) -> i32 {
        (self.pw(p, 3) & 0xffff) as i32
    }
    fn sc_player_energy_consumption(&self, p: PlayerH) -> i32 {
        (self.pw(p, 3) >> 16) as i32
    }
    fn sc_player_teams(&self, p: PlayerH) -> Vec<Vec<TeamH>> {
        self.protos().into_iter().filter(|(o, _)| *o == p && *o < self.nplayers()).map(|(_, i)| i).collect()
    }
    fn sc_player_count_objects_by_thing_template(&self, p: PlayerH, templates: &[TemplateH], ignore_dead: bool) -> Vec<i32> {
        templates
            .iter()
            .map(|t| {
                (1..=16)
                    .filter(|o| {
                        self.sc_obj_controlling_player(*o) == Some(p)
                            && self.sc_obj_template(*o) == Some(*t)
                            && !(ignore_dead && self.sc_obj_is_effectively_dead(*o))
                    })
                    .count() as i32
            })
            .collect()
    }
    fn sc_player_science_purchase_points(&self, p: PlayerH) -> i32 {
        (self.pw(p, 0) >> 24) as i32
    }
    fn sc_player_is_capable_of_purchasing_science(&self, p: PlayerH, science: ScienceType) -> bool {
        hh(self.seed_c(), p as u32, 1300u32.wrapping_add(science.0 as u32)) & 1 != 0
    }
    fn sc_player_side(&self, p: PlayerH) -> AsciiString {
        name("side", ((self.pw(p, 0) >> 4) & 15) as i32)
    }
    fn sc_player_supply_box_value(&self, p: PlayerH) -> i32 {
        ((self.pw(p, 0) >> 12) & 15) as i32
    }
    fn sc_player_is_supply_source_safe(&mut self, p: PlayerH, min_supplies: i32) -> bool {
        let r = min_supplies <= ((self.pw(p, 2) >> 16) & 255) as i32;
        self.log.push(format!("supplysafe {p} {min_supplies} {}", u8::from(r)));
        r
    }
    fn sc_player_is_supply_source_attacked(&mut self, p: PlayerH) -> bool {
        let r = (self.pw(p, 0) >> 3) & 1 != 0;
        self.log.push(format!("supplyattacked {p} {}", u8::from(r)));
        r
    }
    fn sc_player_mp_start_index(&self, p: PlayerH) -> i32 {
        ((self.pw(p, 0) >> 8) & 15) as i32
    }
    fn sc_player_attacked_by(&self, p: PlayerH, index: i32) -> bool {
        (0..8).contains(&index) && (self.pw(p, 0) >> (16 + index)) & 1 != 0
    }

    fn sc_player_from_name(&self, n: &[u8]) -> Option<PlayerH> {
        if n == THIS_PLAYER_ENEMY {
            return self.player_or_none(((self.cfg[4] >> 4) & 15) as i32);
        }
        let k = num(n, "player");
        (k >= 0 && k < self.nplayers()).then_some(k)
    }
    fn sc_team_named(&self, n: &[u8]) -> Option<TeamH> {
        if n == b"<This Team>" {
            let t = self.cfg[5] & 255;
            return (t < 16).then_some(t);
        }
        let k = num(n, "team");
        if !(0..8).contains(&k) {
            return None;
        }
        let t = (self.cfg[48 + k as usize] >> 20) & 255;
        (t < 16).then_some(t)
    }
    fn sc_unit_named(&self, n: &[u8]) -> Option<ObjH> {
        let k = num(n, "unit");
        ((0..16).contains(&k) && self.exists(k)).then_some(k + 1)
    }
    fn sc_did_unit_exist(&self, n: &[u8]) -> bool {
        let k = num(n, "unit");
        (0..32).contains(&k) && (self.cfg[10] >> k) & 1 != 0
    }
    fn sc_trigger_named(&self, n: &[u8]) -> Option<TriggerH> {
        let k = num(n, "trig");
        ((0..8).contains(&k) && (self.cfg[6] >> k) & 1 != 0).then_some(k as u32)
    }
    fn sc_frame_object_count_changed(&self) -> u32 {
        self.cfg[2]
    }
    fn sc_object_types(&self, n: &AsciiString) -> Option<ObjectTypes> {
        let k = num(n.str(), "list");
        if k < 0 || k >= (self.cfg[12] & 7) as i32 {
            return None;
        }
        let mut l = ObjectTypes::with_list_name(n.clone());
        for j in 0..10u32 {
            if hh(self.seed_c(), k as u32, 100 + j) & 1 != 0 {
                l.add_object_type(&if j < 9 { name("tmpl", j as i32) } else { AsciiString::try_from_bytes(b"x0").unwrap() });
            }
        }
        Some(l)
    }
    fn sc_is_video_complete(&mut self, n: &[u8], remove: bool) -> bool {
        self.done("video", n, remove, 1400)
    }
    fn sc_is_speech_complete(&mut self, n: &[u8], remove: bool) -> bool {
        self.done("speech", n, remove, 1401)
    }
    fn sc_is_audio_complete(&mut self, n: &[u8], remove: bool) -> bool {
        self.done("audio", n, remove, 1402)
    }
    fn sc_is_special_power_triggered(&mut self, i: i32, n: &[u8], r: bool, s: ObjectId) -> bool {
        self.power("sptriggered", i, n, r, s, 1500)
    }
    fn sc_is_special_power_midway(&mut self, i: i32, n: &[u8], r: bool, s: ObjectId) -> bool {
        self.power("spmidway", i, n, r, s, 1600)
    }
    fn sc_is_special_power_complete(&mut self, i: i32, n: &[u8], r: bool, s: ObjectId) -> bool {
        self.power("spcomplete", i, n, r, s, 1700)
    }
    fn sc_is_upgrade_complete(&mut self, i: i32, n: &[u8], r: bool, s: ObjectId) -> bool {
        self.power("upgrade", i, n, r, s, 1800)
    }
    fn sc_is_science_acquired(&mut self, index: i32, science: ScienceType, remove: bool) -> bool {
        let r = hh(self.seed_c(), index as u32, 1900u32.wrapping_add(science.0 as u32)) & 1 != 0;
        self.log.push(format!("science {index} {} {} {}", science.0, u8::from(remove), u8::from(r)));
        r
    }
    fn sc_object_count(&self, index: i32, n: &[u8]) -> i32 {
        if let Some(c) = self.counts.iter().find(|c| c.0 == index && c.1 == n) {
            return c.2;
        }
        (hh(self.seed_c(), (index as u32).wrapping_mul(977).wrapping_add(strhash(n)), 2100) % 4) as i32
    }
    fn sc_set_object_count(&mut self, index: i32, n: &[u8], count: i32) {
        self.log.push(format!("setobjectcount {index} {} {count}", String::from_utf8_lossy(n)));
        if let Some(c) = self.counts.iter_mut().find(|c| c.0 == index && c.1 == n) {
            c.2 = count;
        } else {
            self.counts.push((index, n.to_vec(), count));
        }
    }
    fn sc_team_prototype_instances(&self, n: &[u8]) -> Option<Vec<TeamH>> {
        let k = num(n, "team");
        (0..8).contains(&k).then(|| self.protos()[k as usize].1.clone())
    }

    fn sc_team_has_any_objects(&self, t: TeamH) -> bool {
        self.tw(t) & 1 != 0
    }
    fn sc_team_has_any_units(&self, t: TeamH) -> bool {
        (self.tw(t) >> 1) & 1 != 0
    }
    fn sc_team_name(&self, t: TeamH) -> AsciiString {
        for (k, (_, inst)) in self.protos().iter().enumerate() {
            if inst.contains(&t) {
                return name("team", k as i32);
            }
        }
        name("lone", t as i32)
    }
    fn sc_team_some_inside_some_outside(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 1 != 0
    }
    fn sc_team_all_inside(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 2 != 0
    }
    fn sc_team_did_all_enter(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 4 != 0
    }
    fn sc_team_did_partial_enter(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 8 != 0
    }
    fn sc_team_did_all_exit(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 16 != 0
    }
    fn sc_team_did_partial_exit(&self, t: TeamH, trig: TriggerH, which: u32) -> bool {
        self.ths(t, trig, which) & 32 != 0
    }
    fn sc_team_did_enter_or_exit(&self, t: TeamH) -> bool {
        (self.tw(t) >> 3) & 1 != 0
    }
    fn sc_team_state(&self, t: TeamH) -> AsciiString {
        name("state", ((self.tw(t) >> 4) & 15) as i32)
    }
    fn sc_team_is_created(&self, t: TeamH) -> bool {
        (self.tw(t) >> 2) & 1 != 0
    }
    fn sc_team_controlling_player(&self, t: TeamH) -> Option<PlayerH> {
        self.player_or_none(((self.tw(t) >> 8) & 15) as i32)
    }
    fn sc_team_members(&self, t: TeamH) -> Vec<ObjH> {
        (0..16).filter(|j| (self.tw(t) >> (16 + j)) & 1 != 0).map(|j| j + 1).collect()
    }

    fn sc_find_object(&self, id: ObjectId) -> Option<ObjH> {
        (1..=16).contains(&id).then_some(id)
    }
    fn sc_obj_is_effectively_dead(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 1) & 1 != 0
    }
    fn sc_obj_position(&self, o: ObjH) -> Coord3D {
        let w = self.ow(o, 3);
        Coord3D { x: ((w >> 16) & 255) as f32 * 0.75, y: (w >> 24) as f32 * 0.75, z: (o - 1) as f32 * 0.25 }
    }
    fn sc_obj_template(&self, o: ObjH) -> Option<TemplateH> {
        let t = (self.ow(o, 0) >> 12) & 7;
        (t < 7).then_some(t)
    }
    fn sc_obj_is_kind_of(&self, o: ObjH, kind: KindOfType) -> bool {
        (self.ow(o, 1) >> (kind as u32 & 15)) & 1 != 0
    }
    fn sc_obj_is_inside(&self, o: ObjH, trig: TriggerH) -> bool {
        hh(self.seed_b(), (o - 1) as u32, 200 + trig) & 1 != 0
    }
    fn sc_obj_did_enter(&self, o: ObjH, trig: TriggerH) -> bool {
        hh(self.seed_b(), (o - 1) as u32, 200 + trig) & 2 != 0
    }
    fn sc_obj_did_exit(&self, o: ObjH, trig: TriggerH) -> bool {
        hh(self.seed_b(), (o - 1) as u32, 200 + trig) & 4 != 0
    }
    fn sc_obj_has_body(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 2) & 1 != 0
    }
    fn sc_obj_health(&self, o: ObjH) -> (f32, f32) {
        let w = self.ow(o, 1);
        (((w >> 16) & 255) as f32, (w >> 24) as f32)
    }
    fn sc_obj_last_damage(&self, o: ObjH) -> Option<LastDamage> {
        let w = self.ow(o, 0);
        if (w >> 10) & 1 == 0 {
            return None;
        }
        let t = (w >> 20) & 7;
        let p = w >> 28;
        Some(LastDamage {
            source_template: ((w >> 11) & 1 != 0 && t < 7).then_some(t),
            source_id: ((w >> 23) & 31) as i32,
            source_player_mask: if p == 15 { 0 } else { 1 << p },
        })
    }
    fn sc_obj_controlling_player(&self, o: ObjH) -> Option<PlayerH> {
        self.player_or_none(((self.ow(o, 0) >> 16) & 15) as i32)
    }
    fn sc_obj_contain(&self, o: ObjH) -> Option<ContainInfo> {
        if (self.ow(o, 0) >> 3) & 1 == 0 {
            return None;
        }
        let w = self.ow(o, 3);
        let p = (w >> 9) & 15;
        Some(ContainInfo { player_who_entered: if p == 15 { 0 } else { 1 << p }, count: w & 15, max: (w >> 4) & 15, garrisonable: (w >> 8) & 1 != 0 })
    }
    fn sc_obj_status(&self, o: ObjH) -> ObjectStatusMaskType {
        status_of(self.ow(o, 2))
    }
    fn sc_obj_is_disabled_by_type(&self, o: ObjH, t: DisabledType) -> bool {
        if t == DISABLED_HELD {
            (self.ow(o, 0) >> 7) & 1 != 0
        } else if t == DISABLED_UNMANNED {
            (self.ow(o, 0) >> 8) & 1 != 0
        } else {
            false
        }
    }
    fn sc_obj_is_disabled(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 9) & 1 != 0
    }
    fn sc_obj_shrouded_status(&self, o: ObjH, index: i32) -> ObjectShroudStatus {
        match hh(self.seed_b(), (o - 1) as u32, 300u32.wrapping_add(index as u32)) % 6 {
            0 => ObjectShroudStatus::Invalid,
            1 => ObjectShroudStatus::Clear,
            2 => ObjectShroudStatus::PartialClear,
            3 => ObjectShroudStatus::Fogged,
            4 => ObjectShroudStatus::Shrouded,
            _ => ObjectShroudStatus::InvalidButPreviousValid,
        }
    }
    fn sc_obj_completed_waypoint(&self, o: ObjH) -> Option<Option<[AsciiString; 3]>> {
        if (self.ow(o, 0) >> 4) & 1 == 0 {
            return None;
        }
        let k = ((self.ow(o, 3) >> 13) & 7) as i32;
        Some((k != 7).then(|| [name("path", k), name("path", k + 1), name("path", k * 2 % 7)]))
    }
    fn sc_obj_is_contained(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 5) & 1 != 0
    }
    fn sc_obj_is_captured(&self, o: ObjH) -> bool {
        (self.ow(o, 0) >> 6) & 1 != 0
    }
    fn sc_obj_special_power_module(&self, o: ObjH, power: SpecialPowerH) -> Option<SpecialPowerModuleInfo> {
        let h = hh(self.seed_c(), (o - 1) as u32, 400 + power);
        (h & 1 != 0).then_some(SpecialPowerModuleInfo { ready: h & 2 != 0, ready_frame: (h >> 8) % 2000 })
    }
    fn sc_obj_has_special_power(&self, o: ObjH, power_type: i32) -> bool {
        hh(self.seed_c(), (o - 1) as u32, 600u32.wrapping_add(power_type as u32)) & 1 != 0
    }
    fn sc_obj_warehouse_boxes(&self, o: ObjH) -> Option<i32> {
        let h = hh(self.seed_b(), (o - 1) as u32, 700);
        (h & 1 != 0).then_some(((h >> 4) % 20) as i32)
    }

    fn sc_find_template(&self, n: &AsciiString) -> Option<TemplateH> {
        let k = num(n.str(), "tmpl");
        (k >= 0 && k < ((self.cfg[11] & 255) as i32).min(8)).then_some(k as u32)
    }
    fn sc_template_name(&self, t: TemplateH) -> AsciiString {
        name("tmpl", t as i32)
    }
    fn sc_template_build_cost(&self, t: TemplateH) -> i32 {
        t as i32 * 100 + 50
    }
    fn sc_player_can_build(&self, p: PlayerH, t: TemplateH) -> bool {
        hh(self.seed_c(), p as u32, 1000 + t) & 1 != 0
    }
    fn sc_find_special_power(&self, n: &[u8]) -> Option<SpecialPowerH> {
        let k = num(n, "power");
        (k >= 0 && k < ((self.cfg[11] >> 8) & 7) as i32).then_some(k as u32)
    }
    fn sc_can_use_special_power(&self, o: ObjH, power: SpecialPowerH) -> bool {
        hh(self.seed_c(), (o - 1) as u32, 500 + power) & 1 != 0
    }
    fn sc_special_power_type(&self, power: SpecialPowerH) -> i32 {
        power as i32 + 1
    }
    fn sc_science_from_name(&self, n: &[u8]) -> ScienceType {
        let k = num(n, "sci");
        if k >= 0 && k < ((self.cfg[11] >> 16) & 255) as i32 {
            ScienceType(k)
        } else {
            SCIENCE_INVALID
        }
    }
    fn sc_find_command_button(&self, n: &[u8]) -> Option<ButtonH> {
        let k = num(n, "btn");
        (k >= 0 && k < ((self.cfg[11] >> 24) & 7) as i32).then_some(k as u32)
    }
    fn sc_button_special_power(&self, b: ButtonH) -> Option<SpecialPowerH> {
        let n = (self.cfg[11] >> 8) & 7;
        (hh(self.seed_c(), b, 1100) % 3 == 2 && n > 0).then_some(b % n.max(1))
    }
    fn sc_button_has_upgrade(&self, b: ButtonH) -> bool {
        let kind = hh(self.seed_c(), b, 1100) % 3;
        kind == 0 || (kind == 2 && self.sc_button_special_power(b).is_none())
    }
    fn sc_button_is_ready(&self, b: ButtonH, o: ObjH) -> bool {
        hh(self.seed_c(), b * 64 + (o - 1) as u32, 1200) & 1 != 0
    }

    fn sc_trigger_point_in(&self, trig: TriggerH, p: ICoord3D) -> bool {
        let a = (trig as i32).wrapping_mul(1000).wrapping_add(p.x) as u32;
        let b = p.y.wrapping_mul(31).wrapping_add(p.z) as u32;
        hh(self.seed_b(), a, b) & 1 != 0
    }
    fn sc_trigger_center(&self, trig: TriggerH) -> Coord3D {
        Coord3D { x: trig as f32 * 10.0, y: trig as f32 * 5.0, z: 0.0 }
    }
    fn sc_trigger_radius(&self, trig: TriggerH) -> f32 {
        trig as f32 * 2.0 + 1.0
    }
    fn sc_partition_seen_by(&self, o: ObjH, relation: Option<i32>) -> Vec<ObjH> {
        let rel = relation.map(|r| r as u32).unwrap_or(9);
        let i = o - 1;
        (0..16)
            .filter(|j| *j != i && self.exists(*j) && hh(self.seed_b(), (i * 64 + j) as u32, 800 + rel) & 1 != 0)
            .map(|j| j + 1)
            .collect()
    }
    fn sc_partition_supply_structures(&self, _center: Coord3D, _distance: f32, p: PlayerH) -> Vec<ObjH> {
        (0..16).filter(|j| hh(self.seed_b(), *j as u32, 900 + p as u32) & 1 != 0).map(|j| j + 1).collect()
    }
    fn sc_partition_tech_building_near(&self, _center: Coord3D, _radius: f32, p: PlayerH) -> bool {
        (hh(self.seed_b(), p as u32, 950) ^ self.cfg[14] >> p) & 1 != 0
    }

    fn sc_any_bridges_damage_states_changed(&self) -> bool {
        (self.cfg[0] >> 2) & 1 != 0
    }
    fn sc_is_bridge_broken(&self, o: ObjH) -> bool {
        hh(self.seed_b(), (o - 1) as u32, 2200) & 1 != 0
    }
    fn sc_is_bridge_repaired(&self, o: ObjH) -> bool {
        hh(self.seed_b(), (o - 1) as u32, 2200) & 2 != 0
    }

    fn sc_frame(&self) -> u32 {
        self.cfg[1]
    }
    fn sc_is_multiplayer_session(&self) -> bool {
        self.cfg[0] & 1 != 0
    }
    fn sc_frame_selection_changed(&self) -> u32 {
        self.cfg[3]
    }
    fn sc_selected_object_names(&self) -> Vec<Option<AsciiString>> {
        let mut v: Vec<Option<AsciiString>> = (0..16).filter(|j| (self.cfg[13] >> j) & 1 != 0).map(|j| Some(name("unit", j))).collect();
        if self.cfg[13] >> 31 != 0 {
            v.push(None);
        }
        v
    }
    fn sc_camera_movement_finished(&self) -> bool {
        (self.cfg[0] >> 1) & 1 != 0
    }
    fn sc_is_local_allied_victory(&self) -> Result<bool, IniError> {
        Ok((self.cfg[0] >> 3) & 1 != 0)
    }
    fn sc_is_local_allied_defeat(&self) -> Result<bool, IniError> {
        Ok((self.cfg[0] >> 4) & 1 != 0)
    }
    fn sc_is_local_defeat(&self) -> bool {
        (self.cfg[0] >> 5) & 1 != 0
    }
    fn sc_music_track_completed(&self, n: &[u8], times: i32) -> bool {
        hh(self.seed_c(), strhash(n), 2000u32.wrapping_add(times as u32)) & 1 != 0
    }
}

impl Sim {
    fn ths(&self, t: TeamH, trig: TriggerH, which: u32) -> u32 {
        hh(self.seed_a(), t * 8 + trig, which)
    }
    fn done(&mut self, what: &str, n: &[u8], remove: bool, salt: u32) -> bool {
        let r = hh(self.seed_c(), strhash(n), salt) & 1 != 0;
        self.log.push(format!("{what} {} {} {}", String::from_utf8_lossy(n), u8::from(remove), u8::from(r)));
        r
    }
    fn power(&mut self, what: &str, idx: i32, n: &[u8], remove: bool, src: ObjectId, salt: u32) -> bool {
        let r = hh(self.seed_c(), (idx as u32).wrapping_mul(977).wrapping_add(strhash(n)), salt.wrapping_add(src as u32)) & 1 != 0;
        self.log.push(format!("{what} {idx} {} {} {src} {}", String::from_utf8_lossy(n), u8::from(remove), u8::from(r)));
        r
    }
}

/// An object's status word as a mask: bit i is status i, bit 31 is `OBJECT_STATUS_DISGUISED` (43).
fn status_of(w: u32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::default();
    for i in 0..31 {
        if (w >> i) & 1 != 0 {
            m.set(i, 1);
        }
    }
    if w >> 31 != 0 {
        m.set(43, 1);
    }
    m
}

/// A parameter as the program gives it.
#[derive(Clone, Debug)]
struct P {
    int: i32,
    real: f32,
    status: u64,
    s: Vec<u8>,
}

#[derive(Clone, Debug)]
struct C {
    ty: i32,
    parms: Vec<P>,
}

fn build_condition(c: &C) -> Condition {
    let mut cd = Condition::new_for_read();
    cd.condition_type = c.ty;
    cd.num_parms = c.parms.len() as i32;
    for (i, p) in c.parms.iter().enumerate().take(MAX_PARMS) {
        let mut q = Parameter::new(0, p.int);
        q.real = p.real;
        q.string = p.s.clone();
        let mut m = ObjectStatusMaskType::default();
        for b in 0..45 {
            if (p.status >> b) & 1 != 0 {
                m.set(b, 1);
            }
        }
        q.object_status = m;
        cd.parms[i] = Some(q);
    }
    cd
}

fn encode_conditions(cs: &[C]) -> Vec<u32> {
    let mut v = vec![cs.len() as u32];
    for c in cs {
        v.push(c.ty as u32);
        v.push(c.parms.len() as u32);
        for p in &c.parms {
            v.push(p.int as u32);
            v.push(p.real.to_bits());
            v.push(p.status as u32);
            v.push((p.status >> 32) as u32);
            v.push(p.s.len() as u32);
            for ch in p.s.chunks(4) {
                let mut w = 0u32;
                for (k, b) in ch.iter().enumerate() {
                    w |= u32::from(*b) << (8 * k);
                }
                v.push(w);
            }
        }
    }
    v
}

fn put_str(out: &mut Vec<u32>, s: &str) {
    let b = s.as_bytes();
    out.push(b.len() as u32);
    for ch in b.chunks(4) {
        let mut w = 0u32;
        for (k, x) in ch.iter().enumerate() {
            w |= u32::from(*x) << (8 * k);
        }
        out.push(w);
    }
}

/// The Rust run: the output, and the operation at which upstream is undefined (the output stops before it).
fn rust_run(cfg: &[u32; NCFG], cs: &[C], ops: &[[u32; 3]]) -> (Vec<u32>, Option<(usize, String)>) {
    let mut sim = Sim { cfg: *cfg, log: Vec::new(), counts: Vec::new() };
    let mut conds: Vec<Condition> = cs.iter().map(build_condition).collect();
    let mut sc = ScriptConditions::new();
    sc.init();
    let mut out = Vec::new();
    for (i, o) in ops.iter().enumerate() {
        if o[0] == 1 && (o[1] as usize) < NCFG {
            sim.cfg[o[1] as usize] = o[2];
        }
        sim.log.clear();
        let mut res = 0u32;
        if o[0] == 0 && (o[1] as usize) < conds.len() {
            match sc.evaluate_condition(&mut sim, &mut conds[o[1] as usize]) {
                Ok(r) => res = u32::from(r),
                Err(e) => return (out, Some((i, format!("{} {e:?}", conds[o[1] as usize].condition_type)))),
            }
        }
        if o[0] == 2 {
            sc.init();
        }
        if (o[0] == 3 || o[0] == 4) && (o[1] as usize) < conds.len() {
            let cd = &conds[o[1] as usize];
            let r = if o[0] == 3 { sc.team_is_contained(&sim, cd, 0, o[2] != 0) } else { sc.skirmish_command_button_is_ready(&sim, cd, 1, 2, o[2] != 0) };
            match r {
                Ok(r) => res = u32::from(r),
                Err(e) => return (out, Some((i, format!("direct{} {e:?}", o[0])))),
            }
        }
        out.push(res);
        out.push(sim.log.len() as u32);
        for l in &sim.log {
            put_str(&mut out, l);
        }
        if o[0] == 0 && (o[1] as usize) < conds.len() {
            let cd = &conds[o[1] as usize];
            out.push(cd.custom_data as u32);
            out.push(cd.custom_frame);
            for k in 0..cd.num_parms as usize {
                out.push(cd.parms[k].as_ref().unwrap().int as u32);
            }
        }
        out.push(sc.transport_statuses.len() as u32);
        for t in &sc.transport_statuses {
            out.extend([t.obj_id as u32, t.frame_number, t.unit_count as u32]);
        }
    }
    (out, None)
}

fn cpp_run(cfg: &[u32; NCFG], cs: &[C], ops: &[[u32; 3]]) -> Vec<u32> {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let enc = encode_conditions(cs);
    let mut out = vec![0u32; 1 << 18];
    let mut len = 0i32;
    let st = unsafe {
        cpp::ref_conditions_run(cfg.as_ptr(), enc.as_ptr(), flat.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32, &mut len)
    };
    assert_eq!(st, 0, "the output fits");
    out.truncate(len as usize);
    out
}

/// The parameters each condition type's evaluation reads (S side, T team, U unit, A trigger area, W team-area kinds, Y object types,
/// C comparison, I count, H health percent, M money, K kind of, Z team state, P waypoint path, N name, O special power, G the skirmish
/// special power (its Int the next frame), X science, B command button, R distance, V value, Q object status, F faction, L relation).
fn kinds(ty: i32) -> &'static str {
    use ct::*;
    match ty {
        PLAYER_ALL_DESTROYED | PLAYER_ALL_BUILDFACILITIES_DESTROYED | PLAYER_HAS_POWER | PLAYER_HAS_NO_POWER | SUPPLY_SOURCE_ATTACKED => "S",
        TEAM_INSIDE_AREA_PARTIALLY | TEAM_INSIDE_AREA_ENTIRELY | TEAM_OUTSIDE_AREA_ENTIRELY | TEAM_ENTERED_AREA_ENTIRELY
        | TEAM_ENTERED_AREA_PARTIALLY | TEAM_EXITED_AREA_ENTIRELY | TEAM_EXITED_AREA_PARTIALLY => "TAW",
        TEAM_DESTROYED | TEAM_HAS_UNITS | TEAM_CREATED | TEAM_COMPLETED_SEQUENTIAL_EXECUTION => "T",
        TEAM_STATE_IS | TEAM_STATE_IS_NOT => "TZ",
        NAMED_INSIDE_AREA | NAMED_OUTSIDE_AREA | NAMED_ENTERED_AREA | NAMED_EXITED_AREA => "UA",
        NAMED_DESTROYED | NAMED_NOT_DESTROYED | NAMED_DYING | NAMED_TOTALLY_DEAD | NAMED_CREATED | NAMED_SELECTED | BRIDGE_REPAIRED
        | BRIDGE_BROKEN | UNIT_EMPTIED | NAMED_BUILDING_IS_EMPTY | NAMED_HAS_FREE_CONTAINER_SLOTS | UNIT_COMPLETED_SEQUENTIAL_EXECUTION => "U",
        NAMED_ATTACKED_BY_OBJECTTYPE => "UY",
        TEAM_ATTACKED_BY_OBJECTTYPE => "TY",
        NAMED_ATTACKED_BY_PLAYER | NAMED_DISCOVERED | NAMED_OWNED_BY_PLAYER => "US",
        TEAM_ATTACKED_BY_PLAYER | TEAM_DISCOVERED | TEAM_OWNED_BY_PLAYER => "TS",
        BUILT_BY_PLAYER => "YS",
        PLAYER_HAS_CREDITS => "MCS",
        MISSION_ATTEMPTS => "SCI",
        PLAYER_HAS_N_OR_FEWER_BUILDINGS | PLAYER_HAS_N_OR_FEWER_FACTION_BUILDINGS | PLAYER_HAS_SCIENCEPURCHASEPOINTS | SUPPLY_SOURCE_SAFE
        | START_POSITION_IS => "SI",
        NAMED_REACHED_WAYPOINTS_END => "UP",
        TEAM_REACHED_WAYPOINTS_END => "TP",
        HAS_FINISHED_VIDEO | HAS_FINISHED_SPEECH | HAS_FINISHED_AUDIO => "N",
        BUILDING_ENTERED_BY_PLAYER => "SU",
        ENEMY_SIGHTED => "ULS",
        UNIT_HEALTH => "UCH",
        PLAYER_HAS_OBJECT_COMPARISON => "SCIY",
        PLAYER_TRIGGERED_SPECIAL_POWER | PLAYER_COMPLETED_SPECIAL_POWER | PLAYER_MIDWAY_SPECIAL_POWER => "SO",
        PLAYER_TRIGGERED_SPECIAL_POWER_FROM_NAMED | PLAYER_COMPLETED_SPECIAL_POWER_FROM_NAMED | PLAYER_MIDWAY_SPECIAL_POWER_FROM_NAMED => "SOU",
        PLAYER_BUILT_UPGRADE => "SN",
        PLAYER_BUILT_UPGRADE_FROM_NAMED => "SNU",
        PLAYER_DESTROYED_N_BUILDINGS_PLAYER => "SIS",
        PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA => "SCIYA",
        PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA => "SCIKA",
        TYPE_SIGHTED => "UYS",
        UNIT_HAS_OBJECT_STATUS => "UQ",
        TEAM_ALL_HAS_OBJECT_STATUS | TEAM_SOME_HAVE_OBJECT_STATUS => "TQ",
        PLAYER_POWER_COMPARE_PERCENT => "SCH",
        PLAYER_EXCESS_POWER_COMPARE_VALUE | SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS | SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED
        | SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS => "SCI",
        SKIRMISH_SPECIAL_POWER_READY => "SG",
        SKIRMISH_VALUE_IN_AREA => "SCMA",
        SKIRMISH_PLAYER_FACTION => "SF",
        SKIRMISH_SUPPLIES_VALUE_WITHIN_DISTANCE => "SRAV",
        SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE => "SRA",
        SKIRMISH_COMMAND_BUTTON_READY_ALL | SKIRMISH_COMMAND_BUTTON_READY_PARTIAL => "STB",
        SKIRMISH_PLAYER_HAS_PREREQUISITE_TO_BUILD | PLAYER_LOST_OBJECT_TYPE => "SY",
        SKIRMISH_NAMED_AREA_EXIST | SKIRMISH_PLAYER_HAS_UNITS_IN_AREA | SKIRMISH_PLAYER_IS_OUTSIDE_AREA => "SA",
        SKIRMISH_PLAYER_HAS_BEEN_ATTACKED_BY_PLAYER | SKIRMISH_PLAYER_HAS_DISCOVERED_PLAYER => "SS",
        PLAYER_ACQUIRED_SCIENCE | PLAYER_CAN_PURCHASE_SCIENCE => "SX",
        MUSIC_TRACK_HAS_COMPLETED => "NI",
        _ => "",
    }
}

fn s(x: &str) -> Vec<u8> {
    x.as_bytes().to_vec()
}

fn gen_parm(g: &mut Gen, k: char, np: u32) -> P {
    let mut p = P { int: 0, real: 0.0, status: 0, s: Vec::new() };
    match k {
        'S' => {
            p.s = match g.below(20) {
                0..=2 => s("<This Player's Enemy>"),
                3 => s("nobody"),
                _ => s(&format!("player{}", g.below(np + 1))),
            };
            p.int = match g.below(12) {
                0 => 1 << g.below(np + 1),
                1 => 0xFFFF_0000u32 as i32,
                _ => 0,
            };
        }
        'T' => {
            p.s = match g.below(12) {
                0..=2 => s("<This Team>"),
                3 => s(&format!("lone{}", g.below(16))),
                _ => s(&format!("team{}", g.below(9))),
            }
        }
        'U' => p.s = s(&format!("unit{}", g.below(18))),
        'A' => p.s = s(&format!("trig{}", g.below(9))),
        'W' => p.int = g.below(4) as i32,
        'Y' => {
            p.s = match g.below(12) {
                0 => Vec::new(),
                1 => s("x0"),
                2..=4 => s(&format!("list{}", g.below(4))),
                _ => s(&format!("tmpl{}", g.below(9))),
            }
        }
        'C' => p.int = if g.below(15) == 0 { [6, -1][g.below(2) as usize] } else { g.below(6) as i32 },
        'I' => p.int = if g.below(10) == 0 { g.i32() } else { g.below(8) as i32 - 1 },
        'H' => p.int = if g.below(2) == 0 { [0, 17, 20, 25, 33, 34, 40, 50, 60, 66, 67, 75, 80, 83, 100][g.below(15) as usize] } else { g.below(160) as i32 },
        'M' => p.int = if g.below(6) == 0 { -(g.below(1000) as i32) } else { (g.below(30) * 100) as i32 },
        'K' => p.int = [7, 22, 44, 84, 0, 1, 2, 3, 5, 9, 15, 33, 65][g.below(13) as usize],
        'Z' => p.s = s(&format!("state{}", g.below(5))),
        'P' => p.s = s(&format!("path{}", g.below(9))),
        'N' => p.s = s(&format!("name{}", g.below(4))),
        'O' => p.s = s(&format!("power{}", g.below(6))),
        'G' => {
            p.s = s(&format!("power{}", g.below(6)));
            p.int = match g.below(8) {
                0 => -1,
                1 => g.below(3000) as i32,
                2 => g.i32(),
                _ => 0,
            };
        }
        'X' => p.s = if g.below(8) == 0 { s("bad") } else { s(&format!("sci{}", g.below(6))) },
        'B' => p.s = s(&format!("btn{}", g.below(6))),
        'R' => p.real = g.below(40) as f32 * 0.5,
        'V' => p.real = g.below(200) as f32 - 10.0,
        'Q' => p.status = if g.below(3) == 0 { 1 << [3u64, 16, 17, 43][g.below(4) as usize] } else { g.u64() & ((1 << 45) - 1) & g.u64() & g.u64() },
        'F' => p.s = s(&format!("side{}", g.below(5))),
        'L' => p.int = if g.below(25) == 0 { 3 } else { g.below(3) as i32 },
        _ => {}
    }
    if g.below(40) == 0 {
        // a stray Int, as a map could carry
        p.int = g.i32();
    }
    p
}

fn gen_program(g: &mut Gen) -> ([u32; NCFG], Vec<C>, Vec<[u32; 3]>) {
    let mut cfg = [0u32; NCFG];
    for w in cfg.iter_mut() {
        *w = g.u32();
    }
    let np = 1 + g.below(6);
    cfg[0] = g.u32() & 63;
    cfg[1] = [0u32, 1, 30, 100, 1000][g.below(5) as usize];
    cfg[2] = g.below(4);
    cfg[3] = g.below(4);
    cfg[4] = np | g.below(np + 2) << 4 | (if g.below(4) == 0 { 15 } else { g.below(np + 1) }) << 8;
    cfg[5] = if g.below(4) == 0 { 255 } else { g.below(16) };
    cfg[6] = g.u32() & 0xff;
    cfg[10] = g.u32();
    cfg[11] = (3 + g.below(6)) | (g.below(6) << 8) | ((2 + g.below(5)) << 16) | (g.below(7) << 24);
    cfg[12] = g.below(4);
    cfg[13] = g.u32() & 0xffff | if g.below(30) == 0 { 1 << 31 } else { 0 };
    for p in 0..8usize {
        // money small or large; ratio 0..1.5; production and consumption small
        cfg[16 + 4 * p + 1] = if g.below(3) == 0 { g.u32() } else { g.below(3000) };
        cfg[16 + 4 * p + 2] = (g.u32() & 0x00ff_ffff) | ([0u32, 50, 100, 150, 25][g.below(5) as usize] << 24);
        cfg[16 + 4 * p + 3] = g.below(40) | g.below(40) << 16;
    }
    for k in 0..8usize {
        let owner = if g.below(6) == 0 { 15 } else { g.below(np) };
        let inst = g.u32() & g.u32() & 0xffff;
        let named = if g.below(5) == 0 { 255 } else { g.below(16) };
        cfg[48 + k] = owner | inst << 4 | named << 20;
    }
    for t in 0..16usize {
        cfg[64 + t] = (g.u32() & 0x0fff & !(0xf00)) | (if g.below(6) == 0 { 15 } else { g.below(np) }) << 8 | (g.u32() & g.u32() & 0xffff) << 16;
    }
    for j in 0..16usize {
        let mut w0 = g.u32();
        if g.below(5) != 0 {
            w0 |= 1; // named
        }
        if g.below(3) != 0 {
            w0 &= !2; // alive
        }
        w0 = (w0 & !(0xf << 16)) | (if g.below(6) == 0 { 15 } else { g.below(np) }) << 16;
        cfg[80 + 4 * j] = w0;
        let mut w1 = g.u32();
        let (health, initial) = if g.below(2) == 0 {
            let i = 1 + g.below(6);
            (g.below(i + 1), i)
        } else {
            (g.below(256), if g.below(40) == 0 { 0 } else { 1 + g.below(255) })
        };
        w1 = (w1 & 0x0000_ffff) | health << 16 | initial << 24;
        cfg[80 + 4 * j + 1] = w1;
        cfg[80 + 4 * j + 2] = g.u32() & g.u32();
    }
    let nc = 1 + g.below(8) as usize;
    let mut cs = Vec::new();
    for _ in 0..nc {
        let ty = if g.below(4) == 0 {
            // the types that keep state between evaluations
            [
                ct::UNIT_EMPTIED,
                ct::BUILT_BY_PLAYER,
                ct::PLAYER_HAS_OBJECT_COMPARISON,
                ct::PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA,
                ct::PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA,
                ct::SKIRMISH_VALUE_IN_AREA,
                ct::SKIRMISH_PLAYER_HAS_UNITS_IN_AREA,
                ct::SKIRMISH_PLAYER_IS_OUTSIDE_AREA,
                ct::SKIRMISH_TECH_BUILDING_WITHIN_DISTANCE,
                ct::SKIRMISH_SPECIAL_POWER_READY,
                ct::SUPPLY_SOURCE_SAFE,
                ct::NAMED_SELECTED,
                ct::PLAYER_LOST_OBJECT_TYPE,
            ][g.below(13) as usize]
        } else {
            g.below(ct::NUM_ITEMS as u32 + 1) as i32
        };
        let ks = kinds(ty);
        let mut parms: Vec<P> = ks.chars().map(|k| gen_parm(g, k, np)).collect();
        if g.below(30) == 0 && !parms.is_empty() {
            let keep = g.below(parms.len() as u32) as usize;
            parms.truncate(keep);
        }
        if ty == ct::UNIT_HEALTH && parms.len() == 3 && g.below(2) == 0 {
            // a threshold at the named unit's percentage, rounded or truncated
            let k = num(&parms[0].s, "unit");
            if (0..16).contains(&k) {
                let w = cfg[80 + 4 * k as usize + 1];
                let (h, i) = (((w >> 16) & 255) as f32, (w >> 24) as f32);
                if i > 0.0 {
                    parms[2].int = if g.below(2) == 0 { ((h * 100.0 + i / 2.0) / i) as i32 } else { (h * 100.0 / i) as i32 };
                }
            }
        }
        cs.push(C { ty, parms });
    }
    let start = cfg;
    let n_ops = 1 + g.below(60) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=64 => [0, g.below(nc as u32), 0],
            65..=69 => [1, 1, cfg[1].wrapping_add(1 + g.below(2))],
            70..=73 => [1, 2, cfg[2] + g.below(2)],
            74 => [1, 2, cfg[2].saturating_sub(1 + g.below(2))],
            75..=77 => [1, 3, cfg[3] + g.below(2)],
            78 => [2, 0, 0],
            79..=81 => [1, 14, cfg[14] ^ (1 << g.below(8))],
            82..=84 => {
                // a contain's count: emptied, or some aboard
                let j = g.below(16) as usize;
                let w = cfg[80 + 4 * j + 3] & !15 | if g.below(2) == 0 { 0 } else { 1 + g.below(4) };
                [1, (80 + 4 * j + 3) as u32, w]
            }
            85..=87 => {
                // the script engine's direct calls, on a condition whose parameters fit (a team first; a team and a button after it)
                let op = 3 + g.below(2);
                let fits: Vec<u32> = (0..nc as u32)
                    .filter(|i| {
                        let ks = kinds(cs[*i as usize].ty);
                        let n = cs[*i as usize].parms.len();
                        if op == 3 { ks.starts_with('T') && n >= 1 } else { ks == "STB" && n >= 3 }
                    })
                    .collect();
                if fits.is_empty() {
                    [0, g.below(nc as u32), 0]
                } else {
                    [op, fits[g.below(fits.len() as u32) as usize], g.below(2)]
                }
            }
            _ => {
                // mostly the players', teams' and objects' words, sometimes a seed, else any word
                let i = match g.below(10) {
                    0..=6 => 16 + g.below(128),
                    7 => 7 + g.below(3),
                    _ => g.below(NCFG as u32),
                };
                [1, i, cfg[i as usize] ^ (1 << g.below(32))]
            }
        };
        if o[0] == 1 {
            cfg[o[1] as usize] = o[2];
        }
        ops.push(o);
    }
    // (the generator follows the operations' changes; the program starts from the state before them)
    (start, cs, ops)
}

#[test]
fn script_conditions_match_upstream() {
    let cases: usize = std::env::var("CONDITIONS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let mut g = Gen::new(0x5C1C_0001);
    let (mut compared, mut cut, mut ops_run, mut evals, mut words, mut mismatches) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let mut by_type = [0u64; 110];
    let mut cut_why = std::collections::BTreeMap::<String, u64>::new();
    let mut direct = [0u64; 2];
    for case in 0..cases {
        let (cfg, cs, ops) = gen_program(&mut g);
        let (r, undefined_at) = rust_run(&cfg, &cs, &ops);
        let n = undefined_at.as_ref().map(|u| u.0).unwrap_or(ops.len());
        if let Some((_, why)) = &undefined_at {
            cut += 1;
            // "<condition type or directN> <the Undefined variant>"
            let (who, e) = why.split_once(' ').unwrap_or((why.as_str(), ""));
            let variant = e.split("Undefined(").nth(1).and_then(|r| r.split(')').next()).unwrap_or(e);
            let key = if variant == "NullParameter" { variant.to_string() } else { format!("{variant} (type {who})") };
            *cut_why.entry(key).or_insert(0u64) += 1;
        }
        let c = cpp_run(&cfg, &cs, &ops[..n]);
        compared += 1;
        ops_run += n as u64;
        for o in &ops[..n] {
            if o[0] == 0 {
                evals += 1;
                by_type[cs[o[1] as usize].ty as usize] += 1;
            }
            if o[0] == 3 || o[0] == 4 {
                direct[o[0] as usize - 3] += 1;
            }
        }
        if r != c {
            mismatches += 1;
            if mismatches <= 3 {
                let at = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
                println!(
                    "case {case}: conds {:?}\n ops {:?}\n  first difference at word {at}: rust {:x?} c++ {:x?}",
                    cs.iter().map(|c| (c.ty, c.parms.iter().map(|p| (String::from_utf8_lossy(&p.s).to_string(), p.int)).collect::<Vec<_>>())).collect::<Vec<_>>(),
                    &ops[..n],
                    &r[at.saturating_sub(4)..(at + 8).min(r.len())],
                    &c[at.saturating_sub(4)..(at + 8).min(c.len())]
                );
            }
        } else {
            words += r.len() as u64;
        }
    }
    let missing: Vec<usize> = (0..ct::NUM_ITEMS as usize).filter(|t| by_type[*t] == 0).collect();
    println!(
        "C1: {cases} programs ({cut} cut at an undefined operation), {compared} compared ({ops_run} ops, {evals} evaluations, {words} output words), {mismatches} mismatches; types never evaluated: {missing:?}"
    );
    println!("cut at: {cut_why:?}; direct calls: evaluateTeamIsContained {}, evaluateSkirmishCommandButtonIsReady {}", direct[0], direct[1]);
    assert_eq!(mismatches, 0);
    assert!(missing.is_empty(), "every condition type is evaluated");
    assert!(direct[0] > 0 && direct[1] > 0, "the generator must reach both direct calls");
}
