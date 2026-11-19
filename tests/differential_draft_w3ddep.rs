// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dpure) of the draft `src/drafts/ww3d/w3d_dep.rs` against upstream
//! `WW3D2/w3d_dep.cpp`, compiled WHOLE (a byte-for-byte copy #included by differential/wrap_draft_w3ddep.cpp, so its file-static
//! functions are called directly). The real `ChunkLoadClass` (chunkio.cpp), `ffactory.h`, `wwfile.h` and `w3d_file.h`; stand-ins:
//! a `FileClass` over a memory buffer with a `FileFactoryClass` that hands it out, and MSVC's `strupr`/`strlwr`/`_MAX_PATH`
//! (the factory global is renamed on the command line, see the wrapper).
//!
//! # Method
//!
//! 20,000 cases per operation from a fixed seed (`DRAFT_CASES` overrides). `Get_W3D_Name`: random file names (paths with
//! backslashes, several dots, no extension, empty stems, 0..=30 characters, upper and lower case). `Make_W3D_Filename`: random
//! names (0..=15 characters, dots, mixed case). `Get_W3D_Dependencies`: random chunk streams of 0..=6 top-level chunks drawn from
//! MESH (header3 with a container name, TEXTURES / TEXTURE / TEXTURE_NAME with or without padding and an empty name, other
//! children), ANIMATION, COMPRESSED_ANIMATION, HMODEL, HLOD, EMITTER (texture name or empty), AGGREGATE (base model and 0..=3
//! subobjects) and chunks the scanner ignores, names from a small pool that includes the file's own name (to be skipped), in
//! other cases, with dots and empty, so that duplicates and the sort order are exercised. The returned names are compared
//! as bytes, in list order.
//!
//! # Excluded inputs
//!
//! - Names that fill their whole field (16 bytes in a 16-byte field, 32 in a 32-byte field): upstream's `strcmp` then reads past
//!   the field, undefined. A texture or emitter name without a terminating NUL, and a texture-name chunk of length 0 (upstream
//!   reads an uninitialised `texture[0]`).
//! - Non-ASCII bytes in a file name or a name: the draft builds a Rust `String`, which would encode a Windows-1252 byte as two
//!   UTF-8 bytes, where upstream keeps the byte. The game's W3D names are ASCII. (A draft limitation, recorded, not fixed here.)
//! - Truncated or corrupt chunk streams (a struct body shorter than the struct); the chunk reader is tested elsewhere.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. The place to watch: an empty file, where the draft returns `None` for "bytes that do not form a chunk stream" and
//! upstream returns `true` with an empty list (the file opens; there is just nothing in it).
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::ww3d::w3d_dep::*;
use rts_lockstep_engine::formats::w3d::w3d_file::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dpw_w3d_name(filename: *const std::ffi::c_char, out: *mut u8);
    fn ref_dpw_w3d_filename(name: *const std::ffi::c_char, out: *mut u8);
    fn ref_dpw_w3d_deps(data: *const u8, len: i32, filename: *const std::ffi::c_char, out: *mut u8, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn cstring(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap()
}

fn rand_text(g: &mut Gen, max: u32, dots: bool) -> String {
    const ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_";
    let n = g.below(max + 1);
    (0..n)
        .map(|_| if dots && g.below(8) == 0 { '.' } else { ALNUM[g.below(ALNUM.len() as u32) as usize] as char })
        .collect()
}

fn rand_filename(g: &mut Gen) -> String {
    let mut s = String::new();
    for _ in 0..g.below(3) {
        s.push_str(&rand_text(g, 6, true));
        s.push('\\');
    }
    s.push_str(&rand_text(g, 24, true));
    match g.below(4) {
        0 => s.push_str(".w3d"),
        1 => s.push_str(".W3D"),
        2 => s.push('.'),
        _ => {}
    }
    s
}

/// A NUL-padded name field of `width` bytes holding `s` (`s.len() < width`).
fn field(s: &str, width: usize) -> Vec<u8> {
    assert!(s.len() < width);
    let mut v = s.as_bytes().to_vec();
    v.resize(width, 0);
    v
}

fn chunk(id: u32, body: &[u8], container: bool) -> Vec<u8> {
    let mut v = id.to_le_bytes().to_vec();
    let size = body.len() as u32 | if container { 0x8000_0000 } else { 0 };
    v.extend_from_slice(&size.to_le_bytes());
    v.extend_from_slice(body);
    v
}

fn junk(g: &mut Gen, n: usize) -> Vec<u8> {
    (0..n).map(|_| g.below(256) as u8).collect()
}

/// A name for a field of `width`: from a small pool (including the file's own), empty, or random.
fn pick_name(g: &mut Gen, own: &str, width: usize) -> String {
    let pool = ["Box01", "BOX01", "base", "Skel_A", "a.b", "x", "name.ext", "Tank_Skel", "own"];
    let s = match g.below(8) {
        0 => String::new(),
        1 | 2 => own.to_string(),
        3 => own.to_ascii_lowercase(),
        4 => rand_text(g, (width - 1) as u32, true),
        _ => pool[g.below(pool.len() as u32) as usize].to_string(),
    };
    s.chars().take(width - 1).collect()
}

fn header_struct(g: &mut Gen, size: usize, at: usize, name: &str) -> Vec<u8> {
    let mut b = junk(g, size);
    // make the junk tame (no accidental long strings elsewhere matter: only the name field is read)
    b[at..at + 16].copy_from_slice(&field(name, 16));
    b
}

fn texture_chunks(g: &mut Gen) -> Vec<u8> {
    let mut textures = Vec::new();
    for _ in 0..g.below(4) {
        let mut inner = Vec::new();
        for _ in 0..g.below(3) {
            if g.below(5) == 0 {
                inner.extend(chunk(0x33, &junk(g, 12), false)); // a sub-chunk the scanner ignores
            }
            let name = if g.below(6) == 0 { String::new() } else { format!("{}.tga", rand_text(g, 20, false)) };
            let mut body = name.as_bytes().to_vec();
            body.push(0);
            for _ in 0..g.below(3) {
                body.push(0);
            }
            inner.extend(chunk(W3D_CHUNK_TEXTURE_NAME, &body, false));
        }
        textures.extend(chunk(W3D_CHUNK_TEXTURE, &inner, true));
    }
    chunk(W3D_CHUNK_TEXTURES, &textures, true)
}

fn top_chunks(g: &mut Gen, own: &str) -> Vec<u8> {
    let mut file = Vec::new();
    for _ in 0..g.below(7) {
        match g.below(9) {
            0 => {
                let mut kids = Vec::new();
                for _ in 0..g.below(4) {
                    match g.below(3) {
                        0 => {
                            let n = pick_name(g, own, 16);
                            kids.extend(chunk(W3D_CHUNK_MESH_HEADER3, &header_struct(g, 116, 24, &n), false));
                        }
                        1 => kids.extend(texture_chunks(g)),
                        _ => kids.extend(chunk(0x02, &junk(g, 12), false)),
                    }
                }
                file.extend(chunk(W3D_CHUNK_MESH, &kids, true));
            }
            1 | 2 => {
                let id = if g.below(2) == 0 { W3D_CHUNK_ANIMATION } else { W3D_CHUNK_COMPRESSED_ANIMATION };
                let mut kids = Vec::new();
                for _ in 0..g.below(3) {
                    let n = pick_name(g, own, 16);
                    kids.extend(chunk(W3D_CHUNK_ANIMATION_HEADER, &header_struct(g, 44, 20, &n), false));
                    if g.below(3) == 0 {
                        kids.extend(chunk(0x202, &junk(g, 8), false));
                    }
                }
                file.extend(chunk(id, &kids, true));
            }
            3 => {
                let mut kids = Vec::new();
                for _ in 0..g.below(3) {
                    let n = pick_name(g, own, 16);
                    kids.extend(chunk(W3D_CHUNK_HMODEL_HEADER, &header_struct(g, 40, 20, &n), false));
                    kids.extend(chunk(0x302, &junk(g, 8), false));
                }
                file.extend(chunk(W3D_CHUNK_HMODEL, &kids, true));
            }
            4 => {
                let mut kids = Vec::new();
                for _ in 0..g.below(3) {
                    let n = pick_name(g, own, 16);
                    let mut b = junk(g, 40);
                    b[24..40].copy_from_slice(&field(&n, 16));
                    kids.extend(chunk(W3D_CHUNK_HLOD_HEADER, &b, false));
                }
                file.extend(chunk(W3D_CHUNK_HLOD, &kids, true));
            }
            5 => {
                let mut kids = Vec::new();
                for _ in 0..g.below(3) {
                    let name = if g.below(3) == 0 { String::new() } else { format!("{}.tga", rand_text(g, 30, false)) };
                    let mut b = junk(g, 332);
                    b[..260].copy_from_slice(&field(&name, 260));
                    kids.extend(chunk(W3D_CHUNK_EMITTER_INFO, &b, false));
                }
                file.extend(chunk(W3D_CHUNK_EMITTER, &kids, true));
            }
            6 => {
                let mut kids = Vec::new();
                for _ in 0..g.below(3) {
                    let count = g.below(4);
                    let mut b = field(&pick_name(g, own, 32), 32);
                    b.extend_from_slice(&count.to_le_bytes());
                    for _ in 0..count {
                        b.extend(field(&pick_name(g, own, 32), 32));
                        b.extend(junk(g, 32));
                    }
                    kids.extend(chunk(W3D_CHUNK_AGGREGATE_INFO, &b, false));
                }
                file.extend(chunk(W3D_CHUNK_AGGREGATE, &kids, true));
            }
            7 => file.extend(chunk(0x100 + g.below(4), &junk(g, 16), false)),
            _ => {
                // a chunk the scanner ignores, with sub-chunks
                let sub = chunk(0x101, &junk(g, 8), false);
                file.extend(chunk(0x400, &sub, true));
            }
        }
    }
    file
}

#[test]
fn w3d_dep_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_14_05);
    let mut c_name = Check::new("Get_W3D_Name");
    let mut c_file = Check::new("Make_W3D_Filename");
    let mut c_deps = Check::new("Get_W3D_Dependencies");
    for case in 0..cases() {
        // Get_W3D_Name
        let f = rand_filename(&mut g);
        let mut out = [0u8; 16];
        unsafe { ref_dpw_w3d_name(cstring(&f).as_ptr(), out.as_mut_ptr()) };
        let cpp = String::from_utf8_lossy(&out[..out.iter().position(|&b| b == 0).unwrap_or(16)]).into_owned();
        c_name.eq(get_w3d_name(&f), cpp, || format!("{f:?}"));

        // Make_W3D_Filename
        let n = if g.below(6) == 0 { String::new() } else { rand_text(&mut g, 15, true) };
        let mut out = [0u8; 64];
        unsafe { ref_dpw_w3d_filename(cstring(&n).as_ptr(), out.as_mut_ptr()) };
        let cpp = String::from_utf8_lossy(&out[..out.iter().position(|&b| b == 0).unwrap_or(64)]).into_owned();
        c_file.eq(make_w3d_filename(&n), cpp, || format!("{n:?}"));

        // Get_W3D_Dependencies
        let fname = rand_filename(&mut g);
        let own = get_w3d_name(&fname);
        let data = top_chunks(&mut g, &own);
        let mut out = vec![0u8; 8192];
        let r = unsafe { ref_dpw_w3d_deps(data.as_ptr(), data.len() as i32, cstring(&fname).as_ptr(), out.as_mut_ptr(), out.len() as i32) };
        assert!(r >= 0 && (r as usize) <= out.len(), "case {case}: the C++ output ({r} bytes)");
        let cpp: Vec<String> = if r == 0 { Vec::new() } else { String::from_utf8_lossy(&out[..r as usize]).split('\n').map(str::to_string).collect() };
        let rust = get_w3d_dependencies(&fname, &data);
        c_deps.eq(rust, Some(cpp), || format!("case {case} file {fname:?} len {}", data.len()));
    }
    finish(vec![c_name, c_file, c_deps]);
}
