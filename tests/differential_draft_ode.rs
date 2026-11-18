// SPDX-License-Identifier: GPL-3.0-only
#![cfg(all(feature = "differential", feature = "untested-drafts"))]
//! Code-to-code verification (#215, group ww3dgeo) of the ODE integrator draft (`src/drafts/sweep_ww3d/ode.rs`):
//! `IntegrationSystem::Euler_Integrate`, `Midpoint_Integrate`, `Runge_Kutta_Integrate`, `Runge_Kutta5_Integrate`, against
//! upstream's `WWMath/ode.cpp`, compiled whole and unchanged (build.rs `drafts_ww3dgeo`, differential/wrap_draft_ode.cpp).
//!
//! # Method
//!
//! 20,000 cases (fixed seed, `DRAFT_CASES` overrides): a random system of 1 to 8 state variables with a nonlinear derivative
//! (`c0*s[i+1] - c1*s^2 + t*c2 + c3*s`), the same expression in the stand-in `ODESystemClass` of the wrapper and in the
//! Rust [`OdeSystem`]; each of the four integrators on a random state and step (including 0, negative, tiny and huge, so
//! that inf and NaN occur). Compared by bits: the new state AND the full call log of the system (every `Compute_Derivatives`
//! t, the test state's elements and start index; every `Set_State` start index; every `Get_State`), so a wrong time
//! argument or a wrong intermediate state is caught even where it does not reach the result.
//!
//! The draft keeps the work vectors as locals where upstream keeps file-scope statics that persist between calls
//! (`Resize` never shrinks and never clears). The stand-in system writes every element of `dydt` and reads only its own
//! `n` elements, so the stale contents are never read; that is the only way the two differ, and no input excluded.
//!
//! # Predictions (before the first run, 2026-11-18)
//!
//! 0 mismatches in every integrator (the draft is a line-by-line translation with the same f32 operation order). The
//! nine planted slips (11661 to 11669) are all expected to be caught, the call log catching the time-argument ones.
//!
//! # Results
//!
//! See the `DIFF` lines of the run and the comment on #215.

mod support;

use rts_lockstep_engine::drafts::sweep_ww3d::ode::*;
use std::sync::Mutex;
use support::{Check, Gen};

extern "C" {
    fn ref_dode_run(op: i32, dt_bits: u32, n: i32, coef: *const u32, state: *const u32, out: *mut u32, cap: i32) -> i32;
}

static CPP: Mutex<()> = Mutex::new(());

struct Sys {
    n: usize,
    c: [Vec<f32>; 4],
    s: Vec<f32>,
    log: Vec<u32>,
}

impl OdeSystem for Sys {
    fn get_state(&mut self, set_state: &mut StateVector) {
        self.log.push(3);
        set_state.extend_from_slice(&self.s);
    }
    fn set_state(&mut self, new_state: &StateVector, start_index: i32) -> i32 {
        self.log.push(2);
        self.log.push(start_index as u32);
        for i in 0..self.n {
            self.s[i] = new_state[start_index as usize + i];
        }
        start_index + self.n as i32
    }
    fn compute_derivatives(&mut self, t: f32, test_state: Option<&StateVector>, dydt: &mut StateVector, start_index: i32) -> i32 {
        self.log.push(1);
        self.log.push(t.to_bits());
        self.log.push(u32::from(test_state.is_some()));
        self.log.push(start_index as u32);
        let n = self.n;
        let inp: Vec<f32> = (0..n).map(|i| test_state.map_or(self.s[i], |ts| ts[start_index as usize + i])).collect();
        if test_state.is_some() {
            self.log.extend(inp.iter().map(|x| x.to_bits()));
        }
        for i in 0..n {
            dydt[start_index as usize + i] = self.c[0][i] * inp[(i + 1) % n] - self.c[1][i] * inp[i] * inp[i] + t * self.c[2][i] + self.c[3][i] * inp[i];
        }
        start_index + n as i32
    }
}

fn cases() -> u64 {
    std::env::var("DRAFT_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(20000)
}

fn finish(chk: Vec<Check>) {
    let planted = std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0;
    let mut bad = 0;
    for c in chk {
        println!("DIFF {} compared {} mismatches {} nan-payload-only {} max-ulp {}", c.name, c.compared, c.mismatches, c.nan_only, c.max_ulp);
        bad += c.mismatches;
        if !planted {
            c.finish();
        }
    }
    if planted {
        println!("PLANTED mismatches {bad}");
        assert!(bad > 0, "plant not caught");
    }
}

fn value(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => g.special(),
        1 => g.mixed(),
        _ => g.uniform(-3.0, 3.0),
    }
}

#[test]
fn ode_integrators_match_upstream() {
    let _l = CPP.lock().unwrap_or_else(|e| e.into_inner());
    let mut g = Gen::new(0x215_0661);
    let names = ["Euler_Integrate", "Midpoint_Integrate", "Runge_Kutta_Integrate", "Runge_Kutta5_Integrate"];
    let mut chk: Vec<Check> = names.iter().map(|n| Check::new(n)).collect();
    for case in 0..cases() {
        for op in 0..4usize {
            let n = 1 + g.below(8) as usize;
            let coef: Vec<f32> = (0..4 * n).map(|_| if g.below(12) == 0 { value(&mut g) } else { g.uniform(-2.0, 2.0) }).collect();
            let state: Vec<f32> = (0..n).map(|_| value(&mut g)).collect();
            let dt = match g.below(8) {
                0 => g.special(),
                1 => 0.0,
                _ => g.uniform(-0.5, 0.5),
            };
            let mut sys = Sys {
                n,
                c: [coef[..n].to_vec(), coef[n..2 * n].to_vec(), coef[2 * n..3 * n].to_vec(), coef[3 * n..].to_vec()],
                s: state.clone(),
                log: Vec::new(),
            };
            match op {
                0 => euler_integrate(&mut sys, dt),
                1 => midpoint_integrate(&mut sys, dt),
                2 => runge_kutta_integrate(&mut sys, dt),
                _ => runge_kutta5_integrate(&mut sys, dt),
            }
            let mut rust: Vec<u32> = sys.s.iter().map(|x| x.to_bits()).collect();
            rust.extend(sys.log.iter().copied());
            let cb: Vec<u32> = coef.iter().map(|x| x.to_bits()).collect();
            let sb: Vec<u32> = state.iter().map(|x| x.to_bits()).collect();
            let mut out = vec![0u32; 4000];
            let cnt = unsafe { ref_dode_run(op as i32, dt.to_bits(), n as i32, cb.as_ptr(), sb.as_ptr(), out.as_mut_ptr(), out.len() as i32) };
            assert!(cnt >= 0);
            out.truncate(cnt as usize);
            // NaN payloads: both sides NaN with different payload bits are not a mismatch (support/mod.rs rule); compare as f32
            // for the n result words, and as words for the log (t and test state words are floats too)
            let c = &mut chk[op];
            c.compared += 1;
            let same = rust.len() == out.len() && rust.iter().zip(&out).all(|(a, b)| a == b || (f32::from_bits(*a).is_nan() && f32::from_bits(*b).is_nan()));
            if !same {
                c.mismatches += 1;
                if c.examples.len() < 5 {
                    c.examples.push(format!("case {case} op {op} n {n} dt {dt}: rust {:?} cpp {:?}", &rust[..rust.len().min(12)], &out[..out.len().min(12)]));
                }
            } else if rust != out {
                c.nan_only += 1;
            }
        }
    }
    finish(chk);
}
