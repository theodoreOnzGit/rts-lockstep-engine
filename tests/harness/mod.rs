// SPDX-License-Identifier: GPL-3.0-only
//! The case loop shared by the differential tests of one module each (`p1w12-sonnet`, update batch N on): define 1 to 8 upgrades, generate a `D` block,
//! read it on both sides, generate the program (a configuration and ops of five words), run the Rust side first (a case that reaches upstream
//! undefined behaviour is counted and never sent to the C++), run the C++ side (status 99 is a crash trapped by the driver: a mismatch), and compare the
//! two dumps line by line and the name keys afterwards. A test supplies the module's own part: the generators, the Rust run, the C++ call and the decoder.
#![allow(dead_code)]

use rts_lockstep_engine::common::ini::{Ini, IniLoadType};
use rts_lockstep_engine::common::module::ModuleDataKind;
use rts_lockstep_engine::differential as cpp;
use std::ffi::CString;

use crate::support::{Check, Gen};
use crate::up_common::*;

/// What the Rust run returns.
pub struct RunOut {
    /// The dump after the data dump, as the driver writes it.
    pub dump: Vec<u8>,
    /// The name keys of the probe names afterwards.
    pub keys: Vec<u32>,
    /// The case reached upstream undefined behaviour (or a constructor that throws): count it, do not send it.
    pub excluded: bool,
}

/// What the C++ call returns: the driver's status, and the dump.
pub struct CppOut {
    pub status: i32,
    pub dump: Vec<u8>,
    pub code: u32,
    pub msg: Vec<u8>,
}

/// The counts of a run.
#[derive(Default)]
pub struct Counts {
    pub cases: usize,
    pub read_failed: u64,
    pub ops_run: u64,
    pub excluded: u64,
}

/// A case's configuration words and its ops of five words.
pub type Program = (Vec<u32>, Vec<[u32; 5]>);
/// The generator of a `D` block (the upgrades' names, whether to keep it well formed).
pub type GenData<'a> = dyn Fn(&mut Gen, &[String], bool) -> Vec<u8> + 'a;
/// The generator of a case's program.
pub type GenCase<'a> = dyn Fn(&mut Gen, &Option<ModuleDataKind>, usize, &[String]) -> Program + 'a;
/// The Rust run of a case.
pub type RustRun<'a> = dyn Fn(Ctx, ModuleDataKind, &[u32], &[[u32; 5]]) -> RunOut + 'a;
/// The C++ run of a case.
pub type CppRun<'a> = dyn Fn(&CString, &CString, &[u32], &[[u32; 5]]) -> CppOut + 'a;

type RustRead<'a> = &'a dyn Fn(&[u8], &[u8]) -> (rts_lockstep_engine::common::ini::IniResult<()>, Option<ModuleDataKind>);

/// One module's test.
pub struct Harness<'a> {
    /// The label of the checks, `update module N1`.
    pub label: &'a str,
    pub seed: u64,
    /// The environment variable that overrides the case count.
    pub cases_var: &'a str,
    /// The stem of the INI file names (cycled over five).
    pub ini_stem: &'a str,
    /// The text of a `D` block (the upgrades' names, whether to keep it well formed).
    pub gen_data: &'a GenData<'a>,
    /// Reads the block on the Rust side.
    pub rust_read: RustRead<'a>,
    /// The configuration and the ops of a case (the data is `None` when the read failed).
    pub gen_case: &'a GenCase<'a>,
    pub rust_run: &'a RustRun<'a>,
    pub cpp_run: &'a CppRun<'a>,
    pub decode: &'a dyn Fn(&[u8], usize) -> Vec<String>,
    /// The names whose keys are compared afterwards (empty: not compared).
    pub probe: &'a [&'a str],
    pub xfer_path: &'a str,
}

type Driver = unsafe extern "C" fn(*const std::os::raw::c_char, *const std::os::raw::c_char, *const u32, i32, *const u32, *mut u8, i32, *mut i32, *mut std::os::raw::c_char, i32, *mut u32) -> i32;

/// Calls a driver `ref_xxx_run(filename, xfer_path, cfg, n, ops, buf, cap, len, msg, msg_cap, code)`.
pub fn call_driver(f: Driver, name: &CString, xfer: &CString, cfg: &[u32], ops: &[[u32; 5]]) -> CppOut {
    let flat: Vec<u32> = ops.iter().flatten().copied().collect();
    let mut buf = vec![0u8; 1 << 22];
    let mut msg = vec![0u8; 4096];
    let (mut len, mut code) = (0i32, 0u32);
    let status = unsafe { f(name.as_ptr(), xfer.as_ptr(), cfg.as_ptr(), ops.len() as i32, flat.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, &mut len, msg.as_mut_ptr() as *mut _, msg.len() as i32, &mut code) };
    buf.truncate(len as usize);
    CppOut { status, dump: buf, code, msg }
}

impl Harness<'_> {
    /// Runs the cases; panics (through the checks) on a mismatch.
    pub fn run(&self) -> Counts {
        let cases: usize = std::env::var(self.cases_var).ok().and_then(|s| s.parse().ok()).unwrap_or(9_000);
        let mut g = Gen::new(self.seed);
        let table = blocks();
        let mut read = Check::new(&format!("{} data read (throw, message)", self.label));
        let mut run = Check::new(&format!("{} ops", self.label));
        let mut keys = Check::new("name keys after the ops");
        let mut counts = Counts { cases, ..Default::default() };
        let mut shown = 0;
        for case in 0..cases {
            let init_names = g.below(3) != 0;
            let init_center = g.below(5) != 0;
            let n_up = if g.below(10) == 0 { 0 } else { 1 + g.below(8) as usize };
            let upgrades: Vec<String> = (0..n_up).map(|i| format!("Up{i}")).collect();
            let defs: String = upgrades.iter().map(|u| format!("Upgrade {u}\nEnd\n")).collect();
            let mut ctx = Ctx::new(init_names, init_center);
            Ini::load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, None, &table, &mut ctx).expect("upgrades load");
            cpp_reset(init_names, init_center);
            assert_eq!(cpp_load(b"Data\\INI\\Up.ini", defs.as_bytes(), IniLoadType::Overwrite, false).status, 0);

            let clean = g.below(10) < 7;
            let text = (self.gen_data)(&mut g, &upgrades, clean);
            let name = format!("Data\\INI\\{}{}.ini", self.ini_stem, case % 5).into_bytes();
            let (r, data) = (self.rust_read)(&name, &text);
            let outcome = match rust_outcome(r, 0) {
                Ok(o) => o,
                Err(e) => {
                    assert!(e.is_undefined(), "{e}");
                    counts.excluded += 1;
                    continue;
                }
            };
            let (cfg, ops) = (self.gen_case)(&mut g, &data, n_up, &upgrades);
            // Rust first: a case that reaches upstream undefined behaviour is counted and never sent to the C++
            let rust = if outcome.status == 0 {
                let out = (self.rust_run)(ctx, data.expect("read ok"), &cfg, &ops);
                if out.excluded {
                    counts.excluded += 1;
                    continue;
                }
                Some(out)
            } else {
                None
            };
            let cfile = CString::new(name.clone()).unwrap();
            let cxfer = CString::new(self.xfer_path).unwrap();
            let cout = unsafe {
                cpp::ref_ini_reset();
                cpp::ref_ini_add_file(cfile.as_ptr(), text.as_ptr() as *const _, text.len() as i32);
                (self.cpp_run)(&cfile, &cxfer, &cfg, &ops)
            };
            let what = || format!("case {case}: upgrades {upgrades:?} cfg {cfg:x?}, file {:?}, ops {ops:?}", show(&text));
            if cout.status == 99 {
                // the C++ crashed on a case the planted Rust steered past upstream undefined behaviour: a mismatch, not a process kill
                run.eq(&"C++ crashed", &"no crash", what);
                continue;
            }
            let detail = match cout.status {
                0 | 4 => Vec::new(),
                1 => cout.msg[..cout.code as usize].to_vec(),
                2 | 3 => cout.code.to_le_bytes().to_vec(),
                s => panic!("C++ threw something unexpected (status {s}) -- a constructor that threw upstream but not here"),
            };
            read.eq(&outcome, &Outcome { status: cout.status, detail, crc: 0 }, what);
            let Some(rust) = rust else {
                counts.read_failed += 1;
                continue;
            };
            let (dr, dc) = ((self.decode)(&rust.dump, ops.len()), (self.decode)(&cout.dump, ops.len()));
            if dr != dc && shown < 6 {
                shown += 1;
                let i = dr.iter().zip(dc.iter()).position(|(a, b)| a != b).unwrap_or(dr.len().min(dc.len()));
                eprintln!("--- case {case} file {:?} ops {ops:?}\n  rust {:?}\n  c++  {:?}\n  prev {:?}\n  cfg {cfg:x?}", show(&text), dr.get(i), dc.get(i), i.checked_sub(1).and_then(|j| dr.get(j)));
            }
            run.eq(&dr.len(), &dc.len(), what);
            for (a, b) in dr.iter().zip(dc.iter()) {
                run.eq(a, b, what);
            }
            if !self.probe.is_empty() {
                let ck: Vec<u32> = self
                    .probe
                    .iter()
                    .map(|n| {
                        let c = CString::new(*n).unwrap();
                        unsafe { cpp::ref_up_name_key(c.as_ptr()) }
                    })
                    .collect();
                keys.eq(rust.keys, ck, what);
            }
            counts.ops_run += ops.len() as u64;
        }
        read.finish();
        run.finish();
        if !self.probe.is_empty() {
            keys.finish();
        }
        counts
    }
}
