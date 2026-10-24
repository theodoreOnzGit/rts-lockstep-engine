// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 2 (`docs/phase1-porting-graph.md`):
//! `SubsystemInterface` and `SubsystemInterfaceList` (`common::subsystem_interface`),
//! upstream commit 0a05454d.
//!
//! References: the unchanged `SubsystemInterface.h` and `SubsystemInterface.cpp`
//! (`differential/wrap_subsystem_interface.cpp`, see build.rs). The C++
//! subsystems are a derived class whose `init`, `postProcessLoad`, `reset` and
//! destructor write one line each to a log, and whose name is the one
//! `initSubsystem` sets before `init()`.
//!
//! Inputs: 20,000 random scripts of up to 25 list operations (initialise a
//! subsystem, post-process load, reset, shut down), with subsystem ids 0 to
//! 6, so the same id can be initialised more than once. The log of every
//! `init`, `postProcessLoad`, `reset` and destructor call, in order, must be
//! identical byte for byte, which fixes the ordering rules: `postProcessLoadAll`
//! and `init` in insertion order, `resetAll` and `shutdownAll` in reverse,
//! and the list's destructor shutting down what remains. The name passed to
//! `initSubsystem` must be visible in `init()`.
//!
//! What is not compared: the INI loads (`initSubsystem`'s three paths are
//! passed as NULL in the C++, and the Rust closure is empty), and the
//! `DUMP_PERF_STATS` build, which is not translated.
//!
//! **Planted bug**, which must be caught: `resetAll` in insertion order.
//!
//! **Results** (2026-10-18, release, `--features differential -- --nocapture`):
//! 20,000 scripts, 0 mismatches in the call logs (byte for byte); the planted
//! `resetAll` in insertion order caught in 8,993 of 20,000.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::ascii_string::AsciiString;
use rts_lockstep_engine::common::subsystem_interface::{SubsystemInterface, SubsystemInterfaceList};
use rts_lockstep_engine::differential as cpp;
use std::cell::RefCell;
use std::rc::Rc;
use support::{Check, Gen};

const SCRIPTS: usize = 20_000;
const LOG_CAP: usize = 1 << 16;

/// A subsystem that logs its calls. Its drop is its destructor.
struct LogSys {
    name: AsciiString,
    id: i32,
    log: Rc<RefCell<String>>,
}

impl SubsystemInterface for LogSys {
    fn name(&self) -> &AsciiString {
        &self.name
    }

    fn set_name(&mut self, name: AsciiString) {
        self.name = name;
    }

    fn init(&mut self) {
        let line = format!("init {} {}\n", self.id, String::from_utf8_lossy(self.name.str()));
        self.log.borrow_mut().push_str(&line);
    }

    fn post_process_load(&mut self) {
        self.log.borrow_mut().push_str(&format!("post {}\n", self.id));
    }

    fn reset(&mut self) {
        self.log.borrow_mut().push_str(&format!("reset {}\n", self.id));
    }

    fn update(&mut self) {}
}

impl Drop for LogSys {
    fn drop(&mut self) {
        self.log.borrow_mut().push_str(&format!("drop {}\n", self.id));
    }
}

/// Runs the script on the Rust list and returns its log. `planted` resets in
/// insertion order instead of reverse order.
fn rust_run(ops: &[(i32, i32)], planted: bool) -> String {
    let log = Rc::new(RefCell::new(String::new()));
    {
        let mut list: SubsystemInterfaceList<LogSys> = SubsystemInterfaceList::new();
        for &(op, v) in ops {
            match op {
                0 => {
                    let name = if v == 0 { String::new() } else { format!("N{v}") };
                    let sys = LogSys { name: AsciiString::new(), id: v, log: Rc::clone(&log) };
                    list.init_subsystem(sys, AsciiString::try_from_bytes(name.as_bytes()).expect("short name"), || {});
                }
                1 => list.post_process_load_all(),
                2 => {
                    if planted {
                        for sys in list.subsystems_mut() {
                            sys.reset();
                        }
                    } else {
                        list.reset_all();
                    }
                }
                _ => list.shutdown_all(),
            }
        }
        // The list is dropped here: its destructor shuts down what remains.
    }
    let text = log.borrow().clone();
    text
}

fn cpp_run(ops: &[(i32, i32)]) -> String {
    let flat: Vec<i32> = ops.iter().flat_map(|&(op, v)| [op, v]).collect();
    let mut out = vec![0u8; LOG_CAP];
    let n = unsafe { cpp::ref_subsys_run(flat.as_ptr(), ops.len() as i32, out.as_mut_ptr().cast(), LOG_CAP as i32) };
    assert!(n >= 0, "subsystem log exceeds the buffer");
    String::from_utf8(out[..n as usize].to_vec()).expect("ASCII log")
}

fn gen_script(g: &mut Gen) -> Vec<(i32, i32)> {
    let len = g.below(26) as usize;
    (0..len)
        .map(|_| {
            let op = match g.below(20) {
                0..=7 => 0,
                8..=10 => 1,
                11..=15 => 2,
                _ => 3,
            };
            (op, g.below(7) as i32)
        })
        .collect()
}

#[test]
fn subsystem_list_matches() {
    let mut g = Gen::new(304);
    let mut ok = Check::new("SubsystemInterfaceList (call order and names)");
    let mut bug = Check::new("SubsystemInterfaceList (call order and names)");
    for _ in 0..SCRIPTS {
        let ops = gen_script(&mut g);
        let input = || format!("ops (op, id) = {ops:?}");
        let reference = cpp_run(&ops);
        ok.eq(rust_run(&ops, false), reference.clone(), input);
        bug.eq(rust_run(&ops, true), reference, input);
    }
    ok.finish();
    bug.finish_planted();
}
