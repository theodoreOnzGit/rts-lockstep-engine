#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/SaveGame/GameState.cpp (doesSaveGameExist) and Common/System/XferLoad.cpp (open, close)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_common_client::game_state::does_save_game_exist` against `GameState::doesSaveGameExist` (#215, group lc3).
//!
//! # Method
//!
//! `doesSaveGameExist`, `getSaveDirectory`, `getFilePathInSaveDirectory` and `XferLoad::open` / `close` are cut verbatim by build.rs into
//! `differential/wrap_draft_game_state.cpp` (stand-ins listed there: `fopen` is the case's set of files, `TheGlobalData`'s user data path is the case's string).
//! The draft's host answers "does the file open" from the same set. A case: a user data path (empty, with and without a trailing backslash, long, with spaces), a leaf (a name,
//! empty, with a path separator, upper and lower case), and a set of up to 6 existing files made from the true path, near misses (the leaf alone, the path in another case, the
//! directory without `Save\`, the leaf with a changed byte) and unrelated names. 20,000 cases from a fixed seed. Excluded: names with an embedded NUL (upstream's AsciiString ends there).
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! 0 mismatches (three lines over functions the verified `GameState` already tests).
//!
//! # Results
//!
//! 2026-11-20: **0 mismatches over 20,000 cases** (prediction held). Planted bugs, 4 of 4 caught (of 20,000 cases): 19941 20,000; 19942 9,565; 19943 5,457; 19944 9,677.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 19941 the answer is inverted; 19942 the path is the leaf alone; 19943 the leaf is lowercased; 19944 the path is built with the user data path and no `Save\`.

mod support;

use rts_lockstep_engine::common::game_state::{GameState, GameStateHost};
use rts_lockstep_engine::common::xfer::{Xfer, XferResult};
use rts_lockstep_engine::drafts::sweep_common_client::game_state::does_save_game_exist;
use std::collections::BTreeSet;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc3_gs_exists(blob: *const std::os::raw::c_char, n: i32) -> i32;
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

struct Host {
    user: Vec<u8>,
    files: BTreeSet<Vec<u8>>,
}

impl GameStateHost<u32> for Host {
    fn xfer_block(&mut self, _block: u32, _gs: &mut GameState<u32>, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    fn load_post_process(&mut self, _block: u32) {}
    fn partition_update(&mut self) {}
    fn user_data_path(&self) -> Vec<u8> {
        self.user.clone()
    }
    fn map_name(&self) -> Vec<u8> {
        Vec::new()
    }
    fn local_time(&mut self) -> [u16; 8] {
        [0; 8]
    }
    fn world_dict_map_name(&self) -> Option<Vec<u8>> {
        None
    }
    fn current_campaign(&self) -> Option<(Vec<u8>, i32)> {
        None
    }
    fn file_exists(&self, path: &[u8]) -> bool {
        self.files.contains(path)
    }
}

#[test]
fn game_state_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_65A);
    let mut c = Check::new("GameState::doesSaveGameExist");
    let users: [&[u8]; 6] = [b"", b"C:\\Users\\Me\\Generals\\", b"data\\", b"D:\\Games Folder\\Zero Hour Data\\", b"x", b"\\\\server\\share\\a_very_long_name_to_pass_the_short_buffers_of_a_formatter\\"];
    let leaves: [&[u8]; 7] = [b"00000000.sav", b"", b"Save1.sav", b"SAVE1.SAV", b"sub\\a.sav", b"a b.sav", b"Mission_0001.sav"];
    for case in 0..20_000 {
        let user = users[g.below(6) as usize];
        let leaf = leaves[g.below(7) as usize];
        let mut right = user.to_vec();
        right.extend_from_slice(b"Save\\");
        right.extend_from_slice(leaf);
        let mut set: Vec<Vec<u8>> = Vec::new();
        for _ in 0..g.below(7) {
            let mut v = match g.below(8) {
                0 | 1 | 2 => right.clone(),
                3 => leaf.to_vec(),
                4 => right.to_ascii_uppercase(),
                5 => {
                    let mut v = user.to_vec();
                    v.extend_from_slice(leaf);
                    v
                }
                6 => {
                    let mut v = right.clone();
                    if !v.is_empty() {
                        let i = g.below(v.len() as u32) as usize;
                        v[i] = v[i].wrapping_add(1);
                    }
                    v
                }
                _ => format!("other{}.sav", g.below(5)).into_bytes(),
            };
            v.retain(|&b| b != 0);
            set.push(v);
        }
        let mut blob: Vec<u8> = Vec::new();
        blob.extend_from_slice(user);
        blob.push(0);
        blob.extend_from_slice(leaf);
        blob.push(0);
        blob.extend_from_slice(set.len().to_string().as_bytes());
        blob.push(0);
        for f in &set {
            blob.extend_from_slice(f);
            blob.push(0);
        }
        let cpp = unsafe { ref_lc3_gs_exists(blob.as_ptr() as *const _, blob.len() as i32) } != 0;
        let host = Host { user: user.to_vec(), files: set.iter().cloned().collect() };
        let gs = GameState::<u32>::default();
        let rust = does_save_game_exist(&gs, &host, leaf);
        c.eq(rust, cpp, || format!("case {case}: user {:?} leaf {:?} files {:?}", String::from_utf8_lossy(user), String::from_utf8_lossy(leaf), set.iter().map(|f| String::from_utf8_lossy(f).into_owned()).collect::<Vec<_>>()));
    }
    finish(c);
}
