// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::name_key_generator` against
//! upstream `GameEngine/Source/Common/NameKeyGenerator.cpp` and
//! `Include/Common/NameKeyGenerator.h` (commit 0a05454d), compiled unchanged
//! (`differential/wrap_namekey.cpp`).
//!
//! Method: 3,000 random scripts of up to 300 operations each (`nameToKey`,
//! `nameToLowercaseKey`, `keyToName`, `reset`, `init`, a fresh uninitialised
//! generator, `StaticNameKey::key`) run on both sides; the logs of keys and
//! names must be identical byte for byte. Names come from a small pool with
//! case variants (so that the two lookups meet in the same socket and chains
//! hold several case-insensitively equal names), random ASCII names
//! (including the empty name), names with bytes from 0x80 (only to
//! `nameToKey`; for `nameToLowercaseKey` upstream is undefined, so the
//! script is not sent and the Rust must return an error), and names that
//! collide in a socket.
//!
//! Planted bugs, each of which must be caught: chains searched oldest first,
//! `nameToLowercaseKey` storing the lower-cased name, and the hash over
//! unsigned bytes.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::name_key_generator::*;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// Both C++ entry points install their generator as the process-global
/// `TheNameKeyGenerator` while they run, so the tests take turns.
static GLOBAL_GENERATOR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug)]
enum Op {
    Key(Vec<u8>),
    Lower(Vec<u8>),
    Name(u32),
    Reset,
    Init,
    Fresh,
    Static(u8, Vec<u8>),
}

fn encode(ops: &[Op]) -> Vec<u8> {
    let mut s = Vec::new();
    let name = |s: &mut Vec<u8>, n: &[u8]| {
        s.extend_from_slice(&(n.len() as u16).to_le_bytes());
        s.extend_from_slice(n);
    };
    for op in ops {
        match op {
            Op::Key(n) => {
                s.push(0);
                name(&mut s, n);
            }
            Op::Lower(n) => {
                s.push(1);
                name(&mut s, n);
            }
            Op::Name(k) => {
                s.push(2);
                s.extend_from_slice(&k.to_le_bytes());
            }
            Op::Reset => s.push(3),
            Op::Init => s.push(4),
            Op::Fresh => s.push(5),
            Op::Static(slot, n) => {
                s.push(6);
                s.push(*slot);
                name(&mut s, n);
            }
        }
    }
    s
}

/// The Rust run: the same log format as the wrapper, or None where upstream
/// is undefined (the Rust returned an error).
fn run_rust(ops: &[Op]) -> Option<Vec<u8>> {
    let mut g = NameKeyGenerator::new();
    let mut statics: Vec<Option<StaticNameKey>> = (0..256).map(|_| None).collect();
    let mut log = Vec::new();
    for op in ops {
        match op {
            Op::Key(n) => log.extend_from_slice(&g.try_name_to_key(n).ok()?.0.to_le_bytes()),
            Op::Lower(n) => {
                log.extend_from_slice(&g.try_name_to_lowercase_key(n).ok()?.0.to_le_bytes())
            }
            Op::Name(k) => {
                let n = g.key_to_name(NameKeyType(*k));
                log.extend_from_slice(&(n.len() as u32).to_le_bytes());
                log.extend_from_slice(n);
            }
            Op::Reset => g.reset(),
            Op::Init => g.init(),
            Op::Fresh => g = NameKeyGenerator::new(),
            Op::Static(slot, n) => {
                let s = statics[*slot as usize].get_or_insert_with(|| {
                    StaticNameKey::new(Box::leak(n.clone().into_boxed_slice()))
                });
                log.extend_from_slice(&s.key(&mut g).0.to_le_bytes());
            }
        }
    }
    Some(log)
}

/// A deliberately wrong generator for the planted bugs.
#[derive(Clone, Copy, PartialEq)]
enum Bug {
    OldestFirst,
    StoreLowercase,
    UnsignedHash,
}

fn run_planted(ops: &[Op], bug: Bug) -> Vec<u8> {
    let mut sockets: Vec<Vec<(u32, Vec<u8>)>> = vec![Vec::new(); SOCKET_COUNT];
    let mut next = 0u32;
    let mut statics: Vec<Option<(Vec<u8>, u32)>> = vec![None; 256];
    let mut log = Vec::new();
    let hash = |n: &[u8], lower: bool| -> usize {
        let mut r = 0u32;
        for &b in n {
            let b = if lower { b.to_ascii_lowercase() } else { b };
            let v = if bug == Bug::UnsignedHash {
                u32::from(b)
            } else {
                b as i8 as i32 as u32
            };
            r = (r << 5).wrapping_add(r).wrapping_add(v);
        }
        (r % SOCKET_COUNT as u32) as usize
    };
    let lookup =
        |sockets: &mut Vec<Vec<(u32, Vec<u8>)>>, next: &mut u32, n: &[u8], lower: bool| -> u32 {
            let s = hash(n, lower);
            let eq = |a: &[u8]| {
                if lower {
                    a.eq_ignore_ascii_case(n)
                } else {
                    a == n
                }
            };
            let found = if bug == Bug::OldestFirst {
                sockets[s].iter().find(|b| eq(&b.1)).map(|b| b.0)
            } else {
                sockets[s].iter().rev().find(|b| eq(&b.1)).map(|b| b.0)
            };
            if let Some(k) = found {
                return k;
            }
            let k = *next;
            *next += 1;
            let stored = if lower && bug == Bug::StoreLowercase {
                n.to_ascii_lowercase()
            } else {
                n.to_vec()
            };
            sockets[s].push((k, stored));
            k
        };
    for op in ops {
        match op {
            Op::Key(n) => {
                log.extend_from_slice(&lookup(&mut sockets, &mut next, n, false).to_le_bytes())
            }
            Op::Lower(n) => {
                log.extend_from_slice(&lookup(&mut sockets, &mut next, n, true).to_le_bytes())
            }
            Op::Name(k) => {
                let n = sockets
                    .iter()
                    .find_map(|s| s.iter().rev().find(|b| b.0 == *k))
                    .map(|b| b.1.clone())
                    .unwrap_or_default();
                log.extend_from_slice(&(n.len() as u32).to_le_bytes());
                log.extend_from_slice(&n);
            }
            Op::Reset | Op::Init => {
                sockets.iter_mut().for_each(Vec::clear);
                next = 1;
            }
            Op::Fresh => {
                sockets.iter_mut().for_each(Vec::clear);
                next = 0;
            }
            Op::Static(slot, n) => {
                let e = statics[*slot as usize].get_or_insert_with(|| (n.clone(), 0));
                if e.1 == 0 {
                    let name = e.0.clone();
                    e.1 = lookup(&mut sockets, &mut next, &name, false);
                }
                log.extend_from_slice(&e.1.to_le_bytes());
            }
        }
    }
    log
}

fn run_cpp(ops: &[Op]) -> Vec<u8> {
    let script = encode(ops);
    let mut log = vec![0u8; 16 * 1024 * 1024];
    let n = unsafe {
        cpp::ref_namekey_run(
            script.as_ptr(),
            script.len() as i32,
            log.as_mut_ptr(),
            log.len() as i32,
        )
    };
    assert!(n >= 0, "C++ log overflow");
    log.truncate(n as usize);
    log
}

/// Names that share a socket under the plain hash (found by search).
fn colliding_names(g: &mut Gen) -> Vec<Vec<u8>> {
    let mut by_socket: std::collections::BTreeMap<u32, Vec<Vec<u8>>> =
        std::collections::BTreeMap::new();
    let mut out = Vec::new();
    for _ in 0..200_000 {
        let len = 3 + g.below(6) as usize;
        let n: Vec<u8> = (0..len).map(|_| b'a' + g.below(26) as u8).collect();
        let s = calc_hash_for_string(&n) % SOCKET_COUNT as u32;
        let e = by_socket.entry(s).or_default();
        if !e.contains(&n) {
            e.push(n);
        }
    }
    for v in by_socket.values().filter(|v| v.len() >= 2) {
        out.extend(v.iter().cloned());
        if out.len() > 200 {
            break;
        }
    }
    out
}

fn case_variant(g: &mut Gen, n: &[u8]) -> Vec<u8> {
    n.iter()
        .map(|&b| {
            if g.below(2) == 0 {
                b.to_ascii_uppercase()
            } else {
                b.to_ascii_lowercase()
            }
        })
        .collect()
}

#[test]
fn name_key_generator_matches_cpp() {
    let _turn = GLOBAL_GENERATOR_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(201);
    let collide = colliding_names(&mut g);
    assert!(
        collide.len() >= 50,
        "found {} colliding names",
        collide.len()
    );
    let pool: Vec<&[u8]> = vec![
        b"Object",
        b"ModuleTag_01",
        b"AmericaTankCrusader",
        b"foo",
        b"Bar",
        b"",
        b"KindOf",
        b"weapon",
    ];
    let mut ok = Check::new("NameKeyGenerator script logs");
    let mut planted = [
        (
            Bug::OldestFirst,
            Check::new("NameKeyGenerator (chains oldest first)"),
        ),
        (
            Bug::StoreLowercase,
            Check::new("NameKeyGenerator (stores lower case)"),
        ),
        (
            Bug::UnsignedHash,
            Check::new("NameKeyGenerator (unsigned-byte hash)"),
        ),
    ];
    let mut undefined = 0u32;
    let mut ops_total = 0usize;
    for script in 0..3000 {
        let mut ops = Vec::new();
        if g.below(4) != 0 {
            ops.push(Op::Init);
        }
        let n_ops = g.below(300) as usize;
        for _ in 0..n_ops {
            let name = match g.below(10) {
                0..=3 => {
                    let p = pool[g.below(pool.len() as u32) as usize];
                    case_variant(&mut g, p)
                }
                4 | 5 => collide[g.below(collide.len() as u32) as usize].clone(),
                6 => {
                    let mut n = collide[g.below(collide.len() as u32) as usize].clone();
                    n = case_variant(&mut g, &n);
                    n
                }
                7 => (0..g.below(12))
                    .map(|_| 0x20 + g.below(0x5f) as u8)
                    .collect(),
                8 => (0..1 + g.below(6))
                    .map(|_| 0x80 + g.below(0x80) as u8)
                    .collect(),
                _ => {
                    let mut n = case_variant(&mut g, b"MixedName");
                    n.push(0xC0 + g.below(0x3f) as u8);
                    n
                }
            };
            let op = match g.below(100) {
                0..=39 => Op::Key(name),
                // One script in ten may send a byte >= 0x80 to nameToLowercaseKey.
                40..=74 => Op::Lower(if script % 10 == 0 && g.below(4) == 0 {
                    name
                } else {
                    name.iter().map(|&b| b & 0x7f).collect()
                }),
                75..=89 => Op::Name(g.below(40)),
                90 => Op::Reset,
                91 => Op::Init,
                92 => Op::Fresh,
                _ => Op::Static(g.below(8) as u8, name),
            };
            ops.push(op);
        }
        ops_total += ops.len();
        let rust = match run_rust(&ops) {
            Some(l) => l,
            None => {
                // Upstream is undefined: a nameToLowercaseKey of a byte >= 0x80.
                assert!(
                    ops.iter()
                        .any(|o| matches!(o, Op::Lower(n) if n.iter().any(|&b| b >= 0x80))),
                    "script {script}"
                );
                undefined += 1;
                continue;
            }
        };
        let c = run_cpp(&ops);
        ok.eq(&rust, &c, || format!("script {script} ({} ops)", ops.len()));
        for (bug, check) in planted.iter_mut() {
            check.eq(run_planted(&ops, *bug), c.clone(), || {
                format!("script {script}")
            });
        }
    }
    println!("     {ops_total} operations; {undefined} scripts left out (nameToLowercaseKey of a byte >= 0x80, undefined upstream; Rust returned Err)");
    ok.finish();
    for (_, check) in planted {
        check.finish_planted();
    }
}

/// `WellKnownKeys.h`: the Rust table against the header's own `DEFINE_KEY`s
/// (instantiated in `differential/wrap_namekey.cpp`). Each entry's name (the
/// header's `#NAME`) and its key, looked up in order on a fresh generator,
/// must match; the number of `DEFINE_KEY(` lines in the header must equal
/// the table's length. Planted bug: the table with two entries swapped.
#[test]
fn well_known_keys_match_cpp() {
    let _turn = GLOBAL_GENERATOR_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    use rts_lockstep_engine::common::well_known_keys::*;
    let header = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/upstream/GeneralsMD/Code/GameEngine/Include/Common/WellKnownKeys.h"
    ))
    .expect("read WellKnownKeys.h");
    let defined = header
        .lines()
        .filter(|l| l.trim_start().starts_with("DEFINE_KEY("))
        .count();
    let n = unsafe { cpp::ref_well_known_key_count() } as usize;
    let mut ok = Check::new("WellKnownKeys names and keys");
    ok.eq(WELL_KNOWN_KEY_NAMES.len(), defined, || {
        "DEFINE_KEY lines in the header".into()
    });
    ok.eq(WELL_KNOWN_KEY_NAMES.len(), n, || "entries".into());
    let keys = WellKnownKeys::new();
    let mut gen = NameKeyGenerator::new();
    gen.init();
    let mut cpp_names = Vec::new();
    for (i, &rust_name) in WELL_KNOWN_KEY_NAMES.iter().enumerate().take(n) {
        let mut buf = [0u8; 64];
        let mut key = 0u32;
        let len = unsafe {
            cpp::ref_well_known_key_name(i as i32, buf.as_mut_ptr().cast(), 64, &mut key)
        };
        assert!(len >= 0);
        let name = String::from_utf8_lossy(&buf[..len as usize]).into_owned();
        let k = keys.key(i, &mut gen);
        ok.eq(rust_name, name.as_str(), || format!("entry {i}"));
        ok.eq(k.0, key, || format!("key of entry {i}"));
        ok.eq(gen.key_to_name(k), name.as_bytes(), || {
            format!("keyToName of entry {i}")
        });
        cpp_names.push(name);
    }
    ok.finish();
    let mut bug = Check::new("WellKnownKeys names and keys");
    let mut wrong = WELL_KNOWN_KEY_NAMES.to_vec();
    wrong.swap(10, 11);
    for (i, (w, c)) in wrong.iter().zip(cpp_names.iter()).enumerate() {
        bug.eq(*w, c.as_str(), || format!("entry {i}"));
    }
    bug.finish_planted();
}
