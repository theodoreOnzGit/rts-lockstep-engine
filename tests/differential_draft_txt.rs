// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3d6) of the draft `src/drafts/ww3d/txt.rs` (`TextTextureClass::is_texture_valid`, `build_texture`) against upstream
//! `WW3D2/txt.cpp`'s `Is_Texture_Valid` and `Build_Texture`, cut VERBATIM (build.rs `drafts_ww3d6`, differential/wrap_draft_w6_txt.cpp). Stand-ins: WWLib's legacy
//! `FontClass`, `ConvertClass`, `BSurface` (the font reports the case's width and height; `Blit_From` records its two `Rect`s) and `TextureClass`; `Rect`/`TPoint2D` are the
//! real `trect.h` / `Point.h`, `Find_POT` is the real `pot.cpp`.
//!
//! # Method
//!
//! 20,000 objects (fixed seed, `DRAFT_CASES` overrides), each given 1 to 8 operations over 3 fonts, 3 converts and 3 strings: make a texture exist, `Is_Texture_Valid`
//! (including the object's own `TextureString`), `Build_Texture` with the string's pixel width 0 to 600 (`fw` 1 to 600) and height `fh` 1 to 40. Compared: the results,
//! `TextureSize`, the number of strip blits and the four fields of every destination and source `Rect`.
//!
//! # Deliberate deviations of the draft, excluded
//!
//! - Upstream compares `TextureString == str` as POINTERS (and `TextureString` is a copy made by `nstrdup`), so the texture is valid only when the caller passes the object's own
//!   `TextureString`; the draft compares the strings. The test passes either that very string or a different one (never an equal copy at another address).
//! - Upstream's `Build_Texture` ends with `REF_PTR_RELEASE(Texture)` and a `TODO: copy bsurf2 into texture`: `Texture` is NULL afterwards and the texture is never valid. The
//!   draft sets `has_texture` (the owner makes the texture); the test makes the C++ object's texture exist again after each rebuild, as the owner does.
//!
//! # Upstream defects found (file on #212)
//!
//! 1. `Build_Texture` does `if (TextureString) delete[] TextureString;` and then `TextureString = nstrdup(str)`: with `str` the object's own `TextureString` and a rebuild needed (any
//!    parameter changed, or `Texture` NULL, which it always is) it copies from freed memory. The test passes the own string only where the texture is valid (early return).
//! 2. `Rect destrect(0, fh*lp, blitw, fh*lp+fh)` and `Rect srcrect(lp_tsize, 0, lp_tsize + blitw, fh)` pass (x0, y0, x1, y1) to `TRect(x, y, w, h)` (trect.h): for strip
//!    `lp > 0` the rectangles' Width/Height are `lp_tsize + blitw` / `fh*lp + fh`, not `blitw` / `fh`. The code's own comment (`d->blit(0, fh*lp, *s, lp_tsize, 0, lp_tsize+blitw, fh)`)
//!    shows the (x0, y0, x1, y1) intent. The draft's `StripBlit` records the arguments exactly as upstream passes them. The path is dead in the shipped game (`TextTextureClass` is not used).
//!
//! # Predictions (written before the first run, 2026-11-20)
//!
//! The draft takes `sqrt` in `f32` of `fw*fh` converted to float where upstream calls `sqrt(int)` (double) and converts the result to float; I expect the same bits for
//! the sizes tested (the product is below 2^24, the f32 square root is correctly rounded, and double-then-float rounding of a square root is innocuous). So 0 mismatches.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;
mod support_sweep;

use rts_lockstep_engine::drafts::ww3d::txt::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_w6_txt_run(input: *const u32, out: *mut u32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

const POOL: [&str; 3] = ["alpha", "beta", "gamma"];

#[test]
fn txt_matches_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_61_07);
    let mut valid = Check::new("TextTextureClass::Is_Texture_Valid");
    let mut build = Check::new("TextTextureClass::Build_Texture");
    let mut out = vec![0u32; 20_000];
    for case in 0..support_sweep::cases() {
        let mut t = TextTextureClass::new();
        let nops = 1 + g.below(8);
        let mut input = vec![0u32];
        let mut nwords = 0u32;
        // (is a build, expected words) in output order
        let mut expect: Vec<(bool, Vec<u32>)> = Vec::new();
        let mut last: Option<(usize, usize, i32, i32, usize)> = None;
        for _ in 0..nops {
            let kind = match g.below(8) {
                0 => 0,
                1..=3 => 1,
                _ => 2,
            };
            if kind == 0 {
                input.extend([0, 0, 0, 0, 0, 0, 0, 0]);
                t.has_texture = true;
                nwords += 1;
                continue;
            }
            // repeat the last build's parameters often, so that the valid path is reached
            let (font, str_, fore, back, conv) = match last {
                Some(l) if g.below(2) == 0 => (l.0, 3usize, l.2, l.3, l.4),
                _ => (g.below(3) as usize, g.below(4) as usize, g.below(4) as i32 * 7, g.below(4) as i32 * 3, g.below(3) as usize),
            };
            // `Build_Texture` frees `TextureString` and then copies `str`: with `str` that very pointer and a rebuild needed it reads freed memory (upstream defect, excluded)
            let own_valid = str_ == 3 && t.is_texture_valid(font, &t.texture_string.clone().unwrap_or_else(|| POOL[0].to_string()), fore, back, conv);
            let str_ = if kind == 2 && str_ == 3 && !own_valid { g.below(3) as usize } else { str_ };
            // a pool string equal in content to the stored copy is a different address upstream (invalid) and the same string for the draft: never passed (the deviation above)
            let str_ = if str_ != 3 && t.texture_string.as_deref() == Some(POOL[str_]) { (str_ + 1) % 3 } else { str_ };
            let own = str_ == 3;
            let s: String = if own { t.texture_string.clone().unwrap_or_else(|| POOL[0].to_string()) } else { POOL[str_].to_string() };
            let (fw, fh) = (1 + g.below(600) as i32, 1 + g.below(40) as i32);
            input.extend([kind, font as u32, str_ as u32, fore as u32, back as u32, conv as u32, fw as u32, fh as u32]);
            nwords += 1;
            if kind == 1 {
                expect.push((false, vec![t.is_texture_valid(font, &s, fore, back, conv) as u32]));
            } else {
                let r = t.build_texture(font, &s, fore, back, conv, fw, fh);
                let mut w = vec![r.is_some() as u32, t.get_texture_size() as u32];
                match &r {
                    Some(blits) => {
                        w.push(blits.len() as u32);
                        for b in blits {
                            w.extend([b.dest.0, b.dest.1, b.dest.2, b.dest.3, b.src.0, b.src.1, b.src.2, b.src.3].map(|x| x as u32));
                        }
                        // the owner makes the texture from the surface (upstream leaves Texture NULL)
                        input.extend([0, 0, 0, 0, 0, 0, 0, 0]);
                        nwords += 1;
                    }
                    None => w.push(0),
                }
                expect.push((true, w));
                last = Some((font, str_, fore, back, conv));
                // the next "own string" use names the string this build stored
                last = last.map(|l| (l.0, l.1, l.2, l.3, l.4));
            }
        }
        input[0] = nwords;
        let n = unsafe { ref_w6_txt_run(input.as_ptr(), out.as_mut_ptr()) } as usize;
        // the C++ output for a build holds [result, size, count, blits...] and for a validity test [result]: split in the same order
        let mut pos = 0;
        for (is_build, w) in &expect {
            let take = if *is_build { 3 + 8 * out[pos + 2] as usize } else { 1 };
            let cpp = out[pos..pos + take].to_vec();
            pos += take;
            let check = if *is_build { &mut build } else { &mut valid };
            // a build that did not rebuild: upstream returns early (TextureSize untouched, no blits), the draft's word is the same size
            check.eq(w.clone(), cpp, || format!("case {case} {nops} ops"));
        }
        assert_eq!(pos, n, "case {case}: output length");
    }
    support_sweep::finish(vec![valid, build]);
}
