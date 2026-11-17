#![cfg(feature = "differential")]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/ControlBar.cpp (getStarImage, onPlayerRankChanged,
//     onPlayerSciencePurchasePointsChanged, init's star lines, reset's end)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The general's star and the `ControlBarArrow` group (GitHub #119): `client::control_bar_gen_star` against the C++.
//!
//! # Method
//!
//! The C++ side is the upstream text cut verbatim by build.rs (`differential/wrap_gen_star.cpp` lists the cuts and the
//! words), compiled on stand-ins for the player list, the transition handler, the InGameUI, the window manager, the
//! logic's frame and the mapped image collection. Each sequence draws the local player's index (0-7), the starting
//! `m_genStarFlash`, `m_lastFlashedAtPointValue` (-1 to 5) and whether `m_genArrow` is set, then 1 to 12 steps: the
//! rank change and the points change (three in ten each), `getStarImage` (three in ten), `reset`'s end and `init`'s
//! lines (one in twenty each). Each step draws the player (the local one two times in three), the local player's
//! points (-2 to 6), the transition handler (present seven times in eight), the input (on three times in four), the
//! general's button (present seven times in eight), the frame (any) and the image collection (present half the time).
//! The Rust state is carried through the sequence; compared per sequence: every call made (`setGroup`, `remove`,
//! `markUIDirty`, `GadgetButtonSetEnabledImage` with the image), and after each step the flash, the last flashed value
//! and the arrow.
//!
//! *Predictions, before the first run* (`issue119_predictions.md`): 0 mismatches in 20,000 sequences; each plant
//! 10338-10344 caught.
//!
//! # Results (2026-11-17, `GEN_STAR_SEQUENCES=20000`, the default)
//!
//! First run: 20,000 sequences, 130,442 steps, **0 mismatches**. 18,044 steps set the `ControlBarArrow` group, 12,995
//! were local changes with the input off (the gate's cases), 5,578 showed the highlight.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: **7 of 7 caught** (2026-11-17).
//!
//! | id | planted bug | caught by (sequences) |
//! |---|---|---|
//! | 10338 | `reset` leaves the flash off | 5,545 |
//! | 10339 | `getStarImage` keeps flashing with no points | 2,011 |
//! | 10340 | the highlight's half second starts a frame early | 418 |
//! | 10341 | another player's change counts as the local one's | 13,781 |
//! | 10342 | the comparison `>=` for `>` | 3,446 |
//! | 10343 | the arrow shows with the input off (the #119 gate) | 5,049 |
//! | 10344 | the change does not turn the flash on | 11,575 |

mod support;

use rts_lockstep_engine::client::control_bar_gen_star::{GenStar, GenStarHost, GeneralButtonImage, CONTROL_BAR_ARROW};
use rts_lockstep_engine::differential as cpp;
use support::{Check, Gen};

fn cpp_seq(input: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 4096];
    let n = unsafe { cpp::ref_gen_star_seq(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

/// The host of one step, logging in the C++ stand-ins' words.
struct Host {
    local: i32,
    points: i32,
    handler: bool,
    input: bool,
    button: bool,
    frame: u32,
    out: Vec<i32>,
}

fn group(name: &[u8]) -> i32 {
    if name == CONTROL_BAR_ARROW { 1 } else { 99 }
}

impl GenStarHost for Host {
    fn is_local_player(&self, player_index: i32) -> bool {
        player_index == self.local
    }
    fn local_science_purchase_points(&self) -> i32 {
        self.points
    }
    fn transition_handler(&self) -> bool {
        self.handler
    }
    fn ui_input_enabled(&self) -> bool {
        self.input
    }
    fn transition_set_group(&mut self, name: &[u8]) {
        self.out.extend([1, group(name), 0]);
    }
    fn transition_remove(&mut self, name: &[u8]) {
        self.out.extend([2, group(name), 0]);
    }
    fn mark_ui_dirty(&mut self) {
        self.out.extend([3, 0, 0]);
    }
    fn general_button_exists(&self) -> bool {
        self.button
    }
    fn general_button_set_enabled_image(&mut self, image: GeneralButtonImage) {
        self.out.extend([4, if image == GeneralButtonImage::Enable { 1 } else { 2 }, 0]);
    }
    fn logic_frame(&self) -> u32 {
        self.frame
    }
}

#[test]
fn gen_star_matches_upstream() {
    let sequences: u64 = std::env::var("GEN_STAR_SEQUENCES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut g = Gen::new(0x119_57A2);
    let mut c = Check::new("ControlBar general's star");
    let mut stats = [0u64; 4];
    let mut steps = 0u64;
    for i in 0..sequences {
        let local = g.below(8) as i32;
        let mut s = GenStar { gen_star_flash: g.below(2) == 1, last_flashed_at_point_value: g.below(7) as i32 - 1, gen_arrow_set: g.below(2) == 1 };
        let mut input = vec![local, i32::from(s.gen_star_flash), s.last_flashed_at_point_value, i32::from(s.gen_arrow_set)];
        let mut h = Host { local, points: 0, handler: true, input: true, button: true, frame: 0, out: Vec::new() };
        for _ in 0..1 + g.below(12) {
            let kind = match g.below(20) {
                0..=5 => 0,
                6..=11 => 1,
                12..=17 => 2,
                18 => 3,
                _ => 4,
            };
            let player = if g.below(3) != 0 { local } else { (local + 1 + g.below(7) as i32) % 8 };
            h.points = g.below(9) as i32 - 2;
            h.handler = g.below(8) != 0;
            h.input = g.below(4) != 0;
            h.button = g.below(8) != 0;
            h.frame = g.u32();
            let collection = g.below(2) == 1;
            input.extend([kind, player, h.points, i32::from(h.handler), i32::from(h.input), i32::from(h.button), h.frame as i32, i32::from(collection)]);
            let before = h.out.len();
            match kind {
                0 => s.on_player_rank_changed(player, &mut h),
                1 => s.on_player_science_purchase_points_changed(player, &mut h),
                2 => s.get_star_image(&mut h),
                3 => s.reset(&mut h),
                _ => {
                    s.init();
                    // (the image lookups are the host's: found with the collection, NULL without)
                    h.out.extend([6, i32::from(collection), i32::from(collection)]);
                }
            }
            stats[0] += u64::from(h.out[before..].chunks(3).any(|r| r[0] == 1));
            stats[1] += u64::from(kind < 2 && player == local && !h.input);
            stats[2] += u64::from(h.out[before..].chunks(3).any(|r| r[0] == 4 && r[1] == 2));
            h.out.extend([100, i32::from(s.gen_star_flash), s.last_flashed_at_point_value, 101, i32::from(s.gen_arrow_set), 0]);
            steps += 1;
        }
        let cpp = cpp_seq(&input);
        c.eq(std::mem::take(&mut h.out), cpp, || format!("sequence {i} {input:?}"));
    }
    println!("gen star: {sequences} sequences, {steps} steps; {} arrows set, {} local changes with the input off, {} highlights", stats[0], stats[1], stats[2]);
    if planted {
        c.report();
    } else {
        c.finish();
    }
    assert!(!planted, "a planted run: see the MEASURED lines (a mismatch is the plant caught)");
}
