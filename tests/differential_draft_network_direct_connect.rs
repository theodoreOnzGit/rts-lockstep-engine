// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui5) of the draft `src/drafts/ui/network_direct_connect.rs` (the file's statics, `PopulateRemoteIPComboBox`,
//! `UpdateRemoteIPList`, `HostDirectConnectGame`, `JoinDirectConnectGame`, `NetworkDirectConnectInit`, `Shutdown`, `Update`, `Input`, `System`) against upstream
//! `NetworkDirectConnect.cpp`, cut verbatim by build.rs (`drafts_ui5`) into the program `UI5_NDC_EXE` (differential/wrap_draft_ui5_ndc.cpp lists the three edits and every
//! stand-in), which also holds upstream's real window manager, gadgets (combo box, text entry, static text, push button) and WindowLayout.cpp.
//!
//! # Method
//!
//! `DRAFT_CASES` sequences (default 3,000: one run is under a minute; fixed seed). Each is a scene of named windows (`NetworkDirectConnect.wnd:...`): the menu's parent
//! window (menu callbacks), 0 to 2 decoy top-level windows, the three buttons, the player-name entry and the local-IP static text each present with probability 5/6, and
//! the remote-IP combo box (always present: `UpdateRemoteIPList` dereferences it, see the exclusions), under the parent (70 %), top level or under a decoy, a decoy copy
//! of each name one time in three; the menu's layout is a random subset of the windows. 4 to 22 operations: Init, Shutdown (immediate or not), Update, the system callback called
//! directly (`GBM_SELECTED` with a control, `GEM_EDIT_DONE`, `CREATE`, `DESTROY`, others), the input callback (Escape and other keys in each state), presses and releases at the
//! buttons, the player name set (text of white space, non-ASCII, a unit 0x100), the combo's text and entries set (well-formed and malformed IPs, `ip:name`, `ip(name)`, signs,
//! white space, extra colons), a selected position, the ini file's entries (`UserName`, `NumRemoteIPs`, `RemoteIP<n>`), the enumerated addresses and the online one, TheLAN
//! deleted / created / given an address, the animate manager's `needsUpdate`, hiding, enabling, focusing. After each operation both programs print the stand-in calls (TheLAN, the
//! shell, the ini file's writes), the messages, the dump of every window, the layouts, the statics, TheLAN, the ini file, the transition handler's and animate manager's state; the
//! texts must be identical.
//!
//! The Rust host's LANPreferences accessors are the verified `lan_lobby_menu::get_num_remote_ips` / `get_remote_ip_entry` and `map_cache`'s QuotedPrintable; the C++ runs upstream's
//! own (cut). The transition handler and the shell are the verified Rust ones; the C++ models what they do for this menu (the wrapper's header says how).
//!
//! Excluded inputs (upstream's undefined behaviour; the draft reports each through `NetworkDirectConnectHost::undefined`, or the generator avoids it): `TheLAN` NULL at a
//! `RequestSetName` / `RequestGame*` that upstream dereferences (Back before the first Init, with TheLAN deleted); `UpdateRemoteIPList` with no combo box; `JoinDirectConnectGame`
//! where `sscanf` reads fewer than 4 fields (`ip1..ip4` uninitialised) or the text before `(` is 16 bytes or longer (`strcpy` into `char ipstr[16]`); `UpdateRemoteIPList` comparing an
//! entry whose `swscanf` read too few fields (`n2` uninitialised); a NULL control as `GBM_SELECTED`'s `mData1`; `GWM_INPUT_FOCUS` sent directly with `mData1` TRUE; field values
//! beyond `int` (`scanf` overflow differs between C libraries). `getRemoteIPEntry` of an entry with no `:` reads one byte past the string: defined as 0 in the shim (see
//! differential/shim_ui5/Common/AsciiString.h), as the verified Rust takes it. `QuotedPrintableToUnicodeString` of a string that decodes to an odd number of bytes leaves its `static` buffer's
//! stale units after the result (it writes one zero byte, not a zero unit): the Rust host reports it through `undefined` (see `qp_odd`) and the input is excluded.
//!
//! # Prediction (before the first run, 2026-11-21)
//!
//! Draft defects I expect from reading upstream against the draft: (1) `TheLAN = NEW LANAPI(); ... TheLAN->init()` in Init is `lan_create(false)` then `lan_create(true)`: a second
//! `new` and `init` (the host cannot tell them from a fresh one); (2) `nextToken` skips leading separators (the draft's `take_while != ':'`/`!= '('` does not); (3) `scanf`'s `%d`
//! skips white space and reads a sign (the draft's `scan_ip` does neither); (4) `translate` drops a unit whose low byte is 0 (the draft keeps a NUL); (5) `(ip1 << 24) + ...`
//! wraps (the draft's `+` panics with overflow checks on). Nothing else.
//!
//! # Planted bugs (`L9_PLANT=<id>`, the draft's `plant(id)`)
//!
//! | id | planted bug |
//! |---|---|
//! | 22661 | `PopulateRemoteIPComboBox` selects position 1, not 0 |
//! | 22662 | the duplicate test compares 3 fields, not 4 |
//! | 22663 | the ini entries are numbered from 0, not 1 |
//! | 22664 | `NumRemoteIPs` is not incremented for a new entry |
//! | 22665 | Host asks for a game that is not a direct connect |
//! | 22666 | the player name is trimmed to 11 characters |
//! | 22667 | Back does not set `buttonPushed` |
//! | 22668 | Escape acts on the key-down |
//! | 22669 | Init leaves the lobby not forced |
//! | 22670 | an immediate shutdown still animates |
//! | 22671 | Update completes when EITHER the animation or the transition is finished |
//! | 22672 | Join packs the first field at bit 16 |
//! | 22675 | `translate` keeps a unit whose low byte is 0 as a NUL |
//!
//! # Results
//!
//! 2026-11-21, `DRAFT_CASES=3000` (the default, about 15 s a run), max ULP 0 (the comparison is of text). Before the fixes, the first run had **2,069 mismatches of 69,406 outputs**: the
//! draft asked its host for a second `new` and `init` of `TheLAN` (prediction (1): fixed with `lan_init`). Then 416 of 81,762 (the C++ program crashed at Escape with `buttonBack` NULL:
//! upstream's undefined behaviour, now reported through `undefined`), 8 and 4 (two stand-in defects of mine: `Shell::pop` detaches the IME; `QuotedPrintableToUnicodeString` leaves stale
//! units after an odd-length result: excluded, see above), then **0 of 87,445** once `shutdownComplete` no longer ended the sequence (the host's pending push answers with the menu's layout),
//! and a last mismatch **found by the combo composites, in the verified-looking `lan_lobby_menu::get_remote_ip_entry`** (a byte above 0x7F zero-extended where `(WideChar)char` sign-extends;
//! fixed there): **0 mismatches over 3,000 sequences, 92,399 outputs compared**. Prediction (2) to (5) (`nextToken`, `scanf`, `translate`, wrapping) were fixed on reading, before a run:
//! each of those inputs made the draft report `undefined` (so they were excluded, not mismatched), which is also why no plant on `nextToken` or the `scanf` white space can be caught (two
//! such plants were dropped). Planted bugs (mismatches of 3,000 sequences): 22661 552; 22662 31; 22663 318; 22664 373; 22665 82; 22666 19; 22667 290; 22668 740; 22669 1,815; 22670 684;
//! 22671 162; 22672 441; 22675 4. **13 of 13 caught.** Not covered by the C++ reference: `reverseAnimatewindow` and the transition group's state are models of the verified Rust (the
//! wrapper's header).

mod support;
mod ui4_common;

use rts_lockstep_engine::client::gadget_combo_box as cbx;
use rts_lockstep_engine::client::gadget_text_entry as tex;
use rts_lockstep_engine::client::shell::{Shell, ShellEnv, ShellHost, ShellRequest};
use rts_lockstep_engine::client::window_manager::*;
use rts_lockstep_engine::client::window_script::{gws, win_status, ComboBoxData, GameFont, WinId, WinInstanceData};
use rts_lockstep_engine::client::window_transitions::TransitionGroup;
use rts_lockstep_engine::common::map_cache::{quoted_printable_to_unicode_string, unicode_string_to_quoted_printable};
use rts_lockstep_engine::client::user_preferences::UserPreferences;
use rts_lockstep_engine::drafts::sweep_gui_logic::lan_lobby_menu::{get_num_remote_ips, get_remote_ip_entry};
use rts_lockstep_engine::drafts::ui::network_direct_connect::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Mutex;
use support::{Check, Gen};
use ui4_common::*;

static CPP: Mutex<()> = Mutex::new(());

const SYS: &[u8] = b"NetworkDirectConnectSystem";
const INP: &[u8] = b"NetworkDirectConnectInput";
const GROUP: &[u8] = b"NetworkDirectConnectFade";

#[derive(Default)]
struct Host {
    log: String,
    keys: KeyTable,
    shell: Shell,
    ndc: NetworkDirectConnect,
    lan: Option<u32>,
    ini: BTreeMap<Vec<u8>, Vec<u8>>,
    pending: Vec<(Vec<u8>, Vec<u8>)>,
    ips: Vec<u32>,
    online: u32,
    lan_button_pushed: bool,
    lan_is_shutting_down: bool,
    ub: bool,
    layout: usize,
}

impl HasLog for Host {
    fn log(&mut self) -> &mut String {
        &mut self.log
    }
}

impl GuiHost for Host {
    fn system(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == SYS {
            network_direct_connect_system(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn input(&mut self, wm: &mut GameWindowManager, func: &[u8], win: WinId, msg: u32, d1: WindowMsgData, d2: WindowMsgData) -> MsgHandled {
        if func == INP {
            network_direct_connect_input(wm, self, win, msg, d1, d2)
        } else {
            MsgHandled::Ignored
        }
    }
    fn add_audio_event(&mut self, name: &[u8]) {
        let _ = writeln!(self.log, "audio {}", String::from_utf8_lossy(name));
    }
    fn block_input_left_up(&mut self) {
        self.log.push_str("block\n");
    }
    fn font_height(&self, font: Option<&GameFont>) -> i32 {
        font.map_or(12, |f| f.point_size)
    }
    fn printable_key(&self, key: u8, state: i32) -> u16 {
        if state == 1 { u16::from(key) } else { 0 }
    }
    fn image_size(&self, image: &[u8]) -> Option<(i32, i32)> {
        Some(if image.ends_with(b"Mid") { (7, 10) } else { (5, 10) })
    }
}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, filename: &[u8]) -> Option<usize> {
        match filename {
            b"Menus/BlankWindow.wnd" => self.log.push_str("shell map 0\n"),
            b"pushed" => {
                let _ = writeln!(self.log, "shell complete {}", self.layout);
                return Some(self.layout);
            }
            _ => panic!("an unexpected layout"),
        }
        None
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, func: &[u8], _layout: usize, _user: Option<bool>) {
        assert_eq!(func, b"shutdown");
        self.log.push_str("shell pop\n");
    }
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv { shell_map_on: true, ..ShellEnv::default() }
    }
    fn request(&mut self, r: ShellRequest) {
        assert_eq!(r, ShellRequest::NewShellGame);
        self.log.push_str("shell map 1\n");
    }
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.key(name)
    }
}

fn prefs(h: &Host) -> UserPreferences {
    UserPreferences { map: h.ini.clone(), ..Default::default() }
}

/// Whether `QuotedPrintableToUnicodeString` of `s` decodes to an odd number of bytes: upstream then writes ONE zero byte after the last unit, not the terminating zero unit, so the
/// units after it are whatever an earlier call left in its `static` buffer (undefined; the verified Rust takes them as 0).
fn qp_odd(s: &[u8]) -> bool {
    let (mut i, mut n) = (0usize, 0usize);
    while i < s.len() && n < 1023 {
        if s[i] == b'_' {
            if i + 1 >= s.len() {
                break;
            }
            i += 1;
            if i + 1 < s.len() {
                i += 1;
            }
        }
        i += 1;
        n += 1;
    }
    n % 2 == 1
}

fn c_string(mut v: Vec<u8>) -> Vec<u8> {
    if let Some(p) = v.iter().position(|&b| b == 0) {
        v.truncate(p);
    }
    v
}

impl NetworkDirectConnectHost for Host {
    fn network_direct_connect(&mut self) -> &mut NetworkDirectConnect {
        &mut self.ndc
    }
    fn set_lan_button_pushed(&mut self, v: bool) {
        self.lan_button_pushed = v;
    }
    fn set_lan_is_shutting_down(&mut self, v: bool) {
        self.lan_is_shutting_down = v;
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        let mut t = b"T:".to_vec();
        t.extend_from_slice(label);
        t.iter().map(|&b| u16::from(b)).collect()
    }
    fn lan_prefs_get_user_name(&mut self) -> Vec<u16> {
        let machine: Vec<u16> = "MACHINE".encode_utf16().collect();
        let Some(v) = self.ini.get(&b"UserName"[..]).cloned() else { return machine };
        if qp_odd(&v) {
            self.ub = true;
        }
        let mut r = quoted_printable_to_unicode_string(&v);
        let ws = |c: &u16| *c == 0x20 || (9..=13).contains(c);
        while r.first().is_some_and(ws) {
            r.remove(0);
        }
        while r.last().is_some_and(ws) {
            r.pop();
        }
        if r.is_empty() { machine } else { r }
    }
    fn lan_prefs_get_num_remote_ips(&mut self) -> i32 {
        get_num_remote_ips(&mut prefs(self))
    }
    fn lan_prefs_get_remote_ip_entry(&mut self, i: i32) -> Vec<u16> {
        if let Some(e) = self.ini.get(format!("RemoteIP{i}").as_bytes()) {
            let start = e.iter().position(|&c| c != b':').unwrap_or(e.len());
            let end = e[start..].iter().position(|&c| c == b':').map_or(e.len(), |p| start + p);
            if end < e.len() && qp_odd(&e[end + 1..]) {
                self.ub = true;
            }
        }
        get_remote_ip_entry(&prefs(self), i)
    }
    fn lan_prefs_set(&mut self, key: &[u8], value: &[u8]) {
        self.pending.push((key.to_vec(), c_string(value.to_vec())));
    }
    fn lan_prefs_write(&mut self) {
        for (k, v) in std::mem::take(&mut self.pending) {
            self.ini.insert(k, v);
        }
        self.log.push_str("prefs write\n");
    }
    fn unicode_to_quoted_printable(&self, name: &[u16]) -> Vec<u8> {
        unicode_string_to_quoted_printable(name)
    }
    fn lan_exists(&self) -> bool {
        self.lan.is_some()
    }
    fn lan_create(&mut self, init: bool) {
        self.lan = Some(0);
        self.log.push_str("lan new\n");
        if init {
            self.log.push_str("lan init\n");
        }
    }
    fn lan_init(&mut self) {
        self.log.push_str("lan init\n");
    }
    fn lan_delete(&mut self) {
        if self.lan.take().is_some() {
            self.log.push_str("lan delete\n");
        }
    }
    fn lan_reset(&mut self) {
        if self.lan.is_some() {
            self.log.push_str("lan reset\n");
        } else {
            self.ub = true;
        }
    }
    fn lan_get_local_ip(&self) -> u32 {
        self.lan.unwrap_or(0)
    }
    fn lan_set_local_ip(&mut self, ip: u32) {
        if self.lan.is_some() {
            self.lan = Some(ip);
            let _ = writeln!(self.log, "lan setip {ip}");
        } else {
            self.ub = true;
        }
    }
    fn lan_request_set_name(&mut self, name: &[u16]) {
        if self.lan.is_none() {
            self.ub = true;
            return;
        }
        self.log.push_str("lan name");
        text(&mut self.log, name);
        self.log.push('\n');
    }
    fn lan_request_game_create(&mut self, ip_string: &[u16], direct_connect: bool) {
        self.log.push_str("lan create");
        text(&mut self.log, ip_string);
        let _ = writeln!(self.log, " {}", u8::from(direct_connect));
    }
    fn lan_request_game_join_direct_connect(&mut self, ip: u32) {
        let _ = writeln!(self.log, "lan join {ip}");
    }
    fn lan_request_lobby_leave(&mut self, forced: bool) {
        let _ = writeln!(self.log, "lan leave {}", u8::from(forced));
    }
    fn option_prefs_online_ip_address(&mut self) -> u32 {
        self.online
    }
    fn enumerated_ip_addresses(&mut self) -> Vec<u32> {
        self.ips.clone()
    }
    fn undefined(&mut self, _what: &str) {
        self.ub = true;
    }
}

// ---------------------------------------------------------------------------------------------------------------------

const IPS: [u32; 4] = [0, 0xC0A8_0001, 0x0A00_0001, 0x7F00_0001];

/// hex of a text's units, comma separated
fn hx(s: &str) -> String {
    hex(&s.encode_utf16().collect::<Vec<u16>>())
}

/// combo texts: well-formed, malformed and odd ones, as units in hex
fn combo_texts() -> Vec<String> {
    let mut v: Vec<String> = [
        "1.2.3.4", "1.2.3.5", "1.2.3.5:b", "1.2.3.4:b", "10.0.0.1:Bob", "192.168.0.1(bob)", "1.2.3.4(x)", "999.1.1.1", "1.2", "", "abc", " 5.6.7.8", "5. 6.7.8", "+1.2.3.4", "(1.2.3.4)", ":1.2.3.4", "1.2.3.4:", "::1.2.3.4:5", "1.2.3.4(",
        "12.34.56.78:abcdef", "((1.2.3.4", "1.2.3.4:b(c)", "10.0.0.1", "10.0.0.1:Al", "-1.2.3.4", "1.2.3.4.5", "1 .2.3.4", "255.255.255.255", "8.8.8.8:Al(1)", "(:1.2.3.4", "1.2.3.4 (z)",
    ]
    .iter()
    .map(|s| hx(s))
    .collect();
    // a unit whose low byte is 0, non-ASCII ones
    v.push("31,100,2e,32,2e,33,2e,34".into());
    v.push("31,2e,32,2e,33,2e,34,3a,e9,4e2d".into());
    v.push("31,2e,32,2e,33,2e,34,28,100,29".into());
    v
}

fn name_texts() -> Vec<String> {
    let mut v: Vec<String> = ["Bob", "A", "ABCDEFGHIJKL", "ABCDEFGHIJKLM", "ABCDEFGHIJKLMNOPQRST", "  Al  ", "a_b", "x:y(z)"].iter().map(|s| hx(s)).collect();
    v.push("-".into());
    v.push("20,20,20".into());
    v.push("e9,4e2d,100,41".into());
    v.push("41,42,43,44,45,46,47,48,49,4a,4b,100,4c,4d".into());
    v
}

struct Scene {
    ops: Vec<String>,
    named: Vec<(usize, String)>,
    /// (index, kind, x, y, w, h): 0 top-level, 1 back, 2 host, 3 join, 4 name entry, 5 static, 6 combo
    wins: Vec<(usize, u8, i32, i32, i32, i32)>,
}

fn pick<T: Clone>(g: &mut Gen, v: &[T]) -> T {
    v[g.below(v.len() as u32) as usize].clone()
}

fn gen_scene(g: &mut Gen) -> Scene {
    let mut ops = Vec::new();
    let mut wins: Vec<(usize, u8, i32, i32, i32, i32)> = Vec::new();
    let en = win_status::ENABLED;
    let mut next = 0usize;
    let mut tops = Vec::new();
    let parent_present = g.below(6) != 0;
    if parent_present {
        ops.push(format!("@ wu -1 0 0 0 640 480 {en} {}", gws::USER_WINDOW));
        wins.push((0, 0, 0, 0, 640, 480));
        tops.push(0usize);
        next += 1;
    }
    let decoys = if parent_present { g.below(3) as usize } else { 1 + g.below(2) as usize };
    for _ in 0..decoys {
        let (x, y) = (g.below(400) as i32, g.below(300) as i32);
        ops.push(format!("@ wu -1 0 {x} {y} 300 200 {en} {}", gws::USER_WINDOW));
        wins.push((next, 0, x, y, 300, 200));
        tops.push(next);
        next += 1;
    }
    let mut named: Vec<(usize, String)> = Vec::new();
    let mut k = 0;
    // the combo box last: its children take the indices after it
    for (name, kind) in [("ButtonBack", 1u8), ("ButtonHost", 2), ("ButtonJoin", 3), ("EditPlayerName", 4), ("StaticLocalIP", 5), ("ComboboxRemoteIP", 6)] {
        for copy in 0..2 {
            let present = if copy == 0 { kind == 6 || g.below(6) != 0 } else { g.below(3) == 0 && decoys > 0 && kind != 6 };
            if !present {
                continue;
            }
            let parent: Option<usize> = if copy == 1 {
                Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize])
            } else {
                match g.below(20) {
                    0..=13 if parent_present => Some(0),
                    0..=13 => None,
                    14..=16 => None,
                    _ if decoys > 0 => Some(tops[tops.len() - 1 - g.below(decoys as u32) as usize]),
                    _ => None,
                }
            };
            let (x, y, w, h) = (10 + (k % 5) * 70, 10 + (k / 5) * 30, 60, 20);
            k += 1;
            let mut status = en;
            if g.below(8) == 0 {
                status |= win_status::HIDDEN;
            }
            let at = |p: usize| wins.iter().position(|q| q.0 == p).expect("parent");
            let (px, py) = parent.map_or((0, 0), |p| (wins[at(p)].2, wins[at(p)].3));
            let ps = parent.map_or(-1, |p| p as i32);
            let line = match kind {
                4 => format!("W {ps} 9 {x} {y} {w} {h} {status} {} 0 {} 0 0 0 0 _", gws::ENTRY_FIELD, g.below(20) as i32 + 1),
                5 => format!("W {ps} 11 {x} {y} {w} {h} {status} {} 0", gws::STATIC_TEXT),
                6 => format!("W {ps} 4 {x} {y} {w} {h} {status} {} 0 {} 5 64 0 0 6 1 0", gws::COMBO_BOX, u8::from(g.below(5) != 0)),
                _ => format!("W {ps} 1 {x} {y} {w} {h} {status} {} 0", gws::PUSH_BUTTON),
            };
            ops.push(line);
            wins.push((next, kind, px + x, py + y, w, h));
            named.push((next, name.to_string()));
            next += 1;
            if kind == 6 {
                next += 3; // (not used: the combo is last)
            }
        }
    }
    for (i, name) in &named {
        ops.push(format!("@ nm {i} NetworkDirectConnect.wnd:{name}"));
    }
    if parent_present {
        ops.push("@ nm 0 NetworkDirectConnect.wnd:ParentNetworkDirectConnect".to_string());
    }
    // the menu's layout: a random subset of the windows (the first operation of the menu)
    let members: Vec<usize> = wins.iter().map(|w| w.0).filter(|_| g.below(3) != 0).collect();
    let mut l = format!("@ nd layout {}", members.len());
    for m in &members {
        let _ = write!(l, " {m}");
    }
    ops.push(l);
    // the starting state of the stand-ins
    let nips = 1 + g.below(3);
    let mut l = format!("@ nd ips {nips}");
    for _ in 0..nips {
        let _ = write!(l, " {}", pick(g, &IPS[1..]));
    }
    ops.push(l);
    ops.push(format!("@ nd online {}", pick(g, &IPS)));
    Scene { ops, named, wins }
}

fn gen_op(g: &mut Gen, s: &Scene) -> String {
    let combo: Vec<usize> = s.wins.iter().filter(|w| w.1 == 6).map(|w| w.0).collect();
    let entries: Vec<usize> = s.wins.iter().filter(|w| w.1 == 4).map(|w| w.0).collect();
    let buttons: Vec<(usize, u8, i32, i32, i32, i32)> = s.wins.iter().filter(|w| (1..=3).contains(&w.1)).cloned().collect();
    let any = |g: &mut Gen| pick(g, &s.wins).0;
    let menu_win = |g: &mut Gen| pick(g, &s.wins.iter().filter(|w| w.1 == 0).map(|w| w.0).collect::<Vec<_>>());
    let by_name = |n: &str| s.named.iter().find(|(_, k)| k == n).map(|(i, _)| *i);
    if g.below(100) < 14 && !combo.is_empty() {
        // composites: the combo's entries and text set, then the press that reads them
        let win = menu_win(g);
        let texts = ["1.2.3.4", "1.2.3.5", "1.2.3.5:b", "1.2.3.4:b", "10.0.0.1(bob)", "10.0.0.1", "(1.2.3.4)", "5. 6.7.8", "+1.2.3.4", "1.2.3.4(", "300.4.5.6", "7.7.7.7:a"];
        let (a, b, c) = (hx(pick(g, &texts)), hx(pick(g, &texts)), hx(pick(g, &texts)));
        if let Some(join) = by_name("ButtonJoin") {
            return match g.below(3) {
                0 => format!("@ nd cbadd {} {a} 16777215\n@ nd cbadd {} {b} 16777215\n@ nd cbtext {} {c}\n@ sys 0 {win} {} 1 {join} 0", combo[0], combo[0], combo[0], ggm::GBM_SELECTED),
                1 => format!("@ nd cbadd {} {a} 16777215\n@ nd cbadd {} {b} 16777215\ns {} {} 0\n@ sys 0 {win} {} 1 {join} 0", combo[0], combo[0], combo[0], g.below(3), ggm::GBM_SELECTED),
                _ => format!("@ nd cbtext {} {c}\n@ sys 0 {win} {} 1 {join} 0", combo[0], ggm::GBM_SELECTED),
            };
        }
    }
    match g.below(100) {
        0..=11 => "@ nd init".into(),
        12..=17 => format!("@ nd shutdown {}", g.below(2)),
        18..=25 => "@ nd update".into(),
        26..=37 => {
            let msg = match g.below(6) {
                0..=2 => ggm::GBM_SELECTED,
                3 => ggm::GEM_EDIT_DONE,
                _ => pick(g, &[1, 2, 3, 4, 21, 23, 24, 25]),
            };
            let win = menu_win(g);
            let (kind, d1, d2) = if msg == ggm::GBM_SELECTED || msg == ggm::GEM_EDIT_DONE { (1, any(g) as u32, g.below(3)) } else if msg == gwm::INPUT_FOCUS { (0, 2 + g.below(2), 0) } else { (0, g.below(3), g.below(3)) };
            format!("@ sys 0 {win} {msg} {kind} {d1} {d2}")
        }
        38..=47 => {
            let key = pick(g, &[1u32, 1, 1, 2, 28, 0x101, 0x201, 57]);
            let state = pick(g, &[1u32, 2, 3, 0, 0x101, 4]);
            let win = menu_win(g);
            let msg = if g.below(6) == 0 { pick(g, &[1u32, 2, 24, 25, 123]) } else { gwm::CHAR };
            format!("@ inp 0 {win} {msg} 0 {key} {state}")
        }
        48..=57 if !buttons.is_empty() => {
            let (_, _, x, y, ww, hh) = pick(g, &buttons);
            format!("M {} {} {} 0 0", if g.below(2) == 0 { gwm::LEFT_DOWN } else { gwm::LEFT_UP }, x + ww / 2, y + hh / 2)
        }
        58..=63 if !entries.is_empty() => format!("@ tx {} {}", pick(g, &entries), pick(g, &name_texts())),
        64..=69 if !entries.is_empty() => format!("I {} {}", pick(g, &entries), pick(g, &[u32::from(b'a'), 32, 13, 0xE9, 0x100, 0x4E2D])),
        70..=77 if !combo.is_empty() => format!("@ nd cbtext {} {}", combo[0], pick(g, &combo_texts())),
        78..=83 if !combo.is_empty() => format!("@ nd cbadd {} {} 16777215", combo[0], pick(g, &combo_texts())),
        84..=86 if !combo.is_empty() => format!("s {} {} {}", combo[0], g.below(5) as i32 - 1, g.below(2)),
        87 if !combo.is_empty() => format!("r {}", combo[0]),
        88..=90 => {
            let n = g.below(5);
            let mut l = format!("@ nd prefs NumRemoteIPs {}", n);
            if g.below(3) == 0 {
                l = format!("@ nd prefs NumRemoteIPs {}", pick(g, &["-", "0", "x", "2"]));
            }
            l
        }
        91..=93 => {
            let vals = ["1.2.3.4:Bob", "10.0.0.1:", "192.168.0.1:Al_20B", "5.6.7.8:x", "1.2.3.4:", "9.9.9.9:_41_E9", "-"];
            format!("@ nd prefs RemoteIP{} {}", g.below(5), pick(g, &vals))
        }
        94 => format!("@ nd prefs UserName {}", pick(g, &["Bob", "_20_20", "-", "_42_6F_62", "_E9", "Abcdefghijklmnop"])),
        95 => format!("@ nd lan {}", g.below(3)).replace("lan 2", &format!("lan 2 {}", pick(g, &IPS))),
        96 => format!("@ nd anim {}", g.below(2)),
        97 => format!("H {} {}", any(g), g.below(2)),
        98 => format!("F {}", any(g) as i32),
        _ => format!("E {} {}", any(g), g.below(2)),
    }
}

fn state_dump(wm: &GameWindowManager, h: &Host) -> String {
    let mut o = dump(wm);
    o.push_str(&dump_layouts(wm));
    o.push_str(&dump_static_texts(wm));
    let d = &h.ndc;
    let _ = writeln!(o, "nd shutting {} pushed {} lanbp {} lansd {} lan {} ip {}", u8::from(d.is_shutting_down), u8::from(d.button_pushed), u8::from(h.lan_button_pushed), u8::from(h.lan_is_shutting_down), u8::from(h.lan.is_some()), h.lan.unwrap_or(0));
    let _ = writeln!(
        o,
        "nd ids {} {} {} {} {} {} wins {} {} {} {} {} {}",
        d.button_back_id, d.button_host_id, d.button_join_id, d.edit_player_name_id, d.combobox_remote_ip_id, d.static_local_ip_id, idx(d.button_back), idx(d.button_host), idx(d.button_join), idx(d.edit_player_name), idx(d.combobox_remote_ip), idx(d.static_local_ip)
    );
    let t = &wm.transitions;
    let _ = writeln!(o, "nd trans cur {} pend {} dir {} anim nu {} rev {}", u8::from(t.current.is_some()), u8::from(t.pending.is_some()), t.groups[0].direction_multiplier, u8::from(h.shell.animate.needs_update), u8::from(h.shell.animate.reverse));
    o.push_str("nd ini");
    for (k, v) in &h.ini {
        let _ = write!(o, " {}=", String::from_utf8_lossy(k));
        for b in v {
            let _ = write!(o, "{b:02x}");
        }
    }
    o.push('\n');
    o
}

fn hexs(t: &str) -> Vec<u16> {
    if t == "-" { Vec::new() } else { t.split(',').map(|x| u16::from_str_radix(x, 16).expect("hex")).collect() }
}

fn menu_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) {
    let v: Vec<&str> = op.split(' ').collect();
    let num = |i: usize| -> i64 { v[i].parse().expect("number") };
    match v[1] {
        "sys" | "inp" => {
            let (win, msg, kind, d1, d2) = (num(3) as usize, num(4) as u32, num(5), num(6) as u32, num(7) as u32);
            let a = if kind == 1 { win_data(Some(d1 as usize)) } else { d1 };
            let r = if v[1] == "sys" { network_direct_connect_system(wm, h, win, msg, a, d2) } else { network_direct_connect_input(wm, h, win, msg, a, d2) };
            let _ = writeln!(h.log, "ret {}", u8::from(r == MsgHandled::Handled));
        }
        "nd" => match v[2] {
            "layout" => {
                let n = num(3) as usize;
                let members: Vec<usize> = (0..n).map(|i| num(4 + i) as usize).collect();
                let l = make_layout(wm, &members);
                wm.layouts[l].shutdown = Some(b"shutdown".to_vec());
                h.shell.screen_stack = vec![l];
                h.shell.is_shell_active = true;
                h.layout = l;
                h.log.push_str("layout NetworkDirectConnect.wnd\n");
            }
            "init" => network_direct_connect_init(wm, h, h.layout),
            "shutdown" => network_direct_connect_shutdown(wm, h, h.layout, num(3) != 0),
            "update" => network_direct_connect_update(wm, h, h.layout),
            "ips" => h.ips = (0..num(3) as usize).map(|i| num(4 + i) as u32).collect(),
            "online" => h.online = num(3) as u32,
            "prefs" => {
                if v[4] == "-" {
                    h.ini.remove(v[3].as_bytes());
                } else {
                    h.ini.insert(v[3].as_bytes().to_vec(), v[4].as_bytes().to_vec());
                }
            }
            "lan" => match num(3) {
                0 => h.lan_delete(),
                1 => {
                    if h.lan.is_none() {
                        h.lan_create(false);
                    }
                }
                _ => {
                    if h.lan.is_some() {
                        let ip = num(4) as u32;
                        h.lan = Some(ip);
                        let _ = writeln!(h.log, "lan setip {ip}");
                    }
                }
            },
            "cbtext" => cbx::set_text(wm, h, num(3) as usize, &hexs(v[4])),
            "cbadd" => {
                cbx::add_entry(wm, h, num(3) as usize, &hexs(v[4]), num(5) as i32);
            }
            "anim" => h.shell.animate.needs_update = num(3) != 0,
            other => panic!("nd {other}"),
        },
        other => panic!("@ {other}"),
    }
}

/// `W` of a combo box (type 4) and the combo's own operations, which `ui4_common::scene_op` does not carry; false when `op` is neither.
fn combo_op(wm: &mut GameWindowManager, h: &mut Host, op: &str) -> bool {
    let v: Vec<&str> = op.split(' ').collect();
    match v[0] {
        "W" if v[2] == "4" => {
            let n: Vec<i64> = v[1..].iter().filter(|s| s.parse::<i64>().is_ok()).map(|s| s.parse().unwrap()).collect();
            let (parent, x, y, ww, hh) = (n[0] as i32, n[2] as i32, n[3] as i32, n[4] as i32, n[5] as i32);
            let (status, style, visual) = (n[6] as u32, n[7] as u32, n[8] != 0);
            let f: Vec<i32> = n[9..17].iter().map(|&x| x as i32).collect();
            let d = ComboBoxData { is_editable: f[0] != 0, max_display: f[1], max_chars: f[2], ascii_only: f[3] != 0, letters_and_numbers_only: f[4] != 0 };
            let mut t = cbx::combo_template(&d);
            t.listbox.list_length = f[5] as i16;
            t.listbox.scroll_bar = f[6] != 0;
            let mut inst = WinInstanceData { style, ..WinInstanceData::new() };
            inst.font = (f[7] != 0).then(|| GameFont { name: b"F".to_vec(), point_size: f[7], bold: false });
            let p = (parent >= 0).then_some(parent as usize);
            cbx::gogo_gadget_combo_box(wm, h, p, status, x, y, ww, hh, inst, t, visual);
            true
        }
        "s" => {
            cbx::set_selected_pos(wm, h, v[1].parse().unwrap(), v[2].parse().unwrap(), v[3] != "0");
            true
        }
        "r" => {
            cbx::reset(wm, h, v[1].parse().unwrap());
            true
        }
        _ => false,
    }
}

#[test]
fn network_direct_connect_matches_upstream() {
    let _lock = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_C5_01);
    let mut chk = Check::new("NetworkDirectConnect");
    let menu_funcs: [(&[u8], &[u8]); 1] = [(SYS, INP)];
    let _ = tex::set_text::<Host>;
    for case in 0..cases(3_000) as u64 {
        let scene = gen_scene(&mut g);
        let steps = 4 + g.below(19);
        let mut ops = scene.ops.clone();
        for _ in 0..steps {
            let o = gen_op(&mut g, &scene);
            ops.extend(o.split('\n').map(str::to_string));
        }
        let mut wm = GameWindowManager::new((800, 600));
        wm.transitions.groups.push(TransitionGroup { name: GROUP.to_vec(), ..Default::default() });
        let mut h = Host::default();
        let mut chunks = Vec::new();
        let mut run = ops.len();
        for (i, op) in ops.iter().enumerate() {
            h.shell.pending_push = true;
            if !h.shell.screen_stack.is_empty() {
                h.shell.screen_stack = vec![h.layout];
            }
            h.shell.pending_push_name = b"pushed".to_vec();
            if op.starts_with("@ nd") || op.starts_with("@ sys") || op.starts_with("@ inp") {
                menu_op(&mut wm, &mut h, op);
            } else if !combo_op(&mut wm, &mut h, op) {
                let mut keys = std::mem::take(&mut h.keys);
                let ok = scene_op(&mut wm, &mut h, &mut keys, &menu_funcs, op);
                h.keys = keys;
                assert!(ok, "unknown op {op}");
            }
            if wm.undefined.is_some() || h.ub {
                run = i;
                break;
            }
            let log = std::mem::take(&mut h.log);
            let st = state_dump(&wm, &h);
            chunks.push(format!("#{i}\n{log}{st}"));
        }
        ops.truncate(run);
        compare_case(env!("UI5_NDC_EXE"), &mut chk, case, &ops, &chunks);
    }
    finish(chk);
}
