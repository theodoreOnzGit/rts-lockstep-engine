// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `logic::object_types` (`ObjectTypes`) against
//! upstream `GameLogic/Object/ObjectTypes.cpp` (commit 0a05454d), compiled
//! unchanged in `differential/wrap_object_types.cpp` (see build.rs).
//! Phase 1 layer 8, p1w6-trackers.
//!
//! # Methodology
//!
//! A **program** is a random sequence of operations on one list, run on both
//! sides from an empty list. Operations: `addObjectType`, `removeObjectType`,
//! `setListName`, `isInSet` (by name and by template, null included),
//! `getListSize`, `getNthInList` (in and out of range, negative included),
//! `prepForPlayerCounting` (with random initial templates and counts),
//! `canBuildAny` (with a random buildable mask), `XferSave` (the bytes),
//! `XferLoad` of a blob (the status and the list afterwards) and a dump of the
//! list. Names come from a shared table of ten names: five the factory knows,
//! two it does not, the empty name and a lower-case name.
//!
//! The blob a program loads is one of: random bytes; a valid save of a random
//! list; that save with one byte flipped; or that save truncated. All four
//! reach `ObjectTypes::xfer`'s load branch, including its refusal of a
//! non-empty list and the end-of-file paths.
//!
//! Compared, word for word: the log of every program (every return value, the
//! save bytes, the load status and the list afterwards). A program is one
//! comparison.
//!
//! **Planted bug.** `prepForPlayerCounting` that counts only the names it adds
//! (not the whole vector, as upstream returns) runs on the same programs and
//! must differ.
//!
//! # Results
//!
//! Recorded by `cargo test --release --features differential --test
//! differential_object_types -- --nocapture` (the `DIFF` and `PLANTED` lines).

#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::object_types::ObjectTypes;
use std::ffi::CString;
use support::{Check, Gen};

/// Programs per run.
const PROGRAMS: usize = 3000;
/// The shared name table (`NAMES` in wrap_object_types.cpp, same order).
const NAMES: [&str; 10] = [
    "ALPHA", "BETA", "GAMMA", "DELTA", "EPSILON", "ZETA", "ETA", "THETA", "", "alpha",
];
/// The names the factory knows: indices below this.
const KNOWN: usize = 6;

/// `findTemplate`: the template index of a known name.
fn find(name: &AsciiString) -> Option<usize> {
    NAMES
        .iter()
        .position(|n| n.as_bytes() == name.str())
        .filter(|&i| i < KNOWN)
}

fn name(i: usize) -> AsciiString {
    AsciiString::try_from_bytes(NAMES[i].as_bytes()).unwrap()
}

/// `XferSave` of a list: the bytes, as upstream's `xferSnapshot` writes them to a file.
fn save_bytes(ot: &ObjectTypes) -> Vec<u8> {
    let mut save = XferSave::new();
    save.open(b"object_types").unwrap();
    ot.clone().xfer(&mut save).unwrap();
    save.close().unwrap();
    save.take_bytes()
}

/// `XferLoad` of `blob` into a list: the transfer's result.
fn load_into(ot: &mut ObjectTypes, blob: &[u8]) -> Result<(), XferStatus> {
    let mut load = XferLoad::new(blob.to_vec());
    load.open(b"object_types").unwrap();
    let r = ot.xfer(&mut load);
    load.close().unwrap();
    r
}

/// A state dump, as the C++ `dump` writes it.
fn dump(out: &mut Vec<u32>, ot: &ObjectTypes) {
    push_ascii(out, ot.list_name());
    out.push(ot.list_size() as u32);
    for k in 0..ot.list_size() {
        push_ascii(out, &ot.get_nth_in_list(k as i32));
    }
}

fn push_ascii(out: &mut Vec<u32>, s: &AsciiString) {
    out.push(s.get_length() as u32);
    out.extend(s.str().iter().map(|&b| u32::from(b)));
}

/// The Rust side of one program: the same log words as `ref_object_types_run`.
/// `planted` selects the deliberately wrong `prepForPlayerCounting`.
fn rust_run(ops: &[[i32; 3]], blob: &[u8], planted: bool) -> Vec<u32> {
    let mut ot = ObjectTypes::new();
    let mut out = Vec::new();
    for &[code, a, b] in ops {
        match code {
            0 => ot.add_object_type(&name(a as usize)),
            1 => {
                let was = ot.is_in_set(&name(a as usize));
                ot.remove_object_type(&name(a as usize));
                out.push(u32::from(was));
            }
            2 => ot.set_list_name(name(a as usize)),
            3 => out.push(u32::from(ot.is_in_set(&name(a as usize)))),
            4 => {
                let t = usize::try_from(a).ok().map(name);
                out.push(u32::from(ot.is_in_set_of_template(t.as_ref())));
            }
            5 => out.push(ot.list_size() as u32),
            6 => push_ascii(&mut out, &ot.get_nth_in_list(a)),
            7 => {
                let mut templates: Vec<usize> = (0..a as usize).collect();
                let mut counts: Vec<i32> = (0..b as usize).map(|k| 5 + k as i32).collect();
                let ret = if planted {
                    planted_prep(&ot, &mut templates, &mut counts)
                } else {
                    ot.prep_for_player_counting(find, &mut templates, &mut counts)
                };
                out.push(ret as u32);
                out.push(templates.len() as u32);
                out.extend(templates.iter().map(|&t| t as u32));
                out.push(counts.len() as u32);
                out.extend(counts.iter().map(|&c| c as u32));
            }
            8 => {
                let mask = a as u32;
                out.push(u32::from(ot.can_build_any(find, |t| (mask >> t) & 1 == 1)));
            }
            9 => {
                let bytes = save_bytes(&ot);
                out.push(bytes.len() as u32);
                out.extend(bytes.iter().map(|&x| u32::from(x)));
            }
            10 => {
                let status = match load_into(&mut ot, blob) {
                    Ok(()) => 0,
                    Err(XferStatus::ListNotEmpty) => 1,
                    Err(s) => 2 + s as u32,
                };
                out.push(status);
                dump(&mut out, &ot);
            }
            _ => dump(&mut out, &ot),
        }
    }
    out
}

/// The planted variant: counts only the names it adds, not the vector's
/// whole length (upstream returns `templates.size()`).
fn planted_prep(ot: &ObjectTypes, templates: &mut Vec<usize>, counts: &mut Vec<i32>) -> i32 {
    let before = templates.len();
    ot.prep_for_player_counting(find, templates, counts);
    let added = (templates.len() - before) as i32;
    counts.resize(added as usize, 0);
    added
}

/// Serialises the C++ side: the wrapper keeps its stand-ins (the buildable mask, the
/// template table) in process globals, and the two tests of this file run in parallel
/// threads, so an unserialised run could read the other program's state.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The C++ side of one program.
fn cpp_run(ops: &[[i32; 3]], blob: &[u8], path: &std::path::Path) -> Vec<u32> {
    let flat: Vec<i32> = ops.iter().flat_map(|o| o.iter().copied()).collect();
    let cpath = CString::new(path.to_string_lossy().as_bytes()).expect("path");
    let _serial = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = vec![0u32; 1 << 16];
    let n = unsafe {
        cpp::ref_object_types_run(
            flat.as_ptr(),
            ops.len() as i32,
            blob.as_ptr(),
            blob.len() as i32,
            cpath.as_ptr(),
            out.as_mut_ptr(),
            out.len() as i32,
        )
    };
    assert!(n >= 0, "ref_object_types_run returned {n}");
    out.truncate(n as usize);
    out
}

/// A random valid save of a random list, as this crate writes it.
fn random_list_save(g: &mut Gen) -> Vec<u8> {
    let mut ot = ObjectTypes::with_list_name(name(g.below(10) as usize));
    for _ in 0..g.below(6) {
        ot.add_object_type(&name(g.below(10) as usize));
    }
    save_bytes(&ot)
}

/// The blob a program loads: random bytes, a valid save, that save with one
/// byte flipped, or that save truncated.
fn random_blob(g: &mut Gen) -> Vec<u8> {
    match g.below(4) {
        0 => (0..g.below(40)).map(|_| g.u32() as u8).collect(),
        1 => random_list_save(g),
        2 => {
            let mut b = random_list_save(g);
            if !b.is_empty() {
                let i = g.below(b.len() as u32) as usize;
                b[i] ^= 1 << g.below(8);
            }
            b
        }
        _ => {
            let b = random_list_save(g);
            let keep = g.below(b.len() as u32 + 1) as usize;
            b[..keep].to_vec()
        }
    }
}

/// One random program: operations with their operands, and the blob it loads.
fn random_program(g: &mut Gen) -> (Vec<[i32; 3]>, Vec<u8>) {
    let blob = random_blob(g);
    let n_ops = 1 + g.below(40);
    let mut ops = Vec::with_capacity(n_ops as usize);
    for _ in 0..n_ops {
        let op = match g.below(30) {
            0..=6 => [0, g.below(10) as i32, 0],
            7 | 8 => [1, g.below(10) as i32, 0],
            9 => [2, g.below(10) as i32, 0],
            10..=12 => [3, g.below(10) as i32, 0],
            13 => [4, g.below(11) as i32 - 1, 0],
            14 => [5, 0, 0],
            15 | 16 => [6, g.below(14) as i32 - 2, 0],
            17..=19 => [7, g.below(7) as i32, g.below(5) as i32],
            20 | 21 => [8, g.below(1024) as i32, 0],
            22 | 23 => [9, 0, 0],
            24 => [10, 0, 0],
            _ => [11, 0, 0],
        };
        ops.push(op);
    }
    (ops, blob)
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "p1w6-object-types-{}-{tag}.bin",
        std::process::id()
    ))
}

#[test]
fn object_types_match_upstream_word_for_word() {
    let path = temp_path("main");
    let mut g = Gen::new(0x0601);
    let mut check = Check::new("ObjectTypes (programs, saves, loads)");
    let mut ops_total = 0usize;
    let mut loads = 0usize;
    for _ in 0..PROGRAMS {
        let (ops, blob) = random_program(&mut g);
        ops_total += ops.len();
        loads += ops.iter().filter(|o| o[0] == 10).count();
        let rust = rust_run(&ops, &blob, false);
        let cpp = cpp_run(&ops, &blob, &path);
        check.eq(rust, cpp, || format!("program {ops:?} blob {blob:?}"));
    }
    let _ = std::fs::remove_file(&path);
    println!("ObjectTypes: {ops_total} operations, {loads} loads, {PROGRAMS} programs");
    check.finish();
}

#[test]
fn planted_prep_bug_is_caught() {
    let path = temp_path("planted");
    let mut g = Gen::new(0x0601);
    let mut check = Check::new("ObjectTypes prep (planted: counts only additions)");
    for _ in 0..PROGRAMS {
        let (ops, blob) = random_program(&mut g);
        let rust = rust_run(&ops, &blob, true);
        let cpp = cpp_run(&ops, &blob, &path);
        check.eq(rust, cpp, || format!("program {ops:?} blob {blob:?}"));
    }
    let _ = std::fs::remove_file(&path);
    check.finish_planted();
}
