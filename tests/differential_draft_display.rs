#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Display.cpp and View.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::display` against `Display` and `View::prependViewToList` (#215, group lc3).
//!
//! # Method
//!
//! `deleteViews`, `attachView`, `drawViews`, `updateViews`, `stopMovie`, `playLogoMovie`, the two debug-callback functions and
//! `View::prependViewToList` are cut verbatim by build.rs into `differential/wrap_draft_display.cpp` (stand-ins listed there: views that log their
//! draw / update / delete, a video player that opens or not, a buffer whose allocation always succeeds). A case is a program of up to 30 operations
//! on a fresh display (attach, deleteViews, drawViews, updateViews, playLogoMovie with the stream opening or not, stopMovie, set and get the callback).
//! After every operation the call log, the view list head first, and every movie / hold-time / callback field are compared. 20,000 programs from a
//! fixed seed. The draft has no `stopMovie`; the test's closure reproduces what upstream's cut does to the fields the draft holds.
//! EXCLUDED: a failed `VideoBuffer::allocate` (upstream then calls stopMovie and returns; the draft leaves the buffer out as device-only).
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 programs** (prediction held). Planted bugs, 10 of 10 caught (of 20,000 programs): 19871 15,013; 19872 8,741; 19873 11,507;
//! 19874 6,817; 19875 10,036; 19876 11,254; 19877 14,922; 19878 8,739; 19879 6,311; 19880 11,982.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19871 attachView appends at the tail; 19872 deleteViews goes tail to head; 19873 deleteViews leaves the list; 19874 drawViews
//! tail to head; 19875 updateViews skips the head; 19876 playLogoMovie goes on after a failed open; 19877 the two hold times swapped; 19878 playLogoMovie
//! does not stop the old movie first; 19879 getDebugDisplayCallback returns the user data; 19880 setDebugDisplayCallback stores the callback as the user data.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::display::DisplayViews;
use std::cell::RefCell;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_display_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

/// What upstream's `Display::stopMovie` does to the fields `DisplayViews` holds.
fn stop_movie(d: &mut DisplayViews) {
    d.has_video_stream = false;
    if !d.currently_playing_movie.is_empty() {
        d.currently_playing_movie = Vec::new();
    }
    d.copyright_hold_time = -1;
    d.movie_hold_time = -1;
}

fn dump(d: &DisplayViews, log: &[i32], ans: i32, out: &mut Vec<i32>) {
    out.push(log.len() as i32);
    out.extend(log);
    out.push(d.view_list.len() as i32);
    out.extend(d.view_list.iter().map(|&v| v as i32));
    out.push(d.has_video_stream as i32);
    out.push(if d.currently_playing_movie.is_empty() { -1 } else { std::str::from_utf8(&d.currently_playing_movie[1..]).unwrap().parse().unwrap() });
    out.push(d.movie_hold_time);
    out.push(d.copyright_hold_time);
    out.push(d.elapsed_movie_time as i32);
    out.push(d.debug_display_callback.map_or(0, |v| v as i32));
    out.push(d.debug_display_user_data.map_or(0, |v| v as i32));
    out.push(ans);
}

#[test]
fn display_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_D15);
    let mut c = Check::new("Display views, movie and callbacks");
    for case in 0..20_000 {
        let n = 1 + g.below(30);
        let mut ops: Vec<[i32; 6]> = Vec::new();
        let mut next_view = 1;
        for _ in 0..n {
            let kind = [0, 0, 0, 1, 2, 3, 4, 4, 5, 6, 7][g.below(11) as usize];
            let mut o = [kind, 0, 0, 0, 0, 0];
            match kind {
                0 => {
                    o[1] = next_view;
                    next_view += 1;
                }
                4 => {
                    o[1] = g.below(6) as i32;
                    o[2] = [0, 1, 500, 30000, -1][g.below(5) as usize];
                    o[3] = [0, 2, 700, 40000, -1][g.below(5) as usize];
                    o[4] = (g.below(3) != 0) as i32;
                    o[5] = [0, 1, 123456, i32::MAX, -7][g.below(5) as usize];
                }
                6 => {
                    o[1] = g.below(4) as i32;
                    o[2] = g.below(4) as i32;
                }
                _ => {}
            }
            ops.push(o);
        }
        let prog: Vec<i32> = ops.iter().flatten().copied().collect();
        let mut cpp = vec![0i32; 20000];
        let len = unsafe { ref_lc3_display_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut d = DisplayViews::default();
        let mut rust = Vec::new();
        for o in &ops {
            let log = RefCell::new(Vec::<i32>::new());
            let mut ans = 0;
            match o[0] {
                0 => d.attach_view(o[1] as u32),
                1 => d.delete_views(&mut |v| log.borrow_mut().push(2000 + v as i32)),
                2 => d.draw_views(&mut |v| log.borrow_mut().push(v as i32)),
                3 => d.update_views(&mut |v| log.borrow_mut().push(1000 + v as i32)),
                4 => {
                    let ok = o[4] != 0;
                    let name = format!("m{}", o[1]).into_bytes();
                    d.play_logo_movie(&mut stop_movie, &mut |_| ok, o[5] as u32, &name, o[2], o[3]);
                }
                5 => stop_movie(&mut d),
                6 => d.set_debug_display_callback(if o[1] == 0 { None } else { Some(o[1] as u32) }, if o[2] == 0 { None } else { Some(o[2] as u32) }),
                _ => ans = d.get_debug_display_callback().map_or(0, |v| v as i32),
            }
            dump(&d, &log.borrow(), ans, &mut rust);
        }
        c.eq(rust, cpp, || format!("case {case}: ops {ops:?}"));
    }
    finish(c);
}
