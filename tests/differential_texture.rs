// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `formats::texture` against the upstream C++
//! (commit 0a05454d): `WW3D2/ddsfile.cpp`, `formconv.cpp`, `ww3dformat.cpp`,
//! the real `bitmaphandler.h` and `colorspace.h`, and `WWLib/TARGA.CPP` (see
//! `differential/wrap_texture.cpp` and `build.rs`; the file underneath is
//! the real `RAMFile.cpp`, read through `GameFileClass`'s functions).
//!
//! No game assets are available, so every input is **synthetic**, built
//! byte by byte in the layout the upstream readers read. DDS: the 4-byte
//! magic, a `LegacyDDSURFACEDESC2` whose `Size` is right or wrong, sizes
//! from 0 to a few hundred pixels (and some huge), mip counts, DXT and
//! other FourCCs (including codes with the top bit set), the cube-map and
//! volume caps, payloads of exactly the computed length or shorter or
//! longer, random block bytes; then reduction factors (including large
//! ones), and scripts of reads: pixels, 4x4 blocks, level, cube-map and
//! volume copies to every destination format, with matching and wrong
//! sizes, pitches and buffer lengths, with and without a hue shift. Targa:
//! headers with every image type, colour-map settings and pixel depth the
//! loader branches on (and random ones, negative sizes included), ID
//! fields, palettes, raw and run-length pixel data that fits, overruns or
//! falls short, origin bits, and Targa 2.0 footers with extension areas at
//! good and bad offsets; each file goes through the game's texture-loader
//! sequence and through the tools' `Load(TGAF_IMAGE | TGAF_PAL, true)`.
//! Half of the files are well-formed; the others also get truncations and
//! byte mutations.
//!
//! Compared, byte for byte (floats as raw bits): every field of the parsed
//! headers and objects, the level table, the payload, every pixel, block and
//! surface written (destinations start filled with 0xA5, so bytes upstream
//! does not write are compared too), every return value and error code.
//!
//! Inputs or operations for which the Rust reports undefined behaviour are
//! **not** given to the C++ (counted per kind), nor are files whose payload
//! or level table is too large to log (counted).
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::formats::texture::colorspace::recolor;
use rts_lockstep_engine::formats::texture::dds::*;
use rts_lockstep_engine::formats::texture::format::*;
use rts_lockstep_engine::formats::texture::targa::*;
use std::collections::BTreeMap;
use std::sync::Mutex;
use support::{Check, Gen};

/// The wrapper keeps its log in a global.
static CPP_LOCK: Mutex<()> = Mutex::new(());

type Log = Vec<u8>;

fn p32(log: &mut Log, v: u32) {
    log.extend_from_slice(&v.to_le_bytes());
}

fn cpp_log(mut f: impl FnMut(*mut u8, i32, &mut i32) -> i32) -> Log {
    thread_local! {
        static BUF: std::cell::RefCell<Vec<u8>> = std::cell::RefCell::new(vec![0u8; 1 << 16]);
    }
    BUF.with(|b| {
        let mut buf = b.borrow_mut();
        loop {
            let mut n = 0i32;
            let r = f(buf.as_mut_ptr(), buf.len() as i32, &mut n);
            assert_eq!(r, 0, "C++ wrapper failed");
            if n as usize <= buf.len() {
                return buf[..n as usize].to_vec();
            }
            let len = n as usize;
            buf.resize(len, 0);
        }
    })
}

fn mutate(g: &mut Gen, mut data: Vec<u8>) -> Vec<u8> {
    match g.below(4) {
        0 => {
            let n = g.below(data.len() as u32 + 1) as usize;
            data.truncate(n);
        }
        1 => {
            for _ in 0..1 + g.below(4) {
                if !data.is_empty() {
                    let i = g.below(data.len() as u32) as usize;
                    data[i] = g.u32() as u8;
                }
            }
        }
        2 => {
            for _ in 0..g.below(40) {
                data.push(g.u32() as u8);
            }
        }
        _ => {}
    }
    data
}

fn rand_bytes(g: &mut Gen, n: usize) -> Vec<u8> {
    (0..n).map(|_| g.u32() as u8).collect()
}

fn hsv_shift(g: &mut Gen) -> [f32; 3] {
    match g.below(10) {
        0..=4 => [0.0; 3],
        5..=7 => [
            g.uniform(-400.0, 400.0),
            g.uniform(-1.5, 1.5),
            g.uniform(-1.5, 1.5),
        ],
        8 => [g.uniform(-60.0, 60.0), 0.0, 0.0],
        _ => [g.mixed(), g.mixed(), g.mixed()],
    }
}

// ---------------------------------------------------------------------------
// Formats, the pixel writer and Recolor
// ---------------------------------------------------------------------------

#[test]
fn format_tables_match_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("D3DFormat_To_WW3DFormat, Get_Bytes_Per_Pixel");
    let mut g = Gen::new(0xF0F0);
    let mut excluded = 0u64;
    let mut codes: Vec<u32> = (0..300).collect();
    codes.extend([
        D3DFMT_DXT1,
        D3DFMT_DXT2,
        D3DFMT_DXT3,
        D3DFMT_DXT4,
        D3DFMT_DXT5,
        0x7FFF_FFFF,
        0x8000_0000,
        0xFFFF_FFFF,
    ]);
    for _ in 0..20_000 {
        codes.push(g.u32());
    }
    for code in codes {
        match d3d_format_to_ww3d(code) {
            Some(f) => check.eq(f as u32, unsafe { cpp::ref_d3d_to_ww3d(code) }, || {
                format!("code {code:#x}")
            }),
            None => excluded += 1,
        }
    }
    for f in Ww3dFormat::ALL {
        check.eq(
            bytes_per_pixel(f),
            unsafe { cpp::ref_bytes_per_pixel(f as u32) },
            || format!("{f:?}"),
        );
    }
    println!("D3DFormat_To_WW3DFormat: {excluded} codes with the top bit set excluded (undefined upstream)");
    check.finish();
}

#[test]
fn write_b8g8r8a8_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("BitmapHandlerClass::Write_B8G8R8A8");
    let mut g = Gen::new(0xB8B8);
    for i in 0..200_000u32 {
        let f = Ww3dFormat::ALL[(i % 25) as usize];
        let argb = if i < 1000 {
            [0, 0xFFFF_FFFF, 0x8000_0000, 0x00FF_FFFF][i as usize % 4]
        } else {
            g.u32()
        };
        let mut rust = [0xA5u8; 8];
        write_b8g8r8a8(&mut rust, 0, f, argb).expect("8 bytes are enough");
        let mut c = [0u8; 8];
        unsafe { cpp::ref_write_b8g8r8a8(f as u32, argb, c.as_mut_ptr()) };
        check.eq(rust, c, || format!("{f:?} {argb:#010x}"));
    }
    check.finish();
}

#[test]
fn recolor_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut check = Check::new("Recolor (colorspace.h)");
    let mut g = Gen::new(0x45F);
    let mut excluded = 0u64;
    for i in 0..300_000u32 {
        let rgba = match i % 4 {
            0 => g.u32(),
            1 => g.u32() & 0x00F8_FCF8, // as DXT colours expand
            2 => [
                0,
                0x00FF_FFFF,
                0x00FF_0000,
                0x0000_FF00,
                0x0000_00FF,
                0x0080_8080,
            ][g.below(6) as usize],
            _ => g.u32() | 0xFF00_0000,
        };
        let hsv = if i % 5 == 0 {
            [g.special(), g.special(), g.special()]
        } else {
            hsv_shift(&mut g)
        };
        let Ok(rust) = recolor(rgba, hsv) else {
            excluded += 1;
            continue;
        };
        let c = unsafe { cpp::ref_recolor(rgba, hsv.as_ptr()) };
        check.eq(rust, c, || format!("{rgba:#010x} {hsv:?}"));
    }
    println!("Recolor: {excluded} shifts excluded (hue sector not an int: undefined upstream)");
    check.finish();
}

// ---------------------------------------------------------------------------
// DDS
// ---------------------------------------------------------------------------

/// A level table larger than this is not given to the C++ (it would build
/// and log every entry), nor a payload larger than this many bytes.
const MAX_LEVELS: u32 = 1 << 16;
const MAX_PAYLOAD: u32 = 64 << 20;

fn small_dim(g: &mut Gen) -> u32 {
    match g.below(20) {
        0 => 0,
        1..=3 => g.below(8),
        4..=13 => 4 << g.below(6),
        14..=17 => g.below(130),
        18 => 256,
        _ => g.u32(),
    }
}

fn gen_dds(g: &mut Gen) -> (Vec<u8>, u32) {
    let mut f = Vec::new();
    f.extend_from_slice(if g.below(8) == 0 { b"XXXX" } else { b"DDS " });
    let width = small_dim(g);
    let height = small_dim(g);
    let mips = match g.below(10) {
        0..=2 => 0,
        3..=8 => g.below(12),
        _ => g.u32() >> g.below(32),
    };
    let four_cc = match g.below(12) {
        0..=2 => D3DFMT_DXT1,
        3..=5 => D3DFMT_DXT5,
        6 => D3DFMT_DXT2,
        7 => D3DFMT_DXT3,
        8 => D3DFMT_DXT4,
        9 => g.below(70),
        10 => g.u32(),
        _ => 21,
    };
    let caps2 = match g.below(8) {
        0 => DDSCAPS2_CUBEMAP,
        1 => DDSCAPS2_VOLUME,
        2 => g.u32(),
        _ => 0,
    };
    let depth = match g.below(4) {
        0 => 0,
        1 | 2 => 1 + g.below(8),
        _ => g.u32() >> g.below(32),
    };
    let size = match g.below(12) {
        0 => g.below(130),
        1 => g.u32(),
        _ => 124,
    };
    let mut sd = [0u32; 31];
    for w in sd.iter_mut() {
        if g.below(4) == 0 {
            *w = g.u32();
        }
    }
    sd[0] = size;
    sd[2] = height;
    sd[3] = width;
    sd[5] = depth;
    sd[6] = mips;
    sd[20] = four_cc;
    sd[27] = caps2;
    for w in sd {
        f.extend_from_slice(&w.to_le_bytes());
    }
    // the payload the header describes, roughly (all levels, all faces)
    let fmt = d3d_format_to_ww3d(four_cc).unwrap_or(Ww3dFormat::Unknown);
    let mut level = calculate_dxtc_surface_size(width, height, fmt);
    let mut total = 0u64;
    for _ in 0..mips.clamp(1, 16) {
        total += u64::from(level);
        if level > 16 {
            level /= 4;
        }
    }
    if caps2 & DDSCAPS2_CUBEMAP != 0 {
        total *= 6;
    }
    let total = total.min(1 << 18) as usize;
    let len = match g.below(6) {
        0 => g.below(total as u32 + 1) as usize,
        1 => total + g.below(64) as usize,
        2 => g.below(64) as usize,
        _ => total,
    };
    let mut payload = rand_bytes(g, len);
    // some all-equal colour pairs (the 3-colour DXT1 mode is col0 <= col1)
    if g.below(3) == 0 {
        for b in payload.chunks_mut(8) {
            if b.len() == 8 && g.below(4) == 0 {
                b[2] = b[0];
                b[3] = b[1];
            }
        }
    }
    f.extend_from_slice(&payload);
    let reduction = match g.below(10) {
        0..=5 => 0,
        6..=8 => 1 + g.below(4),
        _ => g.below(40),
    };
    (f, reduction)
}

/// One scripted call (see `ref_dds_run`).
#[derive(Clone, Copy, Debug)]
struct Op {
    w: [u32; 9],
    hsv: [f32; 3],
}

fn gen_ops(g: &mut Gen, d: &DdsFile<'_>) -> Vec<Op> {
    let n = g.below(14);
    let mut ops = Vec::new();
    for _ in 0..n {
        let kind = g.below(7);
        let mips = d.mip_levels.max(1);
        let level = match g.below(12) {
            0 => g.below(40),
            1 => mips,
            _ => g.below(mips.min(8)),
        };
        let lw = d.get_width(level).unwrap_or(4);
        let lh = d.get_height(level).unwrap_or(4);
        let fmt = match g.below(6) {
            0 => d.format,
            1 => Ww3dFormat::Dxt2,
            _ => Ww3dFormat::ALL[g.below(25) as usize],
        };
        let (w, h) = if g.below(8) == 0 {
            (small_dim(g), small_dim(g))
        } else {
            (lw, lh)
        };
        let bpp = bytes_per_pixel(fmt).max(1);
        let pitch = match g.below(6) {
            0 => g.below(64),
            1 => (w.min(1024) * bpp).wrapping_add(g.below(8)),
            _ => w.min(1024) * bpp,
        };
        let need = if fmt.is_dxt() {
            u64::from(w.div_ceil(4)) * u64::from(h.div_ceil(4)) * 16
        } else {
            u64::from(pitch) * u64::from(h.min(1024)) + 4
        };
        let blen = match g.below(6) {
            0 => g.below(need.min(1 << 16) as u32 + 1),
            1 => need.min(1 << 18) as u32 + g.below(16),
            _ => need.min(1 << 18) as u32,
        };
        let (x, y) = match kind {
            3 => (
                4 * g.below(lw.div_ceil(4).max(1) + 1),
                4 * g.below(lh.div_ceil(4).max(1) + 1),
            ),
            2 | 5 => (g.below(7), 0),
            _ => (g.below(lw + 2), g.below(lh + 2)),
        };
        let hsv = hsv_shift(g);
        let (w, h, pitch, blen) = if kind == 3 {
            (0, 0, 4 * bpp + g.below(3), (4 * bpp + 2) * 4 + g.below(8))
        } else {
            (w, h, pitch, blen)
        };
        ops.push(Op {
            w: [kind, level, x, y, fmt as u32, w, h, pitch, blen],
            hsv,
        });
    }
    ops
}

fn log_dds_header(log: &mut Log, d: &DdsFile<'_>) {
    p32(log, d.width);
    p32(log, d.height);
    p32(log, d.depth);
    p32(log, d.full_width);
    p32(log, d.full_height);
    p32(log, d.full_depth);
    p32(log, d.mip_levels);
    p32(log, d.date_time);
    p32(log, d.reduction_factor);
    p32(log, d.format as u32);
    p32(
        log,
        match d.dds_type {
            DdsType::Texture => 0,
            DdsType::CubeMap => 1,
            DdsType::Volume => 2,
        },
    );
    p32(log, d.cube_face_size);
    log.extend_from_slice(&d.surface_desc.to_bytes());
    log.push(d.is_available() as u8);
    if d.is_available() && d.mip_levels <= MAX_LEVELS {
        for l in 0..d.mip_levels {
            p32(log, d.get_level_size(l).unwrap());
            p32(log, d.get_level_offset(l).unwrap());
        }
    }
}

/// Runs one operation; `None` if upstream is undefined on it.
fn run_op(d: &DdsFile<'_>, op: &Op, log: &mut Log) -> Result<(), DdsUndefined> {
    let [kind, level, xf, y, fmt, w, h, pitch, blen] = op.w;
    let fmt = Ww3dFormat::from_u32(fmt).unwrap();
    let mut buf = vec![0xA5u8; blen as usize];
    let mut out = vec![b'o', kind as u8];
    match kind {
        0 => p32(&mut out, d.get_pixel(level, xf, y)?),
        1 => {
            d.copy_level_to_surface(level, fmt, w, h, &mut buf, pitch, op.hsv)?;
            out.extend_from_slice(&buf);
        }
        2 => {
            d.copy_cube_map_level_to_surface(xf, level, fmt, w, h, &mut buf, pitch, op.hsv)?;
            out.extend_from_slice(&buf);
        }
        3 => {
            let a = d.get_4x4_block(&mut buf, 0, pitch, fmt, level, xf, y, op.hsv)?;
            out.push(a as u8);
            out.extend_from_slice(&buf);
        }
        4 => {
            p32(&mut out, d.get_width(level)?);
            p32(&mut out, d.get_height(level)?);
            p32(&mut out, d.get_depth(level)?);
        }
        5 => {
            p32(&mut out, d.get_level_size(level)?);
            p32(&mut out, d.get_level_offset(level)?);
            p32(&mut out, d.get_cube_map_offset(xf, level)?);
        }
        _ => {
            d.copy_volume_level_to_surface();
            out.extend_from_slice(&buf);
        }
    }
    log.extend_from_slice(&out);
    Ok(())
}

#[test]
fn dds_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(
        unsafe { cpp::ref_dds_surface_desc_size() },
        LEGACY_DDSURFACEDESC2_SIZE as i32
    );
    let mut g = Gen::new(0xDD5);
    let mut check = Check::new("DDSFileClass (header, Load, reads, copies)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let mut ops_excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut available, mut loaded, mut ops_run, mut large) = (0u64, 0u64, 0u64, 0u64);
    let mut ops_by_kind: BTreeMap<&str, u64> = BTreeMap::new();
    let mut surfaces_written = 0u64;
    const CASES: usize = 20_000;
    for case in 0..CASES {
        let (clean, reduction) = gen_dds(&mut g);
        let data = if case % 2 == 0 {
            clean
        } else {
            mutate(&mut g, clean)
        };
        let mut d = match DdsFile::new(&data, reduction) {
            Ok(d) => d,
            Err(u) => {
                *excluded.entry(format!("{u:?}")).or_default() += 1;
                continue;
            }
        };
        if d.mip_levels > MAX_LEVELS {
            large += 1;
            continue;
        }
        let mut log = Log::new();
        log_dds_header(&mut log, &d);
        available += d.is_available() as u64;
        let do_load = g.below(10) != 0;
        if do_load {
            log.push(b'L');
            let r = match d.load() {
                Ok(r) => r,
                Err(u) => {
                    *excluded.entry(format!("{u:?}")).or_default() += 1;
                    continue;
                }
            };
            log.push(r as u8);
            match d.memory() {
                Some(m) if m.size > MAX_PAYLOAD => {
                    large += 1;
                    continue;
                }
                Some(m) => {
                    loaded += 1;
                    log.push(1);
                    p32(&mut log, m.size);
                    let shown = (m.size as usize).min(data.len());
                    let mut bytes = m.bytes.clone();
                    bytes.resize(shown, 0);
                    log.extend_from_slice(&bytes[..shown]);
                    log.push(1);
                }
                None => log.push(0),
            }
        }
        let mut kept = Vec::new();
        for op in gen_ops(&mut g, &d) {
            // the C++ block loop calls Get_4x4_Block for every block of the
            // destination even when it does nothing (DXT2-4 and other
            // formats): keep the ones it would take long over to the Rust
            let [kind, _, _, _, _, w, h, ..] = op.w;
            if (kind == 1 || kind == 2)
                && u64::from(w.div_ceil(4)) * u64::from(h.div_ceil(4)) > 1 << 20
            {
                let mut scratch = Log::new();
                let r = run_op(&d, &op, &mut scratch);
                let why = match r {
                    Ok(()) => "over 2^20 blocks".to_string(),
                    Err(u) => format!("over 2^20 blocks, and {u:?}"),
                };
                *ops_excluded.entry(why).or_default() += 1;
                continue;
            }
            let before = log.len();
            match run_op(&d, &op, &mut log) {
                Ok(()) => {
                    // a destination buffer that is no longer all 0xA5
                    if matches!(op.w[0], 1..=3) && log[before + 2..].iter().any(|&b| b != 0xA5) {
                        surfaces_written += 1;
                    }
                    kept.push(op)
                }
                Err(u) => *ops_excluded.entry(format!("{u:?}")).or_default() += 1,
            }
        }
        ops_run += kept.len() as u64;
        for o in &kept {
            *ops_by_kind
                .entry(
                    [
                        "Get_Pixel",
                        "Copy_Level",
                        "Copy_CubeMap",
                        "Get_4x4_Block",
                        "Get_Width/Height/Depth",
                        "level size/offset",
                        "Copy_Volume",
                    ][o.w[0] as usize],
                )
                .or_default() += 1;
        }
        let words: Vec<u32> = kept.iter().flat_map(|o| o.w).collect();
        let hsv: Vec<f32> = kept.iter().flat_map(|o| o.hsv).collect();
        let c = cpp_log(|lg, cap, n| unsafe {
            cpp::ref_dds_run(
                data.as_ptr() as *const _,
                data.len() as i32,
                reduction,
                do_load as i32,
                MAX_LEVELS,
                words.as_ptr(),
                hsv.as_ptr(),
                kept.len() as i32,
                lg,
                cap,
                n,
            )
        });
        check.eq(&log, &c, || {
            format!(
                "case {case} (reduction {reduction}, load {do_load}, ops {kept:?}): {data:02x?}"
            )
        });
    }
    println!(
        "DDS: {CASES} files, {available} with a header, {loaded} with a payload; {ops_run} operations compared {ops_by_kind:?} \
         ({surfaces_written} of the copies and blocks wrote pixels); \
         files excluded (undefined upstream): {} {excluded:?}; operations excluded (undefined upstream, or too slow for the C++): {} {ops_excluded:?}; \
         files excluded (over {MAX_LEVELS} levels or {MAX_PAYLOAD} payload bytes): {large}",
        excluded.values().sum::<u64>(),
        ops_excluded.values().sum::<u64>()
    );
    check.finish();
}

// ---------------------------------------------------------------------------
// Targa
// ---------------------------------------------------------------------------

/// Images larger than this are not given to the C++.
const MAX_IMAGE: i64 = 16 << 20;

fn gen_tga(g: &mut Gen) -> Vec<u8> {
    let mut h = TgaHeader {
        id_length: match g.below(6) {
            0 => g.below(20) as i8,
            1 => g.u32() as i8,
            _ => 0,
        },
        color_map_type: match g.below(6) {
            0 | 1 => 1,
            2 => g.u32() as i8,
            _ => 0,
        },
        image_type: [1, 2, 3, 9, 10, 11, 0, 2, 10, 2][g.below(10) as usize],
        cmap_start: if g.below(6) == 0 {
            (g.u32() % 300) as i16 - 20
        } else {
            0
        },
        cmap_length: match g.below(4) {
            0 => 0,
            1 => g.u32() as i16,
            _ => g.below(300) as i16,
        },
        cmap_depth: [24, 32, 16, 15, 8][g.below(5) as usize],
        x_offset: g.u32() as i16,
        y_offset: g.u32() as i16,
        width: match g.below(10) {
            0 => 0,
            1 => g.u32() as i16,
            2 => -(g.below(8) as i16),
            _ => g.below(40) as i16,
        },
        height: match g.below(10) {
            0 => 0,
            1 => g.u32() as i16,
            2 => -(g.below(8) as i16),
            _ => g.below(40) as i16,
        },
        pixel_depth: match g.below(10) {
            0 => g.u32() as i8,
            1 => 15,
            2 => 8,
            3 | 4 => 16,
            5 | 6 => 24,
            _ => 32,
        },
        image_descriptor: (g.u32() as i8) & if g.below(2) == 0 { 0x30 } else { -1 },
    };
    if g.below(10) == 0 {
        h.cmap_depth = g.u32() as i8;
    }
    if h.image_type == 1 || h.image_type == 9 {
        h.pixel_depth = 8;
    }
    if g.below(15) == 0 {
        h.image_type = g.u32() as i8;
    }
    let mut f = h.to_bytes().to_vec();
    if h.id_length > 0 {
        f.extend(rand_bytes(g, h.id_length as usize));
    }
    if h.color_map_type == 1 {
        let n =
            (i64::from(h.cmap_length) * i64::from(h.cmap_depth >> 3)).clamp(0, 1 << 16) as usize;
        f.extend(rand_bytes(g, n));
    }
    let depth = i64::from(tga_bytes_per_pixel(h.pixel_depth)).clamp(0, 16) as usize;
    let pixels = (i64::from(h.width) * i64::from(h.height)).clamp(0, 1 << 14) as usize;
    if h.image_type == 9 || h.image_type == 10 || h.image_type == 11 {
        // run-length packets for the pixels (sometimes one too many)
        let mut left = pixels as i64
            + if g.below(8) == 0 {
                1 + i64::from(g.below(3))
            } else {
                0
            };
        while left > 0 {
            let count = (1 + g.below(128) as i64).min(if g.below(4) == 0 { 128 } else { left });
            if g.below(2) == 0 {
                f.push(0x80 | (count - 1) as u8);
                f.extend(rand_bytes(g, depth));
            } else {
                f.push((count - 1) as u8);
                f.extend(rand_bytes(g, depth * count as usize));
            }
            left -= count;
        }
    } else {
        f.extend(rand_bytes(g, pixels * depth));
    }
    match g.below(4) {
        0 => {}
        1 => f.extend(rand_bytes(g, 26)),
        _ => {
            // a Targa 2.0 extension area and footer
            let ext_at = f.len() as i32;
            let mut ext = rand_bytes(g, TGA2_EXTENSION_SIZE);
            ext[0..2].copy_from_slice(&495i16.to_le_bytes());
            f.extend(ext);
            let off = match g.below(6) {
                0 => 0,
                1 => g.i32(),
                2 => ext_at + g.below(600) as i32,
                _ => ext_at,
            };
            f.extend_from_slice(&off.to_le_bytes());
            f.extend_from_slice(&0i32.to_le_bytes());
            f.extend_from_slice(if g.below(5) == 0 {
                b"TRUEVISION-XFILF"
            } else {
                TGA2_SIGNATURE
            });
            f.extend_from_slice(b".\0");
        }
    }
    f
}

fn log_tga_state(log: &mut Log, t: &Targa<'_>, err: i32) {
    p32(log, err as u32);
    log.extend_from_slice(&t.header.to_bytes());
    p32(log, t.flags() as u32);
    log.extend_from_slice(&t.raw_extension().bytes);
    log.push(t.extension().is_some() as u8);
    log.push(t.is_compressed() as u8);
    log.push(t.is_open() as u8);
}

/// The Rust side of `ref_tga_run`, and how many pixel bytes a successful
/// `Load` gave (`None` if it failed); `Err` where upstream is undefined.
fn rust_tga(data: &[u8], flow: i32) -> Result<(Log, Option<usize>), TgaUndefined> {
    let mut log = Log::new();
    let mut ok = None;
    let mut t = Targa::new();
    if flow == 0 {
        let err = t.open(data)?;
        log.push(b'O');
        log_tga_state(&mut log, &t, err);
        if err == 0 {
            t.header.image_descriptor ^= TGAIDF_YORIGIN;
            let (src, bpp) = ww3d_format_of_targa(
                t.header.pixel_depth,
                t.header.color_map_type,
                t.header.image_type,
            );
            p32(&mut log, src as u32);
            p32(&mut log, bpp);
            t.set_palette(Some(vec![0; 1024]));
            let err = t.load(data, TGAF_IMAGE, false)?;
            log.push(b'L');
            log_tga_state(&mut log, &t, err);
            ok = (err == 0).then(|| t.image().map_or(0, |i| i.len()));
            if err == 0 {
                log.push(t.image().is_some() as u8);
                if let Some(img) = t.image() {
                    log.extend_from_slice(img);
                }
                log.extend_from_slice(t.palette().expect("the palette set above"));
            }
        }
    } else {
        let err = t.load(data, TGAF_IMAGE | TGAF_PAL, true)?;
        log.push(b'L');
        log_tga_state(&mut log, &t, err);
        ok = (err == 0).then(|| t.image().map_or(0, |i| i.len()));
        if err == 0 {
            log.push(t.image().is_some() as u8);
            if let Some(img) = t.image() {
                log.extend_from_slice(img);
            }
            log.push(t.palette().is_some() as u8);
            // upstream reads the palette only for a positive length; with a
            // negative one (and depth) it allocates it and leaves it
            // uninitialised, so only its presence is compared
            if let (Some(p), true) = (t.palette(), t.header.cmap_length > 0) {
                log.extend_from_slice(p);
            }
        }
    }
    Ok((log, ok))
}

#[test]
fn targa_matches_upstream() {
    let _l = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x7A6A);
    let mut check = Check::new("Targa (Open, Load, flips, RLE, palette)");
    let mut excluded: BTreeMap<String, u64> = BTreeMap::new();
    let (mut loaded, mut with_pixels, mut large) = (0u64, 0u64, 0u64);
    const CASES: usize = 20_000;
    for case in 0..CASES {
        let clean = gen_tga(&mut g);
        let data = if case % 2 == 0 {
            clean
        } else {
            mutate(&mut g, clean)
        };
        // the header as Open will read it, to keep huge images out
        let hd = (data.len() >= TGA_HEADER_SIZE)
            .then(|| TgaHeader::from_bytes(data[..TGA_HEADER_SIZE].try_into().unwrap()));
        if let Some(h) = hd {
            let size = i64::from(h.width)
                * i64::from(h.height)
                * i64::from(tga_bytes_per_pixel(h.pixel_depth));
            if size.abs() > MAX_IMAGE {
                large += 2;
                continue;
            }
        }
        for flow in 0..2 {
            let rust = match rust_tga(&data, flow) {
                Ok((l, ok)) => {
                    loaded += ok.is_some() as u64;
                    with_pixels += (ok.unwrap_or(0) > 0) as u64;
                    l
                }
                Err(u) => {
                    *excluded.entry(format!("{u:?}")).or_default() += 1;
                    continue;
                }
            };
            let c = cpp_log(|lg, cap, n| unsafe {
                cpp::ref_tga_run(
                    data.as_ptr() as *const _,
                    data.len() as i32,
                    flow,
                    lg,
                    cap,
                    n,
                )
            });
            if rust != c && std::env::var_os("TGA_TRACE").is_some() {
                let at = rust
                    .iter()
                    .zip(&c)
                    .position(|(a, b)| a != b)
                    .unwrap_or(rust.len().min(c.len()));
                eprintln!("case {case} flow {flow}: len {} vs {}, first difference at {at}: rust {:02x?} c++ {:02x?}; header {:02x?}",
                    rust.len(), c.len(), &rust[at.saturating_sub(4)..(at + 16).min(rust.len())], &c[at.saturating_sub(4)..(at + 16).min(c.len())], &data[..18.min(data.len())]);
            }
            check.eq(&rust, &c, || {
                format!("case {case} flow {flow}: {data:02x?}")
            });
        }
    }
    println!(
        "Targa: {CASES} files x 2 load sequences, {loaded} loads that returned 0 ({with_pixels} with pixels); excluded (undefined upstream): {} {excluded:?}; \
         excluded (images over {MAX_IMAGE} bytes): {large}",
        excluded.values().sum::<u64>()
    );
    check.finish();
}
