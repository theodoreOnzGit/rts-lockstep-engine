#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/mixfile.h, mixfile.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::wwlib::mixfile` against the upstream C++ (#215, group wwlib4).
//!
//! # Method
//!
//! `differential/wrap_draft_mixfile.cpp` compiles `mixfile.h` and `mixfile.cpp` (the Win32 `long` spelled `int`, the Windows
//! directory tool cut off; see the wrapper) over stand-ins for the file layer (`RawFileClass` with its `Bias`/`Seek`/`Size` transcribed, an
//! in-memory file system). The Rust draft runs on real files in a temporary directory. A case is a program of 1 to 14 operations: write
//! an archive with `MixFileCreator` (1 to 6 files, three forms of `Add_File`: source path and saved name, source path alone, and a
//! `FileClass`; sources of 1 to 9000 bytes, one that does not exist) or put a hand-built (and often damaged) archive image in place;
//! construct the factory; look up every name in the pool (`Get_File`: found, `Size()`, the bytes read); the name lists
//! (`Build_Filename_List`, `Build_Ordered_Filename_List`, `Build_Internal_Filename_List` and `Get_Filename_List`); `Delete_File`,
//! `Add_File`, `Flush_Changes` (skipped for an archive put in place as a raw image: upstream writes `Size()` bytes per sub-file from a buffer
//! that a short read leaves uninitialised) (the factory is destroyed by it, by writing an archive and by an image put in place: upstream keeps the old
//! table and names offset, which then no longer describe the file) (also with 1 to 20 of the temporary names `_tmpmixNN.dat` already taken); then the final archive bytes.
//!
//! **Excluded inputs:** a negative file count, a name table with more names than the file table has entries (upstream indexes
//! past `FileInfo`), equal sub-file offsets in the ordered list (`qsort` and the draft's sort are not stable), two saved names with one
//! `CRC_Stringi` in one archive (the same), names that are not ASCII (the draft's names are `String`s).
//!
//! # Predictions and results
//!
//! Predicted before the first run: 0 mismatches on well-formed archives; mismatches on damaged ones, where a table entry reaches past the end
//! of the file: upstream's `Bias` clips the sub-file's length to the file's, and a read returns what exists, while the draft reports the
//! table's size and its read fails.
//!
//! **Results (2026-11-20).** First run that completed: 73 of 3,000 programs differed. (1) `Build_Internal_Filename_List`: upstream APPENDS to the
//! factory's list (a second call doubles it); the draft replaced it: fixed, 11 left. (2) The 11 were the prediction (a sub-file past the archive's end:
//! upstream's `Size()` is `min(archive size, table size)` and a read returns the bytes there are, the draft reported the table's size and failed
//! the read) and, in one program, `Flush_Changes` with all 20 temporary names taken: upstream's `Get_Temp_Filename` leaves the 20th name in
//! `full_path`, so `MoveFile` moves that file over the archive; the draft thought the name was empty. Both fixed: 0 mismatches over 3,000 programs.
//! (A first version of the test crashed in the C++: a std::string-based `StringClass` stand-in cannot be moved by `memcpy`/`qsort` as the real one is;
//! the stand-in is a single pointer now. Upstream also indexes `FileInfo[i]` for every name read from the name table: excluded, see above.)
//! Planted bugs (mismatching programs of 3,000): 18311 binary search `<=`: 1,224; 18312 sub-files padded to 4 not 8: 1,434; 18313 table not sorted by CRC:
//! 1,396; 18314 name length without the NUL: 1,961; 18315 the two header offsets swapped: 2,216; 18316 ordered list descending: 1,041; 18317 `Delete_File`
//! case-sensitive: 176; 18318 a pending add of an existing name is not dropped: 379; 18319 `Bias` length not clipped: 1,222; 18320 the old (empty) name
//! when all 20 temporary names are taken: 140. 10 of 10 caught.

mod support;

use rts_lockstep_engine::drafts::wwlib::mixfile::{MixFileCreator, MixFileFactoryClass};
use rts_lockstep_engine::ww::realcrc::crc_stringi;
use support::{Check, Gen};

extern "C" {
    fn ref_mx_run(dir: *const u8, sizes: *const i32, nsizes: i32, names: *const u8, nnames: i32, prog: *const i32, nwords: i32, out: *mut i32, cap: i32) -> i32;
}

const SIZES: [i32; 8] = [1, 7, 8, 100, 300, 9000, 4097, 0];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_MX_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn pool() -> Vec<String> {
    ["a.txt", "A.TXT", "data\\x.w3d", "b.ini", "c", "", "Z", "long_name_".repeat(15).as_str(), "dir/sub/file.bin", "q"].iter().map(|s| s.to_string()).collect()
}

fn source_bytes(i: i32, len: i32) -> Vec<u8> {
    (0..len).map(|k| (i * 13 + k * 7 + (k >> 8)) as u8).collect()
}

fn le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn rd(img: &[u8], at: usize) -> u32 {
    let mut b = [0u8; 4];
    for (i, x) in b.iter_mut().enumerate() {
        *x = img.get(at + i).copied().unwrap_or(0);
    }
    u32::from_le_bytes(b)
}

/// A hand-built archive: "MIX1", the two offsets, 4 unused bytes, the files each padded to 8, the CRC-sorted table, the names.
fn build_image(g: &mut Gen, pool: &[String]) -> Vec<u8> {
    let nfiles = g.below(5) as usize;
    let mut used: Vec<usize> = Vec::new();
    while used.len() < nfiles {
        let k = g.below(pool.len() as u32) as usize;
        if !used.iter().any(|&u| crc_stringi(pool[u].as_bytes(), 0) == crc_stringi(pool[k].as_bytes(), 0)) {
            used.push(k);
        }
    }
    let mut body = Vec::new();
    let mut entries = Vec::new();
    for &k in &used {
        let size = 1 + g.below(40);
        entries.push((crc_stringi(pool[k].as_bytes(), 0), 16 + body.len() as u32, size, pool[k].clone()));
        body.extend((0..size).map(|_| g.below(256) as u8));
        while (16 + body.len()) % 8 != 0 {
            body.push(0);
        }
    }
    entries.sort_by_key(|e| e.0);
    let header_offset = 16 + body.len() as u32;
    let mut img = b"MIX1".to_vec();
    le(&mut img, header_offset);
    le(&mut img, 0);
    le(&mut img, 0);
    img.extend(body);
    le(&mut img, nfiles as u32);
    for e in &entries {
        le(&mut img, e.0);
        le(&mut img, e.1);
        le(&mut img, e.2);
    }
    let names_offset = img.len() as u32;
    le(&mut img, nfiles as u32);
    for e in &entries {
        img.push(e.3.len() as u8 + 1);
        img.extend_from_slice(e.3.as_bytes());
        img.push(0);
    }
    img[8..12].copy_from_slice(&names_offset.to_le_bytes());
    let table = header_offset as usize + 4;
    let mut truncate_at_end = false;
    for _ in 0..g.below(3) {
        match g.below(7) {
            0 => img[g.below(4) as usize] = g.below(256) as u8,
            1 => {
                let v = header_offset.wrapping_add(g.below(30)).wrapping_sub(10);
                img[4..8].copy_from_slice(&v.to_le_bytes());
            }
            2 => {
                let v = (nfiles as u32 + g.below(3)).saturating_sub(g.below(2));
                img[header_offset as usize..header_offset as usize + 4].copy_from_slice(&v.to_le_bytes());
            }
            3 => {
                if nfiles > 0 {
                    let e = table + 12 * g.below(nfiles as u32) as usize;
                    let which = 4 + 4 * g.below(2) as usize;
                    let v = g.below(img.len() as u32 + 60);
                    img[e + which..e + which + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            4 => truncate_at_end = true,
            5 => {
                let v = g.below(img.len() as u32 + 20);
                img[8..12].copy_from_slice(&v.to_le_bytes());
            }
            _ => {
                let v = (nfiles as u32).saturating_sub(g.below(3));
                if names_offset as usize + 4 <= img.len() {
                    img[names_offset as usize..names_offset as usize + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    if truncate_at_end {
        let n = g.below(img.len() as u32 + 1) as usize;
        img.truncate(n);
    }
    img
}

/// How many names the draft's name-table reader returns for `img`, and whether the table's offsets are pairwise different.
fn names_ok(img: &[u8]) -> bool {
    if img.len() < 12 || &img[0..4] != b"MIX1" {
        return true;
    }
    let header_offset = rd(img, 4) as i32;
    let names_offset = rd(img, 8) as i32;
    if header_offset < 0 || (header_offset as usize + 4 > img.len()) {
        return true;
    }
    let count = rd(img, header_offset as usize) as i32;
    if count < 0 || count > 1000 {
        return false;
    }
    let table_end = header_offset as usize + 4 + 12 * count as usize;
    if table_end > img.len() {
        return true; // invalid: the factory discards the table
    }
    let mut offs: Vec<u32> = (0..count as usize).map(|i| rd(img, header_offset as usize + 4 + 12 * i + 4)).collect();
    offs.sort();
    if offs.windows(2).any(|w| w[0] == w[1]) {
        return false;
    }
    // the names the reader finds
    if names_offset < 0 || names_offset as usize + 4 > img.len() {
        return true;
    }
    let mut n = 0;
    let mut at = names_offset as usize + 4;
    let nn = rd(img, names_offset as usize) as i32;
    let mut i = 0;
    while i < nn {
        if at >= img.len() {
            break;
        }
        let l = img[at] as usize;
        if at + 1 + l > img.len() {
            break;
        }
        if !img[at + 1..at + 1 + l].iter().all(|&b| b < 128) {
            return false;
        }
        n += 1;
        at += 1 + l;
        i += 1;
    }
    n <= count
}

/// A structured flow: make an archive, open it, build the internal name list, then delete and add names (case variants of the ones in the
/// archive among them), perhaps with the temporary names taken, flush, open the result and look at everything.
fn gen_scenario(g: &mut Gen, pool: &[String]) -> Vec<i32> {
    let np = pool.len() as u32;
    let mut made: Vec<usize> = Vec::new();
    let mut body = Vec::new();
    // "a.txt" or "A.TXT" first: the two spellings have one CRC and compare equal without case
    let mut candidates = vec![g.below(2) as usize];
    for _ in 0..g.below(5) {
        candidates.push(g.below(np) as usize);
    }
    for n in candidates {
        let crc = crc_stringi(pool[n].as_bytes(), 0);
        if made.iter().any(|&m| crc_stringi(pool[m].as_bytes(), 0) == crc) {
            continue;
        }
        made.push(n);
        body.extend([g.below(7) as i32, n as i32, 0]);
    }
    let mut w = vec![0, made.len() as i32];
    w.extend(body);
    w.extend([1, 3]);
    if g.below(4) == 0 {
        w.extend([7, if g.below(2) == 0 { 20 } else { 1 + g.below(20) as i32 }]);
    }
    let variant = |g: &mut Gen, n: usize| -> i32 {
        if n < 2 && g.below(2) == 0 {
            1 - n as i32
        } else {
            n as i32
        }
    };
    for _ in 0..1 + g.below(3) {
        let n = made[g.below(made.len() as u32) as usize];
        if g.below(2) == 0 {
            w.extend([4, variant(g, n)]);
        } else {
            let target = if g.below(3) == 0 { g.below(np) as i32 } else { variant(g, n) };
            w.extend([5, g.below(7) as i32, target]);
        }
    }
    w.extend([6, 1, 3, 2]);
    w
}

fn gen_prog(g: &mut Gen, pool: &[String]) -> Vec<i32> {
    if g.below(100) < 35 {
        return gen_scenario(g, pool);
    }
    let mut w = Vec::new();
    let np = pool.len() as u32;
    let mut first = true;
    for _ in 0..1 + g.below(14) {
        let choice = if first { if g.below(10) < 7 { 0 } else { 8 } } else { g.below(20) };
        first = false;
        match choice {
            0 => {
                let k = 1 + g.below(6) as usize;
                let mut crcs: Vec<u32> = Vec::new();
                let mut body = Vec::new();
                let mut count = 0;
                let mut srcs_form1: Vec<i32> = Vec::new();
                for _ in 0..k {
                    let src = g.below(8) as i32;
                    let name = g.below(np) as usize;
                    let form = g.below(3) as i32;
                    // form 1 saves the source's own path (the directory is part of the name, so its CRC is not known here: at most one source
                    // per archive, which cannot equal a pool name's CRC in practice)
                    if form == 1 {
                        if !srcs_form1.is_empty() {
                            continue;
                        }
                        srcs_form1.push(src);
                    } else {
                        let crc = crc_stringi(pool[name].as_bytes(), 0);
                        if crcs.contains(&crc) {
                            continue;
                        }
                        crcs.push(crc);
                    }
                    count += 1;
                    body.extend([src, name as i32, form]);
                }
                w.extend([0, count as i32]);
                w.extend(body);
            }
            1..=2 => w.push(1),
            3..=5 => w.push(2),
            6..=7 => w.push(3),
            8 | 9 => {
                for _ in 0..8 {
                    let img = build_image(g, pool);
                    if names_ok(&img) {
                        w.extend([8, img.len() as i32]);
                        w.extend(img.iter().map(|&b| b as i32));
                        break;
                    }
                }
            }
            10 | 11 => w.extend([4, g.below(np) as i32]),
            12 | 13 => w.extend([5, g.below(8) as i32, g.below(np) as i32]),
            14 | 15 => w.push(6),
            16 => w.extend([7, 1 + g.below(20) as i32]),
            _ => w.push(1),
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

fn run_rust(dir: &str, pool: &[String], prog: &[i32]) -> Vec<i32> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    for (i, &len) in SIZES.iter().enumerate() {
        if i != 7 {
            std::fs::write(format!("{dir}s{i}"), source_bytes(i as i32, len)).unwrap();
        }
    }
    let mix = format!("{dir}pkg.mix");
    let mut out = Vec::new();
    let mut mf: Option<MixFileFactoryClass> = None;
    let mut raw = false;
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
                let k = next();
                let mut c = MixFileCreator::new(&mix);
                for _ in 0..k {
                    let (src, name, form) = (next(), next() as usize, next());
                    let sp = format!("{dir}s{src}");
                    match form {
                        0 => c.add_file(&sp, Some(&pool[name])),
                        1 => c.add_file(&sp, None),
                        _ => c.add_file_data(&pool[name], &std::fs::read(&sp).unwrap_or_default()),
                    }
                }
                drop(c);
                mf = None;
                raw = false;
                out.push(-10);
            }
            1 => {
                let f = MixFileFactoryClass::new(&mix);
                out.push(f.is_valid() as i32);
                mf = Some(f);
            }
            2 => match mf.as_ref() {
                None => out.push(-6),
                Some(f) => {
                    for n in pool {
                        match f.get_file(n) {
                            None => out.push(0),
                            Some(sub) => {
                                out.push(1);
                                out.push(sub.size as i32);
                                let b = sub.read_all().unwrap_or_default();
                                out.push(b.len() as i32);
                                out.extend(b.iter().take(64).map(|&x| x as i32));
                            }
                        }
                    }
                }
            },
            3 => match mf.as_mut() {
                None => out.push(-6),
                Some(f) => {
                    let mut l1 = Vec::new();
                    out.push(f.build_filename_list(&mut l1) as i32);
                    put_names(&mut out, &l1);
                    let mut l2 = Vec::new();
                    out.push(f.build_ordered_filename_list(&mut l2) as i32);
                    put_names(&mut out, &l2);
                    out.push(f.build_internal_filename_list() as i32);
                    let l3 = f.get_filename_list().clone();
                    put_names(&mut out, &l3);
                }
            },
            4 => {
                let a = next() as usize;
                if let Some(f) = mf.as_mut() {
                    f.delete_file(&pool[a]);
                }
            }
            5 => {
                let (src, a) = (next(), next() as usize);
                if let Some(f) = mf.as_mut() {
                    f.add_file(&format!("{dir}s{src}"), &pool[a]);
                }
            }
            6 => {
                if raw {
                    out.push(-13);
                    continue;
                }
                if let Some(f) = mf.as_mut() {
                    f.flush_changes();
                }
                mf = None;
                out.push(-12);
            }
            7 => {
                let k = next();
                for j in 1..=k {
                    std::fs::write(format!("{dir}_tmpmix{j:02}.dat"), [7u8, 7, 7]).unwrap();
                }
            }
            _ => {
                let len = next();
                let b: Vec<u8> = (0..len).map(|_| next() as u8).collect();
                std::fs::write(&mix, b).unwrap();
                mf = None;
                raw = true;
            }
        }
    }
    match std::fs::read(&mix) {
        Ok(img) => {
            out.push(img.len() as i32);
            out.extend(img.iter().map(|&b| b as i32));
        }
        Err(_) => out.push(-1),
    }
    out
}

fn run_cpp(dir: &str, pool: &[String], prog: &[i32]) -> Vec<i32> {
    let mut nb = Vec::new();
    for n in pool {
        nb.extend_from_slice(n.as_bytes());
        nb.push(0);
    }
    let mut d = dir.as_bytes().to_vec();
    d.push(0);
    let mut out = vec![0i32; 1 << 20];
    let n = unsafe { ref_mx_run(d.as_ptr(), SIZES.as_ptr(), SIZES.len() as i32, nb.as_ptr(), pool.len() as i32, prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn mixfile_matches_upstream_over_random_programs() {
    let pool = pool();
    let dir = format!("{}/mx_draft_{}/", std::env::temp_dir().display(), std::process::id());
    let n = cases(3_000);
    let mut g = Gen::new(0x215_18263);
    let mut c = Check::new("MixFile programs (results, bytes, archive)");
    for i in 0..n {
        let mut prog = gen_prog(&mut g, &pool);
        // the form 1 saved name contains the directory: patch nothing, the CRC test above used a placeholder (the directory is the same on both sides)
        if prog.is_empty() {
            prog.push(1);
        }
        if std::env::var("DRAFT_TRACE").is_ok() {
            eprintln!("case {i}: {:?}", prog);
        }
        let cpp = run_cpp(&dir, &pool, &prog);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&dir, &pool, &prog))).ok();
        let same = rs.as_ref() == Some(&cpp);
        c.eq(same, true, || {
            let at = rs.as_ref().map_or(0, |r| r.iter().zip(cpp.iter()).take_while(|(a, b)| a == b).count());
            let rsv = rs.as_ref().map(|r| r.iter().skip(at.saturating_sub(3)).take(8).copied().collect::<Vec<_>>());
            let cv: Vec<i32> = cpp.iter().skip(at.saturating_sub(3)).take(8).copied().collect();
            format!("{} -- first difference at word {at} (lengths {:?} / {}): rust {:?} c++ {:?}", format!("case {i}: program {:?}", prog), rs.as_ref().map(|r| r.len()), cpp.len(), rsv, cv)
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
