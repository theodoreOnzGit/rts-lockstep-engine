#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (LETTER_BOX_FADE_TIME, renderLetterBox,
//     isLetterBoxFading, isLetterBoxed, toggleLetterBox, enableLetterBox)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp (Display::reset's letterbox lines)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::letterbox` (GitHub #144) against the C++.
//!
//! # Method
//!
//! The C++ side is W3DDisplay.cpp's `renderLetterBox`, `isLetterBoxFading`, `isLetterBoxed`, `toggleLetterBox` and
//! `enableLetterBox` and Display.cpp's `reset` lines, cut verbatim by build.rs (`differential/wrap_letterbox.cpp` lists
//! the cuts, the stand-ins and the words). Both sides run the same sequence and record every `drawFillRect` (its five
//! arguments) and every `setZoomLimited`, then, after each step, the enabled flag, the fade level's **bits**, the fade's
//! start time, `isLetterBoxFading()` and `isLetterBoxed()`.
//!
//! Each case: a display of 800x600, 1024x768, 390x844 (a phone), 844x390, 1920x1080, 2560x1080 (wider than 16:9: the
//! bars' sizes are negative) or random (1..4,096 each way); the clock starting at 0, at a random value, or within 3
//! seconds of its 2^32 wrap; the tactical view present or not. Then 1 to 150 steps, mostly rendered frames (the
//! clock moving 0 to 50 ms, then `renderLetterBox(timeGetTime())`), with `enableLetterBox(TRUE/FALSE)` (repeated ones
//! too), the debug key's `toggleLetterBox`, `Display::reset`, jumps of the clock up to 5 s, the view coming and going,
//! and the display's size changing.
//!
//! # Predictions (written before the first run, 2026-11-11)
//!
//! - 0 mismatches over 20,000 sequences: the only arithmetic is `UnsignedInt` differences (wrapping on both sides),
//!   one `u32`-to-`float` conversion, one division by 1,000, `1 - x`, `x * 255` and the bars' `(h - 9/16 w) * 0.5`, each a
//!   single binary32 operation on both sides (SSE, no contraction), and truncating conversions to `Int`.
//! - Every plant 9331-9340 caught. The fewest: 9338 (`reset` keeping the fade level: it only shows when a reset comes
//!   while the bars are up or fading, and the steps have one reset in 40) and 9332 (the fade-out's clamp at 0: it only
//!   shows on a frame past the fade's end before the next frame finds it at 0; that is every fade-out's last frame, so
//!   not rare).
//!
//! # Results (2026-11-11, `LETTERBOX_CASES=20000`, the default)
//!
//! 20,000 sequences, 2,685,694 steps: 1,175,486 bars drawn (158,348 opaque), 67,741 zoom limits, 1,475,442 steps
//! fading. **0 mismatches** on the first run, as predicted.
//!
//! Planted bugs (same sequences), mismatching sequences of 20,000: 9331 7,701; 9332 4,988; 9333 14,971; 9334 17,152;
//! 9335 12,307; 9336 13,438; 9337 15,516; 9338 12,270; 9339 10,846; 9340 17,648. **10 of 10 caught.** **Prediction
//! missed:** 9338 was not among the fewest (12,270): a reset while the bars are up or fading is common, since a sequence
//! is mostly fading (55 % of steps). The fewest were 9332 (as predicted) and 9331 (the fade-in's clamp at 1, which only
//! shows on the frames between the fade's end and the first frame that sees it at 1: one frame per fade-in).
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose):
//!
//! | id | planted bug |
//! |---|---|
//! | 9331 | the fade-in is not clamped at 1 |
//! | 9332 | the fade-out is not clamped at 0 |
//! | 9333 | the fade-out draws the bottom bar too (the line upstream comments out) |
//! | 9334 | the bars' height misses the `* 0.5` |
//! | 9335 | `enableLetterBox(TRUE)` restarts the fade when already enabled |
//! | 9336 | `enableLetterBox(FALSE)` does not restart the fade's clock |
//! | 9337 | `isLetterBoxFading` misses the fade-out |
//! | 9338 | `Display::reset` keeps the fade level |
//! | 9339 | `toggleLetterBox` gives the view the inverted zoom limit |
//! | 9340 | the alpha is rounded, not truncated |

mod support;

use rts_lockstep_engine::client::letterbox::LetterBox;
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let (mut width, mut height) = (input[0], input[1]);
    let mut now = input[2] as u32;
    let mut view = input[3] != 0;
    let mut lb = LetterBox::default();
    let mut out = Vec::new();
    let zoom = |out: &mut Vec<i32>, view: bool, limit: Option<bool>| {
        if let (true, Some(l)) = (view, limit) {
            out.extend([2, i32::from(l), 0, 0, 0, 0]);
        }
    };
    for w in input[4..].chunks(4) {
        match w[0] {
            0 => {
                let l = lb.enable_letter_box(w[1] != 0, now);
                zoom(&mut out, view, l);
            }
            1 => {
                let l = lb.toggle_letter_box(now);
                zoom(&mut out, view, Some(l));
            }
            2 => {
                for r in lb.render_letter_box(now, width, height) {
                    out.extend([1, r.x, r.y, r.width, r.height, r.color as i32]);
                }
            }
            3 => lb.reset(),
            4 => now = now.wrapping_add(w[1] as u32),
            5 => view = w[1] != 0,
            _ => (width, height) = (w[1], w[2]),
        }
        out.extend([100, i32::from(lb.enabled), lb.fade_level.to_bits() as i32, lb.fade_start_time as i32, i32::from(lb.is_letter_box_fading()), i32::from(lb.is_letter_boxed())]);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 200_000];
    let n = unsafe { cpp::ref_letterbox_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn size(g: &mut Gen) -> (i32, i32) {
    match g.below(8) {
        0 => (800, 600),
        1 => (1024, 768),
        2 => (390, 844),
        3 => (844, 390),
        4 => (1920, 1080),
        5 => (2560, 1080),
        _ => (1 + g.below(4096) as i32, 1 + g.below(4096) as i32),
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (w, h) = size(g);
    let start = match g.below(3) {
        0 => 0u32,
        1 => g.u32(),
        _ => u32::MAX - g.below(3000),
    };
    let mut v = vec![w, h, start as i32, i32::from(g.below(4) != 0)];
    let steps = 1 + g.below(150);
    for _ in 0..steps {
        match g.below(40) {
            0..=3 => v.extend([0, i32::from(g.below(2) == 0), 0, 0]),
            4 => v.extend([1, 0, 0, 0]),
            5 => v.extend([3, 0, 0, 0]),
            6 => v.extend([4, g.below(5001) as i32, 0, 0]),
            7 => v.extend([5, i32::from(g.below(2) == 0), 0, 0]),
            8 => {
                let (w, h) = size(g);
                v.extend([6, w, h, 0]);
            }
            _ => {
                v.extend([4, g.below(51) as i32, 0, 0]);
                v.extend([2, 0, 0, 0]);
            }
        }
    }
    v
}

#[test]
fn the_letterbox_fades_and_draws_as_upstream() {
    let cases: u64 = std::env::var("LETTERBOX_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x144_2013);
    let mut whole = Check::new("the script letterbox");
    let (mut steps, mut rects, mut zooms, mut fading_steps, mut full) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 4) / 4) as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        for rec in c.chunks(6) {
            match rec[0] {
                1 => {
                    rects += 1;
                    if rec[5] as u32 == 0xFF00_0000 {
                        full += 1;
                    }
                }
                2 => zooms += 1,
                100 if rec[4] == 1 => fading_steps += 1,
                _ => {}
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len())) / 6 * 6;
            let k0 = k.saturating_sub(12);
            println!("sequence {i}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 12).min(r.len())], &c[k0.min(c.len())..(k + 12).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: {rects} bars drawn ({full} opaque), {zooms} zoom limits, {fading_steps} steps fading");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
