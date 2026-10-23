#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/Sabotage*CrateCollide.cpp, SabotageFakeBuilding.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The eight sabotage crates (`logic::modules::sabotage_crate_collide`) against the unchanged C++, `p1w13-sonnet`, batch R13.
//!
//! # Method
//!
//! The C++ side is the eight `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_sabotage_r13.inc`; the two thieves with
//! `Lib/BaseType.h`'s `min` macro) on the module build's stand-ins: a saboteur (id 1) with the crate module and maybe an AI with a goal, a victim
//! building (id 2: the test's kinds, player, drawable, a body that logs damage, special power modules that log their recharge, a REAL `OCLUpdate`, REAL
//! `SpyVisionUpdate`s, a contain holding two hackers, ids 3 and 4), three players (money, energy, brown-out and score that log; player 0 is neutral), the
//! local player, and a radar, an Eva, an in-game UI and an audio manager that log.
//!
//! The Rust side is the real thing: a `World` with the same objects (the crate module from `modules::new_module`; `ProbeModule`s for the saboteur's
//! AI and the victim's body, special powers and contain; the real `OCLUpdate` and `SpyVisionUpdate`), `cuts.crate_stub_object_calls` set (the victim's
//! `setDisabledUntil` and `attemptDamage` are records, as the driver's are lines), and every op a `World::module_call` or a change to the scene.
//!
//! Each case reads a `D` block (the `CrateCollide` fields and the class's own: `SabotageDuration`, `SabotagePowerDuration` or `StealCashAmount`; mostly
//! well formed) and runs 1 to 14 ops. Compared, bit for bit: the read's throw; the data; per op the throw status, a result, the effects in order
//! (`infiltrate`, `audio`, `flash`, `eva`, `recharge`, `powersabotage`, `brownout`, `disableduntil`, `withdraw`, `deposit`, `earned`, `float`,
//! `damage`, `fx`, `destroy`), the xfer status, bytes and CRC, the `OCLUpdate`'s remaining frames, and each `SpyVisionUpdate`'s reset flag and
//! disabled-until frame.
//!
//! # Results (2026-10-23, `L9SAB_CASES=9000`)
//!
//! 9,000 cases (8,894 compared, 106 excluded as undefined upstream; 1,808 data blocks threw, as the C++ did), 53,470 ops (21,866 `onCollide`
//! calls, 1,478 crates used, 7,989 xfers) and 67,642 op outputs: **0 mismatches**. Effect lines compared: infiltrate 1,470, audio 1,343, flash 918,
//! recharge 639, disableduntil 615, eva 361, float 276, brownout 199, powersabotage 199, withdraw / deposit / earned 138 each, damage 122.
//!
//! The first 9,000-case run found one translation defect: the Eva message was played only when there was a local player, where upstream's
//! `isLocallyControlled` compares the two players and so is true for an unowned victim when there is no local player (`NULL == NULL`).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 14 of 15 caught. 1646 is unobservable: `CrateCollide::isValidToExecute` (the base,
//! called first) already refuses a dead `other`, so the sabotage classes' own dead test cannot be reached. 1648 is caught by 15 mismatches in the
//! first 30 cases; the full run then ends in a C++ segfault, on a case that the planted Rust no longer excludes as undefined (the
//! crash's line was not traced).
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1641 | the shut-down buildings play the withdrawal sound | 583 |
//! | 1642 | no flash | 918 |
//! | 1643 | the special powers not restarted | 342 |
//! | 1644 | the whole `StealCashAmount` taken, whatever the victim has | 182 |
//! | 1645 | the loss text at the gain text's height | 138 |
//! | 1646 | a dead victim accepted | unobservable |
//! | 1647 | an aircraft carrier sabotaged as a factory | 35 |
//! | 1648 | the relationship test inverted | 15 in 30 cases |
//! | 1649 | the AI's goal ignored | 306 |
//! | 1650 | the power sabotaged until now | 120 |
//! | 1651 | the internet centres' spy vision left on | 239 |
//! | 1652 | the hackers not disabled | 126 |
//! | 1653 | the drop zone's timer not reset | 167 |
//! | 1654 | the fake building takes half its health | 91 |
//! | 1655 | `isSabotageBuildingCrateCollide` false | 4,052 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY, MODULEINTERFACE_CONTAIN, MODULEINTERFACE_SPECIAL_POWER};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::upgrade::UpgradeCenter;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::sabotage_crate_collide::{SabotageCrateCollideModuleData, SabotageVictim};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{EvaMessage, ObjectClientEvent, ObjectSound, RadarCall};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9sab_r13.bin");
const XFER_FILE: &str = "l9sab_r13.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;

/// The kinds a victim is given, by bit (the driver's `g_sabKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 16] = [
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_PARACHUTE, "PARACHUTE"),
    (KINDOF_COMMANDCENTER, "COMMANDCENTER"),
    (KINDOF_FS_POWER, "FS_POWER"),
    (KINDOF_FS_INTERNET_CENTER, "FS_INTERNET_CENTER"),
    (KINDOF_FS_BARRACKS, "FS_BARRACKS"),
    (KINDOF_FS_WARFACTORY, "FS_WARFACTORY"),
    (KINDOF_FS_AIRFIELD, "FS_AIRFIELD"),
    (KINDOF_AIRCRAFT_CARRIER, "AIRCRAFT_CARRIER"),
    (KINDOF_FS_SUPERWEAPON, "FS_SUPERWEAPON"),
    (KINDOF_FS_STRATEGY_CENTER, "FS_STRATEGY_CENTER"),
    (KINDOF_FS_SUPPLY_CENTER, "FS_SUPPLY_CENTER"),
    (KINDOF_FS_SUPPLY_DROPZONE, "FS_SUPPLY_DROPZONE"),
    (KINDOF_FS_FAKE, "FS_FAKE"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_VEHICLE, "VEHICLE"),
];
/// The kind bits each class looks for (so that the generator mostly gives the victim the right one).
const CLASS_KIND_BITS: [u32; 8] = [1 << 2, 1 << 13, 1 << 4, (1 << 5) | (1 << 6) | (1 << 7), 1 << 3, (1 << 9) | (1 << 10), 1 << 11, 1 << 12];

fn put_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_i32(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    put_u32(v, s.len() as u32);
    v.extend_from_slice(s);
}

#[derive(Default)]
struct FxIds(BTreeMap<Vec<u8>, i32>);
impl FxIds {
    fn id(&mut self, name: &Option<Vec<u8>>) -> i32 {
        match name {
            None => 0,
            Some(n) => {
                let next = self.0.len() as i32 + 1;
                *self.0.entry(n.clone()).or_insert(next)
            }
        }
    }
}

fn victim_of(kind: usize) -> SabotageVictim {
    SabotageVictim::ALL[kind].0
}

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    out.1 = Some(ModuleDataKind::SabotageCrateCollide(Arc::new(SabotageCrateCollideModuleData::friend_new_module_data(victim_of(out.0), Some(ini))?)));
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

/// The data dump of `ref_l9sab_run`: the crate base, then the class's own field (0 for none).
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::SabotageCrateCollide(d) = data else { unreachable!("not a sabotage crate") };
    let c = &d.crate_data;
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of.test(i)));
    }
    for i in 0..KINDOF_COUNT {
        put_u32(v, u32::from(c.kind_of_not.test(i)));
    }
    put_u32(v, u32::from(c.is_forbid_owner_player));
    put_u32(v, u32::from(c.is_building_pickup));
    put_u32(v, u32::from(c.is_human_only_pickup));
    let sci = SCIENCES.iter().position(|s| s.as_bytes().eq_ignore_ascii_case(&c.pickup_science)).map_or(-1, |i| i as i32);
    put_i32(v, sci);
    put_i32(v, fx.id(&c.execute_fx));
    put_str(v, &c.execution_animation_template);
    put_u32(v, c.execute_animation_display_time_in_seconds.to_bits());
    put_u32(v, c.execute_animation_z_rise_per_second.to_bits());
    put_u32(v, u32::from(c.execute_animation_fades));
    put_u32(
        v,
        match d.victim {
            SabotageVictim::InternetCenter | SabotageVictim::MilitaryFactory | SabotageVictim::PowerPlant => d.sabotage_frames,
            SabotageVictim::SupplyCenter | SabotageVictim::SupplyDropzone => d.steal_cash_amount,
            _ => 0,
        },
    );
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut p = 0usize;
    let mut out = Vec::new();
    let u32_ = |p: &mut usize| -> u32 {
        let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap());
        *p += 4;
        v
    };
    let str_ = |p: &mut usize| -> String {
        let n = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap()) as usize;
        *p += 4;
        let s = String::from_utf8_lossy(&b[*p..*p + n]).into_owned();
        *p += n;
        s
    };
    if b.is_empty() {
        return out;
    }
    let kinds: Vec<u32> = (0..2 * KINDOF_COUNT).map(|_| u32_(&mut p)).collect();
    let set: Vec<usize> = kinds.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, _)| i).collect();
    let flags = [u32_(&mut p), u32_(&mut p), u32_(&mut p)];
    let sci = u32_(&mut p) as i32;
    let fx = u32_(&mut p) as i32;
    let anim = str_(&mut p);
    let (t, z, fades) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let own = u32_(&mut p);
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades} own {own}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p) as i32;
        let n = u32_(&mut p);
        let log: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        let crc = u32_(&mut p);
        let ocl = u32_(&mut p);
        let n_spy = u32_(&mut p);
        let spies: Vec<(u32, u32)> = (0..n_spy).map(|_| (u32_(&mut p), u32_(&mut p))).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} ocl {ocl} spies {spies:?}"));
    }
    out
}

// -- generators --

fn kind_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = KINDS[g.below(16) as usize].1;
        t.push(match g.below(10) {
            0 => "NONE".to_string(),
            1..=6 => format!("+{n}"),
            _ => format!("-{n}"),
        });
    }
    t.join(" ")
}

/// The block `D`: the class's own field and the crate fields (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let own = match victim_of(kind) {
        SabotageVictim::InternetCenter | SabotageVictim::MilitaryFactory => Some("SabotageDuration"),
        SabotageVictim::PowerPlant => Some("SabotagePowerDuration"),
        SabotageVictim::SupplyCenter | SabotageVictim::SupplyDropzone => Some("StealCashAmount"),
        _ => None,
    };
    if let Some(f) = own {
        if g.below(100) < 85 {
            lines.push(format!("{f} = {}", if f == "StealCashAmount" { gen_int(g, clean) } else { pick(g, &["0", "33", "1000", "5000", "-5"]).to_string() }));
        }
    }
    if g.below(100) < 8 {
        lines.push(format!("RequiredKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 8 {
        lines.push(format!("ForbiddenKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 10 {
        lines.push(format!("ForbidOwnerPlayer = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 40 {
        lines.push(format!("BuildingPickup = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 8 {
        lines.push(format!("HumanOnly = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 30 {
        lines.push(format!("ExecuteFX = {}", pick(g, &["FX_A", "FX_B", "None"])));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "RequiredKindOf = +BOGUS".to_string(),
            1 => "BuildingPickup = Perhaps".to_string(),
            _ => "NoSuchField = 1".to_string(),
        });
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(l);
        s.push('\n');
    }
    s.push_str("End\n");
    s.into_bytes()
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    collides: u64,
    destroyed: u64,
    xfers: u64,
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

fn run_xfer(w: &mut World, me: ObjectId, idx: usize, how: u32, saved: &[u8]) -> (i32, Vec<u8>, u32) {
    fn status<T>(r: Result<T, rts_lockstep_engine::common::xfer::XferStatus>) -> i32 {
        match r {
            Ok(_) => 0,
            Err(e) => e as i32,
        }
    }
    match how {
        0 => {
            let mut x = XferSave::new();
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, x.bytes().to_vec(), 0)
        }
        1 => {
            let mut b = saved.to_vec();
            b.truncate(512);
            let mut x = XferLoad::new(b);
            let st = match x.open(XFER_FILE.as_bytes()) {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), 0)
        }
        _ => {
            let mut x = XferCrc::new_without_nan_guard();
            let st = match x.open(b"crc") {
                Ok(()) => status(w.xfer_module_slot(me, idx, &mut x)),
                Err(e) => e as i32,
            };
            let _ = x.close();
            (st, Vec::new(), x.get_crc())
        }
    }
}

fn set_player_of_team(w: &mut World, team: u32, player: Option<u32>) {
    match player {
        Some(p) => {
            w.cuts.team_players.insert(team, p as i32);
        }
        None => {
            w.cuts.team_players.remove(&team);
        }
    }
}

fn set_kinds(w: &mut World, id: ObjectId, bits: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects.find_mut(id).unwrap().thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The players' object lists (`Player::iterateObjects`), as the driver keeps them: each player's objects in id order.
fn set_player_objects(w: &mut World) {
    w.cuts.player_objects.clear();
    for p in 0..3 {
        let v: Vec<ObjectId> = (1..=4).filter(|&id| w.get_controlling_player(id) == Some(p)).collect();
        w.cuts.player_objects.insert(p, v);
    }
}

/// The flags of `cfg[3]` that ops change, applied to the scene.
fn apply_flags(w: &mut World, flags: u32, sab_ai: Option<u16>) {
    let (saboteur, victim) = (1, 2);
    w.objects.find_mut(saboteur).unwrap().ai = if flags & 1 != 0 { sab_ai } else { None };
    if let Some(ai) = sab_ai {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(saboteur).unwrap().modules[ai as usize].body.as_mut() {
            p.answers.goal_object = if flags & 2 != 0 { victim } else { INVALID_ID };
        }
    }
    let rel = match flags >> 2 & 3 {
        0 => Relationship::Enemies,
        1 => Relationship::Neutral,
        _ => Relationship::Allies,
    };
    w.cuts.team_relationships.insert((1, 2), rel);
    // the victim's AI is a module index nothing calls (only its presence is read)
    w.objects.find_mut(victim).unwrap().ai = if flags & 16 != 0 { Some(u16::MAX) } else { None };
    let o = w.objects.find_mut(victim).unwrap();
    if flags & 32 != 0 {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
    w.friend_bind_to_drawable(victim, if flags & 64 != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let local = flags >> 14 & 3;
    w.players.friend_set_local_index(if local == 3 { None } else { Some(local as i32) });
    w.set_position(saboteur, &Coord3D { x: 0.0, y: 0.0, z: if flags & 65536 != 0 { 5.0 } else { 0.0 } });
}

/// Runs the ops on a fresh world and returns the dump (after the data dump) and the world.
fn rust_run(kind: usize, names: NameKeyGenerator, center: UpgradeCenter, data: ModuleDataKind, cfg: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut sc = SciCtx { store: ScienceStore::new(), names };
    let defs: String = SCIENCES.iter().map(|s| format!("Science {s}\nEnd\n")).collect();
    static BLOCKS: [BlockParse<SciCtx>; 1] = [BlockParse { token: "Science", parse: science_block }];
    Ini::load(b"Data\\INI\\Science.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &BLOCKS, &mut sc).expect("sciences load");

    let mut w = World::new();
    w.names = sc.names;
    w.statics = Arc::new(GameStatics { science_store: sc.store, upgrade_center: center, ..Default::default() });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.probe_effect_trace = true;
    w.cuts.globals.draw_icon_ui = false;
    w.cuts.globals.anim2d_collection_present = false;
    w.set_frame(cfg[8]);
    w.random.init_game_logic_random(cfg[10]);
    for team in 1..=4u32 {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    for i in 0..3 {
        let p = w.players.get_nth_mut(i).unwrap();
        p.money.init();
        p.money.set_player_index(i);
        let mut events = Vec::new();
        p.money.deposit(cfg[4 + i as usize], false, &mut events);
    }
    let flags = cfg[3];
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));
    for h in 0..2 {
        let p = cfg[9] >> (2 * h) & 3;
        set_player_of_team(&mut w, 3 + h, (p != 3).then_some(p));
    }

    // the saboteur: the crate module, then its AI
    let class = SabotageVictim::ALL[kind].1;
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), class.as_bytes(), b"ModuleTag_Crate", &data).unwrap().unwrap();
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    let plain_template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, ai] }).unwrap();
    assert_eq!(me, 1);
    let sab_ai = w.objects.find(me).unwrap().ai;

    // the victim: body, special powers, OCLUpdate, SpyVisionUpdates, contain (the driver's order)
    let next = w.next_object_id();
    let mut mods = Vec::new();
    if flags & 128 != 0 {
        let mut pm = NewModule::plain(b"CtBody", b"ModuleTag_Body");
        pm.interfaces = MODULEINTERFACE_BODY;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), f32::from_bits(cfg[7]), Default::default())));
        mods.push(pm);
    }
    for _ in 0..(flags >> 9 & 3) {
        let mut pm = NewModule::plain(b"CtPower", b"ModuleTag_Power");
        pm.interfaces = MODULEINTERFACE_SPECIAL_POWER;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
        mods.push(pm);
    }
    if flags & 2048 != 0 {
        let d = ModuleDataKind::OclUpdate(Arc::new(rts_lockstep_engine::logic::modules::ocl_update::OclUpdateModuleData { min_delay: cfg[11], max_delay: cfg[11], ..Default::default() }));
        mods.push(rts_lockstep_engine::logic::modules::new_module(next, b"OCLUpdate", b"ModuleTag_OCL", &d).unwrap().unwrap());
    }
    for _ in 0..(flags >> 12 & 3) {
        let d = rts_lockstep_engine::logic::modules::spy_vision_update::new_module_data(None).unwrap();
        mods.push(rts_lockstep_engine::logic::modules::new_module(next, b"SpyVisionUpdate", b"ModuleTag_Spy", &d).unwrap().unwrap());
    }
    if flags & 256 != 0 {
        let mut pm = NewModule::plain(b"CtContain", b"ModuleTag_Contain");
        pm.interfaces = MODULEINTERFACE_CONTAIN;
        pm.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers { contained: vec![3, 4], ..Default::default() }, 0.0, Default::default())));
        mods.push(pm);
    }
    let victim = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(2), modules: mods }).unwrap();
    assert_eq!(victim, 2);
    for team in 3..=4 {
        let id = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(team), modules: Vec::new() }).unwrap();
        assert_eq!(id, team as i32);
    }
    set_kinds(&mut w, victim, cfg[2]);
    w.set_position(victim, &Coord3D { x: 0.0, y: 0.0, z: 0.0 });
    let mut flags = flags;
    apply_flags(&mut w, flags, sab_ai);
    set_player_objects(&mut w);
    let ocl_idx = w.find_module(victim, b"OCLUpdate");
    let spy_idx: Vec<u16> = w.modules_with_spy_vision(victim);
    let idx = w.find_module(me, class.as_bytes()).unwrap();
    // where the victim's own modules start among its slots
    // where the victim's own modules start among its slots: after the object's standard helpers
    let test_modules_from = w.objects.find(victim).unwrap().modules.iter().take_while(|m| m.name.ends_with(b"Helper")).count();
    // the constructors' scheduling and draws are done: the trace starts here
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0i32);
        let target = if o[1] != 0 { victim } else { INVALID_ID };
        match o[0] {
            0 => {
                w.module_call(me, idx, ModuleCall::OnCollide { other: target, loc: None, normal: None });
                tally.collides += 1;
            }
            1 => res = i32::from(w.module_call(me, idx, ModuleCall::WouldLikeToCollideWith { other: target }).as_bool()),
            2 => {
                set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1));
                set_player_objects(&mut w);
            }
            3 => {
                flags = (flags & !0x1c1ff) | (o[1] & 0x1c1ff);
                apply_flags(&mut w, flags, sab_ai);
            }
            4 => set_kinds(&mut w, victim, o[1]),
            5 => {
                let p = w.players.get_nth_mut((o[1] % 3) as i32).unwrap();
                p.money.init();
                let mut events = Vec::new();
                p.money.deposit(o[2], false, &mut events);
            }
            6 => w.set_frame(o[1]),
            7..=9 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 7, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 7 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            10 => res = i32::from(w.module_call(me, idx, ModuleCall::IsSabotageBuildingCrateCollide).as_bool()),
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, res);
        let events: Vec<ObjectClientEvent> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(e) => Some(e.clone()),
                _ => None,
            })
            .collect();
        let mut event_iter = events.into_iter();
        let cuts = w.cuts.log.take().unwrap();
        let mut cut_iter = cuts.iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("cut ") {
                match cut_iter.next().expect("a cut line has a cut call") {
                    CutCall::PlayerMoneyDeposit { player, amount } => log.push(format!("deposit {player} {amount} 1")),
                    CutCall::PlayerMoneyWithdraw { player, amount } => log.push(format!("withdraw {player} {amount} 1")),
                    CutCall::PlayerScoreAddMoneyEarned { player, amount } => log.push(format!("earned {player} {amount}")),
                    CutCall::PlayerSetPowerSabotagedTillFrame { player, frame } => log.push(format!("powersabotage {player} {frame}")),
                    CutCall::PlayerOnPowerBrownOutChange { player, brown_out } => log.push(format!("brownout {player} {}", i32::from(*brown_out))),
                    CutCall::ObjectSetDisabledUntil { object, disabled, frame } => log.push(format!("disableduntil {object} {disabled} {frame}")),
                    CutCall::ObjectAttemptDamage { object, info } => log.push(format!(
                        "damage {object} {} {} {} {:08x}",
                        info.input.damage_type,
                        info.input.death_type,
                        info.input.source_id,
                        info.input.amount.to_bits()
                    )),
                    CutCall::InGameUiAddFloatingTextNamed { text, amount, position: p, color } => {
                        log.push(format!("float {} {amount} {},{},{} {:08x}", String::from_utf8_lossy(text), p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), *color as u32))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match event_iter.next() {
                    Some(ObjectClientEvent::Sound { sound, object_id, position }) => {
                        let name = match sound {
                            ObjectSound::SabotageResetTimerBuilding => "sabotageResetTimer",
                            ObjectSound::MoneyWithdraw => "moneyWithdraw",
                            ObjectSound::SabotageShutDownBuilding => "sabotageShutDown",
                            other => panic!("unexpected sound {other:?}"),
                        };
                        let pos = match position {
                            Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {name} {pos} -1 {object_id}"));
                    }
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => log.push(format!("fx{}", fx.id(&Some(name)))),
                    Some(ObjectClientEvent::Radar { call: RadarCall::TryInfiltrationEvent, object_id, .. }) => log.push(format!("infiltrate {object_id}")),
                    Some(ObjectClientEvent::DrawableFlashAsSelected { .. }) => log.push("flash".to_string()),
                    Some(ObjectClientEvent::Eva { message }) => log.push(format!(
                        "eva {}",
                        match message {
                            EvaMessage::BuildingSabotaged => 15,
                            EvaMessage::CashStolen => 16,
                            other => panic!("unexpected eva {other:?}"),
                        }
                    )),
                    _ => {}
                }
            } else if let Some(r) = t.strip_prefix("recharge ") {
                // the driver names a module by its place among the test's own modules; the Rust slot counts the object's standard ones too
                let (o, slot) = r.split_once(' ').unwrap();
                let slot: usize = slot.parse().unwrap();
                let place = slot - test_modules_from;
                log.push(format!("recharge {o} {place}"));
            } else if let Some(r) = t.strip_prefix("destroy ") {
                tally.destroyed += 1;
                log.push(format!("destroy {r}"));
            }
        }
        tally.effects += log.len() as u64;
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
        put_u32(
            &mut v,
            match ocl_idx {
                Some(i) => match w.module_call(victim, i, ModuleCall::OclUpdateGetRemainingFrames) {
                    ModuleReply::Int(n) => n as u32,
                    r => panic!("unexpected reply {r:?}"),
                },
                None => 0xFFFF_FFFF,
            },
        );
        put_u32(&mut v, spy_idx.len() as u32);
        for &i in &spy_idx {
            let o = w.objects.find(victim).unwrap();
            let words = match o.behaviors[o.modules[i as usize].update_index.unwrap() as usize].body.as_ref() {
                Some(UpdateBody::SpyVisionUpdate(s)) => s.state_words(),
                _ => panic!("the SpyVisionUpdate"),
            };
            put_u32(&mut v, words[2]);
            put_u32(&mut v, words[3]);
        }
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9SAB_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn sabotage_crates_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x5AB0_7A6E);
    let mut read = Check::new("sabotage crates R13 data read (throw, message)");
    let mut run = Check::new("sabotage crates R13 ops (data, effects, xfers, modules)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 8];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 8;
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\Sab{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly an enemy of another player, with the AI's goal on the victim and the class's kind
        let player = |g: &mut Gen| if g.below(12) == 0 { NO_PLAYER } else { g.below(3) };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = if g.below(6) != 0 { CLASS_KIND_BITS[kind] & (1 << (CLASS_KIND_BITS[kind].trailing_zeros() + g.below(CLASS_KIND_BITS[kind].count_ones()))) } else { 0 }
            | (g.u32() & g.u32() & g.u32() & 0xffff)
            | if g.below(2) == 0 { 1 } else { 0 };
        cfg[3] = u32::from(g.below(4) != 0)
            | u32::from(g.below(5) != 0) << 1
            | (if g.below(4) != 0 { 0 } else { g.below(3) }) << 2
            | u32::from(g.below(3) == 0) << 4
            | u32::from(g.below(10) == 0) << 5
            | u32::from(g.below(3) != 0) << 6
            | u32::from(g.below(5) != 0) << 7
            | u32::from(g.below(4) != 0) << 8
            | g.below(4) << 9
            | u32::from(g.below(2) == 0) << 11
            | g.below(4) << 12
            | g.below(4) << 14
            | u32::from(g.below(8) == 0) << 16;
        for i in 0..3 {
            cfg[4 + i] = *pick(&mut g, &[&0u32, &0, &50, &1000, &5000]);
        }
        cfg[7] = (*pick(&mut g, &[&0.0f32, &100.0, &250.5, &1000.0])).to_bits();
        cfg[8] = g.below(400);
        cfg[9] = g.below(16);
        cfg[10] = g.u32();
        cfg[11] = *pick(&mut g, &[&0u32, &30, &300]);
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=15 => [0, u32::from(g.below(12) != 0), 0, 0, 0, 0],
                16..=19 => [1, g.below(2), 0, 0, 0, 0],
                20..=21 => [2, g.below(4), 0, 0, 0, 0],
                22..=24 => [3, g.u32() & 0x1c1ff, 0, 0, 0, 0],
                25..=26 => [4, (g.u32() & 0xffff) | CLASS_KIND_BITS[kind], 0, 0, 0, 0],
                27..=28 => [5, g.below(3), *pick(&mut g, &[&0u32, &10, &1000, &99999]), 0, 0, 0],
                29..=30 => [6, g.below(400), 0, 0, 0, 0],
                31..=33 => [7, 0, 0, 0, 0, 0],
                34..=35 => [8, 0, 0, 0, 0, 0],
                36 => [9, 0, 0, 0, 0, 0],
                _ => [10, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(kind, ctx.names.clone(), ctx.center.clone(), data.expect("read ok"), &cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some(rd)
        } else {
            None
        };
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9sab_run(
                kind as i32,
                cfile.as_ptr(),
                cxfer.as_ptr(),
                cfg.as_ptr(),
                ops.len() as i32,
                flat.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                &mut len,
                msg.as_mut_ptr() as *mut _,
                msg.len() as i32,
                &mut code,
            )
        };
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || format!("case {case} ({}): cfg {cfg:x?}, file {:?}, ops {ops:?}", SabotageVictim::ALL[kind].1, show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["infiltrate", "audio", "flash", "eva", "recharge", "powersabotage", "brownout", "disableduntil", "withdraw", "deposit", "earned", "float", "damage", "destroy"] {
                *counts.entry(key.to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} ({}) cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", SabotageVictim::ALL[kind].1, show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        per_kind[kind] += 1;
    }
    println!(
        "sabotage crates R13: {cases} cases ({per_kind:?} compared per class), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} onCollide calls, {} destroyed, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(tally.destroyed * 10 > cases as u64, "the generator must reach the crates' actions");
}
