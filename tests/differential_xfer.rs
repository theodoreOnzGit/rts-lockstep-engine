// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `common::xfer`, `xfer_crc`, `xfer_save` and
//! `xfer_load` against upstream `Xfer.cpp`, `XferCRC.cpp` (with
//! `XferDeepCRC`), `XferSave.cpp` and `XferLoad.cpp` (commit 0a05454d),
//! compiled unchanged by g++ against the shims in `differential/shim/Common`
//! (see `differential/wrap_xfer.cpp`).
//!
//! # Method
//!
//! The Rust side uses `XferCrc::new_without_nan_guard` (and the deep-CRC
//! equivalent), i.e. upstream's raw-bit hashing; the proposed NaN guard is
//! tested separately at the end of this file.
//!
//! Random **scripts** of Xfer operations are run by an interpreter on each
//! side, against the real upstream classes and the Rust translation. Every
//! operation logs its status (0, or the thrown `XferStatus`) and the values
//! after it; the run ends by logging the mode, the options, `getCRC()` and the
//! number of post-process hand-offs. Compared, word for word:
//!
//! - the logs (so every value moved, every error, every CRC read mid-script);
//! - for `XferSave` and `XferDeepCRC`, the bytes of the written file (the C++
//!   writes a real file; the Rust an in-memory buffer);
//! - for `XferLoad`, the file is the C++ save output of the same script,
//!   sometimes truncated, so reads hit end-of-file at random points.
//!
//! The scripts cover every translated typed method (version, byte, unsigned
//! byte, bool, int, int64, unsigned int, short, unsigned short, real with
//! arbitrary bit patterns including NaN, marker label, ASCII and Unicode
//! strings with embedded NULs and over-long strings, all coordinate, region,
//! range and colour structs, object/drawable ids, the three STL sequences,
//! raw user bytes including zero-length, Matrix3D), blocks, skips, options,
//! double opens, snapshots, and mid-script `getCRC`.
//!
//! # Restrictions (where the C++ itself is undefined)
//!
//! - Save/load/deep-CRC scripts never close mid-script (upstream then calls
//!   `fwrite`/`fread`/`ftell` on a NULL `FILE*`).
//! - Save/load scripts keep every save and load aligned (strings <= 255, no
//!   negative skips, no loads into a non-empty list), because a misaligned
//!   load can read a byte other than 0/1 into a C++ `bool` (undefined). Those
//!   cases are covered by the fixed tests below instead.
//! - Object and drawable ids stay in `0..=0x7ffffff`, the value range of the
//!   upstream enums.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::*;
use rts_lockstep_engine::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus};
use rts_lockstep_engine::common::xfer_crc::{XferCrc, XferDeepCrc, CANONICAL_NAN_BITS};
use rts_lockstep_engine::common::xfer_load::XferLoad;
use rts_lockstep_engine::common::xfer_save::XferSave;
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use support::Gen;

const CRC: i32 = 0;
const SAVE: i32 = 1;
const LOAD: i32 = 2;
const DEEP: i32 = 3;

// ---------------------------------------------------------------------------
// The Rust interpreter (mirror of ref_xfer_run in differential/wrap_xfer.cpp)
// ---------------------------------------------------------------------------

struct Script<'a> {
    w: &'a [u32],
    i: usize,
}

impl Script<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w[self.i];
        self.i += 1;
        v
    }
    fn ascii(&mut self) -> Vec<u8> {
        let n = self.next() as usize;
        (0..n).map(|_| self.next() as u8).collect()
    }
    fn unicode(&mut self) -> Vec<u16> {
        let n = self.next() as usize;
        (0..n).map(|_| self.next() as u16).collect()
    }
    fn real(&mut self) -> f32 {
        f32::from_bits(self.next())
    }
}

struct TestSnapshot {
    a: i32,
    b: f32,
    s: Vec<u8>,
}

impl Snapshot for TestSnapshot {
    fn crc(&mut self, x: &mut dyn Xfer) -> XferResult<()> {
        x.xfer_int(&mut self.a)?;
        x.xfer_real(&mut self.b)
    }
    fn xfer(&mut self, x: &mut dyn Xfer) -> XferResult<()> {
        let mut v = 1u8;
        x.xfer_version(&mut v, 1)?;
        x.xfer_int(&mut self.a)?;
        x.xfer_real(&mut self.b)?;
        x.xfer_ascii_string(&mut self.s)
    }
    fn load_post_process(&mut self) {}
}

fn log_ascii(lg: &mut Vec<u32>, s: &[u8]) {
    lg.push(s.len() as u32);
    lg.extend(s.iter().map(|&c| u32::from(c)));
}

/// Runs one op, logging values; returns the status. Values are logged only
/// on success, except for the sequence and snapshot ops (as the C++ does).
fn run_op<X: Xfer>(op: u32, sc: &mut Script, lg: &mut Vec<u32>, x: &mut X, crc: fn(&X) -> u32) -> XferResult<()> {
    macro_rules! reals {
        ($n:expr, $get:expr) => {{
            let mut v: Vec<f32> = (0..$n).map(|_| sc.real()).collect();
            let r = $get(x, &mut v);
            r?;
            lg.extend(v.iter().map(|f| f.to_bits()));
        }};
    }
    match op {
        0 => {
            let mut v = sc.next() as u8;
            let cur = sc.next() as u8;
            x.xfer_version(&mut v, cur)?;
            lg.push(u32::from(v));
        }
        1 => {
            let mut v = sc.next() as u8 as i8;
            x.xfer_byte(&mut v)?;
            lg.push(i32::from(v) as u32);
        }
        2 => {
            let mut v = sc.next() as u8;
            x.xfer_unsigned_byte(&mut v)?;
            lg.push(u32::from(v));
        }
        3 => {
            let mut v = sc.next() != 0;
            x.xfer_bool(&mut v)?;
            lg.push(u32::from(v));
        }
        4 => {
            let mut v = sc.next() as i32;
            x.xfer_int(&mut v)?;
            lg.push(v as u32);
        }
        5 => {
            let lo = u64::from(sc.next());
            let hi = u64::from(sc.next());
            let mut v = ((hi << 32) | lo) as i64;
            x.xfer_int64(&mut v)?;
            lg.push(v as u64 as u32);
            lg.push((v as u64 >> 32) as u32);
        }
        6 => {
            let mut v = sc.next();
            x.xfer_unsigned_int(&mut v)?;
            lg.push(v);
        }
        7 => {
            let mut v = sc.next() as u16 as i16;
            x.xfer_short(&mut v)?;
            lg.push(i32::from(v) as u32);
        }
        8 => {
            let mut v = sc.next() as u16;
            x.xfer_unsigned_short(&mut v)?;
            lg.push(u32::from(v));
        }
        9 => reals!(1, |x: &mut X, v: &mut Vec<f32>| x.xfer_real(&mut v[0])),
        10 => {
            let s = sc.ascii();
            x.xfer_marker_label(&s)?;
        }
        11 => {
            let mut s = sc.ascii();
            x.xfer_ascii_string(&mut s)?;
            log_ascii(lg, &s);
        }
        12 => {
            let mut s = sc.unicode();
            x.xfer_unicode_string(&mut s)?;
            lg.push(s.len() as u32);
            lg.extend(s.iter().map(|&c| u32::from(c)));
        }
        13 => reals!(3, |x: &mut X, v: &mut Vec<f32>| {
            let mut c = Coord3D { x: v[0], y: v[1], z: v[2] };
            let r = x.xfer_coord3d(&mut c);
            *v = vec![c.x, c.y, c.z];
            r
        }),
        14 => {
            let mut c = ICoord3D { x: sc.next() as i32, y: sc.next() as i32, z: sc.next() as i32 };
            x.xfer_icoord3d(&mut c)?;
            lg.extend([c.x as u32, c.y as u32, c.z as u32]);
        }
        15 => reals!(6, |x: &mut X, v: &mut Vec<f32>| {
            let mut r = Region3D { lo: Coord3D { x: v[0], y: v[1], z: v[2] }, hi: Coord3D { x: v[3], y: v[4], z: v[5] } };
            let res = x.xfer_region3d(&mut r);
            *v = vec![r.lo.x, r.lo.y, r.lo.z, r.hi.x, r.hi.y, r.hi.z];
            res
        }),
        16 => {
            let p: Vec<i32> = (0..6).map(|_| sc.next() as i32).collect();
            let mut r = IRegion3D { lo: ICoord3D { x: p[0], y: p[1], z: p[2] }, hi: ICoord3D { x: p[3], y: p[4], z: p[5] } };
            x.xfer_iregion3d(&mut r)?;
            lg.extend([r.lo.x, r.lo.y, r.lo.z, r.hi.x, r.hi.y, r.hi.z].map(|v| v as u32));
        }
        17 => reals!(2, |x: &mut X, v: &mut Vec<f32>| {
            let mut c = Coord2D { x: v[0], y: v[1] };
            let r = x.xfer_coord2d(&mut c);
            *v = vec![c.x, c.y];
            r
        }),
        18 => {
            let mut c = ICoord2D { x: sc.next() as i32, y: sc.next() as i32 };
            x.xfer_icoord2d(&mut c)?;
            lg.extend([c.x as u32, c.y as u32]);
        }
        19 => reals!(4, |x: &mut X, v: &mut Vec<f32>| {
            let mut r = Region2D { lo: Coord2D { x: v[0], y: v[1] }, hi: Coord2D { x: v[2], y: v[3] } };
            let res = x.xfer_region2d(&mut r);
            *v = vec![r.lo.x, r.lo.y, r.hi.x, r.hi.y];
            res
        }),
        20 => {
            let p: Vec<i32> = (0..4).map(|_| sc.next() as i32).collect();
            let mut r = IRegion2D { lo: ICoord2D { x: p[0], y: p[1] }, hi: ICoord2D { x: p[2], y: p[3] } };
            x.xfer_iregion2d(&mut r)?;
            lg.extend([r.lo.x, r.lo.y, r.hi.x, r.hi.y].map(|v| v as u32));
        }
        21 => reals!(2, |x: &mut X, v: &mut Vec<f32>| {
            let mut r = RealRange { lo: v[0], hi: v[1] };
            let res = x.xfer_real_range(&mut r);
            *v = vec![r.lo, r.hi];
            res
        }),
        22 => {
            let mut c = sc.next() as i32;
            x.xfer_color(&mut c)?;
            lg.push(c as u32);
        }
        23 => reals!(3, |x: &mut X, v: &mut Vec<f32>| {
            let mut c = RGBColor { red: v[0], green: v[1], blue: v[2] };
            let r = x.xfer_rgb_color(&mut c);
            *v = vec![c.red, c.green, c.blue];
            r
        }),
        24 => reals!(4, |x: &mut X, v: &mut Vec<f32>| {
            let mut c = RGBAColorReal { red: v[0], green: v[1], blue: v[2], alpha: v[3] };
            let r = x.xfer_rgba_color_real(&mut c);
            *v = vec![c.red, c.green, c.blue, c.alpha];
            r
        }),
        25 => {
            let mut c = RGBAColorInt { red: sc.next(), green: sc.next(), blue: sc.next(), alpha: sc.next() };
            x.xfer_rgba_color_int(&mut c)?;
            lg.extend([c.red, c.green, c.blue, c.alpha]);
        }
        26 => {
            let mut id = sc.next() as i32;
            x.xfer_object_id(&mut id)?;
            lg.push(id as u32);
        }
        27 => {
            let mut id = sc.next() as i32;
            x.xfer_drawable_id(&mut id)?;
            lg.push(id as u32);
        }
        28..=30 => {
            let cnt = sc.next() as usize;
            let items: Vec<i32> = (0..cnt).map(|_| sc.next() as i32).collect();
            let mut v = if x.get_xfer_mode() == XferMode::Load { Vec::new() } else { items };
            let r = match op {
                28 => x.xfer_stl_object_id_vector(&mut v),
                29 => x.xfer_stl_object_id_list(&mut v),
                _ => x.xfer_stl_int_list(&mut v),
            };
            lg.push(v.len() as u32);
            lg.extend(v.iter().map(|&i| i as u32));
            r?;
        }
        31 => {
            let cnt = sc.next() as usize;
            let mut b: Vec<u8> = (0..cnt).map(|_| sc.next() as u8).collect();
            x.xfer_user(&mut b)?;
            lg.extend(b.iter().map(|&c| u32::from(c)));
        }
        32 => reals!(12, |x: &mut X, v: &mut Vec<f32>| {
            let mut m = Matrix3D::identity();
            for r in 0..3 {
                for c in 0..4 {
                    m[r][c] = v[r * 4 + c];
                }
            }
            let res = x.xfer_matrix3d(&mut m);
            *v = (0..12).map(|k| m[k / 4][k % 4]).collect();
            res
        }),
        33 => {
            let r = x.begin_block()?;
            lg.push(r as u32);
        }
        34 => x.end_block()?,
        35 => x.skip(sc.next() as i32)?,
        36 => {
            x.set_options(sc.next());
            lg.push(x.get_options());
        }
        37 => {
            x.clear_options(sc.next());
            lg.push(x.get_options());
        }
        38 => {
            let mut s = TestSnapshot { a: sc.next() as i32, b: sc.real(), s: Vec::new() };
            s.s = sc.ascii();
            let r = x.xfer_snapshot(&mut s);
            lg.push(s.a as u32);
            lg.push(s.b.to_bits());
            log_ascii(lg, &s.s);
            r?;
        }
        39 => x.open(b"xfer-test")?,
        40 => x.close()?,
        41 => lg.push(crc(x)),
        42 => {
            let mut v = vec![sc.next() as i32];
            let r = x.xfer_stl_int_list(&mut v);
            lg.push(v.len() as u32);
            r?;
        }
        _ => panic!("bad op {op}"),
    }
    Ok(())
}

/// Runs `script` on `x`; returns the log (the per-op log, then mode,
/// options, getCRC and the post-process count).
fn run_with<X: Xfer>(x: &mut X, script: &[u32], crc: fn(&X) -> u32, post: fn(&X) -> u32, close: bool) -> Vec<u32> {
    let mut lg = Vec::new();
    let mut sc = Script { w: script, i: 0 };
    while sc.i < script.len() {
        let op = sc.next();
        let at = lg.len();
        lg.push(0);
        if let Err(s) = run_op(op, &mut sc, &mut lg, x, crc) {
            lg[at] = s as u32;
        }
    }
    lg.push(x.get_xfer_mode() as u32);
    lg.push(x.get_options());
    lg.push(crc(x));
    lg.push(post(x));
    if close {
        let _ = x.close();
    }
    lg
}

/// Returns the log and the written file (save/deep) or nothing.
fn run_rust(kind: i32, script: &[u32], load_file: &[u8]) -> (Vec<u32>, Vec<u8>) {
    match kind {
        CRC => (run_with(&mut XferCrc::new_without_nan_guard(), script, |x| x.get_crc(), |_| 0, false), Vec::new()),
        SAVE => {
            let mut x = XferSave::new();
            let lg = run_with(&mut x, script, |_| 0, |_| 0, true);
            (lg, x.bytes().to_vec())
        }
        LOAD => {
            let mut x = XferLoad::new(load_file.to_vec());
            (run_with(&mut x, script, |_| 0, |x| x.post_process_requests() as u32, true), Vec::new())
        }
        _ => {
            let mut x = XferDeepCrc::new_without_nan_guard();
            let lg = run_with(&mut x, script, |x| x.get_crc(), |_| 0, true);
            (lg, x.bytes().to_vec())
        }
    }
}

fn temp_path(tag: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("rts-lockstep-xfer-{}-{tag}.bin", std::process::id()));
    p
}

/// The C++ wrapper counts post-process hand-offs in a global stand-in for
/// `TheGameState`, so C++ runs are serialised.
static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Runs the C++; for LOAD, `load_file` is written to the path first.
fn run_cpp(kind: i32, script: &[u32], load_file: &[u8], tag: &str) -> (Vec<u32>, Vec<u8>) {
    let _guard = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = temp_path(tag);
    if kind == LOAD {
        std::fs::write(&path, load_file).unwrap();
    } else {
        let _ = std::fs::remove_file(&path);
    }
    let cpath = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let cap = script.len() * 2 + 4096;
    let mut log = vec![0u32; cap];
    let n = unsafe { cpp::ref_xfer_run(kind, cpath.as_ptr(), script.as_ptr(), script.len() as i32, log.as_mut_ptr(), cap as i32) };
    assert!(n >= 0, "malformed script");
    assert!((n as usize) <= cap, "log overflow");
    log.truncate(n as usize);
    let file = if kind == SAVE || kind == DEEP { std::fs::read(&path).unwrap_or_default() } else { Vec::new() };
    let _ = std::fs::remove_file(&path);
    (log, file)
}

// ---------------------------------------------------------------------------
// Script generation
// ---------------------------------------------------------------------------

/// With `canon`, every NaN is replaced by the canonical NaN (for the
/// NaN-guard test); the random draws are the same either way.
fn gen_real(g: &mut Gen, canon: bool) -> u32 {
    let bits = match g.below(4) {
        0 => g.u32(), // any bit pattern, NaNs with payloads included
        1 => g.special().to_bits(),
        _ => g.uniform(-1000.0, 1000.0).to_bits(),
    };
    if canon && f32::from_bits(bits).is_nan() {
        CANONICAL_NAN_BITS
    } else {
        bits
    }
}

fn gen_ascii(g: &mut Gen, out: &mut Vec<u32>, max: u32) {
    let n = if g.below(10) == 0 { g.below(max + 1) } else { g.below(12) };
    out.push(n);
    for _ in 0..n {
        out.push(if g.below(8) == 0 { 0 } else { 1 + g.below(255) });
    }
}

/// A random script for `kind`; `aligned` keeps save and load in step.
fn gen_script(g: &mut Gen, kind: i32, ops: usize) -> Vec<u32> {
    gen_script_nan(g, kind, ops, false)
}

fn gen_script_nan(g: &mut Gen, kind: i32, ops: usize, canon: bool) -> Vec<u32> {
    let gen_real = |g: &mut Gen| gen_real(g, canon);
    let aligned = kind == SAVE || kind == LOAD;
    let max_str = if aligned { 255 } else { 300 };
    let mut s = vec![39u32]; // open
    for _ in 0..ops {
        let op = g.below(42);
        match op {
            40 if kind != CRC => continue, // no mid-script close on file kinds
            35 => {
                s.push(35);
                let n = if aligned { g.below(9) as i32 } else { g.below(21) as i32 - 10 };
                s.push(n as u32);
                continue;
            }
            _ => {}
        }
        s.push(op);
        match op {
            0 => {
                s.push(g.below(4));
                s.push(g.below(4));
            }
            1..=8 | 22 => s.push(g.u32()),
            9 => s.push(gen_real(g)),
            10 | 11 => gen_ascii(g, &mut s, max_str),
            12 => {
                let n = if g.below(10) == 0 { g.below(max_str + 1) } else { g.below(10) };
                s.push(n);
                for _ in 0..n {
                    s.push(if g.below(8) == 0 { 0 } else { g.u32() & 0xffff });
                }
            }
            13 | 23 => (0..3).for_each(|_| s.push(gen_real(g))),
            14 => (0..3).for_each(|_| s.push(g.u32())),
            15 => (0..6).for_each(|_| s.push(gen_real(g))),
            16 => (0..6).for_each(|_| s.push(g.u32())),
            17 | 21 => (0..2).for_each(|_| s.push(gen_real(g))),
            18 => (0..2).for_each(|_| s.push(g.u32())),
            19 | 24 => (0..4).for_each(|_| s.push(gen_real(g))),
            20 | 25 => (0..4).for_each(|_| s.push(g.u32())),
            26 | 27 => s.push(g.below(0x0800_0000)),
            28..=30 => {
                let n = g.below(6);
                s.push(n);
                for _ in 0..n {
                    s.push(if op == 30 { g.u32() } else { g.below(0x0800_0000) });
                }
            }
            31 => {
                let n = g.below(13);
                s.push(n);
                for _ in 0..n {
                    s.push(g.below(256));
                }
            }
            32 => (0..12).for_each(|_| s.push(gen_real(g))),
            36 | 37 => s.push(if g.below(2) == 0 { 1 } else { g.u32() }),
            38 => {
                s.push(g.u32());
                s.push(gen_real(g));
                gen_ascii(g, &mut s, if aligned { 255 } else { 40 });
            }
            _ => {}
        }
        // int64 has two operand words (op 5 above pushed one via 1..=8).
        if op == 5 {
            s.push(g.u32());
        }
    }
    if kind != CRC {
        s.push(40); // close
    }
    s
}

fn compare(name: &str, script: &[u32], rust: &(Vec<u32>, Vec<u8>), cpp: &(Vec<u32>, Vec<u8>)) {
    if rust.0 != cpp.0 {
        let i = rust.0.iter().zip(cpp.0.iter()).position(|(a, b)| a != b).unwrap_or(rust.0.len().min(cpp.0.len()));
        panic!(
            "{name}: logs differ at word {i} (rust len {}, c++ len {}): rust {:?} c++ {:?}\nscript {:?}",
            rust.0.len(),
            cpp.0.len(),
            &rust.0[i.saturating_sub(4)..(i + 4).min(rust.0.len())],
            &cpp.0[i.saturating_sub(4)..(i + 4).min(cpp.0.len())],
            script
        );
    }
    assert_eq!(rust.1, cpp.1, "{name}: written files differ\nscript {script:?}");
}

const SCRIPTS: usize = 2_000;
const OPS: usize = 60;

#[test]
fn xfer_crc_scripts_match() {
    let mut g = Gen::new(200);
    let (mut words, mut ops) = (0usize, 0usize);
    for k in 0..SCRIPTS {
        let s = gen_script(&mut g, CRC, OPS);
        let r = run_rust(CRC, &s, &[]);
        let c = run_cpp(CRC, &s, &[], "crc");
        compare(&format!("XferCRC script {k}"), &s, &r, &c);
        words += r.0.len();
        ops += OPS;
    }
    println!("DIFF {:<44} scripts {SCRIPTS}  ops {ops}  log words {words}  mismatches 0", "XferCRC (scripts)");
}

#[test]
fn xfer_deep_crc_scripts_match() {
    let mut g = Gen::new(201);
    let (mut words, mut bytes) = (0usize, 0usize);
    for k in 0..SCRIPTS {
        let s = gen_script(&mut g, DEEP, OPS);
        let r = run_rust(DEEP, &s, &[]);
        let c = run_cpp(DEEP, &s, &[], "deep");
        compare(&format!("XferDeepCRC script {k}"), &s, &r, &c);
        words += r.0.len();
        bytes += r.1.len();
    }
    println!("DIFF {:<44} scripts {SCRIPTS}  log words {words}  file bytes {bytes}  mismatches 0", "XferDeepCRC (scripts)");
}

#[test]
fn xfer_save_then_load_scripts_match() {
    let mut g = Gen::new(202);
    let (mut words, mut bytes, mut truncated) = (0usize, 0usize, 0usize);
    for k in 0..SCRIPTS {
        let s = gen_script(&mut g, SAVE, OPS);
        let r = run_rust(SAVE, &s, &[]);
        let c = run_cpp(SAVE, &s, &[], "save");
        compare(&format!("XferSave script {k}"), &s, &r, &c);
        words += r.0.len();
        bytes += r.1.len();
        // Load the same script back from the C++ file, sometimes truncated.
        let mut file = c.1.clone();
        if g.below(3) == 0 && !file.is_empty() {
            file.truncate(g.below(file.len() as u32) as usize);
            truncated += 1;
        }
        let ls: Vec<u32> = s.clone();
        let rl = run_rust(LOAD, &ls, &file);
        let cl = run_cpp(LOAD, &ls, &file, "load");
        compare(&format!("XferLoad script {k}"), &ls, &rl, &cl);
        words += rl.0.len();
    }
    println!(
        "DIFF {:<44} scripts {SCRIPTS} x2  log words {words}  file bytes {bytes}  truncated loads {truncated}  mismatches 0",
        "XferSave + XferLoad (scripts)"
    );
}

/// Fixed cases outside the aligned random scripts: over-long strings on save,
/// negative skips, a load into a non-empty list, a version above current,
/// and the deep-CRC Unicode length quirk (128-255 characters).
#[test]
fn xfer_fixed_edge_cases_match() {
    let long = |n: u32| -> Vec<u32> {
        let mut v = vec![n];
        v.extend((0..n).map(|i| 1 + i % 200));
        v
    };
    let mut cases: Vec<(i32, Vec<u32>)> = Vec::new();
    for kind in [CRC, SAVE, DEEP] {
        let mut s = vec![39, 11];
        s.extend(long(256));
        s.push(12);
        s.extend(long(200));
        s.push(12);
        s.extend(long(256));
        s.extend([35, (-3i32) as u32, 4, 7, 35, (-100i32) as u32, 4, 9, 0, 3, 2, 33, 4, 1, 34, 34]);
        if kind != CRC {
            s.push(40);
        }
        cases.push((kind, s));
    }
    // Load: a non-empty list, a too-new version, a negative skip, a skip
    // before the start, a skip past the end then a read.
    let mut save = vec![39, 30, 2, 5, 6, 0, 3, 1, 4, 77, 4, 88];
    save.push(40);
    let file = run_cpp(SAVE, &save, &[], "fixed-save").1;
    let load = vec![39, 42, 9, 0, 3, 1, 35, (-4i32) as u32, 4, 0, 35, (-1000i32) as u32, 35, 1000, 4, 0, 33, 40, 40];
    for (kind, s) in &cases {
        let r = run_rust(*kind, s, &[]);
        let c = run_cpp(*kind, s, &[], "fixed");
        compare(&format!("fixed case kind {kind}"), s, &r, &c);
    }
    let r = run_rust(LOAD, &load, &file);
    let c = run_cpp(LOAD, &load, &file, "fixed-load");
    compare("fixed load case", &load, &r, &c);
    assert!(r.0.contains(&(XferStatus::ListNotEmpty as u32)));
    assert!(r.0.contains(&(XferStatus::InvalidVersion as u32)));
    assert!(r.0.contains(&(XferStatus::SkipError as u32)));
    println!("DIFF {:<44} cases {}  mismatches 0", "Xfer fixed edge cases", cases.len() + 1);
}

/// PROPOSED NaN guard (release builds): a guarded Rust `XferCRC` /
/// `XferDeepCRC` on a script with arbitrary NaN bit patterns gives the same
/// CRC and deep-CRC file as the upstream C++ on the same script with every
/// NaN replaced by the canonical NaN. Only the final CRC and the file are
/// compared (the logged values keep the caller's NaN, which the guard does
/// not modify).
#[cfg(not(debug_assertions))]
#[test]
fn nan_guard_equals_upstream_on_canonicalised_nans() {
    let mut nan_scripts = 0;
    for kind in [CRC, DEEP] {
        for k in 0..500u64 {
            let s = gen_script_nan(&mut Gen::new(300 + k), kind, OPS, false);
            let sc = gen_script_nan(&mut Gen::new(300 + k), kind, OPS, true);
            assert_eq!(s.len(), sc.len());
            if s != sc {
                nan_scripts += 1;
            }
            let lg = match kind {
                CRC => {
                    let mut x = XferCrc::new();
                    (run_with(&mut x, &s, |x| x.get_crc(), |_| 0, false), Vec::new())
                }
                _ => {
                    let mut x = XferDeepCrc::new();
                    let lg = run_with(&mut x, &s, |x| x.get_crc(), |_| 0, true);
                    (lg, x.bytes().to_vec())
                }
            };
            let c = run_cpp(kind, &sc, &[], "nan-guard");
            let crc_at = |l: &Vec<u32>| l[l.len() - 2];
            assert_eq!(crc_at(&lg.0), crc_at(&c.0), "kind {kind} script {k}: guarded CRC differs");
            assert_eq!(lg.1, c.1, "kind {kind} script {k}: guarded deep-CRC file differs");
        }
    }
    assert!(nan_scripts > 100, "too few scripts contained a NaN");
    println!("DIFF {:<44} scripts 1000 ({nan_scripts} with NaNs)  mismatches 0", "XferCRC NaN guard vs canonicalised C++");
}

/// PROPOSED NaN guard (debug builds): a NaN into a guarded CRC panics.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "xfered into a lockstep CRC")]
fn nan_guard_panics_in_debug_builds() {
    let mut x = XferCrc::new();
    let _ = x.xfer_real(&mut f32::NAN);
}
