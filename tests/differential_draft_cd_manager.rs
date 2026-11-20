#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/Common/System/CDManager.cpp and List.cpp
//   GeneralsMD/Code/GameEngineDevice/Source/Win32Device/Common/Win32CDManager.cpp
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::logic_common::cd_manager` against `CDManager` / `Win32CDManager` (#215, group lc2).
//!
//! # Method
//!
//! `CDManager.cpp`, `Win32CDManager.cpp`, their headers and the linked list `LList` (List.h, List.cpp) the drives live in are cut verbatim by build.rs
//! into `differential/wrap_draft_cd_manager.cpp` (stand-ins listed there: `GetDriveType`, `GetVolumeInformation`, `unloadMusicFilesFromCD` and
//! the frame number are the test's fakes; the manager is built in zeroed storage, the zero-filled `allocateBytes` the game's `NEW` uses, because
//! `LList::LList` leaves `m_addToEndOfGroup` uninitialised and `LList::add` reads it). A case is a program of up to 25 operations on a fresh
//! manager: `init`, `update(frame)` (frames around 299 mod 300), `refreshDrives`, a drive's volume appearing, changing or vanishing, `getDrive`
//! (negative and past-the-end indices too), `newDrive`, `destroyAllDrives`, `reset`. After each operation: the drive count, the number of music
//! unloads, and the first six drives' path letter, disk-name number and disk. 20,000 programs from a fixed seed.
//!
//! The Windows calls themselves (`GetDriveType`, `GetVolumeInformation`, the file system's unload) are the host's and are not tested; only
//! what the manager does with their answers is.
//!
//! # Prediction (written before the first run, 2026-11-20)
//!
//! One MISS expected: `LList::add` with `m_addToEndOfGroup` FALSE and all priorities 0 inserts the new drive BEFORE the first node, i.e. at the
//! head, so upstream's drive order is the reverse of detection order; the draft's `new_drive` pushes at the tail. Everything else: 0 mismatches.
//!
//! # Results
//!
//! 2026-11-20: **1 MISS, as predicted, then 0 mismatches.** First run: 13,194 of 20,000 programs differed (one cause: `new_drive` appended where `LList::add` inserts at the head). Fix: the draft's
//! `new_drive` inserts at index 0 (comment struck through and dated in the draft). After the fix: **0 mismatches over 20,001 comparisons** (20,000 programs and the base `CDDrive`). Planted bugs, 11 of 11 caught:
//! 16921 a negative `getDrive` index wraps 1,276; 16922 refresh at frame % 300 == 0 2,857; 16923 refresh at frame % 100 == 99 1,424 (MISSED at first, 0: the generator's frames 299, 599 and 899 satisfy both rules;
//! frames 99, 199, 399, 100 and 29999 were added to the generator, not the plant dropped); 16924 unload even when the drive had no disc 6,997; 16925 the disk name kept on failure 60; 16926 a readable disc
//! reported `NO_DISK` 13,992; 16927 letters `a`..`y` only 1,484; 16928 `init` keeps the old drives 11,580; 16929 `init` does not refresh 13,654; 16930 `destroyAllDrives` does nothing 14,144;
//! 16931 the base `refreshInfo` gives `NO_DISK` 1.
//! UPSTREAM NOTE: `LList::LList` leaves `m_addToEndOfGroup` uninitialised (List.cpp); the drive order depends on the allocator zero-filling (it does: GameMemory.cpp `allocateBytes`).

mod support;

use rts_lockstep_engine::drafts::logic_common::cd_manager::{CdDisk, CdDrive, CdHost, CdManager};
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_lc2_cd_run(prog: *const i32, nprog: i32, out: *mut i32) -> i32;
    fn ref_lc2_cd_base(out: *mut i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Fake {
    mask: u32,
    vol: [i32; 26],
    unloads: i32,
}

impl CdHost for Fake {
    fn is_cdrom(&mut self, drive_path: &[u8]) -> bool {
        (self.mask >> (drive_path[0] - b'a')) & 1 != 0
    }
    fn volume_name(&mut self, drive_path: &[u8]) -> Option<Vec<u8>> {
        let l = (drive_path[0] - b'a') as usize;
        if self.vol[l] == 0 {
            None
        } else {
            Some(format!("VOL{}", self.vol[l]).into_bytes())
        }
    }
    fn unload_music_files_from_cd(&mut self) {
        self.unloads += 1;
    }
}

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

fn disk_code(d: CdDisk) -> i32 {
    match d {
        CdDisk::UnknownDisk => -3,
        CdDisk::NoDisk => -2,
    }
}

fn name_number(n: &[u8]) -> i32 {
    if n.is_empty() {
        0
    } else {
        (n[3] - b'0') as i32
    }
}

#[test]
fn cd_manager_matches() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_CD);
    let mut c = Check::new("CDManager");
    for case in 0..20_000 {
        let mut mask = 0u32;
        for _ in 0..g.below(6) {
            mask |= 1 << g.below(26);
        }
        let vol: Vec<i32> = (0..26).map(|_| if g.below(3) == 0 { 0 } else { 1 + g.below(4) as i32 }).collect();
        let mut prog: Vec<i32> = vec![mask as i32];
        prog.extend(&vol);
        let mut ops: Vec<[i32; 3]> = Vec::new();
        for _ in 0..1 + g.below(25) {
            ops.push(match g.below(11) {
                0 | 1 => [0, 0, 0],
                2 | 3 => [1, [299, 298, 300, 599, 0, 1, 899, 99, 199, 399, 100, 29999][g.below(12) as usize], 0],
                4 | 5 => [2, 0, 0],
                6 => [3, g.below(26) as i32, g.below(5) as i32],
                7 => [4, g.below(11) as i32 - 2, 0],
                8 => [5, g.below(26) as i32, 0],
                9 => [6, 0, 0],
                _ => [7, 0, 0],
            });
        }
        for o in &ops {
            prog.extend(o);
        }
        let mut cpp = vec![0i32; 4000];
        let len = unsafe { ref_lc2_cd_run(prog.as_ptr(), prog.len() as i32, cpp.as_mut_ptr()) } as usize;
        cpp.truncate(len);

        let mut host = Fake { mask, vol: [0; 26], unloads: 0 };
        host.vol.copy_from_slice(&vol);
        let mut m = CdManager::new();
        let mut rust: Vec<i32> = Vec::new();
        for &[kind, a, b] in &ops {
            let mut ans: Option<[i32; 4]> = None;
            match kind {
                0 => m.init(&mut host),
                1 => m.update(&mut host, a as u32),
                2 => m.refresh_drives(&mut host),
                3 => host.vol[a as usize] = b,
                4 => {
                    ans = Some(match m.get_drive(a) {
                        Some(d) => [1, d.get_path()[0] as i32, name_number(d.get_disk_name()), disk_code(d.get_disk())],
                        None => [0, 0, 0, 0],
                    })
                }
                5 => {
                    m.new_drive(&[b'a' + a as u8, b':', b'\\']);
                }
                6 => m.destroy_all_drives(),
                _ => m.reset(),
            }
            rust.push(m.drive_count());
            rust.push(host.unloads);
            for i in 0..6 {
                match m.get_drive(i) {
                    Some(d) => rust.extend([d.get_path()[0] as i32, name_number(d.get_disk_name()), disk_code(d.get_disk())]),
                    None => rust.extend([0, 0, 0]),
                }
            }
            if let Some(a) = ans {
                rust.extend(a);
            }
        }
        c.eq(rust, cpp, || format!("case {case}: mask {mask:#x}, volumes {vol:?}, ops {ops:?}"));
    }
    // the base drive alone
    {
        let mut cpp = [0i32; 6];
        assert_eq!(unsafe { ref_lc2_cd_base(cpp.as_mut_ptr()) }, 6);
        let mut d = CdDrive::new();
        let before = [disk_code(d.get_disk()), d.get_path().len() as i32, d.get_disk_name().len() as i32];
        d.set_path(b"q:\\");
        d.refresh_info();
        let rust = [before[0], before[1], before[2], disk_code(d.get_disk()), d.get_path()[0] as i32, d.get_path().len() as i32];
        c.eq(rust.to_vec(), cpp.to_vec(), || "the base CDDrive".to_string());
    }
    finish(c);
}
