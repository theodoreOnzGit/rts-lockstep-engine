// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 3's flag sets
//! (`common::bit_flags` and its instantiations) against upstream
//! `BitFlags.h`, `BitFlagsIO.h`, `BitFlags.cpp`, `KindOf.cpp`,
//! `ObjectStatusTypes.cpp`, `DisabledTypes.cpp`, the name lists of
//! `WeaponSet.cpp` and `SpecialPower.cpp`, and the enum headers (commit
//! 0a05454d), compiled as `differential/wrap_flags.cpp` describes.
//!
//! For each of the seven instantiations (`KindOfMaskType`,
//! `ObjectStatusMaskType`, `DisabledMaskType`, `ModelConditionFlags`,
//! `ArmorSetFlags`, `WeaponSetFlags`, `SpecialPowerMaskType`):
//!
//! - the name table and the enumerator values;
//! - every mask operation on 20,000 random mask triples (empty, full, sparse,
//!   dense, equal, subsets), every index `0..NUMBITS` for `test`/`set`;
//! - the name lookups on exact, case-changed, unknown and empty names, and
//!   indices from -3 to `NUMBITS + 2`;
//! - `parse` on 3,000 generated INI lines (names, `+`/`-` edits, `NONE`,
//!   unknown names, mixtures), from random starting masks, comparing the
//!   mask, whether upstream threw, and the description string; and
//!   `parseSingleBitFromINI`;
//! - `xfer`: save bytes (500 masks), load of those and of corrupted files
//!   (unknown names, short files), and the CRC branch (see below).
//!
//! **The CRC branch** hashes `sizeof(this)` bytes of the flags: 4 on the
//! shipped Win32 build (bits 0-31), 8 in the 64-bit C++ reference. The Rust
//! reproduces the 4 bytes (see `common::bit_flags`). The test checks the C++
//! CRC against `XferCRC` over the version byte and bits 0-63, which confirms
//! what the C++ hashes, and the Rust CRC against `XferCRC` over the version
//! byte and bits 0-31.
//!
//! Every comparison also runs a planted bug, which must be caught.
//!
//! The first run of this test found an upstream hazard, not a translation
//! difference: `XferLoad::xferAsciiString` reads into a function-static
//! buffer, so two loads on different threads corrupt each other's names
//! (here: parallel tests). The engine loads on one thread; the tests now
//! take turns (`CPP_GLOBALS_LOCK`). The Rust `XferLoad` has no shared buffer.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::bit_flags::{BitFlagNames, BitFlags, BitIndexOutOfRange};
use rts_lockstep_engine::common::ini::Ini;
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::common::{disabled_types, kind_of, model_state, object_status_types, special_power_mask_type};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::{armor_set, weapon_set_flags};
use std::ffi::{CStr, CString};
use support::{Check, Gen};

type Words = [u64; 4];

/// The C++ side keeps process-global state that the tests would share:
/// `strtok`'s position (INI parsing) and `XferLoad::xferAsciiString`'s
/// static buffer. So the tests take turns.
static CPP_GLOBALS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn random_mask<T: BitFlagNames>(g: &mut Gen) -> BitFlags<T> {
    let n = T::NUM_BITS as u32;
    let mut f = BitFlags::<T>::new();
    match g.below(8) {
        0 => {}
        1 => f.flip(),
        2 => {
            for _ in 0..1 + g.below(3) {
                f.set(g.below(n) as i32, 1);
            }
        }
        3 => {
            f.flip();
            for _ in 0..1 + g.below(3) {
                f.set(g.below(n) as i32, 0);
            }
        }
        _ => f = BitFlags::from_words([g.u64(), g.u64(), g.u64(), g.u64()]),
    }
    f
}

/// A mask related to `a`: equal, a subset, a superset, or unrelated.
fn related_mask<T: BitFlagNames>(g: &mut Gen, a: &BitFlags<T>) -> BitFlags<T> {
    let w = a.words();
    match g.below(5) {
        0 => *a,
        1 => BitFlags::from_words([w[0] & g.u64(), w[1] & g.u64(), w[2] & g.u64(), w[3] & g.u64()]),
        2 => BitFlags::from_words([w[0] | g.u64(), w[1] | g.u64(), w[2] | g.u64(), w[3] | g.u64()]),
        _ => random_mask(g),
    }
}

fn cstr(p: *const std::os::raw::c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
    }
}

/// Name table, operations and lookups for one instantiation.
fn ops_and_names<T: BitFlagNames>(which: i32, tag: &str, g: &mut Gen) {
    // the name table
    let mut ok = Check::new(&format!("{tag} names"));
    let mut bug = Check::new(&format!("{tag} names"));
    let mut cpp_names = Vec::new();
    for i in 0.. {
        match cstr(unsafe { cpp::ref_flags_name_at(which, i) }) {
            Some(n) => cpp_names.push(n),
            None => break,
        }
    }
    ok.eq(T::NAMES.len(), cpp_names.len(), || "count".into());
    let mut swapped: Vec<&str> = T::NAMES.to_vec();
    swapped.swap(0, 1);
    for (i, c) in cpp_names.iter().enumerate() {
        ok.eq(T::NAMES.get(i).copied(), Some(c.as_str()), || format!("name {i}"));
        bug.eq(swapped.get(i).copied(), Some(c.as_str()), || format!("name {i}"));
    }
    ok.finish();
    bug.finish_planted();

    // operations
    let names = [
        "operator==", "operator!=", "testForAny", "testForAll", "testForNone", "count", "any", "flip", "clear",
        "countIntersection", "countInverseIntersection", "anyIntersectionWith", "clear(mask)", "set(mask)",
        "clearAndSet", "testSetAndClear", "size", "test(i)", "set(i, val)",
    ];
    let n = T::NUM_BITS as i32;
    for (code, name) in names.iter().enumerate() {
        let mut ok = Check::new(&format!("{tag} {name}"));
        let mut bug = Check::new(&format!("{tag} {name}"));
        for k in 0..20_000 {
            let a = random_mask::<T>(g);
            let b = related_mask(g, &a);
            let c = related_mask(g, &a);
            let i = if code >= 17 { k % n } else { 0 };
            let val = [0i32, 1, 2, -1][k as usize % 4];
            let cw: Words = if code == 18 { [val as u64, 0, 0, 0] } else { c.words() };
            let mut out: Words = [0; 4];
            let r = unsafe {
                cpp::ref_flags_op(which, code as i32, a.words().as_ptr(), b.words().as_ptr(), cw.as_ptr(), i, out.as_mut_ptr())
            };
            let mut m = a;
            let (good, wrong): ((i32, Words), (i32, Words)) = match code {
                0 => ((i32::from(a == b), [0; 4]), (i32::from(a.count() == b.count()), [0; 4])),
                1 => ((i32::from(a != b), [0; 4]), (i32::from(a.count() != b.count()), [0; 4])),
                2 => ((i32::from(a.test_for_any(&b)), [0; 4]), (i32::from(a.any() && b.any()), [0; 4])),
                3 => ((i32::from(a.test_for_all(&b)), [0; 4]), (i32::from(b.test_for_all(&a)), [0; 4])),
                4 => ((i32::from(a.test_for_none(&b)), [0; 4]), (i32::from(!b.test_for_all(&a)), [0; 4])),
                5 => ((a.count(), [0; 4]), (a.count() - i32::from(a.test(0)), [0; 4])),
                6 => ((i32::from(a.any()), [0; 4]), (i32::from(a.count() > 1), [0; 4])),
                7 => {
                    m.flip();
                    let w = a.words();
                    ((0, m.words()), (0, [!w[0], !w[1], !w[2], !w[3]])) // planted: bits past NUMBITS set
                }
                8 => {
                    m.clear();
                    ((0, m.words()), (0, a.words()))
                }
                9 => ((a.count_intersection(&b), [0; 4]), (a.count_inverse_intersection(&b), [0; 4])),
                10 => ((a.count_inverse_intersection(&b), [0; 4]), (b.count_inverse_intersection(&a), [0; 4])),
                11 => ((i32::from(a.any_intersection_with(&b)), [0; 4]), (i32::from(a.test_for_all(&b) && b.any()), [0; 4])),
                12 => {
                    m.clear_mask(&b);
                    let mut w = b;
                    w.clear_mask(&a);
                    ((0, m.words()), (0, w.words()))
                }
                13 => {
                    m.set_mask(&b);
                    ((0, m.words()), (0, b.words()))
                }
                14 => {
                    m.clear_and_set(&b, &c);
                    let mut w = a;
                    w.set_mask(&c);
                    w.clear_mask(&b); // planted: set before clear
                    ((0, m.words()), (0, w.words()))
                }
                15 => (
                    (i32::from(a.test_set_and_clear(&b, &c)), [0; 4]),
                    (i32::from(a.test_set_and_clear(&c, &b)), [0; 4]),
                ),
                16 => ((a.size(), [0; 4]), (a.size() - 1, [0; 4])),
                17 => ((i32::from(a.test(i)), [0; 4]), (i32::from(a.test((i + 1) % n)), [0; 4])),
                _ => {
                    m.set(i, val);
                    let mut w = a;
                    w.set(i, i32::from(val == 1)); // planted: only 1 counts as true
                    ((0, m.words()), (0, w.words()))
                }
            };
            ok.eq(good, (if code == 7 || code == 8 || (12..=14).contains(&code) || code == 18 { 0 } else { r }, out), || {
                format!("a {:x?} b {:x?} c {:x?} i {i}", a.words(), b.words(), c.words())
            });
            bug.eq(wrong, (if code == 7 || code == 8 || (12..=14).contains(&code) || code == 18 { 0 } else { r }, out), || {
                format!("case {k}")
            });
        }
        ok.finish();
        bug.finish_planted();
    }
    // out-of-range indices: upstream throws std::out_of_range (not sent to C++)
    let a = random_mask::<T>(g);
    for i in [-1, -100, n, n + 5, i32::MIN] {
        assert_eq!(a.try_test(i), Err(BitIndexOutOfRange), "{tag} test({i})");
        let mut b = a;
        assert_eq!(b.try_set(i, 1), Err(BitIndexOutOfRange), "{tag} set({i})");
        assert_eq!(BitFlags::<T>::get_name_from_single_bit(i), None);
    }

    // name lookups
    let mut ok = Check::new(&format!("{tag} name lookups"));
    let mut bug = Check::new(&format!("{tag} name lookups"));
    for k in 0..6_000 {
        let a = random_mask::<T>(g);
        let base = T::NAMES[g.below(T::NAMES.len() as u32) as usize];
        let token: Vec<u8> = match g.below(6) {
            0 => base.as_bytes().to_vec(),
            1 => base.bytes().map(|c| if g.below(2) == 0 { c.to_ascii_lowercase() } else { c }).collect(),
            2 => format!("{base}X").into_bytes(),
            3 => base.as_bytes()[..base.len() - 1].to_vec(),
            4 => Vec::new(),
            _ => b"NOT_A_FLAG_NAME".to_vec(),
        };
        let ctoken = CString::new(token.clone()).expect("no NUL");
        let i = (k % (n + 6)) - 3;
        let mut name = [0 as std::os::raw::c_char; 2048];
        let mut out: Words = [0; 4];
        for code in 0..5 {
            if code == 2 && !(0..n).contains(&i) {
                continue; // getBitNameIfSet tests the bit: std::out_of_range upstream
            }
            let r = unsafe {
                cpp::ref_flags_names(which, code, a.words().as_ptr(), ctoken.as_ptr(), i, out.as_mut_ptr(), name.as_mut_ptr(), 2048)
            };
            assert!(r != -2, "name buffer");
            let cname = if r == 1 { cstr(name.as_ptr()) } else { None };
            let input = || format!("op {code} token {:?} i {i}", String::from_utf8_lossy(&token));
            match code {
                0 => {
                    ok.eq(BitFlags::<T>::get_single_bit_from_name(&token), r, input);
                    // planted: case-sensitive comparison
                    let w = T::NAMES.iter().position(|n| n.as_bytes() == token.as_slice()).map_or(-1, |p| p as i32);
                    bug.eq(w, r, input);
                }
                1 => {
                    ok.eq(BitFlags::<T>::get_name_from_single_bit(i).map(str::to_string), cname.clone(), input);
                    let w = if (0..=n).contains(&i) { T::NAMES.get(i as usize).map(|s| s.to_string()) } else { None };
                    bug.eq(w.or_else(|| (i == n).then(|| T::NAMES[0].to_string())), cname, input); // planted: i == NUMBITS accepted
                }
                2 => {
                    let good = a.try_get_bit_name_if_set(i).expect("in range").map(str::to_string);
                    ok.eq(good, cname.clone(), input);
                    bug.eq(Some(T::NAMES[i as usize].to_string()), cname, input); // planted: ignores the bit
                }
                3 => {
                    let mut m = a;
                    let rr = m.set_bit_by_name(&token);
                    ok.eq((i32::from(rr), m.words()), (r, out), input);
                    let mut w = a;
                    let wr = w.set_bit_by_name(&token);
                    if wr {
                        w.clear(); // planted: setBitByName replaces the mask
                        w.set_bit_by_name(&token);
                    }
                    bug.eq((i32::from(wr), w.words()), (r, out), input);
                }
                _ => {
                    let mut d = Vec::new();
                    a.build_description(&mut d);
                    ok.eq(Some(String::from_utf8(d.clone()).expect("ASCII")), cname.clone(), input);
                    let w = String::from_utf8(d).expect("ASCII").replace(",\n", "\n"); // planted: no commas
                    bug.eq(Some(w), cname, input);
                }
            }
        }
    }
    ok.finish();
    bug.finish_planted();
}

/// A random INI field line for a flags field.
fn random_line<T: BitFlagNames>(g: &mut Gen) -> Vec<u8> {
    let mut line = b"Flags".to_vec();
    if g.below(3) == 0 {
        line.extend_from_slice(b" =");
    }
    let style = g.below(4);
    for t in 0..g.below(6) {
        line.push(if g.below(4) == 0 { b'\t' } else { b' ' });
        let name = T::NAMES[g.below(T::NAMES.len() as u32) as usize];
        let name: String = name.chars().map(|c| if g.below(3) == 0 { c.to_ascii_lowercase() } else { c }).collect();
        let tok = match (style, g.below(12)) {
            (_, 0) => "NONE".to_string(),
            (_, 1) => "none".to_string(),
            (_, 2) => "UNKNOWN_FLAG".to_string(),
            (0, _) => name,
            (1, 3..=6) => format!("+{name}"),
            (1, _) => format!("-{name}"),
            (2, _) if t == 2 => format!("+{name}"),
            _ => {
                if g.below(2) == 0 {
                    name
                } else {
                    format!("-{name}")
                }
            }
        };
        line.extend_from_slice(tok.as_bytes());
    }
    if g.below(4) == 0 {
        line.extend_from_slice(b" ; comment"); // not a comment to strtok: a token
    }
    line
}

fn parse_and_xfer<T: BitFlagNames>(which: i32, tag: &str, g: &mut Gen) {
    let mut ok = Check::new(&format!("{tag} parse"));
    let mut bug = Check::new(&format!("{tag} parse"));
    let mut threw = 0;
    for k in 0..3_000 {
        let start = random_mask::<T>(g);
        let line = random_line::<T>(g);
        let mut cline = CString::new(line.clone()).expect("no NUL").into_bytes_with_nul();
        let mut out: Words = [0; 4];
        let mut desc = [0 as std::os::raw::c_char; 4096];
        let mut index = 0i32;
        let single = i32::from(k % 5 == 4);
        let r = unsafe {
            cpp::ref_flags_parse(
                which,
                cline.as_mut_ptr().cast(),
                start.words().as_ptr(),
                out.as_mut_ptr(),
                single,
                &mut index,
                desc.as_mut_ptr(),
                4096,
            )
        };
        threw += r;
        let cdesc = cstr(desc.as_ptr()).unwrap_or_default();
        let mut ini = Ini::new();
        ini.begin_line(&line);
        let input = || format!("line {:?} from {:x?}", String::from_utf8_lossy(&line), start.words());
        if single == 1 {
            let res = BitFlags::<T>::parse_single_bit_from_ini(&mut ini);
            ok.eq((i32::from(res.is_err()), res.as_ref().copied().unwrap_or(index)), (r, index), input);
            bug.eq((i32::from(res.is_err()), res.map(|v| v + 1).unwrap_or(index)), (r, index), input);
            continue;
        }
        let mut m = start;
        let mut d = Vec::new();
        let res = m.parse(&mut ini, Some(&mut d));
        ok.eq((i32::from(res.is_err()), m.words(), String::from_utf8_lossy(&d).into_owned()), (r, out, cdesc.clone()), input);
        // planted: plain names added to the old value instead of replacing it
        let mut w = start;
        let mut ini = Ini::new();
        ini.begin_line(&line);
        let snapshot = w;
        let wres = w.parse(&mut ini, None);
        if wres.is_ok() && line.split(|&b| b == b' ' || b == b'\t').skip(1).any(|t| !t.is_empty() && t[0] != b'+' && t[0] != b'-' && t != b"=") {
            w.set_mask(&snapshot);
        }
        bug.eq((i32::from(wres.is_err()), w.words(), String::from_utf8_lossy(&d).into_owned()), (r, out, cdesc), input);
    }
    println!("     {tag}: upstream threw on {threw} of 3000 lines");
    ok.finish();
    bug.finish_planted();

    // xfer: save, load, CRC
    let path = std::env::temp_dir().join(format!("rts-lockstep-flags-{}-{which}.bin", std::process::id()));
    let cpath = CString::new(path.to_str().expect("UTF-8 temp path")).expect("no NUL");
    let mut save = Check::new(&format!("{tag} xfer save"));
    let mut save_bug = Check::new(&format!("{tag} xfer save"));
    let mut load = Check::new(&format!("{tag} xfer load"));
    let mut load_bug = Check::new(&format!("{tag} xfer load"));
    let mut crc = Check::new(&format!("{tag} xfer CRC"));
    let mut crc_bug = Check::new(&format!("{tag} xfer CRC"));
    for k in 0..500 {
        let mut m = random_mask::<T>(g);
        let mut out: Words = [0; 4];
        let st = unsafe { cpp::ref_flags_xfer(which, 0, cpath.as_ptr(), m.words().as_ptr(), out.as_mut_ptr()) };
        assert_eq!(st, 0);
        let cbytes = std::fs::read(&path).expect("read C++ save");
        let mut xs = XferSave::new();
        xs.open(b"rust").expect("open");
        m.xfer(&mut xs).expect("save");
        xs.close().expect("close");
        let rbytes = xs.bytes().to_vec();
        save.eq(&rbytes, &cbytes, || format!("mask {:x?}", m.words()));
        let mut planted = rbytes.clone();
        if planted.len() > 5 {
            planted[1] = planted[1].wrapping_add(1); // planted: count off by one
        } else {
            planted.push(0);
        }
        save_bug.eq(&planted, &cbytes, || format!("case {k}"));

        // load: the saved file, or a corrupted one
        let mut file = cbytes.clone();
        match k % 4 {
            1 if file.len() > 6 => {
                let last = file.len() - 1;
                file[last] ^= 0x20; // a name changes case: still valid (stricmp)
            }
            2 if file.len() > 6 => {
                let last = file.len() - 1;
                file[last] = b'#'; // an unknown name
            }
            3 => {
                file.truncate(file.len().saturating_sub(1 + g.below(4) as usize)); // a short file
            }
            _ => {}
        }
        std::fs::write(&path, &file).expect("write load file");
        let mut cout: Words = [0; 4];
        let start = random_mask::<T>(g);
        let cst = unsafe { cpp::ref_flags_xfer(which, 1, cpath.as_ptr(), start.words().as_ptr(), cout.as_mut_ptr()) };
        let mut xl = XferLoad::new(file.clone());
        let mut r = start;
        xl.open(b"rust").expect("open");
        let rst = match r.xfer(&mut xl) {
            Ok(()) => 0,
            Err(e) => e as i32,
        };
        load.eq((rst, r.words()), (cst, cout), || format!("file {file:?}"));
        let mut w = start;
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"rust").expect("open");
        let wst = match w.xfer(&mut xl) {
            Ok(()) => 0,
            Err(e) => e as i32,
        };
        if wst == 0 {
            w.set_mask(&start); // planted: load does not clear first
        }
        load_bug.eq((wst, w.words()), (cst, cout), || format!("case {k}"));

        // CRC
        let mut cc: Words = [0; 4];
        let st = unsafe { cpp::ref_flags_xfer(which, 2, cpath.as_ptr(), m.words().as_ptr(), cc.as_mut_ptr()) };
        assert_eq!(st, 0);
        let mut expect64 = XferCrc::new_without_nan_guard();
        expect64.add_bytes(&[1]);
        expect64.add_bytes(&m.words()[0].to_le_bytes());
        crc.eq(expect64.get_crc(), cc[0] as u32, || format!("64-bit reference: version + bits 0-63 of {:x?}", m.words()));
        let mut rc = XferCrc::new_without_nan_guard();
        m.xfer(&mut rc).expect("crc");
        let mut expect32 = XferCrc::new_without_nan_guard();
        expect32.add_bytes(&[1]);
        expect32.add_bytes(&(m.words()[0] as u32).to_le_bytes());
        crc.eq(rc.get_crc(), expect32.get_crc(), || format!("shipped Win32: version + bits 0-31 of {:x?}", m.words()));
        let mut wrong = XferCrc::new_without_nan_guard();
        wrong.add_bytes(&[1]);
        wrong.add_bytes(&((m.words()[0] >> 32) as u32).to_le_bytes()); // planted: the high half
        crc_bug.eq(wrong.get_crc(), expect32.get_crc(), || format!("case {k}"));
    }
    let _ = std::fs::remove_file(&path);
    save.finish();
    save_bug.finish_planted();
    load.finish();
    load_bug.finish_planted();
    crc.finish();
    crc_bug.finish_planted();
}

fn enum_values(which: i32) -> Vec<i64> {
    let mut out = vec![0i64; 512];
    let n = unsafe { cpp::ref_flags_enum_values(which, out.as_mut_ptr(), 512) };
    out.truncate(n as usize);
    out
}

#[test]
fn enumerators_match() {
    let _turn = CPP_GLOBALS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tables: [(&str, Vec<i64>); 5] = [
        ("KindOfType", kind_of::KIND_OF_ENUMERATORS.iter().map(|e| e.1 as i64).collect()),
        ("ObjectStatusTypes", object_status_types::OBJECT_STATUS_ENUMERATORS.iter().map(|e| e.1 as i64).collect()),
        ("DisabledType", disabled_types::DISABLED_ENUMERATORS.iter().map(|e| e.1 as i64).collect()),
        ("ModelConditionFlagType", model_state::MODEL_CONDITION_ENUMERATORS.iter().map(|e| e.1 as i64).collect()),
        ("ArmorSetType", armor_set::ARMOR_SET_ENUMERATORS.iter().map(|e| e.1 as i64).collect()),
    ];
    for (which, (name, rust)) in tables.into_iter().enumerate() {
        let c = enum_values(which as i32);
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        ok.eq(rust.len(), c.len(), || "count".into());
        let mut w = rust.clone();
        let i = (1..w.len()).find(|&i| w[i] != w[i - 1]).expect("two distinct values");
        w.swap(i, i - 1);
        for (k, (&r, &v)) in rust.iter().zip(c.iter()).enumerate() {
            ok.eq(r, v, || format!("entry {k}"));
            bug.eq(w[k], v, || format!("entry {k}"));
        }
        ok.finish();
        bug.finish_planted();
    }

    let mut fs: Words = [0; 4];
    let mut all: Words = [0; 4];
    unsafe {
        cpp::ref_flags_globals(0, fs.as_mut_ptr());
        cpp::ref_flags_globals(1, all.as_mut_ptr());
    }
    let mut ok = Check::new("KINDOFMASK_FS, DISABLEDMASK_ALL");
    ok.eq(kind_of::kindofmask_fs().words(), fs, || "KINDOFMASK_FS".into());
    ok.eq(disabled_types::disabledmask_all().words(), all, || "DISABLEDMASK_ALL".into());
    ok.finish();
    let mut bug = Check::new("KINDOFMASK_FS, DISABLEDMASK_ALL");
    let mut w = kind_of::kindofmask_fs();
    w.set(kind_of::KINDOF_FS_AIRFIELD, 0); // planted: one kind missing
    bug.eq(w.words(), fs, || "KINDOFMASK_FS".into());
    bug.finish_planted();
}

macro_rules! per_type {
    ($test:ident, $marker:ty, $which:expr, $tag:expr, $seed:expr) => {
        #[test]
        fn $test() {
            let _turn = CPP_GLOBALS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut g = Gen::new($seed);
            ops_and_names::<$marker>($which, $tag, &mut g);
            parse_and_xfer::<$marker>($which, $tag, &mut g);
        }
    };
}

per_type!(kind_of_mask_type, kind_of::KindOfNames, 0, "KindOfMaskType", 301);
per_type!(object_status_mask_type, object_status_types::ObjectStatusNames, 1, "ObjectStatusMaskType", 302);
per_type!(disabled_mask_type, disabled_types::DisabledNames, 2, "DisabledMaskType", 303);
per_type!(model_condition_flags, model_state::ModelConditionNames, 3, "ModelConditionFlags", 304);
per_type!(armor_set_flags, armor_set::ArmorSetNames, 4, "ArmorSetFlags", 305);
per_type!(weapon_set_flags, weapon_set_flags::WeaponSetNames, 5, "WeaponSetFlags", 306);
per_type!(special_power_mask_type, special_power_mask_type::SpecialPowerNames, 6, "SpecialPowerMaskType", 307);


/// `Xfer::xferKindOf` (Xfer.cpp, with the real `KindOfMaskType` lookups, see
/// `differential/wrap_xfer.cpp`) against `Xfer::xfer_kind_of`: CRC values,
/// saved bytes, and loads of those bytes and of files with unknown names
/// (which leave the value unchanged). Values -5..=120 and 2,000 random ints.
/// Planted bug: an unknown name on load sets `KINDOF_INVALID`.
#[test]
fn xfer_kind_of_matches() {
    let _turn = CPP_GLOBALS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(310);
    let path = std::env::temp_dir().join(format!("rts-lockstep-kindof-{}.bin", std::process::id()));
    let cpath = CString::new(path.to_str().expect("UTF-8 temp path")).expect("no NUL");
    let mut ok = Check::new("Xfer::xferKindOf");
    let mut bug = Check::new("Xfer::xferKindOf");
    let values: Vec<i32> = (-5..=120).chain((0..2000).map(|_| g.i32())).collect();
    for (k, &v) in values.iter().enumerate() {
        // CRC
        let mut out = [0i32; 2];
        let st = unsafe { cpp::ref_xfer_kind_of(0, cpath.as_ptr(), v, out.as_mut_ptr()) };
        let mut x = XferCrc::new_without_nan_guard();
        x.open(b"x").expect("open");
        let mut r = v;
        x.xfer_kind_of(&mut r).expect("crc");
        ok.eq((0, r, x.get_crc()), (st, out[0], out[1] as u32), || format!("CRC of {v}"));
        // save
        let st = unsafe { cpp::ref_xfer_kind_of(1, cpath.as_ptr(), v, out.as_mut_ptr()) };
        let saved = std::fs::read(&path).expect("read C++ save");
        let mut xs = XferSave::new();
        xs.open(b"x").expect("open");
        let mut r = v;
        xs.xfer_kind_of(&mut r).expect("save");
        xs.close().expect("close");
        ok.eq((0, xs.bytes().to_vec()), (st, saved.clone()), || format!("save of {v}"));
        // load: the saved file, or one with an unknown name
        let mut file = saved;
        if k % 3 == 1 && file.len() > 2 {
            let last = file.len() - 1;
            file[last] = b'~';
        }
        std::fs::write(&path, &file).expect("write");
        let start = (k as i32 % 7) - 1;
        let st = unsafe { cpp::ref_xfer_kind_of(2, cpath.as_ptr(), start, out.as_mut_ptr()) };
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"x").expect("open");
        let mut r = start;
        let rst = match xl.xfer_kind_of(&mut r) {
            Ok(()) => 0,
            Err(e) => e as i32,
        };
        ok.eq((rst, r), (st, out[0]), || format!("load of {file:?} from {start}"));
        let wrong = if rst == 0 && r == start && kind_of::KindOfMaskType::get_single_bit_from_name(&file[2..]) == -1 {
            -1
        } else {
            r
        };
        bug.eq((rst, wrong), (st, out[0]), || format!("case {k}"));
    }
    let _ = std::fs::remove_file(&path);
    ok.finish();
    bug.finish_planted();
}
