#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpectreGunshipDeploymentUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SpectreGunshipDeploymentUpdate` (`logic::modules::spectre_gunship_deployment_update`) against the unchanged C++, `p1w13-sonnet`, batch R60.
//!
//! # Method
//!
//! The C++ side is `SpectreGunshipDeploymentUpdate.cpp`, unchanged, compiled into `wrap_module.cpp`'s unit
//! (`differential/l9_spectre_deploy_r60.inc`, `ref_l9r60_run`), on the module build's stand-ins: the command centre (maybe a special power
//! module: a REAL `SpecialPowerModule` of the store's `SP_0`..`SP_3`, as upstream casts the interface to one, whose `setReadyFrame`,
//! `markSpecialPowerTriggered` and `doSpecialPowerAtLocation` are lines; sold, under construction, effectively dead or not; a team of three,
//! with its player, whose sciences are the test's), the thing factory (`GunshipA`: an AI and a special power module; `GunshipB`: an AI;
//! `GunshipC`: neither; the AI's current locomotor and its preferred height the test's), the terrain's edge points (offsets), the game logic's
//! `selectObject` (a line), the REAL special power store, `UpdateModule`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the special power store and a science store (`SCIENCE_A`, `SCIENCE_B`), the thing
//! factory's three templates (their modules untranslated, so made as probes through `cuts.untranslated_probe`: `...AIUpdate` the AI), three
//! teams and players, the centre's power a probe; the gunships are made by the real `World::thing_factory_new_object` and placed by the real
//! `set_position` and `set_orientation`. Every op an `update()` as the scheduler runs it, `initiateIntentToDoSpecialPower` (a template, a
//! target position or none, the script flag and other options), `doesSpecialPowerUpdatePassScienceTest`, a change to the scene or the frame,
//! or an xfer.
//!
//! The harness (`tests/harness`) runs the cases: a block `D` (mostly well formed), 1 to 40 ops. Compared, bit for bit: the read's throw; the
//! data and the constructor's state; per op the throw status, the result, the effects in order (the ready frame, the gunship's power, the
//! selection, the centre's power), the xfer status, bytes (the scheduling word masked) and CRC, the gunship's id and the target position, and
//! the number of gunships with the last one's position, orientation, producer and team.
//!
//! # Results (2026-10-25, `L9R60_CASES=9000`)
//!
//! 9,000 cases (283 data blocks threw, as the C++ did; 3,278 excluded as undefined upstream: 1,537 an intent with the centre's power module
//! never found, 822 a `NULL` target position, 645 a gunship with no AI, 131 one whose AI has no current locomotor, 143 a centre with no
//! controlling player at the selection), 104,041 ops and 114,919 outputs: **0 mismatches**. In the compared cases: 10,472 updates, 20,885
//! xfers and 70,089 effect lines; 35,983 intents, 20,028 taken (each a gunship placed and selected), 9,226 science passes and 8,690 throws
//! of `newObject(NULL)` (an unknown gunship template). The first runs differed only in the test's own bookkeeping (the C++ stand-in numbered
//! gunships from 4, and the last one was read in id order).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 7 of 7 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 2296 | the line to the edge not normalised before it is scaled | 62,535 |
//! | 2297 | a script-fired power not made ready at once | 7,143 |
//! | 2298 | the edge nearest the source taken as the farthest | 10,522 |
//! | 2299 | the gunship added to the selection, not made the selection | 20,028 |
//! | 2300 | the gunship's power marked but not fired | 11,445 |
//! | 2301 | the gunship's id not saved | 13,402 |
//! | 2302 | an effectively dead centre stays awake | 409 |

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniErrorKind, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::logic::module::behavior_module::BehaviorModuleData;
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore, ScienceType};
use rts_lockstep_engine::common::special_power::SpecialPowerStore;
use rts_lockstep_engine::common::thing_factory::ThingFactory;
use rts_lockstep_engine::common::thing_template::{ModuleInfoData, ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::spectre_gunship_deployment_update::{SpectreGunshipDeploymentUpdateModuleData, COMMAND_FIRED_BY_SCRIPT};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r60.bin");
const XFER_FILE: &str = "l9_r60.bin";

const POWERS: &str = "SpecialPower SP_0\nEnd\nSpecialPower SP_1\nEnd\nSpecialPower SP_2\nEnd\nSpecialPower SP_3\nEnd\n";
const SCIENCES: [&str; 2] = ["SCIENCE_A", "SCIENCE_B"];
const GUNSHIPS: [&str; 3] = ["GunshipA", "GunshipB", "GunshipC"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpectreGunshipDeploymentUpdate(Arc::new(SpectreGunshipDeploymentUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

fn template_id(store: &SpecialPowerStore, name: &[u8]) -> u32 {
    store.find_special_power_template(name).map_or(0, |t| t.id())
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let sp = r.u32();
    let (sci, ship) = (r.str(), r.str());
    let rest: Vec<u32> = (0..3).map(|_| r.u32()).collect();
    let constructed: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    out.push(format!("data sp {sp} science {sci:?} gunship {ship:?} {rest:x?} constructed {constructed:x?}"));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let (st, res) = (r.u32() as i32, r.u32());
        let n = r.u32();
        let log: Vec<String> = (0..n).map(|_| r.str()).collect();
        // (the scheduling half's frame-and-phase word masked: the C++ stand-in has no scheduler)
        let x = r.xfer();
        let state: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        let ships = r.u32();
        let last: Vec<u32> = if ships > 0 { (0..6).map(|_| r.u32()).collect() } else { Vec::new() };
        out.push(format!("op {i}: status {st} result {res:#x} log {log:?} {x} state {state:x?} ships {ships} last {last:x?}"));
    }
    out
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, choices: &[&str], strict: bool| {
        if g.below(100) < p {
            let v = if !clean && !strict && g.below(15) == 0 { pick(g, &["x", "-3"]).to_string() } else { pick(g, choices).to_string() };
            lines.push(format!("{name} = {v}"));
        }
    };
    field(g, 97, "SpecialPowerTemplate", &["SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_0", "SP_1"], true);
    field(g, 85, "GunshipTemplateName", &["GunshipA", "GunshipA", "GunshipA", "GunshipA", "GunshipB", "GunshipB", "GunshipC", "NoSuchShip"], true);
    field(g, 25, "RequiredScience", &["SCIENCE_A", "SCIENCE_B"], true);
    field(g, 40, "AttackAreaRadius", &["100", "250.5", "0"], false);
    field(
        g,
        70,
        "CreateLocation",
        &["CREATE_AT_EDGE_NEAR_SOURCE", "CREATE_AT_EDGE_FARTHEST_FROM_SOURCE", "CREATE_AT_EDGE_NEAR_TARGET", "CREATE_AT_EDGE_FARTHEST_FROM_TARGET"],
        false,
    );
    if !clean && g.below(20) == 0 {
        lines.push("Bogus = 1".to_string());
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

fn gen_coord(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &5.0, &12.5, &-20.0, &30.0, &60.0, &-300.0, &450.0])).to_bits()
}

/// The centre's flags but its power (bits 4 up: sold, under construction, dead, the team, the sciences).
fn gen_centre(g: &mut Gen) -> u32 {
    let p = |g: &mut Gen, pct: u32| u32::from(g.below(100) < pct);
    p(g, 8) << 4 | p(g, 8) << 5 | p(g, 5) << 6 | (if g.below(40) == 0 { 0 } else { 1 + g.below(3) }) << 7 | p(g, 50) << 9 | p(g, 50) << 10
}

fn gen_ship(g: &mut Gen) -> u32 {
    u32::from(g.below(40) != 0) | (if g.below(5) == 0 { g.below(4) } else { 0 }) << 1
}

fn gen_cfg_word(g: &mut Gen, a: u32) -> u32 {
    match a {
        1 => gen_centre(g),
        2 => gen_ship(g),
        3 => (*pick(g, &[&0.0f32, &100.0, &250.0, &33.25])).to_bits(),
        _ => gen_coord(g),
    }
}

fn gen_case(g: &mut Gen, _data: &Option<ModuleDataKind>, _n_up: usize, _upgrades: &[String]) -> (Vec<u32>, Vec<[u32; 5]>) {
    let mut cfg = vec![0u32; 12];
    cfg[0] = *pick(g, &[&0u32, &1, &5, &100]);
    // the centre's power: mostly SP_0 (the data's usual template)
    let power = if g.below(25) == 0 { 0 } else { 1 | (if g.below(12) == 0 { g.below(4) } else { 0 }) << 1 };
    cfg[1] = power | gen_centre(g);
    for (a, c) in cfg.iter_mut().enumerate().skip(2) {
        *c = gen_cfg_word(g, a as u32);
    }
    let n_ops = 1 + g.below(40) as usize;
    let mut ops = Vec::new();
    while ops.len() < n_ops {
        let o = match g.below(100) {
            0..=9 => [0, 0, 0, 0, 0],
            10..=14 => [1, gen_frame(g), 0, 0, 0],
            15..=34 => {
                let a = 1 + g.below(11);
                [2, a, gen_cfg_word(g, a), 0, 0]
            }
            35..=69 => {
                let k = if g.below(5) == 0 { *pick(g, &[&1u32, &2, &7]) } else { 0 };
                let flags = u32::from(g.below(40) != 0) | u32::from(g.below(4) == 0) << 1 | (if g.below(5) == 0 { 1 + g.below(3) } else { 0 }) << 2;
                [3, k << 8 | flags, gen_coord(g), gen_coord(g), 0]
            }
            70..=79 => [4, 0, 0, 0, 0],
            80..=87 => [5, 0, 0, 0, 0],
            88..=95 => [6, 0, 0, 0, 0],
            _ => [7, 0, 0, 0, 0],
        };
        ops.push(o);
    }
    (cfg, ops)
}

#[derive(Default, Clone)]
struct Tally {
    updates: u64,
    xfers: u64,
    effects: u64,
    intents: u64,
    taken: u64,
    gunships: u64,
    selects: u64,
    science: u64,
    threw: u64,
    excluded: std::collections::BTreeMap<String, u64>,
}

struct Text;
impl GameTextFetch for Text {
    fn fetch(&self, _label: &[u8]) -> UnicodeString {
        UnicodeString::new()
    }
}
struct SciCtx {
    store: ScienceStore,
    names: NameKeyGenerator,
}
fn science_block(ini: &mut Ini<'_>, c: &mut SciCtx) -> IniResult<()> {
    c.store.parse_science_definition(ini, &mut c.names, &Text)
}

fn sp_k(v: u32) -> u32 {
    if v <= 3 {
        v
    } else {
        0
    }
}

fn set_scene(w: &mut World, cfg: &[u32], store: &SpecialPowerStore) {
    let f = cfg[1];
    {
        let o = w.objects.find_mut(1).unwrap();
        o.status = Default::default();
        o.status.set(OBJECT_STATUS_SOLD, (f >> 4 & 1) as i32);
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, (f >> 5 & 1) as i32);
        if f >> 6 & 1 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        o.team = f >> 7 & 3;
    }
    let keys: Vec<ScienceType> = (0..2).filter(|k| f >> (9 + k) & 1 != 0).map(|k| ScienceType::from_name_key(w.names.name_to_key(SCIENCES[k as usize].as_bytes()))).collect();
    for p in 0..3 {
        w.players.get_nth_mut(p).unwrap().sciences = keys.clone();
    }
    w.set_position(1, &Coord3D { x: f32::from_bits(cfg[4]), y: f32::from_bits(cfg[5]), z: 0.0 });
    w.cuts.edge_point_offset = Coord3D { x: f32::from_bits(cfg[6]), y: f32::from_bits(cfg[7]), z: f32::from_bits(cfg[8]) };
    w.cuts.farthest_edge_point_offset = Coord3D { x: f32::from_bits(cfg[9]), y: f32::from_bits(cfg[10]), z: f32::from_bits(cfg[11]) };
    let ship_power = template_id(store, format!("SP_{}", sp_k(cfg[2] >> 1 & 7)).as_bytes());
    w.cuts.untranslated_probe =
        Some(ProbeAnswers { has_locomotor: cfg[2] & 1 != 0, preferred_height: f32::from_bits(cfg[3]), power_template: ship_power, ..Default::default() });
}

/// The effects of the trace since it was last taken, as the C++ effect log writes them.
fn effects(w: &mut World, tally: &mut Tally) -> Vec<String> {
    let mut cut_iter = w.cuts.log.take().unwrap().into_iter();
    let mut log = Vec::new();
    for t in w.cuts.trace.take().unwrap() {
        if t.starts_with("cut ") {
            if let Some(CutCall::GameLogicSelectObject { object, create_new_selection, player_mask, affect_client }) = cut_iter.next() {
                tally.selects += 1;
                log.push(format!("select {object} {} {player_mask} {}", i32::from(create_new_selection), i32::from(affect_client)));
            }
        } else if let Some(l) = t.strip_prefix("bt ") {
            if l.starts_with("readyframe ") || l.starts_with("triggered ") || l.starts_with("poweratloc ") {
                log.push(l.to_string());
            }
        }
    }
    log
}

fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg_in: &[u32], ops: &[[u32; 5]], tally: &RefCell<Tally>) -> RunOut {
    let tally_before = tally.borrow().clone();
    let mut t = tally.borrow_mut();
    let mut cfg = cfg_in.to_vec();
    let mut v = Vec::new();
    let ModuleDataKind::SpectreGunshipDeploymentUpdate(d) = &data else { unreachable!() };
    put_u32(&mut v, d.special_power_template.as_deref().map_or(0, |n| template_id(&ctx.store, n)));
    put_str(&mut v, d.extra_required_science.as_deref().unwrap_or(b""));
    put_str(&mut v, &d.gunship_template_name);
    put_u32(&mut v, d.attack_area_radius.to_bits());
    put_u32(&mut v, d.gunship_orbit_radius.to_bits());
    put_i32(&mut v, d.create_loc);

    // the sciences the store knows
    let mut sc = SciCtx { store: ScienceStore::new(), names: ctx.names };
    let defs: String = SCIENCES.iter().map(|s| format!("Science {s}\nEnd\n")).collect();
    static BLOCKS: [BlockParse<SciCtx>; 1] = [BlockParse { token: "Science", parse: science_block }];
    Ini::load(b"Data\\INI\\Science.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut sc).expect("sciences load");

    let store = ctx.store.clone();
    let mut factory = ThingFactory::new();
    for name in GUNSHIPS {
        factory.new_template(name.as_bytes(), 0).expect("a template");
    }
    let plain = || ModuleInfoData::Logic(ModuleDataKind::Behavior(Arc::new(BehaviorModuleData::default())));
    for (name, ai, power) in [("GunshipA", true, true), ("GunshipB", true, false)] {
        let info = factory.friend_find_template_mut(name.as_bytes()).unwrap().data.friend_behavior_module_info_mut();
        if power {
            info.add_module_info(b"R60Power", b"ModuleTag_Power", plain(), MODULEINTERFACE_SPECIAL_POWER, false, false);
        }
        if ai {
            info.add_module_info(b"R60GunshipAIUpdate", b"ModuleTag_AI", plain(), 0, false, false);
        }
    }
    let mut w = World::new();
    w.names = sc.names;
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        science_store: sc.store,
        thing_factory: factory,
        ..Default::default()
    });
    w.set_frame(cfg[0]);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.probe_power_trace = true;
    w.cuts.body.probe_trace = true;
    w.players.set_player_count(3);
    for t in 1..=3u32 {
        w.teams.insert_team(Team::new(t, 0)).unwrap();
        w.cuts.team_players.insert(t, (t - 1) as i32);
        w.cuts.player_default_teams.insert((t - 1) as i32, t);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let mut modules = Vec::new();
    if cfg[1] & 1 != 0 {
        let power_template = template_id(&store, format!("SP_{}", sp_k(cfg[1] >> 1 & 7)).as_bytes());
        let mut pm = NewModule::plain(b"R60Power", b"ModuleTag_Power");
        pm.interfaces = MODULEINTERFACE_SPECIAL_POWER;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { power_template, ..Default::default() }, 0.0, Default::default())));
        modules.push(pm);
    }
    modules.push(rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"SpectreGunshipDeploymentUpdate", b"ModuleTag_Deploy", &data).unwrap().unwrap());
    let me = w.new_object(NewObject { template, status: Default::default(), team: None, modules }).unwrap();
    assert_eq!(me, 1);
    let idx = w.find_module(me, b"SpectreGunshipDeploymentUpdate").unwrap();
    let u = w.update_handle_of_module(ModuleHandle { object: me, index: idx }).expect("an update module");
    set_scene(&mut w, &cfg, &store);
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    let state = |w: &World| -> Vec<u32> {
        let ob = w.objects.find(me).unwrap();
        let Some(UpdateBody::SpectreGunshipDeploymentUpdate(m)) = ob.behaviors[u.index as usize].body.as_ref() else { panic!("the module is back in its slot") };
        m.state_words()
    };
    for x in state(&w) {
        put_u32(&mut v, x);
    }
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        if o[0] == 2 && (1..=11).contains(&o[1]) {
            cfg[o[1] as usize] = if o[1] == 1 { (o[2] & !15) | (cfg[1] & 15) } else { o[2] };
        }
        if o[0] == 1 {
            w.set_frame(o[1]);
        }
        set_scene(&mut w, &cfg, &store);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                t.updates += 1;
            }
            3 => {
                let k = o[1] >> 8;
                let template = if k <= 3 { template_id(&store, format!("SP_{k}").as_bytes()) } else { 0 };
                let target_position = (o[1] & 1 != 0).then(|| Coord3D { x: f32::from_bits(o[2]), y: f32::from_bits(o[3]), z: 0.0 });
                let command_options = (if o[1] >> 1 & 1 != 0 { COMMAND_FIRED_BY_SCRIPT } else { 0 }) | (o[1] & 0xff) >> 2;
                let call = ModuleCall::InitiateIntentToDoSpecialPower { template, target_object: 0, target_position, waypoint: 0, command_options };
                res = u32::from(w.module_call(me, idx, call).as_bool());
                t.intents += 1;
                t.taken += u64::from(res);
            }
            4 => {
                res = u32::from(w.module_call(me, idx, ModuleCall::DoesSpecialPowerUpdatePassScienceTest).as_bool());
                t.science += u64::from(res);
            }
            5..=7 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 5, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 5 {
                    saved = xbytes.clone();
                }
                t.xfers += 1;
            }
            1 | 2 => {}
            _ => unreachable!(),
        }
        let new_errors = &w.cuts.module_errors[errors_before..];
        let threw = new_errors.iter().find(|(_, e)| !e.is_undefined());
        // (`ThingFactory::newObject(NULL)` throws `ERROR_BAD_ARG`, an `ErrorCode`: `CATCH_ALL`'s status 3)
        let st = match threw {
            Some((_, e)) if matches!(e.kind, IniErrorKind::BadArg) => 3,
            Some(_) => 2,
            None => 0,
        };
        t.threw += u64::from(st != 0);
        put_i32(&mut v, st);
        put_u32(&mut v, res);
        let log = effects(&mut w, &mut t);
        t.effects += log.len() as u64;
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
        for s in state(&w) {
            put_u32(&mut v, s);
        }
        let ships: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&id| id > 1).collect();
        t.gunships = t.gunships.max(ships.len() as u64);
        put_u32(&mut v, ships.len() as u32);
        if let Some(&s) = ships.iter().max() {
            let ob = w.objects.find(s).unwrap();
            let p = ob.get_position();
            for x in [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), ob.thing.orientation().to_bits(), ob.producer_id as u32, ob.team] {
                put_u32(&mut v, x);
            }
        }
    }
    drop(t);
    let excluded = match w.cuts.module_errors.iter().find(|(_, e)| e.is_undefined()) {
        Some((_, e)) => {
            let reason = format!("{e}");
            let mut t = tally.borrow_mut();
            *t = tally_before;
            *t.excluded.entry(reason).or_insert(0) += 1;
            true
        }
        None => false,
    };
    RunOut { dump: v, keys: Vec::new(), excluded }
}

#[test]
fn spectre_gunship_deployment_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    EXTRA_DEFS.with(|e| *e.borrow_mut() = POWERS.to_string());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "R60 SpectreGunshipDeploymentUpdate",
        seed: 0x0060_5BEC,
        cases_var: "L9R60_CASES",
        ini_stem: "Sg",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9r60_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "R60: {} cases, {} data blocks threw, {} excluded, {} ops; in the compared cases {} updates, {} xfers, {} effect lines ({} intents, {} \
         taken, {} selections, {} science passes, {} throws, at most {} gunships)",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.intents, t.taken, t.selects, t.science, t.threw, t.gunships
    );
    println!("excluded by: {:?}", t.excluded);
    assert!(t.taken > 0 && t.selects > 0 && t.science > 0 && t.threw > 0, "the generator must reach every effect");
}
