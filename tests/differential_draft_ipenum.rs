#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/IPEnumeration.cpp and Include/GameNetwork/IPEnumeration.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/ip_enumeration.rs` against the C++ (GitHub #215, group net4).
//!
//! # Method
//!
//! `differential/wrap_draft_ipenum.cpp` compiles upstream's `EnumeratedIP` and `IPEnumeration` (header and `IPEnumeration.cpp` whole), cut verbatim by
//! build.rs (`drafts_net4`), on upstream's own `AsciiString.cpp`/`UnicodeString.cpp`. Winsock (`WSAStartup`, `gethostname`, `gethostbyname`) is a stand-in that
//! answers from variables the case sets (shim_draft_net4/winsock_standin.h); this file's `Host` answers identically. A case is 3 to 9 calls of
//! `getAddresses` or `getMachineName`, each with its own host configuration (working, `gethostname` fails, `gethostbyname` fails, winsock does not
//! start), machine name and 0 to 8 addresses (small octet ranges, so that equal and nearly equal addresses meet); after each call the whole list (strings and
//! integers, in order) is compared. The draft's host does the winsock start-up and the `h_length == 4` check (a failure of either is "the host answers None").
//!
//! Not covered: `WSAStartup`'s version test and `WSACleanup` (the host's), the destructor's list deletion (not observable).

use rts_lockstep_engine::drafts::network::ip_enumeration::*;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_ip_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// The winsock layer: its start-up (the first call that reaches it) fails under configuration 3, and once it succeeded it is not asked again, as in the
/// C++ (`m_isWinsockInitialized`).
struct Host {
    cfg: u32,
    name: Vec<u8>,
    addrs: Vec<[u8; 4]>,
    started: bool,
}

impl LocalAddressSource for Host {
    fn host_name(&mut self) -> Option<Vec<u8>> {
        if !self.started {
            if self.cfg == 3 {
                return None;
            }
            self.started = true;
        }
        if self.cfg == 1 {
            None
        } else {
            Some(self.name.clone())
        }
    }
    fn host_addresses(&mut self, _name: &[u8]) -> Option<Vec<[u8; 4]>> {
        if self.cfg == 2 {
            None
        } else {
            Some(self.addrs.clone())
        }
    }
}

fn put_str(w: &mut Vec<u32>, s: &[u8]) {
    w.push(s.len() as u32);
    w.extend(s.iter().map(|&b| b as u32));
}

fn program(g: &mut Gen) -> (Vec<u32>, Vec<u32>) {
    let mut e = IPEnumeration::new();
    let mut host = Host { cfg: 0, name: Vec::new(), addrs: Vec::new(), started: false };
    let mut input = Vec::new();
    let mut out = Vec::new();
    let calls = 3 + g.below(7);
    input.push(calls);
    for _ in 0..calls {
        let cfg = if g.below(10) < 7 { 0 } else { 1 + g.below(3) };
        let name: Vec<u8> = (0..g.below(21)).map(|_| (33 + g.below(90)) as u8).collect();
        let narrow = g.below(2) == 0;
        let addrs: Vec<[u8; 4]> = (0..g.below(9))
            .map(|_| {
                if narrow {
                    [(g.below(3) * 100) as u8, g.below(2) as u8, g.below(3) as u8, g.below(4) as u8]
                } else {
                    [g.below(256) as u8, g.below(256) as u8, g.below(256) as u8, g.below(256) as u8]
                }
            })
            .collect();
        input.push(cfg);
        put_str(&mut input, &name);
        input.push(addrs.len() as u32);
        for a in &addrs {
            input.extend(a.iter().map(|&b| b as u32));
        }
        let op = if g.below(5) == 0 { 1 } else { 0 };
        input.push(op);
        out.push(0xA5A5A5A5);
        out.push(op);
        host.cfg = cfg;
        host.name = name;
        host.addrs = addrs;
        if op == 0 {
            let list = e.get_addresses(&mut host);
            out.push(!list.is_empty() as u32);
            out.push(list.len() as u32);
            for ip in list {
                put_str(&mut out, ip.get_ip_string().str());
                out.push(ip.get_ip());
            }
        } else {
            let s = e.get_machine_name(&mut host);
            put_str(&mut out, s.str());
        }
    }
    (input, out)
}

#[test]
fn ip_enumeration_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("IP_CASES", 20_000);
    let mut g = Gen::new(0x91a6_0003);
    let mut chk = Check::new("ip enumeration programs");
    for i in 0..n {
        let (input, r) = program(&mut g);
        let mut c = vec![0u32; 100_000];
        let k = unsafe { ref_ip_run(input.as_ptr(), input.len() as c_int, c.as_mut_ptr(), c.len() as c_int) };
        assert!(k >= 0, "the program dump does not fit");
        c.truncate(k as usize);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            println!("case {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k.saturating_sub(8)..(k + 8).min(r.len())], &c[k.saturating_sub(8)..(k + 8).min(c.len())]);
        }
        chk.eq(r == c, true, || format!("ip program {i}"));
    }
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
