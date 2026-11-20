// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/surfaceclass.rs` (`pixel_size`, `convert_pixel_read`, `convert_pixel_write`, `has_alpha`, `alpha_bits`
//! and `SurfaceClass::{clear, copy_from_bytes, copy_rect_from_bytes, create_copy, find_bb, is_transparent_column, get_pixel, draw_pixel, draw_hline, is_monochrome, hue_shift}`) against
//! upstream `WW3D2/surfaceclass.cpp`'s functions of the same names, cut VERBATIM (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_surf.cpp), with the real `surfaceclass.h`,
//! `ww3dformat.h` and `WWLib/bound.h` and colorspace.h's `RGB_To_HSV` / `HSV_To_RGB` / `Recolor`. One named edit: `(unsigned int)lock_rect.pBits` is `(uintptr_t)` (a 32-bit pointer
//! truncation upstream). Stand-in: `IDirect3DSurface8` as a byte buffer with a tight pitch (`GetDesc`, `LockRect` giving the rectangle's top-left pixel, `UnlockRect`).
//! NOT covered: `copy_from_surface` (D3DX `D3DXLoadSurfaceFromSurface`; the draft's non-same-format path is its own per-pixel fallback), `Stretch_Copy`, the constructors from a file or
//! a D3D surface, `Attach`/`Detach`, and the compressed-format branch of `Is_Monochrome` (needs `Get_Valid_Texture_Format`, the device caps).
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides): (a) `PixelSize`, `Has_Alpha`, `Alpha_Bits` for all 25 formats; (b) `Convert_Pixel` both ways on random bytes (the six formats it
//! handles; write inputs in 0..1 with the exact values 0, 0.5, k/255, 1) ; (c) a surface of 1 to 6 by 1 to 6 pixels in one of the 16 formats with a pixel size, filled with random
//! bytes or with gray pixels (so that `Is_Monochrome` is often true), then 1 to 4 operations among those the format supports. Compared: every returned value and every byte of the surface
//! after each mutating operation.
//!
//! # Excluded inputs
//!
//! - `Convert_Pixel` on a format it does not handle (upstream's `WWASSERT(0)` leaves `rgb` uninitialised): `Get_Pixel`, `Is_Monochrome` (non-trivial formats) and `Hue_Shift` run on the six
//!   handled formats only.
//! - `rgb` outside 0..1 in `Convert_Pixel` (write): `(unsigned char)(rgb.X * 255.0f)` is undefined there.
//! - `FindBB` and `Is_Transparent_Column` on formats without alpha; `Get_Pixel` with a negative coordinate; rectangles outside the surface.
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! Mostly 0. Two places to watch: the draft's `read_argb`-based fallback copy is untested here (D3DX); and `Is_Monochrome`, where upstream's early `return false` for `A8P8`/`P8`/bump formats
//! and the draft's agree by reading.
//!
//! # Results (2026-11-20)
//!
//! **First run: the draft panicked** (`range end index 25 out of range for slice of length 24`, `hue_shift`): `get_pixel`, `is_monochrome` and `hue_shift` sliced `size.max(3)` bytes
//! for the pixel, which for the last pixel of a 2-byte format reads past the buffer where upstream reads the pixel's own bytes. A draft defect (the prediction of "mostly 0" held for
//! the arithmetic, the slice length was not foreseen). Fixed in the draft (`size`). **After the fix: 0 mismatches**: 20,000 `PixelSize`/`Has_Alpha`/`Alpha_Bits`, 20,000 `Convert_Pixel`
//! pairs, 40,000 surface cases (20,000 with 1 to 4 operations each, 20,000 early-answer `Is_Monochrome`).
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching comparisons over the three checks)
//!   19151: 2,075; 19152: 3,623; 19153: 3,308; 19154: 2,221; 19155: 2,497; 19156: 14,172; 19157: 2,162; 19158: 1,482; 19159: 573; 19160: 5,212; 19161: 69; 19162: 1,966
//!   (first versions of 19155 and 19159 were caught by a panic (an out-of-range slice), not by a mismatch, and were replaced; the first 19162, `x <= y` in `Is_Monochrome`, was an equivalent
//!   mutant: the other two comparisons force equality.)
//!
//! **12 of 12 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::surfaceclass::*;
use rts_lockstep_engine::formats::texture::format::Ww3dFormat;
use rts_lockstep_engine::ww::vector3::Vector3;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_surf_run(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn b(f: f32) -> u32 {
    f.to_bits()
}

use Ww3dFormat::*;
const SIZED: [Ww3dFormat; 16] = [A8R8G8B8, X8R8G8B8, R8G8B8, R5G6B5, X1R5G5B5, A1R5G5B5, A4R4G4B4, A8R3G3B2, X4R4G4B4, A8P8, A8L8, R3G3B2, A8, P8, L8, A4L4];
const CONV: [Ww3dFormat; 6] = [A8R8G8B8, X8R8G8B8, R8G8B8, A4R4G4B4, A1R5G5B5, R5G6B5];
const ALPHA: [Ww3dFormat; 8] = [A8R8G8B8, A1R5G5B5, A4R4G4B4, A8, A8R3G3B2, A8P8, A8L8, A4L4];
const MONO_EARLY: [Ww3dFormat; 10] = [A8L8, A8, L8, A4L4, Unknown, A8P8, P8, U8V8, L6V5U5, X8L8V8U8];

fn unit(g: &mut Gen) -> f32 {
    match g.below(5) {
        0 => 0.0,
        1 => 1.0,
        2 => 0.5,
        3 => g.below(256) as f32 / 255.0,
        _ => g.uniform(0.0, 1.0),
    }
}

fn ops_for(format: Ww3dFormat) -> Vec<u32> {
    let mut ops = vec![3, 4, 5, 6, 10, 11];
    if ALPHA.contains(&format) {
        ops.extend([7, 8]);
    }
    if CONV.contains(&format) {
        ops.extend([9, 12, 13]);
    }
    ops
}

fn dump(out: &mut Vec<u32>, s: &SurfaceClass) {
    let (bytes, _, _, _) = s.create_copy(false);
    out.extend(bytes.iter().map(|x| u32::from(*x)));
}

#[test]
fn surfaceclass_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_0a);
    let mut chk_a = Check::new("PixelSize / Has_Alpha / Alpha_Bits");
    let mut chk_b = Check::new("Convert_Pixel (read, write)");
    let mut chk_c = Check::new("SurfaceClass pixel operations");
    let mut out = vec![0u32; 200_000];
    for case in 0..support_sweep::cases() {
        // --- (a)
        {
            let fmt = Ww3dFormat::ALL[g.below(25) as usize];
            let sd = SurfaceDescription { format: fmt, width: 1, height: 1 };
            let input = vec![fmt as u32, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0];
            let n = unsafe { ref_w6_surf_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
            chk_a.eq(vec![pixel_size(&sd), u32::from(has_alpha(fmt)), alpha_bits(fmt) as u32], out[..n].to_vec(), || format!("case {case} {fmt:?}"));
        }
        // --- (b)
        {
            let fmt = CONV[g.below(6) as usize];
            let sd = SurfaceDescription { format: fmt, width: 1, height: 1 };
            let bytes: Vec<u8> = (0..8).map(|_| g.u32() as u8).collect();
            let mut input = vec![fmt as u32, 2, 1, 8];
            input.extend(bytes.iter().map(|x| u32::from(*x)));
            // op 1 reads the pixel at offset 0, op 2 writes `rgb` there
            let rgb = Vector3::new(unit(&mut g), unit(&mut g), unit(&mut g));
            input.push(2);
            input.extend([1, 0, 0, 0, 0, 0, 0, 0]);
            input.extend([2, b(rgb.x), b(rgb.y), b(rgb.z), 0, 0, 0, 0]);
            let n = unsigned_run(&input, &mut out);
            let read = convert_pixel_read(&sd, &bytes);
            let mut wb = bytes.clone();
            convert_pixel_write(&mut wb, &sd, &rgb);
            let mut rust = vec![b(read.x), b(read.y), b(read.z)];
            rust.extend(wb.iter().map(|x| u32::from(*x)));
            chk_b.eq(rust, out[..n].to_vec(), || format!("case {case} {fmt:?} bytes {bytes:?} rgb {rgb:?}"));
        }
        // --- (c)
        {
            let fmt = SIZED[g.below(16) as usize];
            let (w, h) = (1 + g.below(6), 1 + g.below(6));
            let sd = SurfaceDescription { format: fmt, width: w, height: h };
            let size = pixel_size(&sd) as usize;
            let n_bytes = size * (w * h) as usize;
            let gray = g.below(2) == 0 && CONV.contains(&fmt);
            let mut bytes: Vec<u8> = (0..n_bytes).map(|_| g.u32() as u8).collect();
            if gray {
                for p in 0..(w * h) as usize {
                    let v = (g.below(256) as u8) & 0xf8;
                    let px = &mut bytes[p * size..(p + 1) * size];
                    match fmt {
                        A8R8G8B8 | X8R8G8B8 | R8G8B8 => {
                            px[0] = v;
                            px[1] = v;
                            px[2] = v;
                        }
                        R5G6B5 => {
                            let t = (u16::from(v) << 8) | ((u16::from(v) & 0xfc) << 3) | (u16::from(v) >> 3);
                            px.copy_from_slice(&t.to_le_bytes());
                        }
                        A1R5G5B5 => {
                            let t = (u16::from(v) << 7) | (u16::from(v) << 2) | (u16::from(v) >> 3) | (u16::from(px[1] & 0x80) << 8);
                            px.copy_from_slice(&t.to_le_bytes());
                        }
                        _ => {
                            let v4 = u16::from(v >> 4);
                            let t = (v4 << 8) | (v4 << 4) | v4 | (u16::from(px[1] & 0xf0) << 8);
                            px.copy_from_slice(&t.to_le_bytes());
                        }
                    }
                }
            }
            let mut s = SurfaceClass::from_pixels(w, h, fmt, bytes.clone());
            let mut input = vec![fmt as u32, w, h, n_bytes as u32];
            input.extend(bytes.iter().map(|x| u32::from(*x)));
            let nops = 1 + g.below(4);
            input.push(nops);
            let mut expect: Vec<u32> = Vec::new();
            let allowed = ops_for(fmt);
            for _ in 0..nops {
                let kind = allowed[g.below(allowed.len() as u32) as usize];
                let mut frame = [0u32; 7];
                let mut extra: Vec<u32> = Vec::new();
                match kind {
                    3 => {
                        s.clear();
                        dump(&mut expect, &s);
                    }
                    4 | 5 => {
                        let other: Vec<u8> = (0..n_bytes).map(|_| g.u32() as u8).collect();
                        extra = other.iter().map(|x| u32::from(*x)).collect();
                        if kind == 4 {
                            s.copy_from_bytes(&other);
                        } else {
                            let (x0, y0) = (g.below(w), g.below(h));
                            let (x1, y1) = (x0 + g.below(w - x0 + 1), y0 + g.below(h - y0 + 1));
                            frame[..4].copy_from_slice(&[x0, y0, x1, y1]);
                            s.copy_rect_from_bytes((x0 as i32, y0 as i32), (x1 as i32, y1 as i32), &other);
                        }
                        dump(&mut expect, &s);
                    }
                    6 => {
                        let flip = g.below(2);
                        frame[0] = flip;
                        let (bytes, cw, ch, cs) = s.create_copy(flip != 0);
                        expect.extend([cw, ch, cs]);
                        expect.extend(bytes.iter().map(|x| u32::from(*x)));
                    }
                    7 => {
                        let (x0, y0) = (g.below(w), g.below(h));
                        let (x1, y1) = (x0 + 1 + g.below(w - x0), y0 + 1 + g.below(h - y0));
                        frame[..4].copy_from_slice(&[x0, y0, x1, y1]);
                        let (mut mn, mut mx) = ((x0 as i32, y0 as i32), (x1 as i32, y1 as i32));
                        s.find_bb(&mut mn, &mut mx);
                        expect.extend([mn.0 as u32, mn.1 as u32, mx.0 as u32, mx.1 as u32]);
                    }
                    8 => {
                        let col = g.below(w);
                        frame[0] = col;
                        expect.push(u32::from(s.is_transparent_column(col)));
                    }
                    9 => {
                        let (x, y) = (g.below(w + 2) as i32, g.below(h + 2) as i32);
                        frame[0] = x as u32;
                        frame[1] = y as u32;
                        let v = s.get_pixel(x, y);
                        expect.extend([b(v.x), b(v.y), b(v.z)]);
                    }
                    10 => {
                        let (x, y, c) = (g.below(w), g.below(h), g.u32());
                        frame[..3].copy_from_slice(&[x, y, c]);
                        s.draw_pixel(x, y, c);
                        dump(&mut expect, &s);
                    }
                    11 => {
                        let y = g.below(h);
                        let x1 = g.below(w);
                        let x2 = x1 + g.below(w - x1);
                        let c = g.u32();
                        frame[..4].copy_from_slice(&[y, x1, x2, c]);
                        s.draw_hline(y, x1, x2, c);
                        dump(&mut expect, &s);
                    }
                    12 => {
                        expect.push(u32::from(s.is_monochrome().expect("a handled format")));
                    }
                    _ => {
                        let hsv = Vector3::new(g.uniform(-60.0, 60.0), g.uniform(-0.5, 0.5), g.uniform(-0.5, 0.5));
                        frame[..3].copy_from_slice(&[b(hsv.x), b(hsv.y), b(hsv.z)]);
                        s.hue_shift(&hsv);
                        dump(&mut expect, &s);
                    }
                }
                input.push(kind);
                input.extend(frame);
                input.extend(extra);
            }
            // is_monochrome on the formats that return early: separate cases below
            let n = unsigned_run(&input, &mut out);
            chk_c.eq(expect, out[..n].to_vec(), || format!("case {case} {fmt:?} {w}x{h}"));
        }
        // --- Is_Monochrome on the formats with an early answer (no pixel read)
        {
            let fmt = MONO_EARLY[g.below(10) as usize];
            let s = SurfaceClass::from_pixels(1, 1, fmt, vec![0u8; 8]);
            // upstream reads nothing for these formats: the surface bytes are irrelevant
            let input = vec![fmt as u32, 1, 1, 0, 1, 12, 0, 0, 0, 0, 0, 0, 0];
            let n = unsigned_run(&input, &mut out);
            chk_c.eq(vec![u32::from(s.is_monochrome().expect("early answer"))], out[..n].to_vec(), || format!("case {case} Is_Monochrome {fmt:?}"));
        }
    }
    support_sweep::finish(vec![chk_a, chk_b, chk_c]);
}

fn unsigned_run(input: &[u32], out: &mut [u32]) -> usize {
    unsafe { ref_w6_surf_run(input.as_ptr(), out.as_mut_ptr()) as usize }
}
