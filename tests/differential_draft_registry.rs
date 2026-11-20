#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/registry.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::registry` against `registry.cpp` (#215, group lc2).
//!
//! # Method
//!
//! `registry.cpp` (all of its functions) is cut verbatim by build.rs into `differential/wrap_draft_registry.cpp`, four times in four namespaces
//! (so `GetRegistryLanguage`'s function-level statics can be exercised four times from fresh), over a fake Windows registry (what each stand-in
//! call returns is listed there). The draft's [`RegistryHost`] is the same fake on the Rust side; so what is tested is the logic upstream wraps
//! around the five calls: the two key prefixes, the local-machine-then-current-user order, `val` changed only on success, the defaults
//! (`"GeneralsMPTest"`, 65536 twice) and the cached language.
//!
//! * **Programs** (20,000): a random registry (0 to 14 entries over both roots, the Generals and Zero Hour keys with and without a sub-key and one
//!   foreign key; string values of length 0, 3, 7, 254, 255 and 256, so a value that fits the 256-byte buffer exactly and one that is a byte too big
//!   both occur; DWORD values and 3-character strings for the unsigned reads, which fit the 4-byte buffer exactly; ERROR_MORE_DATA for longer ones)
//!   and 1 to 20 operations among the ten public functions, with a random write-deny mask. Each answer (the Bool, and the string's length and byte
//!   sum or the unsigned value, which keeps its sentinel on failure) is compared.
//! * **Language cache** (4 scenarios, one per namespace, each three calls with the registry changed after the first).
//!
//! **Excluded:** a DWORD read as a string and a string shorter than 3 characters read as an unsigned value (both read an unfilled buffer upstream).
//! The Windows calls themselves are the host's.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches** (the prediction held, no draft change): 20,000 programs and the 4 language-cache scenarios. Planted bugs, 11 of 11 caught (programs / cache):
//! 16901 Generals reader uses the Zero Hour key 4,649; 16902 current user tried first 380; 16903 no current-user fallback 2,936 (+1); 16904 the unsigned fallback re-reads local machine 4,309;
//! 16905 the language is never cached 0 / 3; 16906 the cached language not updated 0 / 3; 16907 wrong SKU default 7,476; 16908 version default 65535 7,496; 16909 map-pack version reads
//! `Version` 6,217; 16910 `val` cleared on failure 17,773 (+1); 16911 the unsigned read returns true on failure 15,979.

mod support;

use rts_lockstep_engine::drafts::logic_common::registry::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc2_reg_run(deny: i32, entries: *const i32, nentries: i32, blob: *const u8, ops: *const i32, nops: i32, out: *mut i32) -> i32;
    fn ref_lc2_reg_lang(scenario: i32, entries: *const i32, nentries: i32, after: *const i32, nafter: i32, blob: *const u8, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

const GEN: &str = "SOFTWARE\\Electronic Arts\\EA Games\\Generals";
const ZH: &str = "SOFTWARE\\Electronic Arts\\EA Games\\Command and Conquer Generals Zero Hour";
const STR_KEYS: [&str; 4] = ["Language", "SKU", "InstallPath", "Other"];
const U32_KEYS: [&str; 3] = ["Version", "MapPackVersion", "Count"];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish_all(checks: Vec<Check>) {
    if planted() {
        for c in checks {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    } else {
        for c in checks {
            c.finish();
        }
    }
}

#[derive(Clone)]
struct Entry {
    root: i32,
    path: Vec<u8>,
    key: Vec<u8>,
    dword: bool,
    bytes: Vec<u8>,
}

struct Fake {
    reg: Vec<Entry>,
    deny: i32,
}

impl Fake {
    fn put(&mut self, e: Entry) {
        for x in self.reg.iter_mut() {
            if x.root == e.root && x.path == e.path && x.key == e.key {
                *x = e;
                return;
            }
        }
        self.reg.push(e);
    }
    fn find(&self, root: RegistryRoot, path: &[u8], key: &[u8]) -> Option<&Entry> {
        let r = root_id(root);
        self.reg.iter().find(|e| e.root == r && e.path == path && e.key == key)
    }
}

fn root_id(r: RegistryRoot) -> i32 {
    match r {
        RegistryRoot::LocalMachine => 1,
        RegistryRoot::CurrentUser => 2,
    }
}

impl RegistryHost for Fake {
    fn query_string(&mut self, root: RegistryRoot, path: &[u8], key: &[u8]) -> Option<Vec<u8>> {
        let e = self.find(root, path, key)?;
        if e.bytes.len() > 256 {
            return None;
        }
        let end = e.bytes.iter().position(|&b| b == 0).unwrap_or(e.bytes.len());
        Some(e.bytes[..end].to_vec())
    }
    fn query_u32(&mut self, root: RegistryRoot, path: &[u8], key: &[u8]) -> Option<u32> {
        let e = self.find(root, path, key)?;
        if e.bytes.len() > 4 {
            return None;
        }
        let mut b = [0u8; 4];
        b[..e.bytes.len()].copy_from_slice(&e.bytes);
        Some(u32::from_le_bytes(b))
    }
    fn set_string(&mut self, root: RegistryRoot, path: &[u8], key: &[u8], val: &[u8]) -> bool {
        if (self.deny >> root_id(root)) & 1 != 0 {
            return false;
        }
        let mut bytes = val.to_vec();
        bytes.push(0);
        self.put(Entry { root: root_id(root), path: path.to_vec(), key: key.to_vec(), dword: false, bytes });
        true
    }
    fn set_u32(&mut self, root: RegistryRoot, path: &[u8], key: &[u8], val: u32) -> bool {
        if (self.deny >> root_id(root)) & 1 != 0 {
            return false;
        }
        self.put(Entry { root: root_id(root), path: path.to_vec(), key: key.to_vec(), dword: true, bytes: val.to_le_bytes().to_vec() });
        true
    }
}

fn sig(v: &[u8]) -> [i32; 2] {
    let mut sum: u64 = 0;
    for (i, &b) in v.iter().enumerate() {
        sum = sum.wrapping_add(b as u64 * (i as u64 + 1));
    }
    [v.len() as i32, (sum & 0x7fff_ffff) as i32]
}

/// A NUL-terminated string pool: returns the offset of `s`.
struct Blob(Vec<u8>);

impl Blob {
    fn add(&mut self, s: &[u8]) -> i32 {
        let o = self.0.len() as i32;
        self.0.extend_from_slice(s);
        self.0.push(0);
        o
    }
}

fn raw_path(id: usize) -> Vec<u8> {
    match id {
        0 => GEN.as_bytes().to_vec(),
        1 => format!("{GEN}\\Sub").into_bytes(),
        2 => ZH.as_bytes().to_vec(),
        3 => format!("{ZH}\\Sub").into_bytes(),
        _ => b"SOFTWARE\\Other".to_vec(),
    }
}

fn string_value(g: &mut Gen) -> Vec<u8> {
    let len = [0usize, 3, 7, 254, 255, 256][g.below(6) as usize];
    let mut v: Vec<u8> = b"english".iter().cycle().take(len).copied().collect();
    if len == 3 {
        v = b"abc".to_vec();
    }
    if g.below(3) == 0 && !v.is_empty() {
        v[0] = b'Z';
    }
    v
}

#[test]
fn registry_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_4E6);
    let mut c = Check::new("registry.cpp");
    let mut cl = Check::new("GetRegistryLanguage cache");
    for case in 0..20_000 {
        let mut blob = Blob(Vec::new());
        let mut host = Fake { reg: Vec::new(), deny: 0 };
        let mut entries: Vec<i32> = Vec::new();
        for _ in 0..g.below(15) {
            let root = 1 + g.below(2) as i32;
            let rp = if g.below(4) != 0 { [0usize, 2][g.below(2) as usize] } else { g.below(5) as usize };
            let is_u32 = g.below(2) == 0;
            let key = if is_u32 { U32_KEYS[if g.below(4) != 0 { g.below(2) } else { 2 } as usize] } else { STR_KEYS[if g.below(4) != 0 { g.below(2) } else { g.below(4) } as usize] };
            let path = raw_path(rp);
            let (dword, bytes, vnum_or_off): (bool, Vec<u8>, i32) = if is_u32 {
                if g.below(5) == 0 {
                    // a 3-character string: 4 bytes with its NUL, fits the 4-byte buffer exactly
                    let s: Vec<u8> = vec![b'a' + g.below(26) as u8, b'a' + g.below(26) as u8, b'a' + g.below(26) as u8];
                    let off = blob.add(&s);
                    let mut b = s;
                    b.push(0);
                    (false, b, off)
                } else if g.below(8) == 0 {
                    let s = b"longer".to_vec();
                    let off = blob.add(&s);
                    let mut b = s;
                    b.push(0);
                    (false, b, off)
                } else {
                    let v = [0u32, 65536, 0xFFFF_FFFF, 1, g.u32()][g.below(5) as usize];
                    (true, v.to_le_bytes().to_vec(), v as i32)
                }
            } else {
                let s = string_value(&mut g);
                let off = blob.add(&s);
                let mut b = s;
                b.push(0);
                (false, b, off)
            };
            let po = blob.add(&path);
            let ko = blob.add(key.as_bytes());
            entries.extend([root, po, ko, dword as i32, vnum_or_off]);
            host.put(Entry { root, path, key: key.as_bytes().to_vec(), dword, bytes });
        }
        host.deny = [0, 2, 4, 6][g.below(4) as usize];
        // operations
        let mut ops: Vec<i32> = Vec::new();
        let mut rust_ops: Vec<(i32, i32, Vec<u8>, Vec<u8>, Vec<u8>, i32)> = Vec::new();
        let empty = blob.add(b"");
        for _ in 0..1 + g.below(20) {
            let kind = g.below(10) as i32;
            let root = 1 + g.below(2) as i32;
            let suffix: &[u8] = if g.below(3) == 0 { b"\\Sub" } else { b"" };
            let strkey = STR_KEYS[g.below(4) as usize].as_bytes();
            let u32key = U32_KEYS[g.below(3) as usize].as_bytes();
            let rawp = raw_path(g.below(5) as usize);
            let (path, key, val, num): (Vec<u8>, Vec<u8>, Vec<u8>, i32) = match kind {
                0 | 1 => (suffix.to_vec(), strkey.to_vec(), vec![], 0),
                2 => (suffix.to_vec(), u32key.to_vec(), vec![], 0),
                3..=5 => (vec![], vec![], vec![], 0),
                6 => (rawp, strkey.to_vec(), vec![], 0),
                7 => (rawp, u32key.to_vec(), vec![], 0),
                8 => (rawp, strkey.to_vec(), string_value(&mut g), 0),
                _ => (rawp, u32key.to_vec(), vec![], g.i32()),
            };
            let po = blob.add(&path);
            let ko = blob.add(&key);
            let vo = if kind == 8 { blob.add(&val) } else { num };
            let _ = empty;
            ops.extend([kind, root, po, ko, vo, 0]);
            rust_ops.push((kind, root, path, key, val, num));
        }
        let mut cpp = vec![0i32; 3 * rust_ops.len() + 3];
        let len = unsafe {
            ref_lc2_reg_run(host.deny, entries.as_ptr(), (entries.len() / 5) as i32, blob.0.as_ptr(), ops.as_ptr(), rust_ops.len() as i32, cpp.as_mut_ptr())
        } as usize;
        cpp.truncate(len);

        let mut rust: Vec<i32> = Vec::new();
        for (kind, root, path, key, val, num) in &rust_ops {
            let r = if *root == 1 { RegistryRoot::LocalMachine } else { RegistryRoot::CurrentUser };
            let mut s: Vec<u8> = b"init".to_vec();
            let mut u: u32 = 0xDEAD_BEEF;
            let ok = match kind {
                0 => get_string_from_generals_registry(&mut host, path, key, &mut s),
                1 => get_string_from_zero_hour_registry(&mut host, path, key, &mut s),
                2 => get_unsigned_int_from_zero_hour_registry(&mut host, path, key, &mut u),
                3 => {
                    s = get_registry_game_name(&mut host);
                    true
                }
                4 => {
                    u = get_registry_version(&mut host);
                    true
                }
                5 => {
                    u = get_registry_map_pack_version(&mut host);
                    true
                }
                6 => get_string_from_registry(&mut host, r, path, key, &mut s),
                7 => get_unsigned_int_from_registry(&mut host, r, path, key, &mut u),
                8 => set_string_in_registry(&mut host, r, path, key, val),
                _ => set_unsigned_int_in_registry(&mut host, r, path, key, *num as u32),
            };
            rust.push(ok as i32);
            if matches!(kind, 2 | 4 | 5 | 7) {
                rust.extend([u as i32, 0]);
            } else {
                rust.extend(sig(&s));
            }
        }
        c.eq(rust, cpp, || format!("case {case}: entries {} deny {}, ops {:?}", entries.len() / 5, host.deny, rust_ops.iter().map(|o| (o.0, o.1)).collect::<Vec<_>>()));
    }

    // the language cache: scenario, first registry, registry afterwards
    let long255: Vec<u8> = std::iter::repeat(b'L').take(255).collect();
    let scenarios: Vec<(Vec<(i32, Vec<u8>)>, Vec<(i32, Vec<u8>)>)> = vec![
        (vec![(1, b"french".to_vec()), (2, b"other".to_vec())], vec![(1, b"german".to_vec())]),
        (vec![(2, b"spanish".to_vec())], vec![(1, b"german".to_vec())]),
        (vec![], vec![(1, b"german".to_vec())]),
        (vec![(1, long255)], vec![]),
    ];
    for (k, (first, after)) in scenarios.iter().enumerate() {
        let mut blob = Blob(Vec::new());
        let enc = |list: &Vec<(i32, Vec<u8>)>, blob: &mut Blob| -> Vec<i32> { list.iter().flat_map(|(r, v)| [*r, blob.add(v), 0]).collect() };
        let e1 = enc(first, &mut blob);
        let e2 = enc(after, &mut blob);
        let mut cpp = [0i32; 6];
        let len = unsafe { ref_lc2_reg_lang(k as i32, e1.as_ptr(), first.len() as i32, e2.as_ptr(), after.len() as i32, blob.0.as_ptr(), cpp.as_mut_ptr()) };
        assert_eq!(len, 6);
        let mk = |list: &Vec<(i32, Vec<u8>)>| {
            let mut f = Fake { reg: Vec::new(), deny: 0 };
            for (r, v) in list {
                let mut b = v.clone();
                b.push(0);
                f.put(Entry { root: *r, path: ZH.as_bytes().to_vec(), key: b"Language".to_vec(), dword: false, bytes: b });
            }
            f
        };
        let mut cache = RegistryLanguageCache::new();
        let mut host = mk(first);
        let a = cache.get_registry_language(&mut host);
        let mut host = mk(after);
        let b = cache.get_registry_language(&mut host);
        let d = cache.get_registry_language(&mut host);
        let rust: Vec<i32> = [a, b, d].iter().flat_map(|v| sig(v)).collect();
        cl.eq(rust, cpp.to_vec(), || format!("scenario {k}"));
    }
    finish_all(vec![c, cl]);
}
