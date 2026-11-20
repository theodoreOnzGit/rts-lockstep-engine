// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/bitmaphandler.rs` against upstream
//! `WW3D2/bitmaphandler.cpp` (compiled unchanged, `Bitmap_Assert` renamed because wrap_texture.cpp defines it too) and the inline members of
//! `bitmaphandler.h` (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_bm.cpp). `Recolor` is the real `colorspace.h`, over the
//! `DX8Wrapper::Convert_Color` already transcribed (x87 asm) in wrap_w3d_mesh.cpp.
//!
//! # Method
//!
//! 20,000 cases per function (fixed seed, `DRAFT_CASES` overrides). Surfaces are random bytes with 16 zero bytes in front and 16 behind
//! (upstream reads one pixel before a row start and after the last pixel; the draft reads 0 outside its slice, so the C++ reads the same
//! zeros). `Read_B8G8R8A8` (both overloads and the x,y one), `Write_B8G8R8A8` (R3G3B2 writes two bytes), `Copy_Pixel` (same format and
//! converting, with and without an HSV shift), `Combine_A8R8G8B8`, `Create_Mipmap_B8G8R8A8`, `Copy_Image_Generate_Mipmap` and `Copy_Image`
//! (bump-map destinations, 32-bit copies, scaling, mip generation in place, per-pixel conversions, pitches with padding, palettes of 3 and 4 bytes).
//! Every output byte of every surface (the source too, where the mip level is written into it) is compared.
//!
//! # Excluded inputs
//!
//! Source formats the reader cannot read (DXT, X1R5G5B5, A8R3G3B2, ..., P8 without a palette): upstream leaves a local `unsigned` uninitialised
//! there (undefined); the draft leaves it 0. They are tested for `Read_B8G8R8A8` itself (output left as it was) and as destinations (writes ignored).
//! Bump-map destinations larger than the source (reads outside the surface beyond the guard bytes). Zero-sized destinations (the draft asserts, upstream's
//! `WWASSERT` is compiled out and the loops do nothing).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! 0 mismatches. The place to watch is `Copy_Image`'s many branches; the draft calls the port's verified `write_b8g8r8a8` and `recolor`.
//!
//! # Results (2026-11-20)
//!
//! **0 mismatches**: Read 20,000 (both overloads) and 20,000 (x, y), Write 20,000, Copy_Pixel 10,081 and 9,919 (HSV), Combine 20,000, Create_Mipmap 20,000,
//! Copy_Image_Generate_Mipmap 20,000, Copy_Image 6,005 (bump), 6,023 (32-bit), 7,972 (general). **Prediction held.** The R3G3B2 two-byte write (upstream writes an
//! `unsigned short` into a one-byte pixel) is reproduced.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching comparisons over all functions)
//!   19071: 58,881; 19072: 59,019; 19073: 460; 19074: 299; 19075: 961; 19076: 2,100; 19077: 4,589; 19078: 15; 19079: 2,321; 19080: 7,945
//!
//! **10 of 10 caught.**

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::bitmaphandler::*;
use rts_lockstep_engine::formats::texture::format::{bytes_per_pixel, Ww3dFormat};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_bm_read(fmt: u32, pal: *const u8, pal_len: u32, pal_bpp: u32, src: *const u8, argb_in: *const u8, out: *mut u8);
    fn ref_w6_bm_read_xy(fmt: u32, x: i32, y: i32, w: i32, h: i32, src: *const u8, pal: *const u8, pal_bpp: u32, argb_in: u32) -> u32;
    fn ref_w6_bm_write(fmt: u32, argb: u32, buf8: *mut u8);
    fn ref_w6_bm_copy_pixel(dfmt: u32, sfmt: u32, src: *const u8, pal: *const u8, pal_bpp: u32, use_hsv: i32, hsv: *const f32, buf8: *mut u8);
    fn ref_w6_bm_combine(a: u32, b: u32, c: u32, d: u32) -> u32;
    fn ref_w6_bm_mip(dest: *mut u8, dpitch: u32, src: *mut u8, spitch: u32, w: u32, h: u32);
    fn ref_w6_bm_genmip(w: u32, h: u32, dest: *mut u8, dpitch: u32, dfmt: u32, src: *mut u8, spitch: u32, sfmt: u32, mip: *mut u8, mpitch: u32, hsv: *const f32);
    fn ref_w6_bm_copy_image(dest: *mut u8, dw: u32, dh: u32, dp: u32, dfmt: u32, src: *mut u8, sw: u32, sh: u32, sp: u32, sfmt: u32, pal: *const u8, pal_bpp: u32, genmip: i32, hsv: *const f32);
}

static CPP: Mutex<()> = Mutex::new(());
const PRE: usize = 16;
const POST: usize = 16;

fn fmt_of(i: u32) -> Ww3dFormat {
    Ww3dFormat::ALL[i as usize]
}

/// The formats Read_B8G8R8A8 handles (P8 needs a palette).
fn readable(g: &mut Gen) -> Ww3dFormat {
    use Ww3dFormat::*;
    [A8R8G8B8, X8R8G8B8, R8G8B8, A4R4G4B4, A1R5G5B5, R5G6B5, R3G3B2, L8, A8][g.below(9) as usize]
}
/// Any format but Unknown and the bump-map ones (those destinations take Copy_Image's other branch, tested apart).
fn any_format(g: &mut Gen) -> Ww3dFormat {
    loop {
        let f = fmt_of(1 + g.below(24));
        if !matches!(f, Ww3dFormat::U8V8 | Ww3dFormat::L6V5U5 | Ww3dFormat::X8L8V8U8) {
            return f;
        }
    }
}
/// A destination Write_B8G8R8A8 accepts, or (a fifth of the time) any other format (writes ignored).
fn dest_format(g: &mut Gen) -> Ww3dFormat {
    if g.below(5) == 0 {
        any_format(g)
    } else {
        readable(g)
    }
}

/// zero guard, `len` random bytes, zero guard
fn surface(g: &mut Gen, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; PRE + len + POST];
    for b in &mut v[PRE..PRE + len] {
        *b = g.u32() as u8;
    }
    v
}

fn hsv(g: &mut Gen) -> [f32; 3] {
    match g.below(4) {
        0 | 1 => [0.0; 3],
        2 => [g.uniform(-90.0, 90.0), g.uniform(-0.5, 0.5), g.uniform(-0.5, 0.5)],
        _ => [[0.0, 30.0, -45.0][g.below(3) as usize], [0.0, 0.25, -0.25][g.below(3) as usize], [0.0, 0.5, -0.5][g.below(3) as usize]],
    }
}

fn palette(g: &mut Gen, bpp: u32) -> Vec<u8> {
    (0..256 * bpp.max(1) as usize + 4).map(|_| g.u32() as u8).collect()
}

fn pal_bpp(g: &mut Gen) -> u32 {
    [3, 4, 4, 3, 1, 2, 5, 0][g.below(8) as usize]
}

#[test]
fn bitmaphandler_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_02);
    let names = [
        "Read_B8G8R8A8 (bytes, word)", "Read_B8G8R8A8 (x, y)", "Write_B8G8R8A8", "Copy_Pixel", "Copy_Pixel (HSV)", "Combine_A8R8G8B8", "Create_Mipmap_B8G8R8A8",
        "Copy_Image_Generate_Mipmap", "Copy_Image (bump)", "Copy_Image (32-bit)", "Copy_Image (general)",
    ];
    let mut c: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..support_sweep::cases() {
        // --- reads
        {
            let fmt = if g.below(6) == 0 { fmt_of(g.below(25)) } else if g.below(5) == 0 { Ww3dFormat::P8 } else { readable(&mut g) };
            let pb = pal_bpp(&mut g);
            let pal = palette(&mut g, pb);
            let src = surface(&mut g, 8);
            let argb_in: [u8; 4] = [g.u32() as u8, g.u32() as u8, g.u32() as u8, g.u32() as u8];
            let mut out = [0u8; 8];
            unsafe { ref_w6_bm_read(fmt as u32, pal.as_ptr(), pal.len() as u32, pb, src.as_ptr().add(PRE), argb_in.as_ptr(), out.as_mut_ptr()) };
            let mut b = argb_in;
            read_b8g8r8a8_bytes(&mut b, &src[PRE..], 0, fmt, &pal, pb);
            let mut w = u32::from_le_bytes(argb_in);
            read_b8g8r8a8(&mut w, &src[PRE..], 0, fmt, &pal, pb);
            let mut rust = b.to_vec();
            rust.extend(w.to_le_bytes());
            c[0].eq(rust, out.to_vec(), || format!("case {case} {fmt:?} pal_bpp {pb} src {:?}", &src[PRE..PRE + 8]));

            // x, y overload: a tightly packed w x h image (the reader needs bpp * w * h bytes)
            let rf = readable(&mut g);
            let (w_, h_) = (1 + g.below(5) as i32, 1 + g.below(5) as i32);
            let img = surface(&mut g, (bytes_per_pixel(rf) as i32 * w_ * h_) as usize);
            let (x, y) = (g.below(w_ as u32 + 4) as i32 - 2, g.below(h_ as u32 + 4) as i32 - 2);
            let init = g.u32();
            let cpp = unsafe { ref_w6_bm_read_xy(rf as u32, x, y, w_, h_, img.as_ptr().add(PRE), pal.as_ptr(), pb, init) };
            let mut r = init;
            read_b8g8r8a8_xy(&mut r, &img[PRE..], rf, x, y, w_, h_, &pal, pb);
            c[1].eq(r, cpp, || format!("case {case} {rf:?} ({x},{y}) in {w_}x{h_}"));
        }
        // --- write
        {
            let fmt = if g.below(5) == 0 { fmt_of(g.below(25)) } else { readable(&mut g) };
            let argb = g.u32();
            let init: Vec<u8> = (0..8).map(|_| g.u32() as u8).collect();
            let mut cpp = init.clone();
            unsafe { ref_w6_bm_write(fmt as u32, argb, cpp.as_mut_ptr()) };
            let mut r = init.clone();
            write_b8g8r8a8_word(&mut r, 0, fmt, argb);
            c[2].eq(r, cpp, || format!("case {case} {fmt:?} {argb:#010x}"));
        }
        // --- copy pixel
        {
            let same = g.below(3) == 0;
            let sf = if g.below(8) == 0 { Ww3dFormat::P8 } else { readable(&mut g) };
            let df = if same { sf } else { dest_format(&mut g) };
            let pb = [3u32, 4][g.below(2) as usize];
            let pal = palette(&mut g, pb);
            let src = surface(&mut g, 8);
            let init: Vec<u8> = (0..8).map(|_| g.u32() as u8).collect();
            let use_hsv = g.below(2) == 0;
            let h = hsv(&mut g);
            let mut cpp = init.clone();
            unsafe { ref_w6_bm_copy_pixel(df as u32, sf as u32, src.as_ptr().add(PRE), pal.as_ptr(), pb, use_hsv as i32, h.as_ptr(), cpp.as_mut_ptr()) };
            let mut r = init.clone();
            if use_hsv {
                copy_pixel_hsv(&mut r, 0, df, &src[PRE..], 0, sf, &pal, pb, h);
            } else {
                copy_pixel(&mut r, 0, df, &src[PRE..], 0, sf, &pal, pb);
            }
            let k = if use_hsv { 4 } else { 3 };
            c[k].eq(r, cpp, || format!("case {case} {sf:?} -> {df:?} hsv {h:?} src {:?}", &src[PRE..PRE + 8]));
        }
        // --- combine
        {
            let v: Vec<u32> = (0..4).map(|_| if g.below(8) == 0 { [0u32, 0xffff_ffff, 0x0303_0303, 0x0101_0101][g.below(4) as usize] } else { g.u32() }).collect();
            let cpp = unsafe { ref_w6_bm_combine(v[0], v[1], v[2], v[3]) };
            c[5].eq(combine_a8r8g8b8(v[0], v[1], v[2], v[3]), cpp, || format!("case {case} {v:x?}"));
        }
        // --- Create_Mipmap_B8G8R8A8
        {
            let (w, h) = (2 * (1 + g.below(5)), 2 * (1 + g.below(5)));
            let spitch = w * 4 + 4 * g.below(3);
            let dpitch = (w / 2) * 4 + 4 * g.below(3);
            let src = surface(&mut g, (spitch * h) as usize);
            let dst = surface(&mut g, (dpitch * h / 2) as usize);
            let mut cs = src.clone();
            let mut cd = dst.clone();
            unsafe { ref_w6_bm_mip(cd.as_mut_ptr().add(PRE), dpitch, cs.as_mut_ptr().add(PRE), spitch, w, h) };
            let mut rd = dst.clone();
            create_mipmap_b8g8r8a8(&mut rd[PRE..], dpitch, &src[PRE..], spitch, w, h);
            c[6].eq(rd, cd, || format!("case {case} {w}x{h} pitches {spitch} {dpitch}"));
        }
        // --- Copy_Image_Generate_Mipmap
        {
            let (w, h) = (2 * (1 + g.below(4)), 2 * (1 + g.below(4)));
            let sf = if g.below(2) == 0 { Ww3dFormat::A8R8G8B8 } else { readable(&mut g) };
            let df = if g.below(2) == 0 { Ww3dFormat::A8R8G8B8 } else { dest_format(&mut g) };
            let (sb, db) = (bytes_per_pixel(sf), bytes_per_pixel(df));
            let fast = sf == df && sf == Ww3dFormat::A8R8G8B8;
            let pad = |g: &mut Gen| if fast { 4 * g.below(3) } else { g.below(5) };
            let spitch = w * sb + pad(&mut g);
            let dpitch = w * db + pad(&mut g);
            let mpitch = (w / 2) * db + pad(&mut g);
            let src = surface(&mut g, (spitch * h) as usize);
            let dst = surface(&mut g, (dpitch * h) as usize);
            let mip = surface(&mut g, (mpitch * (h / 2)) as usize);
            let hs = hsv(&mut g);
            let (mut cs, mut cd, mut cm) = (src.clone(), dst.clone(), mip.clone());
            unsafe { ref_w6_bm_genmip(w, h, cd.as_mut_ptr().add(PRE), dpitch, df as u32, cs.as_mut_ptr().add(PRE), spitch, sf as u32, cm.as_mut_ptr().add(PRE), mpitch, hs.as_ptr()) };
            let (mut rd, mut rm) = (dst.clone(), mip.clone());
            copy_image_generate_mipmap(w, h, &mut rd[PRE..], dpitch, df, &src[PRE..], spitch, sf, &mut rm[PRE..], mpitch, hs);
            c[7].eq((rd, rm), (cd, cm), || format!("case {case} {w}x{h} {sf:?} -> {df:?} hsv {hs:?}"));
        }
        // --- Copy_Image
        {
            let kind = g.below(10);
            let (sw, sh) = (1 + g.below(9), 1 + g.below(9));
            let (dw, dh, sf, df, k) = if kind < 3 {
                // bump destination, not larger than the source
                let df = [Ww3dFormat::U8V8, Ww3dFormat::L6V5U5, Ww3dFormat::X8L8V8U8][g.below(3) as usize];
                (1 + g.below(sw), 1 + g.below(sh), readable(&mut g), df, 8)
            } else if kind < 6 {
                let f = if g.below(2) == 0 { Ww3dFormat::A8R8G8B8 } else { Ww3dFormat::X8R8G8B8 };
                let (dw, dh) = if g.below(2) == 0 { (sw, sh) } else { (1 + g.below(9), 1 + g.below(9)) };
                (dw, dh, f, f, 9)
            } else {
                let sf = if g.below(6) == 0 { Ww3dFormat::P8 } else { readable(&mut g) };
                let (dw, dh) = if g.below(2) == 0 { (sw, sh) } else { (1 + g.below(9), 1 + g.below(9)) };
                (dw, dh, sf, if g.below(4) == 0 { sf } else { dest_format(&mut g) }, 10)
            };
            let (sb, db) = (bytes_per_pixel(sf), bytes_per_pixel(df));
            let b32 = k == 9;
            let pad = |g: &mut Gen| if b32 { 4 * g.below(2) } else { g.below(4) };
            let sp = sw * sb + pad(&mut g);
            let dp = dw * db.max(1) + pad(&mut g);
            let pb = [3u32, 4][g.below(2) as usize];
            let pal = palette(&mut g, pb);
            let gen_mip = k != 8 && dw == sw && dh == sh && g.below(2) == 0;
            let hs = hsv(&mut g);
            let src = surface(&mut g, (sp * sh) as usize);
            let dst = surface(&mut g, (dp * dh) as usize);
            let (mut cs, mut cd) = (src.clone(), dst.clone());
            unsafe { ref_w6_bm_copy_image(cd.as_mut_ptr().add(PRE), dw, dh, dp, df as u32, cs.as_mut_ptr().add(PRE), sw, sh, sp, sf as u32, pal.as_ptr(), pb, gen_mip as i32, hs.as_ptr()) };
            let (mut rs, mut rd) = (src.clone(), dst.clone());
            copy_image(&mut rd[PRE..], dw, dh, dp, df, &mut rs[PRE..], sw, sh, sp, sf, &pal, pb, gen_mip, hs);
            c[k].eq((rd, rs), (cd, cs), || format!("case {case} {sw}x{sh} {sf:?} -> {dw}x{dh} {df:?} mip {gen_mip} hsv {hs:?} pal_bpp {pb}"));
        }
    }
    support_sweep::finish(c);
}
