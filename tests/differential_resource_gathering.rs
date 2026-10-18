// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::resource_gathering_manager` (`ResourceGatheringManager`) against
//! upstream's own text (commit 0a05454d): `Common/RTS/ResourceGatheringManager.cpp`, compiled unchanged in
//! `differential/wrap_resource_gathering.cpp` (see build.rs). Phase 1 layer 7, p1w7-sonnet.
//!
//! # What is compared against what
//!
//! The C++ file runs on the stand-ins of `differential/shim_resource`: eight objects of a test world (each with or
//! without an AI, a supply-truck AI interface with a preferred dock and a scan distance, a dock interface that is
//! or is not clear to approach, the warehouse and center dock modules, a position), `TheGameLogic`'s
//! `findObjectByID`, `ActionManager::canTransferSuppliesAt` (a table), and the partition manager's
//! `getDistanceSquared` for `FROM_CENTER_3D` (the one kind the file uses). The Rust side is the real `World` with
//! eight real `Object`s whose modules are `ProbeModule`s with the same answers.
//!
//! # Method
//!
//! A **program** is a header (each object's existence, flags, preferred dock, scan distance and position, and the
//! supply-transfer table) and a list of operations: `addSupplyCenter`/`removeSupplyCenter`/
//! `addSupplyWarehouse`/`removeSupplyWarehouse` (an object or `NULL`), `findBestSupplyWarehouse` and
//! `findBestSupplyCenter` (a query object or `NULL`), a change to an object (it vanishes, its dock clears or
//! blocks, its preferred dock or scan distance, its position, its row of the transfer table), and `XferSave` of
//! the manager followed by `XferLoad` into a fresh one. The same words run on both sides. Compared line for line:
//! every return value; **every call the manager makes on the world, in order, with its arguments** (the
//! `canTransferSuppliesAt` question and its answer, the dock's `isClearToApproach`, the distance's bits); the
//! bytes (length and hash) of each save and the status of each load; and, after every operation, both lists.
//!
//! **Inputs for which upstream is undefined behaviour** (a supply center that no longer exists, for which
//! `findBestSupplyCenter` erases from the wrong list; a destination without a dock interface that the transfer
//! table accepts) are never sent to the C++: the generator keeps a model of the world and avoids them, and
//! `undefined_inputs_are_errors` sends each to the Rust alone.
//!
//! # Results
//!
//! Run 2026-10-18 (`cargo test --release --features differential --test differential_resource_gathering --
//! --nocapture`): **1,500 random programs of 40 operations, 215,917 lines compared, 0 programs differ**; in them
//! the C++ asked `canTransferSuppliesAt` 37,299 times, the dock's `isClearToApproach` 14,582 times, and the
//! distance 14,046 times (each with its arguments and answer), and returned an object from 5,528 finds.
//! `undefined_inputs_are_errors`: two classes of undefined input, each an error in Rust.
//!
//! **Planted bugs** (each put into the Rust translation alone, the default run repeated, then removed): 12 plants
//! (the distance filter `<=`; ties to the later warehouse; the preferred dock winning when not usable; vanished
//! warehouses kept; the scan distance not squared; the centers' override looking for the warehouse module;
//! `removeSupplyCenter` removing one entry; the transfer question skipped; a default scan distance of 10,000
//! squared; the lists saved in the other order; the distance in 2D; the farthest center chosen). **All 12 are
//! caught** (20 to 1,488 of 1,500 programs differ; the rarest is the default scan distance, 20).
#![cfg(feature = "differential")]

mod support;

use std::sync::Arc;

use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_ID};
use rts_lockstep_engine::common::module::MODULEINTERFACE_BODY;
use rts_lockstep_engine::common::thing_template::ThingTemplate;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferResult};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::resource_gathering_manager::{ResourceError, ResourceGatheringManager};
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::update_body::UpdateBody;
use rts_lockstep_engine::logic::world::World;
use support::Gen;

// ---- the program -----------------------------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Prog(Vec<u32>);

impl Prog {
    fn u(&mut self, v: u32) -> &mut Self {
        self.0.push(v);
        self
    }
    fn i(&mut self, v: i32) -> &mut Self {
        self.0.push(v as u32);
        self
    }
    fn f(&mut self, v: f32) -> &mut Self {
        self.0.push(v.to_bits());
        self
    }
}

const F_AI: u32 = 1;
const F_TRUCK: u32 = 2;
const F_DOCK: u32 = 4;
const F_WAREHOUSE_MODULE: u32 = 8;
const F_CENTER_MODULE: u32 = 16;
const F_CLEAR: u32 = 32;

struct Rd<'a> {
    w: &'a [u32],
    at: usize,
}

impl Rd<'_> {
    fn u(&mut self) -> u32 {
        let v = self.w.get(self.at).copied().unwrap_or(0xFFFF_FFFF);
        self.at += 1;
        v
    }
    fn i(&mut self) -> i32 {
        self.u() as i32
    }
    fn f(&mut self) -> f32 {
        f32::from_bits(self.u())
    }
}

// ---- the Rust side ---------------------------------------------------------------------------------------------

fn fnv_hex(bytes: &[u8]) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in bytes {
        for c in format!("{b:02x}").bytes() {
            h ^= u32::from(c);
            h = h.wrapping_mul(16_777_619);
        }
    }
    h
}

fn ids(v: &[ObjectId]) -> String {
    v.iter().map(|i| format!(" {i}")).collect()
}

struct Run {
    w: World,
    m: ResourceGatheringManager,
    lines: Vec<String>,
    /// The module indices of the AI and the dock module of each object (0 = none).
    ai: [Option<u16>; 9],
    dock: [Option<u16>; 9],
}

impl Run {
    fn new(r: &mut Rd) -> Run {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(Team { id: 1, prototype: 0 });
        w.cuts.trace = Some(Vec::new());
        let mut ai = [None; 9];
        let mut dock = [None; 9];
        let mut exists = [false; 9];
        for (n, ex) in exists.iter_mut().enumerate().skip(1) {
            *ex = r.u() != 0;
            let fl = r.u();
            let preferred = r.i();
            let scan = r.f();
            let (x, y, z) = (r.f(), r.f(), r.f());
            let mut modules = Vec::new();
            if fl & F_AI != 0 {
                let mut m = NewModule::plain(b"AIUpdate", b"ModuleTag_AI");
                m.is_ai = true;
                m.interfaces = MODULEINTERFACE_BODY;
                let answers = ProbeAnswers { supply_truck: fl & F_TRUCK != 0, preferred_dock: preferred, scan_distance: scan, ..Default::default() };
                m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 1.0, Default::default())));
                ai[n] = Some(modules.len() as u16);
                modules.push(m);
            }
            if fl & F_DOCK != 0 {
                let mut m = NewModule::plain(b"DockProbe", b"ModuleTag_Dock");
                let answers = ProbeAnswers { has: 1 << 2, clear_to_approach: fl & F_CLEAR != 0, ..Default::default() };
                m.body = Some(UpdateBody::ModuleProbe(ProbeModule::new(answers, 1.0, Default::default())));
                dock[n] = Some(modules.len() as u16);
                modules.push(m);
            }
            if fl & F_WAREHOUSE_MODULE != 0 {
                modules.push(NewModule::plain(b"SupplyWarehouseDockUpdate", b"ModuleTag_Warehouse"));
            }
            if fl & F_CENTER_MODULE != 0 {
                modules.push(NewModule::plain(b"SupplyCenterDockUpdate", b"ModuleTag_Center"));
            }
            let tt = Arc::new(ThingTemplate::new(0));
            let id = w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules }).expect("object");
            assert_eq!(id, n as i32);
            w.set_position(id, &rts_lockstep_engine::common::base_type::Coord3D { x, y, z });
        }
        for q in 1..=8i32 {
            let row = r.u();
            for d in 1..=8i32 {
                if row & (1 << (d - 1)) != 0 {
                    w.cuts.can_transfer_supplies.insert((q, d));
                }
            }
        }
        for (n, ex) in exists.iter().enumerate().skip(1) {
            if !ex {
                let _ = w.objects.remove(n as i32);
            }
        }
        let mut run = Run { w, m: ResourceGatheringManager::new(), lines: Vec::new(), ai, dock };
        run.fix_indices();
        run.w.cuts.trace.as_mut().unwrap().clear();
        run.state("M");
        run
    }

    /// The module index of the AI and the dock probe of each object, found by name (the helpers come first).
    fn fix_indices(&mut self) {
        for n in 1..=8usize {
            self.ai[n] = None;
            self.dock[n] = None;
            if let Some(o) = self.w.objects.find(n as i32) {
                for (i, m) in o.modules.iter().enumerate() {
                    if m.name == b"AIUpdate" {
                        self.ai[n] = Some(i as u16);
                    }
                    if m.name == b"DockProbe" {
                        self.dock[n] = Some(i as u16);
                    }
                }
            }
        }
    }

    fn line(&mut self, s: String) {
        self.lines.push(s);
    }

    fn state(&mut self, name: &str) {
        let s = format!("state {name} warehouses[{}] centers[{}]", ids(self.m.supply_warehouses()), ids(self.m.supply_centers()));
        self.line(s);
    }

    fn flush_log(&mut self) {
        let trace = std::mem::take(self.w.cuts.trace.as_mut().unwrap());
        for l in trace {
            if l.starts_with("rgm ") {
                self.lines.push(l);
            }
        }
    }

    fn probe_mut(&mut self, id: i32, idx: Option<u16>) -> Option<&mut ProbeModule> {
        let o = self.w.objects.find_mut(id)?;
        let slot = o.modules.get_mut(idx? as usize)?;
        match slot.body.as_mut() {
            Some(UpdateBody::ModuleProbe(p)) => Some(p),
            _ => None,
        }
    }

    fn ops(&mut self, r: &mut Rd) {
        loop {
            let op = r.i();
            if op == 99 || r.at > r.w.len() {
                break;
            }
            match op {
                1 => {
                    let id = r.i();
                    self.line("op addCenter".into());
                    self.m.add_supply_center((id != 0).then_some(id));
                }
                2 => {
                    let id = r.i();
                    self.line("op removeCenter".into());
                    self.m.remove_supply_center((id != 0).then_some(id));
                }
                3 => {
                    let id = r.i();
                    self.line("op addWarehouse".into());
                    self.m.add_supply_warehouse((id != 0).then_some(id));
                }
                4 => {
                    let id = r.i();
                    self.line("op removeWarehouse".into());
                    self.m.remove_supply_warehouse((id != 0).then_some(id));
                }
                5 => {
                    let id = r.i();
                    self.line("op findWarehouse".into());
                    let res = self.m.find_best_supply_warehouse(&mut self.w, (id != 0).then_some(id));
                    self.flush_log();
                    let got = res.expect("an undefined input reached the comparison");
                    self.line(format!("ret {}", got.unwrap_or(0)));
                }
                6 => {
                    let id = r.i();
                    self.line("op findCenter".into());
                    let res = self.m.find_best_supply_center(&mut self.w, (id != 0).then_some(id));
                    self.flush_log();
                    let got = res.expect("an undefined input reached the comparison");
                    self.line(format!("ret {}", got.unwrap_or(0)));
                }
                7 => {
                    let (id, field, v) = (r.i(), r.i(), r.u());
                    self.line("op mark".into());
                    let f = f32::from_bits(v);
                    match field {
                        0 => {
                            let _ = self.w.objects.remove(id);
                        }
                        1 => {
                            let idx = self.dock[id as usize];
                            if let Some(p) = self.probe_mut(id, idx) {
                                p.answers.clear_to_approach = v != 0;
                            }
                        }
                        2 => {
                            let idx = self.ai[id as usize];
                            if let Some(p) = self.probe_mut(id, idx) {
                                p.answers.preferred_dock = v as i32;
                            }
                        }
                        3 => {
                            let idx = self.ai[id as usize];
                            if let Some(p) = self.probe_mut(id, idx) {
                                p.answers.scan_distance = f;
                            }
                        }
                        4 | 5 => {
                            if let Some(o) = self.w.objects.find(id) {
                                let mut p = o.get_position();
                                if field == 4 {
                                    p.x = f;
                                } else {
                                    p.y = f;
                                }
                                self.w.set_position(id, &p);
                            }
                        }
                        6 => {
                            self.w.cuts.can_transfer_supplies.retain(|(q, _)| *q != id);
                            for d in 1..=8i32 {
                                if v & (1 << (d - 1)) != 0 {
                                    self.w.cuts.can_transfer_supplies.insert((id, d));
                                }
                            }
                        }
                        _ => {}
                    }
                    self.w.cuts.trace.as_mut().unwrap().clear();
                }
                8 => {
                    self.line("op saveLoad".into());
                    let mut x = XferSave::new();
                    let mut st = 0;
                    let res: XferResult<()> = (|| {
                        x.open(b"rgm")?;
                        x.xfer_snapshot(&mut self.m)?;
                        x.close()
                    })();
                    if let Err(e) = res {
                        st = e as i32 + 1;
                        let _ = x.close();
                    }
                    let bytes = x.take_bytes();
                    self.line(format!("ret save {st} len {} hash {:08x}", bytes.len(), fnv_hex(&bytes)));
                    let mut fresh = ResourceGatheringManager::new();
                    let mut lx = XferLoad::new(bytes);
                    let mut lst = 0;
                    let res: XferResult<()> = (|| {
                        lx.open(b"rgm")?;
                        lx.xfer_snapshot(&mut fresh)?;
                        lx.close()
                    })();
                    if let Err(e) = res {
                        lst = e as i32 + 1;
                        let _ = lx.close();
                    }
                    self.line(format!("ret load {lst}"));
                    let s = format!("state loaded warehouses[{}] centers[{}]", ids(fresh.supply_warehouses()), ids(fresh.supply_centers()));
                    self.line(s);
                    fresh.load_post_process();
                }
                other => panic!("unknown op {other}"),
            }
            self.flush_log();
            self.state("M");
        }
    }
}

fn run_rust(prog: &[u32]) -> Vec<String> {
    let mut r = Rd { w: prog, at: 0 };
    let mut run = Run::new(&mut r);
    run.ops(&mut r);
    run.lines
}

fn run_cpp(prog: &[u32]) -> Vec<String> {
    let mut cap = 1 << 20;
    loop {
        let mut buf = vec![0u8; cap];
        // SAFETY: the buffer is valid for `cap` bytes, which the wrapper does not exceed; the program is valid for its length.
        let n = unsafe { cpp::ref_resource_run(prog.as_ptr(), prog.len() as i32, buf.as_mut_ptr() as *mut _, cap as i32) };
        if n < 0 {
            cap = (-n) as usize + 16;
            continue;
        }
        buf.truncate(n as usize);
        return String::from_utf8(buf).expect("utf-8 lines").lines().map(str::to_string).collect();
    }
}

fn first_difference(r: &[String], c: &[String]) -> Option<String> {
    for i in 0..r.len().max(c.len()) {
        let (a, b) = (r.get(i), c.get(i));
        if a != b {
            return Some(format!("line {i}:\n  rust: {}\n  cpp:  {}", a.map_or("(none)", |s| s), b.map_or("(none)", |s| s)));
        }
    }
    None
}

// ---- the generator ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct Model {
    exists: [bool; 9],
    flags: [u32; 9],
    preferred: [i32; 9],
    rows: [u32; 9],
    warehouses: Vec<i32>,
    centers: Vec<i32>,
}

impl Model {
    fn transfer(&self, q: i32, d: i32) -> bool {
        self.rows[q as usize] & (1 << (d - 1)) != 0
    }
    /// Whether `find_*` for `q` can reach a destination that the transfer table accepts and that has no dock
    /// interface (or a vanished center): both undefined upstream.
    fn undefined_for(&self, q: i32, centers: bool) -> bool {
        let list = if centers { &self.centers } else { &self.warehouses };
        let mut cands: Vec<i32> = list.iter().copied().filter(|d| self.exists[*d as usize]).collect();
        if self.flags[q as usize] & F_TRUCK != 0 && self.flags[q as usize] & F_AI != 0 {
            let p = self.preferred[q as usize];
            if (1..=8).contains(&p) && self.exists[p as usize] {
                cands.push(p);
            }
        }
        if centers && list.iter().any(|d| !self.exists[*d as usize]) {
            return true;
        }
        cands.iter().any(|d| self.transfer(q, *d) && self.flags[*d as usize] & F_DOCK == 0)
    }
}

fn pick_coord(g: &mut Gen) -> f32 {
    match g.below(12) {
        0 => 1.0e19,
        1 => -1.0e19,
        2 => 0.0,
        _ => g.uniform(-600.0, 600.0),
    }
}

fn pick_scan(g: &mut Gen) -> f32 {
    [0.0, 10.0, 100.0, 300.0, 800.0, 5000.0, 1.0e10, -50.0][g.below(8) as usize]
}

fn gen_program(seed: u64, n_ops: usize) -> Prog {
    let mut g = Gen::new(seed);
    let mut p = Prog::default();
    let mut m = Model { exists: [false; 9], flags: [0; 9], preferred: [0; 9], rows: [0; 9], warehouses: Vec::new(), centers: Vec::new() };
    for n in 1..=8usize {
        let exists = g.below(12) != 0;
        let mut fl = 0;
        for (bit, odds) in [(F_AI, 3), (F_TRUCK, 3), (F_DOCK, 4), (F_WAREHOUSE_MODULE, 3), (F_CENTER_MODULE, 3), (F_CLEAR, 4)] {
            // `odds` out of 4: a flag is more often set than not
            if g.below(4) < odds {
                fl |= bit;
            }
        }
        let preferred = if g.below(3) == 0 { 0 } else { 1 + g.below(8) as i32 };
        p.u(u32::from(exists)).u(fl).i(preferred).f(pick_scan(&mut g)).f(pick_coord(&mut g)).f(pick_coord(&mut g)).f(pick_coord(&mut g));
        m.exists[n] = exists;
        m.flags[n] = fl;
        m.preferred[n] = preferred;
    }
    for q in 1..=8usize {
        let row = if g.below(5) == 0 { 0 } else { g.u32() & 0xFF };
        p.u(row);
        m.rows[q] = row;
    }
    let existing = |m: &Model| -> Vec<i32> { (1..=8).filter(|i| m.exists[*i as usize]).collect() };
    for _ in 0..n_ops {
        match g.below(30) {
            0..=3 => {
                let id = g.below(9) as i32;
                if id == 0 || m.exists[id as usize] {
                    p.i(1).i(id);
                    if id != 0 {
                        m.centers.push(id);
                    }
                }
            }
            4 => {
                let id = g.below(9) as i32;
                p.i(2).i(id);
                m.centers.retain(|c| *c != id);
            }
            5..=9 => {
                let id = g.below(9) as i32;
                if id == 0 || m.exists[id as usize] {
                    p.i(3).i(id);
                    if id != 0 {
                        m.warehouses.push(id);
                    }
                }
            }
            10 => {
                let id = g.below(9) as i32;
                p.i(4).i(id);
                m.warehouses.retain(|c| *c != id);
            }
            11..=17 => {
                let ex = existing(&m);
                if ex.is_empty() {
                    continue;
                }
                let q = if g.below(10) == 0 { 0 } else { ex[g.below(ex.len() as u32) as usize] };
                if q == 0 || !m.undefined_for(q, false) {
                    p.i(5).i(q);
                    // (the warehouses that vanished are dropped from upstream's list when met; the model needs only
                    // the existing entries, which `undefined_for` filters)
                }
            }
            18..=22 => {
                let ex = existing(&m);
                if ex.is_empty() {
                    continue;
                }
                let q = if g.below(10) == 0 { 0 } else { ex[g.below(ex.len() as u32) as usize] };
                if q == 0 || !m.undefined_for(q, true) {
                    p.i(6).i(q);
                }
            }
            23..=27 => {
                let ex = existing(&m);
                if ex.is_empty() {
                    continue;
                }
                let id = ex[g.below(ex.len() as u32) as usize];
                match g.below(7) {
                    0 => {
                        if g.below(3) == 0 {
                            p.i(7).i(id).i(0).u(0);
                            m.exists[id as usize] = false;
                        }
                    }
                    1 => {
                        if m.flags[id as usize] & F_DOCK != 0 {
                            let clear = g.below(2);
                            p.i(7).i(id).i(1).u(clear);
                        }
                    }
                    2 => {
                        if m.flags[id as usize] & F_AI != 0 {
                            let pd = g.below(9);
                            p.i(7).i(id).i(2).u(pd);
                            m.preferred[id as usize] = pd as i32;
                        }
                    }
                    3 => {
                        if m.flags[id as usize] & F_AI != 0 {
                            p.i(7).i(id).i(3).f(pick_scan(&mut g));
                        }
                    }
                    4 => {
                        p.i(7).i(id).i(4).f(pick_coord(&mut g));
                    }
                    5 => {
                        p.i(7).i(id).i(5).f(pick_coord(&mut g));
                    }
                    _ => {
                        let row = g.u32() & 0xFF;
                        p.i(7).i(id).i(6).u(row);
                        m.rows[id as usize] = row;
                    }
                }
            }
            _ => {
                p.i(8);
            }
        }
    }
    p.i(99);
    p
}

// ---- the tests -------------------------------------------------------------------------------------------------

#[test]
fn resource_gathering_manager_matches_upstream() {
    let cases: u64 = std::env::var("RGM_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    let (mut lines, mut bad) = (0usize, 0);
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for case in 0..cases {
        let prog = gen_program(0x6A66_0000 + case, 40);
        let c = run_cpp(&prog.0);
        let r = run_rust(&prog.0);
        lines += c.len();
        for l in &c {
            if l.starts_with("rgm") || l.starts_with("ret") {
                let key: Vec<&str> = l.split(' ').take(2).collect();
                *kinds.entry(key.join(" ")).or_default() += 1;
            }
        }
        if let Some(d) = first_difference(&r, &c) {
            bad += 1;
            if bad <= 3 {
                println!("DIFF case {case}: {d}");
            }
        }
    }
    println!("RESULT {cases} programs, {lines} lines compared, {bad} programs differ");
    for (k, n) in &kinds {
        println!("RESULT   {k}: {n}");
    }
    assert_eq!(bad, 0);
}

/// Each class of input for which upstream is undefined behaviour is an error in Rust, never sent to the C++.
#[test]
fn undefined_inputs_are_errors() {
    // eight existing objects; object 1 is a supply truck with an AI; 2 and 3 have no dock interface; 4 has one
    let mut p = Prog::default();
    for n in 1..=8u32 {
        let fl = match n {
            1 => F_AI | F_TRUCK,
            4 => F_DOCK | F_CLEAR,
            _ => 0,
        };
        p.u(1).u(fl).i(0).f(500.0).f(n as f32).f(0.0).f(0.0);
    }
    for _ in 0..8 {
        p.u(0xFF); // everyone may transfer supplies anywhere
    }
    p.i(99);
    let mut counted = 0;
    // 1. a supply center that no longer exists: erase from the wrong list
    {
        let mut r = Rd { w: &p.0, at: 0 };
        let mut run = Run::new(&mut r);
        run.m.add_supply_center(Some(4));
        run.m.add_supply_center(Some(5));
        let _ = run.w.objects.remove(5);
        assert_eq!(run.m.find_best_supply_center(&mut run.w, Some(1)), Err(ResourceError::EraseFromWrongList(5)));
        counted += 1;
        // a warehouse that no longer exists is fine: it is dropped
        run.m.add_supply_warehouse(Some(4));
        run.m.add_supply_warehouse(Some(6));
        let _ = run.w.objects.remove(6);
        assert_eq!(run.m.find_best_supply_warehouse(&mut run.w, Some(1)), Ok(Some(4)));
        assert_eq!(run.m.supply_warehouses(), &[4]);
    }
    // 2. a destination without a dock interface that the transfer table accepts
    {
        let mut r = Rd { w: &p.0, at: 0 };
        let mut run = Run::new(&mut r);
        run.m.add_supply_warehouse(Some(2));
        assert_eq!(run.m.find_best_supply_warehouse(&mut run.w, Some(1)), Err(ResourceError::NoDockInterface(2)));
        counted += 1;
    }
    println!("RESULT {counted} classes of undefined input are errors in Rust (and are never sent to the C++)");
    assert_eq!(counted, 2);
    let _ = (INVALID_ID, Snapshot::load_post_process as fn(&mut ResourceGatheringManager), Xfer::get_xfer_mode as fn(&XferSave) -> _);
}
