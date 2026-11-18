#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/LanguageFilter.cpp (and Include/GameClient/LanguageFilter.h)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::language_filter::LanguageFilter` against `LanguageFilter` (#215).
//!
//! # Method
//!
//! LanguageFilter.cpp from `LanguageFilter *TheLanguageFilter = NULL;` to the end is cut verbatim (build.rs) into
//! `differential/wrap_draft_language_filter.cpp`, with the header read unchanged (`protected` read as `public`), upstream's own
//! UnicodeString.cpp and AsciiString.cpp unchanged (-fshort-wchar), and stand-ins for `File` (over memory, with RAMFile's partial
//! `read`), `TheFileSystem->openFile` and `SubsystemInterface`; the C runtime's `wcsstr`, `wcsrchr` and MSVC's `WEOF` (16-bit, in
//! `differential/shim_draft_text/text_prelude.h`). Three parts, each from a fixed seed:
//! 1. `unHaxor`: 200,000 words from an alphabet of leet characters, `p`/`P`/`h`/`H`, the ignored characters, letters of both
//!    cases and non-ASCII units; the result is compared.
//! 2. `readWord`: 100,000 byte blocks (even and odd lengths; units of space, `0xFFFF` (WEOF), 0, letters, any) read word after word
//!    to the end: each word (to its NUL), the returned Bool and the file position are compared.
//! 3. `init` and `filterLine`: 4,000 word files (XOR'd by the key, space-separated, with empty words, some files absent) and 20 lines
//!    each (bad words in leet, case and with ignored characters, repeated, between delimiters and non-ASCII units): the number of
//!    words, the words in map order (the draft keeps the case-folded key, so the C++ keys are folded the same way for the comparison;
//!    the spelling of the first insertion is not observable through the draft) and each filtered line.
//!
//! Excluded inputs: a `readWord` whose first read has ONE byte left (the high byte of an uninitialised `c`); a file that starts with a
//! separator (`init`'s `word[128]` is uninitialised then); words of 128 units or
//! more (a buffer overrun upstream); units of 0 in a line (the C string ends there).
//!
//! # Predictions (written before the first run, 2026-11-18)
//!
//! Misses predicted (the third found by the first run, see below): (1) the draft keys the map by Rust's Unicode lower-casing; upstream's `_wcsicmp` folds ASCII only (C locale),
//! so words differing only by a non-ASCII case (`E9`/`C9`) differ. (2) The draft's `read_word` takes a trailing odd byte as the end of
//! the file; RAMFile/LocalFile return the one byte, so upstream uses it as the low byte of `c` (high byte kept). Everything else: 0.
//!
//! # Results
//!
//! 2026-11-18: **0 mismatches** after two draft fixes. `unHaxor` 200,000 words; `readWord` 272,539 words (Bool, position and text each); `init` 4,000 files
//! (word count and words) and `filterLine` 80,000 lines. First run: 100 of 4,000 files differed in their word count, and the draft's `read_word` and key fold
//! were not yet exercised. **Prediction misses:** (1) not predicted: upstream's `init` reuses one `word[128]` and XORs it IN PLACE; `readWord` writes no
//! terminator when a word's first unit is the separator (two spaces in a row), so such an "empty word" is that space plus the previous word's leftover
//! units, XOR'd a second time, and is inserted as a word (the draft took it as empty and skipped it). The draft now keeps the buffer and XORs in place.
//! (2) the key fold: Rust's Unicode lower-casing replaced by the C locale's ASCII-only fold (`_wcsicmp` of the shim; the verified `UnicodeString` does the same);
//! this one was found by reading and fixed before the first run. (3) the odd trailing byte: `read` returns the one byte and `c` keeps its high byte; the draft
//! took it as the end of the file; fixed before the first run, and now covered by the 3,396 + other odd blocks (1-byte-first reads excluded).
//! Planted bugs, 13 of 13 caught (the first 11485 was an equivalent mutant until the generator made raw 0 units): 11477 unHaxor 2,692 of 200,000;
//! 11478 16,147; 11479 16,200; 11480 33,287; 11481 1,151; 11482 filterLine 12,986 of 80,000; 11483 3,963; 11484 words 2,820 of 4,000 and filterLine 10,660;
//! 11485 words 692 of 4,000; 11486 3,356 of 4,000; 11487 readWord 30,432 of 272,539; 11488 Bool 89,854; 11489 filterLine 1,816.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 11477 `ph` does not skip the `h`; 11478 `@` is kept; 11479 `+` is kept; 11480 `"` is not ignored; 11481 `P` not
//! handled; 11482 the length measured after un-haxoring; 11483 `wcsstr` over the original line instead of the edited copy; 11484 a
//! case-sensitive key; 11485 empty words kept; 11486 XOR by 0x55 only; 11487 a data unit 0xFFFF is not WEOF; 11488 a word ended by the
//! file's end returns TRUE; 11489 `~` is not a delimiter.

mod support;

use rts_lockstep_engine::drafts::logic_common::language_filter::{BadWordFileHost, LanguageFilter};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lf_unhaxor(input: *const u16, n: i32, out: *mut u16) -> i32;
    fn ref_lf_read_word(data: *const u8, len: i32, pos: i32, out: *mut u16, newpos: *mut i32) -> i32;
    fn ref_lf_init(data: *const u8, len: i32, has_file: i32);
    fn ref_lf_count() -> i32;
    fn ref_lf_word(i: i32, out: *mut u16) -> i32;
    fn ref_lf_filter(input: *const u16, n: i32, out: *mut u16) -> i32;
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

const LEET: &str = "13456 70@$+";

fn letter(g: &mut Gen) -> u16 {
    match g.below(14) {
        0..=3 => (b'a' + g.below(26) as u8) as u16,
        4 => (b'A' + g.below(26) as u8) as u16,
        5 => [b'p', b'P', b'h', b'H'][g.below(4) as usize] as u16,
        6 | 7 => LEET.as_bytes()[g.below(LEET.len() as u32) as usize] as u16,
        8 | 9 => [b'-', b'_', b'*', b'\'', b'"'][g.below(5) as usize] as u16,
        10 => [0xE9u16, 0xC9, 0x00E0, 0x00C0, 0x2000, 0x0130][g.below(6) as usize],
        11 => g.below(0x10000) as u16 | 1,
        _ => (b'0' + g.below(10) as u8) as u16,
    }
}

#[test]
fn un_haxor_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1F01);
    let mut c = Check::new("LanguageFilter::unHaxor");
    for _ in 0..200_000 {
        let n = g.below(14) as usize;
        let w: Vec<u16> = (0..n).map(|_| letter(&mut g)).collect();
        let mut out = [0u16; 64];
        let len = unsafe { ref_lf_unhaxor(w.as_ptr(), w.len() as i32, out.as_mut_ptr()) } as usize;
        let mut r = w.clone();
        LanguageFilter::un_haxor(&mut r);
        c.eq(r, out[..len].to_vec(), || format!("{w:04x?}"));
    }
    finish(c);
}

fn unit_of_file(g: &mut Gen, run: &mut u32) -> u16 {
    let u = match g.below(100) {
        0..=11 => 0x20,
        12..=14 => 0xFFFF,
        15..=17 => 0,
        18..=60 => 0x61 + g.below(26) as u16,
        _ => g.below(0x10000) as u16,
    };
    if *run >= 100 {
        *run = 0;
        return 0x20;
    }
    if u == 0x20 {
        *run = 0;
    } else {
        *run += 1;
    }
    u
}

#[test]
fn read_word_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1F02);
    let mut cw = Check::new("LanguageFilter::readWord (word)");
    let mut cb = Check::new("LanguageFilter::readWord (Bool)");
    let mut cp = Check::new("LanguageFilter::readWord (position)");
    let mut skipped = 0u32;
    for _ in 0..100_000 {
        let units = g.below(40) as usize;
        let mut run = 0u32;
        let mut data: Vec<u8> = Vec::new();
        for _ in 0..units {
            data.extend(unit_of_file(&mut g, &mut run).to_le_bytes());
        }
        if g.below(3) == 0 {
            data.push(g.below(256) as u8); // an odd length
        }
        let (mut rp, mut cpp_pos) = (0usize, 0i32);
        loop {
            if data.len() - rp == 1 {
                skipped += 1;
                break;
            }
            let mut cbuf = [0u16; 128];
            let mut np = 0i32;
            let cb_ret = unsafe { ref_lf_read_word(data.as_ptr(), data.len() as i32, cpp_pos, cbuf.as_mut_ptr(), &mut np) } != 0;
            cpp_pos = np;
            let mut rbuf = [0u16; 128];
            let r_ret = LanguageFilter::read_word(&data, &mut rp, &mut rbuf);
            let end = |b: &[u16; 128]| b.iter().position(|&x| x == 0).unwrap_or(128);
            cw.eq(rbuf[..end(&rbuf)].to_vec(), cbuf[..end(&cbuf)].to_vec(), || format!("{data:02x?} at {rp}"));
            cb.eq(r_ret, cb_ret, || format!("{data:02x?} at {rp}"));
            cp.eq(rp as i32, cpp_pos, || format!("{data:02x?}"));
            if !cb_ret || rp != cpp_pos as usize {
                break;
            }
        }
    }
    println!("(readWord: {skipped} blocks stopped where the first read would have had one byte left)");
    if planted() {
        cb.report();
        cp.report();
        finish(cw);
    } else {
        cb.finish();
        cp.finish();
        cw.finish();
    }
}

struct Host(Option<Vec<u8>>);

impl BadWordFileHost for Host {
    fn open_bad_word_file(&mut self) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

const VOCAB: [&str; 14] = ["fuck", "ph", "shit", "hell", "ass", "bitch", "damn", "Pass", "test", "ok", "Phat", "crap", "x", "stest"];

fn word_form(g: &mut Gen, w: &str) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    let b = w.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let ch = b[i];
        let mut u = ch as u16;
        match g.below(8) {
            0 => u = ch.to_ascii_uppercase() as u16,
            1 => match ch {
                b'l' => u = b'1' as u16,
                b'e' => u = b'3' as u16,
                b'a' => u = if g.below(2) == 0 { b'4' as u16 } else { b'@' as u16 },
                b's' => u = if g.below(2) == 0 { b'5' as u16 } else { b'$' as u16 },
                b't' => u = if g.below(2) == 0 { b'7' as u16 } else { b'+' as u16 },
                b'o' => u = b'0' as u16,
                b'b' => u = b'6' as u16,
                _ => {}
            },
            2 => out.push([b'-', b'_', b'*', b'\'', b'"'][g.below(5) as usize] as u16),
            3 if ch == b'f' && i + 1 < b.len() => {}
            _ => {}
        }
        out.push(u);
        i += 1;
    }
    if g.below(10) == 0 {
        out.push([0xE9u16, 0xC9][g.below(2) as usize]);
    }
    out
}

fn file_of(g: &mut Gen) -> Vec<u8> {
    let mut f: Vec<u8> = Vec::new();
    let words = g.below(13);
    for index in 0..words {
        let w: Vec<u16> = match if index == 0 { 3 + g.below(9) } else { g.below(12) } {
            0 => Vec::new(), // an empty word (two spaces in a row; not first: that reads an uninitialised buffer upstream)
            1 => vec![b'-' as u16, b'_' as u16], // un-haxors to nothing
            2 if index > 0 => vec![0x5555], // a raw 0 unit: the buffer's first unit is NUL, so `init` skips it
            _ => {
                let w = VOCAB[g.below(VOCAB.len() as u32) as usize];
                word_form(g, w)
            }
        };
        for u in w {
            f.extend((u ^ 0x5555).to_le_bytes());
        }
        f.extend(0x20u16.to_le_bytes());
    }
    if g.below(3) == 0 && f.len() >= 2 {
        f.truncate(f.len() - 2); // the last word ends at the end of the file
    }
    f
}

fn line_of(g: &mut Gen) -> Vec<u16> {
    const SEP: &str = " ;,.!?:=\\/><`~()&^%#\n\t";
    let mut line: Vec<u16> = Vec::new();
    let tokens = g.below(7);
    for _ in 0..tokens {
        if g.below(8) == 0 {
            line.extend((0..1 + g.below(3)).map(|_| letter(g)));
        } else if g.below(10) == 0 {
            line.extend([b'-' as u16, b'-' as u16]);
        } else {
            let w = VOCAB[g.below(VOCAB.len() as u32) as usize];
            line.extend(word_form(g, w));
        }
        for _ in 0..g.below(3) {
            line.push(SEP.as_bytes()[g.below(SEP.len() as u32) as usize] as u16);
        }
    }
    if g.below(6) == 0 && !line.is_empty() {
        // a repeated token
        let again = line.clone();
        line.push(0x20);
        line.extend(again);
    }
    line
}

#[test]
fn init_and_filter_line_match() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_1F03);
    let mut cn = Check::new("LanguageFilter::init (word count)");
    let mut cwd = Check::new("LanguageFilter::init (words)");
    let mut cl = Check::new("LanguageFilter::filterLine");
    let fold = |u: u16| if (b'A' as u16..=b'Z' as u16).contains(&u) { u + 32 } else { u };
    for _ in 0..4_000 {
        let has = g.below(15) != 0;
        let data = if has { file_of(&mut g) } else { Vec::new() };
        unsafe { ref_lf_init(data.as_ptr(), data.len() as i32, has as i32) };
        let mut f = LanguageFilter::new();
        // the list must be cleared by init: put something in first
        f.word_list.insert(vec![1, 2, 3], true);
        f.init(&mut Host(if has { Some(data.clone()) } else { None }));
        let n = unsafe { ref_lf_count() } as usize;
        let mut cpp_words: Vec<Vec<u16>> = Vec::new();
        for i in 0..n {
            let mut out = [0u16; 256];
            let len = unsafe { ref_lf_word(i as i32, out.as_mut_ptr()) } as usize;
            cpp_words.push(out[..len].iter().map(|&u| fold(u)).collect());
        }
        let rust_words: Vec<Vec<u16>> = f.word_list.keys().cloned().collect();
        let show = |w: &Vec<Vec<u16>>| -> String { w.iter().map(|x| String::from_utf16_lossy(x)).collect::<Vec<_>>().join("|") };
        cn.eq(f.word_list.len(), n, || format!("file {data:02x?}: rust [{}] c++ [{}]", show(&rust_words), show(&cpp_words)));
        cwd.eq(rust_words, cpp_words, || format!("{data:02x?}"));
        for _ in 0..20 {
            let line = line_of(&mut g);
            let mut out = [0u16; 512];
            let len = unsafe { ref_lf_filter(line.as_ptr(), line.len() as i32, out.as_mut_ptr()) } as usize;
            let mut r = line.clone();
            f.filter_line(&mut r);
            cl.eq(r, out[..len].to_vec(), || format!("line {line:04x?} file {data:02x?}"));
        }
    }
    if planted() {
        cn.report();
        cwd.report();
        finish(cl);
    } else {
        cn.finish();
        cwd.finish();
        cl.finish();
    }
}
