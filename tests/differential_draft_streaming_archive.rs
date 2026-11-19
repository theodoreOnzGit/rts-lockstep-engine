#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/StreamingArchiveFile.cpp (with File.cpp's File::open/close, the base class)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::streaming_archive_file::StreamingArchiveFile` against `StreamingArchiveFile` (#215).
//!
//! # Method
//!
//! `StreamingArchiveFile::StreamingArchiveFile` to the end of `StreamingArchiveFile.cpp`, and `File::File` to `File::print`, are cut verbatim
//! (build.rs) into `differential/wrap_draft_streaming_archive.cpp` on the stand-ins it lists (File's declaration; the archive's `File` over a
//! byte block whose seek clamps to `[0, size]`, as `RAMFile`'s does). The draft runs against a Rust model of the same archive. A case is a
//! random op sequence (openFromArchive with or without an archive file, window offset and size from edge values, close, read with and without a
//! buffer, write, seek in all three modes, `open(File*)`, destroy) on one file over a 0 to 60 byte archive; both sides log every archive call
//! (position, mode, bytes, result), each op's result and the state (`m_open`, `m_file` set, `m_startingPos`, `m_size`, `m_curPos`, the name)
//! after it; the logs are compared whole. 20,000 sequences from a fixed seed.
//!
//! Excluded inputs: `open(filename, access)` (the draft has no such function; upstream asks `TheFileSystem`); `m_deleteOnClose`;
//! `bytes + m_curPos` and `m_curPos + pos` stay far from `i32` overflow (signed overflow is undefined in C).
//!
//! # Prediction (written before the first run, 2026-11-19)
//!
//! Mismatches expected from reading the draft against the cut: (1) the name: the constructor's is `<no file>` (File::File) and `File::close`
//! sets it back; the draft starts empty and never resets it; (2) a second `openFromArchive` on an open file is refused by `File::open`; the draft
//! reopens. Everything else: 0.
//!
//! # Results
//!
//! 2026-11-19: both predicted causes were fixed in the draft before the first run (so no "before" count was taken): `new` starts with the name
//! `<no file>` and `close` restores it; `open_from_archive` refuses an open file. **0 mismatches over 20,000 sequences** (16,870 opened with an
//! archive), max ULP 0. Planted bugs, 10 of 10 caught (of 20,000): 15271 the re-open refusal omitted 8,927; 15272 `close` keeps the name (the
//! original draft) 6,270; 15273 the first window seek unchecked 5,135; 15274 the second seek compared with `size` instead of `offset + size`
//! 6,782; 15275 `read` does not clamp to the window 8,220; 15276 an END seek uses `size - pos` 4,259; 15277 the upper clamp of `seek` omitted
//! 12,923; 15278 `read` does not reposition the archive 10,543; 15279 `m_curPos` advanced by the request, not the bytes read 3,945; 15280 the
//! final seek back to `offset` omitted 8,487.

mod support;

use rts_lockstep_engine::drafts::logic_common::local_file::SeekMode;
use rts_lockstep_engine::drafts::logic_common::streaming_archive_file::{ArchiveHandle, StreamingArchiveFile};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_saf_run(data: *const u8, n: i32, ops: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
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

struct Mem {
    data: Vec<u8>,
    pos: i32,
    log: Vec<i32>,
}

impl ArchiveHandle for Mem {
    fn seek(&mut self, pos: i32, mode: SeekMode) -> i32 {
        let (m, np) = match mode {
            SeekMode::Start => (0, pos),
            SeekMode::Current => (1, self.pos + pos),
            SeekMode::End => (2, self.data.len() as i32 + pos),
        };
        self.pos = np.clamp(0, self.data.len() as i32);
        self.log.extend([200, 1, pos, m, self.pos]);
        self.pos
    }
    fn read(&mut self, buffer: Option<&mut [u8]>, bytes: i32) -> i32 {
        let k = (self.data.len() as i32 - self.pos).min(bytes).max(0);
        let isnull = buffer.is_none() as i32;
        if let Some(b) = buffer {
            if k > 0 {
                b[..k as usize].copy_from_slice(&self.data[self.pos as usize..(self.pos + k) as usize]);
            }
        }
        self.log.extend([200, 2, bytes, isnull, k]);
        self.pos += k;
        k
    }
}

fn state(log: &mut Vec<i32>, f: &StreamingArchiveFile) {
    log.extend([f.open as i32, f.has_file as i32, f.starting_pos, f.size, f.cur_pos, f.name.len() as i32]);
    log.extend(f.name.iter().map(|&b| b as i32));
}

fn run_rust(data: &[u8], ops: &[i32]) -> Vec<i32> {
    let mut a = Mem { data: data.to_vec(), pos: 0, log: Vec::new() };
    let mut f = StreamingArchiveFile::new();
    let mut log = Vec::new();
    state(&mut log, &f);
    let mut p = 0;
    while p < ops.len() {
        let op = ops[p];
        p += 1;
        log.push(1000 + op);
        let mut res: Vec<i32> = Vec::new();
        match op {
            0 => {
                let (has, name, offset, size) = (ops[p], ops[p + 1], ops[p + 2], ops[p + 3]);
                p += 4;
                let nm: &[u8] = match name {
                    0 => b"a.big",
                    1 => b"b",
                    2 => b"",
                    _ => b"dir/c.dat",
                };
                let r = f.open_from_archive(if has != 0 { Some(&mut a) } else { None }, nm, offset, size);
                res.push(r as i32);
            }
            1 => f.close(),
            2 => {
                let (isnull, k) = (ops[p], ops[p + 1]);
                p += 2;
                let mut buf = [0xAAu8; 64];
                let r = f.read(&mut a, if isnull != 0 { None } else { Some(&mut buf) }, k);
                res.push(r);
                if isnull == 0 {
                    res.extend(buf.iter().take(r.max(0).min(k.max(0)) as usize).map(|&b| b as i32));
                }
            }
            3 => {
                let k = ops[p];
                p += 1;
                res.push(f.write(Some(&[1, 2, 3, 4]), k));
            }
            4 => {
                let (pos, mode) = (ops[p], ops[p + 1]);
                p += 2;
                let m = match mode {
                    0 => SeekMode::Start,
                    1 => SeekMode::Current,
                    _ => SeekMode::End,
                };
                res.push(f.seek(pos, m));
            }
            5 => f = StreamingArchiveFile::new(),
            _ => res.push(f.open_from_file() as i32),
        }
        log.append(&mut a.log);
        log.extend(res);
        state(&mut log, &f);
    }
    log
}

fn edge(g: &mut Gen, hi: u32) -> i32 {
    match g.below(8) {
        0 => 0,
        1 => hi as i32,
        2 => hi as i32 + 1,
        3 => g.below(3) as i32 - 1,
        _ => g.below(hi + 3) as i32,
    }
}

#[test]
fn streaming_archive_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_5AF0);
    let mut c = Check::new("StreamingArchiveFile");
    let mut opened = 0u32;
    for case in 0..20_000 {
        let n = if g.below(6) == 0 { g.below(3) } else { g.below(61) } as usize;
        let data: Vec<u8> = (0..n).map(|_| g.below(256) as u8).collect();
        let mut ops: Vec<i32> = Vec::new();
        for _ in 0..2 + g.below(14) {
            match g.below(20) {
                0..=4 => {
                    let offset = edge(&mut g, n as u32);
                    let size = if g.below(3) == 0 { edge(&mut g, n as u32) } else { (n as i32 - offset).max(0) + g.below(3) as i32 - 1 };
                    ops.extend([0, (g.below(8) != 0) as i32, g.below(4) as i32, offset, size]);
                }
                5 | 6 => ops.push(1),
                7 => ops.push(5),
                8..=12 => ops.extend([2, (g.below(8) == 0) as i32, if g.below(4) == 0 { g.below(70) as i32 - 3 } else { g.below(13) as i32 }]),
                13 => ops.extend([3, g.below(10) as i32]),
                14..=18 => ops.extend([4, edge(&mut g, n as u32) - g.below(4) as i32, g.below(3) as i32]),
                _ => ops.push(6),
            }
        }
        let mut out = vec![0i32; 20_000];
        let len = unsafe { ref_saf_run(data.as_ptr(), data.len() as i32, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32) };
        assert!(len >= 0, "the log did not fit");
        out.truncate(len as usize);
        let rust = run_rust(&data, &ops);
        if ops.windows(5).any(|w| w[0] == 0 && w[1] == 1) {
            opened += 1;
        }
        let first = rust.iter().zip(out.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(out.len()));
        c.eq(rust == out, true, || format!("case {case}: archive {data:?} ops {ops:?}; first difference at word {first}: rust {:?} c++ {:?}", rust.get(first.saturating_sub(6)..(first + 4).min(rust.len())), out.get(first.saturating_sub(6)..(first + 4).min(out.len()))));
    }
    println!("(streaming_archive: {opened} sequences opened with an archive)");
    finish(c);
}
