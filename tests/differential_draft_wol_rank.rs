// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol6) of the lobby draft's `lookup_small_rank_image` (with `rank_for_points`,
//! `src/drafts/ui/wol_lobby_menu.rs`) against upstream's `LookupSmallRankImage` and `rankNames[]` (WOLLobbyMenu.cpp), cut verbatim by build.rs
//! (`drafts_wol6`; `differential/wrap_draft_wol_rank.cpp` lists the stand-ins).
//!
//! # Method
//!
//! A case is a side (-3 to 16, with every side the function switches on), rank points (0, negative, on each table boundary, between, past the
//! top ranks), a rank point table (ascending with ties, 14 entries, INT_MAX as the sentinel the unbounded loop stops at) and the set of mapped images that
//! exist (the names `<Rank>-<USA|CHA|GLA|N>`, some missing). Both sides print each image name looked up and the image returned.
//! Excluded: `rankPoints == INT_MAX` (upstream reads past the table).
//!
//! # Predictions (written before the first run)
//!
//! 0 mismatches: the draft is a plain translation of a switch and a loop.
//!
//! # Results: see the DIFF line (the default is 20,000 cases) and the planted table in the commit message.

mod support_wol6;

use rts_lockstep_engine::drafts::ui::wol_lobby_menu as lob;
use support_wol6::lobby_host::LobbyHost;
use support_wol6::Rng;

extern "C" {
    fn ref_wol6_rank_run(input: *const std::os::raw::c_char, out: *mut u8, cap: i32) -> i32;
}

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const RANKS: [&str; 10] = ["Private", "Corporal", "Sergeant", "Lieutenant", "Captain", "Major", "Colonel", "General", "Brigadier", "Commander"];
const SIDES: [&str; 4] = ["USA", "CHA", "GLA", "N"];

struct Case {
    side: i32,
    points: i32,
    images: Vec<String>,
    table: [i32; 14],
}

fn line_of(c: &Case) -> String {
    let t: Vec<String> = c.table.iter().map(|v| v.to_string()).collect();
    format!("{} {} {} {} {}", c.side, c.points, c.images.len(), c.images.join(" "), t.join(" "))
}

fn cpp_run(c: &Case) -> String {
    let input = std::ffi::CString::new(line_of(c)).expect("no NUL");
    let mut out = vec![0u8; 1 << 14];
    let n = unsafe { ref_wol6_rank_run(input.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

fn rust_run(c: &Case) -> String {
    let mut h = LobbyHost::new();
    h.rank_values = c.table;
    h.images = c.images.iter().map(|s| s.as_bytes().to_vec()).collect();
    let r = lob::lookup_small_rank_image(&h, c.side, c.points);
    let mut o = h.log.borrow().clone();
    o.push_str(&format!("img {}\n", r.map_or("none".to_string(), |n| String::from_utf8_lossy(&n).into_owned())));
    o
}

fn gen_case(r: &mut Rng) -> Case {
    let mut table = [0i32; 14];
    let mut v = 0i32;
    for slot in table.iter_mut().take(13).skip(1) {
        v = v.saturating_add(match r.below(6) {
            0 => 0,
            1 => 1,
            _ => 1 + r.below(300) as i32,
        });
        *slot = v;
    }
    table[0] = if r.chance(10) { -5 } else { 0 };
    table[13] = i32::MAX;
    let mut images = Vec::new();
    for rk in RANKS {
        for s in SIDES {
            if !r.chance(5) {
                images.push(format!("{rk}-{s}"));
            }
        }
    }
    let points = match r.below(7) {
        0 => 0,
        1 => -(r.below(50) as i32),
        2 => table[1 + r.below(12) as usize],
        3 => table[1 + r.below(12) as usize] - 1,
        4 => table[1 + r.below(12) as usize] + 1,
        5 => i32::MAX - 1 - r.below(3) as i32,
        _ => r.below(4000) as i32,
    };
    Case { side: r.below(21) as i32 - 3, points, images, table }
}

#[test]
fn the_small_rank_image_lookup_matches_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("DRAFT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x215_0D_0001);
    let (mut mismatches, mut found, mut none) = (0usize, 0usize, 0usize);
    for n in 0..cases {
        let c = gen_case(&mut r);
        let rust = rust_run(&c);
        let cpp = cpp_run(&c);
        found += usize::from(!cpp.ends_with("img none\n"));
        none += usize::from(cpp.ends_with("img none\n") && !cpp.contains("find"));
        if rust != cpp {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {n}: {}\n  rust: {rust:?}\n  c++:  {cpp:?}", line_of(&c));
            }
        }
    }
    println!("DIFF WOL small rank image compared {cases} mismatches {mismatches} ({found} images found, {none} with no lookup)");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
