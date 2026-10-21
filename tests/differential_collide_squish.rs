#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/SquishCollide.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SquishCollide` (`logic::modules::squish_collide`) against the unchanged C++, `p1w13-sonnet`, update batch O.
//!
//! # Method
//!
//! The C++ side is `SquishCollide.cpp` (one mechanical edit, made by `build.rs` into `mod_gen`: the `static` of its function-local name key is removed, because
//! the run resets the name key generator for every case), compiled into `wrap_module.cpp`'s unit (`differential/l9_squish_collide_o.inc`) on the module build's
//! stand-ins: two `Object`s (the module's, and the crusher) with the REAL `GeometryInfo`, positions, orientations and a crusher level the test sets, the
//! crusher's physics (the real `PhysicsUpdate.h`'s `PhysicsBehavior`, a zeroed buffer with its velocity set), an AI with a goal object, a `HijackerUpdate` and a
//! `SpecialAbilityUpdate` (type, active) the test gives, the REAL `Object::getRelationship` (cut from `Object.cpp` by `build.rs`) over a `Team` whose relationship
//! is the test's, a body that logs the damage it is dealt, and the REAL `PartitionManager::geomCollidesWithGeom` (`PartitionManager.cpp`, compiled by
//! `wrap_partition.cpp`) reached through `ref_partition_geom_collides`.
//!
//! The Rust side is the real thing: a `World` with the two objects (probe modules for the AI, the physics, the hijacker and the special ability update;
//! a probe body on the module's object), the module from `modules::new_module`, `onCollide` as `World::module_call`, the relationship from
//! `cuts.team_relationships`, the collision test the translated `PartitionManager::geom_collides_with_geom`, and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (a `CollideModuleData` has no fields: a well formed block, a bogus line, a block that is not closed) and, when the read succeeds,
//! makes the module and runs 1 to 40 ops: collisions with the crusher or the ground, new positions, orientations and geometries (sphere, cylinder, box), the
//! crusher's velocity and crusher level, the AI's goal, a hijacker, a special ability update (active or not, TNT attack or another), the undetected-defector
//! flags, the relationship, the physics present or not, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; per op the throw status, the effects (`damage <object> <amount> <type> <death> <source>`), the xfer status, bytes and
//! CRC, the module's object's health and its undetected-defector flag.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9SQ_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 561 data
//! blocks threw (read compared: 9000 comparisons, 0 mismatches), 0 cases excluded, 171907 ops, 62284 collisions, 10431 xfers, 1511 squishes (a damage dealt);
//! ops: 188785 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run was clean; the generator was then tightened (positions closer, the
//! relationship more often hostile, more malformed blocks) because 826 squishes and 214 thrown blocks missed the coverage floors of the test.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1141 | the "moving toward the victim" test is dropped |
//! | 1142 | the death type is `DEATH_NORMAL` |
//! | 1143 | the victim stays an undetected defector |
//! | 1144 | a hijacker's target may crush it |
//! | 1145 | a TNT hunter placing a charge may be crushed |
//! | 1146 | the AI goal is not compared with the crusher |
//! | 1147 | the relationship is asked the usual way round |
//! | 1148 | a crusher level of 0 may crush |
//! | 1149 | the 1.0 crush radius is not applied |
//!
//! All 9 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE};
use rts_lockstep_engine::common::special_power_type::SPECIAL_TANKHUNTER_TNT_ATTACK;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9sq_o.bin");
const XFER_FILE: &str = "l9sq_o.bin";
const TEAM_A: u32 = 100;
const TEAM_B: u32 = 101;

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(rts_lockstep_engine::logic::modules::squish_collide::new_module_data(Some(ini))?);
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..2).map(|_| r.u32()).collect();
        out.push(format!("op {i}: status {st} log {log:?} {x} state {state:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut s = String::from("D\n");
    if !clean && g.below(8) == 0 {
        s.push_str(gen_indent(g));
        s.push_str(&format!("Bogus{}1\n", gen_sep(g)));
    }
    if clean || g.below(12) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn bits(x: f32) -> u32 {
    x.to_bits()
}

fn gen_coord(g: &mut Gen) -> f32 {
    *pick(g, &[&-3.0f32, &-1.5, &-1.0, &0.0, &0.5, &1.0, &2.0, &3.0, &6.0])
}

fn gen_z(g: &mut Gen) -> f32 {
    *pick(g, &[&0.0f32, &0.0, &1.0, &5.0, &-2.0, &12.0])
}

fn gen_angle(g: &mut Gen) -> f32 {
    *pick(g, &[&0.0f32, &0.5, &1.0, &1.5707964, &-1.0, &3.0, &-2.5, &0.25])
}

fn gen_dim(g: &mut Gen) -> f32 {
    *pick(g, &[&0.5f32, &1.0, &1.5, &2.0, &3.0, &5.0, &8.0])
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [self has an AI, self has a body, the health, self has a team, the crusher has a team, the crusher's crusher level]
    let cfg = vec![
        u32::from(g.below(100) < 80),
        u32::from(g.below(100) < 92),
        bits(*pick(g, &[&100.0f32, &1.0e7])),
        u32::from(g.below(100) < 90),
        u32::from(g.below(100) < 90),
        *pick(g, &[&0u32, &1, &1, &2, &3]),
    ];
    let n_ops = 1 + g.below(40) as usize;
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=34 => [0, u32::from(g.below(100) < 8), 0, 0, 0],
            35..=40 => [1, bits(gen_coord(g)), bits(gen_coord(g)), bits(gen_z(g)), 0],
            41..=46 => [2, bits(gen_coord(g)), bits(gen_coord(g)), bits(gen_z(g)), 0],
            47..=51 => [3, bits(gen_angle(g)), bits(gen_angle(g)), 0, 0],
            52..=57 => [4, g.below(3), bits(gen_dim(g)), bits(gen_dim(g)), bits(gen_dim(g))],
            58..=63 => [5, g.below(3), bits(gen_dim(g)), bits(gen_dim(g)), bits(gen_dim(g))],
            64..=72 => [6, bits(gen_coord(g)), bits(gen_coord(g)), bits(*pick(g, &[&0.0f32, &1.0])), 0],
            73..=78 => [7, g.below(3), 0, 0, 0],
            79..=83 => [8, g.below(2), g.below(3), g.below(2), 0],
            84..=87 => [9, g.below(2), g.below(2), 0, 0],
            88..=92 => [10, *pick(g, &[&0u32, &0, &1, &2]), u32::from(g.below(100) < 90), 0, 0],
            93..=94 => [11, 0, 0, 0, 0],
            95..=96 => [12, 0, 0, 0, 0],
            97..=98 => [13, 0, 0, 0, 0],
            _ => [0, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    collides: u64,
    squishes: u64,
}

fn geometry(t: u32, h: u32, major: u32, minor: u32) -> GeometryInfo {
    let ty = [GeometryType::Sphere, GeometryType::Cylinder, GeometryType::Box][t as usize % 3];
    GeometryInfo::new(ty, false, f32::from_bits(h), f32::from_bits(major), f32::from_bits(minor))
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let mut v = Vec::new();

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.teams.insert_team(Team::new(TEAM_A, 0)).unwrap();
    w.teams.insert_team(Team::new(TEAM_B, 0)).unwrap();
    // self: the module, a body, an AI
    let mut modules = Vec::new();
    if cfg[1] != 0 {
        let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
        m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
        m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), f32::from_bits(cfg[2]), Default::default())));
        modules.push(m);
    }
    if cfg[0] != 0 {
        let mut ai = NewModule::plain(b"ProbeAI", b"ModuleTag_AI");
        ai.is_ai = true;
        ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        modules.push(ai);
    }
    // a hijacker (absent: renamed) and a special ability update (absent: no type), probes whose answers the ops set
    let mut hij = NewModule::plain(b"XHijackerUpdate", b"ModuleTag_Hijacker");
    hij.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    modules.push(hij);
    let mut sa = NewModule::plain(b"SpecialAbilityUpdate", b"ModuleTag_SA");
    sa.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    modules.push(sa);
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SquishCollide", b"ModuleTag_Squish", &data).unwrap().unwrap());
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template, status: Default::default(), team: if cfg[3] != 0 { Some(TEAM_A) } else { None }, modules }).unwrap();
    assert_eq!(me, 1);
    let values = ObjectTemplateValues { crusher_level: cfg[5] as u8, ..Default::default() };
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let mut cmods = Vec::new();
    let mut phys = NewModule::plain(b"PhysicsBehavior", b"ModuleTag_Physics");
    phys.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    cmods.push(phys);
    let crusher = w.new_object(NewObject { template, status: Default::default(), team: if cfg[4] != 0 { Some(TEAM_B) } else { None }, modules: cmods }).unwrap();
    assert_eq!(crusher, 2);
    let idx = w.find_module(me, b"SquishCollide").unwrap();
    let mut saved: Vec<u8> = Vec::new();
    let mut physics_idx = w.objects.find(crusher).unwrap().physics;
    let mut excluded = false;
    let mut geom_self = (0u32, 0u32, 0u32, 0u32);
    let _ = &mut geom_self;
    for phase in 0..=ops.len() {
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc) = (-2i32, Vec::new(), 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    tally.borrow_mut().collides += 1;
                    let other = if o[1] != 0 { 0 } else { crusher };
                    w.module_call(me, idx, rts_lockstep_engine::logic::world::module_call::ModuleCall::OnCollide { other, loc: None, normal: None });
                }
                1 => w.set_position(me, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
                2 => w.set_position(crusher, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
                3 => {
                    w.set_orientation(me, f32::from_bits(o[1]));
                    w.set_orientation(crusher, f32::from_bits(o[2]));
                }
                4 => w.objects.find_mut(me).unwrap().geometry_info = geometry(o[1], o[2], o[3], o[4]),
                5 => w.objects.find_mut(crusher).unwrap().geometry_info = geometry(o[1], o[2], o[3], o[4]),
                6 => {
                    if let Some(i) = physics_idx.or(w.objects.find(crusher).unwrap().modules.iter().position(|m| m.name == b"PhysicsBehavior").map(|i| i as u16)) {
                        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(crusher).unwrap().modules[i as usize].body.as_mut() {
                            p.answers.velocity = Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) };
                        }
                    }
                }
                7 => {
                    let goal = [0, crusher, me][o[1] as usize % 3];
                    if let Some(i) = w.objects.find(me).unwrap().ai {
                        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[i as usize].body.as_mut() {
                            p.answers.goal_object = goal;
                        }
                    }
                }
                8 => set_extras(&mut w, me, o[1] != 0, o[2], o[3] != 0),
                9 => {
                    w.friend_set_undetected_defector(me, o[1] != 0);
                    w.friend_set_undetected_defector(crusher, o[2] != 0);
                }
                10 => {
                    let rel = [Relationship::Enemies, Relationship::Neutral, Relationship::Allies][o[1] as usize % 3];
                    w.cuts.team_relationships.insert((TEAM_B, TEAM_A), rel);
                    w.objects.find_mut(crusher).unwrap().physics = if o[2] != 0 { physics_idx } else { None };
                }
                11..=13 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 11, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 11 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                _ => unreachable!(),
            }
        }
        let _ = &mut physics_idx;
        if w.cuts.module_errors.len() > errors_before {
            assert!(w.cuts.module_errors[errors_before..].iter().all(|(_, e)| e.is_undefined()), "{:?}", w.cuts.module_errors);
            excluded = true;
        }
        let trace = w.cuts.trace.take().unwrap_or_default();
        let log: Vec<String> = trace.into_iter().filter(|t| t.starts_with("damage ")).collect();
        tally.borrow_mut().squishes += log.len() as u64;
        tally.borrow_mut().effects += log.len() as u64;
        put_i32(&mut v, 0);
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let o = w.objects.find(me).unwrap();
        let health = o.modules.iter().find_map(|b| match (b.name.as_slice(), b.body.as_ref()) {
            (b"ProbeBody", Some(UpdateBody::ModuleProbe(p))) => Some(p.health.to_bits()),
            _ => None,
        });
        put_u32(&mut v, health.unwrap_or(0));
        put_u32(&mut v, u32::from(o.is_undetected_defector()));
    }
    RunOut { dump: v, keys: Vec::new(), excluded }
}

/// Op 8: the module's object gets (or loses) a `HijackerUpdate` and a `SpecialAbilityUpdate` (power type 0 none, 1 TNT attack, 2 another; active). Both probes
/// are made with the object; the first is renamed to be absent, the second answers no type to be absent.
fn set_extras(w: &mut World, me: i32, hijacker: bool, ability: u32, active: bool) {
    let o = w.objects.find_mut(me).unwrap();
    for m in o.modules.iter_mut() {
        if m.name == b"HijackerUpdate" || m.name == b"XHijackerUpdate" {
            m.name = if hijacker { b"HijackerUpdate".to_vec() } else { b"XHijackerUpdate".to_vec() };
        }
        if m.name == b"SpecialAbilityUpdate" {
            if let Some(UpdateBody::ModuleProbe(p)) = m.body.as_mut() {
                p.answers.has = if ability == 0 { 0 } else { 1 << 9 };
                p.answers.special_power_type = if ability == 1 { SPECIAL_TANKHUNTER_TNT_ATTACK } else { 3 };
                p.answers.yes = active;
            }
        }
    }
}

#[test]
fn squish_collide_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "collide module O3",
        seed: 0x00C0_1115,
        cases_var: "L9SQ_CASES",
        ini_stem: "Sq",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9sq_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "collide module O3: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} collisions, {} xfers, {} squishes",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.collides, t.xfers, t.squishes
    );
    assert!(c.read_failed * 40 > c.cases as u64 && c.ops_run * 5 > c.cases as u64 * 4 && t.squishes * 10 > c.cases as u64, "the generator must reach every path");
}
