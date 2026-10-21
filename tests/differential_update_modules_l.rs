#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `AutoDepositUpdate` (`logic::modules::auto_deposit_update`) against the unchanged C++, `p1w11-sonnet`, update batch L.
//!
//! # Method
//!
//! The C++ side is `AutoDepositUpdate.cpp`, compiled into `wrap_module.cpp`'s unit (`differential/l9_updates_l.inc`) with one mechanical edit (the
//! function-local `static` of `getUpgradedSupplyBoost`, a global and a flag the driver resets; `build.rs` asserts it applies once) on the module build's
//! stand-ins: an `Object` with a controlling player, status bits, a construction percentage, kinds, a geometry and a position, three players (a colour, completed
//! upgrades, a money and a score keeper that log), a game text whose label the floating text remembers, an in-game UI that logs the floating text, a game logic
//! whose frame the test sets, and the REAL `UpdateModule`, `UpgradeCenter`, `INI`, `Xfer` and client random values (`RandomValue.cpp`, seeded per case).
//!
//! The Rust side is the real thing: a `World` with the loaded upgrade center, three players (player 0 is the neutral player), one object with the module from
//! `modules::new_module`; `update()` is run as the scheduler runs it (`World::with_body`), `awardInitialCaptureBonus` is `ModuleCall::AwardInitialCaptureBonus`,
//! and the xfers go through `World::xfer_module_slot`. The money, the score and the floating text are cut calls.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the four fields and 0 to 3 `UpgradedBoost` pairs; mostly well formed) and, when the read succeeds,
//! runs 1 to 14 ops.
//!
//! Compared, bit for bit: the read's throw; the data; per op the throw status, the sleep time `update()` returned, the effects in order (`deposit`,
//! `earned`, `float`), the xfer status, bytes and CRC, and the module's three state words; and the name keys after the ops.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch E). The local player is never `NULL` on the
//! Rust side (`setLocalPlayer(NULL)` selects the neutral player), so the cases give the C++ one too. Inputs for which upstream is undefined (a deposit or a
//! floating text for an object with no controlling player; a radius whose product with `0.3f` does not fit an `int`; an `UpgradedBoost` line that ends after the
//! upgrade's name) are never sent to the C++: the Rust records an `is_undefined()` error and the case is counted as excluded.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9UL_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 456 data
//! blocks threw (read compared: 8558 comparisons, 0 mismatches), 442 cases excluded (a path upstream leaves undefined: a deposit or floating text for an
//! object with no controlling player), 59972 ops, 20915 updates, 2731 deposits, 3148 floating texts, 5209 xfers, 8610 effect lines; ops (data, effects, xfers,
//! state): 76176 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0; name keys after the ops: 8102 comparisons, 0 mismatches. First run of this
//! test (before the stand-in Money and ScoreKeeper were given their player index, the C++ log read an uninitialised index): 2596 mismatches, all in
//! that one field -- a defect of the driver, not of the module.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 901 | the function-local `static` re-initialised on every call instead of once (the upstream defect kept) |
//! | 902 | the boost of the matching pair read as 0 |
//! | 903 | `awardInitialCaptureBonus` pays whether or not the award flag is set |
//! | 904 | the construction-percentage guard skipped (a building under construction pays) |
//! | 905 | the score keeper credited with the amount plus the boost instead of the amount alone |
//! | 906 | the stealth rule inverted (display when not locally controlled and not detected) |
//! | 907 | the floating text's y scatter drawn from the major radius |
//! | 908 | the neutral-player guard skipped |
//! | 909 | a deposit amount of 0 not skipped (`< 0` for `<= 0`) |
//! | 910 | the next deposit frame set without the `DepositTiming` duration |
//!
//! All ten are caught with 9000 cases (906 and 910 steer the Rust past a path upstream leaves undefined, which makes the C++ dereference a null player:
//! the driver traps that crash and reports it as a mismatch, `status 99`, instead of dying).

mod support;
mod up_common;

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::KINDOF_STRUCTURE;
use rts_lockstep_engine::common::module::{ModuleDataKind, ObjectId};
use rts_lockstep_engine::common::object_status_types::{ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::auto_deposit_update::AutoDepositUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9autodep_l.bin");
const XFER_FILE: &str = "l9autodep_l.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
/// The team of the object.
const OBJECT_TEAM: u32 = 100;

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
    *out = Some(ModuleDataKind::AutoDepositUpdate(Arc::new(AutoDepositUpdateModuleData::friend_new_module_data(Some(ini))?)));
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<ModuleDataKind>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

/// The data dump of `ref_l9ul_run`.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::AutoDepositUpdate(d) = data else { unreachable!("not an AutoDepositUpdate") };
    put_u32(v, d.deposit_frame);
    put_i32(v, d.deposit_amount);
    put_i32(v, d.initial_capture_bonus);
    put_u32(v, u32::from(d.is_actual_money));
    put_u32(v, d.upgrade_boost.len() as u32);
    for pair in &d.upgrade_boost {
        put_str(v, &pair.type_name);
        put_i32(v, pair.amount);
    }
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
    let (frame, amount, bonus, actual) = (u32_(&mut p), u32_(&mut p) as i32, u32_(&mut p) as i32, u32_(&mut p));
    let n = u32_(&mut p);
    let pairs: Vec<(String, i32)> = (0..n).map(|_| (str_(&mut p), u32_(&mut p) as i32)).collect();
    out.push(format!("data timing {frame} amount {amount} bonus {bonus} actual {actual} boosts {pairs:?}"));
    for i in 0..n_ops {
        if p >= b.len() {
            break;
        }
        let st = u32_(&mut p) as i32;
        let res = u32_(&mut p);
        let nlog = u32_(&mut p);
        let log: Vec<String> = (0..nlog).map(|_| str_(&mut p)).collect();
        let xst = u32_(&mut p) as i32;
        let blen = u32_(&mut p);
        let mut bytes: Vec<u32> = (0..blen).map(|_| u32_(&mut p)).collect();
        if bytes.len() >= 9 {
            // the scheduling half's frame-and-phase word (after the five versions) is the sleepy list's: the C++ stand-in has no scheduler
            bytes[5..9].iter_mut().for_each(|b| *b = 0);
        }
        let crc = u32_(&mut p);
        let state: Vec<u32> = (0..3).map(|_| u32_(&mut p)).collect();
        out.push(format!("op {i}: status {st} sleep {res} log {log:?} xfer {xst} bytes {bytes:?} crc {crc:#x} state {state:?}"));
    }
    out
}

// -- generators --

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

fn gen_pair(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let name = if upgrades.is_empty() || g.below(12) == 0 { format!("Unknown{}", g.below(2)) } else { upgrades[g.below(upgrades.len() as u32) as usize].clone() };
    let amount = g.below(400) as i32 - 100;
    match g.below(40) {
        0 if !clean => format!("UpgradeType:{name}"),
        1 if !clean => format!("Type:{name} Boost:{amount}"),
        2 if !clean => format!("UpgradeType:{name} Bonus:{amount}"),
        _ => format!("UpgradeType:{name} Boost:{amount}"),
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
    let v = match g.below(10) {
        0..=5 => pick(g, &["100", "500", "1000", "1500", "3000", "33", "0"]).to_string(),
        _ => gen_dur(g, clean),
    };
    field(g, 85, "DepositTiming", v);
    let v = match g.below(10) {
        0 => format!("{}", -(g.below(50) as i32)),
        1..=7 => format!("{}", 1 + g.below(300)),
        _ => gen_int(g, clean),
    };
    field(g, 90, "DepositAmount", v);
    let v = match g.below(10) {
        0 => format!("{}", -(g.below(50) as i32)),
        1..=7 => format!("{}", 1 + g.below(500)),
        _ => gen_int(g, clean),
    };
    field(g, 60, "InitialCaptureBonus", v);
    let v = gen_bool(g, clean);
    field(g, 40, "ActualMoney", v);
    for _ in 0..g.below(4) {
        let v = gen_pair(g, upgrades, clean);
        field(g, 80, "UpgradedBoost", v);
    }
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
        let extra = if g.below(3) == 0 { g.below(n_up as u32 + 1) } else { g.below(3) };
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

fn gen_radius(g: &mut Gen) -> u32 {
    match g.below(12) {
        0..=7 => g.uniform(0.0, 100.0).to_bits(),
        8 => 0.0f32.to_bits(),
        9 => 3.0e9f32.to_bits(), // 0.3f times it does not fit an int
        10 => 5.0f32.to_bits(),
        _ => g.uniform(0.0, 4000.0).to_bits(),
    }
}

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    deposits: u64,
    floats: u64,
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


const PROBE: [&str; 9] = ["none", "Up0", "Up3", "Unknown0", "Unknown1", "Up9", "AutoDepositUpdate", "zz_fresh", "GUI:AddCash"];

fn player_color(p: i32) -> i32 {
    0x0011_2233 * (p + 1)
}

/// The effect lines of a trace's cut calls, in the driver's format.
fn effects(trace: Vec<String>, cuts: Vec<CutCall>) -> Vec<String> {
    let mut log = Vec::new();
    let mut cut_iter = cuts.into_iter();
    for t in trace {
        if t.starts_with("cut ") {
            match cut_iter.next().expect("a cut line has a cut call") {
                CutCall::PlayerMoneyDeposit { player, amount } => log.push(format!("deposit {player} {amount} 1")),
                CutCall::PlayerScoreAddMoneyEarned { player, amount } => log.push(format!("earned {player} {amount}")),
                CutCall::InGameUiAddFloatingText { amount, position, color } => {
                    log.push(format!("float GUI:AddCash {amount} {},{},{} {:08x}", position.x.to_bits(), position.y.to_bits(), position.z.to_bits(), color as u32))
                }
                _ => {}
            }
        }
    }
    log
}

/// Runs the ops on a fresh world and returns the dump (after the data dump), the name keys afterwards and the world.
fn rust_run(ctx: Ctx, data: ModuleDataKind, cfg: &[u32; 8], ops: &[[u32; 5]], tally: &mut Tally) -> (Vec<u8>, Vec<u32>, World) {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center, special_power_store: ctx.store, ..Default::default() });
    w.names = ctx.names;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.teams.insert_team(Team::new(OBJECT_TEAM, 0)).unwrap();
    w.players.set_player_count(3);
    for p in 0..3 {
        w.cuts.player_colors.insert(p, player_color(p));
    }
    if cfg[0] != NO_PLAYER {
        w.cuts.team_players.insert(OBJECT_TEAM, cfg[0] as i32);
    }
    w.players.set_local_player((cfg[1] != NO_PLAYER).then_some(cfg[1] as i32));
    w.set_frame(cfg[2]);
    w.random.init_random(cfg[3]);

    let m = rts_lockstep_engine::logic::modules::new_module(w.next_object_id(), b"AutoDepositUpdate", b"ModuleTag_Deposit", &data).unwrap().unwrap();
    let mut values = ObjectTemplateValues::default();
    if cfg[4] & 1 != 0 {
        values.kind_of.push(KINDOF_STRUCTURE);
    }
    let template = Arc::new(ThingTemplate::from_object_values(&values));
    let me = w.new_object(NewObject { template, status: ObjectStatusMaskType::default(), team: Some(OBJECT_TEAM), modules: vec![m] }).unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    {
        let o = w.objects.find_mut(me).unwrap();
        o.geometry_info.set_major_radius(f32::from_bits(cfg[5]));
        o.geometry_info.set_minor_radius(f32::from_bits(cfg[6]));
        o.construction_percent = f32::from_bits(cfg[7]);
    }
    w.set_position(me, &Coord3D { x: 11.0, y: 22.0, z: 5.0 });
    let idx = w.find_module(me, b"AutoDepositUpdate").unwrap();
    let handle = ModuleHandle { object: me, index: idx };
    let u = w.update_handle_of_module(handle).expect("an update module");
    let _ = (OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_DETECTED);
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        match o[0] {
            0 => {
                res = w.with_body(u, |b, w| b.update(w, u)).expect("the module is in its slot").0;
                tally.updates += 1;
            }
            1 => {
                w.module_call(me, idx, ModuleCall::AwardInitialCaptureBonus { player: o[1] as i32 - 1 });
            }
            2 => w.set_frame(o[1]),
            3 => {
                if o[1] == 0 {
                    w.cuts.team_players.remove(&OBJECT_TEAM);
                } else {
                    w.cuts.team_players.insert(OBJECT_TEAM, o[1] as i32 - 1);
                }
            }
            4 => w.objects.find_mut(me).unwrap().construction_percent = f32::from_bits(o[1]),
            5 => {
                let ob = w.objects.find_mut(me).unwrap();
                ob.status.set(OBJECT_STATUS_STEALTHED, i32::from(o[1] & 1 != 0));
                ob.status.set(OBJECT_STATUS_DETECTED, i32::from(o[1] & 2 != 0));
            }
            6 => {
                w.players.set_local_player(Some(o[1] as i32 - 1));
            }
            7 => {
                for p in 0..3 {
                    w.players.get_nth_mut(p).unwrap().upgrades_completed = mask;
                }
            }
            8 => {
                let g = &mut w.objects.find_mut(me).unwrap().geometry_info;
                g.set_major_radius(f32::from_bits(o[1]));
                g.set_minor_radius(f32::from_bits(o[2]));
            }
            10..=12 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 10, &saved);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 10 {
                    saved = xbytes.clone();
                }
                tally.xfers += 1;
            }
            13 => w.set_position(me, &Coord3D { x: f32::from_bits(o[1]), y: f32::from_bits(o[2]), z: f32::from_bits(o[3]) }),
            _ => unreachable!(),
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        put_u32(&mut v, if threw { 0 } else { res });
        let log = effects(w.cuts.trace.take().unwrap(), w.cuts.log.take().unwrap());
        tally.effects += log.len() as u64;
        tally.deposits += log.iter().filter(|l| l.starts_with("deposit")).count() as u64;
        tally.floats += log.iter().filter(|l| l.starts_with("float")).count() as u64;
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
        let state = w.objects.find(me).unwrap().behaviors.iter().find_map(|b| match b.body.as_ref() {
            Some(UpdateBody::AutoDepositUpdate(m)) => Some(m.state_words()),
            _ => None,
        });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w)
}

fn cases() -> usize {
    std::env::var("L9UL_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(9_000)
}

#[test]
fn auto_deposit_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x1091_AD05);
    let table = blocks();
    let mut read = Check::new("update module L data read (throw, message)");
    let mut run = Check::new("update module L ops (data, effects, xfers, state)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
    let mut shown = 0;
    for case in 0..cases {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\n Type = PLAYER\nEnd\n")).collect();
        let mut ctx = Ctx::new(init_names, init_center);
        Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        cpp_reset(init_names, init_center);
        assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

        let clean = g.below(10) < 7;
        let text = gen_data(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Ul{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's player, the local player, the frame, the client random seed, the kinds, the geometry's radii, the construction percentage]
        let mut cfg = [0u32; 8];
        cfg[0] = if g.below(10) == 0 { NO_PLAYER } else { g.below(3) };
        cfg[1] = g.below(3);
        cfg[2] = gen_frame(&mut g);
        cfg[3] = g.u32();
        cfg[4] = u32::from(g.below(10) < 6);
        cfg[5] = gen_radius(&mut g);
        cfg[6] = gen_radius(&mut g);
        cfg[7] = if g.below(8) == 0 { (g.uniform(0.0, 99.0)).to_bits() } else { (-1.0f32).to_bits() };
        let n_ops = 1 + g.below(14) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| match g.below(60) {
                0..=19 => [0, 0, 0, 0, 0],
                20..=23 => [1, g.below(4), 0, 0, 0],
                24..=31 => [2, gen_frame(&mut g), 0, 0, 0],
                32..=34 => [3, g.below(4), 0, 0, 0],
                35..=36 => [4, if g.below(2) == 0 { (-1.0f32).to_bits() } else { g.uniform(0.0, 99.0).to_bits() }, 0, 0, 0],
                37..=40 => [5, g.below(4), 0, 0, 0],
                41..=42 => [6, 1 + g.below(3), 0, 0, 0],
                43..=46 => {
                    let m = gen_mask(&mut g, n_up, 0);
                    [7, m[0], m[1], m[2], m[3]]
                }
                47..=50 => [8, gen_radius(&mut g), gen_radius(&mut g), 0, 0],
                51..=52 => [10, 0, 0, 0, 0],
                53..=54 => [11, 0, 0, 0, 0],
                55 => [12, 0, 0, 0, 0],
                _ => [13, g.uniform(-100.0, 100.0).to_bits(), g.uniform(-100.0, 100.0).to_bits(), g.uniform(-5.0, 20.0).to_bits(), 0],
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
            cpp::ref_l9ul_run(
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
        if status == 99 {
            // the C++ crashed on a case the planted Rust steered past upstream undefined behaviour: a mismatch, not a process kill
            run.eq(&"C++ crashed", &"no crash", || format!("case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text)));
            continue;
        }
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status})"),
        };
        let what = || format!("case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk)) = rust else {
            read_failed += 1;
            continue;
        };
        let (dr, dc) = (decode(&rd, ops.len()), decode(&buf, ops.len()));
        if dr != dc && shown < 6 {
            shown += 1;
            let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
            eprintln!("--- case {case} cfg {cfg:x?} file {:?}\n  rust {:?}\n  c++  {:?}", show(&text), dr.get(i), dc.get(i));
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
        "update module L: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} deposits, {} floating texts, \
         {} xfers, {} effect lines",
        tally.updates, tally.deposits, tally.floats, tally.xfers, tally.effects
    );
    read.finish();
    run.finish();
    keys.finish();
    assert!(read_failed * 40 > cases as u64 && ops_run * 5 > cases as u64 * 4 && tally.deposits * 10 > cases as u64 && tally.floats * 10 > cases as u64, "the generator must reach every path");
}
