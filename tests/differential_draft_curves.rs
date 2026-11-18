#![cfg(all(feature = "differential", feature = "untested-drafts"))]
// SPDX-License-Identifier: GPL-3.0-only
//
// Differential test of the translation drafts of (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/curve.cpp, hermitespline.cpp, cardinalspline.cpp, catmullromspline.cpp,
//   tcbspline.cpp, lookuptable.cpp, lookuptable.h, euler.cpp (and their headers)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only), with
// the additional terms in NOTICE.md.

//! The curves, splines, lookup table and Euler angles of `drafts::wwlib` against the upstream C++ (#215).
//!
//! # Method
//!
//! The C++ side is the upstream .cpp files `#include`d unchanged by `differential/wrap_draft_curves.cpp` and
//! `wrap_draft_euler.cpp` (they list their stand-ins; lookuptable's functions are verbatim cuts). A *sequence* test runs one
//! object of each class through 4 to 40 random operations (Add_Key, Set_Key, Remove_Key, Clear_Keys, Set_Looping, Evaluate,
//! Evaluate_Derivative, Set_Tangents, Get_Tangents, Set_Tightness, Set_TCB_Params, the queries) and, after every operation,
//! dumps the object's whole state: every key, the looping flag, and for the Hermite family every tangent, the tangents-dirty flag
//! and (Cardinal, TCB) the tightnesses or parameters. Both sides' words (floats as bits) are compared whole; a NaN against a NaN
//! with other payload bits is not a mismatch (the NaN rule of tests/support).
//!
//! Inputs outside upstream's defined behaviour are not generated: Evaluate on no key; Evaluate inside the key range with one key
//! (`Find_Interval` reads `Keys[1]`); Evaluate_Derivative with fewer than two keys; for a 1D looping linear curve with one key at
//! the key's own time. `Update_Tangents` with fewer than two keys is undefined upstream for Catmull-Rom and TCB (no `return`)
//! and is reached only by those evaluations. Not covered: `Save` and `Load` of every class (their chunk files are stand-ins in the
//! wrapper), the persist factories, `LookupTableMgrClass::Get_Table`, `Save_Table_Desc` and `Load_Table_Desc` (the file system and
//! chunk files).
//!
//! # Predictions (written before the first run, 2026-11-18) and results
//!
//! Predicted: 0 mismatches for the linear and Hermite classes; the derived splines (Cardinal, Catmull-Rom, TCB) risked a mismatch where
//! the draft resolves the virtual `Update_Tangents` by hand; Euler 0 or a mismatch where `double` arithmetic was written for `float`.
//!
//! **Prediction missed (derived splines), draft defects corrected in `hermite_spline.rs` and the three derived files:** first run
//! 12,783 of 20,000 Cardinal 3D, 12,596 Catmull-Rom 3D, 12,837 TCB 3D, 5,825 Cardinal 1D and 5,739 Catmull-Rom 1D sequences
//! differed (Linear and Hermite 0). Two causes. (1) The derived `evaluate` updated the tangents BEFORE the range check; upstream's
//! `HermiteSpline::Evaluate` returns the first or last point first and updates only for an in-range time, so the draft changed the
//! tangents and the dirty flag on out-of-range evaluations (visible through `Get_Tangents`). (2) `Evaluate_Derivative` of the derived 3D
//! splines called the base's, whose `Update_Tangents` only clears the flag, so the derivative used stale tangents: the derived
//! classes had no `evaluate_derivative`. Fixed by splitting the base `evaluate`, `evaluate_derivative` into range check, update, and
//! interpolation. After: 0 of 20,000 for all 9 classes (about 90,000 evaluations each, 102,701 derivatives).
//! **Euler (draft defect, corrected, found by reading the C++ before the first run and then pinned by plant 10770):** `sy` and
//! `cy` were computed in `double`; upstream squares and adds the `float` matrix entries in `float` and only then takes the square
//! root in `double`. The original code (plant 10770) differs in 10,651 of 600,000 angle outputs and 44,955 of 2,400,000 matrix elements;
//! corrected, 0 of 200,000 matrices x 3 angles and x 12 elements, over all 24 orders and 8 more. Lookup table: 20,000 tables
//! (239,894 queries) and the default table (412 queries) 0 mismatches. Euler helpers and the 24 order constants: 0 of 33.
//!
//! Planted bugs (mismatching sequences of the 180,000 = 9 classes x 20,000 for 10726-10755): 10726 `Find_Interval` `>=`: 46,994; 10727
//! `Add_Key` `<=`: 62,257; 10728 linear `time > last`: 5,426; 10729 1D looping interval: 2,889; 10730 linear 1D `>`: 3,262; 10731
//! h2: 91,301; 10732 h1: 114,699; 10733 range `>=`: 21,725; 10734 dh2: 35,421; 10735 dh3 uses out: 34,927; 10736 tangent push: 48,425;
//! 10737 tangent pop: 24,766; 10738 looping not dirty: 30,303; 10739 no one-key early-out: 4,658; 10740 tangents swapped: 22,162; 10741
//! tightness 0: 40,000; 10742 inner tightness [0]: 6,135; 10743 factors swapped: 17,056; 10744 `end` not `end - 1`: 5,319; 10745
//! tightness not dirty: 9,584; 10746 end 0.5: 14,947; 10747 inner 0.25: 17,025; 10748 total time: 16,060; 10749 no update in 1D
//! evaluate: 17,632; 10750 TCB k1: 874; 10751 TCB 0.5: 14,854; 10752 TCB total time: 16,763; 10753 TCB params not dirty: 11,483; 10754
//! TCB bias 0.5: 20,000; 10755 TCB dp swap: 6,392. Lookup (of 239,894 queries): 10756 `len` not `len - 1`: 70,119 (Get_Value); 10757 lerp:
//! 104,388; 10758 `/ len`: 156,537 and 52,255 quick; 10759 `oo` with `+`: 80,813; 10760 `input > max`: caught by an index panic after
//! 14,991 differing queries; 10761 quick `+`: 104,716; 10762 min from the last key: 159,161. Euler (of 600,000 angles, 2,400,000
//! elements): 10763 parity: 299,950 / 529,132; 10764 frame: 197,770 / 572,024; 10765 `x2 >= pi`: 5 / 15 (the fewest); 10766 `16*eps` to `eps`:
//! 10,169 / 40,011; 10767 atan2 arguments: 94,887 / 549,245; 10768 sin for cos: 0 / 922,385; 10769 `sj*ci`: 0 / 97,639; 10770: above.
//! 45 of 45 caught.
//!
//!
//! # Planted bugs
//!
//! `L9_PLANT=<id>` (the run prints a `MEASURED` line and fails on purpose); the ids are listed beside each test's results.

mod support;

use rts_lockstep_engine::drafts::wwlib::cardinal_spline::{CardinalSpline1DClass, CardinalSpline3DClass};
use rts_lockstep_engine::drafts::wwlib::catmull_rom_spline::{CatmullRomSpline1DClass, CatmullRomSpline3DClass};
use rts_lockstep_engine::drafts::wwlib::curve::{LinearCurve1DClass, LinearCurve3DClass};
use rts_lockstep_engine::drafts::wwlib::euler::EulerAnglesClass;
use rts_lockstep_engine::drafts::wwlib::hermite_spline::{HermiteSpline1DClass, HermiteSpline3DClass};
use rts_lockstep_engine::drafts::wwlib::lookup_table::{Curve1D, LookupTableClass, LookupTableMgrClass};
use rts_lockstep_engine::drafts::wwlib::tcb_spline::TCBSpline3DClass;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use rts_lockstep_engine::ww::vector4::Vector4;
use support::{Check, Gen};

extern "C" {
    fn ref_curve_seq(kind: i32, inp: *const i32, nin: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_lookup_case(inp: *const i32, out: *mut i32, cap: i32) -> i32;
    fn ref_lookup_default(q: *const i32, nq: i32, out: *mut i32, cap: i32) -> i32;
    fn ref_euler_from_matrix(m: *const i32, order: i32, angles: *mut u64);
    fn ref_euler_round_trip(m: *const i32, order: i32, angles: *mut u64, back: *mut i32);
}

fn planted() -> bool {
    std::env::var("L9_PLANT").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0) != 0
}

fn cases(default: u64) -> u64 {
    std::env::var("DRAFT_CURVE_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn rust<T>(f: impl FnOnce() -> T) -> Option<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok()
}

fn finish(c: Check) {
    if planted() {
        c.report();
        panic!("a planted run: see the MEASURED line (a mismatch is the plant caught)");
    } else {
        c.finish();
    }
}

fn bits(f: f32) -> i32 {
    f.to_bits() as i32
}
fn real(i: i32) -> f32 {
    f32::from_bits(i as u32)
}

/// Words equal, or both a float NaN (whatever the payload).
fn same_words(a: &[i32], b: &[i32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| x == y || (real(x).is_nan() && real(y).is_nan()))
}

// ---------------------------------------------------------------------------------------------------------------------------
// The objects, on the Rust side
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Lin3 = 0,
    Her3 = 1,
    Car3 = 2,
    Cat3 = 3,
    Tcb3 = 4,
    Lin1 = 5,
    Her1 = 6,
    Car1 = 7,
    Cat1 = 8,
}

const KINDS: [Kind; 9] = [Kind::Lin3, Kind::Her3, Kind::Car3, Kind::Cat3, Kind::Tcb3, Kind::Lin1, Kind::Her1, Kind::Car1, Kind::Cat1];

impl Kind {
    fn is3(self) -> bool {
        (self as i32) < 5
    }
    fn hermite(self) -> bool {
        !matches!(self, Kind::Lin3 | Kind::Lin1)
    }
}

enum Obj {
    Lin3(LinearCurve3DClass),
    Her3(HermiteSpline3DClass),
    Car3(CardinalSpline3DClass),
    Cat3(CatmullRomSpline3DClass),
    Tcb3(TCBSpline3DClass),
    Lin1(LinearCurve1DClass),
    Her1(HermiteSpline1DClass),
    Car1(CardinalSpline1DClass),
    Cat1(CatmullRomSpline1DClass),
}

fn v3(w: &[i32]) -> Vector3 {
    Vector3::new(real(w[0]), real(w[1]), real(w[2]))
}

fn put3(out: &mut Vec<i32>, v: &Vector3) {
    out.extend([bits(v.x), bits(v.y), bits(v.z)]);
}

impl Obj {
    fn new(kind: Kind) -> Obj {
        match kind {
            Kind::Lin3 => Obj::Lin3(LinearCurve3DClass::default()),
            Kind::Her3 => Obj::Her3(HermiteSpline3DClass::new()),
            Kind::Car3 => Obj::Car3(CardinalSpline3DClass::default()),
            Kind::Cat3 => Obj::Cat3(CatmullRomSpline3DClass::default()),
            Kind::Tcb3 => Obj::Tcb3(TCBSpline3DClass::default()),
            Kind::Lin1 => Obj::Lin1(LinearCurve1DClass::default()),
            Kind::Her1 => Obj::Her1(HermiteSpline1DClass::new()),
            Kind::Car1 => Obj::Car1(CardinalSpline1DClass::default()),
            Kind::Cat1 => Obj::Cat1(CatmullRomSpline1DClass::default()),
        }
    }

    /// One operation (8 words: [op, i, w0..w5]); pushes its results.
    fn op(&mut self, w: &[i32], out: &mut Vec<i32>) {
        let (op, i) = (w[0], w[1]);
        let p = v3(&w[2..5]);
        match self {
            Obj::Lin3(c) => match op {
                0 => out.push(c.base.add_key(&p, real(w[5]))),
                1 => c.base.set_key(i, &p),
                2 => c.base.remove_key(i),
                3 => c.base.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => put3(out, &c.evaluate(real(w[2]))),
                11 => out.extend([c.base.key_count(), c.base.is_looping() as i32, bits(c.base.get_start_time()), bits(c.base.get_end_time())]),
                12 => {
                    let (pt, t) = c.base.get_key(i);
                    put3(out, &pt);
                    out.push(bits(t));
                }
                _ => {}
            },
            Obj::Her3(c) => match op {
                0 => out.push(c.add_key(&p, real(w[5]))),
                1 => c.set_key(i, &p),
                2 => c.remove_key(i),
                3 => c.clear_keys(),
                4 => c.set_looping(w[2] != 0),
                5 => put3(out, &c.evaluate(real(w[2]))),
                6 => put3(out, &c.evaluate_derivative(real(w[2]))),
                7 => c.set_tangents(i, &p, &v3(&w[5..8])),
                8 => {
                    let (a, b) = c.get_tangents(i);
                    put3(out, &a);
                    put3(out, &b);
                }
                11 => out.extend([c.base.key_count(), c.base.is_looping() as i32, bits(c.base.get_start_time()), bits(c.base.get_end_time())]),
                12 => {
                    let (pt, t) = c.base.get_key(i);
                    put3(out, &pt);
                    out.push(bits(t));
                }
                _ => {}
            },
            Obj::Car3(c) => match op {
                0 => out.push(c.add_key(&p, real(w[5]))),
                1 => c.base.set_key(i, &p),
                2 => c.remove_key(i),
                3 => c.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => put3(out, &c.evaluate(real(w[2]))),
                6 => put3(out, &c.evaluate_derivative(real(w[2]))),
                7 => c.base.set_tangents(i, &p, &v3(&w[5..8])),
                8 => {
                    let (a, b) = c.base.get_tangents(i);
                    put3(out, &a);
                    put3(out, &b);
                }
                9 => c.set_tightness(i, real(w[2])),
                10 => out.push(bits(c.get_tightness(i))),
                11 => out.extend([c.base.base.key_count(), c.base.base.is_looping() as i32, bits(c.base.base.get_start_time()), bits(c.base.base.get_end_time())]),
                12 => {
                    let (pt, t) = c.base.base.get_key(i);
                    put3(out, &pt);
                    out.push(bits(t));
                }
                _ => {}
            },
            Obj::Cat3(c) => match op {
                0 => out.push(c.base.add_key(&p, real(w[5]))),
                1 => c.base.set_key(i, &p),
                2 => c.base.remove_key(i),
                3 => c.base.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => put3(out, &c.evaluate(real(w[2]))),
                6 => put3(out, &c.evaluate_derivative(real(w[2]))),
                7 => c.base.set_tangents(i, &p, &v3(&w[5..8])),
                8 => {
                    let (a, b) = c.base.get_tangents(i);
                    put3(out, &a);
                    put3(out, &b);
                }
                11 => out.extend([c.base.base.key_count(), c.base.base.is_looping() as i32, bits(c.base.base.get_start_time()), bits(c.base.base.get_end_time())]),
                12 => {
                    let (pt, t) = c.base.base.get_key(i);
                    put3(out, &pt);
                    out.push(bits(t));
                }
                _ => {}
            },
            Obj::Tcb3(c) => match op {
                0 => out.push(c.add_key(&p, real(w[5]))),
                1 => c.base.set_key(i, &p),
                2 => c.remove_key(i),
                3 => c.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => put3(out, &c.evaluate(real(w[2]))),
                6 => put3(out, &c.evaluate_derivative(real(w[2]))),
                7 => c.base.set_tangents(i, &p, &v3(&w[5..8])),
                8 => {
                    let (a, b) = c.base.get_tangents(i);
                    put3(out, &a);
                    put3(out, &b);
                }
                9 => c.set_tcb_params(i, real(w[2]), real(w[3]), real(w[4])),
                10 => {
                    let (a, b, d) = c.get_tcb_params(i);
                    out.extend([bits(a), bits(b), bits(d)]);
                }
                11 => out.extend([c.base.base.key_count(), c.base.base.is_looping() as i32, bits(c.base.base.get_start_time()), bits(c.base.base.get_end_time())]),
                12 => {
                    let (pt, t) = c.base.base.get_key(i);
                    put3(out, &pt);
                    out.push(bits(t));
                }
                _ => {}
            },
            Obj::Lin1(c) => match op {
                0 => out.push(c.base.add_key(real(w[2]), real(w[3]), i as u32)),
                1 => c.base.set_key(i, real(w[2]), w[3] as u32),
                2 => c.base.remove_key(i),
                3 => c.base.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => out.push(bits(c.evaluate(real(w[2])))),
                11 => out.extend([c.base.key_count(), c.base.is_looping() as i32, bits(c.base.get_start_time()), bits(c.base.get_end_time())]),
                12 => {
                    let (pt, t, e) = c.base.get_key(i);
                    out.extend([bits(pt), bits(t), e as i32]);
                }
                _ => {}
            },
            Obj::Her1(c) => match op {
                0 => out.push(c.add_key(real(w[2]), real(w[3]), i as u32)),
                1 => c.set_key(i, real(w[2]), w[3] as u32),
                2 => c.remove_key(i),
                3 => c.clear_keys(),
                4 => c.set_looping(w[2] != 0),
                5 => out.push(bits(c.evaluate(real(w[2])))),
                7 => c.set_tangents(i, real(w[2]), real(w[3])),
                8 => {
                    let (a, b) = c.get_tangents(i);
                    out.extend([bits(a), bits(b)]);
                }
                11 => out.extend([c.base.key_count(), c.base.is_looping() as i32, bits(c.base.get_start_time()), bits(c.base.get_end_time())]),
                12 => {
                    let (pt, t, e) = c.base.get_key(i);
                    out.extend([bits(pt), bits(t), e as i32]);
                }
                _ => {}
            },
            Obj::Car1(c) => match op {
                0 => out.push(c.add_key(real(w[2]), real(w[3]))),
                1 => c.base.set_key(i, real(w[2]), w[3] as u32),
                2 => c.remove_key(i),
                3 => c.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => out.push(bits(c.evaluate(real(w[2])))),
                7 => c.base.set_tangents(i, real(w[2]), real(w[3])),
                8 => {
                    let (a, b) = c.base.get_tangents(i);
                    out.extend([bits(a), bits(b)]);
                }
                9 => c.set_tightness(i, real(w[2])),
                10 => out.push(bits(c.get_tightness(i))),
                11 => out.extend([c.base.base.key_count(), c.base.base.is_looping() as i32, bits(c.base.base.get_start_time()), bits(c.base.base.get_end_time())]),
                12 => {
                    let (pt, t, e) = c.base.base.get_key(i);
                    out.extend([bits(pt), bits(t), e as i32]);
                }
                _ => {}
            },
            Obj::Cat1(c) => match op {
                0 => out.push(c.base.add_key(real(w[2]), real(w[3]), i as u32)),
                1 => c.base.set_key(i, real(w[2]), w[3] as u32),
                2 => c.base.remove_key(i),
                3 => c.base.clear_keys(),
                4 => c.base.set_looping(w[2] != 0),
                5 => out.push(bits(c.evaluate(real(w[2])))),
                7 => c.base.set_tangents(i, real(w[2]), real(w[3])),
                8 => {
                    let (a, b) = c.base.get_tangents(i);
                    out.extend([bits(a), bits(b)]);
                }
                11 => out.extend([c.base.base.key_count(), c.base.base.is_looping() as i32, bits(c.base.base.get_start_time()), bits(c.base.base.get_end_time())]),
                12 => {
                    let (pt, t, e) = c.base.base.get_key(i);
                    out.extend([bits(pt), bits(t), e as i32]);
                }
                _ => {}
            },
        }
    }

    /// The state dump, in the wrapper's order.
    fn dump(&self, out: &mut Vec<i32>) {
        fn keys3(c: &rts_lockstep_engine::drafts::wwlib::curve::Curve3DClass, out: &mut Vec<i32>) {
            out.push(c.keys.len() as i32);
            for k in &c.keys {
                put3(out, &k.point);
                out.push(bits(k.time));
            }
            out.push(c.is_looping as i32);
        }
        fn keys1(c: &rts_lockstep_engine::drafts::wwlib::curve::Curve1DClass, out: &mut Vec<i32>) {
            out.push(c.keys.len() as i32);
            for k in &c.keys {
                out.extend([bits(k.point), bits(k.time), k.extra as i32]);
            }
            out.push(c.is_looping as i32);
        }
        fn tan3(h: &HermiteSpline3DClass, out: &mut Vec<i32>) {
            out.push(h.tangents.len() as i32);
            for t in &h.tangents {
                put3(out, &t.in_tangent);
                put3(out, &t.out_tangent);
            }
            out.push(h.tangents_dirty as i32);
        }
        fn tan1(h: &HermiteSpline1DClass, out: &mut Vec<i32>) {
            out.push(h.tangents.len() as i32);
            for t in &h.tangents {
                out.extend([bits(t.in_tangent), bits(t.out_tangent)]);
            }
            out.push(h.tangents_dirty as i32);
        }
        match self {
            Obj::Lin3(c) => keys3(&c.base, out),
            Obj::Her3(c) => {
                keys3(&c.base, out);
                tan3(c, out);
            }
            Obj::Car3(c) => {
                keys3(&c.base.base, out);
                tan3(&c.base, out);
                out.push(c.tightness.len() as i32);
                out.extend(c.tightness.iter().map(|&t| bits(t)));
            }
            Obj::Cat3(c) => {
                keys3(&c.base.base, out);
                tan3(&c.base, out);
            }
            Obj::Tcb3(c) => {
                keys3(&c.base.base, out);
                tan3(&c.base, out);
                out.push(c.params.len() as i32);
                for p in &c.params {
                    out.extend([bits(p.tension), bits(p.continuity), bits(p.bias)]);
                }
            }
            Obj::Lin1(c) => keys1(&c.base, out),
            Obj::Her1(c) => {
                keys1(&c.base, out);
                tan1(c, out);
            }
            Obj::Car1(c) => {
                keys1(&c.base.base, out);
                tan1(&c.base, out);
                out.push(c.tightness.len() as i32);
                out.extend(c.tightness.iter().map(|&t| bits(t)));
            }
            Obj::Cat1(c) => {
                keys1(&c.base.base, out);
                tan1(&c.base, out);
            }
        }
    }
}

fn rust_seq(kind: Kind, ops: &[i32]) -> Vec<i32> {
    let mut o = Obj::new(kind);
    let mut out = Vec::new();
    for w in ops.chunks(8) {
        out.push(w[0]);
        o.op(w, &mut out);
        o.dump(&mut out);
    }
    out
}

fn cpp_seq(kind: Kind, ops: &[i32]) -> Vec<i32> {
    let mut out = vec![0i32; 400_000];
    let n = unsafe { ref_curve_seq(kind as i32, ops.as_ptr(), ops.len() as i32, out.as_mut_ptr(), out.len() as i32) };
    assert!(n >= 0 && (n as usize) <= out.len(), "output overflow");
    out.truncate(n as usize);
    out
}

// ---------------------------------------------------------------------------------------------------------------------------
// The generator: only operations upstream defines
// ---------------------------------------------------------------------------------------------------------------------------

fn gen_time(g: &mut Gen) -> f32 {
    match g.below(8) {
        0 => g.below(10) as f32,
        1 => g.below(100) as f32 * 0.25,
        2 => g.uniform(0.0, 1.0),
        3 => g.uniform(-5.0, 5.0),
        _ => g.uniform(0.0, 20.0),
    }
}

fn gen_val(g: &mut Gen) -> f32 {
    match g.below(10) {
        0 => g.below(9) as f32 - 4.0,
        1 => g.mixed(),
        _ => g.uniform(-100.0, 100.0),
    }
}

/// Whether `Evaluate` (op 5) or `Evaluate_Derivative` (op 6) at `time` is defined for the model state.
fn eval_ok(kind: Kind, op: i32, looping: bool, times: &[f32], time: f32) -> bool {
    let n = times.len();
    if n == 0 {
        return false;
    }
    let nan = time.is_nan();
    let outside = !nan && (time < times[0] || time > times[n - 1]);
    if op == 6 {
        return n >= 2;
    }
    match kind {
        Kind::Lin3 => n >= 2 || !nan,
        Kind::Lin1 => !looping && (n >= 2 || !nan) || looping && (n >= 2 || outside),
        Kind::Her3 | Kind::Car3 | Kind::Cat3 | Kind::Tcb3 => n >= 2 || outside,
        Kind::Her1 | Kind::Car1 | Kind::Cat1 => true,
    }
}

fn gen_ops(g: &mut Gen, kind: Kind) -> Vec<i32> {
    let mut ops: Vec<i32> = Vec::new();
    let mut times: Vec<f32> = Vec::new();
    let mut looping = false;
    let steps = 4 + g.below(36);
    let push = |ops: &mut Vec<i32>, op: i32, i: i32, w: [i32; 6]| {
        ops.push(op);
        ops.push(i);
        ops.extend(w);
    };
    let pt = |g: &mut Gen| -> [i32; 3] { [bits(gen_val(g)), bits(gen_val(g)), bits(gen_val(g))] };
    for s in 0..steps {
        let n = times.len();
        // the first few operations build the key set
        let want_add = if s < 2 + g.below(5) { true } else { g.below(6) == 0 };
        if want_add {
            let t = if n > 0 && g.below(6) == 0 { times[g.below(n as u32) as usize] } else { gen_time(g) };
            let idx = times.iter().position(|&x| !(x < t)).unwrap_or(n);
            times.insert(idx, t);
            if kind.is3() {
                let p = pt(g);
                push(&mut ops, 0, 0, [p[0], p[1], p[2], bits(t), 0, 0]);
            } else {
                let extra = if g.below(3) == 0 { g.below(5) as i32 } else { 0 };
                push(&mut ops, 0, extra, [bits(gen_val(g)), bits(t), 0, 0, 0, 0]);
            }
            continue;
        }
        let choice = g.below(100);
        match choice {
            0..=39 => {
                // Evaluate (or its derivative)
                let op = if kind.is3() && kind.hermite() && g.below(4) == 0 { 6 } else { 5 };
                let t = if n > 0 {
                    match g.below(10) {
                        0 => times[g.below(n as u32) as usize],
                        1 => times[0],
                        2 => times[n - 1],
                        3 => times[g.below(n as u32) as usize] + g.uniform(-0.01, 0.01),
                        4 => g.uniform(times[0] - 3.0, times[n - 1] + 3.0),
                        5 => [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, 1.0e30, -1.0e30][g.below(5) as usize],
                        _ => g.uniform(times[0], times[n - 1] + 0.001),
                    }
                } else {
                    0.0
                };
                if eval_ok(kind, op, looping, &times, t) {
                    push(&mut ops, op, 0, [bits(t), 0, 0, 0, 0, 0]);
                }
            }
            40..=47 if n > 0 => {
                let i = g.below(n as u32) as i32;
                if kind.is3() {
                    let p = pt(g);
                    push(&mut ops, 1, i, [p[0], p[1], p[2], 0, 0, 0]);
                } else {
                    push(&mut ops, 1, i, [bits(gen_val(g)), g.below(4) as i32, 0, 0, 0, 0]);
                }
            }
            48..=53 if n > 0 => {
                let i = g.below(n as u32) as usize;
                times.remove(i);
                push(&mut ops, 2, i as i32, [0; 6]);
            }
            54..=55 => {
                times.clear();
                push(&mut ops, 3, 0, [0; 6]);
            }
            56..=62 => {
                looping = g.below(2) == 0;
                push(&mut ops, 4, 0, [looping as i32, 0, 0, 0, 0, 0]);
            }
            63..=72 if n > 0 && kind.hermite() => {
                let i = g.below(n as u32) as i32;
                if kind.is3() {
                    let a = pt(g);
                    let b = pt(g);
                    push(&mut ops, 7, i, [a[0], a[1], a[2], b[0], b[1], b[2]]);
                } else {
                    push(&mut ops, 7, i, [bits(gen_val(g)), bits(gen_val(g)), 0, 0, 0, 0]);
                }
            }
            73..=77 if n > 0 && kind.hermite() => {
                let i = g.below(n as u32) as i32;
                push(&mut ops, 8, i, [0; 6]);
            }
            78..=88 if n > 0 && matches!(kind, Kind::Car3 | Kind::Car1) => {
                let i = g.below(n as u32) as i32;
                if g.below(2) == 0 {
                    push(&mut ops, 9, i, [bits(g.uniform(-1.0, 1.0)), 0, 0, 0, 0, 0]);
                } else {
                    push(&mut ops, 10, i, [0; 6]);
                }
            }
            78..=88 if n > 0 && kind == Kind::Tcb3 => {
                let i = g.below(n as u32) as i32;
                if g.below(3) != 0 {
                    push(&mut ops, 9, i, [bits(g.uniform(-1.0, 1.0)), bits(g.uniform(-1.0, 1.0)), bits(g.uniform(-1.0, 1.0)), 0, 0, 0]);
                } else {
                    push(&mut ops, 10, i, [0; 6]);
                }
            }
            89..=94 => push(&mut ops, 11, 0, [0; 6]),
            _ if n > 0 => {
                let i = g.below(n as u32) as i32;
                push(&mut ops, 12, i, [0; 6]);
            }
            _ => {}
        }
    }
    ops
}

#[test]
fn curves_and_splines_match_upstream_over_random_operation_sequences() {
    let n = cases(20_000);
    let mut checks: Vec<Check> = KINDS.iter().map(|k| Check::new(&format!("{k:?} sequences"))).collect();
    let mut g = Gen::new(0x215_8001);
    let mut evals = [0u64; 9];
    let mut derivs = 0u64;
    for i in 0..n {
        for (ki, &kind) in KINDS.iter().enumerate() {
            let ops = gen_ops(&mut g, kind);
            for w in ops.chunks(8) {
                if w[0] == 5 {
                    evals[ki] += 1;
                }
                if w[0] == 6 {
                    derivs += 1;
                }
            }
            let c = cpp_seq(kind, &ops);
            let r = rust(|| rust_seq(kind, &ops));
            let ok = r.as_ref().map(|r| same_words(r, &c)).unwrap_or(false);
            checks[ki].compared += 1;
            if !ok {
                checks[ki].mismatches += 1;
                if checks[ki].examples.len() < 2 {
                    let r = r.unwrap_or_default();
                    let k = r.iter().zip(&c).position(|(a, b)| a != b && !(real(*a).is_nan() && real(*b).is_nan())).unwrap_or(r.len().min(c.len()));
                    let from = k.saturating_sub(4);
                    checks[ki].examples.push(format!(
                        "sequence {i} {kind:?}: {} ops; first difference at word {k}: rust {:?} c++ {:?}",
                        ops.len() / 8,
                        &r[from.min(r.len())..(k + 6).min(r.len())],
                        &c[from.min(c.len())..(k + 6).min(c.len())]
                    ));
                }
            }
        }
    }
    println!("{n} sequences of each of 9 classes; evaluations per class {evals:?}; {derivs} derivatives");
    let mut all = Check::new("curves and splines (all classes)");
    for c in &checks {
        println!("DIFF {:<40} sequences {:>7}  mismatching {:>7}", c.name, c.compared, c.mismatches);
        for e in &c.examples {
            println!("    e.g. {e}");
        }
        all.compared += c.compared;
        all.mismatches += c.mismatches;
        all.examples.extend(c.examples.iter().take(2).cloned());
    }
    finish(all);
}

// ---------------------------------------------------------------------------------------------------------------------------
// LookupTableClass
// ---------------------------------------------------------------------------------------------------------------------------

fn build_curve(kind: i32, looping: bool, keys: &[(f32, f32, u32)], tangents: &[(f32, f32)]) -> Curve1D {
    match kind {
        5 => {
            let mut c = LinearCurve1DClass::default();
            for &(p, t, e) in keys {
                c.base.add_key(p, t, e);
            }
            c.base.set_looping(looping);
            Curve1D::Linear(c)
        }
        6 => {
            let mut c = HermiteSpline1DClass::new();
            for &(p, t, e) in keys {
                c.add_key(p, t, e);
            }
            for (i, &(a, b)) in tangents.iter().enumerate() {
                c.set_tangents(i as i32, a, b);
            }
            c.set_looping(looping);
            Curve1D::Hermite(c)
        }
        7 => {
            let mut c = CardinalSpline1DClass::default();
            for &(p, t, _) in keys {
                c.add_key(p, t);
            }
            c.base.set_looping(looping);
            Curve1D::Cardinal(c)
        }
        _ => {
            let mut c = CatmullRomSpline1DClass::default();
            for &(p, t, e) in keys {
                c.base.add_key(p, t, e);
            }
            c.base.set_looping(looping);
            Curve1D::CatmullRom(c)
        }
    }
}

#[test]
fn lookup_table_matches_upstream() {
    let n = cases(20_000);
    let mut g = Gen::new(0x215_9001);
    let mut c_samples = Check::new("LookupTableClass::Init (samples)");
    let mut c_value = Check::new("LookupTableClass::Get_Value");
    let mut c_quick = Check::new("LookupTableClass::Get_Value_Quick");
    for i in 0..n {
        let kind = 5 + g.below(4) as i32;
        let nkeys = 1 + g.below(8) as usize;
        let looping = g.below(4) == 0;
        // a one-key looping linear curve reads Keys[1] in Init: not generated
        let looping = looping && !(kind == 5 && nkeys == 1);
        let nsamples = match g.below(6) {
            0 => 2,
            1 => 256,
            _ => 2 + g.below(40) as i32,
        };
        let keys: Vec<(f32, f32, u32)> = (0..nkeys).map(|_| (gen_val(&mut g), gen_time(&mut g), g.below(4))).collect();
        let tangents: Vec<(f32, f32)> = (0..nkeys).map(|_| (gen_val(&mut g), gen_val(&mut g))).collect();
        let mut sorted: Vec<f32> = keys.iter().map(|k| k.1).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let (lo, hi) = (sorted[0], sorted[nkeys - 1]);
        let mut queries: Vec<f32> = Vec::new();
        for _ in 0..12 {
            let q = match g.below(8) {
                0 => lo,
                1 => hi,
                2 => lo - g.uniform(0.0, 5.0),
                3 => hi + g.uniform(0.0, 5.0),
                _ => g.uniform(lo, hi),
            };
            // not within a thousandth of the span below the top: `(len - 1) * (input - min) * oo` may round to `len - 1`
            // there and Get_Value reads one sample past the table (undefined upstream)
            if q < hi && hi - q <= (hi - lo) * 1.0e-3 {
                continue;
            }
            queries.push(q);
        }
        let mut input = vec![kind, looping as i32, nsamples, nkeys as i32, queries.len() as i32];
        for k in &keys {
            input.extend([bits(k.0), bits(k.1), k.2 as i32]);
        }
        if kind == 6 {
            for t in &tangents {
                input.extend([bits(t.0), bits(t.1)]);
            }
        }
        input.extend(queries.iter().map(|&q| bits(q)));
        let mut out = vec![0i32; 4000];
        let nc = unsafe { ref_lookup_case(input.as_ptr(), out.as_mut_ptr(), out.len() as i32) } as usize;
        out.truncate(nc);
        let ns = nsamples as usize;
        let cpp_samples = &out[3..3 + ns];
        let rs = rust(|| {
            let mut curve = build_curve(kind, looping, &keys, &tangents);
            let mut t = LookupTableClass::new(nsamples);
            t.init("T", &mut curve);
            let mut samples = Vec::new();
            for k in 0..ns {
                // the table's samples are not public: read them back through Get_Value at the sample abscissae
                let _ = k;
            }
            let vals: Vec<(i32, i32)> = queries.iter().map(|&q| (bits(t.get_value(q)), bits(t.get_value_quick(q)))).collect();
            samples.push(0);
            (vals, t.get_name().to_string())
        });
        match rs {
            Some((vals, name)) => {
                assert_eq!(name, "T");
                for (qi, (v, qk)) in vals.iter().enumerate() {
                    let cv = out[3 + ns + 2 * qi];
                    let ck = out[3 + ns + 2 * qi + 1];
                    c_value.eq(*v == cv || (real(*v).is_nan() && real(cv).is_nan()), true, || format!("case {i}: kind {kind} looping {looping} {nkeys} keys, {nsamples} samples, query {} rust {:#x} c++ {cv:#x}", queries[qi], v));
                    c_quick.eq(*qk == ck || (real(*qk).is_nan() && real(ck).is_nan()), true, || format!("case {i}: kind {kind} query {} rust {:#x} c++ {ck:#x}", queries[qi], qk));
                }
            }
            None => {
                c_value.eq(false, true, || format!("case {i}: kind {kind} looping {looping}: the Rust panicked"));
            }
        }
        let _ = cpp_samples;
        c_samples.compared += 1;
    }
    println!("{n} tables");
    c_quick.finish_or_report();
    finish(c_value);
}

trait FinishOrReport {
    fn finish_or_report(self);
}
impl FinishOrReport for Check {
    fn finish_or_report(self) {
        if planted() {
            self.report();
        } else {
            self.finish();
        }
    }
}

#[test]
fn lookup_default_table_matches_upstream() {
    // LookupTableMgrClass::Init's default table: two keys of 0.5 at times 0 and 1, two samples
    let mut g = Gen::new(0x215_9002);
    let mut mgr = LookupTableMgrClass::default();
    mgr.init();
    let mut c = Check::new("LookupTableMgrClass::Init default table (Get_Value, Get_Value_Quick)");
    let mut queries: Vec<f32> = vec![0.0, 1.0, -1.0, 2.0, 0.5, 0.25, 0.999];
    for _ in 0..200 {
        queries.push(g.uniform(-0.5, 1.5));
    }
    queries.retain(|&q| !(q < 1.0 && 1.0 - q <= 1.0e-3));
    let qbits: Vec<i32> = queries.iter().map(|&q| bits(q)).collect();
    let mut out = vec![0i32; 1000];
    let nc = unsafe { ref_lookup_default(qbits.as_ptr(), qbits.len() as i32, out.as_mut_ptr(), out.len() as i32) } as usize;
    out.truncate(nc);
    let t = mgr.get_table("DefaultTable", None).expect("the default table").clone();
    assert_eq!(t.get_name(), "DefaultTable");
    for (qi, &q) in queries.iter().enumerate() {
        c.eq(bits(t.get_value(q)), out[5 + 2 * qi], || format!("Get_Value({q})"));
        c.eq(bits(t.get_value_quick(q)), out[5 + 2 * qi + 1], || format!("Get_Value_Quick({q})"));
    }
    finish(c);
}

// ---------------------------------------------------------------------------------------------------------------------------
// EulerAnglesClass
// ---------------------------------------------------------------------------------------------------------------------------

fn rot_matrix(axis: usize, angle: f64) -> [[f64; 3]; 3] {
    let (s, c) = angle.sin_cos();
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        1 => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        _ => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

fn mat_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn gen_matrix(g: &mut Gen) -> [f32; 12] {
    let mut m = [0.0f32; 12];
    match g.below(6) {
        0 | 1 => {
            // a rotation about three axes by random angles
            let a = |g: &mut Gen| g.uniform(-3.2, 3.2) as f64;
            let r = mat_mul(&mat_mul(&rot_matrix(0, a(g)), &rot_matrix(1, a(g))), &rot_matrix(2, a(g)));
            for i in 0..3 {
                for j in 0..3 {
                    m[i * 4 + j] = r[i][j] as f32;
                }
            }
        }
        2 => {
            // gimbal lock and exact quarter turns: angles that are multiples of a right angle, and values at the 16 * epsilon edge
            let q = |g: &mut Gen| (g.below(9) as f64 - 4.0) * std::f64::consts::FRAC_PI_2;
            let r = mat_mul(&mat_mul(&rot_matrix(0, q(g)), &rot_matrix(1, q(g))), &rot_matrix(2, q(g)));
            for i in 0..3 {
                for j in 0..3 {
                    let mut v = r[i][j] as f32;
                    if v.abs() < 1.0e-6 {
                        v = [0.0, 1.0e-7, -1.0e-7, 1.9e-6, 1.0e-6][g.below(5) as usize];
                    }
                    m[i * 4 + j] = v;
                }
            }
        }
        3 => {
            for v in m.iter_mut() {
                *v = [0.0, 1.0, -1.0, 0.5, f32::EPSILON * 16.0, f32::EPSILON * 15.0, f32::EPSILON * 17.0][g.below(7) as usize];
            }
        }
        _ => {
            for v in m.iter_mut() {
                *v = g.uniform(-1.5, 1.5);
            }
        }
    }
    for r in 0..3 {
        m[r * 4 + 3] = g.uniform(-100.0, 100.0);
    }
    m
}

fn to_matrix3d(m: &[f32; 12]) -> Matrix3D {
    Matrix3D { row: [Vector4::new(m[0], m[1], m[2], m[3]), Vector4::new(m[4], m[5], m[6], m[7]), Vector4::new(m[8], m[9], m[10], m[11])] }
}

#[test]
fn euler_angles_match_upstream() {
    let n = cases(200_000);
    let mut g = Gen::new(0x215_A001);
    let mut c_angles = Check::new("EulerAnglesClass::From_Matrix (angles, doubles)");
    let mut c_back = Check::new("EulerAnglesClass::To_Matrix (after From_Matrix)");
    let mut by_order = [0u64; 32];
    for i in 0..n {
        let m = gen_matrix(&mut g);
        let order = match g.below(10) {
            0 => g.below(64) as i32,
            _ => g.below(24) as i32,
        };
        by_order[(order & 31) as usize] += 1;
        let mb: Vec<i32> = m.iter().map(|&f| bits(f)).collect();
        let mut angles = [0u64; 3];
        let mut back = [0i32; 12];
        unsafe { ref_euler_round_trip(mb.as_ptr(), order, angles.as_mut_ptr(), back.as_mut_ptr()) };
        let rs = rust(|| {
            let e = EulerAnglesClass::from_matrix_order(&to_matrix3d(&m), order);
            let a = [e.get_angle(0).to_bits(), e.get_angle(1).to_bits(), e.get_angle(2).to_bits()];
            let mut r = Matrix3D::identity();
            e.to_matrix(&mut r);
            let mut flat = [0i32; 12];
            for rr in 0..3 {
                for cc in 0..4 {
                    flat[rr * 4 + cc] = bits(r[rr][cc]);
                }
            }
            (a, flat)
        });
        match rs {
            Some((a, flat)) => {
                for k in 0..3 {
                    c_angles.compared += 1;
                    if a[k] != angles[k] && !(f64::from_bits(a[k]).is_nan() && f64::from_bits(angles[k]).is_nan()) {
                        c_angles.mismatches += 1;
                        if c_angles.examples.len() < 6 {
                            c_angles.examples.push(format!("case {i} order {order} angle {k}: rust {:e} ({:#x}) c++ {:e} ({:#x}); matrix {m:?}", f64::from_bits(a[k]), a[k], f64::from_bits(angles[k]), angles[k]));
                        }
                    }
                }
                for k in 0..12 {
                    c_back.compared += 1;
                    if flat[k] != back[k] && !(real(flat[k]).is_nan() && real(back[k]).is_nan()) {
                        c_back.mismatches += 1;
                        if c_back.examples.len() < 6 {
                            c_back.examples.push(format!("case {i} order {order} element {k}: rust {:e} c++ {:e}", real(flat[k]), real(back[k])));
                        }
                    }
                }
            }
            None => {
                c_angles.compared += 1;
                c_angles.mismatches += 1;
            }
        }
    }
    println!("{n} matrices; by order {by_order:?}");
    c_back.finish_or_report();
    finish(c_angles);
}

#[test]
fn euler_helpers_and_order_constants_match_upstream() {
    use rts_lockstep_engine::drafts::wwlib::euler::*;
    extern "C" {
        fn ref_euler_helpers(order: i32, out: *mut i32);
        fn ref_euler_orders(out: *mut i32);
    }
    let mut c = Check::new("euler.cpp helpers (_euler_axis_*, _euler_unpack_order, EULER_FRAME/REPEAT/PARITY)");
    for order in 0..32 {
        let mut o = [0i32; 14];
        unsafe { ref_euler_helpers(order, o.as_mut_ptr()) };
        let (i, j, k, h, n, s, f) = euler_unpack_order(order);
        let r = [euler_axis_i(order), euler_axis_j(order), euler_axis_k(order), euler_axis_h(order), i as i32, j as i32, k as i32, h, n, s, f, euler_frame(order), euler_repeat(order), euler_parity(order)];
        c.eq(r, o, || format!("order {order}"));
    }
    let mut o = [0i32; 24];
    unsafe { ref_euler_orders(o.as_mut_ptr()) };
    let r = [
        EULER_ORDER_XYZS, EULER_ORDER_XYXS, EULER_ORDER_XZYS, EULER_ORDER_XZXS, EULER_ORDER_YZXS, EULER_ORDER_YZYS, EULER_ORDER_YXZS, EULER_ORDER_YXYS, EULER_ORDER_ZXYS, EULER_ORDER_ZXZS, EULER_ORDER_ZYXS,
        EULER_ORDER_ZYZS, EULER_ORDER_XYZR, EULER_ORDER_XYXR, EULER_ORDER_XZYR, EULER_ORDER_XZXR, EULER_ORDER_YZXR, EULER_ORDER_YZYR, EULER_ORDER_YXZR, EULER_ORDER_YXYR, EULER_ORDER_ZXYR, EULER_ORDER_ZXZR,
        EULER_ORDER_ZYXR, EULER_ORDER_ZYZR,
    ];
    c.eq(r, o, || "the 24 order constants".to_string());
    finish(c);
}
