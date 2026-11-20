#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/DownloadManager.cpp and Include/GameNetwork/DownloadManager.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/download_manager.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_dl.cpp` compiles upstream's `QueuedDownload` and `DownloadManager` (header and `DownloadManager.cpp` whole), cut verbatim by
//! build.rs (`drafts_net4`), on upstream's own `AsciiString.cpp`/`UnicodeString.cpp`. The FTP engine (`CDownload`, whose header `WWDownload/download.h` is not in
//! the tree), the numeric `DOWNLOADEVENT_*`/`DOWNLOADSTATUS_*` values (not in the tree either: the wrapper chooses distinct ones, and this test maps the
//! draft's enums onto them, so what is compared is each event's text label, not its number), the game text and winsock are stand-ins listed in the wrapper;
//! this file's `Host` answers and logs identically. A program is 30 to 90 operations (opcodes at `ref_dl_run`) on a manager with random engine answers;
//! after every operation the results and, at the end, the log of every engine call (its arguments) are compared word by word. Events outside the known
//! codes (the draft's `Unknown` error and `None` status) are included.
//!
//! Not covered: the winsock start-up in the constructor and destructor (host-side), `downloadNextQueuedFile`'s `DEBUG_CRASH` (compiled out in release).

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::drafts::network::download_manager::*;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_dl_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
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

fn put_units(w: &mut Vec<u32>, s: &[u16]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&b| b as u32));
}

struct Host {
    pump: i32,
    download: i32,
    last_local: Vec<u8>,
    log: RefCell<Vec<u32>>,
}

impl DownloadHost for Host {
    fn game_text_fetch(&self, label: &[u8]) -> Vec<u16> {
        let mut v: Vec<u16> = vec![b'T' as u16, b':' as u16];
        v.extend(label.iter().map(|&b| b as u16));
        v
    }
    fn pump_messages(&mut self) -> HResult {
        self.log.borrow_mut().push(1);
        self.pump
    }
    fn download_file(&mut self, server: &[u8], username: &[u8], password: &[u8], file: &[u8], localfile: &[u8], regkey: &[u8], try_resume: bool) -> HResult {
        let mut l = self.log.borrow_mut();
        l.push(2);
        for s in [server, username, password, file, localfile, regkey] {
            put_str(&mut l, s);
        }
        l.push(try_resume as u32);
        self.download
    }
    fn get_last_local_file(&self) -> Vec<u8> {
        self.last_local.clone()
    }
}

fn word(g: &mut Gen, max: u32) -> Vec<u8> {
    (0..g.below(max + 1)).map(|_| (33 + g.below(90)) as u8).collect()
}

fn error_of(code: u32) -> DownloadError {
    match code {
        1 => DownloadError::NoSuchServer,
        2 => DownloadError::CouldNotConnect,
        3 => DownloadError::LoginFailed,
        4 => DownloadError::NoSuchFile,
        5 => DownloadError::LocalFileOpenFailed,
        6 => DownloadError::TcpError,
        7 => DownloadError::DisconnectError,
        _ => DownloadError::Unknown,
    }
}

fn status_of(code: u32) -> DownloadStatus {
    match code {
        1 => DownloadStatus::Connecting,
        2 => DownloadStatus::LoggingIn,
        3 => DownloadStatus::FindingFile,
        4 => DownloadStatus::QueryingResume,
        5 => DownloadStatus::Downloading,
        6 => DownloadStatus::Disconnecting,
        7 => DownloadStatus::Finishing,
        8 => DownloadStatus::Done,
        _ => DownloadStatus::None,
    }
}

fn args(g: &mut Gen, input: &mut Vec<u32>) -> QueuedDownload {
    let mut q = QueuedDownload::default();
    let mut fields: Vec<AsciiString> = Vec::new();
    for _ in 0..6 {
        let w = word(g, 8);
        put_str(input, &w);
        fields.push(AsciiString::try_from_bytes(&w).unwrap_or_default());
    }
    q.server = fields[0].clone();
    q.user_name = fields[1].clone();
    q.password = fields[2].clone();
    q.file = fields[3].clone();
    q.local_file = fields[4].clone();
    q.reg_key = fields[5].clone();
    q.try_resume = g.below(2) == 0;
    input.push(q.try_resume as u32);
    q
}

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let mut host = Host { pump: g.i32(), download: g.i32(), last_local: word(g, 20), log: RefCell::new(Vec::new()) };
    let mut input = vec![host.pump as u32, host.download as u32];
    put_str(&mut input, &host.last_local);
    let mut m = DownloadManager::new(&host);
    let mut out = Vec::new();
    put_units(&mut out, &m.get_status_string());
    let nops = 30 + g.below(61);
    input.push(nops);
    for _ in 0..nops {
        let op = g.below(13);
        input.push(op);
        out.push(0xA5A5A5A5);
        out.push(op);
        match op {
            0 => out.push(m.update(&mut host) as u32),
            1 => {
                let q = args(g, &mut input);
                out.push(m.download_file(&mut host, &q) as u32);
            }
            2 => {
                let q = args(g, &mut input);
                m.queue_file_for_download(q);
            }
            3 => out.push(m.download_next_queued_file(&mut host) as u32),
            4 => out.push(m.is_file_queued_for_download() as u32),
            5 => {
                let s = m.get_last_local_file(&host);
                put_str(&mut host.log.borrow_mut(), s.str());
            }
            6 => {
                out.push(m.is_done() as u32);
                out.push(m.is_ok() as u32);
                out.push(m.was_error() as u32);
            }
            7 => {
                put_units(&mut out, &m.get_status_string());
                put_units(&mut out, &m.get_error_string());
            }
            8 => {
                let code = if g.below(5) == 0 { g.below(40) } else { 1 + g.below(7) };
                input.push(code);
                out.push(m.on_error(&host, error_of(code)) as u32);
            }
            9 => out.push(m.on_end() as u32),
            10 => out.push(m.on_query_resume() as u32),
            11 => {
                let a = [g.i32(), g.i32(), g.i32(), g.i32()];
                input.extend(a.iter().map(|&x| x as u32));
                out.push(m.on_progress_update(a[0], a[1], a[2], a[3]) as u32);
            }
            _ => {
                let code = if g.below(5) == 0 { g.below(40) } else { 1 + g.below(8) };
                input.push(code);
                out.push(m.on_status_update(&host, status_of(code)) as u32);
            }
        }
    }
    out.push(0xB6B6B6B6);
    out.extend(host.log.borrow().iter());
    (input, out)
}

#[test]
fn download_manager_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("DL_CASES", 20_000);
    let mut g = Gen::new(0x91a6_0004);
    let mut chk = Check::new("download manager programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 200_000];
        let k = unsafe { ref_dl_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let op = r[..k].iter().filter(|&&x| x == 0xA5A5A5A5).count();
            println!("case {i}: operation {op}, first difference at word {k} (rust {} words, c++ {} words)", r.len(), c.len());
        }
        chk.eq(r == c, true, || format!("dl program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
