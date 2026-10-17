// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `formats::big` against upstream
//! `Win32BIGFileSystem.cpp`, `Win32BIGFile.cpp`, `ArchiveFile.cpp`,
//! `ArchiveFileSystem.cpp`, `RAMFile.cpp`, `StreamingArchiveFile.cpp` and
//! `File.cpp` (commit 0a05454d), compiled unchanged by g++ (see
//! `differential/wrap_big.cpp`).
//!
//! No game assets are available, so every archive is **synthetic**, built
//! byte by byte in the layout the upstream parser reads (identifier,
//! little-endian size, big-endian count, entries from 0x10 with big-endian
//! offset and size and a NUL-terminated path). The generator mixes valid
//! archives with the cases the parser must cope with: wrong identifiers,
//! counts that are zero, negative or partial, mixed and doubled separators,
//! upper case, names without a dot or with an empty file name, dotted
//! directory names, duplicate paths, paths of exactly 259 bytes, members
//! whose data lies outside the archive or has size 0, and archives truncated
//! anywhere.
//!
//! Each case loads 1-4 archives (some with the same name, random
//! `overwrite`) into the real `Win32BIGFileSystem` and into the Rust
//! `ArchiveFileSystem`, then compares, for ~40 query paths (members' paths
//! with case and separator changes, directories, unknown names):
//! `doesFileExist`, `getArchiveFilenameForFile`, `openFile` (RAM and
//! streaming: bytes, size) and `getFileInfo`, and, for ~8 directory/mask
//! pairs, `getFileListInDirectory` (names and order).
//!
//! Archives that the Rust rejects as truncated, too short for the
//! identifier, or with a path over 259 bytes are **not** given to the C++,
//! because the C++ is undefined there (it reads uninitialised stack bytes or
//! overruns its path buffer); their count is printed. A non-streaming open of
//! a member that declares more than 16 MiB is not run in the C++ either:
//! `RAMFile::openFromArchive` allocates the declared size before reading it
//! (and throws `std::bad_array_new_length` at 2^31 or more, which would crash
//! the game); the Rust is checked to refuse it. Such members arise when a
//! count larger than the real table makes the parser read entries from the
//! data region; the count of these opens is printed. Offsets of
//! `0xFFFFFFFF` and sizes of 2^31 or more are not generated (see the module
//! docs of `formats::big`).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::big::{ArchiveFileSystem, BigError, FilenameList};
use std::ffi::CString;
use support::Gen;

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn c(s: &[u8]) -> CString {
    CString::new(s.to_vec()).unwrap()
}

fn pick<'a>(g: &mut Gen, xs: &'a [&'a [u8]]) -> &'a [u8] {
    xs[g.below(xs.len() as u32) as usize]
}

fn gen_segment(g: &mut Gen) -> Vec<u8> {
    const NAMES: &[&[u8]] = &[
        b"Data",
        b"data",
        b"INI",
        b"Art",
        b"Textures",
        b"W3D",
        b"a.b",
        b"Maps",
        b"x",
    ];
    if g.below(3) == 0 {
        let n = 1 + g.below(8) as usize;
        (0..n)
            .map(|_| match g.below(20) {
                0 => 0x80 + g.below(0x80) as u8,
                1 => b'_',
                _ => {
                    b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                        [g.below(62) as usize]
                }
            })
            .collect()
    } else {
        pick(g, NAMES).to_vec()
    }
}

fn gen_sep(g: &mut Gen) -> Vec<u8> {
    match g.below(10) {
        0 => b"\\\\".to_vec(),
        1 => b"/\\".to_vec(),
        2..=5 => b"/".to_vec(),
        _ => b"\\".to_vec(),
    }
}

fn gen_path(g: &mut Gen) -> Vec<u8> {
    let mut p = Vec::new();
    if g.below(10) == 0 {
        p.extend(gen_sep(g));
    }
    for _ in 0..g.below(4) {
        p.extend(gen_segment(g));
        p.extend(gen_sep(g));
    }
    match g.below(12) {
        0 => {}                        // empty file name (path ends with a separator)
        1 => p.extend(gen_segment(g)), // maybe no dot
        _ => {
            p.extend(gen_segment(g));
            const EXT: &[&[u8]] = &[b".ini", b".INI", b".tga", b".w3d", b".", b".big", b".txt"];
            p.extend_from_slice(pick(g, EXT));
        }
    }
    if g.below(40) == 0 {
        // exactly at, or just over, the 259-byte limit of char buffer[_MAX_PATH]
        let target = if g.below(2) == 0 {
            259
        } else {
            260 + g.below(5) as usize
        };
        while p.len() < target {
            p.insert(0, b'q');
        }
        p.truncate(target);
    }
    p
}

/// A synthetic archive, built byte by byte.
fn gen_archive(g: &mut Gen) -> Vec<u8> {
    if g.below(25) == 0 {
        // short headers: identifier plus 0-15 bytes
        let mut v = b"BIGF".to_vec();
        for _ in 0..g.below(16) {
            v.push(g.u32() as u8);
        }
        return v;
    }
    let n = g.below(10) as usize;
    let mut members: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for _ in 0..n {
        let path = if !members.is_empty() && g.below(8) == 0 {
            // a duplicate path, possibly with another case
            let mut p = members[g.below(members.len() as u32) as usize].0.clone();
            if g.below(2) == 0 {
                p = p.to_ascii_uppercase();
            }
            p
        } else {
            gen_path(g)
        };
        let data: Vec<u8> = (0..g.below(40)).map(|_| g.u32() as u8).collect();
        members.push((path, data));
    }
    let table: usize = members.iter().map(|(p, _)| 8 + p.len() + 1).sum();
    let mut offset = 0x10 + table;
    let mut out = Vec::new();
    out.extend_from_slice(match g.below(30) {
        0 => b"BIGG",
        1 => b"big4",
        2 => b"BIG\0",
        _ => b"BIGF",
    });
    out.extend_from_slice(&g.u32().to_le_bytes());
    let count: u32 = match g.below(20) {
        0 => n as u32 + 1 + g.below(3),  // more than present: truncated
        1 => n.saturating_sub(1) as u32, // fewer: trailing entries ignored
        2 => 0x8000_0000 | g.u32(),      // negative: no files
        _ => n as u32,
    };
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&g.u32().to_le_bytes()); // the word at 12 is never read
    let mut layout = Vec::new();
    for (p, d) in &members {
        let (mut o, mut s) = (offset as u32, d.len() as u32);
        match g.below(15) {
            0 => o = o.wrapping_add(1000 + g.below(1000)), // data past the end
            1 => s += 1 + g.below(50),                     // runs past the end
            2 => s = 0,                                    // empty member
            3 => o = 0x8000_0000 + g.below(0x7FFF_FFFE),   // offset >= 2^31 (never 0xFFFFFFFF)
            4 => o = g.below(offset as u32 + 64),          // overlapping the header or table
            _ => {}
        }
        out.extend_from_slice(&o.to_be_bytes());
        out.extend_from_slice(&s.to_be_bytes());
        out.extend_from_slice(p);
        out.push(0);
        layout.push(d.clone());
        offset += d.len();
    }
    for d in layout {
        out.extend(d);
    }
    if g.below(8) == 0 && !out.is_empty() {
        out.truncate(g.below(out.len() as u32 + 1) as usize);
    }
    out
}

/// Query paths: the members' paths, changed in case and separators; their
/// directories; unknown names.
fn gen_queries(g: &mut Gen, archives: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut paths: Vec<Vec<u8>> = Vec::new();
    for a in archives {
        let mut pos = 0x10usize;
        while pos + 8 < a.len() {
            pos += 8;
            let end = match a[pos..].iter().position(|&b| b == 0) {
                Some(e) => pos + e,
                None => break,
            };
            paths.push(a[pos..end].to_vec());
            pos = end + 1;
        }
    }
    let mut q = Vec::new();
    for _ in 0..40 {
        let mut p = if !paths.is_empty() && g.below(4) != 0 {
            paths[g.below(paths.len() as u32) as usize].clone()
        } else {
            gen_path(g)
        };
        match g.below(6) {
            0 => p = p.to_ascii_uppercase(),
            1 => p.iter_mut().for_each(|b| {
                if *b == b'/' {
                    *b = b'\\'
                }
            }),
            2 => {
                if let Some(k) = p.iter().rposition(|&b| b == b'/' || b == b'\\') {
                    p.truncate(k + if g.below(2) == 0 { 1 } else { 0 });
                }
            }
            _ => {}
        }
        q.push(p);
    }
    q.push(Vec::new());
    q
}

/// The C++ list: each name NUL-terminated.
fn split_nul(buf: &[u8]) -> Vec<Vec<u8>> {
    match buf.split_last() {
        None => Vec::new(),
        Some((_, names)) => names.split(|&b| b == 0).map(|s| s.to_vec()).collect(),
    }
}

#[test]
fn big_archives_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(400);
    let (mut archives_total, mut archives_to_cpp, mut ub_skipped) = (0, 0, 0);
    let (mut queries, mut opened, mut listed_names, mut big_declared) = (0u64, 0u64, 0u64, 0u64);
    const CASES: usize = 1500;
    let mut buf = vec![0u8; 1 << 16];
    for case in 0..CASES {
        let mut rust = ArchiveFileSystem::new();
        unsafe { cpp::ref_big_reset() };
        let mut loaded: Vec<Vec<u8>> = Vec::new();
        for _ in 0..1 + g.below(4) {
            let name = format!(
                "Archive{}.{}",
                g.below(3),
                if g.below(2) == 0 { "big" } else { "BIG" }
            )
            .into_bytes();
            let data = gen_archive(&mut g);
            let overwrite = g.below(2) == 0;
            archives_total += 1;
            let r = rust.load_big_file(&name, data.clone(), overwrite);
            match r {
                Ok(()) | Err(BigError::BadIdentifier) => {
                    archives_to_cpp += 1;
                    let cname = c(&name);
                    let ok = unsafe {
                        cpp::ref_big_add_local(
                            cname.as_ptr(),
                            data.as_ptr() as *const _,
                            data.len() as i32,
                        );
                        cpp::ref_big_load(cname.as_ptr(), i32::from(overwrite))
                    };
                    assert_eq!(
                        ok == 1,
                        r.is_ok(),
                        "case {case}: load of {:?} differs (rust {r:?})",
                        String::from_utf8_lossy(&name)
                    );
                    if r.is_ok() {
                        loaded.push(data);
                    }
                }
                Err(_) => ub_skipped += 1,
            }
        }

        for q in gen_queries(&mut g, &loaded) {
            queries += 1;
            let cq = c(&q);
            let d = || format!("case {case}, query {:?}", String::from_utf8_lossy(&q));
            let ex = unsafe { cpp::ref_big_exists(cq.as_ptr()) } == 1;
            assert_eq!(rust.does_file_exist(&q), ex, "doesFileExist: {}", d());

            let n = unsafe {
                cpp::ref_big_archive_for(cq.as_ptr(), buf.as_mut_ptr() as *mut _, buf.len() as i32)
            };
            assert_eq!(
                rust.get_archive_filename_for_file(&q),
                buf[..n as usize].to_vec(),
                "getArchiveFilenameForFile: {}",
                d()
            );

            let mut size = 0i32;
            // RAMFile::openFromArchive allocates the declared size before it
            // reads (and `new Char[size]` throws for sizes >= 2^31), so a
            // member declaring more than 16 MiB is not opened in the C++.
            let declared = rust
                .archive(&rust.get_archive_filename_for_file(&q))
                .and_then(|a| a.get_archived_file_info(&q))
                .map_or(0, |i| i.size);
            if declared > 16 << 20 {
                big_declared += 1;
                assert_eq!(
                    rust.open_file(&q),
                    None,
                    "a member declaring {declared} bytes cannot fit: {}",
                    d()
                );
            } else {
                let got = unsafe {
                    cpp::ref_big_open(
                        cq.as_ptr(),
                        buf.as_mut_ptr() as *mut _,
                        buf.len() as i32,
                        0,
                        &mut size,
                    )
                };
                assert_ne!(got, -3, "openFile (RAM) threw: {}", d());
                match rust.open_file(&q) {
                    None => assert_eq!(got, -1, "openFile (RAM) opened upstream only: {}", d()),
                    Some(bytes) => {
                        opened += 1;
                        assert_eq!(got, bytes.len() as i32, "openFile (RAM) size: {}", d());
                        assert_eq!(size, bytes.len() as i32, "openFile (RAM) size(): {}", d());
                        assert_eq!(&buf[..bytes.len()], bytes, "openFile (RAM) bytes: {}", d());
                    }
                }
            }
            let got = unsafe {
                cpp::ref_big_open(
                    cq.as_ptr(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len() as i32,
                    1,
                    &mut size,
                )
            };
            assert_ne!(got, -3, "openFile (streaming) threw: {}", d());
            match rust.open_file_streaming(&q) {
                None => assert_eq!(
                    got,
                    -1,
                    "openFile (streaming) opened upstream only: {}",
                    d()
                ),
                Some(m) => {
                    assert_eq!(size, m.size, "openFile (streaming) size(): {}", d());
                    assert_eq!(
                        got,
                        m.data.len() as i32,
                        "openFile (streaming) bytes read: {}",
                        d()
                    );
                    assert_eq!(
                        &buf[..m.data.len()],
                        m.data,
                        "openFile (streaming) bytes: {}",
                        d()
                    );
                }
            }

            let fi = unsafe { cpp::ref_big_file_info(cq.as_ptr()) };
            assert_eq!(
                rust.get_file_info(&q).map_or(-1, i64::from),
                fi,
                "getFileInfo: {}",
                d()
            );
        }

        let dirs: Vec<Vec<u8>> = gen_queries(&mut g, &loaded)
            .into_iter()
            .take(8)
            .map(|mut p| {
                if let Some(k) = p.iter().rposition(|&b| b == b'/' || b == b'\\') {
                    p.truncate(k + 1);
                }
                p
            })
            .collect();
        const MASKS: &[&[u8]] = &[
            b"*", b"*.ini", b"*.INI", b"?*", b"*a*", b"", b"*.", b"x?y*", b"*.w3d",
        ];
        for (k, dir) in dirs.iter().enumerate() {
            let dir: &[u8] = if k == 0 { b"" } else { dir };
            let mask = pick(&mut g, MASKS);
            let mut list = FilenameList::new();
            rust.get_file_list_in_directory(b"", dir, mask, &mut list, true);
            let n = unsafe {
                cpp::ref_big_list(
                    c(b"").as_ptr(),
                    c(dir).as_ptr(),
                    c(mask).as_ptr(),
                    buf.as_mut_ptr() as *mut _,
                    buf.len() as i32,
                )
            };
            assert!((n as usize) <= buf.len());
            let cpp_names = split_nul(&buf[..n as usize]);
            let rust_names: Vec<Vec<u8>> = list.iter().map(|s| s.to_vec()).collect();
            listed_names += rust_names.len() as u64;
            assert_eq!(
                rust_names,
                cpp_names,
                "case {case}: getFileListInDirectory({:?}, {:?})",
                String::from_utf8_lossy(dir),
                String::from_utf8_lossy(mask)
            );
        }
    }
    unsafe { cpp::ref_big_reset() };
    println!(
        "DIFF {:<44} cases {CASES}  archives {archives_total} (to C++ {archives_to_cpp}, UB-skipped {ub_skipped})  queries {queries}  opened {opened} (RAM opens of >16 MiB declared sizes not run in C++: {big_declared})  listed names {listed_names}  mismatches 0",
        ".big archive file system"
    );
}
