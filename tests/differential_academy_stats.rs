// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::academy_stats` (`AcademyStats`)
//! against upstream `RTS/AcademyStats.cpp` (commit 0a05454d), compiled unchanged
//! in `differential/wrap_academy_stats.cpp` (see build.rs). Phase 1 layer 7
//! (p1w6-energy).
//!
//! # Methodology
//!
//! 1. **Random sequences.** Each case seeds the client random stream on both
//!    sides, `init`s the statistics for a side (no template, USA, China, GLA or
//!    Civilian) with a random set of the player's objects (a dozer among them,
//!    so that upstream's search for the dozer's command set runs), and then
//!    applies 1 to 60 random calls, each at a frame that moves forward (mostly
//!    by a few frames, sometimes by minutes, so the 5, 10 and 15 minute
//!    thresholds and the 2 minute income gap are crossed): `update` (with money
//!    or none, power or none, and the player's objects), one of the record
//!    calls (production with its kind-of mask and tunnel flag, upgrades,
//!    superweapons, income, and the 17 counters), and `calculateAcademyAdvice`
//!    with the local player's mines, the neutral player's sniped vehicles and
//!    the alternate-mouse option. After **every** call both sides are compared
//!    on the four probes (first update, supply center, mines, vehicles sniped),
//!    and the advice's tips are compared by label. The advice draws from the
//!    client random stream, so the draws (count, arguments and order) are part
//!    of the comparison.
//! 2. **Xfer.** At the end of each case the statistics are saved and the bytes
//!    compared with upstream's file (every field, in upstream's order). Upstream's
//!    `XferLoad` of the Rust bytes and the Rust `XferLoad` of upstream's bytes
//!    must agree on the probes.
//! 3. **The dozer's command set.** Upstream never finds it (`findDozerCommandSet`
//!    discards its result), so the supply-center cost stays 1000 and the command
//!    center stays unknown even when the player has a dozer whose command set has
//!    both. The xfer compares that cost; the command center is checked directly.
//!
//! **Planted bugs.** The call comparison also runs a Rust variant whose probes
//! report one mine more than the statistics hold. It must be caught.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_academy_stats -- --nocapture` (the `DIFF` and `PLANTED` lines).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::academy_classification::AcademyClassificationType;
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KINDOF_COMMANDCENTER, KINDOF_DISGUISER, KINDOF_DOZER, KINDOF_FS_ADVANCED_TECH, KINDOF_FS_BARRACKS,
    KINDOF_FS_BLACK_MARKET, KINDOF_FS_STRATEGY_CENTER, KINDOF_FS_SUPPLY_CENTER, KINDOF_FS_SUPPLY_DROPZONE, KINDOF_FS_WARFACTORY,
    KINDOF_HARVESTER, KINDOF_HERO, KINDOF_INFANTRY, KINDOF_MONEY_HACKER, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::random_value::RandomValues;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::academy_stats::{AcademyObject, AcademyStats, AdviceInputs};
use std::ffi::CString;
use support::{Check, Gen};

/// The kinds the objects and records draw from (the ones the advice tests).
const KINDS: [i32; 16] = [
    KINDOF_DOZER,
    KINDOF_HARVESTER,
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_HERO,
    KINDOF_FS_SUPPLY_CENTER,
    KINDOF_FS_STRATEGY_CENTER,
    KINDOF_MONEY_HACKER,
    KINDOF_FS_BLACK_MARKET,
    KINDOF_FS_SUPPLY_DROPZONE,
    KINDOF_FS_BARRACKS,
    KINDOF_FS_WARFACTORY,
    KINDOF_FS_ADVANCED_TECH,
    KINDOF_DISGUISER,
    KINDOF_COMMANDCENTER,
    KINDOF_STRUCTURE,
];

/// A side, by the wrapper's code.
const SIDES: [&str; 5] = ["", "USA", "China", "GLA", "Civilian"];

/// One object: up to six kind bits (-1 unused) and its tunnel flag.
#[derive(Clone, Debug)]
struct Obj {
    kinds: [i32; 6],
    tunnel: bool,
}

fn gen_obj(g: &mut Gen) -> Obj {
    let mut kinds = [-1i32; 6];
    let n = 1 + g.below(3) as usize;
    for k in kinds.iter_mut().take(n) {
        *k = KINDS[g.below(KINDS.len() as u32) as usize];
    }
    Obj { kinds, tunnel: g.below(6) == 0 }
}

fn to_academy_object(o: &Obj) -> AcademyObject {
    let bits: Vec<i32> = o.kinds.iter().copied().filter(|&k| k >= 0).collect();
    AcademyObject { kind_of: KindOfMaskType::from_indices(&bits), tunnel_contain: o.tunnel }
}

/// The objects as the wrapper's arrays take them: 64 slots of six kinds, and the tunnel flags.
fn encode_objects(objs: &[Obj]) -> (Vec<i32>, Vec<i32>) {
    let mut kinds = vec![-1i32; 64 * 6];
    let mut tunnels = vec![0i32; 64];
    for (i, o) in objs.iter().take(64).enumerate() {
        kinds[i * 6..i * 6 + 6].copy_from_slice(&o.kinds);
        tunnels[i] = i32::from(o.tunnel);
    }
    (kinds, tunnels)
}

#[derive(Clone, Debug)]
enum Call {
    Update { frame: u32, money: Option<u32>, power: Option<bool>, objects: Vec<Obj> },
    Produce { frame: u32, obj: Obj },
    Upgrade { frame: u32, class: i32, granted: bool },
    Special { frame: u32, class: i32 },
    Income { frame: u32 },
    /// one of the counters: code 4..=20 (see the wrapper), with an amount for code 5
    Counter { frame: u32, code: i32, amount: i32 },
    Advice { frame: u32, mines: u32, sniped: u32, alt_mouse: bool },
}

fn gen_frame_step(g: &mut Gen) -> u32 {
    match g.below(12) {
        0..=6 => g.below(40),
        7..=8 => g.below(2_000),
        9 => g.below(12_000),
        10 => g.below(40_000),
        _ => 0,
    }
}

fn gen_call(g: &mut Gen, frame: &mut u32) -> Call {
    *frame = frame.wrapping_add(gen_frame_step(g));
    let f = *frame;
    match g.below(20) {
        0..=5 => Call::Update {
            frame: f,
            money: match g.below(4) {
                0 => None,
                1 => Some(g.below(3000)),
                2 => Some(g.below(10)),
                _ => Some(g.u32() >> g.below(31)),
            },
            power: match g.below(3) {
                0 => None,
                1 => Some(true),
                _ => Some(false),
            },
            objects: (0..g.below(5)).map(|_| gen_obj(g)).collect(),
        },
        6..=8 => Call::Produce { frame: f, obj: gen_obj(g) },
        9 => Call::Upgrade { frame: f, class: g.below(3) as i32, granted: g.below(3) == 0 },
        10 => Call::Special { frame: f, class: g.below(3) as i32 },
        11 => Call::Income { frame: f },
        12..=17 => Call::Counter { frame: f, code: 4 + g.below(17) as i32, amount: g.below(100) as i32 - 20 },
        _ => Call::Advice {
            frame: f,
            mines: if g.below(3) == 0 { g.below(3) } else { 0 },
            sniped: if g.below(3) == 0 { g.below(3) } else { 0 },
            alt_mouse: g.below(2) == 0,
        },
    }
}

/// The Rust side of a call; the probes after it, and for an advice call its labels.
/// Planted: the probes report one mine more than the statistics hold.
fn rust_call(s: &mut AcademyStats, rng: &mut RandomValues, call: &Call, planted: bool) -> (Vec<u32>, Option<Vec<String>>) {
    let labels = match call {
        Call::Update { frame, money, power, objects } => {
            let objs: Vec<AcademyObject> = objects.iter().map(to_academy_object).collect();
            s.update(*frame, *money, *power, &objs);
            None
        }
        Call::Produce { frame, obj } => {
            s.record_production(*frame, &to_academy_object(obj));
            None
        }
        Call::Upgrade { class, granted, .. } => {
            s.record_upgrade(AcademyClassificationType::from_index(*class).expect("class"), *granted);
            None
        }
        Call::Special { class, .. } => {
            s.record_special_power_used(AcademyClassificationType::from_index(*class).expect("class"));
            None
        }
        Call::Income { frame } => {
            s.record_income(*frame);
            None
        }
        Call::Counter { code, amount, .. } => {
            counter(s, *code, *amount);
            None
        }
        Call::Advice { frame, mines, sniped, alt_mouse } => {
            let inputs = AdviceInputs { local_mines: *mines, neutral_vehicles_sniped: *sniped, use_alternate_mouse: *alt_mouse };
            let advice = s.calculate_academy_advice(*frame, &inputs, rng);
            Some(advice.tips.iter().map(|t| t.to_string()).collect())
        }
    };
    (probes(s, planted), labels)
}

/// The record call `code` of the wrapper (4..=20).
fn counter(s: &mut AcademyStats, code: i32, amount: i32) {
    match code {
        4 => s.record_building_capture(),
        5 => s.record_generals_points_spent(amount),
        6 => s.record_building_garrisoned(),
        7 => s.record_drag_selection(),
        8 => s.record_strategy_center(),
        9 => s.record_battle_plan_selected(),
        10 => s.record_unit_entered_tunnel_network(),
        11 => s.record_control_groups_used(),
        12 => s.record_cleared_garrisoned_building(),
        13 => s.record_vehicle_disguised(),
        14 => s.record_firestorm_created(),
        15 => s.record_guard_ability_used(),
        16 => s.record_salvage_collected(),
        17 => s.record_double_click_attack_move_order_given(),
        18 => s.record_mine_cleared(),
        19 => s.record_vehicle_sniped(),
        20 => s.record_mine(),
        _ => unreachable!("counter codes are 4..=20"),
    }
}

/// The five probes, as the wrapper's `ref_academy_probe` gives them. Planted:
/// the mines count is one more than the statistics hold.
fn probes(s: &AcademyStats, planted: bool) -> Vec<u32> {
    vec![
        u32::from(s.is_first_update()),
        u32::from(s.had_a_supply_center()),
        s.mines() + u32::from(planted),
        s.vehicles_sniped(),
        u32::from(s.command_center_template().is_some()),
    ]
}

/// The C++ side of a call, as the wrapper takes it.
fn cpp_call(call: &Call) -> (Vec<u32>, Option<Vec<String>>) {
    let mut probe = [0u32; 5];
    let mut labels_out: Option<Vec<String>> = None;
    match call {
        Call::Update { frame, money, power, objects } => {
            let (kinds, tunnels) = encode_objects(objects);
            let m = money.map_or(-1, |m| m as i32);
            let p = match power {
                None => -1,
                Some(v) => i32::from(*v),
            };
            unsafe { cpp::ref_academy_update(*frame, m, p, kinds.as_ptr(), tunnels.as_ptr(), objects.len().min(64) as i32) };
        }
        Call::Produce { frame, obj } => unsafe {
            cpp::ref_academy_record(0, *frame, 0, 0, obj.kinds.as_ptr(), i32::from(obj.tunnel))
        },
        Call::Upgrade { frame, class, granted } => unsafe {
            cpp::ref_academy_record(1, *frame, *class, i32::from(*granted), [-1; 6].as_ptr(), 0)
        },
        Call::Special { frame, class } => unsafe { cpp::ref_academy_record(2, *frame, *class, 0, [-1; 6].as_ptr(), 0) },
        Call::Income { frame } => unsafe { cpp::ref_academy_record(3, *frame, 0, 0, [-1; 6].as_ptr(), 0) },
        Call::Counter { frame, code, amount } => unsafe {
            cpp::ref_academy_record(*code, *frame, *amount, 0, [-1; 6].as_ptr(), 0)
        },
        Call::Advice { frame, mines, sniped, alt_mouse } => {
            let mut buf = vec![0u8; 4096];
            let n = unsafe {
                cpp::ref_academy_advice(*frame, *mines as i32, *sniped as i32, i32::from(*alt_mouse), buf.as_mut_ptr() as *mut _, buf.len() as i32)
            };
            let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            let text = String::from_utf8_lossy(&buf[..end]).into_owned();
            let labels: Vec<String> = if text.is_empty() { Vec::new() } else { text.split('\n').map(str::to_string).collect() };
            assert_eq!(labels.len(), n as usize, "the wrapper's count and labels agree");
            labels_out = Some(labels);
        }
    }
    unsafe { cpp::ref_academy_probe(probe.as_mut_ptr()) };
    (probe.to_vec(), labels_out)
}

/// The C++ stand-ins keep the player, the statistics and the client seed in
/// globals (wrap_academy_stats.cpp), so the tests that call them run one at a time.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn temp_path(tag: &str, n: u32) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("p1w6-academy-{}-{tag}-{n}.bin", std::process::id()))
}

#[test]
fn academy_calls_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    const CASES: u32 = 2_000;
    let mut g = Gen::new(0x3064_2001);
    let mut ok = Check::new("AcademyStats calls (probes, advice labels)");
    let mut bug = Check::new("AcademyStats calls (planted: mines off by one)");
    let (mut advice_calls, mut advice_given, mut tips_given) = (0u32, 0u32, 0u32);
    for case in 0..CASES {
        let seed = g.u32() | 1;
        let side = g.below(SIDES.len() as u32) as i32;
        let mut frame = g.below(5_000);
        // a dozer among the objects, so that upstream's search for its command set runs
        let mut init_objects: Vec<Obj> = (0..g.below(6)).map(|_| gen_obj(&mut g)).collect();
        init_objects.push(Obj { kinds: [KINDOF_DOZER, -1, -1, -1, -1, -1], tunnel: false });
        let (k, t) = encode_objects(&init_objects);
        unsafe {
            cpp::ref_init_random(seed);
            cpp::ref_academy_init(frame, side, k.as_ptr(), t.as_ptr(), init_objects.len() as i32);
        }
        let mut rng = RandomValues::new(seed);
        let mut s = AcademyStats::default();
        s.init(frame, if side == 0 { None } else { Some(SIDES[side as usize].as_bytes()) });
        let mut p = s.clone();
        let mut prng = RandomValues::new(seed);
        let mut trace: Vec<String> = Vec::new();
        for step in 0..1 + g.below(60) {
            let call = gen_call(&mut g, &mut frame);
            trace.push(format!("{call:?}"));
            let input = || format!("case {case} step {step} (side {side}): {}", trace.join(", "));
            let (cpp_probe, cpp_labels) = cpp_call(&call);
            if let Some(labels) = &cpp_labels {
                advice_calls += 1;
                advice_given += u32::from(!labels.is_empty());
                tips_given += labels.len() as u32;
            }
            let (rust_probe, rust_labels) = rust_call(&mut s, &mut rng, &call, false);
            ok.eq((rust_probe, rust_labels.map(|l| l.join("\n"))), (cpp_probe.clone(), cpp_labels.clone().map(|l| l.join("\n"))), input);
            let (planted_probe, planted_labels) = rust_call(&mut p, &mut prng, &call, true);
            bug.eq((planted_probe, planted_labels.map(|l| l.join("\n"))), (cpp_probe, cpp_labels.map(|l| l.join("\n"))), input);
        }
        let input = || format!("case {case}");
        let mut save = XferSave::new();
        save.open(b"academy").expect("open");
        s.xfer(&mut save).expect("save");
        save.close().expect("close");
        let rust_bytes = save.take_bytes();
        let path = temp_path("save", case);
        let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
        assert_eq!(unsafe { cpp::ref_academy_save(cpath.as_ptr()) }, 0, "upstream XferSave threw");
        let cpp_bytes = std::fs::read(&path).expect("upstream wrote the file");
        let _ = std::fs::remove_file(&path);
        ok.eq(rust_bytes.clone(), cpp_bytes.clone(), input);

        // load both ways
        let path2 = temp_path("load", case);
        std::fs::write(&path2, &rust_bytes).expect("write");
        let cpath2 = CString::new(path2.to_string_lossy().as_bytes()).expect("path");
        let mut out = [0u32; 6];
        unsafe { cpp::ref_academy_load(cpath2.as_ptr(), out.as_mut_ptr()) };
        let _ = std::fs::remove_file(&path2);
        assert_eq!(out[0], 0, "upstream XferLoad threw");
        let mut load = XferLoad::new(rust_bytes.clone());
        load.open(b"academy").expect("open");
        let mut back = AcademyStats::default();
        back.xfer(&mut load).expect("load");
        ok.eq(probes(&back, false), out[1..].to_vec(), input);
        let mut load2 = XferLoad::new(cpp_bytes.clone());
        load2.open(b"academy").expect("open");
        let mut back2 = AcademyStats::default();
        back2.xfer(&mut load2).expect("load");
        ok.eq(probes(&back2, false), out[1..].to_vec(), input);
    }
    println!("advice: {advice_calls} calls, {advice_given} gave a tip ({tips_given} tips in all)");
    ok.finish();
    bug.finish_planted();
}
