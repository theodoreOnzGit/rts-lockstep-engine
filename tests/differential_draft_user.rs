#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation draft of (EA's release,
// https://github.com/electronicarts/CnC_Generals_Zero_Hour, commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameNetwork/User.cpp and Include/GameNetwork/User.h
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The network `User` (GitHub #215, group netlogic): `src/drafts/network/user.rs`, every function.
//!
//! # Method
//!
//! `differential/wrap_draft_user.cpp` compiles upstream's `User.cpp` and `User.h` whole and unchanged (stand-ins: `networkdefs.h`, and the
//! 16-bit `UnicodeString` of differential/shim with `set(const UnicodeString &)` added). A case is one to four users (names from a pool of
//! equal, case-different, prefix and empty names; addresses; 32-bit ports, which the constructor truncates to 16 bits) and a sequence of
//! operations (assignment, `==`, `!=`, `setName`, `SetPort`, `SetIPAddr`, a default-constructed user); every user is written after each
//! operation, with the comparison results, as a log of words compared whole.
//!
//! The default `User()` leaves the port and address uninitialised upstream (0 in the draft): the test sets both before reading them.

mod support;

use rts_lockstep_engine::common::unicode_string::UnicodeString;
use rts_lockstep_engine::drafts::network::user::User;
use support::{Check, Gen};

extern "C" {
    fn us_run(input: *const u32, nin: i32, out: *mut u32, cap: i32) -> i32;
}

static CPP: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cpp_run(input: &[u32]) -> Vec<u32> {
    let mut out = vec![0u32; 100_000];
    let n = unsafe { us_run(input.as_ptr(), input.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n as usize <= out.len(), "log too long");
    out.truncate(n as usize);
    out
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(var: &str, default: u64) -> u64 {
    std::env::var(var).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
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
}

fn read_name(r: &mut Rd) -> UnicodeString {
    let n = r.next();
    let mut v = Vec::new();
    for _ in 0..n {
        let c = (r.next() & 0xffff) as u16;
        v.push(if c == 0 { 1 } else { c });
    }
    UnicodeString::try_from_chars(&v).unwrap()
}

fn dump(o: &mut Vec<u32>, u: &User) {
    let n = u.get_name();
    o.push(n.str().len() as u32);
    for c in n.str() {
        o.push(*c as u32);
    }
    o.push(u.get_port() as u32);
    o.push(u.get_ip_addr());
}

fn rust_run(input: &[u32]) -> Vec<u32> {
    let mut r = Rd { w: input, i: 0 };
    let n = r.next();
    let mut o = Vec::new();
    if n == 0 {
        return o;
    }
    let mut us = Vec::new();
    for _ in 0..n {
        let nm = read_name(&mut r);
        let addr = r.next();
        let port = r.next();
        us.push(User::with_address(nm, addr, port));
    }
    for u in &us {
        dump(&mut o, u);
    }
    let nops = r.next();
    for _ in 0..nops {
        let code = r.next();
        let a = (r.next() % n) as usize;
        let b = r.next();
        let bi = (b % n) as usize;
        match code {
            0 => {
                let other = us[bi].clone();
                us[a].assign(&other);
            }
            1 => o.push(us[a].is_equal(&us[bi]) as u32),
            2 => o.push(us[a].is_not_equal(&us[bi]) as u32),
            3 => {
                let nm = us[bi].get_name();
                us[a].set_name(nm);
            }
            4 => us[a].set_port(b as u16),
            5 => us[a].set_ip_addr(b),
            6 => {
                let mut u = User::new();
                u.set_port(a as u16);
                u.set_ip_addr((a as u32).wrapping_mul(3));
                dump(&mut o, &u);
            }
            _ => {}
        }
        for u in &us {
            dump(&mut o, u);
        }
    }
    o
}

fn gen_case(g: &mut Gen) -> Vec<u32> {
    let pool: [&[u32]; 7] = [&[], &[65, 66], &[97, 98], &[65], &[65, 66, 67], &[0x4e2d, 0x6587], &[65, 66, 0xffff]];
    let n = 1 + g.below(4);
    let mut v = vec![n];
    for _ in 0..n {
        let nm = pool[g.below(7) as usize];
        v.push(nm.len() as u32);
        v.extend_from_slice(nm);
        v.push(if g.below(3) == 0 { g.u32() } else { g.below(5) });
        v.push(match g.below(4) {
            0 => g.u32(),
            1 => 65535,
            2 => 65536 + g.below(10),
            _ => g.below(70000),
        });
    }
    let nops = g.below(12);
    v.push(nops);
    for _ in 0..nops {
        let code = g.below(7);
        v.push(code);
        v.push(g.below(4));
        v.push(match code {
            4 => g.below(70000),
            5 => g.u32(),
            _ => g.below(4),
        });
    }
    v
}

#[test]
fn the_user_matches_upstream() {
    let n = cases("NETLOGIC_USER_CASES", 20000);
    let mut g = Gen::new(0x215_4001);
    let mut chk = Check::new("User operations");
    for i in 0..n {
        let v = gen_case(&mut g);
        let _one = CPP.lock().unwrap_or_else(|e| e.into_inner());
        let c = cpp_run(&v);
        let r = rust_run(&v);
        drop(_one);
        if r != c {
            let k = r.iter().zip(&c).position(|(a, b)| a != b).unwrap_or(r.len().min(c.len()));
            println!("user case {i}: first difference at word {k} (rust {} words, c++ {} words)", r.len(), c.len());
        }
        chk.eq(r, c, || format!("user case {i}"));
    }
    println!("{n} user sequences");
    if planted() {
        chk.finish_planted();
    } else {
        chk.finish();
    }
}
