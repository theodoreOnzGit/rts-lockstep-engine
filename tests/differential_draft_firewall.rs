#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/FirewallHelper.cpp and Include/GameNetwork/FirewallHelper.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! `src/drafts/network/firewall_helper.rs` against the C++ (GitHub #215, group network3).
//!
//! # Method
//!
//! `differential/wrap_draft_firewall.cpp` compiles upstream's whole `FirewallHelper.cpp` (cut by build.rs, five for-scope edits) on the real
//! `FirewallHelper.h`. The surroundings (options file, global data, mangler configuration, `gethostbyname`, clock, UDP) are stand-ins; this
//! file's `Host` answers and logs identically. Three tests:
//! * `getNATPortAllocationScheme`: 60,000 four to eight port sets (arithmetic runs, runs with a break, relative runs, ties, 16-bit wraps),
//!   the returned delta, both flags and both arrays afterwards compared;
//! * the sizes and constants (`sizeof(ManglerData)`, the behaviour and state values);
//! * programs of 10 to 40 operations on one helper (constructor and its clock-derived port pool, `detectFirewall`,
//!   `behaviorDetectionUpdate` on the paths that need no socket, `getFirewallBehavior`, `isNAT`, `isNetgear`, `getFirewallHardness`,
//!   `getFirewallRetries`, `writeFirewallBehavior`, `flagNeedToRefresh`, `getManglerName`, `reset`), every field of the helper dumped after each.
//!
//! Not compared (the draft's sockets are native, the stand-in's are in-memory, so a path that opens one cannot be compared): `openSpareSocket`,
//! `closeSpareSocket`, `closeAllSpareSockets` (only through `reset`, with no socket open), `getNextTemporarySourcePort`,
//! `sendToManglerFromPort`, `getManglerResponse`, `byteAdjust` (private, only reached through those two) and `detectionTest1..5Update` with
//! `detectionTest3WaitForResponsesUpdate`; `detectionBeginUpdate` is compared only up to the point where it opens a socket (with three or more
//! distinct mangler addresses), so every case has at most two distinct addresses. A program starts one detection per helper (a second one
//! reads `mangler_addresses[]` uninitialised in C++).

use rts_lockstep_engine::drafts::network::firewall_helper::*;
use rts_lockstep_engine::drafts::network::nat::firewall_type;
use std::cell::RefCell;
use std::os::raw::{c_int, c_uint};

mod support;
use support::{Check, Gen};

extern "C" {
    fn ref_fw_scheme(num: c_int, orig: *mut u16, mang: *mut u16, rel: *mut c_int, good: *mut c_int) -> c_int;
    fn ref_fw_sizes(out: *mut c_uint) -> c_int;
    fn ref_fw_run(input: *const c_uint, n: c_int, out: *mut c_uint, cap: c_int) -> c_int;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// ---------------------------------------------------------------------------------------------------------------
// getNATPortAllocationScheme
// ---------------------------------------------------------------------------------------------------------------

fn port_sets(g: &mut Gen) -> (Vec<u16>, Vec<u16>) {
    let n = 4 + g.below(5) as usize;
    let base = match g.below(4) {
        0 => g.below(100) as u16,
        1 => 65_500u16.wrapping_add(g.below(100) as u16),
        _ => 1024 + g.below(60_000) as u16,
    };
    let orig: Vec<u16> = (0..n)
        .map(|_| match g.below(4) {
            0 => 4096 + g.below(8) as u16,
            1 => g.below(65_536) as u16,
            _ => 4096 + (g.below(300) as u16) * 2,
        })
        .collect();
    let mut mang: Vec<u16> = Vec::with_capacity(n);
    let step: i32 = match g.below(8) {
        0 => 1,
        1 => 2,
        2 => 3 + g.below(200) as i32,
        3 => 0,
        4 => 100,
        _ => 1 + g.below(50) as i32,
    };
    let kind = g.below(6);
    for i in 0..n {
        let v: i32 = match kind {
            0 | 1 => base as i32 + step * i as i32,                        // absolute run
            2 => orig[i] as i32 + step * i as i32 + 1000,                  // relative run
            3 => base as i32 + step * i as i32 + if i == 1 + g.below(3) as usize { g.below(20) as i32 } else { 0 }, // broken run
            4 => orig[i] as i32 + step * i as i32 + if g.below(3) == 0 { g.below(10) as i32 } else { 0 }, // relative, broken
            _ => g.below(65_536) as i32,
        };
        mang.push(v as u16);
    }
    // shuffle so the sort matters: the pairs together (the relative runs keep their originals), and sometimes the mangled ports alone
    let mut orig = orig;
    let alone = g.below(6) == 0;
    for i in (1..n).rev() {
        let j = g.below(i as u32 + 1) as usize;
        mang.swap(i, j);
        if !alone {
            orig.swap(i, j);
        }
    }
    if g.below(10) == 0 {
        let a = g.below(n as u32) as usize;
        let b = g.below(n as u32) as usize;
        mang[a] = mang[b];
    }
    (orig, mang)
}

#[test]
fn nat_port_allocation_scheme_matches_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("FW_CASES", 60_000);
    let mut g = Gen::new(0xf1e0_0001);
    let mut chk = Check::new("firewall getNATPortAllocationScheme");
    let mut seen = [0u32; 8];
    for i in 0..n {
        let (orig, mang) = port_sets(&mut g);
        let num = orig.len() as i32;
        let (mut ro, mut rm) = (orig.clone(), mang.clone());
        let (mut co, mut cm) = (orig.clone(), mang.clone());
        let (mut rrel, mut rgood) = (false, false);
        let rv = FirewallHelperClass::get_nat_port_allocation_scheme(num, &mut ro, &mut rm, &mut rrel, &mut rgood);
        let (mut crel, mut cgood) = (0 as c_int, 0 as c_int);
        let cv = unsafe { ref_fw_scheme(num, co.as_mut_ptr(), cm.as_mut_ptr(), &mut crel, &mut cgood) };
        seen[(rrel as usize) * 2 + rgood as usize] += 1;
        let r = (rv, rrel, rgood, ro, rm);
        let c = (cv, crel != 0, cgood != 0, co, cm);
        chk.eq(r, c, || format!("case {i}: original {orig:?} mangled {mang:?}"));
    }
    println!("outcomes (relative, looks good): (f,f) {} (f,t) {} (t,f) {} (t,t) {}", seen[0], seen[1], seen[2], seen[3]);
    assert!(seen[0] > 0 && seen[1] > 0 && seen[2] > 0 && seen[3] > 0, "the generator must reach every outcome");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}

#[test]
fn firewall_sizes_and_constants_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = [0u32; 64];
    let n = unsafe { ref_fw_sizes(out.as_mut_ptr()) } as usize;
    let mut chk = Check::new("firewall sizes and constants");
    let rust: Vec<u32> = vec![
        MANGLER_DATA_SIZE as u32,
        30, // sizeof(ManglerMessage): see the module doc of the draft (data 20 + length 4 + ip 4 + port 2, packed)
        MANGLER_PORT as u32,
        MAX_NUM_MANGLERS as u32,
        MAX_SPARE_SOCKETS as u32,
        NUM_TEST_PORTS as u32,
        firewall_type::UNKNOWN,
        firewall_type::SIMPLE,
        firewall_type::DUMB_MANGLING,
        firewall_type::SMART_MANGLING,
        firewall_type::NETGEAR_BUG,
        firewall_type::SIMPLE_PORT_ALLOCATION,
        firewall_type::RELATIVE_PORT_ALLOCATION,
        firewall_type::DESTINATION_PORT_DELTA,
        FirewallDetectionState::Idle as u32,
        FirewallDetectionState::Begin as u32,
        FirewallDetectionState::Test1 as u32,
        FirewallDetectionState::Test2 as u32,
        FirewallDetectionState::Test3 as u32,
        FirewallDetectionState::Test3WaitForResponses as u32,
        FirewallDetectionState::Test4_1 as u32,
        FirewallDetectionState::Test4_2 as u32,
        FirewallDetectionState::Test5 as u32,
        FirewallDetectionState::Done as u32,
    ];
    for (i, r) in rust.iter().enumerate() {
        chk.eq(*r, out[i], || format!("value {i}"));
    }
    assert_eq!(n, rust.len());
    chk.finish();
}

// ---------------------------------------------------------------------------------------------------------------
// programs
// ---------------------------------------------------------------------------------------------------------------

struct Host {
    now: u32,
    refresh: u32,
    behavior: u32,
    over: u32,
    delay: bool,
    delta: i32,
    hosts: Vec<(Vec<u8>, u32)>,
    log: RefCell<Vec<u32>>,
}

impl Host {
    fn push(&self, v: u32) {
        self.log.borrow_mut().push(v);
    }
}

impl FirewallHost for Host {
    fn time_get_time(&self) -> u32 {
        self.now
    }
    fn pref_firewall_need_to_refresh(&self) -> bool {
        self.refresh == 1 || self.refresh == 2
    }
    fn pref_write_firewall_need_to_refresh(&mut self, flag: bool) {
        self.push(62);
        self.push(u32::from(flag));
    }
    fn pref_write_firewall_behavior(&mut self, behavior: u32, port_allocation_delta: i32) {
        self.push(61);
        self.push(behavior);
        self.push(port_allocation_delta as u32);
    }
    fn global_firewall_behavior(&self) -> u32 {
        self.behavior
    }
    fn set_global_firewall_behavior(&mut self, value: u32) {
        self.behavior = value;
    }
    fn global_firewall_port_override(&self) -> u16 {
        self.over as u16
    }
    fn global_firewall_send_delay(&self) -> bool {
        self.delay
    }
    fn global_firewall_port_allocation_delta(&self) -> i32 {
        self.delta
    }
    fn set_global_firewall_port_allocation_delta(&mut self, value: i32) {
        self.delta = value;
    }
    fn mangler_location(&self, index: i32) -> Vec<u8> {
        self.hosts.get(index as usize).map(|h| h.0.clone()).unwrap_or_default()
    }
    fn gethostbyname(&mut self, name: &[u8]) -> Option<[u8; 4]> {
        for h in &self.hosts {
            if h.0 == name && h.1 != 0 {
                return Some(h.1.to_be_bytes());
            }
        }
        None
    }
}

const HOST_NAMES: &[&str] = &["m1.example", "m2.example", "m3.example", "mangler", "a", "b"];

fn push_bytes(v: &mut Vec<u32>, b: &[u8]) {
    v.push(b.len() as u32);
    for c in b.chunks(4) {
        let mut w = 0u32;
        for (j, &x) in c.iter().enumerate() {
            w |= u32::from(x) << (8 * j);
        }
        v.push(w);
    }
}

struct Rd<'a> {
    w: &'a [u32],
    i: usize,
}
impl Rd<'_> {
    fn next(&mut self) -> u32 {
        let v = self.w.get(self.i).copied().unwrap_or(0);
        self.i += 1;
        v
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.next() as usize;
        let mut b = vec![0u8; n];
        let mut i = 0;
        while i < n {
            let v = self.next();
            for j in 0..4 {
                if i + j < n {
                    b[i + j] = (v >> (8 * j)) as u8;
                }
            }
            i += 4;
        }
        b
    }
}

fn behavior_value(g: &mut Gen) -> u32 {
    match g.below(4) {
        0 => g.below(128),
        1 => 1 << g.below(7),
        2 => g.below(256),
        _ => 0,
    }
}

fn program(g: &mut Gen) -> Vec<u32> {
    let mut v = vec![];
    v.push(match g.below(3) {
        0 => 0,
        1 => g.below(100_000),
        _ => g.u32(),
    });
    v.push(g.below(5)); // refresh
    v.push(behavior_value(g));
    v.push(match g.below(3) {
        0 => 0,
        1 => 1 + g.below(65_535),
        _ => 0,
    });
    v.push(g.below(2));
    // up to two distinct addresses, some names unresolvable or empty
    let nh = g.below(5);
    v.push(nh);
    let addrs = [0x0a000001u32, 0x0a000002u32, 0];
    for _ in 0..nh {
        let name = if g.below(8) == 0 { String::new() } else { HOST_NAMES[g.below(HOST_NAMES.len() as u32) as usize].to_string() };
        push_bytes(&mut v, name.as_bytes());
        v.push(addrs[g.below(3) as usize]);
    }
    let nops = 10 + g.below(31);
    for _ in 0..nops {
        let op = 1 + g.below(22);
        v.push(op);
        match op {
            2 => v.push(match g.below(3) {
                0 => 0,
                1 => 65_530 + g.below(20),
                _ => g.below(70_000),
            }),
            5 => {
                v.push(5);
                v.push(1); // a second detection would read mangler_addresses[] uninitialised: start from a new helper
            }
            11 => v.push(g.below(2)),
            12 => v.push(g.below(6)),
            15 => v.push(g.below(10_000)),
            16 => v.push(behavior_value(g)),
            17 => v.push(g.below(3) * 1000),
            18 => v.push(g.below(2)),
            19..=22 => v.push(behavior_value(g)),
            _ => {}
        }
    }
    v
}

fn dump(o: &mut Vec<u32>, h: &FirewallHelperClass, host: &Host) {
    o.push(0xD0D0D0D0);
    o.extend(h.debug_words());
    o.push(host.behavior);
    o.push(host.delta as u32);
}

fn rust_run(w: &[u32]) -> Vec<u32> {
    let mut rd = Rd { w, i: 0 };
    let now = rd.next();
    let refresh = rd.next();
    let behavior = rd.next();
    let over = rd.next();
    let delay = rd.next() != 0;
    let nh = rd.next();
    let mut hosts = Vec::new();
    for _ in 0..nh {
        let n = rd.bytes();
        let a = rd.next();
        hosts.push((n, a));
    }
    let mut host = Host { now, refresh, behavior, over, delay, delta: 0, hosts, log: RefCell::new(Vec::new()) };
    let mut h = FirewallHelperClass::new(&host);
    let mut o: Vec<u32> = Vec::new();
    dump(&mut o, &h, &host);
    // the C++ keeps one global behaviour that the helper's write reads back
    while rd.i < w.len() {
        let op = rd.next();
        o.push(0xA5A5A5A5);
        o.push(op);
        match op {
            1 => h = FirewallHelperClass::new(&host),
            2 => h.set_source_port_pool_start(rd.next() as i32),
            3 => o.push(h.get_source_port_pool() as u32),
            4 => o.push(u32::from(h.detect_firewall(&mut host))),
            5 => o.push(u32::from(h.behavior_detection_update(&mut host))),
            6 => o.push(h.get_firewall_behavior()),
            7 => o.push(u32::from(h.get_raw_firewall_behavior())),
            8 => o.push(u32::from(h.is_nat())),
            9 => o.push(u32::from(h.is_netgear())),
            10 => h.write_firewall_behavior(&mut host),
            11 => {
                let f = rd.next() != 0;
                h.flag_need_to_refresh(&mut host, f)
            }
            12 => {
                let name = FirewallHelperClass::get_mangler_name(&host, rd.next() as i32);
                o.push(name.len() as u32);
                o.extend(name.iter().map(|&c| u32::from(c)));
            }
            13 => h.reset(),
            14 => o.push(h.get_source_port_allocation_delta() as i32 as u32),
            15 => host.now = host.now.wrapping_add(rd.next()),
            16 => host.behavior = rd.next(),
            17 => host.over = rd.next(),
            18 => host.delay = rd.next() != 0,
            19 => o.push(FirewallHelperClass::get_firewall_hardness(rd.next()) as u32),
            20 => o.push(FirewallHelperClass::get_firewall_retries(rd.next()) as u32),
            21 => o.push(u32::from(FirewallHelperClass::is_nat_behavior(rd.next()))),
            22 => o.push(u32::from(FirewallHelperClass::is_netgear_behavior(rd.next()))),
            _ => {}
        }
        o.append(&mut host.log.borrow_mut());
        dump(&mut o, &h, &host);
    }
    o
}

fn cpp_run(w: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 1_000_000];
    let n = unsafe { ref_fw_run(w.as_ptr(), w.len() as c_int, out.as_mut_ptr(), out.len() as c_int) };
    assert!((n as usize) <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

#[test]
fn firewall_programs_match_cpp() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let n = cases("FW_PROGRAMS", 20_000);
    let mut g = Gen::new(0xf1e0_0002);
    let mut chk = Check::new("firewall helper programs");
    for i in 0..n {
        let w = program(&mut g);
        let r = rust_run(&w);
        let c = cpp_run(&w);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            let k0 = k.saturating_sub(12);
            println!("case {i}: first difference at word {k} (rust {} words, c++ {} words)\n  rust {:?}\n  c++  {:?}", r.len(), c.len(), &r[k0..(k + 12).min(r.len())], &c[k0..(k + 12).min(c.len())]);
            if std::env::var("FW_DEBUG").is_ok() {
                println!("program {:?}", w);
            }
        }
        chk.eq(r == c, true, || format!("firewall case {i}"));
    }
    println!("{n} programs");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
