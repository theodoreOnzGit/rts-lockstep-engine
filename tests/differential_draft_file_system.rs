// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui2) of the draft `src/drafts/sweep_common_client/file_system.rs`
//! (`load_music_files_from_cd`, `copy_data_to_file`) against upstream `FileSystem.cpp`'s `FileSystem::loadMusicFilesFromCD`
//! and `RAMFile.cpp`'s `RAMFile::copyDataToFile`, cut verbatim by build.rs (`drafts_ui2`;
//! `differential/wrap_draft_file_system.cpp` lists the stand-ins: the CD manager, the archive file system and the file are
//! scripted and log what they are asked, so no OS call is made).
//!
//! # Method
//!
//! - `load_music_files_from_cd`, 10,000 cases (fixed seed, `DRAFT_CASES` overrides): with or without a CD manager, 0 to 5 drives (each
//!   NULL or a path, and what `loadBigFilesFromDirectory` answers for it); compared: the calls made (the directory and the
//!   archive name passed, in order), so the walk's stop at the first success, the skip of a NULL drive and `MUSIC_BIG` are all
//!   checked.
//! - `copy_data_to_file`, 10,000 cases: a missing destination or one that answers the count written (exact, one short, one
//!   past, 0, -1, random) for 0 to 40 bytes; compared: the result and the bytes the destination was given.
//!   **Mapping (stated):** the Rust closure returns `usize`, so the C++ answer -1 is `usize::MAX`; both sides then differ from the size.
//!
//! # Predictions (before the first run, 2026-11-19)
//!
//! 0 mismatches. Plants 15487 to 15492 all caught; 15488 (a NULL drive stops the walk) and 15492 (a missing destination
//! reports success) the fewest.
//!
//! # Planted bugs
//!
//! | id | planted bug |
//! |---|---|
//! | 15487 | the last drive is never tried |
//! | 15488 | a NULL drive ends the walk instead of being skipped |
//! | 15489 | the walk does not stop at the first archive that loads |
//! | 15490 | `copyDataToFile` accepts a write of more than the size |
//! | 15491 | `copyDataToFile` accepts any non-zero write |
//! | 15492 | a missing destination reports success |
//!
//! # Results (2026-11-19)
//!
//! 10,000 cases each: **0 mismatches** on the first run for both functions. Planted bugs (cases of 10,000): 15487 3,073; 15488 1,785; 15489 2,269 (music walk); 15490 4,057; 15491 6,092; 15492 1,679 (copy). **6 of 6 caught**; 15488 and 15492 were the two predicted fewest, and 15492 (1,679) was the fewest of the copy plants, 15488 of the walk plants.
//! Mismatch before fixes: none to fix; no draft changed behaviour. The `DIFF` line is printed by the run.

mod support;

use rts_lockstep_engine::drafts::sweep_common_client::file_system::*;
use support::{Check, Gen};

extern "C" {
    fn ref_u2_fs_music(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_u2_fs_copy(input: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(10000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} (cases)", c.name, c.compared, c.mismatches);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        assert!(bad > 0, "plant not caught");
    }
}

struct Host {
    has: bool,
    drives: Vec<(bool, i32, bool)>, // (null, path id, answer)
    next_answer: usize,
    calls: Vec<i32>,
    n_calls: i32,
}

impl CdHost for Host {
    fn has_cd_manager(&self) -> bool {
        self.has
    }
    fn drive_count(&self) -> i32 {
        self.drives.len() as i32
    }
    fn drive_path(&self, i: i32) -> Option<Vec<u8>> {
        let (null, id, _) = self.drives[i as usize];
        if null { None } else { Some(format!("p{id}").into_bytes()) }
    }
    fn load_big_files_from_directory(&mut self, root: &[u8], music_big: &str) -> bool {
        let id: i32 = std::str::from_utf8(&root[1..]).unwrap().parse().unwrap();
        self.calls.push(id);
        self.calls.push(music_big.len() as i32);
        self.calls.extend(music_big.bytes().map(|b| b as i32));
        self.n_calls += 1;
        // the answer of the drive that was asked: the next non-NULL drive's, in order
        let mut seen = 0;
        for &(null, _, ans) in &self.drives {
            if !null {
                if seen == self.next_answer {
                    self.next_answer += 1;
                    return ans;
                }
                seen += 1;
            }
        }
        false
    }
}

#[test]
fn file_system_drafts_match_upstream() {
    let mut g = Gen::new(0x215_2_06);
    let mut chk_m = Check::new("FileSystem::loadMusicFilesFromCD");
    let mut chk_c = Check::new("RAMFile::copyDataToFile");
    for _ in 0..cases() {
        let has = g.below(5) != 0;
        let nd = g.below(6) as usize;
        let drives: Vec<(bool, i32, bool)> = (0..nd).map(|_| (g.below(4) == 0, g.below(100) as i32, g.below(3) == 0)).collect();
        let mut input = vec![has as i32, nd as i32];
        for &(null, id, ans) in &drives {
            input.extend([null as i32, id, ans as i32]);
        }
        let mut h = Host { has, drives, next_answer: 0, calls: Vec::new(), n_calls: 0 };
        load_music_files_from_cd(&mut h);
        let mut rust = vec![h.n_calls];
        rust.extend(h.calls.iter().copied());
        let mut out = vec![0i32; 256];
        let k = unsafe { ref_u2_fs_music(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), 256) } as usize;
        out.truncate(k);
        chk_m.eq(rust, out, || format!("{input:?}"));

        let has_file = g.below(6) != 0;
        let size = if g.below(5) == 0 { 0 } else { g.below(41) as i32 };
        let ans = match g.below(6) {
            0 => size,
            1 => size - 1,
            2 => size + 1,
            3 => 0,
            4 => -1,
            _ => g.below(60) as i32,
        };
        let data: Vec<u8> = (0..size).map(|_| g.below(256) as u8).collect();
        let mut input = vec![has_file as i32, size, ans];
        input.extend(data.iter().map(|&b| b as i8 as i32));
        let mut written: Vec<u8> = Vec::new();
        let mut f = |d: &[u8]| -> usize {
            written = d.to_vec();
            if ans < 0 { usize::MAX } else { ans as usize }
        };
        let res = if has_file { copy_data_to_file(&data, Some(&mut f)) } else { copy_data_to_file(&data, None) };
        let mut rust = vec![res as i32, written.len() as i32];
        rust.extend(written.iter().map(|&b| b as i32));
        let mut out = vec![0i32; 128];
        let k = unsafe { ref_u2_fs_copy(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), 128) } as usize;
        out.truncate(k);
        chk_c.eq(rust, out, || format!("{input:?}"));
    }
    finish(vec![chk_m, chk_c]);
}
