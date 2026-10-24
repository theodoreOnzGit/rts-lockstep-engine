#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoHealBehavior.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `AutoHealBehavior` (`logic::modules::auto_heal_behavior`, with `logic::modules::upgrade_mux_user`) against the unchanged C++, `p1w12-sonnet`,
//! update batch M.
//!
//! # Method
//!
//! The C++ side is `AutoHealBehavior.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_auto_heal_m.inc`), with the healing
//! functions of `Object` cut verbatim out of `Object.cpp` by `build.rs` (`attemptHealing`, `getSoleHealingBenefactor`,
//! `attemptHealingFromSoleBenefactor`; both markers asserted once), on the module build's stand-ins: four objects (a player, kinds, an
//! effectively-dead flag, an off-map flag, a body with a health and a maximum that logs the healing it is given, a position and a geometry height),
//! three players (an object list for `Player::iterateObjects`), a partition manager whose range scan is the list the test gives, a particle system
//! manager that logs, the world animation log of crate batch F, a game logic whose frame the test sets and whose wake recorder counts the calls
//! `setWakeFrame` makes, and the REAL `UpdateModule`, `UpgradeMux`, `UpgradeCenter`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with the loaded upgrade center, three players and four objects (object 1 has the module from
//! `modules::new_module`; each has a scripted probe body, `module_probe.rs`, whose healing is a line of the trace when `probe_effect_trace` asks);
//! `update()` is run as the scheduler runs it (`World::with_body`), `onDamage`, `stopHealing`, `undoUpgrade` and the upgrade calls are
//! `World::module_call`, and the xfers go through `World::xfer_module_slot`. The scans are the tables `cuts.range_scan` and `cuts.player_object_list`.
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the module's twelve fields and the mux's five; mostly well formed), and when the read
//! succeeds makes the module (compared: its constructor's effects and wake calls) and runs 1 to 30 ops.
//!
//! Compared, bit for bit: the read's throw; the data; the constructor's effects and wakes; per op the throw status, the result (the sleep time or the
//! upgrade call's answer), `isAlreadyUpgraded`, the effects in order (`heal`, `psys`, `findanim`, `anim`, `fx`, `remove`), the wake calls (number and
//! frame of the last), the xfer status, bytes and CRC, the module's three state words, and for each object its health and its sole healing
//! benefactor; and the name keys after the ops.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). Inputs for which upstream
//! is undefined (a scanned object with no body module; an upgrade name that is not an upgrade, with an empty or `None` name) and a constructor that
//! throws (a removal name that is no upgrade, when the module starts active) are never sent to the C++: the Rust records an error and the case is
//! counted as excluded. `~AutoHealBehavior` (the particle system's destruction) is not sent (no hook for a module's destruction).
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9AH_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 691 data
//! blocks threw (read compared: 8449 comparisons, 0 mismatches), 551 cases excluded (undefined upstream, or a constructor that throws), 149574 ops, 37394
//! updates, 736 upgrades fired, 16492 xfers, 11503 effect lines (5885 heals, 143 icons; 1605 cases with a heal); ops (data, constructor, effects, xfers,
//! state): 211638 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0; name keys after the ops: 7758 comparisons, 0 mismatches. The first runs found
//! four defects of the test, none of the module: the Rust objects started off the map (a world with no map; the flags are now set after the positions),
//! the C++ driver dumped the health of an object without a body, the constructor's position was `-0.0` on one side, and the generator reached the
//! icon in four cases (a hurt crowd, the icon on and a range list now open four cases in ten). One defect of the module, found by the comparison:
//! `stopHealing()` also calls `setWakeFrame(FOREVER)`, which the first translation had left out.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 941 | `onDamage` wakes a self-healer whatever its soonest heal frame |
//! | 942 | an area healer heals with `attemptHealing`, not from the sole benefactor |
//! | 943 | `xfer` leaves out the stopped flag |
//! | 944 | the healing amount is one too many |
//! | 945 | the whole-player scan does not compare the controlling players |
//! | 946 | `SkipSelfForHealing` ignored in the radius scan |
//! | 947 | a single burst sleeps for the healing delay, not for ever |
//! | 948 | a self-healer at full health sleeps for the delay, not for ever |
//! | 949 | `ForbiddenKindOf` ignored in the whole-player scan |
//! | 950 | off-map objects are healed in the whole-player scan |
//! | 951 | the soonest heal frame ignores the healing delay |
//! | 952 | the constructor's random phase starts at 0 |
//! | 953 | `stopHealing` does not put the module to sleep |
//! | 954 | `undoUpgrade` leaves the soonest heal frame |
//!
//! All 14 are caught (9000 cases each), all as mismatches.

mod mux_common;
mod support;
mod up_common;

use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::geometry::{GeometryInfo, GeometryType};
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::kind_of::{
    KindOfMaskType, KINDOF_AIRCRAFT, KINDOF_CAN_ATTACK, KINDOF_COUNT, KINDOF_HUGE_VEHICLE,
    KINDOF_INFANTRY, KINDOF_SCORE, KINDOF_SELECTABLE, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use rts_lockstep_engine::common::module::{
    ModuleDataKind, MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE,
};
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::damage::DamageInfo;
use rts_lockstep_engine::logic::modules::auto_heal_behavior::AutoHealBehaviorModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::{PRIVATE_EFFECTIVELY_DEAD, PRIVATE_OFF_MAP};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
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
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9ah_m.bin");
const XFER_FILE: &str = "l9ah_m.bin";
/// The team of object `k` is `100 + k`.
const TEAM0: u32 = 100;
/// The eight kinds of the driver, in bit order.
const KINDS: [i32; 8] = [
    KINDOF_STRUCTURE,
    KINDOF_INFANTRY,
    KINDOF_VEHICLE,
    KINDOF_AIRCRAFT,
    KINDOF_SELECTABLE,
    KINDOF_CAN_ATTACK,
    KINDOF_SCORE,
    KINDOF_HUGE_VEHICLE,
];
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
const CFG_WORDS: usize = 28;

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::AutoHealBehavior(Arc::new(
        AutoHealBehaviorModuleData::friend_new_module_data(Some(ini))?,
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

/// A kind-of mask as the driver prints it in the data dump: the indices of its set bits, each followed by a comma.
fn kind_str(m: &KindOfMaskType) -> String {
    (0..KINDOF_COUNT)
        .filter(|&i| m.test(i))
        .map(|i| format!("{i},"))
        .collect()
}

fn name_of(n: &Option<Vec<u8>>) -> Vec<u8> {
    n.clone().unwrap_or_default()
}

/// The data dump of `ref_l9ah_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::AutoHealBehavior(d) = data else {
        unreachable!("not an AutoHealBehavior")
    };
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_u32(v, u32::from(d.initially_active));
    put_u32(v, u32::from(d.single_burst));
    put_i32(v, d.healing_amount);
    put_u32(v, d.healing_delay);
    put_u32(v, d.start_healing_delay);
    put_u32(v, d.radius.to_bits());
    put_u32(v, u32::from(d.affects_whole_player));
    put_u32(v, u32::from(d.skip_self_for_healing));
    put_str(v, kind_str(&d.kind_of).as_bytes());
    put_str(v, kind_str(&d.forbidden_kind_of).as_bytes());
    put_str(v, &name_of(&d.radius_particle_system));
    put_str(v, &name_of(&d.unit_heal_pulse_particle_system));
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    r.mux(&mut out);
    let mut f = Vec::new();
    for _ in 0..2 {
        f.push(r.u32().to_string());
    }
    f.push((r.u32() as i32).to_string());
    for _ in 0..5 {
        f.push(r.u32().to_string());
    }
    let (kinds, forbidden, ps1, ps2) = (r.str(), r.str(), r.str(), r.str());
    out.push(format!(
        "data {f:?} kinds [{kinds}] forbidden [{forbidden}] radiusps {ps1:?} pulseps {ps2:?}"
    ));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut res, mut up) = (0, 0, 0);
        if i > 0 {
            st = r.u32() as i32;
            res = r.u32();
            up = r.u32();
        }
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let (wakes, last) = (r.u32(), r.u32());
        let mut tail = String::new();
        if i > 0 {
            let x = r.xfer();
            let state: Vec<u32> = (0..3).map(|_| r.u32()).collect();
            let objs: Vec<u32> = (0..12).map(|_| r.u32()).collect();
            tail = format!(" {x} state {state:?} objects {objs:?}");
        }
        out.push(format!("op {i}: status {st} result {res} upgraded {up} log {log:?} wakes {wakes} last {last}{tail}"));
    }
    out
}

// -- generators --

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
    match g.below(14) {
        0..=1 => "0".to_string(),
        2..=7 => pick(g, &["100", "300", "500", "1000", "3000", "33", "34", "67"]).to_string(),
        8 => format!("{}", g.below(100_000)),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        10 if !clean => gen_int(g, false),
        _ => "1000".to_string(),
    }
}

fn gen_ps(g: &mut Gen) -> String {
    pick(g, &["PS_A", "PS_B", "PS_A", "None", "none", "ps_a"]).to_string()
}

fn gen_data(g: &mut Gen, upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_bool(g, clean);
    field(g, 55, "StartsActive", v);
    let v = gen_bool(g, clean);
    field(g, 30, "SingleBurst", v);
    let v = if clean {
        format!("{}", g.below(40))
    } else {
        gen_int(g, false)
    };
    field(g, 75, "HealingAmount", v);
    let v = gen_dur(g, clean);
    field(g, 75, "HealingDelay", v);
    let v = match g.below(4) {
        0 | 1 => "0".to_string(),
        _ => pick(g, &["50", "100", "250.5", "1000"]).to_string(),
    };
    field(g, 55, "Radius", v);
    let v = gen_kind_list(g, clean);
    field(g, 40, "KindOf", v);
    let v = gen_kind_list(g, clean);
    field(g, 25, "ForbiddenKindOf", v);
    let v = gen_ps(g);
    field(g, 30, "RadiusParticleSystemName", v);
    let v = gen_ps(g);
    field(g, 35, "UnitHealPulseParticleSystemName", v);
    let v = gen_dur(g, clean);
    field(g, 30, "StartHealingDelay", v);
    let v = gen_bool(g, clean);
    field(g, 30, "AffectsWholePlayer", v);
    let v = gen_bool(g, clean);
    field(g, 30, "SkipSelfForHealing", v);
    let mut mux_lines = Vec::new();
    gen_mux_lines(g, upgrades, clean, &mut mux_lines);
    lines.extend(mux_lines);
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

// -- the Rust side --

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    fired: u64,
    heals: u64,
    scans: u64,
    anims: u64,
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
            match cut_iter.next() {
                Some(CutCall::ParticleSystemCreate {
                    template,
                    position,
                    id,
                }) => {
                    log.push(format!(
                        "psys {} {id} {:08x},{:08x},{:08x}",
                        String::from_utf8_lossy(&template),
                        position.x.to_bits(),
                        position.y.to_bits(),
                        position.z.to_bits()
                    ));
                }
                Some(CutCall::InGameUiAddWorldAnimation {
                    template,
                    position,
                    display_time_in_seconds,
                    z_rise_per_second,
                }) => {
                    log.push(format!("findanim {}", String::from_utf8_lossy(&template)));
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
        } else if t.starts_with("evt FxListDoFxObj") {
            log.push(format!("fx{}", fx.id(&fx_iter.next())));
        } else if let Some(n) = t.strip_prefix("remove ") {
            log.push(format!("remove {n}"));
        } else if t.starts_with("heal ") {
            log.push(t);
        }
    }
    (log, wakes, last)
}

fn new_events(w: &World, before: usize) -> Vec<Vec<u8>> {
    w.events.events()[before..]
        .iter()
        .filter_map(|e| match e {
            rts_lockstep_engine::logic::client_event::ClientEvent::Object(rts_lockstep_engine::logic::world::object_events::ObjectClientEvent::FxListDoFxObj { fx, .. }) => Some(fx.clone()),
            _ => None,
        })
        .collect()
}

fn set_player(w: &mut World, k: usize, player_plus_one: u32) {
    let team = TEAM0 + k as u32;
    if player_plus_one == 0 {
        w.cuts.team_players.remove(&team);
    } else {
        w.cuts.team_players.insert(team, player_plus_one as i32 - 1);
    }
}

fn probe_of(w: &mut World, id: i32) -> Option<&mut ProbeModule> {
    w.objects
        .find_mut(id)?
        .modules
        .iter_mut()
        .find_map(|b| match b.body.as_mut() {
            Some(UpdateBody::ModuleProbe(p)) => Some(p),
            _ => None,
        })
}

/// The height the driver's geometry answers for `getMaxHeightAbovePosition`: a box of that height.
fn set_height(g: &mut GeometryInfo, height: f32) {
    g.set(GeometryType::Box, false, height, 1.0, 1.0);
}

fn ids(packed: u32) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for k in 0..4 {
        let id = packed >> (4 * k) & 15;
        if id == 0 {
            break;
        }
        if id <= 4 {
            v.push(id as i32);
        }
    }
    v
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
    "AutoHealBehavior",
];

/// Runs the ops on a fresh world and returns the dump (after the data dump), the name keys afterwards, the world and whether the constructor threw.
fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32; CFG_WORDS],
    ops: &[[u32; 5]],
    tally: &mut Tally,
) -> (Vec<u8>, Vec<u32>, World, bool) {
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
    w.players.set_player_count(3);
    w.random.init_game_logic_random(cfg[25]);
    w.set_frame(cfg[24]);
    w.cuts.globals.anim2d_collection_present = true;
    w.cuts.globals.draw_icon_ui = true;
    w.cuts.globals.get_healed_animation_display_time_in_seconds = 0.0;
    w.cuts.globals.get_healed_animation_z_rise_per_second = 0.0;
    w.cuts.get_healed_animation_name.clear();

    let mut me = 0;
    let mut healer_idx = 0;
    let mut constructor: (Vec<String>, u32, u32) = (Vec::new(), 0, 0);
    let mut ctor_threw = false;
    for k in 0..4usize {
        let team = TEAM0 + k as u32;
        w.teams.insert_team(Team::new(team, 0)).unwrap();
        set_player(&mut w, k, cfg[k]);
        let mut modules = Vec::new();
        if cfg[16 + k] & 1 != 0 {
            let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
            m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
            m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(
                ProbeAnswers::default(),
                f32::from_bits(cfg[12 + k]),
                Default::default(),
            )));
            modules.push(m);
        }
        let mut status = ObjectStatusMaskType::default();
        if k == 0 {
            // made under construction (as batches C and I do): `initObject` runs the upgrade pass of a finished object, which resolves the mux's names;
            // the C++ driver makes the object with no such pass
            status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
            modules.push(
                rts_lockstep_engine::logic::modules::new_module(
                    w.next_object_id(),
                    b"AutoHealBehavior",
                    b"ModuleTag_Heal",
                    &data,
                )
                .unwrap()
                .unwrap(),
            );
        }
        let mut values = ObjectTemplateValues::default();
        for (i, kind) in KINDS.iter().enumerate() {
            if cfg[4 + k] >> i & 1 != 0 {
                values.kind_of.push(*kind);
            }
        }
        let template = Arc::new(ThingTemplate::from_object_values(&values));
        let events_before = w.events.events().len();
        let id = w
            .new_object(NewObject {
                template,
                status,
                team: Some(team),
                modules,
            })
            .unwrap();
        assert_eq!(
            id as usize,
            k + 1,
            "the objects of a fresh world have the ids 1, 2, 3, 4"
        );
        if k == 0 {
            me = id;
            w.objects
                .find_mut(me)
                .unwrap()
                .status
                .set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
            healer_idx = w.find_module(me, b"AutoHealBehavior").unwrap();
            ctor_threw = !w.cuts.module_errors.is_empty();
            let evs = new_events(&w, events_before);
            constructor = effects(
                w.cuts.trace.take().unwrap(),
                w.cuts.log.take().unwrap(),
                evs,
                healer_idx,
                &mut fx,
            );
        }
        if let Some(p) = probe_of(&mut w, id) {
            p.health = f32::from_bits(cfg[8 + k]);
        }
        let o = w.objects.find_mut(id).unwrap();
        set_height(&mut o.geometry_info, f32::from_bits(cfg[20 + k]));
        let _ = o;
    }
    for k in 0..4usize {
        w.set_position(
            k as i32 + 1,
            &Coord3D {
                x: 10.0 * k as f32,
                y: -3.0 * k as f32,
                z: 0.5 * k as f32,
            },
        );
    }
    // the flags are the test's, set after the positions (a position change sets the off-map flag from the map extent)
    for k in 0..4usize {
        let o = w.objects.find_mut(k as i32 + 1).unwrap();
        o.private_status &= !(PRIVATE_EFFECTIVELY_DEAD | PRIVATE_OFF_MAP);
        if cfg[16 + k] & 2 != 0 {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        }
        if cfg[16 + k] & 4 != 0 {
            o.private_status |= PRIVATE_OFF_MAP;
        }
    }
    let handle = ModuleHandle {
        object: me,
        index: healer_idx,
    };
    let u = w.update_handle_of_module(handle).expect("an update module");
    tally.effects += constructor.0.len() as u64;
    put_u32(&mut v, constructor.0.len() as u32);
    for l in &constructor.0 {
        put_str(&mut v, l.as_bytes());
    }
    put_u32(&mut v, constructor.1);
    put_u32(&mut v, if constructor.1 > 0 { constructor.2 } else { 0 });
    let idx = healer_idx;
    let mut saved: Vec<u8> = Vec::new();
    for o in ops {
        let mask = mask_from(&o[1..5]);
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
                w.module_call(me, idx, ModuleCall::OnDamage(DamageInfo::default()));
            }
            2 => {
                w.module_call(me, idx, ModuleCall::StopHealing);
            }
            3 => {
                w.module_call(me, idx, ModuleCall::UndoUpgrade);
            }
            4 => {
                let k = (o[1] & 3) as usize;
                let id = k as i32 + 1;
                let value = o[3];
                match o[2] {
                    0 => set_player(&mut w, k, value),
                    1 | 2 => {
                        let bit = if o[2] == 1 {
                            PRIVATE_EFFECTIVELY_DEAD
                        } else {
                            PRIVATE_OFF_MAP
                        };
                        let ob = w.objects.find_mut(id).unwrap();
                        if value != 0 {
                            ob.private_status |= bit;
                        } else {
                            ob.private_status &= !bit;
                        }
                    }
                    3 => {
                        if let Some(p) = probe_of(&mut w, id) {
                            p.health = f32::from_bits(value);
                        }
                    }
                    4 => {
                        if let Some(p) = probe_of(&mut w, id) {
                            p.max_health = f32::from_bits(value);
                        }
                    }
                    5 => {
                        let p = w.objects.find(id).unwrap().get_position();
                        let flags = w.objects.find(id).unwrap().private_status;
                        w.set_position(
                            id,
                            &Coord3D {
                                x: p.x,
                                y: p.y,
                                z: f32::from_bits(value),
                            },
                        );
                        w.objects.find_mut(id).unwrap().private_status = flags;
                    }
                    6 => set_height(
                        &mut w.objects.find_mut(id).unwrap().geometry_info,
                        f32::from_bits(value),
                    ),
                    _ => unreachable!(),
                }
            }
            5 => w.cuts.range_scan = ids(o[1]),
            6 => {
                w.cuts
                    .player_object_list
                    .insert((o[1] % 3) as i32, ids(o[2]));
            }
            7 => {
                w.cuts.globals.anim2d_collection_present = o[1] & 1 != 0;
                w.cuts.globals.draw_icon_ui = o[1] & 2 != 0;
                w.cuts.get_healed_animation_name = if o[1] & 4 != 0 {
                    b"ANIM_HEAL".to_vec()
                } else {
                    Vec::new()
                };
                w.cuts.globals.get_healed_animation_display_time_in_seconds = f32::from_bits(o[2]);
                w.cuts.globals.get_healed_animation_z_rise_per_second = f32::from_bits(o[3]);
            }
            8 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::AttemptUpgrade(mask))
                        .as_bool(),
                );
                tally.fired += u64::from(res);
            }
            9 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::WouldUpgrade(mask))
                        .as_bool(),
                )
            }
            10 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::ResetUpgrade(mask))
                        .as_bool(),
                )
            }
            11 => {
                res = u32::from(
                    w.module_call(me, idx, ModuleCall::IsAlreadyUpgraded)
                        .as_bool(),
                )
            }
            12 => {
                w.module_call(me, idx, ModuleCall::ForceRefreshUpgrade);
            }
            13 => w.set_frame(o[1]),
            14..=16 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 14, &saved, XFER_FILE);
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
        let fxs = new_events(&w, events_before);
        let (log, wakes, last) = effects(
            w.cuts.trace.take().unwrap(),
            w.cuts.log.take().unwrap(),
            fxs,
            idx,
            &mut fx,
        );
        tally.effects += log.len() as u64;
        tally.heals += log.iter().filter(|l| l.starts_with("heal ")).count() as u64;
        tally.anims += log.iter().filter(|l| l.starts_with("anim ")).count() as u64;
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
                    Some(UpdateBody::AutoHealBehavior(m)) => Some(m.state_words()),
                    _ => None,
                });
        let state = state.expect("the module is back in its slot");
        for s in &state[..3] {
            put_u32(&mut v, *s);
        }
        for k in 0..4i32 {
            let health = probe_of(&mut w, k + 1).map_or(0.0, |p| p.health);
            put_u32(&mut v, health.to_bits());
            let ob = w.objects.find(k + 1).unwrap();
            put_u32(&mut v, ob.sole_healing_benefactor_id as u32);
            put_u32(&mut v, ob.sole_healing_benefactor_expiration_frame);
        }
    }
    let keys = PROBE
        .iter()
        .map(|n| w.names.name_to_key(n.as_bytes()).0)
        .collect();
    (v, keys, w, ctor_threw)
}

fn cases() -> usize {
    std::env::var("L9AH_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

fn gen_health_bits(g: &mut Gen, max: f32) -> u32 {
    match g.below(6) {
        0 => 0.0f32,
        1 => max,
        2 => max * 0.5,
        3 => g.uniform(0.0, max),
        4 => max - 1.0,
        _ => 10.0,
    }
    .to_bits()
}

#[test]
fn auto_heal_behavior_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0A17_0EA1);
    let table = blocks();
    let mut read = Check::new("update module M2 data read (throw, message)");
    let mut run = Check::new("update module M2 ops (data, constructor, effects, xfers, state)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut excluded) = (0u64, 0u64, 0u64);
    let mut tally = Tally::default();
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
        let name = format!("Data\\INI\\Ah{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let mut cfg = [0u32; CFG_WORDS];
        for k in 0..4 {
            // the players: mostly the first two, so that the objects are often on one side
            cfg[k] = if g.below(10) == 0 {
                0
            } else {
                1 + g.below(2) + if g.below(6) == 0 { 1 } else { 0 }
            };
            cfg[4 + k] = if g.below(3) == 0 { 0xFF } else { g.below(256) };
            let max = pick(&mut g, &[&100.0f32, &200.0, &50.0, &1000.0]).to_owned();
            cfg[12 + k] = max.to_bits();
            cfg[8 + k] = gen_health_bits(&mut g, max);
            cfg[16 + k] = (if g.below(10) < 9 { 1 } else { 0 })
                | (if g.below(12) == 0 { 2 } else { 0 })
                | (if g.below(12) == 0 { 4 } else { 0 });
            cfg[20 + k] = g.uniform(0.0, 20.0).to_bits();
        }
        cfg[24] = g.below(300);
        cfg[25] = g.u32();
        let n_ops = 1 + g.below(30) as usize;
        // Four cases in ten start with a hurt crowd, the icon on and a range list, so that the scans and the icon are reached often.
        let mut ops: Vec<[u32; 5]> = Vec::new();
        if g.below(100) < 40 {
            ops.push([
                7,
                7,
                pick(&mut g, &[&1.5f32, &3.0]).to_owned().to_bits(),
                pick(&mut g, &[&0.0f32, &10.0]).to_owned().to_bits(),
                0,
            ]);
            for k in 0..4 {
                if g.below(4) != 0 {
                    ops.push([4, k, 3, 10.0f32.to_bits(), 0]);
                }
            }
            let mut packed = 0u32;
            for j in 0..1 + g.below(4) {
                packed |= (1 + g.below(4)) << (4 * j);
            }
            ops.push([5, packed, 0, 0, 0]);
            let mut list = 0u32;
            for j in 0..1 + g.below(4) {
                list |= (1 + g.below(4)) << (4 * j);
            }
            ops.push([6, 0, list, 0, 0]);
            ops.push([6, 1, list, 0, 0]);
            ops.push([6, 2, list, 0, 0]);
            let m = gen_mask(&mut g, n_up, 1);
            ops.push([8, m[0], m[1], m[2], m[3]]);
            ops.push([0, 0, 0, 0, 0]);
        }
        ops.extend((0..n_ops).map(|_| match g.below(100) {
            0..=19 => [0, 0, 0, 0, 0],
            20..=26 => [1, 0, 0, 0, 0],
            27 => [2, 0, 0, 0, 0],
            28..=29 => [3, 0, 0, 0, 0],
            30..=41 => {
                let k = g.below(4);
                match g.below(8) {
                    0 => [4, k, 0, g.below(4), 0],
                    1 => [4, k, 1, g.below(2), 0],
                    2 => [4, k, 2, g.below(2), 0],
                    3 | 4 => [4, k, 3, gen_health_bits(&mut g, 100.0), 0],
                    5 => [
                        4,
                        k,
                        4,
                        pick(&mut g, &[&100.0f32, &200.0, &50.0])
                            .to_owned()
                            .to_bits(),
                        0,
                    ],
                    6 => [4, k, 6, g.uniform(0.0, 20.0).to_bits(), 0],
                    _ => [4, k, 5, g.uniform(-2.0, 6.0).to_bits(), 0],
                }
            }
            42..=47 => {
                let n = g.below(5);
                let mut packed = 0u32;
                for j in 0..n {
                    packed |= (1 + g.below(4)) << (4 * j);
                }
                [5, packed, 0, 0, 0]
            }
            48..=52 => {
                let n = g.below(5);
                let mut packed = 0u32;
                for j in 0..n {
                    packed |= (1 + g.below(4)) << (4 * j);
                }
                [6, g.below(3), packed, 0, 0]
            }
            53..=56 => [
                7,
                g.below(8),
                pick(&mut g, &[&0.0f32, &1.5, &3.0]).to_owned().to_bits(),
                pick(&mut g, &[&0.0f32, &10.0, &2.5]).to_owned().to_bits(),
                0,
            ],
            57..=62 => {
                let m = gen_mask(&mut g, n_up, 1);
                [8, m[0], m[1], m[2], m[3]]
            }
            63..=65 => {
                let m = gen_mask(&mut g, n_up, 1);
                [9, m[0], m[1], m[2], m[3]]
            }
            66..=67 => {
                let m = gen_mask(&mut g, n_up, 1);
                [10, m[0], m[1], m[2], m[3]]
            }
            68 => [11, 0, 0, 0, 0],
            69 => [12, 0, 0, 0, 0],
            70..=79 => [13, gen_frame(&mut g), 0, 0, 0],
            80..=84 => [14, 0, 0, 0, 0],
            85..=89 => [15, 0, 0, 0, 0],
            90..=92 => [16, 0, 0, 0, 0],
            _ => [0, 0, 0, 0, 0],
        }));

        // Rust first: a case that reaches upstream undefined behaviour (or a constructor that throws) is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w, ctor_threw) =
                rust_run(ctx, data.expect("read ok"), &cfg, &ops, &mut tally);
            if ctor_threw || w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
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
            cpp::ref_l9ah_run(
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
            run.eq(&"C++ crashed", &"no crash", || {
                format!(
                    "case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}",
                    show(&text)
                )
            });
            continue;
        }
        buf.truncate(len as usize);
        let detail = match status {
            0 | 4 => Vec::new(),
            1 => msg[..code as usize].to_vec(),
            2 | 3 => code.to_le_bytes().to_vec(),
            _ => panic!("C++ threw something unexpected (status {status}) -- a constructor that threw upstream but not here"),
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
        tally.scans += u64::from(dr.iter().any(|l| l.contains("heal ")));
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
        "update module M2: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream or a constructor that throws), {ops_run} ops, \
         {} updates, {} upgrades fired, {} xfers, {} effect lines, {} heals, {} icons, {} cases with a heal",
        tally.updates, tally.fired, tally.xfers, tally.effects, tally.heals, tally.anims, tally.scans
    );
    read.finish();
    run.finish();
    keys.finish();
    let _ = BTreeMap::<u8, u8>::new();
    assert!(
        read_failed * 40 > cases as u64
            && ops_run * 5 > cases as u64 * 4
            && tally.fired * 50 > cases as u64
            && tally.heals * 5 > cases as u64
            && tally.anims * 200 > cases as u64,
        "the generator must reach every path"
    );
}
