#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/Chat.cpp (the GameSpyColor table and field table, sendChat, both addChat,
//   addText, registerTextWindow, unregisterTextWindow)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `drafts::sweep_gui_logic::chat` against upstream's (#215).
//!
//! # Method
//!
//! Chat.cpp's colour tables and functions, PeerDefs.h's `GameSpyColors` enum and Color.h's `GameMakeColor` are cut verbatim
//! (build.rs, drafts_guilogic3) into `differential/wrap_draft_chat.cpp`, on upstream's own UnicodeString.cpp and AsciiString.cpp
//! (`%ls` added to the wide printf stand-in). The wrapper's header lists every stand-in.
//!
//! - The 27 defaults of `GameSpyColor` and the 27 names and offsets of `GameSpyColorFieldParse` against the draft's tables.
//! - 4,000 random programs of 3 to 14 operations over a configuration (group room, the two text restrictions, a saved-ignore mask, a buddy
//!   mask, a game in progress, a local name), window list boxes (rows, columns, selected rows), a player map, and `sendChat`, `addChat` by nick
//!   and with a player, `addText`, `registerTextWindow`, `unregisterTextWindow`. Compared: every side effect, in order (the requests to
//!   the peer thread, the audio events, the in-game message, each list box entry with its colour, each item data) and each result.
//!   Messages have 0 to 12 units from a set with spaces, tabs, `x` (the filter stand-in's target), brackets, `%`, and units 0xE9, 0xFF,
//!   0x100, 0x4E00 and 0xA0; names from `Bob bob BOB Al ig1 IGnore carl Zed Max`, the empty name, and two with a byte of 0x80 or more (`\xE9a`, `Z\xFF`: the
//!   widening of such a name is signed upstream). `sendChat`'s `static s_prevMsg` persists
//!   across programs in the C++ (a function static), so the draft's `s_prev_msg` is threaded through all of them in the same order.
//!
//! Excluded: `trim` of non-ASCII white space (the wrapper's `iswspace` is the ASCII one, as MSVC's and glibc's C locale; the draft trims ASCII
//! white space only), and `AsciiString::format`'s 2,048-byte limit (strings are short).
//!
//! # Prediction (before the first run, 2026-11-19)
//!
//! 0 mismatches in the tables and the functions. The places to look: `static s_prevMsg` threading, the private-message name list (`selections`
//! with the local name excluded by `compareNoCase`), the `-1` window handling in `addChat`.
//!
//! # Results
//!
//! 2026-11-19: the 27 colour defaults and the 27 field names with their offsets: **0 mismatches**. The 4,000 programs: first run **2,858 mismatches**, all of them a wrapper
//! stand-in's fault: `PeerRequest::text` as `std::basic_string<WideChar>`, whose libstdc++ traits read the 16-bit units as 32-bit under `-fshort-wchar` (the real field is MSVC's
//! 16-bit `std::wstring`); the stand-in is now a vector of units. Second run **739 mismatches**, a real draft defect (prediction missed: I did not predict it): `add_chat` widened
//! the player's name with `c as u16`, but `UnicodeString::translate` casts a signed `char`, so a byte of 0x80 or more becomes 0xFF80..0xFFFF (`0xE9` is `0xFFE9`). Fixed in the
//! draft. After the fix **0 mismatches**. Planted bugs, 12 of 12 caught (mismatches of 4,000): 15765 674 (duplicate filter inverted); 15766 81 (case-sensitive name compare in the
//! private list); 15767 131 (previous message not kept after a public send); 15768 117 (is-me ignores case); 15769 144 (`> 256`, off by one); 15770 131 (buddy checked after emote);
//! 15771 1,304 (public messages play the sound); 15772 3,106 (no item data in addText); 15773 2,401 (in-game message always); 15774 223 (registered windows ignored); 15775 333
//! (filter applied to the whole line, name included: **missed at first**, no name had an `x`; `Max` was added); 15776 1 (a colour default wrong).

mod support;

use rts_lockstep_engine::drafts::network::peer_defs::GameSpyColors;
use rts_lockstep_engine::drafts::sweep_gui_logic::chat as c;
use std::collections::BTreeSet;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_chat_colors(out: *mut i32) -> i32;
    fn ref_chat_fields(i: i32, name: *mut std::os::raw::c_char, off: *mut i32) -> i32;
    fn ref_chat_run(inp: *const i32, n: i32, out: *mut i32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

const PEER_FLAG_OP: i32 = 0x10;
const NAMES: [&str; 10] = ["Bob", "bob", "BOB", "Al", "ig1", "IGnore", "carl", "Zed", "Max", ""];

#[derive(Clone, Default)]
struct Win {
    rows: Vec<Vec<u8>>,
    ncols: i32,
    selected: Vec<i32>,
    added: i32,
}

#[derive(Default)]
struct Model {
    group_room: bool,
    disallow_asian: bool,
    disallow_non_asian: bool,
    saved_mask: u32,
    buddy_mask: u32,
    in_game: bool,
    local: Vec<u8>,
    wins: Vec<Win>,
    text_windows: BTreeSet<u32>,
    log: Vec<i32>,
    players: Vec<(Vec<u8>, i32, i32)>,
}

fn units(m: &mut Vec<i32>, t: &[u16]) {
    m.push(t.len() as i32);
    m.extend(t.iter().map(|&u| u as i32));
}

fn bytes(m: &mut Vec<i32>, t: &[u8]) {
    m.push(t.len() as i32);
    m.extend(t.iter().map(|&u| u as i32));
}

impl c::GameSpyChatHost for Model {
    fn in_group_room(&self) -> bool {
        self.group_room
    }
    fn queue_message_room(&mut self, text: &[u16], is_action: bool) {
        self.log.extend([1, 1, is_action as i32]);
        let mut l = std::mem::take(&mut self.log);
        units(&mut l, text);
        self.log = l;
    }
    fn queue_message_player(&mut self, text: &[u16], nicks: &[u8], is_action: bool) {
        self.log.extend([1, 2, is_action as i32]);
        let mut l = std::mem::take(&mut self.log);
        units(&mut l, text);
        bytes(&mut l, nicks);
        self.log = l;
    }
    fn local_name(&self) -> Vec<u8> {
        self.local.clone()
    }
    fn listbox_length(&self, lb: u32) -> i32 {
        self.wins[lb as usize - 1].rows.len() as i32
    }
    fn listbox_selected(&self, lb: u32) -> Vec<i32> {
        let mut v = self.wins[lb as usize - 1].selected.clone();
        v.push(-1);
        v
    }
    fn listbox_num_columns(&self, lb: u32) -> i32 {
        self.wins[lb as usize - 1].ncols
    }
    fn listbox_text_ascii(&self, lb: u32, row: i32, col: i32) -> Vec<u8> {
        let w = &self.wins[lb as usize - 1];
        if col == w.ncols - 1 && row >= 0 && (row as usize) < w.rows.len() {
            w.rows[row as usize].clone()
        } else {
            b"?".to_vec()
        }
    }
    fn listbox_add_entry_text(&mut self, win: u32, message: &[u16], color: i32) -> i32 {
        self.log.extend([5, win as i32, color]);
        let mut l = std::mem::take(&mut self.log);
        units(&mut l, message);
        self.log = l;
        let w = &mut self.wins[win as usize - 1];
        w.added += 1;
        w.added - 1
    }
    fn listbox_set_item_data(&mut self, win: u32, data: i32, index: i32) {
        self.log.extend([6, win as i32, data, index]);
    }
    fn is_saved_ignored(&self, profile_id: i32) -> bool {
        (0..32).contains(&profile_id) && (self.saved_mask >> profile_id) & 1 == 1
    }
    fn is_ignored(&self, name: &[u8]) -> bool {
        name.len() >= 2 && name[..2].eq_ignore_ascii_case(b"ig")
    }
    fn is_buddy(&self, profile_id: i32) -> bool {
        (0..32).contains(&profile_id) && (self.buddy_mask >> profile_id) & 1 == 1
    }
    fn disallow_asian_text(&self) -> bool {
        self.disallow_asian
    }
    fn disallow_non_asian_text(&self) -> bool {
        self.disallow_non_asian
    }
    fn audio_add_event(&mut self, name: &[u8]) {
        self.log.push(3);
        let mut l = std::mem::take(&mut self.log);
        bytes(&mut l, name);
        self.log = l;
    }
    fn language_filter_line(&self, msg: &mut Vec<u16>) {
        for u in msg.iter_mut() {
            if *u == b'x' as u16 {
                *u = b'*' as u16;
            }
        }
    }
    fn game_in_progress(&self) -> bool {
        self.in_game
    }
    fn in_game_ui_message(&mut self, message: &[u16]) {
        self.log.push(4);
        let mut l = std::mem::take(&mut self.log);
        units(&mut l, message);
        self.log = l;
    }
    fn first_text_window(&self) -> Option<u32> {
        self.text_windows.iter().next().copied()
    }
    fn text_windows_insert(&mut self, win: u32) {
        self.text_windows.insert(win);
    }
    fn text_windows_erase(&mut self, win: u32) {
        self.text_windows.remove(&win);
    }
    fn game_spy_color(&self, style: GameSpyColors) -> i32 {
        c::game_spy_color_defaults()[style as usize]
    }
    fn player_is_op(&self, flags: i32) -> bool {
        flags & PEER_FLAG_OP != 0
    }
}

fn message(g: &mut Gen) -> Vec<u16> {
    const SET: [u16; 16] = [b'a' as u16, b'b' as u16, b' ' as u16, b'\t' as u16, b'x' as u16, b'X' as u16, b'[' as u16, b']' as u16, b'%' as u16, 0xE9, 0xFF, 0x100, 0x4E00, 0xA0, b'\n' as u16, b'q' as u16];
    (0..g.below(13)).map(|_| SET[g.below(16) as usize]).collect()
}

fn put_str(prog: &mut Vec<i32>, s: &[u16]) {
    prog.push(s.len() as i32);
    prog.extend(s.iter().map(|&u| u as i32));
}

fn put_name(prog: &mut Vec<i32>, s: &[u8]) {
    prog.push(s.len() as i32);
    prog.extend(s.iter().map(|&u| u as i32));
}

fn to_u16(s: &[u8]) -> Vec<u16> {
    s.iter().map(|&b| b as u16).collect()
}

fn name(g: &mut Gen) -> Vec<u8> {
    // a tenth of the names have a byte of 0x80 or more (a Latin-1 name): `UnicodeString::translate` widens a `char` (signed), so 0xE9 becomes 0xFFE9
    match g.below(10) {
        0 => b"\xE9a".to_vec(),
        1 => b"Z\xFF".to_vec(),
        _ => NAMES[g.below(NAMES.len() as u32) as usize].as_bytes().to_vec(),
    }
}

fn lookup(m: &Model, nick: &[u8]) -> Option<c::ChatPlayer> {
    m.players.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(nick)).map(|(n, p, f)| c::ChatPlayer { name: n.clone(), profile_id: *p, flags: *f })
}

fn program(g: &mut Gen, prev: &mut Vec<u16>) -> (Vec<i32>, Vec<i32>) {
    let mut m = Model::default();
    let mut prog = Vec::new();
    m.group_room = g.below(2) == 0;
    m.disallow_asian = g.below(5) == 0;
    m.disallow_non_asian = g.below(5) == 0;
    m.saved_mask = if g.below(3) == 0 { 0 } else { g.u32() & g.u32() };
    m.buddy_mask = if g.below(3) == 0 { 0 } else { g.u32() & g.u32() };
    m.in_game = g.below(4) == 0;
    m.local = name(g);
    m.wins = vec![Win { ncols: 1, ..Win::default() }; 8];
    prog.extend([0, m.group_room as i32, m.disallow_asian as i32, m.disallow_non_asian as i32, m.saved_mask as i32, m.buddy_mask as i32, m.in_game as i32]);
    put_name(&mut prog, &m.local.clone());
    for _ in 0..(3 + g.below(12)) {
        match g.below(11) {
            0 | 1 => {
                // a window's rows and selection
                let w = 1 + g.below(8);
                let nr = g.below(6) as usize;
                let nc = 1 + g.below(3) as i32;
                let local = m.local.clone();
                let rows: Vec<Vec<u8>> = (0..nr)
                    .map(|_| match g.below(5) {
                        0 => vec![0x80 + g.below(100) as u8, b'a'],
                        // the local name in another case: `compareNoCase` takes it for the local name, a case-sensitive compare does not
                        1 => local.iter().map(|&b| if b.is_ascii_lowercase() { b.to_ascii_uppercase() } else { b.to_ascii_lowercase() }).collect(),
                        _ => name(g),
                    })
                    .collect();
                let selected: Vec<i32> = (0..nr as i32).filter(|_| g.below(2) == 0).collect();
                prog.extend([1, w as i32, nr as i32, nc]);
                for r in &rows {
                    put_str(&mut prog, &to_u16(r));
                }
                prog.push(selected.len() as i32);
                prog.extend(selected.iter().copied());
                let wm = &mut m.wins[w as usize - 1];
                wm.rows = rows;
                wm.ncols = nc;
                wm.selected = selected;
            }
            2 => {
                let (n, p, f) = (name(g), g.below(34) as i32 - 1, if g.below(2) == 0 { PEER_FLAG_OP } else { 0 } | g.below(4) as i32);
                prog.push(2);
                put_name(&mut prog, &n);
                prog.extend([p, f]);
                if let Some(e) = m.players.iter_mut().find(|(k, _, _)| k.eq_ignore_ascii_case(&n)) {
                    // std::map operator[]: the existing key stays, the value is replaced
                    e.0 = n.clone();
                    e.1 = p;
                    e.2 = f;
                } else {
                    m.players.push((n, p, f));
                }
            }
            3 | 4 => {
                // a third of the messages repeat the previous one (padded with white space, which `trim` removes): the duplicate filter
                let msg = if g.below(3) == 0 && !prev.is_empty() {
                    let mut v = vec![b' ' as u16];
                    v.extend(prev.iter().copied());
                    v.push(b'\t' as u16);
                    v
                } else {
                    message(g)
                };
                let act = g.below(2) == 0;
                let lb = if g.below(4) == 0 { 0 } else { 1 + g.below(8) };
                prog.push(3);
                put_str(&mut prog, &msg);
                prog.extend([act as i32, lb as i32]);
                let r = c::send_chat(&mut m, prev, &msg, act, if lb == 0 { None } else { Some(lb) });
                m.log.extend([0, r as i32]);
            }
            5 => {
                let (nick, prof, msg, public, act, win) = (name(g), g.below(34) as i32 - 1, message(g), g.below(2) == 0, g.below(2) == 0, g.below(9));
                prog.push(4);
                put_name(&mut prog, &nick);
                prog.push(prof);
                put_str(&mut prog, &msg);
                prog.extend([public as i32, act as i32, win as i32]);
                let p = lookup(&m, &nick);
                c::add_chat_by_nick(&mut m, p.as_ref(), &msg, public, act, if win == 0 { None } else { Some(win) });
            }
            6 | 7 => {
                let (n, prof, flags, msg, public, act, win) = (name(g), g.below(34) as i32 - 1, if g.below(2) == 0 { PEER_FLAG_OP } else { 0 }, message(g), g.below(2) == 0, g.below(2) == 0, g.below(9));
                prog.push(5);
                put_name(&mut prog, &n);
                prog.extend([prof, flags]);
                put_str(&mut prog, &msg);
                prog.extend([public as i32, act as i32, win as i32]);
                let p = c::ChatPlayer { name: n, profile_id: prof, flags };
                c::add_chat(&mut m, &p, &msg, public, act, if win == 0 { None } else { Some(win) });
            }
            8 => {
                let (msg, col, win) = (message(g), g.i32(), g.below(9));
                prog.push(6);
                put_str(&mut prog, &msg);
                prog.extend([col, win as i32]);
                let r = c::add_text(&mut m, &msg, col, if win == 0 { None } else { Some(win) });
                m.log.extend([7, r]);
            }
            9 => {
                let w = 1 + g.below(8);
                prog.extend([7, w as i32]);
                c::register_text_window(&mut m, w);
            }
            _ => {
                let w = 1 + g.below(8);
                prog.extend([8, w as i32]);
                c::unregister_text_window(&mut m, w);
            }
        }
    }
    (prog, m.log)
}

#[test]
fn chat() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C4A7);
    let mut cols = Check::new("GameSpyColor defaults");
    let mut fields = Check::new("GameSpyColorFieldParse");
    let mut run = Check::new("sendChat, addChat, addText, register");
    let mut out = [0i32; 32];
    let n = unsafe { ref_chat_colors(out.as_mut_ptr()) } as usize;
    cols.eq(c::game_spy_color_defaults().to_vec(), out[..n].to_vec(), || "defaults".to_string());
    let mut want = Vec::new();
    let mut got = Vec::new();
    let mut buf = vec![0 as std::os::raw::c_char; 64];
    let mut i = 0;
    loop {
        let mut off = 0;
        if unsafe { ref_chat_fields(i, buf.as_mut_ptr(), &mut off) } == 0 {
            break;
        }
        let nm: Vec<u8> = buf.iter().take_while(|&&b| b != 0).map(|&b| b as u8).collect();
        got.push((String::from_utf8_lossy(&nm).to_string(), off));
        i += 1;
    }
    for (nm, col) in c::GAME_SPY_COLOR_FIELDS.iter() {
        want.push((nm.to_string(), *col as i32));
    }
    fields.eq(want, got, || "field table".to_string());

    let mut prev: Vec<u16> = Vec::new();
    for case in 0..4_000 {
        let (prog, want) = program(&mut g, &mut prev);
        let mut o = vec![0i32; 8192];
        let k = unsafe { ref_chat_run(prog.as_ptr(), prog.len() as i32, o.as_mut_ptr(), o.len() as i32) };
        assert!(k >= 0, "buffer too small");
        o.truncate(k as usize);
        run.eq(want, o, || format!("program {case}: {prog:?}"));
    }
    if planted() {
        for ch in [cols, fields, run] {
            ch.report();
        }
        panic!("a planted run: see the MEASURED lines (a mismatch is the plant caught)");
    }
    for ch in [cols, fields, run] {
        ch.finish();
    }
}
