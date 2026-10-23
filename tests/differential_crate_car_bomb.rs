#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/ConvertToCarBombCrateCollide.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `ConvertToCarBombCrateCollide` (`logic::modules::convert_to_car_bomb_crate_collide`) against the unchanged C++, `p1w13-sonnet`, batch R13d.
//!
//! # Method
//!
//! The C++ side is `ConvertToCarBombCrateCollide.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crates_r13d.inc`) on the
//! module build's stand-ins: the terrorist (id 1: player, level, the name `Pilot`, vision and shroud clearing ranges, maybe an AI with a goal) and the
//! vehicle (id 2: kinds, player, the car bomb status bit and weapon set flag, its template's weapon sets, found by the real `SparseMatchFinder`, level,
//! maybe an AI and a drawable), three players with default teams 11 to 13, and a radar, a script engine and an FX list that log; the booby trap answers
//! as the test says.
//!
//! The Rust side is the real thing: a `World` with the same objects (the crate module from `modules::new_module`, a `ProbeModule` AI for the
//! terrorist), `cuts.crate_stub_object_calls` set (the booby trap, the defection, the veterancy and the radar are records, as the driver's are lines),
//! and every op a `World::module_call` or a change to the scene.
//!
//! Each case reads a `D` block (the `CrateCollide` fields and `FXList`; mostly well formed) and runs 1 to 14 ops. Compared, bit for bit: the read's
//! throw; the data; per op the throw status, a result, the effects in order (`booby`, `wset`, `mc`, `fx`, `defect`, `transfername`, `xplevel`,
//! `radarremove`, `radaradd`, `destroy`), the xfer status, bytes and CRC, and the vehicle's car bomb status bit and weapon set flag, vision range,
//! shroud clearing range and level.
//!
//! # Results (2026-10-23, `L9R13D_CASES=9000`)
//!
//! 9,000 cases (6,959 compared, 118 excluded as undefined upstream: an unowned terrorist, a `NULL` player; 1,923 data blocks threw, as the C++ did),
//! 52,594 ops (24,022 `onCollide` calls, 1,369 crates used, 10,776 xfers) and 66,512 op outputs: **0 mismatches**. Effect lines compared: booby 1,509,
//! wset / defect / transfername / xplevel / radarremove / radaradd 1,369 each, fx 848, mc 687.
//!
//! Harness correction on the way, none in the translation: the C++ `FXList::doFXObj` stand-in records a `NULL` list, which the real header's static
//! skips (`FXList.h:151-156`); the driver drops it, as the active body driver does.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 12 of 12 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1701 | aircraft and boats converted | 2,160 |
//! | 1702 | a car bomb converted again | 2,172 |
//! | 1703 | a best weapon set without the car bomb flag accepted | 1,405 |
//! | 1704 | a car bomb weapon set in use accepted | 1,899 |
//! | 1705 | the AI's goal ignored | 1,728 |
//! | 1706 | the conversion goes on after a fatal booby trap | 310 |
//! | 1707 | no `FXList` | 568 |
//! | 1708 | the name not handed on | 1,369 |
//! | 1709 | the vision range not handed on | 5,809 |
//! | 1710 | no `IS_CARBOMB` status | 6,547 |
//! | 1711 | the veterancy reset to regular | 4,831 |
//! | 1712 | not taken off the radar first | 1,369 |

mod support;
mod up_common;

use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
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
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_IS_CARBOMB;
use rts_lockstep_engine::logic::modules::convert_to_car_bomb_crate_collide::ConvertToCarBombCrateCollideModuleData;
use rts_lockstep_engine::logic::weapon_types::{WEAPONSET_CARBOMB, WEAPONSET_CRATEUPGRADE_ONE, WEAPONSET_VETERAN};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{veterancy_from_i32, PRIVATE_EFFECTIVELY_DEAD};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ModelConditionOp, ObjectClientEvent};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r13d.bin");
const XFER_FILE: &str = "l9_r13d.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
const CLASS: &str = "ConvertToCarBombCrateCollide";

/// The kinds the vehicle is given, by bit (the driver's `g_r13dKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 8] = [
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_AIRCRAFT, "AIRCRAFT"),
    (KINDOF_BOAT, "BOAT"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_IMMUNE_TO_CAPTURE, "IMMUNE_TO_CAPTURE"),
    (KINDOF_DRONE, "DRONE"),
    (KINDOF_TRANSPORT, "TRANSPORT"),
];
/// The weapon set flags a template's set may hold, by bit (the driver's `g_r13dSetFlags`).
const SET_FLAGS: [i32; 3] = [WEAPONSET_CARBOMB, WEAPONSET_VETERAN, WEAPONSET_CRATEUPGRADE_ONE];

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

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::ConvertToCarBombCrateCollide(Arc::new(ConvertToCarBombCrateCollideModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r13d_run`: the crate base, then the FX list.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::ConvertToCarBombCrateCollide(d) = data else { unreachable!("not a car bomb crate") };
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
    put_i32(v, fx.id(&d.fx_list));
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
    let fx_list = u32_(&mut p) as i32;
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades} fxlist {fx_list}"));
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
        let state: Vec<u32> = (0..5).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} vehicle {state:x?}"));
    }
    out
}

// -- generators --

fn kind_tokens(g: &mut Gen) -> String {
    let mut t = Vec::new();
    for _ in 0..1 + g.below(3) {
        let n = KINDS[g.below(8) as usize].1;
        t.push(match g.below(10) {
            0 => "NONE".to_string(),
            1..=6 => format!("+{n}"),
            _ => format!("-{n}"),
        });
    }
    t.join(" ")
}

/// The block `D`: `FXList` and the crate fields (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if g.below(100) < 60 {
        lines.push(format!("FXList = {}", pick(g, &["FX_A", "FX_B", "None"])));
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
    if g.below(100) < 10 {
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
            1 => "EffectRange = 5".to_string(),
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

/// The vehicle's template: the test's kinds and weapon sets (up to three, 4 bits each: bit 0 present, bits 1-3 its flags over `SET_FLAGS`).
fn set_template(w: &mut World, id: ObjectId, bits: u32, sets: u32) {
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    for k in 0..3 {
        let c = sets >> (4 * k) & 15;
        if c & 1 != 0 {
            values.weapon_sets.push(((0..3).filter(|f| c >> (1 + f) & 1 != 0).map(|f| SET_FLAGS[f as usize]).collect(), false));
        }
    }
    w.objects.find_mut(id).unwrap().thing.friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The flags of `cfg[3]` that ops change (bits 0-12), applied to the scene.
fn apply_flags(w: &mut World, flags: u32, crate_ai: Option<u16>) {
    let (me, other) = (1, 2);
    w.objects.find_mut(me).unwrap().ai = if flags & 1 != 0 { crate_ai } else { None };
    if let Some(ai) = crate_ai {
        if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(me).unwrap().modules[ai as usize].body.as_mut() {
            p.answers.goal_object = if flags & 2 != 0 { other } else { INVALID_ID };
        }
    }
    // the vehicle's AI is a module index nothing calls (only its presence is read)
    w.objects.find_mut(other).unwrap().ai = if flags & 4 != 0 { Some(u16::MAX) } else { None };
    for (id, bit) in [(other, 8), (me, 16)] {
        let o = w.objects.find_mut(id).unwrap();
        if flags & bit != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
    }
    w.friend_bind_to_drawable(other, if flags & 32 != 0 { 7 } else { INVALID_DRAWABLE_ID });
    let o = w.objects.find_mut(other).unwrap();
    o.status.set(OBJECT_STATUS_IS_CARBOMB, i32::from(flags & 64 != 0));
    o.cur_weapon_set_flags.set(WEAPONSET_CARBOMB, i32::from(flags & 128 != 0));
    w.cuts.crate_booby_trapped = flags & 256 != 0;
    w.objects.find_mut(me).unwrap().experience.level = veterancy_from_i32((flags >> 9 & 3) as i32);
    w.objects.find_mut(other).unwrap().experience.level = veterancy_from_i32((flags >> 11 & 3) as i32);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(names: NameKeyGenerator, center: UpgradeCenter, data: ModuleDataKind, cfg: &[u32; 12], ops: &[[u32; 6]], tally: &mut Tally) -> (Vec<u8>, World) {
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
    w.cuts.globals.draw_icon_ui = false;
    w.cuts.globals.anim2d_collection_present = false;
    for team in [1u32, 2, 11, 12, 13] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    let flags0 = cfg[3];
    for p in 0..3u32 {
        let pl = w.players.get_nth_mut(p as i32).unwrap();
        pl.player_type = if flags0 >> (13 + p) & 1 != 0 { PlayerType::Computer } else { PlayerType::Human };
        pl.default_team = Some(11 + p);
        set_player_of_team(&mut w, 11 + p, Some(p));
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));

    // the terrorist: the crate module, then its AI
    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), CLASS.as_bytes(), b"ModuleTag_Crate", &data).unwrap().unwrap();
    let mut ai = NewModule::plain(b"CtProbe", b"ModuleTag_AI");
    ai.is_ai = true;
    ai.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(ProbeAnswers::default(), 0.0, Default::default())));
    let plain_template = Arc::new(ThingTemplate::from_object_values(&ObjectTemplateValues::default()));
    let me = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(1), modules: vec![m, ai] }).unwrap();
    assert_eq!(me, 1);
    let crate_ai = w.objects.find(me).unwrap().ai;
    {
        let o = w.objects.find_mut(me).unwrap();
        o.name = b"Pilot".to_vec();
        o.vision_range = f32::from_bits(cfg[5]);
        o.shroud_clearing_range = f32::from_bits(cfg[6]);
    }
    let other = w.new_object(NewObject { template: plain_template.clone(), status: ObjectStatusMaskType::default(), team: Some(2), modules: Vec::new() }).unwrap();
    assert_eq!(other, 2);
    let mut kinds = cfg[2];
    let mut sets = cfg[4];
    set_template(&mut w, other, kinds, sets);
    let mut flags = flags0;
    apply_flags(&mut w, flags, crate_ai);
    let idx = w.find_module(me, CLASS.as_bytes()).unwrap();
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let mut events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0i32);
        let target = if o[1] != 0 { other } else { INVALID_ID };
        match o[0] {
            0 => {
                w.module_call(me, idx, ModuleCall::OnCollide { other: target, loc: None, normal: None });
                tally.collides += 1;
            }
            1 => res = i32::from(w.module_call(me, idx, ModuleCall::WouldLikeToCollideWith { other: target }).as_bool()),
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => {
                flags = (flags & !0x1FFF) | (o[1] & 0x1FFF);
                apply_flags(&mut w, flags, crate_ai);
            }
            4 => {
                kinds = o[1];
                set_template(&mut w, other, kinds, sets);
            }
            5 => {
                sets = o[1];
                set_template(&mut w, other, kinds, sets);
            }
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
            _ => unreachable!(),
        }
        if (2..=5).contains(&o[0]) {
            // a change to the scene (the drawable's binding makes client events of its own): the driver's changes log nothing
            w.cuts.trace = Some(Vec::new());
            w.cuts.log = Some(Vec::new());
            events_before = w.events.events().len();
        }
        let threw = w.cuts.module_errors[errors_before..].iter().any(|(_, e)| !e.is_undefined());
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, res);
        // every client event has its `evt` line, so the lines and the events are paired before choosing
        let events: Vec<Option<ObjectClientEvent>> = w.events.events()[events_before..]
            .iter()
            .map(|e| match e {
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
                    CutCall::ObjectCheckAndDetonateBoobyTrap { object, victim, result } => log.push(format!("booby {object} {victim} {}", i32::from(*result))),
                    CutCall::WeaponSetUpdate { .. } => log.push("wset".to_string()),
                    CutCall::ObjectDefect { object, team, detection_time } => log.push(format!("defect {object} {} {detection_time}", team.map_or(-1, |t| t as i32))),
                    CutCall::ScriptEngineTransferObjectName { name, object } => log.push(format!("transfername {} {object}", String::from_utf8_lossy(name))),
                    CutCall::ExperienceSetVeterancyLevel { object, level, .. } => log.push(format!("xplevel {object} {level}")),
                    CutCall::RadarRemoveObject { object } => log.push(format!("radarremove {object}")),
                    CutCall::RadarAddObject { object } => log.push(format!("radaradd {object}")),
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match event_iter.next().flatten() {
                    Some(ObjectClientEvent::DrawableModelCondition { op, .. }) => log.push(match op {
                        ModelConditionOp::Set(a) if a.len() == 1 => format!("mc {}", a[0]),
                        ModelConditionOp::Clear(a) if a.len() == 1 => format!("mcclear {}", a[0]),
                        ModelConditionOp::ClearAndSet { clear, set } if clear.len() == 1 && set.len() == 1 => format!("clearset {} {}", clear[0], set[0]),
                        other => format!("model condition {other:?}"),
                    }),
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => log.push(format!("fx{}", fx.id(&Some(name)))),
                    _ => {}
                }
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
        let ob = w.objects.find(other).unwrap();
        put_u32(&mut v, u32::from(ob.test_status(OBJECT_STATUS_IS_CARBOMB)));
        put_u32(&mut v, u32::from(ob.test_weapon_set_flag(WEAPONSET_CARBOMB)));
        put_u32(&mut v, ob.vision_range.to_bits());
        put_u32(&mut v, ob.shroud_clearing_range.to_bits());
        put_u32(&mut v, ob.experience.level as u32);
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R13D_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn car_bomb_crate_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0013_DCB0);
    let mut read = Check::new("car bomb crate R13d data read (throw, message)");
    let mut run = Check::new("car bomb crate R13d ops (data, effects, xfers, vehicle)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R13d{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a living vehicle of an enemy player, a car bomb weapon set to find, the terrorist's AI after it
        let player = |g: &mut Gen| match g.below(12) {
            0 => NO_PLAYER,
            1 => 0,
            _ => 1 + g.below(2),
        };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = if g.below(4) != 0 { 1 } else { g.u32() & 0xff };
        cfg[3] = u32::from(g.below(4) != 0)
            | u32::from(g.below(5) != 0) << 1
            | u32::from(g.below(5) != 0) << 2
            | u32::from(g.below(12) == 0) << 3
            | u32::from(g.below(12) == 0) << 4
            | u32::from(g.below(2) == 0) << 5
            | u32::from(g.below(8) == 0) << 6
            | u32::from(g.below(8) == 0) << 7
            | u32::from(g.below(6) == 0) << 8
            | g.below(4) << 9
            | g.below(4) << 11
            | (g.u32() & g.u32() & 7) << 13;
        cfg[4] = if g.below(3) != 0 { 1 | 3 << 4 } else { g.u32() & 0xfff };
        cfg[5] = (*pick(&mut g, &[&0.0f32, &150.0, &300.5])).to_bits();
        cfg[6] = (*pick(&mut g, &[&0.0f32, &200.0, &450.25])).to_bits();
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=17 => [0, u32::from(g.below(12) != 0), 0, 0, 0, 0],
                18..=20 => [1, g.below(2), 0, 0, 0, 0],
                21..=22 => [2, g.below(4), 0, 0, 0, 0],
                23..=26 => [3, (g.u32() & 0x1FFF) | 3 | u32::from(g.below(4) != 0) << 2, 0, 0, 0, 0],
                27..=28 => [4, if g.below(2) == 0 { 1 } else { g.u32() & 0xff }, 0, 0, 0, 0],
                29..=31 => [5, g.u32() & 0xfff, 0, 0, 0, 0],
                32..=34 => [7, 0, 0, 0, 0, 0],
                35..=37 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(ctx.names.clone(), ctx.center.clone(), data.expect("read ok"), &cfg, &ops, &mut tally);
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
            cpp::ref_l9r13d_run(
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
        let what = || format!("case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["booby", "wset", "mc ", "fx", "defect", "transfername", "xplevel", "radarremove", "radaradd", "destroy"] {
                *counts.entry(key.trim().to_string()).or_default() += l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "car bomb crate R13d: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onCollide calls, {} destroyed, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<20} {c}");
    }
    read.finish();
    run.finish();
    assert!(tally.destroyed * 10 > cases as u64, "the generator must reach the crate's actions");
}
