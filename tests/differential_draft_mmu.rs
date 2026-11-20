#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/GameSpy/MainMenuUtils.cpp and Include/GameNetwork/DownloadManager.h (QueuedDownload)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/main_menu_utils.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_mmu.cpp` compiles upstream's `MainMenuUtils.cpp` (all of it but `asyncGethostbynameThreadFunc` and `asyncGethostbyname`, a winsock
//! thread) and `QueuedDownload`, cut verbatim by build.rs (`drafts_net4`), on upstream's own `AsciiString.cpp`/`UnicodeString.cpp`. Everything the file calls
//! outside itself (GameSpy's http SDK, windows and message boxes, the shell, the script engine, the download manager, the registry, `Config.txt`, the DNS
//! thread) is a stand-in listed in the wrapper that logs what it is asked and answers from the case's configuration; this file's `Host` answers and logs
//! identically. An http request stays pending until the program delivers it (so callbacks run in the program's order, old ones after a new
//! `StartPatchCheck` included), and a message box stays open until the program presses one of its buttons. A program is a configuration (registry,
//! proxy, URLs, DNS polls, whether HTTP throws, the saved config file) and 40 to 120 operations (opcodes at `ref_mmu_run`) of: `StartPatchCheck`, delivering a
//! pending answer (success or failure, with a body from a generator per request kind: patch lists with `patch`/`server` lines and good and bad URLs,
//! configs around the 100-byte limit, messages of the day, stats texts, players-online texts, `Content-Length` headers equal and unequal to the saved file),
//! `HTTPThinkWrapper`, `StopAsyncDNSCheck`, button presses, the stats fetches, `CancelPatchCheckCallback`, `StartDownloadingPatches`, reconfiguring, and
//! `queuePatch` with URLs with and without credentials. After every operation the log and the whole file-level state (`state_words`, a test accessor added
//! to the draft) and the config file are compared word by word.
//!
//! Excluded input: a message of the day of length 0 (upstream writes `MOTDBuffer[-1]`), a saved config file of length 0 whose `Content-Length` is 0
//! (upstream writes `configBuffer[-1]`). Not covered: the winsock thread itself (the draft's host answers the lookup), `hasWriteAccess`'s file system calls
//! (stand-ins), `ghttp` and the registry.

use rts_lockstep_engine::drafts::network::download_manager::QueuedDownload;
use rts_lockstep_engine::drafts::network::main_menu_utils::*;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_mmu_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn put_str(w: &mut Vec<u32>, s: &[u8]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&b| b as u32));
}

fn c_string(b: &[u8]) -> &[u8] {
    match b.iter().position(|&c| c == 0) {
        Some(p) => &b[..p],
        None => b,
    }
}

fn button_id(b: MenuButton) -> u32 {
    match b {
        MenuButton::PatchBeforeOnline => 0,
        MenuButton::NoPatchBeforeOnline => 1,
        MenuButton::CancelPatchCheckAndReopenDropdown => 2,
    }
}

fn kind_id(k: HttpRequestKind) -> u32 {
    match k {
        HttpRequestKind::PatchCheck => 0,
        HttpRequestKind::ConfigHead => 1,
        HttpRequestKind::Config => 2,
        HttpRequestKind::Motd => 3,
        HttpRequestKind::OverallStats => 4,
        HttpRequestKind::NumPlayersOnline => 5,
    }
}

struct Host {
    log: RefCell<Vec<u32>>,
    pending: Vec<(HttpRequestKind, i32)>,
    boxes: Vec<MenuButton>,
    headers: Vec<u8>,
    can_create: bool,
    version_get: bool,
    version_set: bool,
    version: u32,
    have_proxy: bool,
    proxy: String,
    think_throws: bool,
    has_dm: bool,
    dns_polls: u32,
    dns_succeeds: bool,
    urls: PatchUrls,
    file: Option<Vec<u8>>,
    // the DNS thread miniature
    dns_stat: u32,
    dns_handle: bool,
    dns_count: u32,
}

impl Host {
    fn l(&self, w: &[u32]) {
        self.log.borrow_mut().extend_from_slice(w);
    }
    fn ls(&self, s: &[u8]) {
        put_str(&mut self.log.borrow_mut(), s);
    }
    fn box_log(&mut self, kind: u32, title: &str, text: &str, buttons: &[MenuButton]) {
        self.l(&[20, kind]);
        self.l(&[title.len() as u32]);
        self.l(&title.bytes().map(|b| b as u32).collect::<Vec<_>>());
        self.l(&[text.len() as u32]);
        self.l(&text.bytes().map(|b| b as u32).collect::<Vec<_>>());
        for b in buttons {
            self.l(&[button_id(*b)]);
        }
        self.boxes = buttons.to_vec();
    }
}

impl MainMenuHost for Host {
    fn http_get(&mut self, url: &str, kind: HttpRequestKind, run: i32) {
        self.l(&[1, kind_id(kind)]);
        self.ls(url.as_bytes());
        self.l(&[run as u32]);
        self.pending.push((kind, run));
    }
    fn http_head(&mut self, url: &str, kind: HttpRequestKind, run: i32) {
        self.l(&[2, kind_id(kind)]);
        self.ls(url.as_bytes());
        self.l(&[run as u32]);
        self.pending.push((kind, run));
    }
    fn http_think(&mut self) -> bool {
        self.l(&[3]);
        !self.think_throws
    }
    fn http_set_proxy(&mut self, proxy: &str) {
        self.l(&[4]);
        self.ls(proxy.as_bytes());
    }
    fn http_get_headers(&self) -> Vec<u8> {
        self.headers.clone()
    }
    fn format_url_from_registry(&self) -> PatchUrls {
        self.l(&[53]);
        self.urls.clone()
    }
    fn registry_proxy(&self) -> Option<String> {
        self.l(&[52]);
        if self.have_proxy {
            Some(self.proxy.clone())
        } else {
            None
        }
    }
    fn can_create_test_file(&mut self) -> bool {
        self.l(&[54]);
        self.can_create
    }
    fn registry_version_get(&self) -> Option<u32> {
        self.l(&[50]);
        if self.version_get {
            Some(self.version)
        } else {
            None
        }
    }
    fn registry_version_set(&mut self, val: u32) -> bool {
        self.l(&[51, val]);
        self.version_set
    }
    fn write_config_file(&mut self, data: &[u8]) {
        self.file = Some(data.to_vec());
    }
    fn read_config_file(&self) -> Option<Vec<u8>> {
        self.file.clone()
    }
    fn message_box_ok(&mut self, title_label: &str, text_label: &str, on_ok: MenuButton) {
        self.box_log(0, title_label, text_label, &[on_ok]);
    }
    fn message_box_ok_cancel(&mut self, title_label: &str, text_label: &str, on_ok: MenuButton, on_cancel: MenuButton) {
        self.box_log(1, title_label, text_label, &[on_ok, on_cancel]);
    }
    fn message_box_yes_no(&mut self, title_label: &str, text_label: &str, on_yes: MenuButton, on_no: MenuButton) {
        self.box_log(2, title_label, text_label, &[on_yes, on_no]);
    }
    fn message_box_cancel(&mut self, title_label: &str, text_label: &str, on_cancel: MenuButton) {
        self.box_log(3, title_label, text_label, &[on_cancel]);
    }
    fn destroy_online_cancel_window(&mut self) {
        self.l(&[14, 1]);
    }
    fn show_download_menu(&mut self) {
        self.l(&[13]);
        self.ls(b"Menus/DownloadMenu.wnd");
        self.l(&[10, 11, 0, 12]);
    }
    fn handle_canceled_download(&mut self, reset_drop_down: bool) {
        self.l(&[33, reset_drop_down as u32]);
    }
    fn has_download_manager(&self) -> bool {
        self.has_dm
    }
    fn download_manager_queue(&mut self, q: &QueuedDownload) {
        self.l(&[40]);
        for s in [&q.server, &q.user_name, &q.password, &q.file, &q.local_file, &q.reg_key] {
            self.ls(s.str());
        }
        self.l(&[q.try_resume as u32]);
    }
    fn download_manager_start_next(&mut self) {
        self.l(&[41]);
    }
    fn signal_online_selected(&mut self) {
        self.l(&[30]);
        self.ls(b"MainMenuOnlineSelected");
    }
    fn set_up_game_spy(&mut self, motd: &[u8], config: &[u8]) {
        self.l(&[32]);
        self.ls(c_string(motd));
        self.ls(c_string(config));
    }
    fn push_login_profile_menu(&mut self) {
        self.l(&[31]);
        self.ls(b"Menus/GameSpyLoginProfile.wnd");
    }
    fn async_gethostbyname(&mut self, name: &str) -> LookupState {
        self.l(&[61]);
        self.ls(name.as_bytes());
        if self.dns_stat == 0 {
            self.dns_handle = true;
            self.dns_stat = 1;
            self.dns_count = 0;
        }
        if self.dns_stat == 1 {
            let c = self.dns_count;
            self.dns_count += 1;
            if c >= self.dns_polls {
                self.dns_stat = 0;
                self.dns_handle = false;
                return if self.dns_succeeds { LookupState::Succeeded } else { LookupState::Failed };
            }
        }
        LookupState::InProgress
    }
    fn terminate_dns_thread(&mut self) {
        if self.dns_handle {
            self.l(&[60]);
        }
        self.dns_handle = false;
    }
    fn handle_overall_stats(&mut self, buffer: &[u8]) {
        self.l(&[34]);
        self.ls(buffer);
    }
    fn handle_num_players_online(&mut self, n: i32) {
        self.l(&[35, n as u32]);
    }
}

fn word(g: &mut Gen, max: u32) -> Vec<u8> {
    (0..g.below(max + 1)).map(|_| (97 + g.below(26)) as u8).collect()
}

fn url_text(g: &mut Gen) -> Vec<u8> {
    const T: [&str; 14] = [
        "http://srv.example.com/dir/file.rtp",
        "ftp://user:pass@host.example.net/path/to/patch.exe",
        "http://host.example.com:8080/a/b/c.bin",
        "ftp://user@host/x.bin",
        "http://host",
        "nonsense",
        "ftp://u:p@h/",
        "http://h/p/a.rtp",
        "http://h/q/a.rtp",
        "ftp://:@h/f",
        "a:b",
        "http://h:80/",
        "http://h/dir/",
        "",
    ];
    if g.below(4) == 0 {
        word(g, 12)
    } else {
        T[g.below(T.len() as u32) as usize].as_bytes().to_vec()
    }
}

fn patch_text(g: &mut Gen) -> Vec<u8> {
    const L: [&str; 10] = ["patch 1 ", "patch 0 ", "patch -1 ", "patch 2 ", "server 1 ", "patch x ", "patch ", "garbage ", "Patch 1 ", "patch 1"];
    let mut out = Vec::new();
    for i in 0..g.below(5) {
        if i > 0 {
            out.extend_from_slice(match g.below(3) {
                0 => b"\r\n",
                1 => b"\n",
                _ => b"\r\n\r\n",
            });
        }
        out.extend_from_slice(L[g.below(L.len() as u32) as usize].as_bytes());
        out.extend_from_slice(&url_text(g));
    }
    if g.below(10) == 0 {
        out.push(0);
        out.extend_from_slice(b"patch 1 http://h/hidden");
    }
    out
}

fn players_text(g: &mut Gen) -> Vec<u8> {
    const T: [&str; 10] = ["\\\\12\n", "  \\34 ", "abc", "5\\", "\\", "\\\\ 7 ", "x\\-3", "\\\\99999999999", "", "  42  "];
    let mut v = T[g.below(T.len() as u32) as usize].as_bytes().to_vec();
    if g.below(8) == 0 {
        v.extend_from_slice(b"\\9");
    }
    v
}

fn blob(g: &mut Gen, min: u32, max: u32) -> Vec<u8> {
    let n = min + g.below(max - min + 1);
    (0..n).map(|_| if g.below(40) == 0 { 0 } else { (32 + g.below(95)) as u8 }).collect()
}

fn config_words(g: &mut Gen, host: &mut Host, input: &mut Vec<u32>) {
    host.can_create = g.below(5) > 0;
    host.version_get = g.below(5) > 0;
    host.version_set = g.below(5) > 0;
    host.version = g.u32();
    host.have_proxy = g.below(2) == 0;
    host.proxy = if g.below(4) == 0 { String::new() } else { format!("proxy{}:{}", g.below(9), 80 + g.below(100)) };
    host.think_throws = g.below(12) == 0;
    host.has_dm = g.below(6) > 0;
    host.dns_polls = g.below(3);
    host.dns_succeeds = g.below(4) > 0;
    host.urls = PatchUrls {
        game_url: format!("http://g{}.example.com/game", g.below(5)),
        map_url: format!("http://m{}.example.com/map", g.below(5)),
        config_url: format!("http://c{}.example.com/Config.txt", g.below(5)),
        motd_url: if g.below(5) == 0 { String::new() } else { format!("http://x{}.example.com/MOTD", g.below(5)) },
    };
    let file = if g.below(2) == 0 { None } else { Some(blob(g, 1, 220)) };
    input.extend_from_slice(&[host.can_create as u32, host.version_get as u32, host.version_set as u32, host.version, host.have_proxy as u32]);
    put_str(input, host.proxy.as_bytes());
    input.extend_from_slice(&[host.think_throws as u32, host.has_dm as u32, host.dns_polls, host.dns_succeeds as u32]);
    for u in [&host.urls.game_url, &host.urls.map_url, &host.urls.config_url, &host.urls.motd_url] {
        put_str(input, u.as_bytes());
    }
    input.push(file.is_some() as u32);
    put_str(input, file.as_deref().unwrap_or(b""));
    host.file = file;
}

fn dump_post(host: &Host, u: &MainMenuUtils) {
    let mut l = host.log.borrow_mut();
    l.extend(u.state_words());
    l.push(0xE1E1E1E1);
    l.push(host.file.is_some() as u32);
    put_str(&mut l, host.file.as_deref().unwrap_or(b""));
}

thread_local! { static TRACE: RefCell<Vec<String>> = RefCell::new(Vec::new()); }

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    TRACE.with(|t| t.borrow_mut().clear());
    let mut host = Host {
        log: RefCell::new(Vec::new()),
        pending: Vec::new(),
        boxes: Vec::new(),
        headers: Vec::new(),
        can_create: true,
        version_get: true,
        version_set: true,
        version: 0,
        have_proxy: false,
        proxy: String::new(),
        think_throws: false,
        has_dm: true,
        dns_polls: 1,
        dns_succeeds: true,
        urls: PatchUrls::default(),
        file: None,
        dns_stat: 0,
        dns_handle: false,
        dns_count: 0,
    };
    let mut u = MainMenuUtils::new();
    let nops = 40 + g.below(81);
    let mut input = vec![nops];
    // the configuration is operation 9, read first
    host.l(&[0xA5A5A5A5, 9]);
    config_words(g, &mut host, &mut input);
    dump_post(&host, &u);
    for _ in 0..nops {
        let op = match g.below(20) {
            0..=2 => 0,
            3..=9 => 1,
            10 | 11 => 2,
            12 => 3,
            13 | 14 => 4,
            15 => 5,
            16 => 6,
            17 => 7,
            _ => match g.below(3) {
                0 => 8,
                1 => 9,
                _ => 10,
            },
        };
        input.push(op);
        host.l(&[0xA5A5A5A5, op]);
        match op {
            0 => u.start_patch_check(&mut host),
            1 => {
                let idx = g.below(8);
                let ok = g.below(10) > 1;
                let kind = if host.pending.is_empty() { HttpRequestKind::PatchCheck } else { host.pending[(idx as usize) % host.pending.len()].0 };
                let buf: Vec<u8> = match kind {
                    HttpRequestKind::PatchCheck => patch_text(g),
                    HttpRequestKind::ConfigHead => Vec::new(),
                    HttpRequestKind::Config => {
                        if g.below(8) == 0 {
                            blob(g, 98, 102)
                        } else {
                            blob(g, 0, 200)
                        }
                    }
                    HttpRequestKind::Motd => blob(g, 1, 60),
                    HttpRequestKind::OverallStats => blob(g, 0, 40),
                    HttpRequestKind::NumPlayersOnline => players_text(g),
                };
                let headers: Vec<u8> = {
                    let len = match &host.file {
                        Some(f) if g.below(3) > 0 => f.len() as u32,
                        _ => g.below(300),
                    };
                    match g.below(5) {
                        0 => format!("HTTP/1.1 200 OK\r\nServer: x\r\nContent-Length: {}\r\n\r\n", len).into_bytes(),
                        1 => format!("Content-Length: {}", len).into_bytes(),
                        2 => b"Content-Length: \r\nX: y\r\n".to_vec(),
                        3 => format!("content-length: {}\r\nContent-Length:{}\r\n", len, len).into_bytes(),
                        _ => format!("X: 1\n\rContent-Length: {}\n", len).into_bytes(),
                    }
                };
                input.extend_from_slice(&[idx, if ok { 0 } else { 1 }]);
                put_str(&mut input, &buf);
                put_str(&mut input, &headers);
                host.headers = headers;
                if !host.pending.is_empty() {
                    let at = (idx as usize) % host.pending.len();
                    let (kind, run) = host.pending.remove(at);
                    TRACE.with(|t| t.borrow_mut().push(format!("deliver kind {} ok {} buf {:?} headers {:?}", kind_id(kind), ok, String::from_utf8_lossy(&buf), String::from_utf8_lossy(&host.headers))));
                    host.l(&[70, kind_id(kind)]);
                    match kind {
                        HttpRequestKind::PatchCheck => u.game_patch_check_callback(&mut host, run, ok, &buf),
                        HttpRequestKind::ConfigHead => u.config_head_callback(&mut host, run, ok),
                        HttpRequestKind::Config => u.config_callback(&mut host, run, ok, &buf),
                        HttpRequestKind::Motd => u.motd_callback(&mut host, run, &buf),
                        HttpRequestKind::OverallStats => u.overall_stats_callback(&mut host, ok, &buf),
                        HttpRequestKind::NumPlayersOnline => u.num_players_online_callback(&mut host, ok, &buf),
                    }
                }
            }
            2 => u.http_think_wrapper(&mut host),
            3 => u.stop_async_dns_check(&mut host),
            4 => {
                let k = g.below(4);
                input.push(k);
                if !host.boxes.is_empty() {
                    let b = host.boxes[(k as usize) % host.boxes.len()];
                    host.l(&[71, button_id(b)]);
                    u.on_button(&mut host, b);
                }
            }
            5 => u.check_overall_stats(&mut host),
            6 => u.check_num_players_online(&mut host),
            7 => u.cancel_patch_check_callback(&mut host),
            8 => u.start_downloading_patches(&mut host),
            9 => config_words(g, &mut host, &mut input),
            _ => {
                let mandatory = g.below(2) == 0;
                let url = url_text(g);
                input.push(mandatory as u32);
                put_str(&mut input, &url);
                let a = rts_lockstep_engine::common::ascii_string::AsciiString::try_from_bytes(&url).unwrap_or_default();
                u.queue_patch(mandatory, &a);
            }
        }
        dump_post(&host, &u);
    }
    let out = host.log.borrow().clone();
    (input, out)
}

#[test]
fn main_menu_utils_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("MMU_CASES", 10_000);
    let mut g = Gen::new(0x4d4d_0001);
    let mut chk = Check::new("main menu utils programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 2_000_000];
        let k = unsafe { ref_mmu_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let marker = r[..k].iter().rposition(|&x| x == 0xA5A5A5A5).unwrap_or(0);
            let opn = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!(
                "case {i}: operation {opn} (opcode {}), first difference at word {k} ({} into it; rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}",
                r[marker + 1],
                k - marker,
                r.len(),
                c.len(),
                &r[marker.max(k.saturating_sub(40))..(k + 8).min(r.len())],
                &c[marker.max(k.saturating_sub(40))..(k + 8).min(c.len())]
            );
        }
        if r != c && std::env::var("MMU_DUMP").is_ok() {
            let txt: Vec<String> = input.iter().map(|x| x.to_string()).collect();
            std::fs::write(std::env::var("MMU_DUMP").unwrap(), txt.join(" ")).ok();
        }
        if r != c && std::env::var("MMU_TRACE").is_ok() {
            TRACE.with(|t| println!("{:#?}", t.borrow()));
        }
        chk.eq(r == c, true, || format!("mmu program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
