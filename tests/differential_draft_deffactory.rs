#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWSaveLoad/definitionfactorymgr.h, definitionfactorymgr.cpp, definitionfactory.h,
//   definitionfactory.cpp, definitionclassids.h (SuperClassID_From_ClassID)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::wwlib::definitionfactorymgr` and `definitionfactory` against the upstream C++ (#215, group wwlib3).
//!
//! # Method
//!
//! `differential/wrap_draft_deffactory.cpp` includes the five upstream files unchanged (copied by build.rs; the stand-in is
//! `wwdebug.h`, WWASSERT compiled out). Eight slots hold test factories (a class id and a name); a program of 1 to 50 random ops:
//! create a factory in a slot (the constructor registers it: `Link_Factory`, at the FRONT of the list), destroy one (the destructor
//! unregisters it), `Find_Factory(class_id)`, `Find_Factory(name)` (`stricmp`), `Get_First(superclass_id)`, `Get_Next(factory,
//! superclass_id)`, `Get_First()`, `Get_Next(factory)`. After every op the whole list from `Get_First()` through `Get_Next()` is
//! output, as slot numbers. Class ids are drawn from a pool that includes duplicates (two factories of one class id are legal
//! upstream; `Find_Factory` returns the newest) and boundary values of `SuperClassID_From_ClassID` (0, 0xfff, 0x1000, 0xffff,
//! 0x7fffffff, 0xffffffff). 20,000 programs from a fixed seed.
//!
//! # Predictions and results
//!
//! Predicted before the first run: a mismatch with duplicate class ids, because the draft identifies a registered factory by its class
//! id (`Unregister`, `Get_Next`), upstream by its address; and an integer-overflow panic in `super_class_id_from_class_id` for class ids
//! near `0x7fffffff` (release builds here have overflow checks on; upstream wraps).

mod support;

use rts_lockstep_engine::drafts::wwlib::definition::{super_class_id_from_class_id, DefinitionHandle};
use rts_lockstep_engine::drafts::wwlib::definitionfactory::DefinitionFactoryClass;
use rts_lockstep_engine::drafts::wwlib::definitionfactorymgr::DefinitionFactoryMgrClass;
use support::{Check, Gen};

extern "C" {
    fn ref_df_run(prog: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
}

const NF: usize = 8;

const IDS: [u32; 14] = [0, 0xfff, 0x1000, 0x1001, 0x1fff, 0x2000, 0x2345, 0x3000, 0x5abc, 0xd000, 0xffff, 0x10000, 0x7fff_ffff, 0xffff_ffff];
const NAMES: [&str; 6] = ["Tile", "tile", "TILE", "Light", "light", "Wave"];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_DF_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn never_created() -> DefinitionHandle {
    unreachable!("Create is not part of this test")
}

struct Prog {
    words: Vec<i32>,
    nops: i32,
}

fn gen_prog(g: &mut Gen) -> Prog {
    let nops = 1 + g.below(50) as i32;
    let mut w = Vec::new();
    for _ in 0..nops {
        match g.below(8) {
            0 | 1 => {
                let name = NAMES[g.below(NAMES.len() as u32) as usize];
                w.extend([0, g.below(NF as u32) as i32, IDS[g.below(IDS.len() as u32) as usize] as i32, name.len() as i32]);
                w.extend(name.bytes().map(|b| b as i32));
            }
            2 => w.extend([1, g.below(NF as u32) as i32]),
            3 => w.extend([2, IDS[g.below(IDS.len() as u32) as usize] as i32]),
            4 => {
                let name = NAMES[g.below(NAMES.len() as u32) as usize];
                w.extend([3, name.len() as i32]);
                w.extend(name.bytes().map(|b| b as i32));
            }
            5 => w.extend([4, super_ids(g) as i32]),
            6 => w.extend([5, g.below(NF as u32) as i32, super_ids(g) as i32]),
            _ => {
                if g.below(2) == 0 {
                    w.push(6)
                } else {
                    w.extend([7, g.below(NF as u32) as i32])
                }
            }
        }
    }
    Prog { words: w, nops }
}

/// A super-class id to look for: the start of a range (what the factories' classes map to) or an off-range value.
fn super_ids(g: &mut Gen) -> u32 {
    match g.below(4) {
        0 => IDS[g.below(IDS.len() as u32) as usize],
        _ => 0x1000 + 0x1000 * g.below(6),
    }
}

#[derive(Clone)]
struct Slot {
    class_id: u32,
    name: &'static str,
    handle: DefinitionFactoryClass,
}

fn run_rust(p: &Prog) -> Vec<i32> {
    let mut slots: Vec<Option<Slot>> = (0..NF).map(|_| None).collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut next = || {
        let v = p.words[i];
        i += 1;
        v
    };
    // the slot a returned factory belongs to: the registered entry that is the same factory (same class id and name and the same
    // creation) as the slot's; a lookup returns a copy, so identity is by (class id, name, instance) via `same_as`
    let slot_of = |slots: &Vec<Option<Slot>>, f: &Option<DefinitionFactoryClass>| -> i32 {
        match f {
            None => -1,
            Some(f) => slots.iter().position(|s| s.as_ref().map_or(false, |s| (s.handle.class_id == f.class_id && s.handle.name == f.name))).map_or(-2, |x| x as i32),
        }
    };
    for _ in 0..p.nops {
        let op = next();
        match op {
            0 => {
                let slot = next() as usize;
                let id = next() as u32;
                let nl = next();
                let name: String = (0..nl).map(|_| next() as u8 as char).collect();
                if slots[slot].is_none() {
                    let name: &'static str = NAMES.iter().copied().find(|n| *n == name).expect("a pool name");
                    let handle = DefinitionFactoryClass::new(id, name, true, never_created);
                    handle.register();
                    slots[slot] = Some(Slot { class_id: id, name, handle });
                }
            }
            1 => {
                let slot = next() as usize;
                if let Some(s) = slots[slot].take() {
                    s.handle.unregister();
                }
            }
            2 => {
                let id = next() as u32;
                let f = DefinitionFactoryMgrClass::find_factory(id);
                out.push(slot_of(&slots, &f));
            }
            3 => {
                let nl = next();
                let name: String = (0..nl).map(|_| next() as u8 as char).collect();
                let f = DefinitionFactoryMgrClass::find_factory_by_name(&name);
                out.push(slot_of(&slots, &f));
            }
            4 => {
                let sc = next() as u32;
                let f = DefinitionFactoryMgrClass::get_first_in_superclass(sc);
                out.push(slot_of(&slots, &f));
            }
            5 => {
                let slot = next() as usize;
                let sc = next() as u32;
                match &slots[slot] {
                    Some(s) => {
                        let f = DefinitionFactoryMgrClass::get_next_in_superclass(&s.handle, sc);
                        out.push(slot_of(&slots, &f));
                    }
                    None => out.push(-3),
                }
            }
            6 => {
                let f = DefinitionFactoryMgrClass::get_first();
                out.push(slot_of(&slots, &f));
            }
            7 => {
                let slot = next() as usize;
                match &slots[slot] {
                    Some(s) => {
                        let f = DefinitionFactoryMgrClass::get_next(&s.handle);
                        out.push(slot_of(&slots, &f));
                    }
                    None => out.push(-3),
                }
            }
            _ => unreachable!(),
        }
        let mut cur = DefinitionFactoryMgrClass::get_first();
        while let Some(f) = cur {
            out.push(slot_of(&slots, &Some(f)));
            cur = DefinitionFactoryMgrClass::get_next(&f);
        }
        out.push(-9);
    }
    for s in slots.iter_mut() {
        if let Some(s) = s.take() {
            s.handle.unregister();
        }
    }
    let _ = (|s: &Slot| (s.class_id, s.name),);
    out
}

fn run_cpp(p: &Prog) -> Vec<i32> {
    let mut out = vec![0i32; 8192];
    let n = unsafe { ref_df_run(p.words.as_ptr(), p.nops, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn definition_factories_match_upstream_over_random_programs() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_17401);
    let mut c = Check::new("DefinitionFactoryMgrClass programs");
    for i in 0..n {
        let p = gen_prog(&mut g);
        let cpp = run_cpp(&p);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&p))).ok();
        c.eq(rs, Some(cpp), || format!("program {i}: {:?}", p.words));
        // a panic in the middle of a program leaves factories registered: clear the registry for the next one
        while let Some(f) = DefinitionFactoryMgrClass::get_first() {
            f.unregister();
        }
    }
    // the super-class id function alone, over the boundary ids
    for id in IDS {
        let cpp = {
            let delta = (id as i32).wrapping_sub(0x1000);
            let ranges = delta.wrapping_div(0x1000);
            (0x1000i32.wrapping_add(ranges.wrapping_mul(0x1000))) as u32
        };
        let rs = std::panic::catch_unwind(|| super_class_id_from_class_id(id)).ok();
        c.eq(rs, Some(cpp), || format!("SuperClassID_From_ClassID({id:#x})"));
    }
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
