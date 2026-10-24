#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SalvageCrateCollide` (`logic::modules::salvage_crate_collide`) against the unchanged C++, `p1w13-sonnet`, batch R13c.
//!
//! # Method
//!
//! The C++ side is `SalvageCrateCollide.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crates_r13c.inc`) on the module build's
//! stand-ins: the crate (id 1, at the test's height) and the collector (id 2: kinds on the object and its template, player, level, whether it trains, a
//! body keeping armor set flags, the weapon set flags, maybe a drawable), three players (money, score, academy stats, colours that log), the logic
//! random, and an in-game UI, a game text and an audio manager that log.
//!
//! The Rust side is the real thing: a `World` with the same objects (the crate module from `modules::new_module`, a `ProbeModule` body for the
//! collector), `cuts.crate_stub_object_calls` set (the experience gain is a record, as the driver's is a line), and every op a `World::module_call` or a
//! change to the scene.
//!
//! Each case reads a `D` block (the `CrateCollide` fields and the five of the class; mostly well formed) and runs 1 to 14 ops. Compared, bit for bit:
//! the read's throw; the data; per op the throw status, a result, the effects in order (`wset`, `mc`, `mcclear`, `clearset`, `audio`, `gainexp`,
//! `deposit`, `earned`, `float`, `salvage`, `fx`, `destroy`), the xfer status, bytes and CRC, and the collector's crate armor and weapon set flags.
//!
//! # Results (2026-10-23, `L9R13C_CASES=9000`)
//!
//! 9,000 cases (6,861 compared, 228 excluded as undefined upstream: an unowned collector, a `NULL` player; 1,911 data blocks threw, as the C++ did),
//! 51,247 ops (24,153 `onCollide` calls, 3,675 crates used, 9,212 xfers) and 64,969 op outputs: **0 mismatches**. Effect lines compared: salvage 3,675,
//! crateMoney 2,167, deposit / earned / float 1,936 each, crateSalvage 827, gainexp 681, wset 517, mc 336, clearset 197, mcclear 129.
//!
//! Harness corrections on the way, none in the translation: a scene change's own client events (binding the drawable) were compared as the op's
//! effects; and the first generator reached the crate in 24 cases of 2,000 (a collector with no AI is refused by `CrateCollide::isValidToExecute`,
//! and player 0 is neutral), so the collector now has an AI four times in five and belongs to player 1 or 2 mostly.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (`logic::modules::plant`), 9,000 cases: 11 of 11 caught.
//!
//! | id | planted bug | mismatches |
//! |---|---|---|
//! | 1681 | any collector accepted, salvager or not | 1,444 |
//! | 1682 | a weapon upgrade past the second | 1,564 |
//! | 1683 | a heroic unit given a level | 467 |
//! | 1684 | the first weapon upgrade always | 908 |
//! | 1685 | the second armor upgrade's model condition not set | 197 |
//! | 1686 | no random money (always the minimum) | 1,928 |
//! | 1687 | no score for the money | 1,936 |
//! | 1688 | the floating text at the crate's own height | 1,936 |
//! | 1689 | the level chance taken from the weapon chance | 879 |
//! | 1690 | no money sound | 2,167 |
//! | 1691 | the academy stats not told | 3,675 |

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::common::game_type::{INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
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
use rts_lockstep_engine::logic::armor_set::{
    ARMORSET_CRATE_UPGRADE_ONE, ARMORSET_CRATE_UPGRADE_TWO,
};
use rts_lockstep_engine::logic::modules::salvage_crate_collide::SalvageCrateCollideModuleData;
use rts_lockstep_engine::logic::weapon_types::{
    WEAPONSET_CRATEUPGRADE_ONE, WEAPONSET_CRATEUPGRADE_TWO,
};
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{veterancy_from_i32, PRIVATE_EFFECTIVELY_DEAD};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{
    ModelConditionOp, ObjectClientEvent, ObjectSound,
};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9_r13c.bin");
const XFER_FILE: &str = "l9_r13c.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
const CLASS: &str = "SalvageCrateCollide";

/// The kinds the collector is given, by bit (the driver's `g_r13cKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 8] = [
    (KINDOF_SALVAGER, "SALVAGER"),
    (KINDOF_WEAPON_SALVAGER, "WEAPON_SALVAGER"),
    (KINDOF_ARMOR_SALVAGER, "ARMOR_SALVAGER"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_PARACHUTE, "PARACHUTE"),
    (KINDOF_HERO, "HERO"),
];

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
    *out = Some(ModuleDataKind::SalvageCrateCollide(Arc::new(
        SalvageCrateCollideModuleData::friend_new_module_data(Some(ini))?,
    )));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: read_block,
    }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9r13c_run`: the crate base, then the class's five fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::SalvageCrateCollide(d) = data else {
        unreachable!("not a salvage crate")
    };
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
    let sci = SCIENCES
        .iter()
        .position(|s| s.as_bytes().eq_ignore_ascii_case(&c.pickup_science))
        .map_or(-1, |i| i as i32);
    put_i32(v, sci);
    put_i32(v, fx.id(&c.execute_fx));
    put_str(v, &c.execution_animation_template);
    put_u32(v, c.execute_animation_display_time_in_seconds.to_bits());
    put_u32(v, c.execute_animation_z_rise_per_second.to_bits());
    put_u32(v, u32::from(c.execute_animation_fades));
    put_u32(v, d.weapon_chance.to_bits());
    put_u32(v, d.level_chance.to_bits());
    put_u32(v, d.money_chance.to_bits());
    put_i32(v, d.minimum_money);
    put_i32(v, d.maximum_money);
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
    let set: Vec<usize> = kinds
        .iter()
        .enumerate()
        .filter(|(_, v)| **v != 0)
        .map(|(i, _)| i)
        .collect();
    let flags = [u32_(&mut p), u32_(&mut p), u32_(&mut p)];
    let sci = u32_(&mut p) as i32;
    let fx = u32_(&mut p) as i32;
    let anim = str_(&mut p);
    let (t, z, fades) = (u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let own: Vec<u32> = (0..5).map(|_| u32_(&mut p)).collect();
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades} own {own:x?}"));
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
        let (armor, weapon) = (u32_(&mut p), u32_(&mut p));
        out.push(format!("op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} armor {armor} weapon {weapon}"));
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

/// The block `D`: the class's own fields and the crate fields (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    let chances = ["100%", "100%", "50%", "0%", "25%", "75%"];
    if g.below(100) < 50 {
        lines.push(format!("WeaponChance = {}", pick(g, &chances)));
    }
    if g.below(100) < 50 {
        lines.push(format!("LevelChance = {}", pick(g, &chances)));
    }
    if g.below(100) < 20 {
        lines.push(format!("MoneyChance = {}", pick(g, &chances)));
    }
    if g.below(100) < 50 {
        lines.push(format!(
            "MinMoney = {}",
            pick(g, &["0", "25", "75", "-5", "100"])
        ));
    }
    if g.below(100) < 50 {
        lines.push(format!(
            "MaxMoney = {}",
            pick(g, &["0", "25", "75", "100", "500"])
        ));
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
        lines.push(format!(
            "ExecuteFX = {}",
            pick(g, &["FX_A", "FX_B", "None"])
        ));
    }
    if !clean {
        lines.push(match g.below(4) {
            0 => "RequiredKindOf = +BOGUS".to_string(),
            1 => "MinMoney = lots".to_string(),
            2 => "WeaponChance = most".to_string(),
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

fn run_xfer(
    w: &mut World,
    me: ObjectId,
    idx: usize,
    how: u32,
    saved: &[u8],
) -> (i32, Vec<u8>, u32) {
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

/// The collector's template: the test's kinds and whether it trains.
fn set_kinds(w: &mut World, id: ObjectId, bits: u32, trainable: bool) {
    let mut values = ObjectTemplateValues {
        is_trainable: trainable,
        ..Default::default()
    };
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if bits >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    w.objects
        .find_mut(id)
        .unwrap()
        .thing
        .friend_set_template(Arc::new(ThingTemplate::from_object_values(&values)));
}

/// The collector's crate armor flags (on its body) and crate weapon set flags (bit 0 the first upgrade, bit 1 the second).
fn set_upgrades(w: &mut World, other: ObjectId, body: u16, armor: u32, weapon: u32) {
    if let Some(UpdateBody::ModuleProbe(p)) = w.objects.find_mut(other).unwrap().modules
        [body as usize]
        .body
        .as_mut()
    {
        p.armor_flags = (armor & 1) << ARMORSET_CRATE_UPGRADE_ONE
            | (armor >> 1 & 1) << ARMORSET_CRATE_UPGRADE_TWO;
    }
    let o = w.objects.find_mut(other).unwrap();
    o.cur_weapon_set_flags
        .set(WEAPONSET_CRATEUPGRADE_ONE, (weapon & 1) as i32);
    o.cur_weapon_set_flags
        .set(WEAPONSET_CRATEUPGRADE_TWO, (weapon >> 1 & 1) as i32);
}

/// The flags of `cfg[3]` that ops change (bits 0-4 and 8), applied to the scene.
fn apply_flags(w: &mut World, flags: u32, kinds: u32) {
    let other = 2;
    w.friend_bind_to_drawable(
        other,
        if flags & 1 != 0 {
            7
        } else {
            INVALID_DRAWABLE_ID
        },
    );
    // the collector's AI is a module index nothing calls (only its presence is read)
    w.objects.find_mut(other).unwrap().ai = if flags & 256 != 0 {
        Some(u16::MAX)
    } else {
        None
    };
    let o = w.objects.find_mut(other).unwrap();
    if flags & 2 != 0 {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
    o.experience.level = veterancy_from_i32((flags >> 3 & 3) as i32);
    set_kinds(w, other, kinds, flags & 4 != 0);
}

/// Runs the ops on a fresh world and returns the dump and the world.
fn rust_run(
    names: NameKeyGenerator,
    center: UpgradeCenter,
    data: ModuleDataKind,
    cfg: &[u32; 12],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut sc = SciCtx {
        store: ScienceStore::new(),
        names,
    };
    let defs: String = SCIENCES
        .iter()
        .map(|s| format!("Science {s}\nEnd\n"))
        .collect();
    static BLOCKS: [BlockParse<SciCtx>; 1] = [BlockParse {
        token: "Science",
        parse: science_block,
    }];
    Ini::load(
        b"Data\\INI\\Science.ini",
        defs.as_bytes(),
        IniLoadType::Overwrite,
        None,
        &BLOCKS,
        &mut sc,
    )
    .expect("sciences load");

    let mut w = World::new();
    w.names = sc.names;
    w.statics = Arc::new(GameStatics {
        science_store: sc.store,
        upgrade_center: center,
        ..Default::default()
    });
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.cuts.crate_stub_object_calls = true;
    w.cuts.globals.draw_icon_ui = false;
    w.cuts.globals.anim2d_collection_present = false;
    w.random.init_game_logic_random(cfg[6]);
    for team in 1..=2u32 {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    let flags0 = cfg[3];
    for p in 0..3u32 {
        let pl = w.players.get_nth_mut(p as i32).unwrap();
        pl.player_type = if flags0 >> (5 + p) & 1 != 0 {
            PlayerType::Computer
        } else {
            PlayerType::Human
        };
        w.cuts
            .player_colors
            .insert(p as i32, cfg[7].wrapping_add(p) as i32);
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));

    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        CLASS.as_bytes(),
        b"ModuleTag_Crate",
        &data,
    )
    .unwrap()
    .unwrap();
    let plain_template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template: plain_template.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1);
    let mut body = NewModule::plain(b"CtBody", b"ModuleTag_Body");
    body.interfaces = MODULEINTERFACE_BODY;
    body.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
        ProbeAnswers::default(),
        0.0,
        Default::default(),
    )));
    let other = w
        .new_object(NewObject {
            template: plain_template.clone(),
            status: ObjectStatusMaskType::default(),
            team: Some(2),
            modules: vec![body],
        })
        .unwrap();
    assert_eq!(other, 2);
    let body_idx = w.objects.find(other).unwrap().body.unwrap();
    w.set_position(
        me,
        &Coord3D {
            x: 0.0,
            y: 0.0,
            z: f32::from_bits(cfg[8]),
        },
    );
    set_upgrades(&mut w, other, body_idx, cfg[4], cfg[5]);
    let mut flags = flags0;
    let mut kinds = cfg[2];
    apply_flags(&mut w, flags, kinds);
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
                w.module_call(
                    me,
                    idx,
                    ModuleCall::OnCollide {
                        other: target,
                        loc: None,
                        normal: None,
                    },
                );
                tally.collides += 1;
            }
            1 => {
                res = i32::from(
                    w.module_call(
                        me,
                        idx,
                        ModuleCall::WouldLikeToCollideWith { other: target },
                    )
                    .as_bool(),
                )
            }
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => {
                flags = (flags & !0x11F) | (o[1] & 0x11F);
                apply_flags(&mut w, flags, kinds);
            }
            4 => {
                kinds = o[1];
                set_kinds(&mut w, other, kinds, flags & 4 != 0);
            }
            5 => set_upgrades(&mut w, other, body_idx, o[1], o[2]),
            6 => w.set_position(
                me,
                &Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: f32::from_bits(o[1]),
                },
            ),
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
        if (2..=6).contains(&o[0]) {
            // a change to the scene (the drawable's binding makes client events of its own): the driver's changes log nothing
            w.cuts.trace = Some(Vec::new());
            w.cuts.log = Some(Vec::new());
            events_before = w.events.events().len();
        }
        let threw = w.cuts.module_errors[errors_before..]
            .iter()
            .any(|(_, e)| !e.is_undefined());
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
                    CutCall::WeaponSetUpdate { .. } => log.push("wset".to_string()),
                    CutCall::ExperienceGainForLevel {
                        object,
                        levels,
                        can_scale_for_bonus,
                    } => log.push(format!(
                        "gainexp {object} {levels} {}",
                        i32::from(*can_scale_for_bonus)
                    )),
                    CutCall::PlayerMoneyDeposit { player, amount } => {
                        log.push(format!("deposit {player} {amount} 1"))
                    }
                    CutCall::PlayerScoreAddMoneyEarned { player, amount } => {
                        log.push(format!("earned {player} {amount}"))
                    }
                    CutCall::InGameUiAddFloatingTextNamed {
                        text,
                        amount,
                        position: p,
                        color,
                    } => log.push(format!(
                        "float {} {amount} {},{},{} {:08x}",
                        String::from_utf8_lossy(text),
                        p.x.to_bits(),
                        p.y.to_bits(),
                        p.z.to_bits(),
                        *color as u32
                    )),
                    CutCall::PlayerAcademyRecordSalvageCollected { player } => {
                        log.push(format!("salvage {player}"))
                    }
                    _ => {}
                }
            } else if t.starts_with("evt ") {
                match event_iter.next().flatten() {
                    Some(ObjectClientEvent::Sound {
                        sound,
                        object_id,
                        position,
                    }) => {
                        let name = match sound {
                            ObjectSound::CrateSalvage => "crateSalvage",
                            ObjectSound::CrateMoney => "crateMoney",
                            other => panic!("unexpected sound {other:?}"),
                        };
                        let pos = match position {
                            Some(p) => {
                                format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits())
                            }
                            None => "-".to_string(),
                        };
                        log.push(format!("audio {name} {pos} -1 {object_id}"));
                    }
                    Some(ObjectClientEvent::DrawableModelCondition { op, .. }) => {
                        log.push(match op {
                            ModelConditionOp::Set(a) if a.len() == 1 => format!("mc {}", a[0]),
                            ModelConditionOp::Clear(a) if a.len() == 1 => {
                                format!("mcclear {}", a[0])
                            }
                            ModelConditionOp::ClearAndSet { clear, set }
                                if clear.len() == 1 && set.len() == 1 =>
                            {
                                format!("clearset {} {}", clear[0], set[0])
                            }
                            other => format!("model condition {other:?}"),
                        })
                    }
                    Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) => {
                        log.push(format!("fx{}", fx.id(&Some(name))))
                    }
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
        let armor = match w.objects.find(other).unwrap().modules[body_idx as usize]
            .body
            .as_ref()
        {
            Some(UpdateBody::ModuleProbe(p)) => p.armor_flags,
            _ => panic!("the collector's body"),
        };
        put_u32(
            &mut v,
            (armor >> ARMORSET_CRATE_UPGRADE_ONE & 1)
                | (armor >> ARMORSET_CRATE_UPGRADE_TWO & 1) << 1,
        );
        let ob = w.objects.find(other).unwrap();
        put_u32(
            &mut v,
            u32::from(ob.test_weapon_set_flag(WEAPONSET_CRATEUPGRADE_ONE))
                | u32::from(ob.test_weapon_set_flag(WEAPONSET_CRATEUPGRADE_TWO)) << 1,
        );
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9R13C_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn salvage_crate_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0013_C5A1);
    let mut read = Check::new("salvage crate R13c data read (throw, message)");
    let mut run = Check::new("salvage crate R13c ops (data, effects, xfers, upgrades)");
    let (mut read_failed, mut ops_run, mut excluded, mut compared) = (0u64, 0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let clean = g.below(10) < 8;
        let text = gen_text(&mut g, clean);
        let name = format!("Data\\INI\\R13c{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // mostly a salvager of some player, alive, with a drawable
        let player = |g: &mut Gen| match g.below(12) {
            0 => NO_PLAYER,
            1 => 0,
            _ => 1 + g.below(2),
        };
        let mut cfg = [0u32; 12];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = u32::from(g.below(6) != 0) | (g.u32() & 0xfe & (g.u32() | 6));
        cfg[3] = u32::from(g.below(3) != 0)
            | u32::from(g.below(10) == 0) << 1
            | u32::from(g.below(4) != 0) << 2
            | g.below(4) << 3
            | (g.u32() & g.u32() & 7) << 5
            | u32::from(g.below(5) != 0) << 8;
        cfg[4] = g.below(4);
        cfg[5] = g.below(4);
        cfg[6] = g.u32();
        cfg[7] = g.u32() & 0x00ff_ffff;
        cfg[8] = (*pick(&mut g, &[&0.0f32, &12.5, &-3.0])).to_bits();
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=17 => [0, u32::from(g.below(12) != 0), 0, 0, 0, 0],
                18..=20 => [1, g.below(2), 0, 0, 0, 0],
                21..=22 => [2, g.below(4), 0, 0, 0, 0],
                23..=25 => [
                    3,
                    (g.u32() & 0x1F) | u32::from(g.below(5) != 0) << 8,
                    0,
                    0,
                    0,
                    0,
                ],
                26..=27 => [4, g.u32() & 0xff, 0, 0, 0, 0],
                28..=30 => [5, g.below(4), g.below(4), 0, 0, 0],
                31..=32 => [
                    6,
                    (*pick(&mut g, &[&0.0f32, &12.5, &-3.0])).to_bits(),
                    0,
                    0,
                    0,
                    0,
                ],
                33..=35 => [7, 0, 0, 0, 0, 0],
                36..=37 => [8, 0, 0, 0, 0, 0],
                _ => [9, 0, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(
                ctx.names.clone(),
                ctx.center.clone(),
                data.expect("read ok"),
                &cfg,
                &ops,
                &mut tally,
            );
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
            cpp::ref_l9r13c_run(
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
        let what = || {
            format!(
                "case {case}: cfg {cfg:x?}, file {:?}, ops {ops:?}",
                show(&text)
            )
        };
        read.eq(
            &outcome,
            &Outcome {
                status,
                detail,
                crc: 0,
            },
            what,
        );
        let Some(rd) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in [
                "wset",
                "mc ",
                "mcclear",
                "clearset",
                "audio crateSalvage",
                "audio crateMoney",
                "gainexp",
                "deposit",
                "earned",
                "float",
                "salvage",
                "destroy",
            ] {
                *counts.entry(key.trim().to_string()).or_default() +=
                    l.matches(&format!("\"{key}")).count() as u64;
            }
        }
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr
                .iter()
                .zip(dc.iter())
                .position(|(a, b)| a != b)
                .unwrap_or(dr.len().min(dc.len()));
            eprintln!(
                "--- case {case} cfg {cfg:x?} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}",
                show(&text),
                dr.get(i),
                dc.get(i)
            );
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        ops_run += ops.len() as u64;
        compared += 1;
    }
    println!(
        "salvage crate R13c: {cases} cases ({compared} compared), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {} onCollide calls, {} destroyed, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<20} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        tally.destroyed * 10 > cases as u64,
        "the generator must reach the crate's actions"
    );
}
