#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/{CrateCollide, HealCrateCollide, ShroudCrateCollide, MoneyCrateCollide}.cpp
//   (and parseUpgradePair of .../Update/AutoDepositUpdate.cpp)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The crate base and three crate modules (`logic::modules::{crate_collide, heal_crate_collide, shroud_crate_collide, money_crate_collide}`, with `upgrade_pair`) against the unchanged C++,
//! `p1w11-sonnet`, crate batch F.
//!
//! # Method
//!
//! The C++ side is the three `.cpp` files, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_crates_f.inc`) on the module build's
//! stand-ins: three players (type, sciences, a `healAllObjects` that logs), a crate and an object that collides with it (kinds, position, AI, death,
//! player), a partition manager that logs the reveal, a miscellaneous audio whose events are named after their fields, a 2D animation collection that
//! logs the lookup, an in-game UI that logs the world animation, a game logic whose `destroyObject` logs, and the REAL `CollideModule`, `INI`, FX list
//! parse and science lookup.
//!
//! The Rust side is the real thing: a `World` with two teams, three players (`World::players`), a science store, and two objects (the crate with the
//! module from `modules::new_module`, the other with the kinds the case gives it); every op is a `World::module_call` (`OnCollide`,
//! `WouldLikeToCollideWith`), a change to the other object or the crate, or an xfer through `World::xfer_module_slot`.
//!
//! Each case defines four upgrades, reads a `D` block (the `CrateCollide` fields and, for a money crate, `MoneyProvided` and `UpgradedBoost` pairs; mostly well formed), and when the read succeeds runs 1 to 14 ops.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the result of `wouldLikeToCollideWith`, the effects in order (`heal`,
//! `reveal`, `deposit`, `earned`, `audio`, `fx`, `destroy`, `findanim` and `anim`) and the xfer status, bytes and CRC.
//!
//! # Deviations the test cannot see
//!
//! `PickupScience` is kept as the science's name and looked up when the crate is tested (see `modules::crate_collide`): the generator names only
//! known sciences, so that the throw upstream makes at the read for an unknown one is not in play. Inputs for which upstream is undefined (a collecting
//! object with no controlling player, for the three modules; an `UpgradedBoost` line that ends after the upgrade's name, whose `stricmp` of a `NULL` token crashes) are never sent to the C++: the Rust records an `is_undefined()`
//! error and the case is counted as excluded.
//!
//! # Results (2026-10-21, 9000 cases, `L9CF_CASES` overrides)
//!
//! 8,768 data reads and 58,166 dump lines (44,280 ops: 15,898 `onCollide`, 5,740 xfers) compared, 0 mismatches; 232 inputs excluded as undefined upstream;
//! 2,828 data blocks threw on both sides. 1,225 crates were destroyed. Effects: 1,199 `audio`, 1,199 `destroy`, 457 `fx`, 453 `deposit` and `earned`,
//! 390 `heal`, 356 `reveal`, 269 `findanim` and `anim`.
//!
//! `MoneyCrateCollide`'s `static` (see `modules::money_crate_collide`): the C++ is `MoneyCrateCollide.cpp` with that one declaration turned into a
//! global and a flag by `build.rs` (`mod_gen/MoneyCrateCollide_static.cpp`, asserted to apply once; the driver resets the flag for each case), and
//! `parseUpgradePair` is cut out of `AutoDepositUpdate.cpp` the same way (`mod_gen/AutoDepositPair.inc`). Plant 773 (no cache) is caught, so the
//! cases do reach the pairs after the first with a template the first pair fixed.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`); the case count is 3000.
//!
//! | plants | module | what is broken |
//! |---|---|---|
//! | 751 | HealCrateCollide | the heal is not asked of the player |
//! | 752 | HealCrateCollide | the sound has no position |
//! | 753 | ShroudCrateCollide | the wrong player's map is revealed |
//! | 754 | CrateCollide | a neutral collector is allowed |
//! | 755 | CrateCollide | `ForbidOwnerPlayer` forbids everyone but the owner |
//! | 756 | CrateCollide | the execute FX is not played |
//! | 757 | CrateCollide | the crate is not destroyed |
//! | 758 | CrateCollide | `ForbiddenKindOf` is ignored |
//! | 759 | CrateCollide | the animation ignores `getDrawIconUI()` |
//! | 760 | CrateCollide | the building exception is gone |
//! | 761 | CrateCollide | a parachute may collect |
//! | 762 | CrateCollide | the science test is inverted |
//! | 763 | CrateCollide | `HumanOnly` admits only computers |
//! | 771 | MoneyCrateCollide | the upgrade boost is never added |
//! | 772 | MoneyCrateCollide | the score keeper is told 0 |
//! | 773 | MoneyCrateCollide | the template is looked up on every pair (no `static`) |
//! | 774 | `parseUpgradePair` | the boost is stored as its absolute value |
//!
//! All 17 are caught. Plants 755 and 760 are detected by the C++ reference segfaulting (exit 139): with the plant the Rust no longer reports a case as
//! undefined upstream (a collector with no controlling player passes the test), so the case reaches the C++ that upstream cannot run. That is a
//! detection, not a clean mismatch; it is recorded as it happened (as plant 619 of `differential_upgrade_modules_b.rs`).

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::game_common::PlayerType;
use rts_lockstep_engine::common::game_type::INVALID_ID;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::*;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::name_key_generator::NameKeyGenerator;
use rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType;
use rts_lockstep_engine::common::science::{GameTextFetch, ScienceStore, ScienceType};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::common::upgrade::{UpgradeCenter, UpgradeMaskType};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::crate_collide::CrateCollideModuleData;
use rts_lockstep_engine::logic::modules::heal_crate_collide::HealCrateCollideModuleData;
use rts_lockstep_engine::logic::modules::money_crate_collide::MoneyCrateCollideModuleData;
use rts_lockstep_engine::logic::modules::shroud_crate_collide::ShroudCrateCollideModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleReply};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9crates_f.bin");
const XFER_FILE: &str = "l9crates_f.bin";

/// The class names of the kinds, in the order of the driver's `kind`.
const CLASSES: [&str; 3] = [
    "HealCrateCollide",
    "ShroudCrateCollide",
    "MoneyCrateCollide",
];
/// The upgrades the cases define, `Up_0` first (they take the mask bits 0 to 3).
const UPGRADES: [&str; 4] = ["Up_0", "Up_1", "Up_2", "Up_3"];
/// The kinds a collecting object is given, by bit (the driver's `g_cfKinds`), with their INI names.
const KINDS: [(KindOfType, &str); 8] = [
    (KINDOF_STRUCTURE, "STRUCTURE"),
    (KINDOF_PARACHUTE, "PARACHUTE"),
    (KINDOF_VEHICLE, "VEHICLE"),
    (KINDOF_INFANTRY, "INFANTRY"),
    (KINDOF_AIRCRAFT, "AIRCRAFT"),
    (KINDOF_SELECTABLE, "SELECTABLE"),
    (KINDOF_CAN_ATTACK, "CAN_ATTACK"),
    (KINDOF_HUGE_VEHICLE, "HUGE_VEHICLE"),
];
const NO_PLAYER: u32 = 0xFFFF_FFFF;

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

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut (usize, Option<ModuleDataKind>)) -> IniResult<()> {
    out.1 = Some(match out.0 {
        0 => ModuleDataKind::HealCrateCollide(Arc::new(
            HealCrateCollideModuleData::friend_new_module_data(Some(ini))?,
        )),
        1 => ModuleDataKind::ShroudCrateCollide(Arc::new(
            ShroudCrateCollideModuleData::friend_new_module_data(Some(ini))?,
        )),
        _ => ModuleDataKind::MoneyCrateCollide(Arc::new(
            MoneyCrateCollideModuleData::friend_new_module_data(Some(ini))?,
        )),
    });
    Ok(())
}

fn rust_read(kind: usize, name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse {
        token: "D",
        parse: read_block,
    }];
    let mut out = (kind, None);
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out.1)
}

fn crate_data(data: &ModuleDataKind) -> &CrateCollideModuleData {
    match data {
        ModuleDataKind::HealCrateCollide(d) => &d.crate_data,
        ModuleDataKind::ShroudCrateCollide(d) => &d.crate_data,
        ModuleDataKind::MoneyCrateCollide(d) => &d.crate_data,
        _ => unreachable!("not a crate module"),
    }
}

/// The data dump of `ref_l9cf_run`: the crate base.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let c = crate_data(data);
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
    if let ModuleDataKind::MoneyCrateCollide(d) = data {
        put_u32(v, d.money_provided);
        put_u32(v, d.upgrade_boost.len() as u32);
        for pair in &d.upgrade_boost {
            put_str(v, &pair.type_name);
            put_i32(v, pair.amount);
        }
    }
}

/// A dump as text, for the comparison.
fn decode(kind: usize, b: &[u8], n_ops: usize) -> Vec<String> {
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
    out.push(format!("data kinds {set:?} flags {flags:?} science {sci} fx {fx} anim {anim:?} time {t:#x} rise {z:#x} fades {fades}"));
    if kind == 2 {
        let money = u32_(&mut p);
        let n = u32_(&mut p);
        let pairs: Vec<(String, i32)> = (0..n)
            .map(|_| (str_(&mut p), u32_(&mut p) as i32))
            .collect();
        out.push(format!("money {money} boosts {pairs:?}"));
    }
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
        out.push(format!(
            "op {i}: status {st} result {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x}"
        ));
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

/// The block `D`: the crate fields (each present with some probability), now and then a bogus token or an unknown field.
fn gen_text(g: &mut Gen, kind: usize, clean: bool) -> Vec<u8> {
    let mut lines: Vec<String> = Vec::new();
    if kind == 2 {
        if g.below(100) < 85 {
            lines.push(format!("MoneyProvided = {}", gen_int(g, clean)));
        }
        for _ in 0..g.below(4) {
            let name = pick(g, &["Up_0", "Up_1", "Up_2", "Up_3", "up_1", "Unknown"]);
            let amount = g.below(400) as i32 - 100;
            lines.push(match g.below(40) {
                0 if !clean => format!("UpgradedBoost = UpgradeType:{name}"),
                1 if !clean => format!("UpgradedBoost = Type:{name} Boost:{amount}"),
                2 if !clean => format!("UpgradedBoost = UpgradeType:{name} Bonus:{amount}"),
                3 => format!("UpgradedBoost = UpgradeType = {name} Boost = {amount}"),
                _ => format!("UpgradedBoost = UpgradeType:{name} Boost:{amount}"),
            });
        }
    }
    if g.below(100) < 14 {
        lines.push(format!("RequiredKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 14 {
        lines.push(format!("ForbiddenKindOf = {}", kind_tokens(g)));
    }
    if g.below(100) < 14 {
        lines.push(format!("ForbidOwnerPlayer = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 30 {
        lines.push(format!("BuildingPickup = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 12 {
        lines.push(format!("HumanOnly = {}", gen_bool(g, clean)));
    }
    if g.below(100) < 12 {
        lines.push(format!("PickupScience = {}", pick(g, &SCIENCES)));
    }
    if g.below(100) < 50 {
        lines.push(format!(
            "ExecuteFX = {}",
            pick(g, &["FX_A", "FX_B", "None", "fx_c"])
        ));
    }
    if g.below(100) < 45 {
        lines.push(format!(
            "ExecuteAnimation = {}",
            pick(g, &["Anim_A", "Anim_B", "anim_c"])
        ));
    }
    if g.below(100) < 30 {
        lines.push(format!("ExecuteAnimationTime = {}", gen_real(g, clean)));
    }
    if g.below(100) < 30 {
        lines.push(format!("ExecuteAnimationZRise = {}", gen_real(g, clean)));
    }
    if g.below(100) < 25 {
        lines.push(format!("ExecuteAnimationFades = {}", gen_bool(g, clean)));
    }
    if !clean {
        lines.push(match g.below(3) {
            0 => "RequiredKindOf = +BOGUS".to_string(),
            1 => "HumanOnly = Perhaps".to_string(),
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
    xfers: u64,
    collides: u64,
    destroyed: u64,
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

fn mask_of_word(w: u32) -> UpgradeMaskType {
    let mut m = UpgradeMaskType::default();
    for i in 0..32 {
        if w >> i & 1 != 0 {
            m.set(i, 1);
        }
    }
    m
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

fn set_dead(w: &mut World, id: ObjectId, dead: bool) {
    let o = w.objects.find_mut(id).unwrap();
    if dead {
        o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
    } else {
        o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
    }
}

/// Runs the ops on a fresh world and returns the dump (after the data dump) and the world.
fn rust_run(
    kind: usize,
    names: NameKeyGenerator,
    center: UpgradeCenter,
    data: ModuleDataKind,
    cfg: &[u32; 22],
    ops: &[[u32; 6]],
    tally: &mut Tally,
) -> (Vec<u8>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    // the sciences the store knows
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
    w.cuts.globals.draw_icon_ui = cfg[3] & 8 != 0;
    w.cuts.globals.anim2d_collection_present = cfg[3] & 4 != 0;
    for team in [1u32, 2] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    for i in 0..3u32 {
        let science_keys: Vec<ScienceType> = (0..5)
            .filter(|k| cfg[9] >> (5 * i + k) & 1 != 0)
            .map(|k| {
                ScienceType::from_name_key(w.names.name_to_key(SCIENCES[k as usize].as_bytes()))
            })
            .collect();
        let p = w.players.get_nth_mut(i as i32).unwrap();
        p.player_type = if cfg[8] >> i & 1 != 0 {
            PlayerType::Computer
        } else {
            PlayerType::Human
        };
        p.sciences = science_keys;
        p.upgrades_completed = mask_of_word(cfg[10 + 4 * i as usize]);
    }
    set_player_of_team(&mut w, 1, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    set_player_of_team(&mut w, 2, (cfg[1] != NO_PLAYER).then_some(cfg[1]));

    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        CLASSES[kind].as_bytes(),
        b"ModuleTag_Crate",
        &data,
    )
    .unwrap()
    .unwrap();
    let crate_template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template: crate_template,
            status: ObjectStatusMaskType::default(),
            team: Some(1),
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    let mut values = ObjectTemplateValues::default();
    for (i, (k, _)) in KINDS.iter().enumerate() {
        if cfg[2] >> i & 1 != 0 {
            values.kind_of.push(*k);
        }
    }
    let other = w
        .new_object(NewObject {
            template: Arc::new(ThingTemplate::from_object_values(&values)),
            status: ObjectStatusMaskType::default(),
            team: Some(2),
            modules: Vec::new(),
        })
        .unwrap();
    assert_eq!(other, 2);
    w.objects.find_mut(other).unwrap().ai = if cfg[3] & 1 != 0 { Some(0) } else { None };
    set_dead(&mut w, other, cfg[3] & 2 != 0);
    w.set_position(
        me,
        &Coord3D {
            x: 0.0,
            y: 0.0,
            z: f32::from_bits(cfg[4]),
        },
    );
    w.set_position(
        other,
        &Coord3D {
            x: f32::from_bits(cfg[5]),
            y: f32::from_bits(cfg[6]),
            z: f32::from_bits(cfg[7]),
        },
    );
    let idx = w.find_module(me, CLASSES[kind].as_bytes()).unwrap();

    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
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
                res = match w.module_call(
                    me,
                    idx,
                    ModuleCall::WouldLikeToCollideWith { other: target },
                ) {
                    ModuleReply::Bool(b) => i32::from(b),
                    r => panic!("unexpected reply {r:?}"),
                };
            }
            2 => set_player_of_team(&mut w, 2, (o[1] != 0).then(|| o[1] - 1)),
            3 => w.objects.find_mut(other).unwrap().ai = if o[1] != 0 { Some(0) } else { None },
            4 => set_dead(&mut w, other, o[1] != 0),
            6 => w.set_position(
                other,
                &Coord3D {
                    x: f32::from_bits(o[1]),
                    y: f32::from_bits(o[2]),
                    z: f32::from_bits(o[3]),
                },
            ),
            7 => w.set_position(
                me,
                &Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: f32::from_bits(o[1]),
                },
            ),
            8 => {
                w.cuts.globals.draw_icon_ui = o[1] & 8 != 0;
                w.cuts.globals.anim2d_collection_present = o[1] & 4 != 0;
            }
            9..=11 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 9, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 9 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            12 => set_player_of_team(&mut w, 1, (o[1] != 0).then(|| o[1] - 1)),
            13 => {
                for i in 0..3 {
                    w.players.get_nth_mut(i as i32).unwrap().upgrades_completed =
                        mask_of_word(o[1 + i]);
                }
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_i32(&mut v, res);
        // the effects, in order: the trace has each cut call, client event and module line
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
                    CutCall::PlayerHealAllObjects { player } => log.push(format!("heal {player}")),
                    CutCall::PartitionRevealMapForPlayer { player } => {
                        log.push(format!("reveal {player}"))
                    }
                    CutCall::PlayerMoneyDeposit { player, amount } => {
                        log.push(format!("deposit {player} {amount} 1"))
                    }
                    CutCall::PlayerScoreAddMoneyEarned { player, amount } => {
                        log.push(format!("earned {player} {amount}"))
                    }
                    CutCall::InGameUiAddWorldAnimation {
                        template,
                        position,
                        display_time_in_seconds,
                        z_rise_per_second,
                    } => {
                        log.push(format!("findanim {}", String::from_utf8_lossy(template)));
                        log.push(format!(
                            "anim {},{},{} 1 {} {}",
                            position.x.to_bits(),
                            position.y.to_bits(),
                            position.z.to_bits(),
                            display_time_in_seconds.to_bits(),
                            z_rise_per_second.to_bits()
                        ));
                    }
                    _ => {}
                }
            } else if t.starts_with("evt Sound") {
                if let Some(ObjectClientEvent::Sound {
                    sound,
                    object_id,
                    position,
                }) = event_iter.next()
                {
                    let name = match sound {
                        ObjectSound::CrateHeal => "crateHeal",
                        ObjectSound::CrateShroud => "crateShroud",
                        ObjectSound::CrateMoney => "crateMoney",
                        other => panic!("unexpected sound {other:?}"),
                    };
                    let pos = match position {
                        Some(p) => format!("{},{},{}", p.x.to_bits(), p.y.to_bits(), p.z.to_bits()),
                        None => "-".to_string(),
                    };
                    log.push(format!("audio {name} {pos} -1 {}", object_id));
                }
            } else if t.starts_with("evt FxListDoFxObj") {
                if let Some(ObjectClientEvent::FxListDoFxObj { fx: name, .. }) = event_iter.next() {
                    log.push(format!("fx{}", fx.id(&Some(name))));
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
    }
    (v, w)
}

fn cases() -> usize {
    std::env::var("L9CF_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

fn f32_bits(g: &mut Gen, range: f32) -> u32 {
    g.uniform(-range, range).to_bits()
}
fn pick_z(g: &mut Gen) -> u32 {
    (*pick(g, &[&0.0f32, &0.0, &0.0, &1.0, &5.0, &0.5, &-1.0, &-0.25])).to_bits()
}

#[test]
fn crate_modules_f_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_FFF5);
    let mut read = Check::new("crate module F data read (throw, message)");
    let mut run = Check::new("crate module F ops (data, effects, xfers)");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut per_kind = [0u64; 3];
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let kind = case % 3;
        let mut ctx = Ctx::new(true, true);
        cpp_reset(true, true);
        let defs: String = UPGRADES
            .iter()
            .map(|u| format!("Upgrade {u}\n Type = PLAYER\nEnd\n"))
            .collect();
        let table = blocks();
        Ini::load(
            b"Data\\INI\\Upgrade.ini",
            defs.as_bytes(),
            IniLoadType::Overwrite,
            None,
            &table,
            &mut ctx,
        )
        .expect("upgrades load");
        assert_eq!(
            cpp_load(
                b"Data\\INI\\Upgrade.ini",
                defs.as_bytes(),
                IniLoadType::Overwrite,
                false
            )
            .status,
            0
        );
        let clean = g.below(10) < 7;
        let text = gen_text(&mut g, kind, clean);
        let name = format!("Data\\INI\\Uf{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(kind, &name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the crate's player, the other's player, the other's kinds, flags, the crate's z, the other's x, y, z, the types of the players, their sciences]
        let player = |g: &mut Gen| {
            if g.below(10) == 0 {
                NO_PLAYER
            } else {
                1 + g.below(2) - u32::from(g.below(6) == 0)
            }
        };
        let mut cfg = [0u32; 22];
        cfg[0] = player(&mut g);
        cfg[1] = player(&mut g);
        cfg[2] = (g.u32() & g.u32() & 0xff) | if g.below(3) == 0 { 1 << g.below(8) } else { 0 };
        cfg[3] = (u32::from(g.below(4) != 0))
            | (u32::from(g.below(8) == 0) << 1)
            | (u32::from(g.below(4) != 0) << 2)
            | (u32::from(g.below(4) != 0) << 3);
        cfg[4] = pick_z(&mut g);
        cfg[5] = f32_bits(&mut g, 100.0);
        cfg[6] = f32_bits(&mut g, 100.0);
        cfg[7] = pick_z(&mut g);
        cfg[8] = g.below(8);
        cfg[9] = g.u32() & g.u32() & 0x7fff;
        for i in 0..3 {
            cfg[10 + 4 * i] = g.u32() & 0xf;
        }
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 6]> = (0..n_ops)
            .map(|_| match g.below(40) {
                0..=13 => [0, g.below(2), 0, 0, 0, 0],
                14..=18 => [1, g.below(2), 0, 0, 0, 0],
                19..=21 => [2, g.below(4), 0, 0, 0, 0],
                22..=23 => [3, g.below(2), 0, 0, 0, 0],
                24..=25 => [4, g.below(2), 0, 0, 0, 0],
                26..=27 => [
                    6,
                    f32_bits(&mut g, 100.0),
                    f32_bits(&mut g, 100.0),
                    pick_z(&mut g),
                    0,
                    0,
                ],
                28..=29 => [7, pick_z(&mut g), 0, 0, 0, 0],
                30..=31 => [8, g.below(16), 0, 0, 0, 0],
                32..=33 => [9, 0, 0, 0, 0, 0],
                34..=35 => [10, 0, 0, 0, 0, 0],
                36 => [11, 0, 0, 0, 0, 0],
                37 => [13, g.u32() & 0xf, g.u32() & 0xf, g.u32() & 0xf, 0, 0],
                _ => [12, g.below(4), 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, w) = rust_run(
                kind,
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
        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 21];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9cf_run(
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
        let what = || {
            format!(
                "case {case} ({}): cfg {cfg:x?}, file {:?}, ops {ops:?}",
                CLASSES[kind],
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
        let (dr, dc) = (decode(kind, &rd, ops.len()), decode(kind, &buf, ops.len()));
        for l in &dr {
            for key in [
                "\"deposit ",
                "\"earned ",
                "\"heal ",
                "\"reveal ",
                "\"audio ",
                "\"fx",
                "\"destroy ",
                "\"findanim ",
                "\"anim ",
            ] {
                *counts
                    .entry(key.trim_matches('"').trim().to_string())
                    .or_default() += l.matches(key).count() as u64;
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
                "--- case {case} ({}) cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}",
                CLASSES[kind],
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
        per_kind[kind] += 1;
        let _ = ctx.sciences;
    }
    println!(
        "crate modules F: {cases} cases ({per_kind:?} compared per kind), {read_failed} data blocks threw, {excluded} excluded (undefined upstream), \
         {ops_run} ops, {} onCollide calls, {} destroyed, {} xfers, {} effect lines",
        tally.collides, tally.destroyed, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    assert!(
        read_failed * 40 > cases as u64
            && ops_run * 5 > cases as u64 * 4
            && tally.destroyed * 10 > cases as u64,
        "the generator must reach every path"
    );
}
