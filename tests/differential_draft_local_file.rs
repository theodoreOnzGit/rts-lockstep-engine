#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/LocalFile.cpp (with File.cpp's File::open/close/size, the base class)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::local_file::LocalFile` against `LocalFile` (#215).
//!
//! # Method
//!
//! `File::File` to `File::position` and `LocalFile::LocalFile` to `convertToRAMFile`, and `readEntireAndClose`, are cut verbatim (build.rs)
//! into `differential/wrap_draft_local_file.cpp`, compiled as the shipped game (no `USE_BUFFERED_IO`, no `_DEBUG`), on stand-ins listed
//! there: the class declarations, `AsciiString` (with `concat(char)` as `AsciiString.h` has it: a NUL appends nothing), and the C
//! runtime's `_open/_read/_write/_lseek/_close` over four in-memory files and eight handles. The draft runs against a Rust model of the same
//! device. A case is a random op sequence (open with random access bits, close, destroy, read, write, seek, scanInt, scanReal, scanString,
//! nextLine, readEntireAndClose) on one file object over three random files. Both sides log, as words, every device call (arguments and
//! result), every op's result and the object's state (`m_access`, `m_open`, `m_handle`, `s_totalOpen`) after it, then the final contents of
//! the four files; the two logs are compared whole. 20,000 sequences from a fixed seed.
//!
//! Excluded inputs: digit runs longer than 9 for `scanInt` (`atoi` overflow is undefined in C; a quarter of the sequences hold long digit
//! runs for `scanReal` only, which they do not use `scanInt` on); `nextLine` with a non-NULL buffer of size < 1 (index below the buffer);
//! `m_deleteOnClose` (`File::close` would delete the object); `convertToRAMFile` (RAMFile is not drafted). Not modelled by the device: text
//! mode's CRLF translation and `_read`'s -1 on a bad handle (the scan loops of the cut never end on -1 with an unset char).
//!
//! # Prediction (written before the first run, 2026-11-19)
//!
//! Mismatches expected, from reading the draft against the cuts: (1) `open_file` records the access mask as given, but `File::open`
//! rejects a second open, STREAMING with WRITE, TEXT with BINARY, and normalises the mask (READ added, TRUNCATE added, BINARY added);
//! (2) the draft's `close_file` closes the handle, upstream's `close()` only clears `m_open` (the handle closes in the destructor);
//! (3) `scan_string` keeps a NUL byte that upstream's `AsciiString::concat(char)` drops. Everything else: 0.
//!
//! # Results
//!
//! 2026-11-19, first run (the draft as drafted): 20,000 sequences, **18,408 mismatches**, the first at the access word of an `open`
//! (the draft recorded 32 where upstream's `File::open` records 33: READ added). The prediction held: the three causes were fixed in the draft
//! (`open_file` now does `File::open`'s refusals and its three defaults, with a new `FILE_STREAMING`; `close_file` is `File::close` only, the
//! handle closing in `drop_file` as in the destructor; `scan_string` drops a NUL as `concat(char)` does). Second run: **28 mismatches**, all
//! `scanInt` of a digit run longer than 9 (two digit pieces of the generator joined; `atoi` overflow, undefined in C, wraps in the draft and
//! saturates in glibc): a miss of the *generator*, not the draft; the generator now keeps `scanInt` out of files with such a run (the
//! excluded input above). Third run: **0 mismatches over 20,000 sequences** (19,093 opened a file; about 16,700 scans), max ULP 0.
//!
//! Planted bugs, 10 of 10 caught (of 20,000 sequences): 15261 TRUNCATE default omitted 1,814; 15262 READ default omitted 8,488; 15263 BINARY
//! default omitted 13,029; 15264 the STREAMING|WRITE refusal omitted 671; 15265 the TEXT|BINARY refusal omitted 1,676; 15266 a second open
//! allowed 7,980; 15267 `close` also closes the handle (the original draft) 10,215; 15268 `scan_string` keeps a NUL (the original draft)
//! 1,047; 15269 `scan_int` does not put the char back 291; 15270 `scan_real` accepts a second `.` 653.
//!
//! Not covered: `convert_to_ram_file` (a stub: RAMFile is not drafted), the `USE_BUFFERED_IO` branch (not compiled in the shipped game).

mod support;

use rts_lockstep_engine::drafts::logic_common::local_file::{LocalFile, LocalFileDevice, LocalFileTotals, OpenFlags, SeekMode};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lf_local_run(f0: *const u8, n0: i32, f1: *const u8, n1: i32, f2: *const u8, n2: i32, ops: *const i32, nops: i32, out: *mut i32, cap: i32) -> i32;
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

/// The wrapper's C runtime stand-in, line for line (see the wrapper's header).
struct Dev {
    files: [(Vec<u8>, bool); 4],
    slots: [Option<(usize, i32, i32)>; 8],
    log: Vec<i32>,
}

fn flags_mask(f: OpenFlags) -> i32 {
    let mut m = 0;
    if f.creat {
        m |= 0x100;
    }
    if f.trunc {
        m |= 0x200;
    }
    if f.append {
        m |= 8;
    }
    if f.text {
        m |= 0x4000;
    }
    if f.binary {
        m |= 0x8000;
    }
    if f.rdwr {
        m |= 2;
    }
    if f.wronly {
        m |= 1;
    }
    m
}

fn readable(fl: i32) -> bool {
    (fl & 3) != 1
}
fn writable(fl: i32) -> bool {
    (fl & 3) != 0
}

impl Dev {
    fn new(f: &[Vec<u8>; 3]) -> Dev {
        Dev { files: [(f[0].clone(), true), (f[1].clone(), true), (f[2].clone(), true), (Vec::new(), false)], slots: [None; 8], log: Vec::new() }
    }
    fn slot(&self, h: i32) -> Option<usize> {
        if (3..11).contains(&h) && self.slots[(h - 3) as usize].is_some() {
            Some((h - 3) as usize)
        } else {
            None
        }
    }
}

impl LocalFileDevice for Dev {
    fn open(&mut self, filename: &[u8], flags: OpenFlags) -> i32 {
        let flags = flags_mask(flags);
        let mut r = -1;
        let f: i32 = if filename.len() == 2 && filename[0] == b'f' && (b'0'..=b'3').contains(&filename[1]) { (filename[1] - b'0') as i32 } else { -1 };
        if f >= 0 {
            let fu = f as usize;
            let mut ok = true;
            if !self.files[fu].1 {
                if flags & 0x100 != 0 {
                    self.files[fu].1 = true;
                    self.files[fu].0.clear();
                } else {
                    ok = false;
                }
            } else if flags & 0x200 != 0 && writable(flags) {
                self.files[fu].0.clear();
            }
            if ok {
                for i in 0..8 {
                    if r < 0 && self.slots[i].is_none() {
                        self.slots[i] = Some((fu, 0, flags));
                        r = i as i32 + 3;
                    }
                }
            }
        }
        self.log.extend([1, f, flags, r]);
        r
    }
    fn close(&mut self, handle: i32) {
        if let Some(i) = self.slot(handle) {
            self.slots[i] = None;
        }
        self.log.extend([5, handle]);
    }
    fn read(&mut self, handle: i32, buf: &mut [u8]) -> i32 {
        let n = buf.len() as i32;
        let mut r = 0;
        if let Some(i) = self.slot(handle) {
            let (f, pos, fl) = self.slots[i].unwrap();
            if readable(fl) {
                let size = self.files[f].0.len() as i32;
                let k = (size - pos).min(n).max(0);
                if k > 0 {
                    buf[..k as usize].copy_from_slice(&self.files[f].0[pos as usize..(pos + k) as usize]);
                }
                self.slots[i] = Some((f, pos + k, fl));
                r = k;
            }
        }
        self.log.extend([2, handle, n, r]);
        r
    }
    fn write(&mut self, handle: i32, buf: &[u8]) -> i32 {
        let n = buf.len() as i32;
        let mut r = -1;
        if let Some(i) = self.slot(handle) {
            let (f, mut pos, fl) = self.slots[i].unwrap();
            if writable(fl) {
                let d = &mut self.files[f].0;
                if fl & 8 != 0 {
                    pos = d.len() as i32;
                }
                if pos + n > d.len() as i32 {
                    d.resize((pos + n) as usize, 0);
                }
                d[pos as usize..(pos + n) as usize].copy_from_slice(buf);
                self.slots[i] = Some((f, pos + n, fl));
                r = n;
            }
        }
        self.log.extend([3, handle, n, r]);
        r
    }
    fn lseek(&mut self, handle: i32, off: i32, mode: SeekMode) -> i32 {
        let m = match mode {
            SeekMode::Start => 0,
            SeekMode::Current => 1,
            SeekMode::End => 2,
        };
        let mut r = -1;
        if let Some(i) = self.slot(handle) {
            let (f, pos, fl) = self.slots[i].unwrap();
            let base = match m {
                0 => 0,
                1 => pos,
                _ => self.files[f].0.len() as i32,
            };
            let np = base as i64 + off as i64;
            if np >= 0 {
                self.slots[i] = Some((f, np as i32, fl));
                r = np as i32;
            }
        }
        self.log.extend([4, handle, off, m, r]);
        r
    }
}

fn state(log: &mut Vec<i32>, lf: &LocalFile, t: &LocalFileTotals) {
    log.extend([lf.access, lf.open as i32, lf.handle, t.total_open]);
}

/// The draft on the op stream; the wrapper's loop, word for word.
fn run_rust(files: &[Vec<u8>; 3], ops: &[i32]) -> Vec<i32> {
    let mut dev = Dev::new(files);
    let mut t = LocalFileTotals::default();
    let mut lf = LocalFile::new();
    let mut log: Vec<i32> = Vec::new();
    let mut p = 0;
    while p < ops.len() {
        let op = ops[p];
        p += 1;
        log.push(1000 + op);
        // the device's calls go inline, before the result: they are moved over from the device's log after each call
        let mut res: Vec<i32> = Vec::new();
        match op {
            0 => {
                let name = [b'f', b'0' + ops[p] as u8];
                let access = ops[p + 1];
                p += 2;
                let r = lf.open_file(&mut dev, &mut t, &name, access);
                res.push(r as i32);
            }
            1 => lf.close_file(&mut dev, &mut t),
            2 => {
                lf.drop_file(&mut dev, &mut t);
                lf = LocalFile::new();
            }
            3 => {
                let (isnull, n) = (ops[p], ops[p + 1]);
                p += 2;
                let mut buf = [0xAAu8; 64];
                let r = lf.read(&mut dev, if isnull != 0 { None } else { Some(&mut buf) }, n);
                res.push(r);
                if isnull == 0 && r > 0 {
                    let k = r.min(n) as usize;
                    res.extend(buf[..k].iter().map(|&b| b as i32));
                }
            }
            4 => {
                let (isnull, n) = (ops[p], ops[p + 1] as usize);
                p += 2;
                let bytes: Vec<u8> = ops[p..p + n].iter().map(|&w| w as u8).collect();
                p += n;
                let r = lf.write(&mut dev, if isnull != 0 { None } else { Some(&bytes) }, n as i32);
                res.push(r);
            }
            5 => {
                let (pos, mode) = (ops[p], ops[p + 1]);
                p += 2;
                let m = match mode {
                    0 => SeekMode::Start,
                    1 => SeekMode::Current,
                    _ => SeekMode::End,
                };
                res.push(lf.seek(&mut dev, pos, m));
            }
            6 => {
                let mut v = 0;
                let ok = lf.scan_int(&mut dev, &mut v);
                res.extend([ok as i32, v]);
            }
            7 => {
                let mut v = 0.0f32;
                let ok = lf.scan_real(&mut dev, &mut v);
                res.extend([ok as i32, v.to_bits() as i32]);
            }
            8 => {
                let mut s = Vec::new();
                let ok = lf.scan_string(&mut dev, &mut s);
                res.extend([ok as i32, s.len() as i32]);
                res.extend(s.iter().map(|&b| b as i32));
            }
            9 => {
                let (isnull, buf_size) = (ops[p], ops[p + 1]);
                p += 2;
                let mut buf = [0x55u8; 64];
                lf.next_line(&mut dev, if isnull != 0 { None } else { Some(&mut buf) }, buf_size);
                if isnull == 0 {
                    res.extend(buf[..(buf_size + 1) as usize].iter().map(|&b| b as i32));
                }
            }
            _ => {
                let k = match dev.slot(lf.handle) {
                    Some(i) if lf.open && readable(dev.slots[i].unwrap().2) => (dev.files[dev.slots[i].unwrap().0].0.len() as i32 - dev.slots[i].unwrap().1).max(0),
                    _ => 0,
                };
                let b = lf.read_entire_and_close(&mut dev, &mut t);
                res.push(k);
                res.extend(b[..k as usize].iter().map(|&x| x as i32));
            }
        }
        log.append(&mut dev.log);
        log.extend(res);
        state(&mut log, &lf, &t);
    }
    for i in 0..4 {
        log.push(dev.files[i].1 as i32);
        log.push(dev.files[i].0.len() as i32);
        log.extend(dev.files[i].0.iter().map(|&b| b as i32));
    }
    lf.drop_file(&mut dev, &mut t);
    log.append(&mut dev.log);
    log.push(t.total_open);
    log
}

fn gen_file(g: &mut Gen, long_digits: bool) -> Vec<u8> {
    let target = match g.below(6) {
        0 => 0,
        1 => g.below(6) as usize,
        _ => g.below(120) as usize,
    };
    let mut v: Vec<u8> = Vec::new();
    let digits = |g: &mut Gen, v: &mut Vec<u8>| {
        let n = if long_digits && g.below(3) == 0 { 1 + g.below(40) } else { 1 + g.below(3).max(g.below(9)) };
        for _ in 0..n {
            v.push(b'0' + g.below(10) as u8);
        }
    };
    while v.len() < target {
        match g.below(14) {
            0..=2 => digits(g, &mut v),
            3 => v.push(b'-'),
            4 => v.push(b'.'),
            5 => {
                digits(g, &mut v);
                v.push(b'.');
                digits(g, &mut v);
            }
            6 | 7 => v.push([b' ', b'\t', b'\n', b'\r', 0x0b, 0x0c][g.below(6) as usize]),
            8 => {
                for _ in 0..1 + g.below(5) {
                    v.push(b'a' + g.below(26) as u8);
                }
            }
            9 => v.push(g.below(256) as u8),
            10 => v.extend_from_slice(b"\r\n"),
            11 => {
                v.push(b'-');
                digits(g, &mut v);
            }
            12 => v.extend_from_slice([&b".5"[..], b"5.", b"-.5", b"1.2.3", b"--4", b"4-5", b"-.", b"0", b"-0", b"0.0"][g.below(10) as usize]),
            _ => v.push(0),
        }
    }
    v
}

fn max_digit_run(f: &[u8]) -> usize {
    let (mut best, mut cur) = (0, 0);
    for &b in f {
        cur = if b.is_ascii_digit() { cur + 1 } else { 0 };
        best = best.max(cur);
    }
    best
}

fn gen_access(g: &mut Gen) -> i32 {
    let mut a = 0;
    for (bit, pct) in [(1, 55), (2, 25), (4, 20), (8, 25), (0x10, 20), (0x20, 20), (0x40, 30), (0x80, 10), (0x100, 10)] {
        if g.below(100) < pct {
            a |= bit;
        }
    }
    a
}

fn gen_ops(g: &mut Gen, scan_int_ok: bool) -> Vec<i32> {
    let mut ops: Vec<i32> = Vec::new();
    let n = 3 + g.below(22);
    if g.below(100) < 85 {
        ops.extend([0, g.below(4).min(if g.below(5) == 0 { 3 } else { 2 }) as i32, gen_access(g)]);
    }
    for _ in 0..n {
        let mut k = g.below(100);
        loop {
            match k {
                0..=9 => ops.extend([0, g.below(4) as i32, gen_access(g)]),
                10..=15 => ops.push(1),
                16..=19 => ops.push(2),
                20..=33 => {
                    let n = if g.below(3) == 0 { g.below(65) } else { g.below(13) } as i32;
                    ops.extend([(g.below(8) == 0) as i32, n]);
                    let at = ops.len() - 2;
                    ops.insert(at, 3);
                }
                34..=41 => {
                    let n = g.below(20) as usize;
                    ops.extend([4, (g.below(8) == 0) as i32, n as i32]);
                    for _ in 0..n {
                        ops.push(if g.below(2) == 0 { (b'0' + g.below(10) as u8) as i32 } else { g.below(256) as i32 });
                    }
                }
                42..=53 => {
                    let pos = if g.below(4) == 0 { g.below(7) as i32 - 5 } else { g.below(100) as i32 };
                    ops.extend([5, pos, g.below(3) as i32]);
                }
                54..=63 => {
                    if !scan_int_ok {
                        k = 64;
                        continue;
                    }
                    ops.push(6)
                }
                64..=73 => ops.push(7),
                74..=83 => ops.push(8),
                84..=96 => {
                    let isnull = g.below(6) == 0;
                    let size = if isnull { g.below(10) as i32 - 3 } else { 1 + g.below(40) as i32 };
                    ops.extend([9, isnull as i32, size]);
                }
                _ => ops.push(10),
            }
            break;
        }
    }
    ops
}

#[test]
fn local_file_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_10CA1);
    let mut c = Check::new("LocalFile");
    let (mut opened, mut scans) = (0u32, 0u32);
    for case in 0..20_000 {
        let long_digits = g.below(4) == 0;
        let files = [gen_file(&mut g, long_digits), gen_file(&mut g, long_digits), gen_file(&mut g, long_digits)];
        // scanInt is only defined for runs of at most 9 digits (atoi overflow is undefined in C; the digit pieces can also join)
        let ops = gen_ops(&mut g, !long_digits && files.iter().all(|f| max_digit_run(f) <= 9));
        let mut out = vec![0i32; 100_000];
        let len = unsafe {
            ref_lf_local_run(files[0].as_ptr(), files[0].len() as i32, files[1].as_ptr(), files[1].len() as i32, files[2].as_ptr(), files[2].len() as i32, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32)
        };
        assert!(len >= 0, "the log did not fit");
        out.truncate(len as usize);
        let rust = run_rust(&files, &ops);
        if rust.iter().any(|&w| w == 1000) && out.windows(5).any(|w| w[0] == 1 && w[3] >= 3) {
            opened += 1;
        }
        scans += ops.iter().filter(|&&o| (6..=8).contains(&o)).count() as u32 / 4;
        let first = rust.iter().zip(out.iter()).position(|(a, b)| a != b).unwrap_or(rust.len().min(out.len()));
        c.eq(rust.len() == out.len() && first >= rust.len(), true, || {
            format!("case {case}: files {files:?} ops {ops:?}; first difference at word {first}: rust {:?} c++ {:?}", rust.get(first.saturating_sub(6)..(first + 4).min(rust.len())), out.get(first.saturating_sub(6)..(first + 4).min(out.len())))
        });
    }
    println!("(local_file: {opened} sequences opened a file; about {scans} scans)");
    finish(c);
}
