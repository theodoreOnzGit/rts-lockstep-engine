#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590), GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/:
//   saveload.h, saveload.cpp, saveloadsubsystem.h, saveloadsubsystem.cpp, persist.h, postloadable.h, persistfactory.h, persistfactory.cpp,
//   definition.h, definition.cpp, definitionmgr.h, definitionmgr.cpp, wwsaveload.h, wwsaveload.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The WWSaveLoad drafts (`saveload`, `saveloadsubsystem`, `persistfactory`, `definition`, `definitionmgr`, `wwsaveload`) against the upstream
//! C++ (#215, group wwlib4).
//!
//! # Method
//!
//! `differential/wrap_draft_saveload.cpp` compiles the upstream files (edits and stand-ins listed there) with five concrete definition types
//! `TDef<0..4>` (class ids 0x3005, 0x3006, 0x5001, 0x0800, 0xE000 = `CLASSID_TWIDDLERS`; persist-factory chunk ids 0x7001, 0x7001, 0x7002, 0x7003,
//! 0x7004: two types share a chunk id). The Rust side builds the same five types from `DefinitionKind::Generic`. A case is a program of 1 to 60
//! operations on 8 definition slots, 6 extra persist-factory slots and the global registries: create a definition (type, id, name, save-enabled),
//! `Register_Definition`, `Unregister_Definition`, `Set_ID` (re-links a registered one), delete, `Find_Definition`, `Find_Named_Definition`,
//! `Find_Typed_Definition` (with and without twiddling), the `Get_First`/`Get_Next` chains (all, by class, by super-class), `Get_New_ID`,
//! `List_Available_Definitions` (both), `SaveLoadSystemClass::Save` of the definition manager into a memory file (every byte compared),
//! `Load` of the kept image (each loaded object asks for a pointer remap of its own id, so `Register_Pointer` and the remap are exercised),
//! `Free_Definitions` and `WWSaveLoad::Shutdown`, creating and destroying persist factories (also several for one chunk id),
//! `Find_Persist_Factory`, `Register_Post_Load_Callback`, `Post_Load_Processing` with and without a network callback and a scripted clock
//! (differences of 19, 20, 21 and across the 32-bit wrap), and unregistering and re-registering the manager as a sub-system. Every op's result and
//! the full state it can see are output as numbers and compared.
//!
//! A `Load` is skipped when an id in the image is already in the manager: upstream appends the loaded definitions and `qsort`s the array, and the
//! order of equal ids is the C library's (glibc's, here, keeps them in order; the draft's `sort_unstable_by` does not).
//!
//! **Not covered:** `Twiddle()` (the C++ stand-in returns the definition itself, as the draft does for a definition that is not a twiddler;
//! twiddler.rs is its own unit); the order in which `On_Post_Load` runs (the draft's `On_Post_Load` has no body to observe); `SaveLoadStatus`
//! (display only); `Create`; two sub-systems (the draft has one); registering a factory or sub-system twice and unregistering one that is not
//! linked (upstream asserts / runs off the list); `Save_Objects` when a definition's factory is not registered (upstream calls the factory the
//! type itself owns through `Get_Factory()`; the draft looks it up by chunk id and writes an empty chunk), so every type's own factory stays
//! registered for the whole program; a definition deleted while the manager still holds it, or after it was registered (the manager's name
//! cache keeps a pointer to a definition that was registered, even after `Unregister_Definition`: an upstream hazard, not a draft difference).
//!
//! # Predictions and results
//!
//! Predicted before the runs: (1) `Unregister_Persist_Factory` removes the first factory with the chunk id, upstream the one asked for: a mismatch when an
//! older factory of a shared chunk id is destroyed while a newer one exists (the instance id was added to the draft before the first run; plant 18271 is the
//! old behaviour and measures it: 5 programs of 10,000); (2) `Save_Objects` writes an empty chunk if no factory is registered (not tested, see above);
//! (3) otherwise 0 mismatches. The runs also met four things that are upstream's and not the draft's, each excluded or worked around above: `Find_Typed_Definition`
//! reads the capacity of a cached vector (an uninitialised read; the C++ crashed on the first program that hit it, which was the first complete run), the name
//! cache keeps definitions after `Unregister_Definition` (dangling after a delete), `qsort`'s order of equal ids, and (harness) a `std::string`-based `StringClass`
//! cannot be moved by `memcpy` as the real one is. After those: 0 mismatches over 10,000 programs (1.04 million program words; 1,844 loads that loaded objects,
//! 806 pointer remaps that found their object, 7,412 typed lookups that found a definition, 203 post-load runs that fired the network callback, 3,338 `Get_New_ID` that
//! found a hole). Planted bugs (mismatching programs of 10,000):
//! saveload.rs: 18271 `Unregister_Persist_Factory` by chunk id: 5; 18272 `Find_Persist_Factory` returns the oldest: 2,963; 18273 factories linked at the tail: 2,963;
//! 18274 network callback at `>= 20` ms: 34; 18275 the time is not advanced after a callback: 11; 18276 no 32-bit wrap in the elapsed time: 66; 18277 a post-load callback
//! registered twice: 5; 18278 `Save` ends the chunk before the sub-system saves: 7,053; 18279 `Load` ignores `auto_post_load`: 107. saveloadsubsystem.rs: 18280 `Chunk_ID`
//! off by one: 7,883. persistfactory.rs: 18281 the data chunk before the pointer chunk: 2,721; 18282 the object number big-endian: 2,717; 18283 the old number read +1: 684;
//! 18284 `Set_Post_Load_Registered(false)` ignored: 858. definition.rs: 18285 the name saved without its NUL: 2,717; 18286 the loaded name keeps its NUL: 846; 18288 `Set_ID`
//! re-links an unregistered definition: 7,468. definitionmgr.rs: 18291 `Find_Named_Definition` case-sensitive: 641; 18292 `Find_Typed_Definition` ignores the super-class: 1,673;
//! 18293 name cache keyed without lower-casing: 165; 18294 duplicate ids registered: 78; 18295 `Unregister_Definition` leaves the link: 1,246; 18296 `Get_New_ID` accepts
//! adjacent ids: 12; 18297 `Save_Objects` ignores `Is_Save_Enabled`: 299; 18298 `Load_Objects` does not sort: 3; 18299 `Get_First`/`Get_Next` with the type swapped: 921; 18300
//! `Free_Definitions` keeps the name cache: 7,380. wwsaveload.rs: 18301 `Shutdown` does nothing: 1,659. 28 of 28 caught.

mod support;

use rts_lockstep_engine::drafts::wwlib::chunk_save::ChunkSaveClass;
use rts_lockstep_engine::drafts::wwlib::definition::{definition_set_id, DefinitionClass, DefinitionHandle, DefinitionKind};
use rts_lockstep_engine::drafts::wwlib::definitionmgr::{DefinitionMgrClass, IdType};
use rts_lockstep_engine::drafts::wwlib::persistfactory::{simple_persist_factory_load, simple_persist_factory_save, PersistFactoryClass, PersistObject};
use rts_lockstep_engine::drafts::wwlib::saveload::SaveLoadSystemClass;
use rts_lockstep_engine::drafts::wwlib::saveloadsubsystem::SaveLoadSubSystemClass;
use rts_lockstep_engine::drafts::wwlib::wwsaveload;
use rts_lockstep_engine::ww::chunkio::{ChunkLoad, GameFile};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use support::{Check, Gen};

extern "C" {
    fn ref_sl_run(names: *const u8, nnames: i32, prog: *const i32, nwords: i32, out: *mut i32, cap: i32) -> i32;
}

const CLS: [u32; 5] = [0x3005, 0x3006, 0x5001, 0x0800, 0xE000];
const CHK: [u32; 5] = [0x7001, 0x7001, 0x7002, 0x7003, 0x7004];
const CHUNKID_SAVELOAD_DEFMGR: u32 = 0x101;

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_SL_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

thread_local! {
    /// The test's object numbers (the C++ wrapper's `oid`): by the definition's address.
    static OIDS: RefCell<HashMap<usize, u32>> = RefCell::new(HashMap::new());
    static NEXT_OID: RefCell<u32> = RefCell::new(0);
    /// The result of each loaded object's pointer-remap request (the C++ object's `ref`): new oid, 0 for NULL.
    static REFS: RefCell<HashMap<u32, u32>> = RefCell::new(HashMap::new());
    /// How often the interesting things happened in the Rust runs: [saves with objects, loads that loaded objects, loaded objects whose remap hit,
    /// Find_Typed hits, (unused), post-load runs that fired the network callback, Get_New_ID that found a hole, (unused)]
    static STATS: RefCell<[u64; 8]> = RefCell::new([0; 8]);
}

fn stat(i: usize) {
    STATS.with(|s| s.borrow_mut()[i] += 1);
}

fn never_created(_d: &DefinitionClass) -> Option<PersistObject> {
    None
}

fn kind(k: usize) -> DefinitionKind {
    DefinitionKind::Generic { class_id: CLS[k], factory_chunk_id: CHK[k], create: never_created }
}

fn assign_oid(h: &DefinitionHandle) -> u32 {
    let oid = NEXT_OID.with(|n| {
        *n.borrow_mut() += 1;
        *n.borrow()
    });
    OIDS.with(|o| o.borrow_mut().insert(Arc::as_ptr(h) as usize, oid));
    oid
}

fn oid_of(h: &DefinitionHandle) -> u32 {
    OIDS.with(|o| *o.borrow().get(&(Arc::as_ptr(h) as usize)).expect("a definition without an oid"))
}

fn oid_opt(h: &Option<DefinitionHandle>) -> i32 {
    h.as_ref().map_or(0, |h| oid_of(h) as i32)
}

/// `TDef<K>`'s persist factory `Load`: `SimplePersistFactoryClass::Load`, then `Register_Pointer(old, new)`; the object's `Load` asks for a
/// pointer remap of its own id (the slot token is the new oid).
fn load_k<const K: usize>(cload: &mut ChunkLoad<'_>) -> Option<PersistObject> {
    let oid = NEXT_OID.with(|n| {
        *n.borrow_mut() += 1;
        *n.borrow()
    });
    let (old, obj) = simple_persist_factory_load(cload, || DefinitionClass::new(kind(K)), |o, cl| o.load(cl));
    let id = obj.get_id();
    let h: DefinitionHandle = Arc::new(RwLock::new(obj));
    OIDS.with(|o| o.borrow_mut().insert(Arc::as_ptr(&h) as usize, oid));
    SaveLoadSystemClass::register_pointer(old as usize, oid as usize);
    SaveLoadSystemClass::request_pointer_remap(oid as usize, id as usize, "test", 0);
    Some(PersistObject::Definition(h))
}

fn save_any(csave: &mut ChunkSaveClass, obj: &PersistObject) {
    let PersistObject::Definition(d) = obj;
    simple_persist_factory_save(csave, oid_of(d), |cs| d.write().unwrap().save(cs));
}

type LoadFn = fn(&mut ChunkLoad<'_>) -> Option<PersistObject>;

fn load_fn(k: usize) -> LoadFn {
    match k {
        0 => load_k::<0>,
        1 => load_k::<1>,
        2 => load_k::<2>,
        3 => load_k::<3>,
        _ => load_k::<4>,
    }
}

fn names_pool() -> Vec<String> {
    ["Tank", "tank", "TANK", "Soldier", "", "Zed", "Tank2"].iter().map(|s| s.to_string()).collect()
}

const CLASS_POOL: [u32; 12] = [0x3005, 0x3006, 0x5001, 0x0800, 0xE000, 0x3000, 0x5000, 0x1000, 0, 0xFFFF_FFFF, 0x7FFF_FFFF, 0x2000];

fn id_pool(g: &mut Gen) -> u32 {
    match g.below(10) {
        0 => 0,
        1 => [0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF][g.below(3) as usize],
        2 | 3 => {
            // near the start of an id range of Get_New_ID
            let cls = [0x3005u32, 0x5001][g.below(2) as usize];
            let start = cls.wrapping_sub(0x1000).wrapping_mul(10000);
            start.wrapping_add([1, 2, 3, 5, 6, 10000, 10001][g.below(7) as usize])
        }
        _ => 1 + g.below(30),
    }
}

/// Several definitions of one name (three spellings) and different types, registered in a random order of ids, then typed lookups by class and by
/// super-class with the spellings varied: the name cache can answer differently from the linear search only here.
fn gen_typed_scenario(g: &mut Gen) -> Vec<i32> {
    let mut w = Vec::new();
    let n = 2 + g.below(5);
    let mut ids: Vec<u32> = (1..=30).collect();
    for s in 0..n {
        let k = g.below(3) as i32;
        let id = ids.remove(g.below(ids.len() as u32) as usize);
        w.extend([0, s as i32, k, id as i32, g.below(3) as i32, 1]);
        w.extend([1, s as i32]);
    }
    for _ in 0..3 + g.below(5) {
        let class = [0x3005u32, 0x3006, 0x5001, 0x3000, 0x5000, 0x0800][g.below(6) as usize];
        w.extend([7, g.below(3) as i32, class as i32, g.below(2) as i32]);
    }
    w
}

fn gen_prog(g: &mut Gen, nn: usize) -> Vec<i32> {
    let mut w = if g.below(100) < 20 { gen_typed_scenario(g) } else { Vec::new() };
    let tw = |g: &mut Gen| g.below(2) as i32;
    let mut created = 0u32;
    for _ in 0..1 + g.below(60) {
        let s = g.below(8) as i32;
        match g.below(100) {
            0..=11 => {
                // sometimes the id is the number the object will get (its position among the creations): its own id then
                // names a saved object when the image is loaded, so the pointer remap finds something
                created += 1;
                let id = if g.below(10) < 4 { created } else { id_pool(g) };
                w.extend([0, s, g.below(5) as i32, id as i32, g.below(nn as u32) as i32, (g.below(8) != 0) as i32]);
            }
            12..=24 => w.extend([1, s]),
            25..=28 => w.extend([2, s]),
            29..=32 => w.extend([3, s, id_pool(g) as i32]),
            33..=36 => w.extend([4, s]),
            37..=40 => w.extend([5, id_pool(g) as i32, tw(g)]),
            41..=44 => w.extend([6, g.below(nn as u32) as i32, tw(g)]),
            45..=52 => w.extend([7, g.below(nn as u32) as i32, CLASS_POOL[g.below(12) as usize] as i32, tw(g)]),
            53..=55 => w.push(8),
            56..=58 => w.extend([9, CLASS_POOL[g.below(12) as usize] as i32, 1 + g.below(2) as i32]),
            59..=61 => w.extend([10, CLASS_POOL[g.below(12) as usize] as i32]),
            62 | 63 => w.push(11),
            64 | 65 => w.extend([12, CLASS_POOL[g.below(12) as usize] as i32]),
            66..=70 => {
                w.push(13);
                if g.below(10) < 7 {
                    // usually Free_Definitions first (a Load into a manager that still holds the saved ids is skipped)
                    if g.below(10) < 8 {
                        w.extend([15, g.below(2) as i32]);
                    }
                    w.extend([14, g.below(2) as i32]);
                }
            }
            71..=75 => w.extend([14, g.below(2) as i32]),
            76 | 77 => w.extend([15, g.below(2) as i32]),
            78..=81 => w.extend([16, g.below(6) as i32, g.below(5) as i32]),
            82..=83 => w.extend([17, g.below(6) as i32]),
            84..=86 => w.extend([18, s]),
            87 => w.extend([19, s]),
            88..=91 => {
                let m = 1 + g.below(8);
                w.extend([20, g.below(2) as i32, m as i32]);
                let mut t = g.below(100);
                for _ in 0..m {
                    t = match g.below(8) {
                        0 => 0xFFFF_FFF0u32.wrapping_add(g.below(40)),
                        1 => t.wrapping_add(19),
                        2 => t.wrapping_add(20),
                        3 => t.wrapping_add(21),
                        4 => t.wrapping_sub(g.below(50)),
                        _ => t.wrapping_add(g.below(60)),
                    };
                    w.push(t as i32);
                }
            }
            92 | 93 => w.push(21),
            94 | 95 => w.push(22),
            _ => w.extend([23, [0x7001, 0x7002, 0x7003, 0x7004, 0x7099][g.below(5) as usize]]),
        }
    }
    w
}

fn put_names(out: &mut Vec<i32>, l: &[String]) {
    out.push(l.len() as i32);
    for s in l {
        out.push(s.len() as i32);
        out.extend(s.bytes().map(|b| b as i32));
    }
}

fn run_rust(names: &[String], prog: &[i32]) -> Vec<i32> {
    OIDS.with(|o| o.borrow_mut().clear());
    REFS.with(|o| o.borrow_mut().clear());
    NEXT_OID.with(|n| *n.borrow_mut() = 0);
    let mut rnd = |_lo: i32, _hi: i32| -> i32 { 0 };
    // the intrinsic factories, in type order
    let mut intrinsic: Vec<PersistFactoryClass> = Vec::new();
    for k in 0..5 {
        let f = PersistFactoryClass::new(CHK[k], load_fn(k), save_any);
        f.register();
        intrinsic.push(f);
    }
    let mut defs: [Option<DefinitionHandle>; 8] = Default::default();
    let mut ever = [false; 8];
    let mut extra: [Option<(PersistFactoryClass, usize)>; 6] = Default::default();
    let mut image: Vec<u8> = Vec::new();
    let mut image_ids: Vec<u32> = Vec::new();
    let mut sub_reg = true;
    SaveLoadSubSystemClass::DefinitionMgr.register();
    let mut out = Vec::new();
    let mut times: Vec<u32> = Vec::new();
    let mut time_at = 0usize;
    let mut i = 0;
    while i < prog.len() {
        let op = prog[i];
        i += 1;
        let mut next = || {
            let v = prog[i];
            i += 1;
            v
        };
        match op {
            0 => {
                let (s, k, id, a, en) = (next() as usize, next() as usize, next() as u32, next() as usize, next());
                if defs[s].is_some() {
                    out.push(-5);
                    continue;
                }
                let h = DefinitionClass::new_handle(kind(k));
                let oid = assign_oid(&h);
                h.write().unwrap().set_name(&names[a]);
                definition_set_id(&h, id);
                h.write().unwrap().enable_save(en != 0);
                defs[s] = Some(h);
                ever[s] = false;
                out.push(oid as i32);
            }
            1 | 2 => {
                let s = next() as usize;
                match &defs[s] {
                    None => out.push(-6),
                    Some(h) => {
                        if op == 1 {
                            ever[s] = true;
                            DefinitionMgrClass::register_definition(h);
                        } else {
                            DefinitionMgrClass::unregister_definition(h);
                        }
                        out.push(h.read().unwrap().mgr_link());
                    }
                }
            }
            3 => {
                let (s, id) = (next() as usize, next() as u32);
                match &defs[s] {
                    None => out.push(-6),
                    Some(h) => {
                        definition_set_id(h, id);
                        out.push(h.read().unwrap().mgr_link());
                    }
                }
            }
            4 => {
                let s = next() as usize;
                match &defs[s] {
                    None => out.push(-6),
                    Some(h) => {
                        let (link, pl) = {
                            let d = h.read().unwrap();
                            (d.mgr_link(), d.post_loadable.is_post_load_registered())
                        };
                        if ever[s] || link != -1 || pl {
                            out.push(0);
                        } else {
                            defs[s] = None;
                            out.push(1);
                        }
                    }
                }
            }
            5 => {
                let (id, tw) = (next() as u32, next());
                out.push(oid_opt(&DefinitionMgrClass::find_definition(id, tw != 0, &mut rnd)));
            }
            6 => {
                let (a, tw) = (next() as usize, next());
                out.push(oid_opt(&DefinitionMgrClass::find_named_definition(&names[a], tw != 0, &mut rnd)));
            }
            7 => {
                let (a, c, tw) = (next() as usize, next() as u32, next());
                let r = DefinitionMgrClass::find_typed_definition(&names[a], c, tw != 0, &mut rnd);
                if r.is_some() {
                    stat(3);
                }
                out.push(oid_opt(&r));
            }
            8 => {
                let mut v = Vec::new();
                let mut cur = DefinitionMgrClass::get_first();
                while let Some(d) = cur {
                    v.push(d.clone());
                    cur = DefinitionMgrClass::get_next(&d);
                }
                out.push(v.len() as i32);
                for d in &v {
                    let g = d.read().unwrap();
                    out.extend([oid_of(d) as i32, g.get_id() as i32, g.get_class_id() as i32, g.mgr_link()]);
                }
            }
            9 => {
                let (c, t) = (next() as u32, next());
                let ty = if t == 1 { IdType::Class } else { IdType::SuperClass };
                let mut v = Vec::new();
                let mut cur = DefinitionMgrClass::get_first_of(c, ty);
                while let Some(d) = cur {
                    v.push(oid_of(&d) as i32);
                    cur = DefinitionMgrClass::get_next_of(&d, c, ty);
                }
                out.push(v.len() as i32);
                out.extend(v);
            }
            10 => {
                let id = DefinitionMgrClass::get_new_id(next() as u32);
                if id > 1 && id % 10000 != 1 {
                    stat(6);
                }
                out.push(id as i32)
            }
            11 => put_names(&mut out, &DefinitionMgrClass::list_available_definitions()),
            12 => put_names(&mut out, &DefinitionMgrClass::list_available_definitions_in_superclass(next() as u32)),
            13 => {
                let mut cs = ChunkSaveClass::new();
                let ok = SaveLoadSystemClass::save(&mut cs, &SaveLoadSubSystemClass::DefinitionMgr);
                image = cs.file.clone();
                image_ids.clear();
                let mut cur = DefinitionMgrClass::get_first();
                while let Some(d) = cur {
                    if d.read().unwrap().is_save_enabled() {
                        image_ids.push(d.read().unwrap().get_id());
                    }
                    cur = DefinitionMgrClass::get_next(&d);
                }
                if image.len() > 20 {
                    stat(0);
                }
                out.push(ok as i32);
                out.push(image.len() as i32);
                out.extend(image.iter().map(|&b| b as i32));
            }
            14 => {
                let auto = next();
                let bytes = std::mem::take(&mut image);
                let mut clash = false;
                let mut cur = DefinitionMgrClass::get_first();
                while let Some(d) = cur {
                    if image_ids.contains(&d.read().unwrap().get_id()) {
                        clash = true;
                    }
                    cur = DefinitionMgrClass::get_next(&d);
                }
                if clash {
                    out.push(-17);
                    continue;
                }
                let before = count_defs();
                let mut cl = ChunkLoad::new(GameFile::new(&bytes).unwrap());
                let (ok, results) = SaveLoadSystemClass::load(&mut cl, auto != 0);
                for (slot, newp) in results {
                    REFS.with(|r| r.borrow_mut().insert(slot as u32, newp.unwrap_or(0) as u32));
                }
                if count_defs() > before {
                    stat(1);
                }
                out.push(ok as i32);
                out.push(count_defs() - before);
                let mut cur = DefinitionMgrClass::get_first();
                while let Some(d) = cur {
                    let oid = oid_of(&d);
                    let rf = REFS.with(|r| r.borrow().get(&oid).copied().unwrap_or(0));
                    if rf != 0 {
                        stat(2);
                    }
                    out.extend([oid as i32, rf as i32]);
                    cur = DefinitionMgrClass::get_next(&d);
                }
            }
            15 => {
                let via = next();
                let pl = defs.iter().flatten().any(|h| {
                    let d = h.read().unwrap();
                    d.mgr_link() != -1 && d.post_loadable.is_post_load_registered()
                });
                if pl {
                    out.push(-16);
                    continue;
                }
                let reg: Vec<bool> = defs.iter().map(|s| s.as_ref().map_or(false, |h| h.read().unwrap().mgr_link() != -1)).collect();
                if via != 0 {
                    wwsaveload::shutdown();
                } else {
                    DefinitionMgrClass::free_definitions();
                }
                for (s, r) in defs.iter_mut().zip(reg) {
                    if r {
                        *s = None;
                    }
                }
                out.push(-15);
            }
            16 => {
                let (f, k) = (next() as usize, next() as usize);
                if extra[f].is_some() {
                    out.push(-5);
                } else {
                    let fac = PersistFactoryClass::new(CHK[k], load_fn(k), save_any);
                    fac.register();
                    extra[f] = Some((fac, k));
                    out.push(1);
                }
            }
            17 => {
                let f = next() as usize;
                match extra[f].take() {
                    None => out.push(-6),
                    Some((fac, _)) => {
                        fac.unregister();
                        out.push(1);
                    }
                }
            }
            18 => {
                let s = next() as usize;
                match &defs[s] {
                    None => out.push(-6),
                    Some(h) => {
                        SaveLoadSystemClass::register_post_load_callback(&PersistObject::Definition(h.clone()));
                        out.push(h.read().unwrap().post_loadable.is_post_load_registered() as i32);
                    }
                }
            }
            19 => {
                let s = next() as usize;
                match &defs[s] {
                    None => out.push(-6),
                    Some(h) => out.push(SaveLoadSystemClass::is_post_load_callback_registered(&PersistObject::Definition(h.clone())) as i32),
                }
            }
            20 => {
                let (cb, m) = (next(), next());
                times = (0..m).map(|_| next() as u32).collect();
                time_at = 0;
                let mut calls = 0;
                let mut now = || {
                    let v = if times.is_empty() { 0 } else { times[time_at.min(times.len() - 1)] };
                    time_at += 1;
                    v
                };
                let ok = {
                    let mut f = || calls += 1;
                    if cb != 0 {
                        SaveLoadSystemClass::post_load_processing(Some(&mut f), &mut now)
                    } else {
                        SaveLoadSystemClass::post_load_processing(None, &mut now)
                    }
                };
                if calls > 0 {
                    stat(5);
                }
                out.push(ok as i32);
                out.push(calls);
                for s in defs.iter() {
                    out.push(s.as_ref().map_or(-1, |h| h.read().unwrap().post_loadable.is_post_load_registered() as i32));
                }
                times.clear();
            }
            21 => {
                if sub_reg {
                    SaveLoadSubSystemClass::DefinitionMgr.unregister();
                    sub_reg = false;
                }
                out.push(SaveLoadSystemClass::find_sub_system(CHUNKID_SAVELOAD_DEFMGR).is_some() as i32);
            }
            22 => {
                if !sub_reg {
                    SaveLoadSubSystemClass::DefinitionMgr.register();
                    sub_reg = true;
                }
                out.push(SaveLoadSystemClass::find_sub_system(CHUNKID_SAVELOAD_DEFMGR).is_some() as i32);
            }
            _ => {
                let chunk = next() as u32;
                match SaveLoadSystemClass::find_persist_factory(chunk) {
                    None => out.push(-1),
                    Some(f) => {
                        let mut t = -2;
                        for (k, fac) in intrinsic.iter().enumerate() {
                            if fac.instance() == f.instance() {
                                t = k as i32;
                            }
                        }
                        for (n, e) in extra.iter().enumerate() {
                            if let Some((fac, k)) = e {
                                if fac.instance() == f.instance() {
                                    t = *k as i32 + 100 * (1 + n as i32);
                                }
                            }
                        }
                        out.push(t);
                    }
                }
            }
        }
    }
    let _ = (&times, time_at);
    // release everything
    if !sub_reg {
        SaveLoadSubSystemClass::DefinitionMgr.register();
    }
    SaveLoadSystemClass::post_load_processing(None, &mut || 0);
    for e in extra.iter_mut() {
        if let Some((fac, _)) = e.take() {
            fac.unregister();
        }
    }
    DefinitionMgrClass::free_definitions();
    for fac in intrinsic.iter().rev() {
        fac.unregister();
    }
    SaveLoadSubSystemClass::DefinitionMgr.unregister();
    out
}

fn count_defs() -> i32 {
    let mut n = 0;
    let mut cur = DefinitionMgrClass::get_first();
    while let Some(d) = cur {
        n += 1;
        cur = DefinitionMgrClass::get_next(&d);
    }
    n
}

fn run_cpp(names: &[String], prog: &[i32]) -> Vec<i32> {
    let mut nb = Vec::new();
    for n in names {
        nb.extend_from_slice(n.as_bytes());
        nb.push(0);
    }
    let mut out = vec![0i32; 1 << 20];
    let n = unsafe { ref_sl_run(nb.as_ptr(), names.len() as i32, prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn wwsaveload_matches_upstream_over_random_programs() {
    let names = names_pool();
    let n = cases(10_000);
    let mut g = Gen::new(0x215_18264);
    let mut c = Check::new("WWSaveLoad programs (results, state, bytes)");
    let mut ops = 0u64;
    for i in 0..n {
        let prog = gen_prog(&mut g, names.len());
        ops += prog.len() as u64;
        if std::env::var("DRAFT_TRACE").is_ok() {
            eprintln!("case {i}: {:?}", prog);
        }
        let cpp = run_cpp(&names, &prog);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&names, &prog))).ok();
        let same = rs.as_ref() == Some(&cpp);
        c.eq(same, true, || {
            let at = rs.as_ref().map_or(0, |r| r.iter().zip(cpp.iter()).take_while(|(a, b)| a == b).count());
            let rsv = rs.as_ref().map(|r| r.iter().skip(at.saturating_sub(3)).take(8).copied().collect::<Vec<_>>());
            let cv: Vec<i32> = cpp.iter().skip(at.saturating_sub(3)).take(8).copied().collect();
            format!("{} -- first difference at word {at} (lengths {:?} / {}): rust {:?} c++ {:?}", format!("case {i}: program {:?}", prog), rs.as_ref().map(|r| r.len()), cpp.len(), rsv, cv)
        });
        // a panic in the middle leaves registrations behind: clear them
        SaveLoadSubSystemClass::DefinitionMgr.unregister();
        DefinitionMgrClass::free_definitions();
    }
    println!("wwsaveload: {n} programs, {ops} program words; stats {:?}", STATS.with(|s| *s.borrow()));
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
