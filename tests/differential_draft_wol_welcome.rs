// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ui3a) of the statistics and message-of-the-day half of the draft
//! `src/drafts/ui/wol_welcome_menu.rs` (`update_server_display`, `enable_controls`, `handle_num_players_online` with `update_num_players_online`,
//! `grab_ubyte`, `handle_overall_stats` with `find_next_number` and `atof`, `update_overall_stats`) against upstream `WOLWelcomeMenu.cpp`'s
//! `updateServerDisplay`, `enableControls`, `HandleNumPlayersOnline`, `updateNumPlayersOnline`, `grabUByte`, `HandleOverallStats`,
//! `FindNextNumber` and `updateOverallStats`, cut verbatim by build.rs (`drafts_ui3a`; `differential/wrap_draft_wol_welcome.cpp` lists the cuts and
//! every stand-in).
//!
//! # Method
//!
//! A case is a subset of the windows the functions look up (the players-online static, the list box, the server name static, the two
//! buttons, three check boxes), the game text (`GUI:NumPlayersOnline`, `MOTD:NumPlayersHeading`, `GUI:WinPercent`), the message of the day (lines of
//! plain text, `\\` escapes, `\AARRGGBB` colour codes, blanks, carriage returns, tabs and vertical tabs, bytes of 0x80 and more), the player
//! templates (sides, playable flags, a duplicate), the colours, and 1 to 8 operations: `HandleNumPlayersOnline(n)`, `HandleOverallStats(text)` (a
//! page with `Today`, the sides in random order, three numeric lines each, now and then missing, with decimals, leading zeros, an exponent),
//! `updateOverallStats()`, `updateServerDisplay(text)` and `enableControls(state)`. The Rust side runs the draft on the REAL window manager and
//! gadgets (a synthetic WND), the C++ side upstream's text on the stand-ins; after each op both print the statics (`lastNumPlayersOnline`,
//! `s_totalWinPercent` and each `s_winStats` entry by their float bits, `gServerName`), the two static texts, the buttons' enabled flags, the list
//! box's rows with their colours, and the check boxes' texts.
//!
//! Excluded inputs (upstream is undefined there): the page always has a digit after the newline `FindNextNumber` looks for, the page is
//! ASCII, and no game text format has a directive but `%d` and `%%`.
//!
//! # Predictions (written before the first run)
//!
//! Expected mismatches from reading the draft against upstream: (1) `trim` of the message's lines: the draft strips `is_ascii_whitespace`, which leaves
//! the vertical tab (0x0B) that C's `isspace` strips; (2) the draft's `atof` does not read an exponent or a leading `+`, which C's `atof` does. All
//! else expected to match.
//!
//! # Planted bugs (`L9_PLANT=<id>`; mismatching cases of 4,000, the run of the final build)
//!
//! | id | planted bug | cases |
//! |---|---|---|
//! | 20694 | `HandleNumPlayersOnline` clamps below 0, not below 1 | 593 |
//! | 20695 | `grabUByte` reads one hex digit | 676 |
//! | 20696 | the MOTD colour's red and blue swapped | 575 |
//! | 20697 | a colour-coded line is one of 10 or more characters, not 9 or more | 355 |
//! | 20698 | the percentage divides by the total with no `max(1, .)` | 412 |
//! | 20699 | `updateOverallStats` rounds instead of truncating | 111 |
//! | 20700 | the zero total is not replaced by 1 | 1,821 |
//! | 20701 | `enableControls` leaves the lobby button alone | 692 |
//! | 20702 | `updateServerDisplay` keeps the name only when the window exists | 130 |
//! | 20703 | a `\\` escape drops two characters, not one | 481 |
//!
//! 10 of 10 caught (20697 was missed at first: no generated colour line had exactly nine characters; the generator now makes some).
//!
//! # Results (2026-11-21, `DRAFT_CASES` 20,000, the default)
//!
//! **First run: 474 of 3,000 cases mismatched**, as predicted: the trim of the message's lines (the vertical tab). After fixing the draft's trim
//! (C's `isspace`): **110 of 3,000**, the second prediction, the exponent of `atof` (`5e1`). After fixing `atof`: 20,000 cases (90,121 operations,
//! 10,619 with list rows), **0 mismatches**. Two draft fixes, both struck through in `wol_welcome_menu.rs`.

mod support_ui3a;

use rts_lockstep_engine::client::gadget;
use rts_lockstep_engine::client::gadget_list_box as lb;
use rts_lockstep_engine::client::shell::{KeyedFiles, LayoutFiles, NameKeys, Shell, ShellEnv, ShellHost, ShellRequest, WndFiles};
use rts_lockstep_engine::client::window_manager::{GameWindowManager, GuiHost};
use rts_lockstep_engine::client::window_script::{win_status, WinId, WndHost};
use rts_lockstep_engine::drafts::ui::wol_welcome_menu as wel;
use std::collections::BTreeMap;
use std::ffi::CString;
use support_ui3a::Rng;

extern "C" {
    #[cfg_attr(u3a_dev, link_name = "ref_u3a_welcome_run_dev")]
    fn ref_u3a_welcome_run(input: *const std::os::raw::c_char, out: *mut u8, cap: i32) -> i32;
}

static CPP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const F: &str = "WOLWelcomeMenu.wnd";
const SIDES: [&str; 3] = ["USA", "China", "GLA"];

fn wnd(flags: i32) -> Vec<u8> {
    let w = |kind: &str, name: &str, extra: &str| {
        format!("WINDOW\n  WINDOWTYPE = {kind};\n  SCREENRECT = UPPERLEFT: 10 10, BOTTOMRIGHT: 210 40, CREATIONRESOLUTION: 800 600;\n  NAME = \"{F}:{name}\";\n  STATUS = ENABLED;\n  STYLE = {kind};\n{extra}END\n")
    };
    let mut s = String::from("FILE_VERSION = 2;\nSTARTLAYOUTBLOCK\n  LAYOUTINIT = [None];\n  LAYOUTUPDATE = [None];\n  LAYOUTSHUTDOWN = [None];\nENDLAYOUTBLOCK\n");
    let st = "  STATICTEXTDATA = CENTERED: 0;\n";
    if flags & 1 != 0 {
        s += &w("STATICTEXT", "StaticTextNumPlayersOnline", st);
    }
    if flags & 2 != 0 {
        s += &w("SCROLLLISTBOX", "ListboxInfo", "  LISTBOXDATA = LENGTH: 100, AUTOSCROLL: 0, AUTOPURGE: 0, SCROLLBAR: 0, MULTISELECT: 0, COLUMNS: 1, COLUMNSWIDTH%: 100, FORCESELECT: 0;\n");
    }
    if flags & 4 != 0 {
        s += &w("STATICTEXT", "StaticTextServerName", st);
    }
    if flags & 8 != 0 {
        s += &w("PUSHBUTTON", "ButtonQuickMatch", "");
    }
    if flags & 16 != 0 {
        s += &w("PUSHBUTTON", "ButtonLobby", "");
    }
    for (i, side) in SIDES.iter().enumerate() {
        if flags & (32 << i) != 0 {
            s += &w("CHECKBOX", &format!("Percent{side}"), "");
        }
    }
    s.into_bytes()
}

#[derive(Clone, Default)]
struct Files {
    wnd: Vec<u8>,
}
impl WndHost for Files {
    fn name_to_key(&mut self, _name: &[u8]) -> u32 {
        unreachable!("the keys are the host's (KeyedFiles)")
    }
    fn find_image(&self, _name: &[u8]) -> Option<Vec<u8>> {
        None
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        b"T:".iter().chain(label).map(|&b| u16::from(b)).collect()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
}
impl WndFiles for Files {
    fn open_wnd(&mut self, _path: &[u8]) -> Option<Vec<u8>> {
        Some(self.wnd.clone())
    }
}

struct Host {
    shell: Shell,
    keys: NameKeys,
    menu: wel::WolWelcomeMenu,
    texts: BTreeMap<Vec<u8>, Vec<u8>>,
    motd: Vec<u8>,
    has_info: bool,
    colors: (i32, i32),
    templates: Vec<(Vec<u8>, bool)>,
}

fn u16s(b: &[u8]) -> Vec<u16> {
    b.iter().map(|&c| u16::from(c)).collect()
}

impl GuiHost for Host {}

impl ShellHost for Host {
    fn shell(&mut self) -> &mut Shell {
        &mut self.shell
    }
    fn create_layout(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<usize> {
        None
    }
    fn create_from_script(&mut self, _wm: &mut GameWindowManager, _filename: &[u8]) -> Option<WinId> {
        None
    }
    fn layout_callback(&mut self, _wm: &mut GameWindowManager, _func: &[u8], _layout: usize, _user: Option<bool>) {}
    fn time_ms(&self) -> u32 {
        0
    }
    fn env(&self) -> ShellEnv {
        ShellEnv::default()
    }
    fn request(&mut self, _r: ShellRequest) {}
    fn name_to_key(&mut self, name: &[u8]) -> u32 {
        self.keys.name_to_key(name)
    }
}

impl wel::WolWelcomeMenuHost for Host {
    fn wol_welcome_menu(&mut self) -> &mut wel::WolWelcomeMenu {
        &mut self.menu
    }
    fn fetch_text(&self, label: &[u8]) -> Vec<u16> {
        match self.texts.get(label) {
            Some(t) => u16s(t),
            None => b"T:".iter().chain(label).map(|&b| u16::from(b)).collect(),
        }
    }
    /// `_vsnwprintf` with one `int`: `%d` is the number, `%%` a percent sign (the cases have no other directive).
    fn format_int(&self, fmt: &[u16], v: i32) -> Vec<u16> {
        let mut o = Vec::new();
        let mut i = 0;
        while i < fmt.len() {
            if fmt[i] == u16::from(b'%') && i + 1 < fmt.len() && fmt[i + 1] == u16::from(b'd') {
                o.extend(v.to_string().encode_utf16());
                i += 2;
            } else if fmt[i] == u16::from(b'%') && i + 1 < fmt.len() && fmt[i + 1] == u16::from(b'%') {
                o.push(u16::from(b'%'));
                i += 2;
            } else {
                o.push(fmt[i]);
                i += 1;
            }
        }
        o
    }
    fn format_text_arg(&self, _fmt: &[u16], _text: &[u8]) -> Vec<u16> {
        unreachable!("not reached by these functions")
    }
    fn gamespy_color_motd_heading(&self) -> i32 {
        self.colors.0
    }
    fn gamespy_color_motd(&self) -> i32 {
        self.colors.1
    }
    fn has_gamespy_info(&self) -> bool {
        self.has_info
    }
    fn gamespy_motd(&self) -> Vec<u8> {
        self.motd.clone()
    }
    fn gamespy_local_base_name(&self) -> Vec<u8> {
        unreachable!()
    }
    fn gamespy_got_group_room_list(&self) -> bool {
        unreachable!()
    }
    fn gamespy_max_messages_per_update(&self) -> i32 {
        unreachable!()
    }
    fn gamespy_local_profile_id(&self) -> i32 {
        unreachable!()
    }
    fn gamespy_local_name(&self) -> Vec<u16> {
        unreachable!()
    }
    fn gamespy_add_group_room(&mut self, _a: i32, _b: i32, _n: &[u8], _c: i32, _d: i32, _e: i32) {
        unreachable!()
    }
    fn gamespy_set_current_group_room(&mut self, _id: i32) {
        unreachable!()
    }
    fn gamespy_join_best_group_room(&mut self) {
        unreachable!()
    }
    /// `MultiByteToWideCharSingleLine`: Windows' `MultiByteToWideChar`; each byte becomes the unit of that value.
    fn multi_byte_to_wide_char(&self, line: &[u8]) -> Vec<u16> {
        u16s(line)
    }
    fn player_template_sides(&self) -> Vec<(Vec<u8>, bool)> {
        self.templates.clone()
    }
    fn populate_player_info_windows(&mut self, _wm: &mut GameWindowManager, _parent_name: &[u8]) {
        unreachable!()
    }
    fn firewall_helper_exists(&self) -> bool {
        unreachable!()
    }
    fn firewall_helper_create(&mut self) {
        unreachable!()
    }
    fn firewall_helper_detect_firewall(&mut self) -> bool {
        unreachable!()
    }
    fn firewall_helper_delete(&mut self) {
        unreachable!()
    }
    fn firewall_helper_behavior_detection_update_and_store(&mut self) -> bool {
        unreachable!()
    }
    fn gamespy_open_overlay_locale_select(&mut self, _wm: &mut GameWindowManager) {
        unreachable!()
    }
    fn gamespy_open_overlay_options(&mut self, _wm: &mut GameWindowManager) {
        unreachable!()
    }
    fn gamespy_toggle_overlay_player_info(&mut self, _wm: &mut GameWindowManager) {
        unreachable!()
    }
    fn gamespy_toggle_overlay_buddy(&mut self, _wm: &mut GameWindowManager) {
        unreachable!()
    }
    fn gamespy_close_all_overlays(&mut self, _wm: &mut GameWindowManager) {
        unreachable!()
    }
    fn raise_gs_message_box(&mut self) {
        unreachable!()
    }
    fn gs_message_box_ok(&mut self, _wm: &mut GameWindowManager, _title: &[u16], _body: &[u16]) {
        unreachable!()
    }
    fn set_look_at_player(&mut self, _profile_id: i32, _name: &[u16]) {
        unreachable!()
    }
    fn misc_prefs_locale(&mut self) -> i32 {
        unreachable!()
    }
    fn misc_prefs_quick_match_res_locked(&mut self) -> bool {
        unreachable!()
    }
    fn display_size(&self) -> (i32, i32) {
        (800, 600)
    }
    fn handle_buddy_responses(&mut self) {
        unreachable!()
    }
    fn handle_persistent_storage_responses(&mut self) {
        unreachable!()
    }
    fn has_peer_message_queue(&self) -> bool {
        unreachable!()
    }
    fn peer_next_response(&mut self) -> Option<wel::WelcomePeerResponse> {
        unreachable!()
    }
    fn peer_buddy_logout_and_tear_down(&mut self) {
        unreachable!()
    }
}

fn hex(b: &[u8]) -> String {
    if b.is_empty() {
        "-".into()
    } else {
        b.iter().map(|c| format!("{c:02x}")).collect()
    }
}
fn whex(t: &[u16]) -> String {
    if t.is_empty() {
        "-".into()
    } else {
        t.iter().map(|c| format!("{c:04x}")).collect()
    }
}

#[derive(Clone, Debug)]
struct Case {
    flags: i32,
    colors: (i32, i32),
    texts: Vec<(String, String)>,
    motd: Vec<u8>,
    templates: Vec<(Vec<u8>, bool)>,
    ops: Vec<String>,
}

fn text_of(c: &Case) -> String {
    let mut t = format!("flags {}\ncolors {} {}\n", c.flags, c.colors.0, c.colors.1);
    for (k, v) in &c.texts {
        t += &format!("text {k} {}\n", hex(v.as_bytes()));
    }
    t += &format!("motd {}\n", hex(&c.motd));
    for (s, p) in &c.templates {
        t += &format!("tpl {} {}\n", i32::from(*p), hex(s));
    }
    for o in &c.ops {
        t += &format!("{o}\n");
    }
    t
}

fn rust_run(c: &Case) -> String {
    let mut texts = BTreeMap::new();
    for (k, v) in &c.texts {
        texts.insert(k.as_bytes().to_vec(), v.as_bytes().to_vec());
    }
    let mut h = Host { shell: Shell::with_display_width(800), keys: NameKeys::default(), menu: Default::default(), texts, motd: c.motd.clone(), has_info: c.flags & 256 != 0, colors: c.colors, templates: c.templates.clone() };
    let mut wm = GameWindowManager::new((800, 600));
    let mut files = LayoutFiles::new(Files { wnd: wnd(c.flags) });
    {
        let data = files.files.wnd.clone();
        let mut kf = KeyedFiles { files: &mut files.files, keys: h.keys.clone() };
        // (the reader's own error, if any, is a bug of this test's WND)
        let r = wm.win_create_from_script(&mut files.script, b"Menus/x.wnd", &data, &mut kf, &mut h);
        if let Err(e) = &r {
            panic!("the synthetic WND failed to load: {e:?}\n{}", String::from_utf8_lossy(&data));
        }
    }
    if std::env::var("DRAFT_DEBUG").is_ok() {
        eprintln!("windows: {:?}", wm.tree.windows.iter().map(|w| String::from_utf8_lossy(w.name()).into_owned()).collect::<Vec<_>>());
    }
    let find = |wm: &GameWindowManager, n: &str| wm.tree.find_by_name(format!("{F}:{n}").as_bytes());
    let num = find(&wm, "StaticTextNumPlayersOnline");
    let list = find(&wm, "ListboxInfo");
    let server = find(&wm, "StaticTextServerName");
    let qm = find(&wm, "ButtonQuickMatch");
    let lobby = find(&wm, "ButtonLobby");
    let checks: Vec<Option<WinId>> = SIDES.iter().map(|s| find(&wm, &format!("Percent{s}"))).collect();
    h.menu.listbox_info = list;
    h.menu.static_text_server_name = server;
    h.menu.button_quick_match = qm;
    h.menu.button_lobby = lobby;
    let mut out = String::new();
    for op in &c.ops {
        out += &format!("> {op}\n");
        let (cmd, rest) = op.split_once(' ').map_or((op.as_str(), ""), |(a, b)| (a, b));
        match cmd {
            "num" => wel::handle_num_players_online(&mut wm, &mut h, rest.parse().expect("number")),
            "stats" => {
                let t: Vec<u8> = (0..rest.len() / 2).map(|i| u8::from_str_radix(&rest[2 * i..2 * i + 2], 16).expect("hex")).collect();
                wel::handle_overall_stats(&mut h, &t);
            }
            "upd" => wel::update_overall_stats(&mut wm, &mut h),
            "server" => {
                let u: Vec<u16> = if rest == "-" { Vec::new() } else { (0..rest.len() / 4).map(|i| u16::from_str_radix(&rest[4 * i..4 * i + 4], 16).expect("hex")).collect() };
                wel::update_server_display(&mut wm, &mut h, &u);
            }
            _ => wel::enable_controls(&mut wm, &mut h, rest == "1"),
        }
        let m = &h.menu;
        out += &format!("last {} total {:08x} server [{}]\n", m.last_num_players_online, m.total_win_percent.to_bits(), whex(&m.g_server_name));
        let st = |w: Option<WinId>, wm: &GameWindowManager| w.map_or("absent".to_string(), |w| whex(&gadget::static_text_get_text(wm, w)));
        out += &format!("numwin [{}] server [{}]\n", st(num, &wm), st(server, &wm));
        let en = |w: Option<WinId>| w.map_or(-1, |w| i32::from(wm.tree.windows[w].status & win_status::ENABLED != 0));
        out += &format!("enabled qm {} lobby {}\n", en(qm), en(lobby));
        let mut rows = String::new();
        let n = list.map_or(0, |l| lb::get_num_entries(&wm, l));
        if let Some(l) = list {
            for r in 0..n {
                let (t, col) = lb::get_text_and_color(&mut wm, &mut h, l, r, 0);
                rows += &format!(" [{}]{{{col}}}", whex(&t));
            }
        }
        out += &format!("list {n}{rows}\n");
        let mut s = String::new();
        for (side, frac) in &h.menu.win_stats {
            s += &format!(" {}={:08x}", String::from_utf8_lossy(side), frac.to_bits());
        }
        out += &format!("stats{s}\n");
        let mut ck = String::new();
        for w in &checks {
            ck += &format!(" [{}]", w.map_or("absent".to_string(), |w| whex(wm.win_get_text(w))));
        }
        out += &format!("checks{ck}\n");
    }
    out
}

fn cpp_run(text: &str) -> String {
    let c = CString::new(text).expect("no NUL");
    let mut out = vec![0u8; 1 << 22];
    let n = unsafe { ref_u3a_welcome_run(c.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
    assert!((n as usize) <= out.len(), "the log overflowed");
    String::from_utf8_lossy(&out[..n as usize]).into_owned()
}

fn pick<'a>(r: &mut Rng, v: &[&'a str]) -> &'a str {
    v[r.below(v.len() as u64) as usize]
}

fn gen_number(r: &mut Rng) -> String {
    match r.below(12) {
        0 => "0".into(),
        1 => format!("{}.{}", r.below(100), r.below(1000)),
        2 => format!("00{}", r.below(50)),
        3 => format!("{}.5", r.below(9)),
        4 => format!("{}", r.below(100_000)),
        5 if r.chance(3) => format!("{}e{}", 1 + r.below(8), r.below(3)),
        _ => format!("{}", r.below(1000)),
    }
}

fn gen_stats(r: &mut Rng) -> Vec<u8> {
    let mut t = String::new();
    t += pick(r, &["Daily stats\n", "", "Yesterday 12\n", "Total games 99\n"]);
    if !r.chance(5) {
        t += &format!("Today {}\n", pick(r, &["", "and so on", "(UTC)"]));
    }
    let mut sides = vec!["USA", "China", "GLA", "Observer"];
    for i in (1..sides.len()).rev() {
        sides.swap(i, r.below(i as u64 + 1) as usize);
    }
    for s in sides {
        if r.chance(6) {
            continue;
        }
        t += &format!("{s} {}\n{}\n{}\n", pick(r, &["", "games", "team"]), gen_number(r), gen_number(r));
    }
    t.into_bytes()
}

fn gen_motd(r: &mut Rng) -> Vec<u8> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    for _ in 0..r.below(9) {
        let body: Vec<u8> = match r.below(14) {
            0 => Vec::new(),
            1 => b"   ".to_vec(),
            2 => b"\t\x0b indented \x0b\t".to_vec(),
            3 => b"\\\\escaped text".to_vec(),
            4 => b"\\short".to_vec(),
            5 | 6 => {
                let digits = |r: &mut Rng| -> String { (0..8).map(|_| char::from_digit(r.below(16) as u32, 16).expect("digit")).collect() };
                let mut d = if r.chance(5) { "ffxx00zz".to_string() } else { digits(r) };
                if r.chance(4) {
                    d = d.to_uppercase();
                }
                format!("\\{d}{}", pick(r, &["Colored", " spaced", "x", "", "", "More text here"])).into_bytes()
            }
            7 => vec![0xE9, b't', 0xE9, 0xFF, 0x80],
            _ => format!("line {} {}", r.below(100), pick(r, &["of the day", "news", "  trailing  "])).into_bytes(),
        };
        let mut l = body;
        if r.chance(4) {
            l.push(b'\r');
        }
        lines.push(l);
    }
    let mut out = Vec::new();
    for l in lines {
        out.extend(l);
        out.push(b'\n');
        if r.chance(6) {
            out.push(b'\n');
        }
    }
    out
}

fn gen_case(r: &mut Rng) -> Case {
    let mut flags = if r.below(4) == 0 { r.below(512) as i32 } else { 511 };
    if r.chance(10) {
        flags &= !256;
    }
    let mut texts = Vec::new();
    texts.push(("GUI:NumPlayersOnline".to_string(), pick(r, &["Players online: %d", "%d", "", "no number", "%d players", "%%"]).to_string()));
    texts.push((
        "MOTD:NumPlayersHeading".to_string(),
        pick(r, &["Welcome\r\n  to the lobby  \n\n\nthree", "", "   ", "\n\n", "a\rb", "Top\r\n", "x\ty", "one\n two \nthree\n", "\x0bvt\x0b\n"]).to_string(),
    ));
    texts.push(("GUI:WinPercent".to_string(), pick(r, &["%d%%", "%d", "Win %d percent", "", "n/a"]).to_string()));
    texts.retain(|_| !r.chance(8));
    let mut templates = Vec::new();
    for _ in 0..r.below(7) {
        templates.push((pick(r, &["America", "China", "GLA", "Boss", "Observer", "America", "USA"]).as_bytes().to_vec(), !r.chance(5)));
    }
    let mut ops = Vec::new();
    for _ in 0..1 + r.below(8) {
        ops.push(match r.below(10) {
            0..=2 => format!("num {}", [-5, 0, 1, 2, 7, 100, 12345, i32::MAX][r.below(8) as usize]),
            3..=5 => format!("stats {}", hex(&gen_stats(r))),
            6 | 7 => "upd".to_string(),
            8 => format!("server {}", if r.chance(4) { "-".into() } else { whex(&u16s(pick(r, &["Server One", "x", "\u{e9}", " "]).as_bytes())) }),
            _ => format!("enable {}", r.below(2)),
        });
    }
    Case { flags, colors: (r.below(1 << 24) as i32 - 1000, r.below(1 << 24) as i32), texts, motd: gen_motd(r), templates, ops }
}

#[test]
fn the_welcome_menu_statistics_match_upstream() {
    let _lock = CPP_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: usize = std::env::var("DRAFT_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(20_000);
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut r = Rng(0x215_0B_0001);
    let (mut mismatches, mut ops, mut with_rows) = (0usize, 0usize, 0usize);
    for n in 0..cases {
        let c = gen_case(&mut r);
        let text = text_of(&c);
        let rust = rust_run(&c);
        let cpp = cpp_run(&text);
        ops += c.ops.len();
        with_rows += usize::from(rust.lines().any(|l| l.starts_with("list ") && l != "list 0"));
        if rust != cpp {
            mismatches += 1;
            if mismatches <= 2 {
                let at = rust.lines().zip(cpp.lines()).position(|(x, y)| x != y).unwrap_or(rust.lines().count().min(cpp.lines().count()));
                let (rl, cl) = (rust.lines().nth(at).unwrap_or(""), cpp.lines().nth(at).unwrap_or(""));
                let col = rl.chars().zip(cl.chars()).position(|(x, y)| x != y).unwrap_or(rl.len().min(cl.len()));
                let win = |s: &str| s.chars().skip(col.saturating_sub(60)).take(160).collect::<String>();
                println!("case {n}:\n{text}\n  first difference at line {at}, column {col}:\n  rust: {:?}\n  c++:  {:?}\n  before: {:?}", win(rl), win(cl), rust.lines().skip(at.saturating_sub(4)).take(4).map(|l| l.chars().take(100).collect::<String>()).collect::<Vec<_>>().join(" | "));
            }
        }
    }
    println!("DIFF WOL welcome statistics compared {cases} mismatches {mismatches} ({ops} ops, {with_rows} cases with list rows)");
    if planted {
        println!("MEASURED mismatches {mismatches}");
    }
    assert_eq!(mismatches, 0);
}
