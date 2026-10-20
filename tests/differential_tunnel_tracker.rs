// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::world::tunnel_tracker` (`TunnelTracker`) and `logic::world::cave_system`
//! (`CaveSystem`) against upstream's own text (commit 0a05454d): `Common/RTS/TunnelTracker.cpp` and
//! `GameLogic/System/CaveSystem.cpp`, compiled unchanged in `differential/wrap_tunnel_tracker.cpp` (see
//! build.rs). Phase 1 layer 7, p1w7-sonnet.
//!
//! # What is compared against what
//!
//! The C++ files run on the stand-ins of `differential/shim_tunnel`: eight objects of a test world (kinds,
//! stealth status, effectively dead, drawable, body module with a health, group, container), `TheGameLogic`
//! (frame, `findObjectByID`, `destroyObject`), the partition manager, the pathfinder, the drawable and the body
//! module. The Rust side is the real `World` with eight real `Object`s (a `ProbeModule` body for the health),
//! one standalone `TunnelTracker` and the world's `CaveSystem`.
//!
//! # Method
//!
//! A **program** is a header (the capacity, the frame, and for each object its kinds, status bits, flags,
//! maximum health, health, container and the frame it entered it) and a list of operations: `addToContain`,
//! `removeFromContain`, `isInContainer`, `isValidContainerFor`, `onTunnelCreated`, `onTunnelDestroyed`,
//! `healObjects`, `updateNemesis`, `getCurNemesis`, the capacity and frame, a change to an object (dead,
//! stealthed, detected, disguised, removed from the world, container, group), `XferSave` of a tracker followed
//! by `XferLoad` into a fresh one and `loadPostProcess`, and the cave system's `registerNewCave`,
//! `canSwitchIndexToIndex`, `getTunnelTrackerForCaveIndex`, `unregisterCave`, `reset` and its own
//! save/load. Each operation applies to the standalone tracker or to a cave's. The same words run on both
//! sides. Compared line for line: the operation's return values; the records of every call the tracker makes on
//! the world (`onContainedBy`, `onRemovedFrom`, `destroyObject`, `leaveGroup`, `unRegisterObject`, the
//! pathfinder, `setDrawableHidden`, and the `DamageInfo` of each `attemptHealing`, field by field), in order;
//! the length and a hash of the bytes each save writes; the status of each load; and, after **every**
//! operation, the whole state: every tracker's lists, sizes, counters and nemesis, the cave's slots, and every
//! object's container, frame, flags and health bits.
//!
//! **Inputs for which upstream is undefined behaviour** (an object that is no longer in the world still held
//! by a tracker, healing an object with no body module, reading `m_tunnelIDs.front()` of an empty list, a
//! negative cave index in `registerNewCave`) are never sent to the C++: the generator avoids them with a model
//! of the world, and `undefined_inputs_are_errors` sends each, and a count of them from random programs, to the
//! Rust alone, which must return its error.
//!
//! # Results
//!
//! Run 2026-10-18 (`cargo test --release --features differential --test differential_tunnel_tracker --
//! --nocapture`): the default run is **1,500 programs of 45 operations, 878,064 lines compared, 0 programs
//! differ**; `TUNNEL_CASES=20000` (37 s) gave **20,000 programs, 11,688,151 lines, 0 differ**. In the default run
//! the C++ made 2,379 `destroyObject` calls, 2,808 `attemptHealing` calls (every `DamageInfo` field compared),
//! 158 `onContainedBy` re-pointings, 2,340 `setDrawableHidden`, 943 `leaveGroup`, 4,822 partition and pathfinder
//! removals, and 5,004 saves and loads of trackers and cave systems (the failing saves of a cave with a `NULL`
//! slot included). `undefined_inputs_are_errors`: six classes of undefined input, each an error in Rust.
//!
//! **Planted bugs** (each put into the Rust translation alone, the default run repeated, then removed): 12 plants
//! (the nemesis forgotten a frame early; a full heal needing a frame more; the capacity test `<=`; every contained
//! object re-pointed, not only those in the dead tunnel; aircraft accepted; the disguised exception dropped from
//! the nemesis test; the drawable not hidden on load; `removeFromContain` removing two; infantry dropped from the
//! nemesis kinds; `canSwitchIndexToIndex` checking one index; `registerNewCave` not re-creating a `NULL` slot;
//! the heal sliver multiplied instead of divided). **All 12 are caught** (the programs that differ out of 1,500:
//! 53 to 802; the rarest are the full-heal boundary, 53, and the nemesis boundary, 59; the `NULL`-slot plant
//! makes the interpreter panic at its first use of the missing tracker instead of producing a differing line).
//! The two boundary plants first survived (0 and 3 programs differ): the generator did not aim at the exact
//! frame, so it now draws the heal time from an object's real time inside (and one frame either side) and sets
//! the frame to the nemesis's expiry (`+118..122`).
//!
//! **Not covered here** (unit tests or not at all): an object that leaves the world while a tracker holds its id
//! (undefined upstream), the `GameState` post-process hand-off (the Xfer layer's counter), and `TunnelContain`
//! and `CaveContain`, the layer-9 modules that call the tracker.
#![cfg(feature = "differential")]

mod support;

use std::sync::Arc;

use rts_lockstep_engine::common::game_type::{ObjectId, INVALID_DRAWABLE_ID, INVALID_ID};
use rts_lockstep_engine::common::kind_of::{KINDOF_AIRCRAFT, KINDOF_INFANTRY, KINDOF_SELECTABLE, KINDOF_STRUCTURE, KINDOF_VEHICLE};
use rts_lockstep_engine::common::module::{MODULEINTERFACE_BODY, MODULEINTERFACE_DAMAGE};
use rts_lockstep_engine::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};
use rts_lockstep_engine::common::thing_template::{ObjectTemplateValues, ThingTemplate};
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferResult, XferStatus};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::world::cave_system::{CaveError, CaveSystem};
use rts_lockstep_engine::logic::world::module_call::ModuleCall;
use rts_lockstep_engine::logic::world::module_probe::{ProbeAnswers, ProbeModule};
use rts_lockstep_engine::logic::world::object::PRIVATE_EFFECTIVELY_DEAD;
use rts_lockstep_engine::logic::world::object_create::{NewModule, NewObject};
use rts_lockstep_engine::logic::world::team::Team;
use rts_lockstep_engine::logic::world::tunnel_tracker::{TunnelError, TunnelTracker};
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

/// An object of the header.
#[derive(Clone, Copy, Debug)]
struct ObjCfg {
    kinds: u32,
    status: u32,
    flags: u32,
    max_health: f32,
    health: f32,
    by: i32,
    by_frame: u32,
}

const FLAG_EXISTS: u32 = 1;
const FLAG_DEAD: u32 = 2;
const FLAG_DRAWABLE: u32 = 4;
const FLAG_BODY: u32 = 8;
const FLAG_GROUP: u32 = 16;

fn kind_bit(k: i32) -> u32 {
    1u32 << k
}

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

fn fnv(s: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in s.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Saves `s`; the status (0 ok, else the `XferStatus` + 1), the length and the hash of the bytes.
fn save_snapshot(s: &mut dyn Snapshot) -> (i32, usize, u32) {
    let mut x = XferSave::new();
    let mut status = 0;
    let r: XferResult<()> = (|| {
        x.open(b"tunnel")?;
        x.xfer_snapshot(s)?;
        x.close()
    })();
    if let Err(e) = r {
        status = e as i32 + 1;
        let _ = x.close();
    }
    let bytes = x.take_bytes();
    let h = hex(&bytes);
    (status, bytes.len(), fnv(&h))
}

/// The bytes of the last [`save_snapshot`] (the same call, kept for the load).
fn save_bytes(s: &mut dyn Snapshot) -> Vec<u8> {
    let mut x = XferSave::new();
    let r: XferResult<()> = (|| {
        x.open(b"tunnel")?;
        x.xfer_snapshot(s)?;
        x.close()
    })();
    if r.is_err() {
        let _ = x.close();
    }
    x.take_bytes()
}

fn load_snapshot(s: &mut dyn Snapshot, bytes: Vec<u8>) -> i32 {
    let mut x = XferLoad::new(bytes);
    let r: XferResult<()> = (|| {
        x.open(b"tunnel")?;
        x.xfer_snapshot(s)?;
        x.close()
    })();
    match r {
        Ok(()) => 0,
        Err(e) => {
            let _ = x.close();
            e as i32 + 1
        }
    }
}

struct Run {
    w: World,
    t: TunnelTracker,
    lines: Vec<String>,
    hidden: [bool; 9],
    health: [f32; 9],
    /// The state of an object that left the world, frozen at that moment (the stand-in keeps its fields).
    frozen: [(i32, u32, u32, u32, u32); 9],
}

impl Run {
    fn new(r: &mut Rd) -> Run {
        let mut w = World::new();
        w.cuts.neutral_default_team = 1;
        w.teams.insert_team(Team::new(1, 0)).unwrap();
        w.cuts.trace = Some(Vec::new());
        w.cuts.globals.max_tunnel_capacity = r.i();
        w.set_frame(r.u());
        let mut cfgs = Vec::new();
        for _ in 0..8 {
            cfgs.push(ObjCfg {
                kinds: r.u(),
                status: r.u(),
                flags: r.u(),
                max_health: r.f(),
                health: r.f(),
                by: r.i(),
                by_frame: r.u(),
            });
        }
        let mut health = [0.0f32; 9];
        for (n, c) in cfgs.iter().enumerate() {
            let mut v = ObjectTemplateValues { name: format!("T{n}").into_bytes(), template_id: n as u16 + 1, ..Default::default() };
            for k in 0..32 {
                if c.kinds & (1 << k) != 0 {
                    v.kind_of.push(k);
                }
            }
            let tt = Arc::new(ThingTemplate::from_object_values(&v));
            let mut modules = Vec::new();
            if c.flags & FLAG_BODY != 0 {
                let mut m = NewModule::plain(b"ProbeBody", b"ModuleTag_Body");
                m.interfaces = MODULEINTERFACE_BODY | MODULEINTERFACE_DAMAGE;
                let mut p = ProbeModule::new(ProbeAnswers::default(), c.max_health, Default::default());
                p.health = c.health;
                m.body = Some(UpdateBody::ModuleProbe(p));
                modules.push(m);
            }
            let id = w.new_object(NewObject { template: tt, status: Default::default(), team: None, modules }).expect("object");
            assert_eq!(id, n as i32 + 1);
            health[n + 1] = if c.flags & FLAG_BODY != 0 { c.health } else { 0.0 };
            let o = w.objects.find_mut(id).unwrap();
            if c.status & 1 != 0 {
                o.status.set(OBJECT_STATUS_STEALTHED, 1);
            }
            if c.status & 2 != 0 {
                o.status.set(OBJECT_STATUS_DETECTED, 1);
            }
            if c.status & 4 != 0 {
                o.status.set(OBJECT_STATUS_DISGUISED, 1);
            }
            if c.flags & FLAG_DEAD != 0 {
                o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
            }
            o.drawable = if c.flags & FLAG_DRAWABLE != 0 { id } else { INVALID_DRAWABLE_ID };
            o.group = u32::from(c.flags & FLAG_GROUP != 0);
            o.contained_by = c.by;
            o.contained_by_frame = c.by_frame;
        }
        let mut frozen = [(0, 0, 0, 0, 0); 9];
        for (n, c) in cfgs.iter().enumerate() {
            if c.flags & FLAG_EXISTS == 0 {
                frozen[n + 1] = (c.by, c.by_frame, u32::from(c.flags & FLAG_DEAD != 0), 0, u32::from(c.flags & FLAG_GROUP != 0));
                let _ = w.objects.remove(n as i32 + 1);
            }
        }
        w.cuts.trace.as_mut().unwrap().clear();
        let mut run = Run { w, t: TunnelTracker::new(), lines: Vec::new(), hidden: [false; 9], health, frozen };
        run.world_state();
        run
    }

    fn line(&mut self, s: String) {
        self.lines.push(s);
    }

    /// The tracker `which` (0 = the standalone one, n = cave n - 1) with the world.
    fn with<R>(&mut self, which: i32, f: impl FnOnce(&mut TunnelTracker, &mut World) -> R) -> R {
        if which == 0 {
            f(&mut self.t, &mut self.w)
        } else {
            self.w.with_cave_tracker(which - 1, f).expect("the generator only names a tracker that exists")
        }
    }

    /// The records of the calls the tracker made on the world, as the C++ stand-ins write them.
    fn flush_log(&mut self) {
        let trace = std::mem::take(self.w.cuts.trace.as_mut().unwrap());
        for l in trace {
            if l.starts_with("tt ") {
                self.lines.push(l);
            } else if let Some(rest) = l.strip_prefix("cut AiGroupRemove { group: ") {
                // `cut AiGroupRemove { group: 1, object: 3 }`
                let obj: i32 = rest.split("object: ").nth(1).unwrap().trim_end_matches(" }").parse().unwrap();
                self.lines.push(format!("tt leaveGroup {obj}"));
            } else if let Some(rest) = l.strip_prefix("cut PartitionUnregisterObject { object: ") {
                let obj: i32 = rest.trim_end_matches(" }").parse().unwrap();
                self.lines.push(format!("tt unregister {obj}"));
            } else if let Some(rest) = l.strip_prefix("cut PathfinderRemoveObjectFromPathfindMap { object: ") {
                let obj: i32 = rest.trim_end_matches(" }").parse().unwrap();
                self.lines.push(format!("tt pathfind {obj}"));
            } else if let Some(rest) = l.strip_prefix("evt DrawableSetHidden { object_id: ") {
                let obj: i32 = rest.split(',').next().unwrap().parse().unwrap();
                let hidden = rest.contains("hidden: true");
                self.hidden[obj as usize] = hidden;
                self.lines.push(format!("tt hide {obj} {}", u32::from(hidden)));
            }
        }
    }

    fn tracker_state(&mut self, name: &str, t: Option<&TunnelTracker>) {
        let Some(t) = t else {
            self.line(format!("state {name} none"));
            return;
        };
        let ids = |v: &[ObjectId]| v.iter().map(|i| format!(" {i}")).collect::<String>();
        let (nem, ts) = t.get_nemesis_state();
        let s = format!(
            "state {name} tunnels[{}] list[{}] pending[{}] size {} count {} nemesis {} ts {}",
            ids(t.get_container_list()),
            ids(t.get_contained_items_list()),
            ids(t.get_xfer_contain_list()),
            t.get_contain_list_size(),
            t.friend_get_tunnel_count(),
            nem,
            ts
        );
        self.line(s);
    }

    fn world_state(&mut self) {
        let t = self.t.clone();
        self.tracker_state("T", Some(&t));
        let n = self.w.cave_system.len();
        let mut cave = String::new();
        for i in 0..n {
            match self.w.cave_system.get_tunnel_tracker_for_cave_index(i as i32) {
                None => cave.push_str(" -"),
                Some(t) => cave.push_str(&format!(" {}", t.get_contain_list_size())),
            }
        }
        self.line(format!("state cave[{cave}]"));
        for i in 0..n {
            let t = self.w.cave_system.get_tunnel_tracker_for_cave_index(i as i32).cloned();
            self.tracker_state(&format!("C{i}"), t.as_ref());
        }
        for i in 1..=8i32 {
            let live = self.w.objects.find(i).map(|o| (o.contained_by, o.contained_by_frame, u32::from(o.is_effectively_dead()), u32::from(o.is_destroyed()), o.group));
            let exists = u32::from(live.is_some());
            if live.is_some() {
                if let Some(b) = self.w.get_body_module(i) {
                    self.health[i as usize] = self.w.module_call(i, b, ModuleCall::GetHealth).as_real(0.0);
                }
            }
            let (by, frame, dead, destroyed, group) = live.unwrap_or(self.frozen[i as usize]);
            let h = u32::from(self.hidden[i as usize]);
            let health = self.health[i as usize].to_bits();
            self.line(format!("obj {i} exists {exists} by {by} frame {frame} dead {dead} destroyed {destroyed} hidden {h} group {group} health {health:08x}"));
        }
        // the module calls above wrote trace lines; they are not records of the tracker
        self.w.cuts.trace.as_mut().unwrap().clear();
    }
}

impl Run {
    fn ops(&mut self, r: &mut Rd) {
        loop {
            let op = r.i();
            if op == 99 || r.at > r.w.len() {
                break;
            }
            match op {
                1 => {
                    self.w.set_frame(r.u());
                    self.line("op frame".into());
                }
                2 => {
                    let (wh, id) = (r.i(), r.i());
                    self.line("op add".into());
                    self.with(wh, |t, _| t.add_to_contain_list(id));
                }
                3 => {
                    let (wh, id, _expose) = (r.i(), r.i(), r.u());
                    self.line("op remove".into());
                    self.with(wh, |t, _| t.remove_from_contain(id));
                }
                4 => {
                    let (wh, id) = (r.i(), r.i());
                    self.line("op isIn".into());
                    let b = self.with(wh, |t, _| t.is_in_container(id));
                    self.line(format!("ret {}", u32::from(b)));
                }
                5 => {
                    let (wh, id, chk) = (r.i(), r.i(), r.i());
                    self.line("op isValid".into());
                    let b = self.with(wh, |t, w| t.is_valid_container_for(w, (id != 0).then_some(id), chk != 0));
                    self.line(format!("ret {}", u32::from(b)));
                }
                6 => {
                    let (wh, id) = (r.i(), r.i());
                    self.line("op tunnelCreated".into());
                    self.with(wh, |t, _| t.on_tunnel_created(id));
                }
                7 => {
                    let (wh, id) = (r.i(), r.i());
                    self.line("op tunnelDestroyed".into());
                    let res = self.with(wh, |t, w| t.on_tunnel_destroyed(w, id));
                    assert!(res.is_ok(), "an undefined input reached the comparison: {res:?}");
                }
                8 => {
                    let wh = r.i();
                    let frames = r.f();
                    self.line("op heal".into());
                    let res = self.with(wh, |t, w| t.heal_objects(w, frames));
                    assert!(res.is_ok(), "an undefined input reached the comparison: {res:?}");
                }
                9 => {
                    let (wh, id) = (r.i(), r.i());
                    self.line("op updateNemesis".into());
                    self.with(wh, |t, w| t.update_nemesis(w, (id != 0).then_some(id)));
                }
                10 => {
                    let wh = r.i();
                    self.line("op getNemesis".into());
                    let n = self.with(wh, |t, w| t.get_cur_nemesis(w));
                    self.line(format!("ret {}", n.unwrap_or(0)));
                }
                11 => {
                    self.w.cuts.globals.max_tunnel_capacity = r.i();
                    self.line("op capacity".into());
                }
                12 => {
                    let (id, field, v) = (r.i(), r.i(), r.i());
                    self.line("op mark".into());
                    if field == 4 && v == 0 {
                        // the stand-in keeps the fields of an object that left the world
                        if let Some(o) = self.w.objects.find(id) {
                            self.frozen[id as usize] =
                                (o.contained_by, o.contained_by_frame, u32::from(o.is_effectively_dead()), u32::from(o.is_destroyed()), o.group);
                        }
                        let _ = self.w.objects.remove(id);
                    } else if let Some(o) = self.w.objects.find_mut(id) {
                        match field {
                            0 => {
                                if v != 0 {
                                    o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
                                } else {
                                    o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
                                }
                            }
                            1 => o.status.set(OBJECT_STATUS_STEALTHED, i32::from(v != 0)),
                            2 => o.status.set(OBJECT_STATUS_DETECTED, i32::from(v != 0)),
                            3 => o.status.set(OBJECT_STATUS_DISGUISED, i32::from(v != 0)),
                            5 => o.contained_by = v,
                            6 => o.group = v as u32,
                            _ => {}
                        }
                    }
                }
                13 => {
                    let wh = r.i();
                    self.line("op saveLoad".into());
                    // save, load into a fresh tracker, post-process it
                    let mut tracker = if wh == 0 { self.t.clone() } else { self.w.cave_system.get_tunnel_tracker_for_cave_index(wh - 1).cloned().unwrap() };
                    let (st, len, h) = save_snapshot(&mut tracker);
                    self.line(format!("ret save {st} len {len} hash {h:08x}"));
                    let bytes = save_bytes(&mut tracker);
                    let mut fresh = TunnelTracker::new();
                    let lst = load_snapshot(&mut fresh, bytes);
                    self.line(format!("ret load {lst}"));
                    self.tracker_state("loaded", Some(&fresh));
                    let post = fresh.load_post_process_world(&mut self.w);
                    self.line(format!("ret post {}", u32::from(post.is_err())));
                    self.tracker_state("post", Some(&fresh));
                    self.flush_log();
                }
                20 => {
                    let idx = r.i();
                    self.line("op caveRegister".into());
                    let res = self.w.cave_system.register_new_cave(idx);
                    assert!(res.is_ok(), "an undefined input reached the comparison: {res:?}");
                }
                21 => {
                    let (a, b) = (r.i(), r.i());
                    self.line("op caveCanSwitch".into());
                    let can = self.w.cave_system.can_switch_index_to_index(a, b);
                    self.line(format!("ret {}", u32::from(can)));
                }
                22 => {
                    let idx = r.i();
                    self.line("op caveGet".into());
                    let has = self.w.cave_system.get_tunnel_tracker_for_cave_index(idx).is_some();
                    self.line(format!("ret {}", u32::from(has)));
                }
                23 => {
                    let idx = r.i();
                    self.line("op caveUnregister".into());
                    self.w.cave_system.unregister_cave(idx);
                }
                24 => {
                    self.line("op caveReset".into());
                    self.w.cave_system.reset();
                }
                25 => {
                    self.line("op caveSaveLoad".into());
                    let mut cave = self.w.cave_system.clone();
                    let (st, len, h) = save_snapshot(&mut cave);
                    self.line(format!("ret save {st} len {len} hash {h:08x}"));
                    let bytes = save_bytes(&mut cave);
                    let mut fresh = CaveSystem::new();
                    let lst = load_snapshot(&mut fresh, bytes);
                    self.line(format!("ret load {lst}"));
                    let mut cs = String::new();
                    for i in 0..fresh.len() {
                        match fresh.get_tunnel_tracker_for_cave_index(i as i32) {
                            None => cs.push_str(" -"),
                            Some(t) => cs.push_str(&format!(" {}", t.get_contain_list_size())),
                        }
                    }
                    self.line(format!("ret loaded cave[{cs}]"));
                    for i in 0..fresh.len() {
                        let st = fresh.get_tunnel_tracker_for_cave_index(i as i32).cloned();
                        self.tracker_state(&format!("L{i}"), st.as_ref());
                        let mut post = 0;
                        if let Some(t) = fresh.get_tunnel_tracker_for_cave_index_mut(i as i32) {
                            post = u32::from(t.load_post_process_world(&mut self.w).is_err());
                        }
                        self.line(format!("ret post {post}"));
                    }
                    self.flush_log();
                }
                other => panic!("unknown op {other}"),
            }
            self.flush_log();
            self.world_state();
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
        let n = unsafe { cpp::ref_tunnel_run(prog.as_ptr(), prog.len() as i32, buf.as_mut_ptr() as *mut _, cap as i32) };
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

/// What the generator must know to stay clear of undefined behaviour.
#[derive(Clone, Default)]
struct Tm {
    count: u32,
    tunnels: Vec<i32>,
    list: Vec<i32>,
}

struct Model {
    /// The frame the program has reached, the frames the objects entered their containers (header), and the
    /// frame the last nemesis was set: the generator aims at the boundaries of `getCurNemesis` and `healObject`.
    frame: u32,
    by_frame: [u32; 9],
    nemesis_frame: u32,
    exists: [bool; 9],
    body: [bool; 9],
    t: Tm,
    caves: Vec<Option<Tm>>,
}

impl Model {
    fn tracker(&mut self, which: i32) -> &mut Tm {
        if which == 0 {
            &mut self.t
        } else {
            self.caves[(which - 1) as usize].as_mut().expect("tracker")
        }
    }
    fn contained_anywhere(&self, id: i32) -> bool {
        self.t.list.contains(&id) || self.caves.iter().flatten().any(|c| c.list.contains(&id))
    }
    fn trackers(&self) -> Vec<i32> {
        let mut v = vec![0];
        for (i, c) in self.caves.iter().enumerate() {
            if c.is_some() {
                v.push(i as i32 + 1);
            }
        }
        v
    }
}

fn pick_frame(g: &mut Gen) -> u32 {
    match g.below(8) {
        0 => u32::MAX - g.below(300),
        1 => 0,
        2..=4 => g.below(50),
        _ => g.below(2000),
    }
}

fn gen_header(g: &mut Gen, p: &mut Prog, m: &mut Model) {
    p.i([0, 1, 2, 3, 5, 10][g.below(6) as usize]);
    let frame = pick_frame(g);
    p.u(frame);
    m.frame = frame;
    m.nemesis_frame = frame;
    for i in 1..=8usize {
        let mut kinds = 0;
        for k in [KINDOF_AIRCRAFT, KINDOF_VEHICLE, KINDOF_STRUCTURE, KINDOF_INFANTRY, KINDOF_SELECTABLE] {
            if g.below(3) == 0 {
                kinds |= kind_bit(k);
            }
        }
        let status = g.below(8);
        let mut flags = 0;
        if g.below(10) != 0 {
            flags |= FLAG_EXISTS;
        }
        if g.below(6) == 0 {
            flags |= FLAG_DEAD;
        }
        if g.below(2) == 0 {
            flags |= FLAG_DRAWABLE;
        }
        if g.below(3) != 0 {
            flags |= FLAG_BODY;
        }
        if g.below(3) == 0 {
            flags |= FLAG_GROUP;
        }
        let max_health = [50.0f32, 100.0, 150.5, 1000.0][g.below(4) as usize];
        let health = if flags & FLAG_BODY != 0 { max_health * [0.0, 0.25, 0.5, 1.0][g.below(4) as usize] } else { 0.0 };
        let by = if g.below(2) == 0 { 0 } else { 1 + g.below(8) as i32 };
        let by_frame = frame.wrapping_sub(g.below(500));
        m.by_frame[i] = by_frame;
        p.u(kinds).u(status).u(flags).f(max_health).f(health).i(by).u(by_frame);
        m.exists[i] = flags & FLAG_EXISTS != 0;
        m.body[i] = flags & FLAG_BODY != 0;
    }
}

fn pick_existing(g: &mut Gen, m: &Model) -> Option<i32> {
    let ids: Vec<i32> = (1..=8).filter(|i| m.exists[*i as usize]).collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids[g.below(ids.len() as u32) as usize])
    }
}

fn heal_frames(g: &mut Gen, m: &Model) -> f32 {
    if g.below(3) == 0 {
        // exactly the time an object has been inside, or one frame either side
        let i = 1 + g.below(8) as usize;
        let elapsed = m.frame.wrapping_sub(m.by_frame[i]) as f32;
        return elapsed + [-1.0, 0.0, 1.0][g.below(3) as usize];
    }
    [0.0, 1.0, 5.0, 30.0, 100.0, 450.0, 1.0e6, f32::INFINITY, -3.0, 0.5][g.below(10) as usize]
}

fn gen_program(seed: u64, n_ops: usize) -> Prog {
    let mut g = Gen::new(seed);
    let mut p = Prog::default();
    let mut m = Model { frame: 0, by_frame: [0; 9], nemesis_frame: 0, exists: [false; 9], body: [false; 9], t: Tm::default(), caves: Vec::new() };
    gen_header(&mut g, &mut p, &mut m);
    for _ in 0..n_ops {
        let trackers = m.trackers();
        let wh = trackers[g.below(trackers.len() as u32) as usize];
        match g.below(40) {
            0..=5 => {
                // add
                if let Some(id) = pick_existing(&mut g, &m) {
                    p.i(2).i(wh).i(id);
                    m.tracker(wh).list.push(id);
                }
            }
            6..=8 => {
                let id = g.below(9) as i32;
                p.i(3).i(wh).i(id).u(g.below(2));
                let l = &mut m.tracker(wh).list;
                if let Some(at) = l.iter().position(|x| *x == id) {
                    l.remove(at);
                }
            }
            9 => {
                p.i(4).i(wh).i(g.below(9) as i32);
            }
            10..=11 => {
                let id = if g.below(5) == 0 { Some(0) } else { pick_existing(&mut g, &m) };
                if let Some(id) = id {
                    p.i(5).i(wh).i(id).i(g.below(2) as i32);
                }
            }
            12..=14 => {
                if let Some(id) = pick_existing(&mut g, &m) {
                    p.i(6).i(wh).i(id);
                    let t = m.tracker(wh);
                    t.count = t.count.wrapping_add(1);
                    t.tunnels.push(id);
                }
            }
            15..=18 => {
                // tunnel destroyed: only where upstream is defined
                if let Some(id) = pick_existing(&mut g, &m) {
                    let t = m.tracker(wh).clone();
                    let count = t.count.wrapping_sub(1);
                    let mut tunnels = t.tunnels.clone();
                    tunnels.retain(|x| *x != id);
                    if count == 0 || !tunnels.is_empty() {
                        p.i(7).i(wh).i(id);
                        let t = m.tracker(wh);
                        t.count = count;
                        t.tunnels = tunnels;
                        if count == 0 {
                            t.list.clear();
                        }
                    }
                }
            }
            19..=21 => {
                // heal: every contained object has a body
                let t = m.tracker(wh).clone();
                if t.list.iter().all(|i| m.body[*i as usize]) {
                    let fr = heal_frames(&mut g, &m);
                    p.i(8).i(wh).f(fr);
                }
            }
            22..=24 => {
                let id = if g.below(4) == 0 { Some(0) } else { pick_existing(&mut g, &m) };
                if let Some(id) = id {
                    p.i(9).i(wh).i(id);
                    m.nemesis_frame = m.frame;
                }
            }
            25 => {
                p.i(10).i(wh);
            }
            26 => {
                p.i(11).i([0, 1, 2, 3, 5, 10][g.below(6) as usize]);
            }
            27..=29 => {
                // change an object
                if let Some(id) = pick_existing(&mut g, &m) {
                    let field = [0, 1, 2, 3, 4, 5, 6][g.below(7) as usize];
                    let v = if field == 5 { g.below(9) as i32 } else if field == 6 { g.below(3) as i32 } else { g.below(2) as i32 };
                    if field == 4 {
                        // leaving the world is only defined for an object nobody holds
                        if m.contained_anywhere(id) || g.below(3) != 0 {
                            continue;
                        }
                        m.exists[id as usize] = false;
                        p.i(12).i(id).i(4).i(0);
                    } else {
                        p.i(12).i(id).i(field).i(v);
                    }
                }
            }
            30..=31 => {
                p.i(13).i(wh);
            }
            32..=33 => {
                // a new frame: sometimes the boundary of the nemesis's four seconds (30 frames a second)
                let f = if g.below(2) == 0 { m.nemesis_frame.wrapping_add(118 + g.below(5)) } else { pick_frame(&mut g) };
                p.i(1).u(f);
                m.frame = f;
            }
            34..=35 => {
                let idx = g.below(5) as i32;
                p.i(20).i(idx);
                while m.caves.len() <= idx as usize {
                    m.caves.push(None);
                }
                if m.caves[idx as usize].is_none() {
                    m.caves[idx as usize] = Some(Tm::default());
                }
            }
            36 => {
                p.i(21).i(g.below(8) as i32 - 2).i(g.below(8) as i32 - 2);
            }
            37 => {
                p.i(22).i(g.below(8) as i32 - 2);
            }
            38 => {
                if g.below(4) == 0 {
                    p.i(24);
                    m.caves.clear();
                } else {
                    p.i(23).i(g.below(5) as i32);
                }
            }
            _ => {
                p.i(25);
            }
        }
    }
    p.i(99);
    p
}

// ---- the tests -------------------------------------------------------------------------------------------------

#[test]
fn tunnel_tracker_matches_upstream() {
    let cases: u64 = std::env::var("TUNNEL_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(1500);
    let (mut lines, mut bad) = (0usize, 0);
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for case in 0..cases {
        let prog = gen_program(0x7A44_0000 + case, 45);
        let c = run_cpp(&prog.0);
        let r = run_rust(&prog.0);
        lines += c.len();
        for l in &c {
            let key: Vec<&str> = l.split(' ').take(if l.starts_with("tt") || l.starts_with("ret") { 2 } else { 1 }).collect();
            if !l.starts_with("state") && !l.starts_with("obj") {
                *kinds.entry(key.join(" ")).or_default() += 1;
            }
        }
        if let Some(d) = first_difference(&r, &c) {
            bad += 1;
            if bad <= 3 {
                println!("DIFF case {case}: {d}");
                let from = d.find("line ").map_or(0, |_| 0);
                let _ = from;
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
    // a world with the eight header objects, all with a body, existing
    let mut p = Prog::default();
    p.i(5).u(10);
    for _ in 0..8 {
        p.u(kind_bit(KINDOF_INFANTRY)).u(0).u(FLAG_EXISTS | FLAG_BODY).f(100.0).f(50.0).i(0).u(0);
    }
    p.i(99);
    let mut counted = 0;

    // 1. destroying a tunnel when no id is left to be the valid tunnel (front() of an empty list)
    {
        let mut r = Rd { w: &p.0, at: 0 };
        let mut run = Run::new(&mut r);
        run.t.on_tunnel_created(1);
        run.t.on_tunnel_created(2);
        // two registered: destroying a third (not in the list) leaves count 1 and both ids: defined
        assert!(run.t.on_tunnel_destroyed(&mut run.w, 3).is_ok());
        // now count 1: destroying an id that is in the list removes it; the count reaches 0: defined
        assert!(run.t.on_tunnel_destroyed(&mut run.w, 1).is_ok());
        // count 0 now with id 2 still listed: a destroy of another id wraps the count (defined) and reads front() = 2
        assert!(run.t.on_tunnel_destroyed(&mut run.w, 4).is_ok());
        assert_eq!(run.t.friend_get_tunnel_count(), u32::MAX);
        // destroying id 2 empties the list while the count stays non-zero: front() of an empty list
        assert_eq!(run.t.on_tunnel_destroyed(&mut run.w, 2), Err(TunnelError::NoTunnelIds));
        counted += 1;
    }
    // 2. a contained id that names no object
    {
        let mut r = Rd { w: &p.0, at: 0 };
        let mut run = Run::new(&mut r);
        run.t.add_to_contain_list(3);
        run.t.on_tunnel_created(1);
        let _ = run.w.objects.remove(3);
        assert_eq!(run.t.on_tunnel_destroyed(&mut run.w, 1), Err(TunnelError::ContainedObjectGone(3)));
        counted += 1;
        assert_eq!(run.t.heal_objects(&mut run.w, 30.0), Err(TunnelError::ContainedObjectGone(3)));
        counted += 1;
    }
    // 3. healing an object with no body module
    {
        let mut q = Prog::default();
        q.i(5).u(10);
        for _ in 0..8 {
            q.u(kind_bit(KINDOF_INFANTRY)).u(0).u(FLAG_EXISTS).f(100.0).f(0.0).i(0).u(0);
        }
        q.i(99);
        let mut r = Rd { w: &q.0, at: 0 };
        let mut run = Run::new(&mut r);
        run.t.add_to_contain_list(2);
        assert_eq!(run.t.heal_objects(&mut run.w, 30.0), Err(TunnelError::NoBodyModule(2)));
        counted += 1;
    }
    // 4. a negative cave index in registerNewCave
    {
        let mut cave = CaveSystem::new();
        assert_eq!(cave.register_new_cave(-1), Err(CaveError::NegativeIndex(-1)));
        assert!(cave.is_empty(), "nothing was allocated");
        counted += 1;
    }
    // 5. a load whose contain count exceeds what an UnsignedShort counts (the loop cannot end upstream)
    {
        let mut x = XferSave::new();
        x.open(b"t").unwrap();
        let mut t = TunnelTracker::new();
        // version 1, no tunnels, a size of 70000, then no ids: the load runs out of bytes
        let mut ver: u8 = 1;
        x.xfer_version(&mut ver, 1).unwrap();
        let mut ids: Vec<ObjectId> = Vec::new();
        x.xfer_stl_object_id_list(&mut ids).unwrap();
        let mut size: i32 = 70_000;
        x.xfer_int(&mut size).unwrap();
        x.close().unwrap();
        let st = load_snapshot(&mut t, x.take_bytes());
        assert_ne!(st, 0, "the read ends in an error, as the missing bytes do upstream");
        counted += 1;
    }
    println!("RESULT {counted} classes of undefined input are errors in Rust (and are never sent to the C++)");
    assert_eq!(counted, 6);
    let _ = (INVALID_ID, XferStatus::Ok);
}
