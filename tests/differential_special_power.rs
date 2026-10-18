// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `SpecialPower`
//! (`common::special_power`, with `common::academy_classification` and
//! `common::audio_event_name`) against upstream `SpecialPower.cpp`,
//! `SpecialPower.h` and `INISpecialPower.cpp` (commit 0a05454d), with
//! `Overridable.h`/`Override.h`, compiled as `differential/wrap_upgrade.cpp`
//! describes, with the real `INI::load` and `NameKeyGenerator` underneath.
//!
//! No game INI files are available, so every file is **synthetic**:
//! `SpecialPower` blocks (new and repeated names, names that differ only in
//! case, `DefaultSpecialPower` before and after the templates that copy it,
//! every field in its several forms, sciences that are and are not defined,
//! `NoSound`, bad and missing values, unknown fields and blocks, a missing
//! `End`, comments, `CRLF`, random damage to the file) mixed with a share of
//! `Upgrade` blocks (shared name keys).
//!
//! 1. `INI sequences`: 6,000 cases of 1 to 4 files loaded one after another
//!    into the same stores, each in a load type that is random (`Overwrite`,
//!    `CREATE_OVERRIDES`, `Multifile`, `Invalid`; the first is mostly
//!    `Overwrite`), with `SpecialPowerStore::reset()` between some of them.
//!    After **each** file: what `INI::load` throws (nothing, an
//!    `INIException` and its whole message, an `INI_*` status, an
//!    `ErrorCode`) and the INI `XferCRC`, then **every field of every node
//!    of every override chain** (name, id, type, reload time, required
//!    science, both sounds' names, academy type, detection time, view object
//!    duration and range, radius cursor radius, the three flags, and each
//!    node's `m_isOverride`) and the next id, also when the load threw. After
//!    the last: the getters through `findSpecialPowerTemplate` (24 names),
//!    `findSpecialPowerTemplateByID`, `getSpecialPowerTemplateByIndex` and
//!    `getNumSpecialPowers`, which read the final override (but for
//!    `getAcademyClassificationType`, which reads the head), and the keys
//!    `nameToKey` then gives to a list of names (`RequiredScience` assigns a
//!    key to every name it reads).
//! 2. The name list `SpecialPowerMaskType::s_bitNameList` and
//!    `DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT`, from the real
//!    `SpecialPower.cpp` (a copy in which `build.rs` renames the definition so
//!    the real text can be dumped; the BitFlags build keeps its own).
//! 3. The mask helpers of `SpecialPower.h` (`TEST_SPECIALPOWERMASK`, `_ANY`,
//!    `_MULTI`, `SPECIALPOWERMASK_ANY_SET`, `CLEAR_`, `SET_`, `SET_ALL_..._BITS`,
//!    `FLIP_`, `MAKE_SPECIALPOWER_MASK`) on random 67-bit masks and indices
//!    inside and outside `[0, 67)`.
//!
//! Inputs for which the Rust reports undefined behaviour upstream, or a case
//! it cannot represent (a number out of range of its type in a field; a
//! template made from a `DefaultSpecialPower` that has an override chain,
//! whose pointer upstream shares and later frees, see the module docs of
//! `common::special_power`), are **not** given to the C++; the file that
//! reaches one ends its case, and their count is printed.
//!
//! Each comparison also runs a planted bug, which must be caught.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::ini::{Ini, IniErrorKind, IniLoadType, Undefined};
use rts_lockstep_engine::common::special_power::{
    clear_special_power_mask, flip_special_power_mask, make_special_power_mask, set_all_special_power_mask_bits, set_special_power_mask,
    special_power_mask_any_set, test_special_power_mask, test_special_power_mask_any, test_special_power_mask_multi,
    SpecialPowerTemplateRef, DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT,
};
use rts_lockstep_engine::common::special_power_mask_type::{SpecialPowerMaskType, SPECIAL_POWER_NAMES};
use rts_lockstep_engine::common::xfer::Xfer;
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;
use support::{Check, Gen};
use up_common::*;

/// The C++ side has process-global state (the name keys, the stores, `strtok`),
/// so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

type Getters = Option<([u32; 10], Vec<u8>)>;

fn put_str(v: &mut Vec<u8>, s: &[u8]) {
    v.extend_from_slice(&(s.len() as u32).to_le_bytes());
    v.extend_from_slice(s);
}

/// The getters of `wrap_upgrade.cpp`'s `spGetters`, from a Rust template.
/// `bug`: 1 reads the academy type from the final override (upstream reads
/// the head's), 2 reads the reload time from the head instead of the final
/// override (the chain is not followed).
fn rust_getters(t: Option<SpecialPowerTemplateRef<'_>>, bug: u32) -> Getters {
    let t = t?;
    let mut flags = 0;
    if t.has_public_timer() {
        flags |= 1;
    }
    if t.is_shared_n_sync() {
        flags |= 2;
    }
    if t.is_shortcut_power() {
        flags |= 4;
    }
    let academy = if bug == 1 { t.chain().get_final_override().data.academy_classification_type } else { t.academy_classification_type() };
    let reload = if bug == 2 { t.chain().data.reload_time } else { t.reload_time() };
    let out = [
        t.id(),
        t.special_power_type() as u32,
        reload,
        t.required_science().0 as u32,
        t.detection_time(),
        t.view_object_duration(),
        t.view_object_range().to_bits(),
        t.radius_cursor_radius().to_bits(),
        flags,
        academy as i32 as u32,
    ];
    let mut names = Vec::new();
    put_str(&mut names, t.name());
    put_str(&mut names, &t.initiate_sound().event_name);
    put_str(&mut names, &t.initiate_at_target_sound().event_name);
    Some((out, names))
}

fn cpp_getters(f: impl FnOnce(*mut u32, *mut u8, i32, *mut i32) -> i32) -> Getters {
    let mut out = [0u32; 10];
    let mut buf = vec![0u8; 4096];
    let mut n = 0i32;
    let st = f(out.as_mut_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut n);
    if st != 0 {
        return None;
    }
    buf.truncate(n as usize);
    Some((out, buf))
}

fn cpp_by_name(name: &str) -> Getters {
    let c = CString::new(name).unwrap();
    cpp_getters(|o, b, cap, n| unsafe { cpp::ref_sp_by_name(c.as_ptr(), o, b, cap, n) })
}
fn cpp_by_id(id: u32) -> Getters {
    cpp_getters(|o, b, cap, n| unsafe { cpp::ref_sp_by_id(id, o, b, cap, n) })
}
fn cpp_by_index(i: u32) -> Getters {
    cpp_getters(|o, b, cap, n| unsafe { cpp::ref_sp_by_index(i, o, b, cap, n) })
}

fn load_type_of(g: &mut Gen, first: bool) -> IniLoadType {
    match g.below(if first { 12 } else { 8 }) {
        0..=1 if first => IniLoadType::CreateOverrides,
        0..=4 if !first => IniLoadType::CreateOverrides,
        5 if !first => IniLoadType::Multifile,
        6 if !first => IniLoadType::Invalid,
        11 if first => IniLoadType::Multifile,
        _ => IniLoadType::Overwrite,
    }
}

#[test]
fn ini_sequences_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let probe = name_probe();
    let mut g = Gen::new(0xB3E1);
    let mut outcome = Check::new("SpecialPower INI::load (throw, CRC)");
    let mut dumps = Check::new("SpecialPowerStore (every node, every field)");
    let mut up_dumps = Check::new("UpgradeCenter beside it");
    let mut getters = Check::new("getters (by name, ID, index; count)");
    let mut keys = Check::new("name keys after the sequence");
    let mut dump_bug1 = Check::new("SpecialPowerStore dump");
    let mut dump_bug2 = Check::new("SpecialPowerStore dump");
    let mut academy_bug = Check::new("getAcademyClassificationType");
    let mut chain_bug = Check::new("getReloadTime (follows the override chain)");
    let mut keys_bug = Check::new("name keys after the sequence");
    let mut excluded = Excluded::default();
    let (mut files, mut ok, mut failed, mut resets, mut templates, mut chains) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    let (mut override_loads, mut aliased, mut with_default) = (0u64, 0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        let init_names = g.below(3) != 0;
        let init_center = g.below(4) != 0;
        let mut ctx = Ctx::new(init_names, init_center);
        cpp_reset(init_names, init_center);
        let nfiles = 1 + g.below(4);
        let mut stopped = false;
        let mut log = String::new();
        for f in 0..nfiles {
            let load_type = load_type_of(&mut g, f == 0);
            let kind = if g.below(8) == 0 { 2 } else { 1 };
            let data = gen_file(&mut g, kind);
            let name = format!("Data\\INI\\Case{}.ini", f % 7).into_bytes();
            let with_xfer = g.below(4) != 0;
            let mut crc = XferCrc::new_without_nan_guard();
            let r = Ini::load(&name, &data, load_type, if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None }, &table, &mut ctx);
            let crc_v = if with_xfer { crc.get_crc() } else { 0 };
            let rust = match rust_outcome(r, crc_v) {
                Ok(o) => o,
                Err(e) => {
                    aliased += u64::from(e.kind == IniErrorKind::Undefined(Undefined::AliasedOverrideChain));
                    excluded.add(&e);
                    stopped = true;
                    break;
                }
            };
            files += 1;
            if rust.status == 0 {
                ok += 1
            } else {
                failed += 1
            }
            override_loads += u64::from(load_type == IniLoadType::CreateOverrides);
            log.push_str(&format!("\n  file {f} ({load_type:?}): {:?}", show(&data)));
            let what = || format!("case {case}: init names {init_names} center {init_center}{log}");
            let c = cpp_load(&name, &data, load_type, with_xfer);
            outcome.eq(&rust, &c, what);
            let cpp_sp = cpp_dump(cpp::ref_sp_dump);
            dumps.eq(rust_special_power_dump(&ctx.store, 0), cpp_sp.clone(), what);
            dump_bug1.eq(rust_special_power_dump(&ctx.store, 1), cpp_sp.clone(), what);
            dump_bug2.eq(rust_special_power_dump(&ctx.store, 2), cpp_sp, what);
            up_dumps.eq(rust_upgrade_dump(&ctx.center, 0), cpp_dump(cpp::ref_up_dump), what);
            if f + 1 < nfiles && g.below(4) == 0 {
                ctx.store.reset();
                unsafe { cpp::ref_sp_reset_store() };
                resets += 1;
                let what = || format!("case {case}: init names {init_names} center {init_center}{log}\n  reset()");
                dumps.eq(rust_special_power_dump(&ctx.store, 0), cpp_dump(cpp::ref_sp_dump), what);
                log.push_str("\n  reset()");
            }
        }
        if stopped {
            continue;
        }
        let what = || format!("case {case}: init names {init_names} center {init_center}{log}");
        templates += ctx.store.len() as u64;
        chains += ctx.store.templates().filter(|t| t.chain().get_next_override().is_some()).count() as u64;
        with_default += u64::from(ctx.store.find_special_power_template(b"DefaultSpecialPower").is_some());

        // the getters
        getters.eq(ctx.store.len() as i32, unsafe { cpp::ref_sp_count() }, || format!("{} getNumSpecialPowers", what()));
        for _ in 0..24 {
            let n = if g.below(3) == 0 { format!("Name{}", g.below(30)) } else { SP_BLOCK_NAMES[g.below(10) as usize].to_string() };
            let rt = ctx.store.find_special_power_template(n.as_bytes());
            let (rg, bug1, bug2) = (rust_getters(rt, 0), rust_getters(rt, 1), rust_getters(rt, 2));
            let cg = cpp_by_name(&n);
            getters.eq(rg.clone(), cg.clone(), || format!("{} find {n:?}", what()));
            academy_bug.eq(bug1, cg.clone(), what);
            chain_bug.eq(bug2, cg, what);
        }
        for id in 0..ctx.store.next_special_power_id() + 2 {
            getters
                .eq(rust_getters(ctx.store.find_special_power_template_by_id(id), 0), cpp_by_id(id), || format!("{} find id {id}", what()));
        }
        for i in 0..ctx.store.len() as u32 + 2 {
            getters.eq(rust_getters(ctx.store.get_special_power_template_by_index(i), 0), cpp_by_index(i), || {
                format!("{} find index {i}", what())
            });
        }

        let rk: Vec<u32> = probe.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
        let ck: Vec<u32> = probe.iter().map(|n| cpp_name_key(n)).collect();
        keys.eq(rk.clone(), ck.clone(), what);
        keys_bug.eq(vec![rk[1] ^ 1], vec![ck[1]], what);
    }
    println!(
        "SpecialPower INI: {CASES} sequences, {files} files ({ok} loaded, {failed} threw; {override_loads} in CREATE_OVERRIDES), {resets} resets; \
         at the end {templates} templates ({chains} with an override chain, {with_default} stores with DefaultSpecialPower); \
         excluded (undefined upstream, ends the case): {} {:?} (of which {aliased} aliased default chains)",
        excluded.total(),
        excluded.0
    );
    assert!(ok > 3000 && failed > 3000, "the generator must reach both outcomes: {ok} loaded, {failed} threw");
    assert!(
        override_loads > 2000 && chains > 500 && resets > 500,
        "too few override loads / chains / resets: {override_loads} / {chains} / {resets}"
    );
    assert!(templates > 8000 && with_default > 500, "too few templates compared: {templates} ({with_default} with DefaultSpecialPower)");
    outcome.finish();
    dumps.finish();
    up_dumps.finish();
    getters.finish();
    keys.finish();
    dump_bug1.finish_planted();
    dump_bug2.finish_planted();
    academy_bug.finish_planted();
    chain_bug.finish_planted();
    keys_bug.finish_planted();
}

#[test]
fn name_list_and_constants_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut ck = Check::new("SpecialPower.cpp s_bitNameList, detection constant");
    let mut bug = Check::new("SpecialPower.cpp s_bitNameList");
    let mut cpp_names: Vec<Option<String>> = Vec::new();
    for i in 0..=SPECIAL_POWER_NAMES.len() as i32 {
        let p = unsafe { cpp::ref_sp_real_name(i) };
        cpp_names.push((!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap().to_string()));
    }
    let mut rust_names: Vec<Option<String>> = SPECIAL_POWER_NAMES.iter().map(|s| Some(s.to_string())).collect();
    rust_names.push(None);
    ck.eq(rust_names.clone(), cpp_names.clone(), || "names".to_string());
    ck.eq(DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT, unsafe { cpp::ref_sp_default_detection() }, || "detection time".to_string());
    rust_names.swap(3, 4);
    bug.eq(rust_names, cpp_names, || "two names swapped".to_string());
    bug.eq(DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT + 1, unsafe { cpp::ref_sp_default_detection() }, || {
        "detection time".to_string()
    });
    ck.finish();
    bug.finish_planted();
}

fn random_mask(g: &mut Gen) -> SpecialPowerMaskType {
    let sparse = g.below(3) == 0;
    let mut w = [g.u64(), g.u64() & 0b111, 0, 0];
    if sparse {
        w[0] &= g.u64() & g.u64();
    }
    SpecialPowerMaskType::from_words(w)
}

fn cpp_mask_op(op: i32, a: &SpecialPowerMaskType, b: &SpecialPowerMaskType, c: &SpecialPowerMaskType, idx: &[i32]) -> (i32, [u64; 4]) {
    let mut out = [0u64; 4];
    let mut padded = [0i32; 5];
    padded[..idx.len()].copy_from_slice(idx);
    let r = unsafe {
        cpp::ref_mask_op(
            1,
            op,
            a.words().as_ptr(),
            b.words().as_ptr(),
            c.words().as_ptr(),
            padded.as_ptr(),
            idx.len() as i32,
            out.as_mut_ptr(),
        )
    };
    (r, out)
}

#[test]
fn special_power_mask_helpers_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xB3E2);
    let mut ck = Check::new("TEST/CLEAR/SET/SET_ALL/FLIP/MAKE_SPECIALPOWER_MASK");
    let mut multi_bug = Check::new("TEST_SPECIALPOWERMASK_MULTI");
    let mut set_bug = Check::new("SET_SPECIALPOWERMASK (val 0 clears)");
    let mut out_of_range = 0u64;
    for case in 0..30_000 {
        let (a, mut b, mut c) = (random_mask(&mut g), random_mask(&mut g), random_mask(&mut g));
        if g.below(3) == 0 {
            // masks for which TEST_SPECIALPOWERMASK_MULTI is true: b inside a, c outside it
            let (wa, wb, wc) = (a.words(), b.words(), c.words());
            b = SpecialPowerMaskType::from_words([wa[0] & wb[0], wa[1] & wb[1], 0, 0]);
            c = SpecialPowerMaskType::from_words([!wa[0] & wc[0], !wa[1] & wc[1] & 0b111, 0, 0]);
        }
        let idx: Vec<i32> = (0..2)
            .map(|i| {
                if g.below(6) == 0 {
                    g.below(80) as i32 - 6
                } else if i == 1 {
                    g.below(3) as i32 - 1
                } else {
                    g.below(67) as i32
                }
            })
            .collect();
        let what = || format!("case {case}: a {a:?} b {b:?} c {c:?} idx {idx:?}");
        let r = test_special_power_mask(&a, idx[0]);
        out_of_range += u64::from(r.is_err());
        ck.eq(r.map_or(-1, i32::from), cpp_mask_op(0, &a, &b, &c, &idx).0, what);
        ck.eq(i32::from(test_special_power_mask_any(&a, &b)), cpp_mask_op(1, &a, &b, &c, &idx).0, what);
        let cm = cpp_mask_op(2, &a, &b, &c, &idx).0;
        ck.eq(i32::from(test_special_power_mask_multi(&a, &b, &c)), cm, what);
        multi_bug.eq(i32::from(test_special_power_mask_multi(&a, &c, &b)), cm, what);
        ck.eq(i32::from(special_power_mask_any_set(&a)), cpp_mask_op(3, &a, &b, &c, &idx).0, what);
        let mut m = a;
        clear_special_power_mask(&mut m);
        ck.eq((0, m.words()), cpp_mask_op(4, &a, &b, &c, &idx), what);
        // SET_SPECIALPOWERMASK(m, t, val): the bit takes `val` (converted to bool)
        let mut m = a;
        let rs = set_special_power_mask(&mut m, idx[0], idx[1]);
        let cs = cpp_mask_op(5, &a, &b, &c, &idx);
        ck.eq(rs.map_or((-1, [0; 4]), |()| (0, m.words())), if cs.0 < 0 { (-1, [0; 4]) } else { cs }, what);
        let mut m2 = a;
        let _ = set_special_power_mask(&mut m2, idx[0], 1);
        set_bug.eq(rs.map_or((-1, [0; 4]), |()| (0, m2.words())), if cs.0 < 0 { (-1, [0; 4]) } else { cs }, what);
        let mut m = a;
        set_all_special_power_mask_bits(&mut m);
        ck.eq((0, m.words()), cpp_mask_op(6, &a, &b, &c, &idx), what);
        let mut m = a;
        flip_special_power_mask(&mut m);
        ck.eq((0, m.words()), cpp_mask_op(7, &a, &b, &c, &idx), what);
        let rmk = make_special_power_mask(idx[0]);
        let (cr, cw) = cpp_mask_op(8, &a, &b, &c, &idx[..1]);
        ck.eq(rmk.map_or((-1, [0; 4]), |m| (0, m.words())), (cr, cw), what);
    }
    println!("SpecialPower mask helpers: 30000 cases ({out_of_range} test indices out of range, std::out_of_range upstream)");
    assert!(out_of_range > 300);
    ck.finish();
    multi_bug.finish_planted();
    set_bug.finish_planted();
}
