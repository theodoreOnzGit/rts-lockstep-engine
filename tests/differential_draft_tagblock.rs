#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWLib/TagBlock.h, tagblock.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::wwlib::tagblock` against the upstream C++ (#215, group wwlib4).
//!
//! # Method
//!
//! `differential/wrap_draft_tagblock.cpp` compiles `TagBlock.h` and `tagblock.cpp` unchanged over a stand-in `RawFileClass` (an in-memory
//! file system with the Win32 open/seek/read/write semantics; `differential/shim_draft_wwlib4/rawfile.h`). The Rust draft runs on a real
//! file in the temp directory. Each case starts from a file image (absent; a well-formed image of 0 to 5 blocks, some with extra bytes after
//! the tag name's NUL; or such an image damaged: wrong version, block count, file size or block index, truncated, extended, a byte
//! overwritten, a data size changed; names include case variants, an empty name, a 200 byte name, a UTF-8 name, a pair of different names
//! with the SAME `CRC_Stringi` and, in images only, a name that is not UTF-8) and then runs 1 to 60 operations on 4 handle slots:
//! `Does_Tag_Exist`, `Open_Tag`, `Create_Tag`, `Close_Tag`, `Write`, `Read`, `Seek` (all three directions, also past the end),
//! `End_Write_Access`, `Reset_File` and re-opening the file (the constructor's index rebuild and header repair). Every result, every byte
//! read, `Tell` and `Get_Data_Size` are compared, and at the end the whole final file image, bit for bit.
//!
//! **Excluded inputs (undefined upstream):** an image whose scanned block has a tag size outside 1..=1024, a tag name without a NUL in
//! its tag size, or a header or tag cut short by the end of the file (upstream then reads an uninitialised stack buffer), or that sends the
//! scan to a negative offset or beyond 64 KiB (the file would grow by that much): such images are not generated (the generator tests the scan's own conditions). `Get_Date_Time` (the file's
//! time stamp, the operating system's) is not compared. `Reset_File` and re-opening happen only with every handle closed (upstream asserts).
//! A negative position after `Seek` is put back to 0 before any I/O on both sides (upstream would seek before the file's start: an error).
//!
//! # Predictions and results
//!
//! Predicted before the first run: a mismatch on images whose tag name is not UTF-8 (the draft decoded names lossily, so its CRC and
//! data offset differ from upstream's); otherwise 0 mismatches.
//!
//! **Results (2026-11-20).** The lossy decoding was found by reading while the test was written and corrected before the first run (a tag name read
//! back from a file is kept as bytes), so its effect was not measured. First run that compared: 0 mismatches over 20,000 programs (1.77 million
//! program words: images of 0 to 5 blocks, 1 to 60 operations each; 1 in 12 image names is not UTF-8). Planted bugs (mismatching
//! programs of 20,000): 18261 index list in ascending CRC order: 9,037; 18262 the search stops at an equal CRC: 490 (needs the two names with one
//! CRC); 18263 a read not clipped to the block's end: 3,407; 18264 file size without the data size: 9,250; 18265 tag size without the NUL: 17,729;
//! 18266 `SEEK_END` subtracts: 2,364; 18267 data size grows by each write instead of reaching the position: 1,109; 18268 repair only when both the
//! size and the count disagree: 2,130; 18269 name compare case-sensitive: 6,431; 18270 a second creation handle allowed: 12,054. 10 of 10 caught.

mod support;

use rts_lockstep_engine::drafts::wwlib::tagblock::{SeekDir, TagBlockFile, TagBlockHandle};
use rts_lockstep_engine::ww::realcrc::crc_stringi;
use std::collections::HashMap;
use support::{Check, Gen};

extern "C" {
    fn ref_tb_run(image: *const u8, ilen: i32, names: *const u8, nnames: i32, prog: *const i32, nwords: i32, out: *mut i32, cap: i32) -> i32;
}

const FILE_VERSION: u32 = 20000626;

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_TB_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Two different names (as lower case letters) with the same `CRC_Stringi`.
fn collision() -> (String, String) {
    let mut seen: HashMap<u32, String> = HashMap::new();
    let mut g = Gen::new(0x215_18261);
    loop {
        let len = 5 + g.below(3) as usize;
        let s: String = (0..len).map(|_| (b'a' + g.below(26) as u8) as char).collect();
        let c = crc_stringi(s.as_bytes(), 0);
        if let Some(o) = seen.get(&c) {
            if *o != s {
                return (o.clone(), s);
            }
        }
        seen.insert(c, s);
    }
}

fn pool() -> Vec<String> {
    let (a, b) = collision();
    assert_eq!(crc_stringi(a.as_bytes(), 0), crc_stringi(b.as_bytes(), 0));
    let mut v: Vec<String> = ["alpha", "ALPHA", "Beta", "", "gamma", "Gamma", "delta", "caf\u{e9}"].iter().map(|s| s.to_string()).collect();
    v.push("x".repeat(200));
    v.push(a);
    v.push(b);
    v
}

fn le(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}

fn rd(img: &[u8], at: i64) -> i32 {
    let mut b = [0u8; 4];
    for (i, x) in b.iter_mut().enumerate() {
        let p = at + i as i64;
        *x = if p >= 0 && (p as usize) < img.len() { img[p as usize] } else { 0 };
    }
    i32::from_le_bytes(b)
}

/// The constructor's scan on `img`: true when it never reads an uninitialised buffer or seeks before the start.
fn safe(img: &[u8]) -> bool {
    let version = rd(img, 0) as u32;
    if version != FILE_VERSION {
        return true;
    }
    let nblocks = rd(img, 4);
    let mut cur: i64 = 12;
    for block in 0..nblocks.max(0) {
        if !(0..(1i64 << 16)).contains(&cur) {
            return false;
        }
        let (index, tag_size, data_size) = (rd(img, cur), rd(img, cur + 4), rd(img, cur + 8));
        if index != block {
            break;
        }
        if !(1..=1024).contains(&tag_size) || cur + 12 + tag_size as i64 > img.len() as i64 {
            return false;
        }
        let name = &img[(cur + 12) as usize..(cur + 12 + tag_size as i64) as usize];
        if !name.contains(&0) {
            return false;
        }
        cur = cur + 12 + tag_size as i64 + data_size as i64;
    }
    (0..(1i64 << 16)).contains(&cur)
}

fn build_image(g: &mut Gen, pool: &[String]) -> Vec<u8> {
    let k = g.below(6) as i32;
    let mut body = Vec::new();
    for i in 0..k {
        let name: Vec<u8> = if g.below(12) == 0 { b"bad\xff\xfe".to_vec() } else { pool[g.below(pool.len() as u32) as usize].as_bytes().to_vec() };
        let extra = if g.below(8) == 0 { g.below(4) as i32 } else { 0 };
        let tag_size = name.len() as i32 + 1 + extra;
        let data_size = g.below(40) as i32;
        le(&mut body, i);
        le(&mut body, tag_size);
        le(&mut body, data_size);
        body.extend_from_slice(&name);
        body.extend(std::iter::repeat(0u8).take(1 + extra as usize));
        body.extend((0..data_size).map(|_| g.below(256) as u8));
    }
    let mut img = Vec::new();
    img.extend_from_slice(&FILE_VERSION.to_le_bytes());
    le(&mut img, k);
    le(&mut img, 12 + body.len() as i32);
    img.extend(body);
    // damage
    for _ in 0..g.below(3) {
        if g.below(3) == 0 {
            continue;
        }
        let kind = g.below(8);
        if kind <= 2 && img.len() < 12 {
            continue;
        }
        match kind {
            0 => {
                let v = if g.below(2) == 0 { FILE_VERSION.wrapping_add(1) } else { g.u32() };
                img[0..4].copy_from_slice(&v.to_le_bytes());
            }
            1 => {
                let v = rd(&img, 4) + g.below(6) as i32 - 2;
                img[4..8].copy_from_slice(&v.to_le_bytes());
            }
            2 => {
                let v = rd(&img, 8) + g.below(41) as i32 - 20;
                img[8..12].copy_from_slice(&v.to_le_bytes());
            }
            3 => {
                if img.len() > 12 {
                    let at = 12 + g.below((img.len() - 12) as u32) as usize;
                    if at + 4 <= img.len() {
                        let v = g.below(5) as i32;
                        img[at..at + 4].copy_from_slice(&v.to_le_bytes());
                    }
                }
            }
            4 => {
                let n = g.below(img.len() as u32 + 1) as usize;
                img.truncate(n);
            }
            5 => {
                for _ in 0..g.below(30) {
                    img.push(g.below(256) as u8);
                }
            }
            6 => {
                if !img.is_empty() {
                    let at = g.below(img.len() as u32) as usize;
                    img[at] = g.below(256) as u8;
                }
            }
            _ => {
                if img.len() >= 24 {
                    let v = g.below(60) as i32 - 5;
                    img[20..24].copy_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    img
}

fn gen_image(g: &mut Gen, pool: &[String]) -> Option<Vec<u8>> {
    if g.below(5) == 0 {
        return None;
    }
    for _ in 0..8 {
        let img = build_image(g, pool);
        if safe(&img) {
            return Some(img);
        }
    }
    Some(Vec::new())
}

fn gen_prog(g: &mut Gen, np: usize) -> Vec<i32> {
    let mut w = Vec::new();
    for _ in 0..1 + g.below(60) {
        let name = g.below(np as u32) as i32;
        let slot = g.below(4) as i32;
        match g.below(80) {
            0..=7 => w.extend([0, name]),
            8..=21 => w.extend([1, name, slot]),
            22..=35 => w.extend([2, name, slot]),
            36..=45 => w.extend([3, slot]),
            46..=59 => w.extend([4, slot, g.below(70) as i32, g.below(1000) as i32]),
            60..=71 => w.extend([5, slot, g.below(80) as i32]),
            72..=75 => {
                let d = g.below(3) as i32;
                let pos = match d {
                    1 => g.below(100) as i32,
                    0 => g.below(40) as i32 - 10,
                    _ => g.below(40) as i32 - 20,
                };
                w.extend([6, slot, pos, d]);
            }
            76 | 77 => w.extend([7, slot]),
            78 => w.push(8),
            _ => w.push(9),
        }
    }
    w
}

fn put_all(out: &mut Vec<i32>, bytes: &[u8]) {
    out.push(bytes.len() as i32);
    out.extend(bytes.iter().map(|&b| b as i32));
}

fn run_rust(path: &std::path::Path, image: &Option<Vec<u8>>, names: &[String], prog: &[i32]) -> Vec<i32> {
    let _ = std::fs::remove_file(path);
    if let Some(img) = image {
        std::fs::write(path, img).unwrap();
    }
    let p = path.to_str().unwrap();
    let mut f = TagBlockFile::new(p).unwrap();
    let mut h: [Option<TagBlockHandle>; 4] = [None; 4];
    let mut out = Vec::new();
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
            0 => out.push(f.does_tag_exist(&names[next() as usize]) as i32),
            1 => {
                let (a, s) = (next() as usize, next() as usize);
                if h[s].is_some() {
                    out.push(-5);
                } else {
                    h[s] = f.open_tag(&names[a]);
                    out.push(h[s].as_ref().map_or(0, |x| 1 + x.get_data_size()));
                }
            }
            2 => {
                let (a, s) = (next() as usize, next() as usize);
                if h[s].is_some() {
                    out.push(-5);
                } else {
                    h[s] = f.create_tag(&names[a]);
                    out.push(h[s].is_some() as i32);
                }
            }
            3 => {
                let s = next() as usize;
                match h[s].take() {
                    Some(x) => {
                        f.close_tag(x);
                        out.push(-7);
                    }
                    None => out.push(-6),
                }
            }
            4 => {
                let (s, len, seed) = (next() as usize, next(), next());
                match h[s].as_mut() {
                    None => out.push(-6),
                    Some(x) => {
                        let b: Vec<u8> = (0..len).map(|i| (seed.wrapping_mul(31).wrapping_add(i * 7)) as u8).collect();
                        let r = x.write(&mut f, &b);
                        out.extend([r, x.tell(), x.get_data_size()]);
                    }
                }
            }
            5 => {
                let (s, len) = (next() as usize, next());
                match h[s].as_mut() {
                    None => out.push(-6),
                    Some(x) => {
                        let mut b = vec![0u8; len as usize];
                        let r = x.read(&mut f, &mut b);
                        out.extend([r, x.tell()]);
                        out.extend(b[..r as usize].iter().map(|&c| c as i32));
                    }
                }
            }
            6 => {
                let (s, pos, d) = (next() as usize, next(), next());
                match h[s].as_mut() {
                    None => out.push(-6),
                    Some(x) => {
                        let r = x.seek(pos, [SeekDir::Cur, SeekDir::Set, SeekDir::End][d as usize]);
                        if r < 0 {
                            x.seek(0, SeekDir::Set);
                        }
                        out.push(r);
                    }
                }
            }
            7 => {
                let s = next() as usize;
                match h[s].as_mut() {
                    None => out.push(-6),
                    Some(x) => out.push(x.end_write_access(&mut f) as i32),
                }
            }
            8 => {
                if h.iter().all(|x| x.is_none()) {
                    f.reset_file();
                }
                out.push(-8);
            }
            _ => {
                if h.iter().all(|x| x.is_none()) {
                    f = TagBlockFile::new(p).unwrap();
                }
                out.push(-9);
            }
        }
    }
    for x in h.iter_mut() {
        if let Some(x) = x.take() {
            f.close_tag(x);
        }
    }
    drop(f);
    let img = std::fs::read(path).unwrap();
    put_all(&mut out, &img);
    out
}

fn run_cpp(image: &Option<Vec<u8>>, names: &[String], prog: &[i32]) -> Vec<i32> {
    let mut nb = Vec::new();
    for n in names {
        nb.extend_from_slice(n.as_bytes());
        nb.push(0);
    }
    let (ip, il) = match image {
        Some(v) => (v.as_ptr(), v.len() as i32),
        None => (std::ptr::null(), -1),
    };
    let mut out = vec![0i32; 1 << 21];
    let n = unsafe { ref_tb_run(ip, il, nb.as_ptr(), names.len() as i32, prog.as_ptr(), prog.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
    assert!(n <= out.len());
    out.truncate(n);
    out
}

#[test]
fn tagblock_matches_upstream_over_random_programs() {
    let names = pool();
    let path = std::env::temp_dir().join(format!("tb_draft_{}.dat", std::process::id()));
    let n = cases(20_000);
    let mut g = Gen::new(0x215_18262);
    let mut c = Check::new("TagBlockFile programs (results, bytes, image)");
    let mut ops = 0u64;
    for i in 0..n {
        let image = gen_image(&mut g, &names);
        let prog = gen_prog(&mut g, names.len());
        ops += prog.len() as u64;
        if std::env::var("DRAFT_TRACE").is_ok() {
            eprintln!("case {i}: {:?}", prog);
        }
        let cpp = run_cpp(&image, &names, &prog);
        let rs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_rust(&path, &image, &names, &prog))).ok();
        let same = rs.as_ref() == Some(&cpp);
        c.eq(same, true, || {
            let at = rs.as_ref().map_or(0, |r| r.iter().zip(cpp.iter()).take_while(|(a, b)| a == b).count());
            let rsv = rs.as_ref().map(|r| r.iter().skip(at.saturating_sub(3)).take(8).copied().collect::<Vec<_>>());
            let cv: Vec<i32> = cpp.iter().skip(at.saturating_sub(3)).take(8).copied().collect();
            format!("{} -- first difference at word {at} (lengths {:?} / {}): rust {:?} c++ {:?}", format!("case {i}: image {:?}, program {:?}", image, prog), rs.as_ref().map(|r| r.len()), cpp.len(), rsv, cv)
        });
    }
    let _ = std::fs::remove_file(&path);
    println!("tagblock: {n} programs, {ops} program words");
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}
