#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FileTransfer.cpp (the path helpers, GetBasePathFromPath to GetReadmeFromMap)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::file_transfer`'s path helpers against upstream's (#215).
//!
//! # Method
//!
//! `GetBasePathFromPath` .. `GetReadmeFromMap` are cut verbatim (build.rs) into `differential/wrap_draft_ft_paths.cpp`, built on
//! upstream's own AsciiString.cpp (unchanged; classes renamed). 60,000 random strings per function (a small alphabet of `\`, `.`,
//! letters, digits, `%`, space and bytes 0x80..0xFF, in lengths 0 to 40, and some of 100 to 300; no NUL, which ends a C string;
//! some without a `\` or without a `.`, some starting or ending with one) are given to each of the ten functions and the results
//! compared as bytes. Excluded: results of 2,047 characters or more (`AsciiString::format`'s 2,048-byte buffer; longest tested
//! path is 300). Duplicate: the verified `crate::common::game_info::base_path_from_path` is the same function as
//! `get_base_path_from_path` (same behaviour, a second implementation).
//!
//! # Prediction (before the first run, 2026-11-18)
//!
//! 0 mismatches: the functions are plain `rposition` scans.
//!
//! # Results
//!
//! 2026-11-18: 60,000 strings per function, **0 mismatches** in all ten, first run, no draft fix needed (prediction met). Planted bugs, 10 of 10
//! caught (mismatches of 60,000, the worst function named): 12281 28,966 (GetBasePathFromPath, and the six paths built on it); 12282 32,430;
//! 12283 32,430 (GetFileFromPath; 12,489 in GetPreviewFromMap); 12284 16,557; 12285 16,557 (and 24,999 in GetPreviewFromMap);
//! 12286 60,000 (every joined path); 12287 50,339; 12288 60,000; 12289 60,000; 12290 36,382.
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>`: 12281 the first `\` not the last; 12282 the base path keeps the `\`; 12283 the file name starts at the `\`;
//! 12284 no extension gives an empty string (upstream: the whole name); 12285 no `.` gives the whole name (upstream: empty);
//! 12286 `/` in the joined paths; 12287 the preview name keeps its extension; 12288 `map.ini` for the solo INI; 12289 the
//! readme path is `assetusage.txt`; 12290 the extension after the first `.`.

mod support;

use rts_lockstep_engine::drafts::sweep_gui_logic::file_transfer as ft;
use std::ffi::CString;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_ft_path(which: i32, path: *const std::os::raw::c_char, out: *mut std::os::raw::c_char, cap: i32) -> i32;
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

fn path(g: &mut Gen) -> Vec<u8> {
    let n = if g.below(10) == 0 { 100 + g.below(201) } else { g.below(41) } as usize;
    let style = g.below(5);
    (0..n)
        .map(|_| match (style, g.below(10)) {
            (0, _) => b'a' + g.below(26) as u8, // no separators at all
            (1, 0..=2) => b'\\',
            (2, 0..=2) => b'.',
            (_, 0..=1) => b'\\',
            (_, 2..=3) => b'.',
            (_, 4) => [b'%', b' ', b's', b'd'][g.below(4) as usize],
            (_, 5) => 0x80 + g.below(128) as u8,
            (_, 6) => b'0' + g.below(10) as u8,
            _ => b'a' + g.below(26) as u8,
        })
        .collect()
}

type Func = fn(&[u8]) -> Vec<u8>;

const FUNCS: [(&str, Func); 10] = [
    ("GetBasePathFromPath", ft::get_base_path_from_path),
    ("GetFileFromPath", ft::get_file_from_path),
    ("GetExtensionFromFile", ft::get_extension_from_file),
    ("GetBaseFileFromFile", ft::get_base_file_from_file),
    ("GetPreviewFromMap", ft::get_preview_from_map),
    ("GetINIFromMap", ft::get_ini_from_map),
    ("GetStrFileFromMap", ft::get_str_file_from_map),
    ("GetSoloINIFromMap", ft::get_solo_ini_from_map),
    ("GetAssetUsageFromMap", ft::get_asset_usage_from_map),
    ("GetReadmeFromMap", ft::get_readme_from_map),
];

#[test]
fn path_helpers() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_F7A7);
    let mut checks: Vec<Check> = FUNCS.iter().map(|(n, _)| Check::new(n)).collect();
    let mut buf = vec![0 as std::os::raw::c_char; 4096];
    for _ in 0..60_000 {
        let p = path(&mut g);
        let cp = CString::new(p.clone()).expect("no NUL generated");
        for (i, (_, f)) in FUNCS.iter().enumerate() {
            let n = unsafe { ref_ft_path(i as i32, cp.as_ptr(), buf.as_mut_ptr(), buf.len() as i32) };
            assert!(n >= 0, "buffer too small");
            let cpp: Vec<u8> = buf[..n as usize].iter().map(|&c| c as u8).collect();
            checks[i].eq(f(&p), cpp, || format!("path {:?}", String::from_utf8_lossy(&p)));
        }
    }
    let mut missed = 0;
    for c in checks {
        if planted() {
            missed += c.report();
        } else {
            c.finish();
        }
    }
    if planted() {
        let _ = missed;
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
}
