#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireOCLAfterWeaponCooldownUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `FireOCLAfterWeaponCooldownUpdate` (`logic::modules::fire_ocl_after_weapon_cooldown_update`, with `logic::modules::upgrade_mux_user`) against the
//! unchanged C++, `p1w12-sonnet`, update batch M.
//!
//! # Method
//!
//! The C++ side is `FireOCLAfterWeaponCooldownUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_fire_ocl_m.inc`) on the
//! module build's stand-ins: three players (a completed-upgrade mask), an `Object` with a controlling player, a current weapon and a weapon per slot
//! (a `Weapon` with a slot, a last shot frame and a possible next shot frame), a game logic whose frame the test sets, the object creation lists
//! `OCL_A` and `OCL_B` of batch B (logging `ocl`, `oclsec` and, new, `ocllife`), and the REAL `UpdateModule`, `UpgradeMux`, `UpgradeCenter`, `INI` and
//! `Xfer`.
//!
//! The Rust side is the real thing: a `World` with the loaded upgrade center, three players, and one object with the module from `modules::new_module`;
//! `update()` is run as the scheduler runs it (`World::with_body`), the upgrade calls are `World::module_call`, and the xfers go through
//! `World::xfer_module_slot`. The object's weapons are the tables `cuts.cur_weapon` and `cuts.weapons` (the full-game adapter fills the weapon
//! set; `World::current_weapon_facts` reads the set first).
//!
//! Each case defines 1 to 8 upgrades, reads a `D` block (the five fields and the mux's five; mostly well formed), and when the read succeeds runs 1 to
//! 24 ops (an update, a frame, a tick that may fire the weapon, a current weapon, the weapons in the slots, the object's and the players' completed
//! upgrades, the controlling player, the upgrade calls, three xfers).
//!
//! Compared, bit for bit: the read's throw; the data (the mux, the list, the slot, the shot minimum, the lifetime rate and cap); per op the throw
//! status, the result (the sleep time or the upgrade call's answer), `isAlreadyUpgraded`, the effects in order (`ocl`, `oclsec`, `ocllife`, `fx`,
//! `remove`), the xfer status, bytes and CRC, and the module's three state words; and the name keys after the ops.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). Inputs for which upstream is
//! undefined (an object with no controlling player when `update()` runs; a lifetime outside `UnsignedInt`; an upgrade name that is not an upgrade at
//! the first use of the mux, with an empty or `None` name: a `NULL` template) are never sent to the C++: the Rust records an `is_undefined()` error
//! and the case is counted as excluded.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9FO_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 885 data
//! blocks threw (read compared: 7976 comparisons, 0 mismatches), 1024 cases excluded (undefined upstream: no controlling player when `update()` runs, a
//! lifetime outside `UnsignedInt`, a `NULL` upgrade template), 159865 ops, 58302 updates, 218 upgrades fired, 8908 xfers, 2818 effect lines (1075 object
//! creation lists); ops (data, effects, xfers, state): 209502 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0; name keys after the ops: 7091
//! comparisons, 0 mismatches. The first run (before the generator started six cases in ten with a run of shots in the module's slot) reached the list in
//! 125 cases and the test's own reach assertion failed: a generator defect, fixed by strengthening the generator, not the check.
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 921 | the lifetime rate scaled by 0.01 instead of 0.001 |
//! | 922 | the lifetime not capped at `OCLLifetimeMaxCap` |
//! | 923 | the list created without the minimum number of shots |
//! | 924 | a change of validity does not reset the shot count |
//! | 925 | `xfer` leaves out the start frame |
//! | 926 | the weapon slot not compared (any current weapon is valid) |
//! | 927 | the shot noticed when the last shot is this frame, not the one before |
//! | 928 | the first shot does not record the start frame |
//! | 929 | the object's own completed upgrades not added to the player's |
//! | 930 | a failed upgrade condition still allows the switch-away list |
//! | 931 | the switch-away list does not need a weapon in the slot |
//!
//! All 11 are caught (9000 cases each), all as mismatches.

mod mux_common;
mod support;
mod up_common;

use mux_common::*;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::fire_ocl_after_weapon_cooldown_update::FireOCLAfterWeaponCooldownUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::{CutCall, WeaponFacts};
use rts_lockstep_engine::logic::world::statics::GameStatics;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9fo_m.bin");
const XFER_FILE: &str = "l9fo_m.bin";
const NO_PLAYER: u32 = 0xFFFF_FFFF;
/// The team of the object.
const OBJECT_TEAM: u32 = 100;

// -- reading the data --

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(Arc::new(
        FireOCLAfterWeaponCooldownUpdateModuleData::friend_new_module_data(Some(ini))?,
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

/// The list as the driver numbers it: 0 none, 1 `OCL_A`, 2 `OCL_B` (any other name is `NULL` to the C++ `findObjectCreationList`).
fn ocl_code(name: &Option<Vec<u8>>) -> u32 {
    match name.as_deref() {
        Some(b"OCL_A") => 1,
        Some(b"OCL_B") => 2,
        _ => 0,
    }
}

/// The data dump of `ref_l9fo_run`: the mux, then the module's own fields.
fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind, fx: &mut FxIds) {
    let ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d) = data else {
        unreachable!("not a FireOCLAfterWeaponCooldownUpdate")
    };
    put_mux(v, &d.upgrade.mux_data(), fx);
    put_u32(v, ocl_code(&d.ocl));
    put_i32(v, d.weapon_slot);
    put_u32(v, d.min_shots_required);
    put_u32(v, d.ocl_lifetime_per_second);
    put_u32(v, d.ocl_max_frames);
}

/// A dump as text, for the comparison.
fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    r.mux(&mut out);
    let (ocl, slot, min, rate, cap) = (r.u32(), r.u32() as i32, r.u32(), r.u32(), r.u32());
    out.push(format!(
        "data ocl {ocl} slot {slot} min {min} rate {rate} cap {cap}"
    ));
    for i in 0..n_ops {
        if r.at_end() {
            break;
        }
        let st = r.u32() as i32;
        let res = r.u32();
        let up = r.u32();
        let nlog = r.u32();
        let log: Vec<String> = (0..nlog).map(|_| r.str()).collect();
        let x = r.xfer();
        let state: Vec<u32> = (0..3).map(|_| r.u32()).collect();
        out.push(format!(
            "op {i}: status {st} result {res} upgraded {up} log {log:?} {x} state {state:?}"
        ));
    }
    out
}

// -- generators --

fn gen_uint(g: &mut Gen, clean: bool, small: bool) -> String {
    match g.below(12) {
        0..=2 => "0".to_string(),
        3..=8 => pick(g, &["1", "2", "3", "4", "5", "10"]).to_string(),
        9 if !clean => pick(g, &["x", "", "-1", "12abc", "4294967296"]).to_string(),
        10 if !small => {
            pick(g, &["100", "500", "1000", "2000", "100000", "4294967295"]).to_string()
        }
        _ => format!("{}", g.below(20)),
    }
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=7 => pick(g, &["100", "300", "500", "1000", "3000", "33", "34"]).to_string(),
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
    let v = if clean {
        pick(g, &["PRIMARY", "SECONDARY", "TERTIARY"]).to_string()
    } else {
        pick(
            g,
            &[
                "PRIMARY",
                "SECONDARY",
                "TERTIARY",
                "primary",
                "Secondary",
                "QUATERNARY",
                "1",
                "",
            ],
        )
        .to_string()
    };
    field(g, 55, "WeaponSlot", v);
    let v = pick(
        g,
        &["OCL_A", "OCL_B", "OCL_A", "None", "none", "ocl_a", "OCL_Z"],
    )
    .to_string();
    field(g, 75, "OCL", v);
    let v = gen_uint(g, clean, true);
    field(g, 60, "MinShotsToCreateOCL", v);
    let v = gen_uint(g, clean, false);
    field(g, 60, "OCLLifetimePerSecond", v);
    let v = gen_dur(g, clean);
    field(g, 60, "OCLLifetimeMaxCap", v);
    let mut mux_lines = Vec::new();
    gen_mux_lines(g, upgrades, clean, &mut mux_lines);
    // a quarter as many TriggeredBy lines: the module should reach its OCL often
    lines.extend(
        mux_lines
            .into_iter()
            .filter(|l| !l.starts_with("TriggeredBy") || g.below(100) < 35),
    );
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
    ocls: u64,
}

/// The effect lines of a trace, in the driver's format.
fn effects(
    trace: Vec<String>,
    cuts: Vec<CutCall>,
    fxs: Vec<Vec<u8>>,
    fx: &mut FxIds,
) -> Vec<String> {
    let mut log = Vec::new();
    let mut cut_iter = cuts.into_iter();
    let mut fx_iter = fxs.into_iter();
    for t in trace {
        if t.starts_with("cut ") {
            if let Some(CutCall::OclCreateWithLifetime {
                secondary,
                ocl,
                lifetime_frames,
                ..
            }) = cut_iter.next()
            {
                let i = match ocl.as_slice() {
                    b"OCL_A" => 0,
                    b"OCL_B" => 1,
                    _ => continue, // a name that is no list makes nothing
                };
                log.push(format!("ocl {i}"));
                if secondary != 0 {
                    log.push(format!("oclsec {secondary}"));
                }
                if lifetime_frames != 0 {
                    log.push(format!("ocllife {lifetime_frames}"));
                }
            }
        } else if t.starts_with("evt FxListDoFxObj") {
            log.push(format!("fx{}", fx.id(&fx_iter.next())));
        } else if let Some(n) = t.strip_prefix("remove ") {
            log.push(format!("remove {n}"));
        }
    }
    log
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

const PROBE: [&str; 9] = [
    "none",
    "Up0",
    "Up3",
    "Unknown0",
    "Unknown2",
    "Up9",
    "SCIENCE_Pathfinder",
    "zz_fresh",
    "FireOCLAfterWeaponCooldownUpdate",
];

/// Runs the ops on a fresh world and returns the dump (after the data dump), the name keys afterwards and the world.
fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32; 2],
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
    set_controller(&mut w, (cfg[0] != NO_PLAYER).then_some(cfg[0]));
    w.set_frame(cfg[1]);

    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"FireOCLAfterWeaponCooldownUpdate",
        b"ModuleTag_Ocl",
        &data,
    )
    .unwrap()
    .unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    // made under construction, as batch C does: `initObject` runs the upgrade pass of a finished object, which resolves the mux's names; the C++ driver
    // makes the object with no such pass
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
    let idx = w
        .find_module(me, b"FireOCLAfterWeaponCooldownUpdate")
        .unwrap();
    let handle = ModuleHandle {
        object: me,
        index: idx,
    };
    let u = w.update_handle_of_module(handle).expect("an update module");
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
            1 => w.set_frame(o[1]),
            2 => {
                let f = w.frame().wrapping_add(1);
                w.set_frame(f);
                if o[1] & 1 != 0 {
                    if let Some(c) = w.cuts.cur_weapon.get_mut(&me) {
                        c.last_shot_frame = f.wrapping_sub(1);
                        c.possible_next_shot_frame = f.wrapping_add(2);
                    }
                }
            }
            3 => {
                if o[1] == 0 {
                    w.cuts.cur_weapon.remove(&me);
                } else {
                    w.cuts.cur_weapon.insert(
                        me,
                        WeaponFacts {
                            slot: o[1] as i32 - 1,
                            last_shot_frame: o[2],
                            possible_next_shot_frame: o[3],
                        },
                    );
                }
            }
            4 => {
                for k in 0..3 {
                    if o[1] >> k & 1 != 0 {
                        w.cuts.weapons.insert((me, k));
                    } else {
                        w.cuts.weapons.remove(&(me, k));
                    }
                }
            }
            5 => w.objects.find_mut(me).unwrap().object_upgrades_completed = mask,
            6 => {
                for p in 0..3 {
                    w.cuts.player_upgrades.insert(p, mask);
                }
            }
            7 => set_controller(&mut w, (o[1] != 0).then(|| o[1] - 1)),
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
            13..=15 => {
                let r = run_xfer(&mut w, me, idx as usize, o[0] - 13, &saved, XFER_FILE);
                xst = r.0;
                xbytes = r.1;
                crc = r.2;
                if o[0] == 13 {
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
        let log = effects(
            w.cuts.trace.take().unwrap(),
            w.cuts.log.take().unwrap(),
            fxs,
            &mut fx,
        );
        tally.effects += log.len() as u64;
        tally.ocls += log.iter().filter(|l| l.starts_with("ocl ")).count() as u64;
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::FireOCLAfterWeaponCooldownUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        let state = state.expect("the module is back in its slot");
        for s in &state[..3] {
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
    std::env::var("L9FO_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9_000)
}

#[test]
fn fire_ocl_after_weapon_cooldown_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases = cases();
    let mut g = Gen::new(0x0C1A_F10C);
    let table = blocks();
    let mut read = Check::new("update module M1 data read (throw, message)");
    let mut run = Check::new("update module M1 ops (data, effects, xfers, state)");
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
        let name = format!("Data\\INI\\Fo{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        // [the object's player, the frame]
        let mut cfg = [0u32; 2];
        cfg[0] = if g.below(12) == 0 {
            NO_PLAYER
        } else {
            g.below(3)
        };
        cfg[1] = g.below(300);
        // The generator follows the frame a little, so that the shots it makes are the last frame's more often than by chance.
        let n_ops = 1 + g.below(24) as usize;
        let mut frame = cfg[1];
        let mut cur_slot = 0u32;
        // Six cases in ten start with a run of shots in the module's own slot (the players holding every upgrade), so that the list is created often.
        let want_slot = match &data {
            Some(ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d)) => d.weapon_slot as u32,
            _ => 0,
        };
        let mut ops: Vec<[u32; 5]> = Vec::new();
        if g.below(100) < 60 {
            // every upgrade but the ones the module conflicts with
            let conflicts: Vec<String> = match &data {
                Some(ModuleDataKind::FireOCLAfterWeaponCooldownUpdate(d)) => d
                    .upgrade
                    .mux_data()
                    .conflicting_upgrade_names
                    .iter()
                    .map(|n| String::from_utf8_lossy(n).to_lowercase())
                    .collect(),
                _ => Vec::new(),
            };
            let mut all = [0u32; 4];
            for (i, u) in upgrades.iter().enumerate() {
                if !conflicts.contains(&u.to_lowercase()) {
                    all[i / 32] |= 1 << (i % 32);
                }
            }
            ops.push([6, all[0], all[1], all[2], all[3]]);
            ops.push([
                7,
                if g.below(10) == 0 { 0 } else { 1 + g.below(3) },
                0,
                0,
                0,
            ]);
            ops.push([
                3,
                want_slot + 1,
                frame.wrapping_sub(1),
                frame.wrapping_add(2),
                0,
            ]);
            for _ in 0..1 + g.below(6) {
                ops.push([2, 1, 0, 0, 0]);
                ops.push([0, 0, 0, 0, 0]);
            }
            // the weapon is ready again three frames after its last shot
            for _ in 0..3 + g.below(2) {
                ops.push([2, 0, 0, 0, 0]);
                ops.push([0, 0, 0, 0, 0]);
            }
            frame = frame.wrapping_add(9);
        }
        ops.extend((0..n_ops).map(|_| match g.below(80) {
            0..=13 => [0, 0, 0, 0, 0],
            14..=15 => {
                frame = gen_frame(&mut g);
                [1, frame, 0, 0, 0]
            }
            16..=29 => {
                frame = frame.wrapping_add(1);
                [2, g.below(2), 0, 0, 0]
            }
            30..=37 => {
                cur_slot = if g.below(8) == 0 { 0 } else { 1 + g.below(3) };
                let last = match g.below(4) {
                    0 => g.below(400),
                    _ => frame.wrapping_sub(1),
                };
                let next = match g.below(4) {
                    0 => frame.wrapping_add(3),
                    1 => g.below(400),
                    _ => frame.wrapping_sub(1),
                };
                [3, cur_slot, last, next, 0]
            }
            38..=41 => [4, g.below(8), 0, 0, 0],
            42..=47 => {
                let m = gen_mask(&mut g, n_up, 0);
                [5, m[0], m[1], m[2], m[3]]
            }
            48..=54 => {
                let m = gen_mask(&mut g, n_up, 1);
                [6, m[0], m[1], m[2], m[3]]
            }
            55..=56 => [7, g.below(4), 0, 0, 0],
            57..=61 => {
                let m = gen_mask(&mut g, n_up, 1);
                [8, m[0], m[1], m[2], m[3]]
            }
            62 => {
                let m = gen_mask(&mut g, n_up, 1);
                [9, m[0], m[1], m[2], m[3]]
            }
            63..=64 => {
                let m = gen_mask(&mut g, n_up, 1);
                [10, m[0], m[1], m[2], m[3]]
            }
            65..=66 => [11, 0, 0, 0, 0],
            67 => [12, 0, 0, 0, 0],
            68..=70 => [13, 0, 0, 0, 0],
            71..=72 => [14, 0, 0, 0, 0],
            73..=74 => [15, 0, 0, 0, 0],
            _ => [0, 0, 0, 0, 0],
        }));

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
            cpp::ref_l9fo_run(
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
        "update module M1: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, {} updates, {} upgrades fired, \
         {} xfers, {} effect lines, {} object creation lists",
        tally.updates, tally.fired, tally.xfers, tally.effects, tally.ocls
    );
    read.finish();
    run.finish();
    keys.finish();
    let _ = BTreeMap::<u8, u8>::new();
    assert!(
        read_failed * 40 > cases as u64
            && ops_run * 5 > cases as u64 * 4
            && tally.fired * 50 > cases as u64
            && tally.ocls * 10 > cases as u64,
        "the generator must reach every path"
    );
}
