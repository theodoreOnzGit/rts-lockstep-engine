#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/VideoPlayer.cpp
//   GeneralsMD/Code/GameEngine/Include/GameClient/VideoPlayer.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::video_player` (`VideoBuffer`, `VideoStream`, `VideoPlayer`) against upstream's base classes (#215, group lc2).
//!
//! # Method
//!
//! The class declarations of `VideoPlayer.h` and the definitions of `VideoPlayer.cpp` (all but `init`, which loads INI files, and the field
//! table) are cut verbatim by build.rs into `differential/wrap_draft_video_player.cpp` (stand-ins listed there: `AsciiString` as a
//! `std::string`, `RectClass`, an empty `SubsystemInterface`). Three comparisons:
//!
//! 1. **Buffer** (20,000 cases): the constructor's fields for each of the five `Type`s, then `Rect(x1,y1,x2,y2)` on a valid and on an invalid
//!    buffer with random sizes (zero texture sizes included, so 0/0 and x/0 reach the division) and floats from the mixed generator, compared by
//!    bits, then `free()`'s four zeroed fields.
//! 2. **Stream defaults**: a fresh stream's `next`, `isFrameReady`, `frameIndex`, `frameCount`, `height`, `width`, and `close()` deleting it.
//! 3. **Player** (20,000 programs of 1 to 30 operations): an arena of 1 to 8 streams, some linked into the player's list in a random order, then
//!    `addVideo`, `removeVideo`, `remove`, `update`, `close`, `closeAllStreams`, `reset`, both `getVideo`s with names that differ only in case
//!    and indices below 0 and past the end. After every operation: the first stream, every stream's alive flag and `next`, the video list
//!    (name, file, comment, in order) and the answer of a `getVideo`.
//!
//! **Excluded inputs (undefined upstream).** `closeAllStreams` and `reset` run only when every stream in the list has a player (upstream's loop is
//! `while (stream = firstStream()) stream->close();` and `close()` is `delete this`, which unlinks only through `m_player`: a stream with no
//! player stays first, is deleted, and the next `firstStream()` reads freed memory, a loop the draft ends instead: see its comment). `close`
//! runs only on a live stream that has a player or is not in the list (otherwise the list keeps a freed stream). `VideoBuffer::VideoBuffer`
//! with a `Type` outside 0..=4 becomes `TYPE_UNKNOWN` upstream; the draft's enum cannot hold one. `Rect` with the buffer's `valid()` virtual
//! is the test's own flag. `init` loads INI files and the field table names INI parsers (host's).
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches: the draft reads as the same statements as the cuts.
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches** (the prediction held, no draft change): 120,000 buffer values (20,000 cases x constructor, four `Rect` floats, `free`; NaN-payload-only 2,
//! max ULP 0), 2 stream-default comparisons and 20,000 player programs. Planted bugs, 11 of 11 caught (mismatches in the buffer / stream / player checks):
//! 16861 `Rect`'s y scale uses the width 26,384; 16862 the scale computed in f64 1,004; 16863 `right` from x1 13,399; 16864 `Rect` of an invalid buffer not zero 19,003;
//! 16865 `free` keeps the texture height 17,851; 16866 `remove` of the first stream leaves `m_firstStream` 8,817; 16867 `remove` also drops the next stream 2,803;
//! 16868 `addVideo` over an existing name replaces only the file name 8,243; 16869 `removeVideo` erases the entry after the match 3,157; 16870 `getVideo(0)` is NULL 18,048;
//! 16871 `close` leaves the stream in the list 4,655. (16872..16880 are unused; the block was not contiguous.)

mod support;

use rts_lockstep_engine::drafts::logic_common::video_player::{Video, VideoBuffer, VideoBufferType, VideoPlayer, VideoStream};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc2_vb_run(
        fmt: i32, w: u32, h: u32, tw: u32, th: u32, pitch: u32, x: u32, y: u32, valid: i32, x1: f32, y1: f32, x2: f32, y2: f32, out: *mut u32,
    ) -> i32;
    fn ref_lc2_vs_defaults(out: *mut i32) -> i32;
    fn ref_lc2_vp_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

const NAMES: [&str; 8] = ["", "a", "A", "ab", "b", "movie1", "Movie1", "x"];
const FILES: [&str; 6] = ["", "f0.bik", "f1.bik", "F0.bik", "f2", "g"];
const COMMENTS: [&str; 4] = ["", "c0", "c1", "C0"];

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn finish_all(checks: Vec<Check>) {
    if planted() {
        for c in checks {
            c.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    } else {
        for c in checks {
            c.finish();
        }
    }
}

fn format_of(i: i32) -> VideoBufferType {
    [VideoBufferType::Unknown, VideoBufferType::R8G8B8, VideoBufferType::X8R8G8B8, VideoBufferType::R5G6B5, VideoBufferType::X1R5G5B5][i as usize]
}

fn buffer_log(b: &VideoBuffer) -> [u32; 8] {
    [b.format as u32, b.x_pos, b.y_pos, b.width, b.height, b.texture_width, b.texture_height, b.pitch]
}

fn size(g: &mut Gen) -> u32 {
    match g.below(10) {
        0 => 0,
        1..=6 => g.below(70),
        7 => 1 + g.below(2048),
        8 => 256 << g.below(4),
        _ => g.u32(),
    }
}

fn video_of(name: i32, file: i32, comment: i32) -> Video {
    Video {
        filename: FILES[file as usize].as_bytes().to_vec(),
        internal_name: NAMES[name as usize].as_bytes().to_vec(),
        comment_for_wb: COMMENTS[comment as usize].as_bytes().to_vec(),
    }
}

fn id_of(table: &[&str], v: &[u8]) -> i32 {
    table.iter().position(|t| t.as_bytes() == v).map(|i| i as i32).unwrap_or(-1)
}

fn log_video(log: &mut Vec<i32>, v: Option<&Video>) {
    match v {
        None => log.extend([-1, -1, -1]),
        Some(v) => log.extend([id_of(&NAMES, &v.internal_name), id_of(&FILES, &v.filename), id_of(&COMMENTS, &v.comment_for_wb)]),
    }
}

fn opt(o: Option<usize>) -> i32 {
    o.map(|i| i as i32).unwrap_or(-1)
}

fn run_rust(n: usize, has_player: &[i32], chain: &[usize], ops: &[[i32; 4]]) -> Vec<i32> {
    let mut p = VideoPlayer::new();
    for i in 0..n {
        let mut s = VideoStream::new();
        s.has_player = has_player[i] != 0;
        p.streams.push(Some(s));
    }
    for w in chain.windows(2) {
        p.streams[w[0]].as_mut().unwrap().next = Some(w[1]);
    }
    p.first_stream = chain.first().copied();
    let mut log: Vec<i32> = Vec::new();
    for &[kind, a, b, c] in ops {
        let mut ans: Option<[i32; 3]> = None;
        match kind {
            0 => p.add_video(&video_of(a, b, c)),
            1 => {
                let mut v = video_of(a, 0, 0);
                v.filename = b"ignored".to_vec();
                p.remove_video(&v);
            }
            2 => p.remove(a as usize),
            3 => p.update(),
            4 => p.close_stream(a as usize),
            5 => p.close_all_streams(),
            6 => p.reset(),
            7 => {
                let mut l = Vec::new();
                log_video(&mut l, p.get_video_by_name(NAMES[a as usize].as_bytes()));
                ans = Some([l[0], l[1], l[2]]);
            }
            _ => {
                let mut l = Vec::new();
                log_video(&mut l, p.get_video_by_index(a));
                ans = Some([l[0], l[1], l[2]]);
            }
        }
        log.push(opt(p.first_stream()));
        for i in 0..n {
            match &p.streams[i] {
                Some(s) => {
                    log.push(1);
                    log.push(opt(s.next));
                }
                None => {
                    log.push(0);
                    log.push(-2);
                }
            }
        }
        log.push(p.get_num_videos());
        for i in 0..p.get_num_videos() {
            log_video(&mut log, p.get_video_by_index(i));
        }
        if let Some(a) = ans {
            log.extend(a);
        }
    }
    log
}

#[test]
fn video_player_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C0DE);
    let mut cb = Check::new("VideoBuffer (new, Rect, free)");
    let mut cs = Check::new("VideoStream defaults");
    let mut cp = Check::new("VideoPlayer");

    // 1. the buffer
    for case in 0..20_000 {
        let fmt = g.below(5) as i32;
        let (w, h, tw, th) = (size(&mut g), size(&mut g), size(&mut g), size(&mut g));
        let (pitch, x, y) = (g.u32(), g.u32(), g.u32());
        let valid = g.below(4) != 0;
        let (x1, y1, x2, y2) = (g.mixed(), g.mixed(), g.mixed(), g.mixed());
        let mut cpp = [0u32; 20];
        let len = unsafe { ref_lc2_vb_run(fmt, w, h, tw, th, pitch, x, y, valid as i32, x1, y1, x2, y2, cpp.as_mut_ptr()) };
        assert_eq!(len, 20);

        let mut b = VideoBuffer::new(format_of(fmt));
        cb.eq(buffer_log(&b).to_vec(), cpp[0..8].to_vec(), || format!("case {case}: constructor, format {fmt}"));
        b.width = w;
        b.height = h;
        b.texture_width = tw;
        b.texture_height = th;
        b.pitch = pitch;
        b.x_pos = x;
        b.y_pos = y;
        let r = b.rect(valid, x1, y1, x2, y2);
        let input = || format!("case {case}: Rect valid {valid} {w}x{h} / {tw}x{th} ({x1:e},{y1:e},{x2:e},{y2:e})");
        for (k, v) in [r.left, r.top, r.right, r.bottom].into_iter().enumerate() {
            cb.f32(v, f32::from_bits(cpp[8 + k]), || format!("{} [component {k}]", input()));
        }
        b.free();
        cb.eq(buffer_log(&b).to_vec(), cpp[12..20].to_vec(), || format!("case {case}: free, {w}x{h} / {tw}x{th}"));
    }

    // 2. a fresh stream
    {
        let mut cpp = [0i32; 7];
        let len = unsafe { ref_lc2_vs_defaults(cpp.as_mut_ptr()) };
        assert_eq!(len, 7);
        let s = VideoStream::new();
        let rust = [s.next.is_none() as i32, s.is_frame_ready() as i32, s.frame_index(), s.frame_count(), s.height(), s.width(), 0];
        cs.eq(rust.to_vec(), cpp.to_vec(), || "a fresh stream".to_string());
        let mut p = VideoPlayer::new();
        p.streams.push(Some(VideoStream::new()));
        p.close_stream(0);
        cs.eq(p.streams[0].is_none() as i32, 1 - cpp[6], || "close() deletes the stream".to_string());
    }

    // 3. the player
    for case in 0..20_000 {
        let n = 1 + g.below(8) as usize;
        let kind_a = g.below(2) == 0;
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = g.below(i as u32 + 1) as usize;
            order.swap(i, j);
        }
        let chain: Vec<usize> = order[..g.below(n as u32 + 1) as usize].to_vec();
        let mut has_player: Vec<i32> = (0..n).map(|_| g.below(2) as i32).collect();
        if kind_a {
            for &i in &chain {
                has_player[i] = 1;
            }
        }
        let nops = 1 + g.below(30);
        let mut ops: Vec<[i32; 4]> = Vec::new();
        // the shadow of what is live and in the list, to draw only defined operations
        let mut live = vec![true; n];
        let mut in_list = vec![false; n];
        for &i in &chain {
            in_list[i] = true;
        }
        let mut nvideos = 0i32;
        for _ in 0..nops {
            let k = match g.below(12) {
                0..=2 => 0,
                3 => 1,
                4 | 5 => 2,
                6 => 3,
                7 => 4,
                8 => 5,
                9 => 6,
                10 => 7,
                _ => 8,
            };
            let live_ids: Vec<usize> = (0..n).filter(|&i| live[i]).collect();
            let op = match k {
                0 => {
                    nvideos += 1;
                    [0, g.below(8) as i32, g.below(6) as i32, g.below(4) as i32]
                }
                1 => [1, g.below(8) as i32, 0, 0],
                2 | 4 if live_ids.is_empty() => [3, 0, 0, 0],
                2 => [2, live_ids[g.below(live_ids.len() as u32) as usize] as i32, 0, 0],
                4 => {
                    let ok: Vec<usize> = live_ids.iter().copied().filter(|&i| kind_a && (has_player[i] != 0 || !in_list[i])).collect();
                    if ok.is_empty() {
                        [3, 0, 0, 0]
                    } else {
                        let i = ok[g.below(ok.len() as u32) as usize];
                        live[i] = false;
                        in_list[i] = false;
                        [4, i as i32, 0, 0]
                    }
                }
                3 => [3, 0, 0, 0],
                5 | 6 => {
                    if kind_a {
                        for i in 0..n {
                            if in_list[i] {
                                live[i] = false;
                                in_list[i] = false;
                            }
                        }
                        [k, 0, 0, 0]
                    } else {
                        [3, 0, 0, 0]
                    }
                }
                7 => [7, g.below(8) as i32, 0, 0],
                _ => [8, g.below(nvideos.max(0) as u32 + 5) as i32 - 2, 0, 0],
            };
            if op[0] == 2 {
                // remove(i) unlinks i from the list
                in_list[op[1] as usize] = false;
            }
            ops.push(op);
        }
        let mut prog: Vec<i32> = vec![n as i32];
        prog.extend(&has_player);
        prog.push(chain.len() as i32);
        prog.extend(chain.iter().map(|&i| i as i32));
        for o in &ops {
            prog.extend(o);
        }
        let mut cpp = vec![0i32; 20_000];
        let len = unsafe { ref_lc2_vp_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);
        let rust = run_rust(n, &has_player, &chain, &ops);
        cp.eq(rust, cpp, || format!("case {case}: kind_a {kind_a}, {n} streams, player flags {has_player:?}, chain {chain:?}, ops {ops:?}"));
    }
    finish_all(vec![cb, cs, cp]);
}
