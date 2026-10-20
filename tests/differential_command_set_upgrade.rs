// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/CommandSetUpgrade.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `CommandSetUpgrade` (`logic::modules::command_set_upgrade`, the lead's
//! layer-9 example) against the unchanged C++.
//!
//! # Method
//!
//! The C++ side is `CommandSetUpgrade.cpp`, compiled unchanged into
//! `wrap_module.cpp`'s unit (`differential/l9_example.inc`), on the module
//! build's stand-ins (`differential/shim_module`: an `Object` with a
//! controlling `Player`, its own completed upgrades and the command-set
//! override; a `ControlBar` whose `markUIDirty` is logged) and the REAL
//! `UpgradeCenter` and `INI` (`wrap_upgrade.cpp`, `wrap_ini.cpp`).
//!
//! Each case defines 0 to 8 upgrades on both sides (`Upgrade UpN` blocks, the
//! same load), generates a `D` block of the module's eight fields (the five
//! `UpgradeMux` ones and `CommandSet`, `CommandSetAlt`, `TriggerAlt`; mostly
//! well-formed, sometimes with unknown names, `None` in any case, bad
//! separators, junk lines or no `End`), and reads it on both sides. When the
//! read succeeds it gives the module an object with (or without) a
//! controlling player, random completed upgrades on each, and runs 1 to 12
//! ops: `wouldUpgrade`, `attemptUpgrade`, `resetUpgrade`,
//! `forceRefreshUpgrade`, `isAlreadyUpgraded`, and changes of the object's
//! and the player's completed upgrades between them.
//!
//! The Rust side is the real thing, not a twin: a `World` whose
//! `statics.upgrade_center` and `names` are the loaded store and generator,
//! one object (team 1, whose player is 0 when there is one) made by
//! `World::new_object` with the module from `modules::new_module`, and every
//! op sent through `World::module_call`. The object is made under
//! construction (so `initObject`'s `updateUpgradeModules` does nothing, as the
//! C++ makes the module directly) and the status cleared after.
//!
//! Compared, bit for bit: the read's throw (status and message); the data (the
//! mux, its masks, the three strings); per op the throw status, the result,
//! `isAlreadyUpgraded`, the effects in the order they happened (FX list,
//! each removal by name, `markUIDirty`), and the command-set override; after
//! the ops, the key `nameToKey` gives to a probe list of names (lookups assign
//! keys, so the order of lookups is compared too).
//!
//! An upstream throw from inside an op (`INI_INVALID_DATA` for a mux name that
//! is not an upgrade) is status 2 on the C++ side and a recorded
//! `world.cuts.module_errors` entry on the Rust side; both are compared as
//! status 2.
//!
//! # Results
//!
//! Measured 2026-10-20: see the clean-run paragraph after the planted-bug
//! table.
//!
//! # Planted bugs
//!
//! Each was applied to the Rust translation alone (one edit at a time,
//! reverted after) and the test run at 4,000 cases; `mismatches` is the
//! count of differing comparisons (the clean run has 0).
//!
//! | # | planted bug (in `src/logic/modules/`) | mismatches |
//! |---|---|---|
//! | 1 | the player check `test_for_all` (and more than one bit) instead of `test_for_any` | 43 |
//! | 2 | the object branch gives `CommandSet`, not `CommandSetAlt` | 63 |
//! | 3 | no `markUIDirty` | 660 |
//! | 4 | `TriggerAlt` defaults to `"None"`, not `"none"` (a different name key) | 1,152 |
//! | 5 | the player's mask is never consulted | 43 |
//! | 6 | the FX list is not played | 144 |
//! | 7 | the removals run after `upgradeImplementation` | 195 |
//! | 8 | `forceRefreshUpgrade` ignores `isAlreadyUpgraded` | 917 |
//!
//! All 8 caught (2026-10-20). Plants 1 and 5 are the same mutation in effect:
//! an upgrade's mask has one bit, so "all of it and more than one bit" never
//! holds, and the counts agree.
//!
//! The clean run (2026-10-20, 12,000 cases): 11,885 reads compared, 11,557 op
//! sequences (75,241 ops: 2,163 upgrades fired, 312 of them giving the
//! alternative set, 7,529 upstream throws inside ops), 11,557 name-key
//! probes; **0 mismatches**; 115 cases excluded as undefined upstream (a mux
//! list naming `None`).

#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use rts_lockstep_engine::common::thing_template::ThingTemplate;
use rts_lockstep_engine::common::upgrade::UpgradeMaskType;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::client_event::ClientEvent;
use rts_lockstep_engine::logic::module::upgrade_module::UpgradeMuxData;
use rts_lockstep_engine::logic::modules::command_set_upgrade::CommandSetUpgradeModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_events::ObjectClientEvent;
use rts_lockstep_engine::logic::world::statics::GameStatics;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::sync::Arc;
use support::{Check, Gen};
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// -- the dump, in ref_l9ex_cmdset_run's format --

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

fn read_block(ini: &mut Ini<'_>, out: &mut Option<CommandSetUpgradeModuleData>) -> IniResult<()> {
    *out = Some(CommandSetUpgradeModuleData::friend_new_module_data(Some(ini))?);
    Ok(())
}

fn rust_read(name: &[u8], text: &[u8]) -> (IniResult<()>, Option<CommandSetUpgradeModuleData>) {
    let blocks = [BlockParse { token: "D", parse: read_block }];
    let mut out = None;
    let r = Ini::load(name, text, IniLoadType::Overwrite, None, &blocks, &mut out);
    (r, out)
}

// -- generators --

fn gen_name(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    if clean && !upgrades.is_empty() {
        return upgrades[g.below(upgrades.len() as u32) as usize].clone();
    }
    match g.below(100) {
        0..=64 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].clone(),
        65..=74 if !upgrades.is_empty() => upgrades[g.below(upgrades.len() as u32) as usize].to_lowercase(),
        75..=76 => format!("Unknown{}", g.below(3)),
        77 => "Up9".to_string(),
        78..=79 => pick(g, &["None", "none", "NONE"]).to_string(),
        _ if !upgrades.is_empty() => upgrades[0].clone(),
        _ => format!("Unknown{}", g.below(3)),
    }
}

fn gen_list(g: &mut Gen, upgrades: &[String], clean: bool) -> String {
    let n = 1 + g.below(3);
    (0..n).map(|_| gen_name(g, upgrades, clean)).collect::<Vec<_>>().join(" ")
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
    field(g, 85, "TriggeredBy", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 30, "ConflictsWith", v);
    let v = gen_list(g, upgrades, clean);
    field(g, 30, "RemovesUpgrades", v);
    let v = pick(g, &["FX_Up", "FX_Other", "None"]).to_string();
    field(g, 35, "FXListUpgrade", v);
    let v = gen_bool(g, clean);
    field(g, 30, "RequiresAllTriggers", v);
    let v = pick(g, &["SetA", "SetB", "Command_Set_X"]).to_string();
    field(g, 80, "CommandSet", v);
    let v = pick(g, &["AltA", "AltB"]).to_string();
    field(g, 70, "CommandSetAlt", v);
    let v = gen_name(g, upgrades, clean);
    field(g, 70, "TriggerAlt", v);
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
        for _ in 0..at_least + g.below(3) {
            let b = g.below(n_up as u32) as usize;
            w[b / 32] |= 1 << (b % 32);
        }
    }
    w
}

// -- the Rust side --

/// Runs the ops on a fresh world and returns the dump (after the data dump).
fn rust_run(
    ctx: Ctx,
    data: CommandSetUpgradeModuleData,
    has_player: bool,
    player_mask: [u32; 4],
    obj_mask: [u32; 4],
    ops: &[[u32; 5]],
) -> (Vec<u8>, Vec<u32>, World, [u64; 2]) {
    let mut fired = [0u64; 2];
    let mut v = Vec::new();
    let mut fx = FxIds::default();
    put_mux(&mut v, &data.upgrade.mux_data(), &mut fx);
    put_str(&mut v, &data.new_command_set);
    put_str(&mut v, &data.new_command_set_alt);
    put_str(&mut v, &data.trigger_alt);

    let mut w = World::new();
    w.statics = Arc::new(GameStatics { upgrade_center: ctx.center });
    w.names = ctx.names;
    w.cuts.neutral_default_team = 1;
    w.teams.insert_team(rts_lockstep_engine::logic::world::team::Team { id: 1, prototype: 0 });
    if has_player {
        w.cuts.team_players.insert(1, 0);
        w.cuts.player_upgrades.insert(0, mask_from(&player_mask));
    }
    w.cuts.trace = Some(Vec::new());
    let kind = ModuleDataKind::CommandSetUpgrade(Arc::new(data));
    let m = rts_lockstep_engine::logic::modules::new_module(0, b"CommandSetUpgrade", b"ModuleTag_CS", &kind).unwrap().unwrap();
    let mut status = rts_lockstep_engine::common::object_status_types::ObjectStatusMaskType::default();
    status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 1);
    let id = w
        .new_object(NewObject { template: Arc::new(ThingTemplate::new(0)), status, team: None, modules: vec![m] })
        .unwrap();
    {
        let o = w.objects.find_mut(id).unwrap();
        o.status.set(OBJECT_STATUS_UNDER_CONSTRUCTION, 0);
        o.object_upgrades_completed = mask_from(&obj_mask);
    }
    let idx = w.find_module(id, b"CommandSetUpgrade").unwrap();
    for o in ops {
        let mask = mask_from(&o[1..]);
        let errors_before = w.cuts.module_errors.len();
        let events_before = w.events.events().len();
        w.cuts.trace = Some(Vec::new());
        let res = match o[0] {
            0 => w.module_call(id, idx, ModuleCall::WouldUpgrade(mask)).as_bool(),
            1 => {
                let r = w.module_call(id, idx, ModuleCall::AttemptUpgrade(mask)).as_bool();
                if r {
                    fired[0] += 1;
                    fired[1] += u64::from(w.objects.find(id).unwrap().command_set_string_override.starts_with(b"Alt"));
                }
                r
            }
            2 => w.module_call(id, idx, ModuleCall::ResetUpgrade(mask)).as_bool(),
            3 => {
                w.module_call(id, idx, ModuleCall::ForceRefreshUpgrade);
                false
            }
            4 => w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool(),
            5 => {
                w.objects.find_mut(id).unwrap().object_upgrades_completed = mask;
                false
            }
            _ => {
                if has_player {
                    w.cuts.player_upgrades.insert(0, mask);
                }
                false
            }
        };
        let threw = w.cuts.module_errors.len() > errors_before;
        put_i32(&mut v, if threw { 2 } else { 0 });
        v.push(u8::from(res && !threw));
        v.push(u8::from(w.module_call(id, idx, ModuleCall::IsAlreadyUpgraded).as_bool()));
        // the effects, in order: the trace has each removal and each event, the events the FX names
        let fxs: Vec<Vec<u8>> = w.events.events()[events_before..]
            .iter()
            .filter_map(|e| match e {
                ClientEvent::Object(ObjectClientEvent::FxListDoFxObj { fx, .. }) => Some(fx.clone()),
                _ => None,
            })
            .collect();
        let mut fx_iter = fxs.into_iter();
        let mut log: Vec<String> = Vec::new();
        for t in w.cuts.trace.take().unwrap() {
            if t.starts_with("evt FxListDoFxObj") {
                log.push(format!("fx{}", fx.id(&fx_iter.next())));
            } else if t.starts_with("evt ControlBarMarkUiDirty") {
                log.push("dirty".into());
            } else if let Some(n) = t.strip_prefix("remove ") {
                log.push(format!("remove {n}"));
            }
        }
        put_u32(&mut v, log.len() as u32);
        for l in &log {
            put_str(&mut v, l.as_bytes());
        }
        put_str(&mut v, &w.objects.find(id).unwrap().command_set_string_override);
    }
    let keys = PROBE.iter().map(|n| w.names.name_to_key(n.as_bytes()).0).collect();
    (v, keys, w, fired)
}

const PROBE: [&str; 8] = ["none", "Up0", "Up3", "Unknown0", "Unknown2", "Up9", "SetA", "zz_fresh"];

#[test]
fn command_set_upgrade_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("L9EX_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(12_000);
    let mut g = Gen::new(0x9C5E);
    let table = blocks();
    let mut read = Check::new("CommandSetUpgradeModuleData read (throw, message)");
    let mut run = Check::new("CommandSetUpgrade ops (data, results, effects, command set)");
    let mut keys = Check::new("name keys after the ops");
    let (mut read_failed, mut ops_run, mut fired, mut alt, mut threw, mut removals, mut excluded) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for case in 0..cases {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
        let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
        let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
        let mut ctx = Ctx::new(init_names, init_center);
        Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
        cpp_reset(init_names, init_center);
        assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

        let clean = g.below(10) < 7;
        let text = gen_data(&mut g, &upgrades, clean);
        let name = format!("Data\\INI\\Cs{}.ini", case % 5).into_bytes();
        let (r, data) = rust_read(&name, &text);
        let outcome = match rust_outcome(r, 0) {
            Ok(o) => o,
            Err(e) => {
                assert!(e.is_undefined(), "{e}");
                excluded += 1;
                continue;
            }
        };
        let has_player = g.below(4) != 0;
        let player_mask = gen_mask(&mut g, n_up, 0);
        let obj_mask = gen_mask(&mut g, n_up, 0);
        let n_ops = 1 + g.below(12) as usize;
        let ops: Vec<[u32; 5]> = (0..n_ops)
            .map(|_| {
                let op = match g.below(20) {
                    0..=2 => 0,
                    3..=10 => 1,
                    11..=12 => 2,
                    13 => 3,
                    14 => 4,
                    15..=16 => 5,
                    _ => 6,
                };
                let m = gen_mask(&mut g, n_up, 1);
                [op, m[0], m[1], m[2], m[3]]
            })
            .collect();

        // Rust first: a case that reaches upstream undefined behaviour (a mux list naming `None`, which
        // upstream dereferences as a null template) is counted and never sent to the C++
        let rust = if outcome.status == 0 {
            let (rd, rk, w, f) = rust_run(ctx, data.expect("read ok"), has_player, player_mask, obj_mask, &ops);
            if w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined()) {
                excluded += 1;
                continue;
            }
            fired += f[0];
            alt += f[1];
            Some((rd, rk, w))
        } else {
            None
        };

        // C++
        let cfile = CString::new(name.clone()).unwrap();
        let flat: Vec<u32> = ops.iter().flatten().copied().collect();
        let mut buf = vec![0u8; 1 << 20];
        let mut msg = vec![0u8; 4096];
        let (mut len, mut code) = (0i32, 0u32);
        let status = unsafe {
            cpp::ref_ini_reset();
            cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
            cpp::ref_l9ex_cmdset_run(
                cfile.as_ptr(),
                i32::from(has_player),
                player_mask.as_ptr(),
                obj_mask.as_ptr(),
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
        let what = || format!("case {case}: upgrades {upgrades:?} player {has_player} {player_mask:?} obj {obj_mask:?}, file {:?}, ops {ops:?}", show(&text));
        read.eq(&outcome, &Outcome { status, detail, crc: 0 }, what);
        let Some((rd, rk, w)) = rust else {
            read_failed += 1;
            continue;
        };
        run.eq(&rd, &buf, what);
        let ck: Vec<u32> = PROBE
            .iter()
            .map(|n| {
                let c = CString::new(*n).unwrap();
                unsafe { cpp::ref_up_name_key(c.as_ptr()) }
            })
            .collect();
        keys.eq(rk, ck, what);
        ops_run += ops.len() as u64;
        threw += w.cuts.module_errors.len() as u64;
        removals += w.events.events().len() as u64;
    }
    println!(
        "CommandSetUpgrade: {cases} cases, {read_failed} data blocks threw, {excluded} excluded (undefined upstream), {ops_run} ops, \
         {fired} upgrades fired ({alt} gave the alternative), {threw} upstream throws inside ops, {removals} client events"
    );
    read.finish();
    run.finish();
    keys.finish();
    assert!(read_failed > 50 && ops_run > 10_000 && fired > 2000 && alt > 300 && threw > 50, "the generator must reach every path");
}
