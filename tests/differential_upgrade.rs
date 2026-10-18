// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 4's `Upgrade`
//! (`common::upgrade`, with `common::academy_classification` and
//! `common::audio_event_name`) against upstream `Upgrade.cpp`, `Upgrade.h`
//! and `INIUpgrade.cpp` (commit 0a05454d), compiled as
//! `differential/wrap_upgrade.cpp` describes, with the real `INI::load`,
//! `NameKeyGenerator` and `XferSave`/`XferLoad`/`XferCRC` underneath.
//!
//! No game INI files are available, so every file is **synthetic**: `Upgrade`
//! blocks (new and repeated names, names that differ only in case,
//! `DefaultUpgrade` before and after the templates that copy it, the veterancy
//! names that `UpgradeCenter::init` makes, every field in its several forms,
//! `NoSound`, bad and missing values, unknown fields and blocks, a missing
//! `End`, comments, `CRLF`, random damage to the file, and files with more
//! than 128 templates) mixed with a share of `SpecialPower` blocks, which the
//! same C++ `INI::load` runs through the real `SpecialPowerStore` (shared name
//! keys). The center is `init()`ed in most cases, the name key generator is
//! `init()`ed or only reset.
//!
//! 1. `INI blocks`: 8,000 files fed identically to both sides, in a random
//!    load type. Compared: what `INI::load` throws (nothing, an
//!    `INIException` and its whole message, an `INI_*` status, an
//!    `ErrorCode`, `std::out_of_range` from the 129th mask bit), the INI
//!    `XferCRC`, **every field of every template in list order** (type,
//!    name, name key, display label, build time bitwise, cost, the 128-bit
//!    mask, both sounds' names, the academy type, the button image name) and
//!    the next mask bit, also when the load threw; `findUpgrade` (list
//!    position) for 14 names, `findUpgradeByKey`, `findVeterancyUpgrade`,
//!    `getUpgradeNames`, `calcTimeToBuild(NULL)` and `calcCostToBuild(NULL)`
//!    of every template; and the keys `nameToKey` then gives to a list of
//!    names (key assignment order is lockstep state: `findUpgrade` assigns a
//!    key to every name it is asked for, `newUpgrade` looks up
//!    `DefaultUpgrade`).
//! 2. `newUpgrade` called directly 135 times in 300 centers (with and without
//!    `DefaultUpgrade`, duplicate names): the status of each call (the
//!    129th throws `std::out_of_range`) and the store after.
//! 3. `Upgrade` (the instance) xfer: 6,000 saves (bytes and status), loads
//!    from mutated files, and `XferCRC`.
//! 4. The mask helpers of `Upgrade.h` (`TEST_UPGRADE_MASK`, `_ANY`, `_MULTI`,
//!    `UPGRADE_MASK_ANY_SET`, `CLEAR_`, `SET_ALL_..._BITS`, `FLIP_`,
//!    `MAKE_UPGRADE_MASK` to `MAKE_UPGRADE_MASK5`) on random 128-bit masks and
//!    indices inside and outside `[0, 128)`.
//! 5. `TheUpgradeTypeNames` and `TheAcademyClassificationTypeNames` (the
//!    latter cut out of the real `AcademyStats.cpp` by `build.rs`).
//!
//! Inputs for which the Rust reports undefined behaviour upstream (a number
//! out of range of its type in a field, `calcTimeToBuild`'s float-to-int
//! conversion out of range, see the module docs of `common::upgrade` and
//! `common::ini`) are **not** given to the C++; their count is printed.
//!
//! `Xfer::xferUpgradeMask` is **not** compared here: the Xfer build stands
//! `TheUpgradeCenter` in with functions that abort (`wrap_xfer.cpp`); its
//! tests are Rust unit tests (`common::upgrade::tests`).
//!
//! Each comparison also runs a planted bug, which must be caught.
#![cfg(feature = "differential")]

mod support;
mod up_common;

use rts_lockstep_engine::common::academy_classification::ACADEMY_CLASSIFICATION_TYPE_NAMES;
use rts_lockstep_engine::common::game_common::VeterancyLevel;
use rts_lockstep_engine::common::ini::{Ini, IniLoadType};
use rts_lockstep_engine::common::name_key_generator::NameKeyType;
use rts_lockstep_engine::common::upgrade::{
    clear_upgrade_mask, flip_upgrade_mask, make_upgrade_mask, set_all_upgrade_mask_bits, test_upgrade_mask, test_upgrade_mask_any,
    test_upgrade_mask_multi, upgrade_mask_any_set, Upgrade, UpgradeCenter, UpgradeCenterError, UpgradeMaskType, UpgradeTemplate,
    UPGRADE_TYPE_NAMES,
};
use rts_lockstep_engine::common::xfer::{Xfer, XferStatus};
use rts_lockstep_engine::common::xfer_crc::XferCrc;
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;
use support::{Check, Gen};
use up_common::*;

/// The C++ side has process-global state (the name keys, the stores, `strtok`,
/// `XferLoad`'s static buffer), so the tests take turns.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load_type_of(g: &mut Gen) -> IniLoadType {
    match g.below(12) {
        0 => IniLoadType::CreateOverrides,
        1 => IniLoadType::Multifile,
        2 => IniLoadType::Invalid,
        _ => IniLoadType::Overwrite,
    }
}

/// The list position of a template a lookup returned.
fn position(center: &UpgradeCenter, found: Option<&UpgradeTemplate>) -> i32 {
    match found {
        None => -1,
        Some(f) => center.templates().position(|t| std::ptr::eq(t, f)).map_or(-2, |p| p as i32),
    }
}

fn names_dump(names: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for n in names {
        v.extend_from_slice(&(n.len() as u32).to_le_bytes());
        v.extend_from_slice(n);
    }
    v
}

#[test]
fn ini_blocks_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let table = blocks();
    let probe = name_probe();
    let mut g = Gen::new(0xB3D1);
    let mut outcome = Check::new("Upgrade INI::load (throw, CRC)");
    let mut dumps = Check::new("UpgradeCenter (every field, list order)");
    let mut sp_dumps = Check::new("SpecialPowerStore beside it");
    let mut finds = Check::new("findUpgrade / ByKey / Veterancy / names");
    let mut calcs = Check::new("calcTimeToBuild / calcCostToBuild");
    let mut keys = Check::new("name keys after the load");
    let mut order_bug = Check::new("UpgradeCenter list order");
    let mut time_bug = Check::new("UpgradeTemplate build time");
    let mut mask_bug = Check::new("UpgradeTemplate mask");
    let mut keys_bug = Check::new("name keys after the load");
    let mut excluded = Excluded::default();
    let mut calc_excluded = 0u64;
    let (mut ok, mut failed, mut out_of_range, mut templates, mut with_default) = (0u64, 0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 8_000;
    for case in 0..CASES {
        let init_names = g.below(3) != 0;
        let init_center = g.below(5) != 0;
        let load_type = load_type_of(&mut g);
        let many = g.below(25) == 0;
        let n_many = 126 + g.below(8);
        let kind = if g.below(8) == 0 { 2 } else { 0 };
        let data = if many { gen_many_upgrades(&mut g, n_many, "P") } else { gen_file(&mut g, kind) };
        let name = format!("Data\\INI\\Case{}.ini", case % 7).into_bytes();
        let with_xfer = g.below(4) != 0;
        let mut ctx = Ctx::new(init_names, init_center);
        let mut crc = XferCrc::new_without_nan_guard();
        let r = Ini::load(&name, &data, load_type, if with_xfer { Some(&mut crc as &mut dyn Xfer) } else { None }, &table, &mut ctx);
        let crc_v = if with_xfer { crc.get_crc() } else { 0 };
        let rust = match rust_outcome(r, crc_v) {
            Ok(o) => o,
            Err(e) => {
                excluded.add(&e);
                continue;
            }
        };
        // (a handler's std::out_of_range reaches INI::load wrapped in an INIException)
        if rust.status == 0 {
            ok += 1;
        } else {
            failed += 1;
            out_of_range += u64::from(ctx.center.len() == 128 && ctx.center.next_template_mask_bit() == 128);
        }
        templates += ctx.center.len() as u64;
        let what = || format!("case {case}: init names {init_names} center {init_center} load type {load_type:?} file {:?}", show(&data));
        cpp_reset(init_names, init_center);
        let c = cpp_load(&name, &data, load_type, with_xfer);
        outcome.eq(&rust, &c, what);

        // the store, field by field, list order
        let cpp_dump_bytes = cpp_dump(cpp::ref_up_dump);
        dumps.eq(rust_upgrade_dump(&ctx.center, 0), cpp_dump_bytes.clone(), what);
        sp_dumps.eq(rust_special_power_dump(&ctx.store, 0), cpp_dump(cpp::ref_sp_dump), what);
        order_bug.eq(rust_upgrade_dump(&ctx.center, 1), cpp_dump_bytes.clone(), what);
        time_bug.eq(rust_upgrade_dump(&ctx.center, 2), cpp_dump_bytes.clone(), what);
        mask_bug.eq(rust_upgrade_dump(&ctx.center, 3), cpp_dump_bytes, what);
        if ctx.center.find_upgrade(&mut ctx.names, b"DefaultUpgrade").unwrap().is_some() {
            with_default += 1;
        }
        // (the lookup above assigned the key on the Rust side; do the same call on the C++ side
        // before comparing the other lookups, so the key streams stay equal)
        let cdn = CString::new("DefaultUpgrade").unwrap();
        let cd = unsafe { cpp::ref_up_find(cdn.as_ptr()) };
        finds.eq(position(&ctx.center, ctx.center.find_upgrade(&mut ctx.names, b"DefaultUpgrade").unwrap()), cd, what);

        // the lookups
        for _ in 0..10 {
            let n = if g.below(3) == 0 { format!("Name{}", g.below(150)) } else { UPGRADE_BLOCK_NAMES[g.below(14) as usize].to_string() };
            let cn = CString::new(n.as_str()).unwrap();
            let rf = position(&ctx.center, ctx.center.find_upgrade(&mut ctx.names, n.as_bytes()).unwrap());
            finds.eq(rf, unsafe { cpp::ref_up_find(cn.as_ptr()) }, || format!("{} find {n:?}", what()));
        }
        for _ in 0..6 {
            let key = if g.below(4) == 0 {
                g.below(300)
            } else {
                ctx.center.templates().nth(g.below(ctx.center.len() as u32 + 1) as usize).map_or(0, |t| t.upgrade_name_key().0)
            };
            let rf = position(&ctx.center, ctx.center.find_upgrade_by_key(NameKeyType(key)));
            finds.eq(rf, unsafe { cpp::ref_up_find_by_key(key) }, || format!("{} find key {key}", what()));
        }
        for level in [VeterancyLevel::Regular, VeterancyLevel::Veteran, VeterancyLevel::Elite, VeterancyLevel::Heroic] {
            let rf = position(&ctx.center, ctx.center.find_veterancy_upgrade(&mut ctx.names, level).unwrap());
            finds.eq(rf, unsafe { cpp::ref_up_find_vet(level as i32) }, || format!("{} find vet {level:?}", what()));
        }
        let mut nb = vec![0u8; 1 << 20];
        let n = unsafe { cpp::ref_up_names(nb.as_mut_ptr(), nb.len() as i32) };
        nb.truncate(n as usize);
        finds.eq(names_dump(&ctx.center.get_upgrade_names()), nb, || format!("{} getUpgradeNames", what()));

        // the Player-free results of calcTimeToBuild / calcCostToBuild
        for (i, t) in ctx.center.templates().enumerate() {
            let Ok(time) = t.time_to_build_frames() else {
                calc_excluded += 1;
                continue;
            };
            let (mut ct, mut cc) = (0i32, 0i32);
            assert_eq!(unsafe { cpp::ref_up_calc(i as i32, &mut ct, &mut cc) }, 0);
            calcs.eq((time, t.cost_to_build()), (ct, cc), || format!("{} template {i} {:?}", what(), show(t.upgrade_name())));
        }

        // the keys nameToKey now gives to the names
        let rk: Vec<u32> = probe.iter().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect();
        let ck: Vec<u32> = probe.iter().map(|n| cpp_name_key(n)).collect();
        let mut planted = probe.iter().rev().map(|n| ctx.names.name_to_key(n.as_bytes()).0).collect::<Vec<_>>();
        planted.reverse();
        keys.eq(rk.clone(), ck.clone(), what);
        keys_bug.eq(vec![planted[0] ^ 1], vec![ck[0]], what);
    }
    println!(
        "Upgrade INI: {CASES} files, {ok} loaded, {failed} threw ({out_of_range} of them at the 129th mask bit), {templates} templates compared ({with_default} stores with DefaultUpgrade); \
         excluded (undefined upstream): {} {:?} files, {calc_excluded} calcTimeToBuild results",
        excluded.total(),
        excluded.0
    );
    assert!(ok > 1500 && failed > 1500, "the generator must reach both outcomes: {ok} loaded, {failed} threw");
    assert!(out_of_range > 100, "the generator must reach the 129th template: {out_of_range}");
    assert!(templates > 20_000 && with_default > 500, "too few templates compared: {templates}, {with_default} with DefaultUpgrade");
    outcome.finish();
    dumps.finish();
    sp_dumps.finish();
    finds.finish();
    calcs.finish();
    keys.finish();
    order_bug.finish_planted();
    time_bug.finish_planted();
    mask_bug.finish_planted();
    keys_bug.finish_planted();
}

#[test]
fn new_upgrade_called_directly_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xB3D2);
    let mut statuses = Check::new("newUpgrade (status of each call)");
    let mut limit_bug = Check::new("newUpgrade (status of each call)");
    let mut dumps = Check::new("UpgradeCenter after newUpgrade");
    let (mut calls, mut threw) = (0u64, 0u64);
    for run in 0..300 {
        let init_names = g.below(2) == 0;
        let init_center = g.below(2) == 0;
        let mut ctx = Ctx::new(init_names, init_center);
        cpp_reset(init_names, init_center);
        let n = if g.below(3) == 0 { 1 + g.below(135) } else { 125 + g.below(11) };
        let default_at = g.below(n.max(2));
        let mut rs = Vec::new();
        let mut cs = Vec::new();
        for i in 0..n {
            let name = if i == default_at && g.below(2) == 0 {
                "DefaultUpgrade".to_string()
            } else if g.below(6) == 0 {
                UPGRADE_BLOCK_NAMES[g.below(14) as usize].to_string()
            } else {
                format!("N{run}_{i}")
            };
            let r = ctx.center.new_upgrade(&mut ctx.names, name.as_bytes());
            let rs_i = match r {
                Ok(_) => 0,
                Err(UpgradeCenterError::MaskBit(_)) => 4,
                Err(e) => panic!("{e}"),
            };
            let cn = CString::new(name.as_str()).unwrap();
            let cs_i = unsafe { cpp::ref_up_new_upgrade(cn.as_ptr()) };
            rs.push(rs_i);
            cs.push(cs_i);
            calls += 1;
            threw += u64::from(cs_i == 4);
        }
        statuses.eq(rs.clone(), cs.clone(), || format!("run {run}: {n} calls, init names {init_names} center {init_center}"));
        // planted: the limit is one too low (the last call that fits is refused)
        let mut planted = rs.clone();
        if rs.contains(&4) {
            if let Some(last_ok) = rs.iter().rposition(|&s| s == 0) {
                planted[last_ok] = 4;
            }
        }
        limit_bug.eq(planted, cs, || format!("run {run}"));
        dumps.eq(rust_upgrade_dump(&ctx.center, 0), cpp_dump(cpp::ref_up_dump), || {
            format!("run {run}: {n} calls, init names {init_names} center {init_center}")
        });
    }
    println!("newUpgrade: {calls} calls in 300 centers, {threw} threw std::out_of_range");
    assert!(threw > 100);
    statuses.finish();
    dumps.finish();
    limit_bug.finish_planted();
}

fn tmp_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("rts-lockstep-upgrade-{}-{tag}.bin", std::process::id()))
}

fn status_of(r: Result<(), XferStatus>) -> i32 {
    match r {
        Ok(()) => 0,
        Err(e) => e as i32,
    }
}

#[test]
fn upgrade_instance_xfer_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xB3D3);
    let path = tmp_path("xfer");
    let cpath = CString::new(path.to_str().unwrap()).unwrap();
    let mut save = Check::new("Upgrade xfer save (bytes, status)");
    let mut save_bug = Check::new("Upgrade xfer save");
    let mut load = Check::new("Upgrade xfer load (status, status)");
    let mut load_bug = Check::new("Upgrade xfer load");
    let mut crc = Check::new("Upgrade xfer CRC");
    let mut crc_bug = Check::new("Upgrade xfer CRC");
    let (mut load_ok, mut load_failed) = (0u64, 0u64);
    const CASES: usize = 6_000;
    for case in 0..CASES {
        // statuses 0..=2 are the enumerators; saving any int is defined, loading one keeps it raw
        let status = if g.below(4) == 0 { g.i32() } else { g.below(3) as i32 };
        let mut u = Upgrade::new(NameKeyType(g.below(300)));
        u.set_status(status);

        // save
        let mut cbytes = vec![0u8; 64];
        let mut clen = 0i32;
        let mut cst_status = status;
        let cst = unsafe { cpp::ref_up_inst_xfer(0, &mut cst_status, cpath.as_ptr(), cbytes.as_mut_ptr(), 64, &mut clen, &mut 0) };
        cbytes.truncate(clen as usize);
        let mut xs = XferSave::new();
        xs.open(b"rust").unwrap();
        let rst = status_of(xs.xfer_snapshot(&mut u));
        let _ = xs.close();
        let rbytes = xs.bytes().to_vec();
        save.eq((rst, &rbytes), (cst, &cbytes), || format!("case {case}: status {status}"));
        let mut planted = rbytes.clone();
        planted[1] ^= 1; // a byte of the status changes
        save_bug.eq((rst, &planted), (cst, &cbytes), || format!("case {case}"));

        // load: the saved file, or a mutated one, into a random starting status
        let mut file = cbytes.clone();
        match g.below(8) {
            1 => file[0] = g.below(6) as u8, // the version byte
            2 => {
                let p = g.below(file.len() as u32) as usize;
                file[p] = g.u32() as u8;
            }
            3 => file.truncate(file.len().saturating_sub(1 + g.below(6) as usize)),
            4 => file.extend((0..1 + g.below(5)).map(|_| g.u32() as u8)),
            _ => {}
        }
        let start = g.i32();
        let mut cs = start;
        let cst = unsafe { cpp::ref_up_inst_xfer(1, &mut cs, cpath.as_ptr(), file.as_mut_ptr(), file.len() as i32, &mut 0, &mut 0) };
        let mut r = Upgrade::new(NameKeyType(1));
        r.set_status(start);
        let mut xl = XferLoad::new(file.clone());
        xl.open(b"rust").unwrap();
        let rst = status_of(xl.xfer_snapshot(&mut r));
        if rst == 0 {
            load_ok += 1
        } else {
            load_failed += 1
        }
        // After a failed load C++ may hold a partial read of the field (fread copies the bytes it got);
        // the Rust XferLoad does not do that, so a failed load compares the status only.
        if rst == 0 {
            load.eq((rst, r.status()), (cst, cs), || format!("case {case}: file {file:?} start {start}"));
            load_bug.eq((rst, r.status() ^ 0x100), (cst, cs), || format!("case {case}"));
        } else {
            load.eq((rst, 0), (cst, 0), || format!("case {case}: file {file:?} start {start}"));
            load_bug.eq((rst + 1, 0), (cst, 0), || format!("case {case}"));
        }

        // CRC
        let mut cc = 7u32;
        let mut s2 = status;
        let cst = unsafe { cpp::ref_up_inst_xfer(2, &mut s2, cpath.as_ptr(), std::ptr::null_mut(), 0, &mut 0, &mut cc) };
        let mut xc = XferCrc::new_without_nan_guard();
        xc.open(b"crc").unwrap();
        let rst = status_of(xc.xfer_snapshot(&mut u));
        crc.eq((rst, xc.get_crc()), (cst, cc), || format!("case {case}: status {status}"));
        crc_bug.eq((rst, xc.get_crc() ^ 1), (cst, cc), || format!("case {case}"));
    }
    let _ = std::fs::remove_file(&path);
    println!("Upgrade xfer: {CASES} instances; loads {load_ok} ok, {load_failed} failed");
    assert!(load_ok > 3000 && load_failed > 500);
    save.finish();
    load.finish();
    crc.finish();
    save_bug.finish_planted();
    load_bug.finish_planted();
    crc_bug.finish_planted();
}

fn words_of(m: &UpgradeMaskType) -> [u64; 4] {
    m.words()
}

fn random_mask(g: &mut Gen) -> UpgradeMaskType {
    let sparse = g.below(3) == 0;
    let mut w = [g.u64(), g.u64(), 0, 0];
    if sparse {
        w[0] &= g.u64() & g.u64();
        w[1] &= g.u64() & g.u64();
    }
    UpgradeMaskType::from_words(w)
}

fn cpp_mask_op(op: i32, a: &UpgradeMaskType, b: &UpgradeMaskType, c: &UpgradeMaskType, idx: &[i32]) -> (i32, [u64; 4]) {
    let mut out = [0u64; 4];
    let mut padded = [0i32; 5];
    padded[..idx.len()].copy_from_slice(idx);
    let r = unsafe {
        cpp::ref_mask_op(
            0,
            op,
            words_of(a).as_ptr(),
            words_of(b).as_ptr(),
            words_of(c).as_ptr(),
            padded.as_ptr(),
            idx.len() as i32,
            out.as_mut_ptr(),
        )
    };
    (r, out)
}

#[test]
fn upgrade_mask_helpers_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0xB3D4);
    let mut ck = Check::new("TEST/CLEAR/SET_ALL/FLIP/MAKE_UPGRADE_MASK");
    let mut multi_bug = Check::new("TEST_UPGRADE_MASK_MULTI");
    let mut range_bug = Check::new("TEST_UPGRADE_MASK (index range)");
    let mut compared = 0u64;
    let mut out_of_range = 0u64;
    for case in 0..30_000 {
        let (a, mut b, mut c) = (random_mask(&mut g), random_mask(&mut g), random_mask(&mut g));
        if g.below(3) == 0 {
            // masks for which TEST_UPGRADE_MASK_MULTI is true: b inside a, c outside it
            let (wa, wb, wc) = (words_of(&a), words_of(&b), words_of(&c));
            b = UpgradeMaskType::from_words([wa[0] & wb[0], wa[1] & wb[1], 0, 0]);
            c = UpgradeMaskType::from_words([!wa[0] & wc[0], !wa[1] & wc[1], 0, 0]);
        }
        let idx: Vec<i32> = (0..5).map(|_| if g.below(6) == 0 { g.below(140) as i32 - 6 } else { g.below(128) as i32 }).collect();
        let what = || format!("case {case}: a {a:?} b {b:?} c {c:?} idx {idx:?}");
        let r = test_upgrade_mask(&a, idx[0]);
        let (cr, _) = cpp_mask_op(0, &a, &b, &c, &idx);
        out_of_range += u64::from(r.is_err());
        ck.eq(r.map_or(-1, i32::from), cr, what);
        let planted = if idx[0] >= 128 { 0 } else { r.map_or(-1, i32::from) };
        range_bug.eq(planted, cr, what);
        ck.eq(i32::from(test_upgrade_mask_any(&a, &b)), cpp_mask_op(1, &a, &b, &c, &idx).0, what);
        let rm = test_upgrade_mask_multi(&a, &b, &c);
        let cm = cpp_mask_op(2, &a, &b, &c, &idx).0;
        ck.eq(i32::from(rm), cm, what);
        multi_bug.eq(i32::from(test_upgrade_mask_multi(&a, &c, &b)), cm, what);
        ck.eq(i32::from(upgrade_mask_any_set(&a)), cpp_mask_op(3, &a, &b, &c, &idx).0, what);
        let mut m = a;
        clear_upgrade_mask(&mut m);
        ck.eq((0, words_of(&m)), cpp_mask_op(4, &a, &b, &c, &idx), what);
        set_all_upgrade_mask_bits(&mut m);
        ck.eq((0, words_of(&m)), cpp_mask_op(6, &a, &b, &c, &idx), what);
        let mut m = a;
        flip_upgrade_mask(&mut m);
        ck.eq((0, words_of(&m)), cpp_mask_op(7, &a, &b, &c, &idx), what);
        let n = 1 + g.below(5) as usize;
        let rmk = make_upgrade_mask(&idx[..n]);
        let (cr, cw) = cpp_mask_op(8, &a, &b, &c, &idx[..n]);
        ck.eq(rmk.map_or((-1, [0; 4]), |m| (0, words_of(&m))), (cr, cw), what);
        compared += 1;
    }
    println!("Upgrade mask helpers: {compared} cases ({out_of_range} test indices out of range, std::out_of_range upstream)");
    assert!(out_of_range > 300);
    ck.finish();
    multi_bug.finish_planted();
    range_bug.finish_planted();
}

#[test]
fn name_tables_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let read = |f: unsafe extern "C" fn(i32) -> *const std::os::raw::c_char, i: i32| -> Option<String> {
        let p = unsafe { f(i) };
        (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_str().unwrap().to_string())
    };
    let mut ck = Check::new("TheUpgradeTypeNames / TheAcademyClassificationTypeNames");
    let mut bug = Check::new("name tables");
    let types: Vec<Option<String>> = (0..=UPGRADE_TYPE_NAMES.len() as i32).map(|i| read(cpp::ref_up_type_name, i)).collect();
    let academy: Vec<Option<String>> =
        (0..=ACADEMY_CLASSIFICATION_TYPE_NAMES.len() as i32).map(|i| read(cpp::ref_academy_name, i)).collect();
    let mut rust_types: Vec<Option<String>> = UPGRADE_TYPE_NAMES.iter().map(|s| Some(s.to_string())).collect();
    rust_types.push(None);
    let mut rust_academy: Vec<Option<String>> = ACADEMY_CLASSIFICATION_TYPE_NAMES.iter().map(|s| Some(s.to_string())).collect();
    rust_academy.push(None);
    ck.eq(rust_types.clone(), types.clone(), || "TheUpgradeTypeNames".to_string());
    ck.eq(rust_academy.clone(), academy.clone(), || "TheAcademyClassificationTypeNames".to_string());
    rust_academy.swap(0, 1);
    bug.eq(rust_academy, academy, || "swapped".to_string());
    ck.finish();
    bug.finish_planted();
}
