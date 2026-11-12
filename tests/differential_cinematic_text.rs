#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Display.h (setCinematicText, setCinematicFont, setCinematicTextFrames)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp (Display::Display's cinematic members)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DDisplay.cpp (draw's cinematic text, lines 1926-1953)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `client::cinematic_text` (GitHub #146) against the C++.
//!
//! # Method
//!
//! The C++ side is Display's three setters and constructor lines and `W3DDisplay::draw`'s cinematic text block, cut
//! verbatim by build.rs, with a stand-in display string that records what the block sets and draws
//! (`differential/wrap_cinematic_text.cpp` lists the cuts and the words). Both sides run the same random sequence: texts
//! from a list (the empty one, short and long ones, and one with bytes 0x80 and above, which `translate` sign-extends),
//! fonts (none, or a name, a size and bold), frame counts (0, small, negative, large), display sizes (a phone's 390x844
//! and 844x390, 800x600, 1024x768, 1920x1080, small ones down to 1x1, or random), and draws with the string's width from
//! 0 to past the display. After each step the frame count and the text's length are compared too.
//!
//! # Predictions (written before the first run, 2026-11-12)
//!
//! - 0 mismatches over 20,000 sequences: integer and `double` arithmetic only.
//! - Plants 9385-9390 all caught. The fewest: 9387 (`y` in `float`), which needs a height where `h * 0.9f` and
//!   `h * .9` truncate differently: 844 gives 759 both ways, but 10 gives 8 and 9; among the random heights, a fair
//!   share. 9389 (the wrap at the full width) on every draw.
//!
//! # Results (2026-11-12, `CINEMATIC_TEXT_CASES=20000`, the default)
//!
//! 20,000 sequences, 610,871 steps (304,895 draws asked, 135,346 drawn, 34,057 at x 20): **0 mismatches** on the first
//! run, as predicted. Plants, mismatching sequences of 20,000: 9385 14,269; 9386 13,705; 9388 11,564; 9389 14,269; 9390
//! 7,796. **Prediction missed: the first 9387 (`y` in `float`) was not caught, 0 of 20,000**, and cannot be: `10 * 0.9f`
//! is 8.9999998 before rounding, which rounds to 9.0 in `float`. `0.9f`'s relative error (2.6e-8) is under half a
//! `float` ULP (6e-8), so `h * 0.9f` rounds to the same integer as `h * .9` whenever that is an integer, and no other
//! product lies near one: an equivalent mutant. 9387 was replaced by `y` rounded instead of truncated (a real
//! difference from upstream for heights whose ninth tenth is .5 or more). Second run (the same sequences): 0 mismatches
//! unplanted; the new 9387 7,805. **6 of 6 caught.**
//!
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 9385 | the frame count does not go down |
//! | 9386 | drawn with no frames left |
//! | 9387 | `y` rounded, not truncated (~~`y` computed in `float`~~: an equivalent mutant, see the results) |
//! | 9388 | a string wider than the display is centred (no 20) |
//! | 9389 | the wrap is the display's full width |
//! | 9390 | drawn with no text |

mod support;

use rts_lockstep_engine::client::cinematic_text::{CinematicFont, CinematicText};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

/// The wrapper's strings, in its order.
const STRINGS: [&[u8]; 8] = [b"", b"Hello", b"The base is under attack", b"\xE9t\xE9 \x80 \xFF", b"A much longer line of cinematic text that wraps across the screen", b"Arial", b"rial - Size:20al - Size:20 - Size:20", b"Times New Roman"];

fn rust_seq(input: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    let (mut w, mut h) = (input[0] as u32, input[1] as u32);
    let mut c = CinematicText::default();
    for s in input[2..].chunks(4) {
        match s[0] {
            0 => c.set_cinematic_text(STRINGS[s[1] as usize % 8]),
            1 => c.set_cinematic_font((s[1] >= 0).then(|| CinematicFont { name: STRINGS[s[1] as usize % 8].to_vec(), point_size: s[2], bold: s[3] != 0 })),
            2 => c.set_cinematic_text_frames(s[1]),
            3 => (w, h) = (s[1] as u32, s[2] as u32),
            _ => {
                if let Some(d) = c.draw(w, h, |_| s[1]) {
                    out.extend([4, d.word_wrap, i32::from(d.word_wrap_centered), d.text.len() as i32]);
                    out.extend(d.text.iter().map(|&u| i32::from(u)));
                    match &d.font {
                        Some(f) => out.extend([STRINGS.iter().position(|n| *n == f.name.as_slice()).map_or(-2, |i| i as i32), f.point_size, i32::from(f.bold)]),
                        None => out.extend([-1, 0, 0]),
                    }
                    out.extend([d.x, d.y, d.color as i32, d.back_color as i32]);
                }
            }
        }
        out.extend([100, c.frames, c.text.len() as i32]);
    }
    out
}

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 100_000];
    let n = unsafe { cpp::ref_cinematic_text_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn size(g: &mut Gen) -> (i32, i32) {
    match g.below(8) {
        0 => (390, 844),
        1 => (844, 390),
        2 => (800, 600),
        3 => (1024, 768),
        4 => (1920, 1080),
        5 => (1 + g.below(30) as i32, 1 + g.below(30) as i32),
        _ => (1 + g.below(4000) as i32, 1 + g.below(4000) as i32),
    }
}

fn gen_case(g: &mut Gen) -> Vec<i32> {
    let (mut w, h) = size(g);
    let mut v = vec![w, h];
    for _ in 0..1 + g.below(60) {
        match g.below(10) {
            0 | 1 => v.extend([0, g.below(8) as i32, 0, 0]),
            2 => v.extend([1, if g.below(4) == 0 { -1 } else { g.below(8) as i32 }, 6 + g.below(40) as i32, i32::from(g.below(2) == 0)]),
            3 => {
                let f = match g.below(5) {
                    0 => 0,
                    1 => -(g.below(5) as i32),
                    2 => g.i32(),
                    _ => 1 + g.below(150) as i32,
                };
                v.extend([2, f, 0, 0]);
            }
            4 => {
                let (a, b) = size(g);
                w = a;
                v.extend([3, a, b, 0]);
            }
            _ => {
                let sw = match g.below(4) {
                    0 => 0,
                    1 => w + g.below(200) as i32,
                    _ => g.below(w as u32 + 1) as i32,
                };
                v.extend([4, sw, 0, 0]);
            }
        }
    }
    v
}

#[test]
fn the_cinematic_text_draws_as_upstream() {
    let cases: u64 = std::env::var("CINEMATIC_TEXT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x146_1927);
    let mut whole = Check::new("the cinematic text");
    let (mut steps, mut draws_asked, mut drawn, mut wide) = (0u64, 0u64, 0u64, 0u64);
    for i in 0..cases {
        let input = gen_case(&mut g);
        steps += ((input.len() - 2) / 4) as u64;
        draws_asked += input[2..].chunks(4).filter(|s| s[0] == 4).count() as u64;
        let r = rust_seq(&input);
        let c = cpp_seq(&input);
        let mut k = 0;
        while k < c.len() {
            if c[k] == 4 {
                drawn += 1;
                let n = c[k + 3] as usize;
                if c[k + 4 + n + 3] == 20 {
                    wide += 1;
                }
                k += 4 + n + 3 + 4;
            } else {
                k += 3;
            }
        }
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(6);
            println!("sequence {i} first difference at word {k}: rust {:?} c++ {:?}", &r[k0.min(r.len())..(k + 8).min(r.len())], &c[k0.min(c.len())..(k + 8).min(c.len())]);
        }
        whole.eq(r, c, || format!("sequence {i}"));
    }
    println!("{cases} sequences, {steps} steps: {draws_asked} draws asked, {drawn} drawn ({wide} at x 20)");
    if planted {
        whole.report();
    } else {
        whole.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED line (a mismatch is the plant caught)");
}
