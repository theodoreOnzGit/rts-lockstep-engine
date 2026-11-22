// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group wol6) of the slash commands of the WOL lobby and game setup drafts: `handle_lobby_slash_commands`
//! (`src/drafts/ui/wol_lobby_menu.rs`, with `refresh_game_list` and `refresh_player_list` which `/refresh` runs) and
//! `handle_game_setup_slash_commands` (`wol_game_setup_menu.rs`) against upstream's `handleLobbySlashCommands`, `refreshGameList`,
//! `refreshPlayerList` (WOLLobbyMenu.cpp) and `handleGameSetupSlashCommands` (WOLGameSetupMenu.cpp), cut verbatim by build.rs (`drafts_wol6`;
//! `differential/wrap_draft_wol_slash.cpp` lists the cuts and every stand-in, `differential/wol6_slash_host.h` the host).
//!
//! # Method
//!
//! A case is one chat text (UTF-16 units: the slash commands with the separators, case, trailing text and wide characters, and plain chat),
//! which menu handles it, the two integers `/host` prints, the default colour, whether the lobby's player list exists, whether the staging room
//! list changed, the refresh statics and the clock. Both sides run the function once and print every call outside it (`addText`, `sendChat`,
//! `hasStagingRoomListChanged`, `RefreshGameListBoxes`), the result, and the refresh statics; the two texts are compared.
//!
//! Excluded inputs (upstream is undefined there): a text with an embedded NUL unit, a first token over 2,047 characters, a clock past 2^31 (`time_t`
//! is 64 bits in the C++ here and the draft's statics are 32 bits). Not covered: `PopulateLobbyPlayerListbox` (the C++ stand-in is empty; the
//! draft's returns at once with no list, which `/refresh` cases guarantee), and the Rust-side effect of a missing `listboxLobbyPlayers` on `/me`.
//!
//! # Predictions (written before the first run)
//!
//! The draft keeps every unit of the text as a byte when it makes the ASCII message (`c as u8`), where upstream's `AsciiString::translate` appends
//! each `(char)unit` with `concat(const char c)`, which adds nothing for a zero byte. So a unit whose low byte is 0 (U+0100, U+0200, ...) inside
//! the text is dropped upstream and kept (as a NUL) by the draft. Expected: mismatches only on such texts (`/h\u{100}ost`); all else 0.
//!
//! # Planted bugs (`L9_PLANT=<id>`, the draft's `plant(id)`)
//!
//! See the table below the results.

mod support_wol6;

use rts_lockstep_engine::client::window_manager::GameWindowManager;
use rts_lockstep_engine::drafts::ui::wol_game_setup_menu as gs;
use rts_lockstep_engine::drafts::ui::wol_lobby_menu as lob;
use support_wol6::game_setup_host::GameSetupHost;
use support_wol6::lobby_host::LobbyHost;
use support_wol6::{hex, Rng};

extern "C" {
    fn ref_wol6_slash_run(input: *const std::os::raw::c_char, out: *mut u8, cap: i32) -> i32;
}

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug)]
struct Case {
    menu: i32,
    qr2: i32,
    thread: i32,
    color: i32,
    players: bool,
    changed: bool,
    gtime: u32,
    ptime: u32,
    now: u32,
    text: Vec<u16>,
}

fn line_of(c: &Case) -> String {
    format!("{} {} {} {} {} {} {} {} {} {}", c.menu, c.qr2, c.thread, c.color, i32::from(c.players), i32::from(c.changed), c.gtime, c.ptime, c.now, hex(&c.text))
}

fn cpp_run(c: &Case) -> String {
    let input = std::ffi::CString::new(line_of(c)).expect("no NUL");
    let mut out = vec![0u8; 1 << 16];
    let n = unsafe { ref_wol6_slash_run(input.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0, "the reference refused the case {c:?}");
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

fn rust_run(c: &Case) -> String {
    let mut wm = GameWindowManager::new((800, 600));
    if c.menu == 0 {
        let mut h = LobbyHost::new();
        h.color_default = c.color;
        h.qr2 = c.qr2;
        h.thread = c.thread;
        h.changed = c.changed;
        h.now = c.now;
        h.menu.listbox_lobby_players = if c.players { Some(7) } else { None };
        h.menu.game_list_refresh_time = c.gtime;
        h.menu.player_list_refresh_time = c.ptime;
        let r = lob::handle_lobby_slash_commands(&mut wm, &mut h, &c.text);
        let mut o = h.log.borrow().clone();
        o.push_str(&format!("ret {}\nstatics {} {}\n", i32::from(r), h.menu.game_list_refresh_time, h.menu.player_list_refresh_time));
        o
    } else {
        let mut h = GameSetupHost::new();
        h.color_default = c.color;
        h.qr2 = c.qr2;
        h.thread = c.thread;
        let r = gs::handle_game_setup_slash_commands(&mut wm, &mut h, &c.text);
        let mut o = h.log.borrow().clone();
        o.push_str(&format!("ret {}\n", i32::from(r)));
        o
    }
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn pick<'a>(r: &mut Rng, v: &[&'a str]) -> &'a str {
    v[r.below(v.len() as u64) as usize]
}

/// A wide unit: ASCII, a letter's low byte under a high byte (`U+0148` truncates to `H`), a unit whose low byte is 0 (`U+0100`), or anything.
fn wide_unit(r: &mut Rng) -> u16 {
    match r.below(8) {
        0 => (1 + r.below(255)) as u16 | ((1 + r.below(255)) as u16) << 8,
        1 => ((1 + r.below(255)) as u16) << 8,
        2 => 1 + r.below(0xFFFF) as u16,
        3 => u16::from(b"hostme /rfsh"[r.below(12) as usize]) | (1 + r.below(255) as u16) << 8,
        _ => 0x20 + r.below(0x5F) as u16,
    }
}

fn gen_text(r: &mut Rng) -> Vec<u16> {
    let base = pick(
        r,
        &[
            "/host", "/Host", "/HOST", "/host extra words", "  /host", "/ host", "/\thost", "/\nhost", "/host\t", "/hos", "/hostile", "/hosts", "/me", "/me waves", "/ME WAVES", "/Me x",
            "/me  two", "/me\tfoo", "/me\nfoo", "/me\r", "/mex", "/m", "/refresh", "/Refresh", "/REFRESH now", "/ refresh", "/refreshing", "/togglegamelist", "/adjustchat", "/fakecrc", "/slots",
            "/discon", "/", "//", "//host", "hello", "hello /host", "", "x", "/ me hi", "/me!", "/me   ", "/me x", "/refresh\u{b}", "/host\u{b}", "/\u{b}host", "/h", "/\t", "/ \t\n",
        ],
    );
    let mut t = units(base);
    // wide units spliced in
    match r.below(6) {
        0 => {
            let at = r.below(t.len() as u64 + 1) as usize;
            let u = wide_unit(r);
            t.insert(at, u);
        }
        1 => {
            // a letter of the token replaced by the same letter under a high byte (still the same token upstream)
            if !t.is_empty() {
                let at = r.below(t.len() as u64) as usize;
                t[at] |= (1 + r.below(255) as u16) << 8;
            }
        }
        2 => {
            for _ in 0..1 + r.below(4) {
                let u = wide_unit(r);
                t.push(u);
            }
        }
        _ => {}
    }
    t.retain(|&u| u != 0);
    t
}

fn gen_case(r: &mut Rng) -> Case {
    let text = gen_text(r);
    let small = |r: &mut Rng| -> i32 {
        match r.below(6) {
            0 => 0,
            1 => 1,
            2 => -1,
            3 => i32::MAX,
            4 => i32::MIN,
            _ => r.below(1 << 31) as i32 - (1 << 30),
        }
    };
    let menu = r.below(2) as i32;
    // the generator's own reading of the text (only to keep a player list away from `/refresh`; the comparison does not use it)
    let slash_refresh = {
        let m: Vec<u8> = text.iter().map(|&u| u as u8).filter(|&b| b != 0).collect();
        m.first() == Some(&b'/') && {
            let rest = &m[1..];
            let rest: Vec<u8> = rest.iter().copied().skip_while(|b| b" \n\r\t".contains(b)).take_while(|b| !b" \n\r\t".contains(b)).collect();
            rest.eq_ignore_ascii_case(b"refresh")
        }
    };
    let clock = |r: &mut Rng| -> u32 { if r.chance(4) { 0 } else { r.below((1 << 31) - 30_000) as u32 } };
    let now = clock(r);
    let pick_t = |r: &mut Rng, interval: u32| -> u32 {
        match r.below(5) {
            0 => 0,
            1 => now.wrapping_sub(interval),
            2 => now.wrapping_sub(interval).wrapping_add(1),
            _ => clock(r),
        }
    };
    Case {
        menu,
        qr2: small(r),
        thread: small(r),
        color: small(r),
        // `/refresh` needs no player list (the draft's `PopulateLobbyPlayerListbox` would run on it)
        players: !(menu == 0 && slash_refresh) && r.chance(2),
        changed: r.chance(2),
        gtime: pick_t(r, 10000),
        ptime: pick_t(r, 5000),
        now,
        text,
    }
}

#[test]
fn the_slash_commands_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("DRAFT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x215_0C_0001);
    let (mut mismatches, mut handled, mut lobby, mut me, mut host, mut refresh) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for n in 0..cases {
        let c = gen_case(&mut r);
        let rust = rust_run(&c);
        let cpp = cpp_run(&c);
        handled += usize::from(cpp.contains("ret 1"));
        lobby += usize::from(c.menu == 0);
        me += usize::from(cpp.contains("sendChat"));
        host += usize::from(cpp.contains("addText"));
        refresh += usize::from(cpp.contains("hasStagingRoomListChanged"));
        if rust != cpp {
            mismatches += 1;
            if mismatches <= 3 {
                println!("case {n}: {c:?}\n  rust: {rust:?}\n  c++:  {cpp:?}");
            }
        }
    }
    println!("DIFF WOL slash commands compared {cases} mismatches {mismatches} ({lobby} lobby, {handled} handled, {host} /host, {me} /me, {refresh} /refresh)");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
