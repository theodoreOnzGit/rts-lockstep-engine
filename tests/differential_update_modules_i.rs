#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpyVisionUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `SpyVisionUpdate` (`logic::modules::spy_vision_update`, with `logic::modules::upgrade_mux_user`) against the unchanged C++, `p1w11-sonnet`, update
//! batch I.
//!
//! # Method
//!
//! The C++ side is `SpyVisionUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_updates_i.inc`) on the module build's
//! stand-ins: three players (a relationship to the default team of each, and a `setUnitsVisionSpied` that logs), a player list of them, an `Object`
//! with a controlling player, a game logic whose frame the test sets and whose wake recorder counts the calls `setWakeFrame` makes, and the REAL
//! `UpdateModule`, `UpgradeMux`, `UpgradeCenter`, `INI` and `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the loaded upgrade center, three players with default teams and relationships, and one object with
//! the module from `modules::new_module`; `update()` is run as the scheduler runs it (`World::with_body`), the other calls are `World::module_call`
//! (`ActivateSpyVision`, `OnCapture`, `OnDisabledEdge`, `OnDelete` and the upgrade calls), and the xfers go through `World::xfer_module_slot`.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the module's four fields, `SpyOnKindof` and the mux's five; mostly well formed), and when the
//! read succeeds runs 1 to 14 ops.
//!
//! Compared, bit for bit: the read's throw; the data (the mux, the four fields, the kinds); the constructor's wake calls; per op the throw status, the
//! result (the sleep time or the upgrade call's answer), `isAlreadyUpgraded`, the effects in order (`spied`, `fx`, `remove`), the number of wake calls and
//! the frame of the last, the xfer status, bytes and CRC, and the module's four state words; and the name keys after the ops (the mux assigns them
//! on first use).
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch E): that word is the sleepy
//! list's, and the C++ stand-in has no scheduler. Inputs for which upstream is undefined (an upgrade name that is not an upgrade at the first use of the mux, with
//! an empty or `None` name: a `NULL` template) are never sent to the C++: the Rust records an `is_undefined()` error and the case is counted as excluded.
//!
//! # Results
//!
//! (2026-10-21, 9000 cases, `L9UI_CASES` overrides.) 8,920 data reads, 128,932 dump lines (62,604 ops: 10,388 `update()`, 702 upgrades fired, 7,440 xfers)
//! and 8,291 name-key checks compared, 0 mismatches; 80 inputs excluded as undefined upstream; 629 data blocks threw on both sides. Effects: 9,016
//! `spied`, 333 `remove`, 144 `fx`.
//!
//! # Upstream facts kept
//!
//! - `SpyVisionUpdate::xfer` does not save the `UpgradeMux`'s `m_upgradeExecuted` (an `UpgradeModule` does): after a load the upgrade may run again.
//!   The state words the test compares do not include it for that reason, but `isAlreadyUpgraded` is compared after every op.
//! - The module is created under construction here (as in batch C): `initObject` runs the upgrade pass of a finished object, which resolves the
//!   mux's names; the C++ driver makes the object without it.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 861 | `activateSpyVision(0)` deactivates at once, not never |
//! | 862 | the deactivation is due one frame late |
//! | 863 | `setDisabledUntilFrame(now)` counts as a disable |
//! | 864 | `onCapture` does not switch the old owner's off |
//! | 865 | neutral players are spied on too |
//! | 866 | an always-on self-powered module does not turn itself on after a disable |
//! | 867 | `onDelete` leaves the vision on |
//! | 868 | `upgradeImplementation` ignores `NeedsUpgrade` |
//! | 869 | the self-powered deactivate frame uses the interval |
//! | 870 | `xfer` leaves out the reset flag and the disabled-until frame |
//! | 871 | the mux never marks the upgrade executed |
//!
//! All 11 are caught (3000 cases each), all as mismatches.

mod support;
mod up_common;

use rts_lockstep_engine::common::game_common::Relationship;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{KindOfMaskType, KINDOF_COUNT};
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::spy_vision_update::SpyVisionUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9spy_i.bin");
const XFER_FILE: &str = "l9spy_i.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
/// The team of the object.
const OBJECT_TEAM: u32 = 100;

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
fn put_names(v: &mut Vec<u8>, names: &[Vec<u8>]) {
    put_u32(v, names.len() as u32);
    for n in names {
        put_str(v, n);
    }
}
fn put_words(v: &mut Vec<u8>, w: [u64; 4]) {
    for x in w {
        v.extend_from_slice(&x.to_le_bytes());
    }
}
fn put_mux(v: &mut Vec<u8>, m: &UpgradeMuxData, fx: &mut FxIds) {
    put_names(v, &m.trigger_upgrade_names);
    put_names(v, &m.activation_upgrade_names);
    put_names(v, &m.conflicting_upgrade_names);
    put_names(v, &m.removal_upgrade_names);
    put_i32(v, fx.id(&m.fx_list_upgrade));
    put_words(v, m.activation_mask().words());
    put_words(v, m.conflicting_mask().words());
    v.push(u8::from(m.requires_all_triggers));
}
/// A kind-of mask as the driver prints it in a log line: four words, hex, the highest first.
fn kind_words(m: &KindOfMaskType) -> String {
    let mut w = [0u32; 4];
    for i in 0..KINDOF_COUNT {
        if m.test(i) {
            w[(i / 32) as usize] |= 1 << (i % 32);
        }
    }
    format!("{:08x}{:08x}{:08x}{:08x}", w[3], w[2], w[1], w[0])
}
/// A kind-of mask as the driver prints it in the data dump: the indices of its set bits, each followed by a comma.
fn kind_str(m: &KindOfMaskType) -> String {
    (0..KINDOF_COUNT)
        .filter(|&i| m.test(i))
        .map(|i| format!("{i},"))
        .collect()
}

fn mask_from(w: &[u32]) -> UpgradeMaskType {
    let mut m = UpgradeMaskType::default();
    for i in 0..128 {
        if w[i / 32] >> (i % 32) & 1 != 0 {
            m.set(i as i32, 1);
        }
    }
    m
}

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::SpyVisionUpdate(Arc::new(
        SpyVisionUpdateModuleData::friend_new_module_data(Some(ini))?,
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

/// The data dump of `ref_l9ui_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::SpyVisionUpdate(d) = data else {
        unreachable!("not a SpyVisionUpdate")
    };
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_u32(v, u32::from(d.needs_upgrade));
    put_u32(v, u32::from(d.self_powered));
    put_u32(v, d.self_powered_duration);
    put_u32(v, d.self_powered_interval);
    put_str(v, kind_str(&d.spy_on_kindof).as_bytes());
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
    let u64_ = |p: &mut usize| -> u64 {
        let v = u64::from_le_bytes(b[*p..*p + 8].try_into().unwrap());
        *p += 8;
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
    for what in ["trigger", "activation", "conflicting", "removal"] {
        let n = u32_(&mut p);
        let names: Vec<String> = (0..n).map(|_| str_(&mut p)).collect();
        out.push(format!("mux {what} {names:?}"));
    }
    let fx = u32_(&mut p) as i32;
    let act: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let con: Vec<u64> = (0..4).map(|_| u64_(&mut p)).collect();
    let all = b[p];
    p += 1;
    out.push(format!("mux fx {fx} act {act:x?} con {con:x?} all {all}"));
    let (needs, selfp, dur, int) = (u32_(&mut p), u32_(&mut p), u32_(&mut p), u32_(&mut p));
    let kinds = str_(&mut p);
    out.push(format!(
        "data needs {needs} selfpowered {selfp} duration {dur} interval {int} kinds [{kinds}]"
    ));
    let (n, f) = (u32_(&mut p), u32_(&mut p));
    out.push(format!("constructor wakes {n} last {f}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let up = u32_(&mut p);
        let nlog = u32_(&mut p);
        let log: Vec<String> = (0..nlog).map(|_| str_(&mut p)).collect();
        let (wakes, last) = (u32_(&mut p), u32_(&mut p));
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let mut bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        if bytes.len() >= 9 {
            // the scheduling half's frame-and-phase word (after the five versions) is the sleepy list's: the C++ stand-in has no scheduler
            bytes[5..9].iter_mut().for_each(|b| *b = 0);
        }
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..4).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} result {res} upgraded {up} log {log:?} wakes {wakes} last {last} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:?}"));
    }
    out
}

// -- generators --

fn gen_name(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    if clean && !upgrades.is_empty() {
        return upgrades[g.below(upgrades.len() as u32) as usize].clone();
    }
    match g.below(100) {
        0..=64 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].clone(),
        65..=74 if !upgrades.is_empty() => {
            upgrades[g.below(upgrades.len() as u32) as usize].to_lowercase()
        }
        75..=76 => format!("Unknown{}", g.below(3)),
        77 => "Up9".to_string(),
        78..=79 => pick(g, &["None", "none", "NONE"]).to_string(),
        _ if !upgrades.is_empty() => upgrades[0].clone(),
        _ => format!("Unknown{}", g.below(3)),
    }
}

fn gen_list(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let n = 1 + g.below(3);
    (0..n)
        .map(|_| gen_name(g, upgrades, clean))
        .collect::<Vec<_>>()
        .join(" ")
}

const KIND_NAMES: [&str; 8] = [
    "STRUCTURE",
    "INFANTRY",
    "VEHICLE",
    "AIRCRAFT",
    "SELECTABLE",
    "CAN_ATTACK",
    "SCORE",
    "HUGE_VEHICLE",
];

fn gen_kind_list(g: &mut Gen, clean: bool) -> String {
    match g.below(8) {
        0 => "NONE".to_string(),
        1 if !clean => pick(
            g,
            &[
                "Bogus",
                "+STRUCTURE",
                "STRUCTURE +VEHICLE",
                "",
                "-INFANTRY",
                "KINDOF_VEHICLE",
            ],
        )
        .to_string(),
        2 => format!("+{}", pick(g, &KIND_NAMES)),
        3 => format!("-{}", pick(g, &KIND_NAMES)),
        _ => (0..1 + g.below(3))
            .map(|_| pick(g, &KIND_NAMES).to_string())
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=2 => "0".to_string(),
        3..=7 => pick(g, &["100", "500", "1000", "3000", "10000", "33", "34"]).to_string(),
        8 => format!("{}", g.below(100_000)),
        _ if clean => "1000".to_string(),
        9 => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => gen_int(g, false),
    }
}

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_list(g, upgrades, clean);
    field(g, 70, "TriggeredBy", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 25, "ConflictsWith", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 25, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None"]).to_string();
    field(g, 30, "FXListUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 25, "RequiresAllTriggers", v);
    let v = gen_bool(g, clean);
    field(g, 50, "NeedsUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 55, "SelfPowered", v);
    let v = gen_dur(g, clean);
    field(g, 65, "SelfPoweredDuration", v);
    let v = gen_dur(g, clean);
    field(g, 65, "SelfPoweredInterval", v);
    let v = gen_kind_list(g, clean);
    field(g, 40, "SpyOnKindof", v);
    if !clean && g.below(25) == 0 {
        lines.push(format!("Bogus{}1", gen_sep(g)));
    }
    let mut s = String::from("D\n");
    for l in &lines {
        s.push_str(gen_indent(g));
        s.push_str(l);
        s.push('\n');
    }
    if clean || g.below(30) != 0 {
        s.push_str("End\n");
    }
    s.into_bytes()
}

fn gen_mask(g: &mut Gen, n_up: usize, at_least: u32) -> [u32; 4] {
    let mut w = [0u32; 4];
    if n_up > 0 {
        // a third of the masks name most of the upgrades, so that a TriggeredBy list is met more often than by chance
        let extra = if g.below(3) == 0 {
            g.below(n_up as u32 + 1)
        } else {
            g.below(3)
        };
        for _ in 0..at_least + extra {
            let b = g.below(n_up as u32) as usize;
            w[b / 32] |= 1 << (b % 32);
        }
    }
    w
}

fn gen_frame(g: &mut Gen) -> u32 {
    match g.below(12) {
        0..=6 => g.below(400),
        7..=8 => g.below(10_000),
        9 => g.below(2_000_000),
        10 => u32::MAX - g.below(40),
        _ => 0,
    }
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    fired: u64,
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

fn relationship(v: u32) -> Relationship {
    match v & 3 {
        0 => Relationship::Enemies,
        2 => Relationship::Allies,
        _ => Relationship::Neutral,
    }
}

/// Player `a`'s relationship to the default team of player `b` is bits `2 * (3 * a + b)` of `word`.
fn set_matrix(w: &mut World, word: u32) {
    for a in 0..3 {
        for b in 0..3 {
            w.cuts
                .player_relationships
                .insert((a, 101 + b as u32), relationship(word >> (2 * (3 * a + b))));
        }
    }
}

fn set_controller(w: &mut World, player: Option<u32>) {
    match player {
        Some(p) => {
            w.cuts.team_players.insert(OBJECT_TEAM, p as i32);
        }
        None => {
            w.cuts.team_players.remove(&OBJECT_TEAM);
        }
    }
}

/// The effect lines of a trace, in the driver's format, and the wake calls of module `idx` among them.
fn effects(
    trace: Vec<String>,
    cuts: Vec<CutCall>,
    fxs: Vec<Vec<u8>>,
    idx: u16,
    fx: &mut FxIds,
) -> (Vec<String>, u32, u32) {
    let (mut log, mut wakes, mut last) = (Vec::new(), 0u32, 0u32);
    let mut cut_iter = cuts.into_iter();
    let mut fx_iter = fxs.into_iter();
    for t in trace {
        if let Some(f) = t.strip_prefix("awaken ") {
            let (i, f) = f.split_once(' ').unwrap();
            if i.parse::<i32>().unwrap() == i32::from(idx) {
                wakes += 1;
                last = f.parse().unwrap();
            }
        } else if t.starts_with("cut ") {
            if let Some(CutCall::PlayerSetUnitsVisionSpied {
                player,
                setting,
                which_units,
                by_whom,
            }) = cut_iter.next()
            {
                log.push(format!(
                    "spied {player} {} {by_whom} {}",
                    i32::from(setting),
                    kind_words(&which_units)
                ));
            }
        } else if t.starts_with("evt FxListDoFxObj") {
            log.push(format!("fx{}", fx.id(&fx_iter.next())));
        } else if let Some(n) = t.strip_prefix("remove ") {
            log.push(format!("remove {n}"));
        }
    }
    (log, wakes, last)
}

const PROBE: [&str; 9] = [
    "none",
    "Up0",
    "Up3",
    "Unknown0",
    "Unknown2",
    "Up9",
    "SCIENCE_Pathfinder",
    "zz_fresh",
    "SpyVisionUpdate",
];

/// Runs the ops on a fresh world and returns the dump (after the data dump), the name keys afterwards and the world.
fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32; 3],
    ops: &[[u32; 5]],
    tally: &mut Tally,
) -> (Vec<u8>, Vec<u32>, World) {
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_data(&mut v, &data, &mut fx);

    let mut w = World::new();
    w.statics = Arc::new(GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        ..Default::default()
    });
    w.names = ctx.names;
    w.cuts.probe_effect_trace = true;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    for team in [OBJECT_TEAM, 101, 102, 103] {
        w.teams.insert_team(Team::new(team, 0)).unwrap();
    }
    w.players.set_player_count(3);
    for i in 0..3 {
        w.players.get_nth_mut(i).unwrap().default_team = Some(101 + i as u32);
    }
    set_matrix(&mut w, cfg[1]);
    set_controller(&mut w, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    w.set_frame(cfg[2]);

    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"SpyVisionUpdate",
        b"ModuleTag_Spy",
        &data,
    )
    .unwrap()
    .unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    // made under construction, as batch C does: `initObject` runs the upgrade pass (`updateUpgradeModules`) of a finished object, which resolves the mux's
    // names; the C++ driver makes the object with no such pass
    let mut status = ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let me = w
        .new_object(NewObject {
            template,
            status,
            team: Some(OBJECT_TEAM),
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    w.objects
        .find_mut(me)
        .unwrap()
        .status
        .set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
    let idx = w.find_module(me, b"SpyVisionUpdate").unwrap();
    let handle = ModuleHandle {
        object: me,
        index: idx,
    };
    let u = w.update_handle_of_module(handle).expect("an update module");
    // the constructor's wake calls
    let (_, wakes, last) = effects(
        w.cuts.trace.take().unwrap_or_default(),
        Vec::new(),
        Vec::new(),
        idx,
        &mut fx,
    );
    put_u32(&mut v, wakes);
    put_u32(&mut v, last);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w
                    .with_body(u, |b, w| b.update(w, u))
                    .expect("the module is in its slot")
                    .0;
                tally.updates += 1;
            }
            1 => {
                w.module_call(me, idx, ModuleCall::ActivateSpyVision { duration: o[1] });
            }
            2 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::SetSpyVisionDisabledUntilFrame { frame: o[1] },
                );
            }
            3 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::OnCapture {
                        old_owner: o[1] as i32 - 1,
                        new_owner: o[2] as i32 - 1,
                    },
                );
            }
            4 => {
                w.module_call(
                    me,
                    idx,
                    ModuleCall::OnDisabledEdge {
                        becoming_disabled: o[1] & 1 != 0,
                    },
                );
            }
            5 => {
                w.module_call(me, idx, ModuleCall::OnDelete);
            }
            6 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::AttemptUpgrade(mask))
                        .as_bool(),
                );
                tally.fired += u64::from(res);
            }
            7 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::WouldUpgrade(mask))
                        .as_bool(),
                )
            }
            8 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::ResetUpgrade(mask))
                        .as_bool(),
                )
            }
            9 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded)
                        .as_bool(),
                )
            }
            10 => {
                w.module_call(me, idx, ModuleCall::ForceRefreshUpgrade);
            }
            11 => w.set_frame(o[1]),
            12 => set_controller(&mut w, (o[1] != 0).then(|| o[1] - 1)),
            13 => set_matrix(&mut w, o[1]),
            14..=16 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 14, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 14 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, if threw { 0 } else { res });
        put_u32(
            &mut v,
            u32::from(
                w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded)
                    .as_bool(),
            ),
        );
        let fxs: Vec<Vec<u8>> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                rts_lockstep_engine::logic::client_event::ClientEvent::Object(rts_lockstep_engine::logic::world::object_events::ObjectClientEvent::FxListDoFxObj { fx, .. }) => Some(fx.clone()),
                _ => None,
            })
            .collect();
        let (log, wakes, last) = effects(
            w.cuts.trace.take().unwrap(),
            w.cuts.log.take().unwrap(),
            fxs,
            idx,
            &mut fx,
        );
        tally.effects += log.len() as u64;
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_u32(&mut v, wakes);
        put_u32(&mut v, if wakes > 0 { last } else { 0 });
        put_i32(&mut v, xst);
        put_u32(&mut v, xbytes.len() as u32);
        for b in &xbytes {
            put_u32(&mut v, u32::from(*b));
        }
        put_u32(&mut v, crc);
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::SpyVisionUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        let state = state.expect("the module is back in its slot");
        for s in &state[..4] {
            put_u32(&mut v, *s);
        }
    }
    let keys = PROBE
        .iter()
        .map(|n| w.names.name_to_key(n.as_bytes()).0)
        .collect();
    (v, keys, w)
}

fn cases() -> usize {
    std::env::var("L9UI_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn spy_vision_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_5A1B);
    let table = blocks();
    let mut read = Check::new("update module I data read (throw, message)");
    let mut run = Check::new("update module I ops (data, wakes, effects, xfers, state)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut shown = 0;
    for case in 0..cases {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 {
            0
        } else {
            1 + g.below(8) as usize
        };
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades
            .iter()
            .map(|u| format!("Upgrade {u}\nEnd\n"))
            .collect();
        let mut ctx = Ctx::new(init_names, init_center);
        Ini::load(
            b"Data\\INI\\Up.ini",
            defs.as_bytes(),
            IniLoadType::Overwrite,
            None,
            &table,
            &mut ctx,
        )
        .expect("upgrades load");
        cpp_reset(init_names, init_center);
        assert_eq!(
            cpp_load(
                b"Data\\INI\\Up.ini",
                defs.as_bytes(),
                IniLoadType::Overwrite,
                false
            )
            .status,
            0
        );

        let clean = g.below(10) < 7;
        let text = gen_data(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Ui{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's player, the relationships to the default teams, the frame]
        let mut cfg = [0u32; 3];
        cfg[0] = if g.below(8) == 0 {
            NO_PLAYER
        } else {
            g.below(3)
        };
        cfg[1] = (0..9).fold(0u32, |acc, i| acc | (g.below(3) << (2 * i)));
        cfg[2] = gen_frame(&mut g);
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| match g.below(60) {
                0..=9 => [0, 0, 0, 0, 0],
                10..=14 => [
                    1,
                    pick(&mut g, &[&0u32, &0, &30, &100, &900, &3000]).to_owned(),
                    0,
                    0,
                    0,
                ],
                15..=17 => [
                    2,
                    pick(&mut g, &[&0u32, &u32::MAX, &50, &10_000, &1]).to_owned(),
                    0,
                    0,
                    0,
                ],
                18..=20 => [3, g.below(4), g.below(4), 0, 0],
                21..=24 => [4, g.below(2), 0, 0, 0],
                25..=26 => [5, 0, 0, 0, 0],
                27..=33 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [6, m[0], m[1], m[2], m[3]]
                }
                34..=36 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [7, m[0], m[1], m[2], m[3]]
                }
                37..=39 => {
                    let m = gen_mask(&mut g, n_up, 1);
                    [8, m[0], m[1], m[2], m[3]]
                }
                40..=41 => [9, 0, 0, 0, 0],
                42..=43 => [10, 0, 0, 0, 0],
                44..=48 => [11, gen_frame(&mut g), 0, 0, 0],
                49..=50 => [12, g.below(4), 0, 0, 0],
                51..=52 => [
                    13,
                    (0..9).fold(0u32, |acc, i| acc | (g.below(3) << (2 * i))),
                    0,
                    0,
                    0,
                ],
                53..=54 => [14, 0, 0, 0, 0],
                55..=56 => [15, 0, 0, 0, 0],
                _ => [16, 0, 0, 0, 0],
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w) = rust_run(ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            Some((rd, rk))
        } else {
            None
        };
        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let cxfer = CString::new(XFER_FILE_PATH).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 22];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9ui_run(
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
                "case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}",
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
        let Some((rd, rk)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        for l in &dr {
            for key in ["spied", "remove ", "fx"] {
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
                "--- case {case} cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}",
                show(&text),
                dr.get(i),
                dc.get(i)
            );
        }
        run.eq(&dr.len(), &dc.len(), what);
        for (a, b) in dr.iter().zip(dc.iter()) {
            run.eq(a, b, what);
        }
        let ck: Vec<u32> = PROBE
            .iter()
            .map(|n| {
                let c = CString::new(*n).unwrap();
                unsafe { cpp::ref_up_name_key(c.as_ptr()) }
            })
            .collect();
        keys.eq(rk, ck, what);
        ops_run += ops.len() as u64;
    }
    println!(
        "update module I: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} upgrades fired, \
         {} xfers, {} effect lines",
        tally.updates, tally.fired, tally.xfers, tally.effects
    );
    for (k, c) in &counts {
        println!("RESULT   effect {k:<14} {c}");
    }
    read.finish();
    run.finish();
    keys.finish();
    assert!(
        read_failed * 40 > cases as u64
            && ops_run * 5 > cases as u64 * 4
            && tally.fired * 20 > cases as u64,
        "the generator must reach every path"
    );
}
