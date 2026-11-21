// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3b) of the draft `src/drafts/ui/window_video_manager.rs` (`WindowVideo`, `WindowVideoManager`) against upstream
//! `WindowVideoManager.cpp` (every member) and `WindowVideoManager.h`'s enums, `class WindowVideo` and accessors, cut verbatim by build.rs (`drafts_ui3b`;
//! `differential/wrap_draft_video_manager.cpp`, whose stand-ins for the video player, the stream, the buffer and the window's instance data are listed there).
//!
//! # Method
//!
//! 20,000 sequences (fixed seed, `DRAFT_CASES` overrides). 1 to 4 windows (random hidden bit), 1 to 4 movies (each opens or not, gets a video buffer or not,
//! allocates or not, 1 to 4 frames long, with a random frame-ready mask), 1 to 20 steps of `playMovie`, `pauseMovie`, `hideMovie`, `resumeMovie`, `stopMovie`,
//! `stopAndRemoveMovie`, `stopAllMovies`, `pauseAllMovies`, `resumeAllMovies`, `update` (the most frequent), `reset`, `init`, a window shown or hidden, `getWinState`.
//! Every call on the stand-ins (open, createVideoBuffer, allocate, frame ready / decompress / render / next, close, delete of the buffer, `setVideoBuffer` on the
//! window) is logged on both sides, in order; after every step the window states, the number of playing videos, the two all-movies flags, and each window's hidden bit are
//! compared. The draft runs on the verified `GameWindowManager` with the same windows. Upstream's `std::hash_map` of windows iterates in an unspecified order, the draft in
//! window-id order: the C++ stand-in map is ordered by pointer, descending (the same order, as the scene creates its windows in reverse of the table), so what is compared is the draft's translation and not that order.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! 0 mismatches. The likeliest miss: `update`'s two state checks in a row (hidden then resumed in the same pass, and a window shown while `Play`).
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 21151 | `playMovie` does not remove a movie already playing in the window |
//! | 21152 | `playMovie` clears the two all-movies flags before it opens the stream (so also on failure) |
//! | 21153 | a movie that plays once pauses at its end (stop and pause swapped) |
//! | 21154 | a show-last-frame movie stops at its end |
//! | 21155 | the end of the movie is frame index 1, not 0 |
//! | 21156 | stopping a movie does not clear the window's buffer |
//! | 21157 | a hidden window's movie is resumed (the hidden test is inverted) |
//! | 21158 | `stopAllMovies` sets the pause flag, not the stop flag |
//! | 21159 | `resumeMovie` leaves the all-movies flags |
//! | 21160 | releasing a movie does not clear the window's buffer |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=20000` (the default): **0 mismatches over 20,000 sequences**, max ULP 0 (nothing is floating point here). The first run showed 2,410 mismatches, all in the order of the calls on different windows: the wrapper's map ordered the windows by ascending pointer, the draft by ascending window id, which is the reverse here (the scene creates its windows in reverse of the table). Upstream's `std::hash_map` order is unspecified, so the stand-in now orders by descending pointer (a harness fix), then **0 mismatches**. One draft doc claim was wrong and corrected in place (the draft said upstream's map is a `std::map` in pointer order; it is a `std::hash_map`).
//! Planted bugs (of 20,000): 21151 6,334; 21152 1,993; 21153 751; 21154 765; 21155 3,764; 21156 4,251; 21157 2,086; 21158 6,304; 21159 1,170; 21160 8,434. **10 of 10 caught.**
//! The `DIFF` line is printed by the run.

mod support;
mod ui3b_common;

use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{win_status, WinId};
use rts_lockstep_engine::drafts::ui::window_video_manager::*;
use std::sync::Mutex;
use support::{Check, Gen};
use ui3b_common::*;

extern "C" {
    fn ref_ui3b_wvm_run(input: *const u32, n: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Script {
    open_ok: bool,
    buf_ok: bool,
    alloc_ok: bool,
    length: u32,
    mask: u32,
}

struct Stream {
    length: u32,
    frame: u32,
    mask: u32,
    calls: u32,
}

struct Host {
    log: Vec<u32>,
    movies: Vec<Script>,
    next_stream: u32,
    next_buffer: u32,
    streams: Vec<Stream>,
}

impl GuiHost for Host {}

impl VideoHost for Host {
    fn open_stream_and_buffer(&mut self, movie_name: &[u8]) -> Option<(usize, usize)> {
        let idx: usize = std::str::from_utf8(&movie_name[1..]).unwrap().parse().unwrap();
        self.log.extend([10, idx as u32]);
        let (open_ok, buf_ok, alloc_ok, length, mask) = {
            let m = &self.movies[idx];
            (m.open_ok, m.buf_ok, m.alloc_ok, m.length, m.mask)
        };
        if !open_ok {
            return None;
        }
        let stream = self.next_stream;
        self.next_stream += 1;
        self.streams.resize_with(stream as usize + 1, || Stream { length: 1, frame: 0, mask: 0, calls: 0 });
        self.streams[stream as usize] = Stream { length, frame: 0, mask, calls: 0 };
        self.log.push(11);
        if !buf_ok {
            self.log.extend([21, stream]);
            return None;
        }
        let buffer = self.next_buffer;
        self.next_buffer += 1;
        self.log.extend([12, buffer]);
        if !alloc_ok {
            self.log.extend([20, buffer, 21, stream]);
            return None;
        }
        Some((stream as usize, buffer as usize))
    }
    fn set_window_video_buffer(&mut self, wm: &mut GameWindowManager, win: WinId, buffer: Option<usize>) {
        self.log.extend([30, wm.tree.windows[win].id as u32, buffer.map_or(0, |b| b as u32)]);
    }
    fn release_stream_and_buffer(&mut self, stream: usize, buffer: usize) {
        self.log.extend([20, buffer as u32, 21, stream as u32]);
    }
    fn is_frame_ready(&mut self, stream: usize) -> bool {
        self.log.extend([1, stream as u32]);
        let s = &mut self.streams[stream];
        let r = (s.mask >> (s.calls % 32)) & 1 == 1;
        s.calls += 1;
        r
    }
    fn frame_decompress_render_next(&mut self, stream: usize, buffer: usize) {
        self.log.extend([2, stream as u32, 3, stream as u32, buffer as u32, 4, stream as u32]);
        let s = &mut self.streams[stream];
        s.frame = (s.frame + 1) % s.length;
    }
    fn frame_index(&self, stream: usize) -> i32 {
        self.streams[stream].frame as i32
    }
}

fn state_word(s: WindowVideoStates) -> u32 {
    match s {
        WindowVideoStates::Stop => 1,
        WindowVideoStates::Pause => 2,
        WindowVideoStates::Play => 3,
        WindowVideoStates::Hidden => 4,
    }
}

#[test]
fn window_video_manager_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_B3_08);
    let mut chk = Check::new("WindowVideoManager");
    for _ in 0..cases(20_000) {
        let nwin = 1 + g.below(4) as usize;
        let specs: Vec<WinSpec> = (0..nwin).map(|i| WinSpec { id: 10 + i as i32, hidden: g.below(3) == 0, enabled: true, no_focus: false, extra: 0 }).collect();
        let (mut wm, order) = make_scene(&specs);
        let mut input = Vec::new();
        put_scene(&mut wm, &order, &specs, 0, &mut input);
        let nm = 1 + g.below(4) as usize;
        input.push(nm as u32);
        let mut movies = Vec::new();
        for _ in 0..nm {
            let m = Script {
                open_ok: g.below(8) != 0,
                buf_ok: g.below(8) != 0,
                alloc_ok: g.below(8) != 0,
                length: 1 + g.below(4),
                mask: match g.below(4) {
                    0 => 0xFFFF_FFFF,
                    1 => 0x5555_5555,
                    _ => g.u32(),
                },
            };
            input.extend([u32::from(m.open_ok), u32::from(m.buf_ok), u32::from(m.alloc_ok), m.length, m.mask]);
            movies.push(m);
        }
        let mut host = Host { log: Vec::new(), movies, next_stream: 1, next_buffer: 1, streams: Vec::new() };
        let mut mgr = WindowVideoManager::new();
        let steps = 1 + g.below(20);
        input.push(steps);
        let mut rust = Vec::new();
        for _ in 0..steps {
            let kind = match g.below(26) {
                0..=5 => 0,
                6..=12 => 9,
                k => k - 12,
            };
            let a = g.below(nwin as u32 + 3);
            let (b, c) = match kind {
                0 => (g.below(nm as u32), g.below(3)),
                12 => (g.below(2), 0),
                _ => (0, 0),
            };
            input.extend([kind, a, b, c]);
            host.log.clear();
            let w = order[a as usize % nwin];
            let mut st13 = None;
            match kind {
                0 => {
                    let name = format!("m{b}");
                    let pt = [WindowVideoPlayType::Once, WindowVideoPlayType::Loop, WindowVideoPlayType::ShowLastFrame][c as usize];
                    mgr.play_movie(&mut wm, &mut host, w, name.as_bytes(), pt)
                }
                1 => mgr.pause_movie(&mut wm, &mut host, w),
                2 => mgr.hide_movie(&mut wm, &mut host, w),
                3 => mgr.resume_movie(&mut wm, &mut host, w),
                4 => mgr.stop_movie(&mut wm, &mut host, w),
                5 => mgr.stop_and_remove_movie(&mut wm, &mut host, w),
                6 => mgr.stop_all_movies(&mut wm, &mut host),
                7 => mgr.pause_all_movies(&mut wm, &mut host),
                8 => mgr.resume_all_movies(&mut wm, &mut host),
                9 => mgr.update(&mut wm, &mut host),
                10 => mgr.reset(&mut wm, &mut host),
                11 => mgr.init(),
                12 => {
                    if b == 1 {
                        wm.tree.windows[w].status |= win_status::HIDDEN;
                    } else {
                        wm.tree.windows[w].status &= !win_status::HIDDEN;
                    }
                }
                _ => st13 = Some(state_word(mgr.get_win_state(w))),
            }
            rust.extend(host.log.iter().copied());
            if let Some(s) = st13 {
                rust.push(s);
            }
            rust.push(100);
            put_state(&wm, &order, &mut rust);
            for &ww in &order {
                rust.push(state_word(mgr.get_win_state(ww)));
            }
            rust.extend([mgr.playing_videos.len() as u32, u32::from(mgr.stop_all_movies), u32::from(mgr.pause_all_movies)]);
        }
        let mut buf = vec![0u32; 1 << 17];
        let len = unsafe { ref_ui3b_wvm_run(input.as_ptr(), input.len() as i32, buf.as_mut_ptr(), buf.len() as i32) };
        assert!(len >= 0, "output buffer too small");
        let cpp = buf[..len as usize].to_vec();
        chk.eq(rust.clone(), cpp.clone(), || format!("{} input {input:?}", diff_note(&rust, &cpp)));
    }
    finish(chk);
}
