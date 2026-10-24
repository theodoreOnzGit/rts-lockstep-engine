#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/OCLUpdate.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `OCLUpdate` (`logic::modules::ocl_update`) against the unchanged C++, `p1w12-sonnet`, update batch N.
//!
//! # Method
//!
//! The C++ side is `OCLUpdate.cpp`, compiled unchanged into `wrap_module.cpp`'s unit (`differential/l9_ocl_update_n.inc`) on the module build's stand-ins:
//! an `Object` with a position, an orientation, status bits and a disabled flag; three players (a colour, a playable flag, a template with a side); a
//! terrain logic whose `findClosestEdgePoint` adds an offset the test sets; the object creation lists `OCL_A` and `OCL_B` of batch B (the five-argument
//! `create` is new and logs); a game logic whose frame the test sets; and the REAL `UpdateModule`, `INI`, `Xfer` and logic random values.
//!
//! The Rust side is the real thing: a `World` with one object, the module from `modules::new_module`, the player tables of `ObjectCuts`
//! (`player_colors`, `unplayable_players`, `player_sides`, `team_players`), `update()` run as the scheduler runs it (`World::with_body`), the three interface
//! calls as `World::module_call`s, and the xfers through `World::xfer_module_slot`.
//!
//! Each case reads a `D` block (the list, up to three `FactionOCL` lines, the delays and two flags; mostly well formed) and, when the read succeeds, makes the
//! module and runs 1 to 40 ops: updates, a new frame, the object disabled or under construction, a new controlling player, a player's colour, playable flag
//! and template, the interface calls, the position and orientation, the edge offset, and the three xfers.
//!
//! Compared, bit for bit: the read's throw; the data (the list, the faction lists, the delays, the flags); per op the throw status, the result (the sleep
//! time, the float bits or the count), the effects in order (`oclat <list> <position> <position> <angle>`), the xfer status, bytes and CRC, and the four state
//! words.
//!
//! # Deviations the test cannot see
//!
//! The four bytes of the scheduling half's frame-and-phase word in an xfer save are masked in both dumps (as in batch I). A `FactionOCL` line that ends
//! after the faction's name (upstream compares a `NULL` token with `stricmp`) is never sent to the C++: the Rust read records
//! [`Undefined::NullTokenCompared`](rts_lockstep_engine::common::ini::Undefined) and the case is counted as excluded.
//!
//! # Results
//!
//! 2026-10-21, upstream commit 0a05454d, `L9OC_CASES` = 9000 (the default), `cargo test --release --features differential`: 9000 cases, 1074 data
//! blocks threw (read compared: 8872 comparisons, 0 mismatches), 128 cases excluded (a `FactionOCL` line that ends after the faction's name), 160074 ops,
//! 52642 updates, 17702 xfers, 1940 effect lines (1940 lists created); ops: 183468 comparisons, 0 mismatches, 0 NaN-payload-only, max ULP 0. The first run
//! found one defect of the test (the C++ driver took a player index from the wrong word of the op).
//!
//! # Planted bugs
//!
//! Each runs the Rust side with `L9_PLANT=k` (`logic::modules::plant`).
//!
//! | plants | what is broken |
//! |---|---|
//! | 1001 | an object under construction creates its list |
//! | 1002 | the delay is always `MinDelay` |
//! | 1003 | `xfer` leaves out the owner's colour |
//! | 1004 | a disabled object does not push the next creation back |
//! | 1005 | the faction name is compared without a case |
//! | 1006 | `CreateAtEdge` creates at the object |
//! | 1007 | an unplayable owner counts as a faction |
//! | 1008 | a change of the owner's colour does not restart the timer |
//! | 1009 | the first try creates at once |
//!
//! All 9 are caught (9000 cases each), all as mismatches.

mod harness;
mod mux_common;
mod support;
mod up_common;

use harness::*;
use mux_common::*;
use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::common::disabled_types::DISABLED_HACKED;
use rts_lockstep_engine::common::ini::{BlockParse, Ini, IniLoadType, IniResult};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::modules::ocl_update::OclUpdateModuleData;
use rts_lockstep_engine::logic::world::game_logic::World;
use rts_lockstep_engine::logic::world::module_call::{ModuleCall, ModuleHandle};
use rts_lockstep_engine::logic::world::object_create::NewObject;
use rts_lockstep_engine::logic::world::object_cuts::CutCall;
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use std::cell::RefCell;
use std::sync::Arc;
use support::Gen;
use up_common::*;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
const XFER_FILE_PATH: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/l9oc_n.bin");
const XFER_FILE: &str = "l9oc_n.bin";
const TEAM: u32 = 100;
const SIDES: [&str; 3] = ["China", "USA", "GLA"];

fn read_block(ini: &mut Ini<'_>, out: &mut Option<ModuleDataKind>) -> IniResult<()> {
    *out = Some(ModuleDataKind::OclUpdate(Arc::new(
        OclUpdateModuleData::friend_new_module_data(Some(ini))?,
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

fn ocl_code(name: &Option<Vec<u8>>) -> u32 {
    match name.as_deref() {
        Some(b"OCL_A") => 1,
        Some(b"OCL_B") => 2,
        _ => 0,
    }
}

fn put_data(v: &mut Vec<u8>, data: &ModuleDataKind) {
    let ModuleDataKind::OclUpdate(d) = data else {
        unreachable!("not an OCLUpdate")
    };
    put_u32(v, ocl_code(&d.ocl));
    put_u32(v, d.faction_ocl.len() as u32);
    for f in &d.faction_ocl {
        put_str(v, &f.faction_name);
        put_u32(v, ocl_code(&f.ocl));
    }
    put_u32(v, d.min_delay);
    put_u32(v, d.max_delay);
    put_u32(v, u32::from(d.is_create_at_edge));
    put_u32(v, u32::from(d.is_faction_triggered));
}

fn decode(b: &[u8], n_ops: usize) -> Vec<String> {
    let mut out = Vec::new();
    if b.is_empty() {
        return out;
    }
    let mut r = Reader::new(b);
    let ocl = r.u32();
    let nf = r.u32();
    let factions: Vec<(String, u32)> = (0..nf).map(|_| (r.str(), r.u32())).collect();
    let rest: Vec<u32> = (0..4).map(|_| r.u32()).collect();
    out.push(format!("data ocl {ocl} factions {factions:?} {rest:?}"));
    for i in 0..=n_ops {
        if r.at_end() {
            break;
        }
        let (mut st, mut res, mut log, mut x) = (0, 0, Vec::new(), String::new());
        if i > 0 {
            st = r.u32() as i32;
            res = r.u32();
            let nlog = r.u32();
            log = (0..nlog).map(|_| r.str()).collect();
            x = r.xfer();
        }
        let state: Vec<u32> = (0..4).map(|_| r.u32()).collect();
        out.push(format!(
            "op {i}: status {st} result {res} log {log:?} {x} state {state:?}"
        ));
    }
    out
}

fn gen_ocl_name(g: &mut Gen) -> String {
    pick(
        g,
        &["OCL_A", "OCL_B", "OCL_A", "None", "none", "ocl_a", "OCL_Z"],
    )
    .to_string()
}

fn gen_dur(g: &mut Gen, clean: bool) -> String {
    match g.below(12) {
        0..=1 => "0".to_string(),
        2..=7 => pick(g, &["100", "300", "500", "33", "34", "67", "200", "1000"]).to_string(),
        8 => format!("{}", g.below(5_000)),
        9 if !clean => pick(g, &["x", "", "-", "12abc"]).to_string(),
        _ => "300".to_string(),
    }
}

fn gen_faction_line(g: &mut Gen, clean: bool) -> String {
    let side = pick(g, &["China", "USA", "GLA", "china", "Other"]);
    if clean || g.below(10) < 7 {
        return format!("Faction:{side} OCL:{}", gen_ocl_name(g));
    }
    match g.below(7) {
        0 => format!("Fact:{side} OCL:OCL_A"),
        1 => format!("Faction:{side}"),
        2 => format!("Faction:{side} OCL"),
        3 => format!("Faction:{side} Other:OCL_A"),
        4 => "Faction".to_string(),
        5 => "Faction:".to_string(),
        _ => format!("Faction = {side} OCL = {}", gen_ocl_name(g)),
    }
}

fn gen_data(g: &mut Gen, _upgrades: &[String], clean: bool) -> Vec<u8> {
    let mut lines = Vec::new();
    let mut field = |g: &mut Gen, p: u32, name: &str, value: String| {
        if g.below(100) < p {
            let sep = gen_sep(g);
            lines.push(format!("{name}{sep}{value}"));
        }
    };
    let v = gen_ocl_name(g);
    field(g, 75, "OCL", v);
    for _ in 0..g.below(4) {
        let v = gen_faction_line(g, clean);
        field(g, 100, "FactionOCL", v);
    }
    let v = gen_dur(g, clean);
    field(g, 80, "MinDelay", v);
    let v = gen_dur(g, clean);
    field(g, 80, "MaxDelay", v);
    let v = gen_bool(g, clean);
    field(g, 40, "CreateAtEdge", v);
    let v = gen_bool(g, clean);
    field(g, 55, "FactionTriggered", v);
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

fn gen_case(
    g: &mut Gen,
    _data: &Option<ModuleDataKind>,
    _n_up: usize,
    _upgrades: &[String],
) -> (Vec<u32>, Vec<[u32; 5]>) {
    // [the frame, the logic random seed, the controlling player]
    let cfg = vec![g.below(200), g.u32(), g.below(4)];
    let n_ops = 1 + g.below(40) as usize;
    let mut frame = cfg[0];
    let ops = (0..n_ops)
        .map(|_| match g.below(100) {
            0..=29 => [0, 0, 0, 0, 0],
            30..=47 => {
                frame = frame.wrapping_add(1 + g.below(300));
                [1, frame, 0, 0, 0]
            }
            48..=50 => [2, g.below(2), 0, 0, 0],
            51..=54 => [3, g.below(2), 0, 0, 0],
            55..=60 => [4, g.below(4), 0, 0, 0],
            61..=63 => [
                5,
                0,
                g.below(3),
                pick(g, &[&100u32, &200, &300, &7]).to_owned(),
                0,
            ],
            64..=66 => [6, 0, g.below(3), g.below(2), 0],
            67..=69 => [7, 0, g.below(3), g.below(4), 0],
            70..=71 => [8, 0, 0, 0, 0],
            72..=74 => [9, 0, 0, 0, 0],
            75..=76 => [10, 0, 0, 0, 0],
            77..=80 => [11, 0, 0, 0, 0],
            81..=84 => [12, 0, 0, 0, 0],
            85..=87 => [13, 0, 0, 0, 0],
            88..=90 => [
                14,
                g.uniform(-50.0, 50.0).to_bits(),
                g.uniform(-50.0, 50.0).to_bits(),
                g.uniform(0.0, 20.0).to_bits(),
                0,
            ],
            91..=93 => [15, g.uniform(-3.0, 3.0).to_bits(), 0, 0, 0],
            94..=96 => [
                16,
                g.uniform(-9.0, 9.0).to_bits(),
                g.uniform(-9.0, 9.0).to_bits(),
                g.uniform(-9.0, 9.0).to_bits(),
                0,
            ],
            _ => [0, 0, 0, 0, 0],
        })
        .collect();
    (cfg, ops)
}

#[derive(Default)]
struct Tally {
    effects: u64,
    xfers: u64,
    updates: u64,
    creations: u64,
}

fn rust_run(
    ctx: Ctx,
    data: ModuleDataKind,
    cfg: &[u32],
    ops: &[[u32; 5]],
    tally: &RefCell<Tally>,
) -> RunOut {
    let mut v = Vec::new();
    put_data(&mut v, &data);

    let mut w = World::new();
    w.statics = Arc::new(rts_lockstep_engine::logic::world::statics::GameStatics {
        upgrade_center: ctx.center,
        special_power_store: ctx.store,
        ..Default::default()
    });
    w.names = ctx.names;
    w.cuts.trace = Some(Vec::new());
    w.cuts.log = Some(Vec::new());
    w.random.init_game_logic_random(cfg[1]);
    w.set_frame(cfg[0]);
    w.teams.insert_team(Team::new(TEAM, 0)).unwrap();
    for p in 0..3i32 {
        w.cuts.player_colors.insert(p, 100 * (p + 1));
        w.cuts
            .player_sides
            .insert(p, SIDES[p as usize].as_bytes().to_vec());
    }
    let set_controller = |w: &mut World, p: u32| {
        if p == 0 {
            w.cuts.team_players.remove(&TEAM);
        } else {
            w.cuts.team_players.insert(TEAM, (p - 1) as i32 % 3);
        }
    };
    set_controller(&mut w, cfg[2]);
    let m = rts_lockstep_engine::logic::modules::new_module(
        w.next_object_id(),
        b"OCLUpdate",
        b"ModuleTag_Ocl",
        &data,
    )
    .unwrap()
    .unwrap();
    let template = Arc::new(ThingTemplate::from_object_values(
        &ObjectTemplateValues::default(),
    ));
    let me = w
        .new_object(NewObject {
            template,
            status: Default::default(),
            team: Some(TEAM),
            modules: vec![m],
        })
        .unwrap();
    assert_eq!(me, 1, "the first object of a fresh world has the id 1");
    w.set_position(
        me,
        &Coord3D {
            x: 11.0,
            y: 22.0,
            z: 5.0,
        },
    );
    w.set_orientation(me, 0.5);
    let idx = w.find_module(me, b"OCLUpdate").unwrap();
    let u = w
        .update_handle_of_module(ModuleHandle {
            object: me,
            index: idx,
        })
        .expect("an update module");
    let mut saved: Vec<u8> = Vec::new();
    for phase in 0..=ops.len() {
        w.cuts.trace = Some(Vec::new());
        w.cuts.log = Some(Vec::new());
        let errors_before = w.cuts.module_errors.len();
        let (mut xst, mut xbytes, mut crc, mut res) = (-2i32, Vec::new(), 0u32, 0u32);
        if phase > 0 {
            let o = &ops[phase - 1];
            match o[0] {
                0 => {
                    res = w
                        .with_body(u, |b, w| b.update(w, u))
                        .expect("the module is in its slot")
                        .0;
                    tally.borrow_mut().updates += 1;
                }
                1 => w.set_frame(o[1]),
                2 => w
                    .objects
                    .find_mut(me)
                    .unwrap()
                    .disabled
                    .set(DISABLED_HACKED, i32::from(o[1] != 0)),
                3 => w
                    .objects
                    .find_mut(me)
                    .unwrap()
                    .status
                    .set(OBJECT_STATUS_UNDER_CONSTRUCTION, i32::from(o[1] != 0)),
                4 => set_controller(&mut w, o[1]),
                5 => {
                    w.cuts.player_colors.insert((o[2] % 3) as i32, o[3] as i32);
                }
                6 => {
                    let p = (o[2] % 3) as i32;
                    if o[3] != 0 {
                        w.cuts.unplayable_players.remove(&p);
                    } else {
                        w.cuts.unplayable_players.insert(p);
                    }
                }
                7 => {
                    let p = (o[2] % 3) as i32;
                    if o[3] == 0 {
                        w.cuts.player_sides.remove(&p);
                    } else {
                        w.cuts
                            .player_sides
                            .insert(p, SIDES[(o[3] as usize - 1) % 3].as_bytes().to_vec());
                    }
                }
                8 => {
                    w.module_call(me, idx, ModuleCall::OclUpdateResetTimer);
                }
                9 => {
                    res = w
                        .module_call(me, idx, ModuleCall::OclUpdateGetCountdownPercent)
                        .as_real(0.0)
                        .to_bits()
                }
                10 => {
                    res = w
                        .module_call(me, idx, ModuleCall::OclUpdateGetRemainingFrames)
                        .as_int(0) as u32
                }
                11..=13 => {
                    let r = run_xfer(&mut w, me, idx as usize, o[0] - 11, &saved, XFER_FILE);
                    xst = r.0;
                    xbytes = r.1;
                    crc = r.2;
                    if o[0] == 11 {
                        saved = xbytes.clone();
                    }
                    tally.borrow_mut().xfers += 1;
                }
                14 => w.set_position(
                    me,
                    &Coord3D {
                        x: f32::from_bits(o[1]),
                        y: f32::from_bits(o[2]),
                        z: f32::from_bits(o[3]),
                    },
                ),
                15 => w.set_orientation(me, f32::from_bits(o[1])),
                _ => {
                    w.cuts.edge_point_offset = Coord3D {
                        x: f32::from_bits(o[1]),
                        y: f32::from_bits(o[2]),
                        z: f32::from_bits(o[3]),
                    }
                }
            }
        }
        let threw = w.cuts.module_errors.len() > errors_before;
        let cuts = w.cuts.log.take().unwrap_or_default();
        let mut log = Vec::new();
        for c in cuts {
            if let CutCall::OclCreateAt {
                ocl,
                primary_pos,
                secondary_pos,
                angle,
                ..
            } = c
            {
                let i = match ocl.as_slice() {
                    b"OCL_A" => 0,
                    b"OCL_B" => 1,
                    _ => continue, // a name that is no list makes nothing
                };
                log.push(format!(
                    "oclat {i} {:08x},{:08x},{:08x} {:08x},{:08x},{:08x} {:08x}",
                    primary_pos.x.to_bits(),
                    primary_pos.y.to_bits(),
                    primary_pos.z.to_bits(),
                    secondary_pos.x.to_bits(),
                    secondary_pos.y.to_bits(),
                    secondary_pos.z.to_bits(),
                    angle.to_bits()
                ));
                tally.borrow_mut().creations += 1;
            }
        }
        tally.borrow_mut().effects += log.len() as u64;
        if phase > 0 {
            put_i32(&mut v, if threw { 2 } else { 0 });
            put_u32(&mut v, if threw { 0 } else { res });
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
        let state =
            w.objects
                .find(me)
                .unwrap()
                .behaviors
                .iter()
                .find_map(|b| match b.body.as_ref() {
                    Some(UpdateBody::OclUpdate(m)) => Some(m.state_words()),
                    _ => None,
                });
        for s in state.expect("the module is back in its slot") {
            put_u32(&mut v, s);
        }
    }
    let excluded = w.cuts.module_errors.iter().any(|(_, e)| e.is_undefined());
    RunOut {
        dump: v,
        keys: Vec::new(),
        excluded,
    }
}

#[test]
fn ocl_update_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tally = RefCell::new(Tally::default());
    let h = Harness {
        label: "update module N3",
        seed: 0x0C10_0DA7,
        cases_var: "L9OC_CASES",
        ini_stem: "Oc",
        gen_data: &gen_data,
        rust_read: &rust_read,
        gen_case: &gen_case,
        rust_run: &|ctx, data, cfg, ops| rust_run(ctx, data, cfg, ops, &tally),
        cpp_run: &|name, xfer, cfg, ops| call_driver(cpp::ref_l9oc_run, name, xfer, cfg, ops),
        decode: &decode,
        probe: &[],
        xfer_path: XFER_FILE_PATH,
    };
    let c = h.run();
    let t = tally.borrow();
    println!(
        "update module N3: {} cases, {} data blocks threw, {} excluded (undefined upstream), {} ops, {} updates, {} xfers, {} effect lines, {} lists created",
        c.cases, c.read_failed, c.excluded, c.ops_run, t.updates, t.xfers, t.effects, t.creations
    );
    assert!(
        c.read_failed * 40 > c.cases as u64
            && c.ops_run * 5 > c.cases as u64 * 4
            && t.creations * 10 > c.cases as u64,
        "the generator must reach every path"
    );
}
