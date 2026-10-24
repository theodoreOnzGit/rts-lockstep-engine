// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of Phase 1 layer 1 (`docs/phase1-porting-graph.md`):
//! the value types and templates of `Libraries/Include/Lib/BaseType.h`
//! (`common::base_type`), and the game-wide enums and name tables of
//! `GameType.h`/`.cpp`, `GameCommon.h`/`.cpp`, `Errors.h`,
//! `SpecialPowerType.h`, `ThingSort.h`, `Powers.h`, `WeaponSetType.h`,
//! `WeaponStatus.h`, `WeaponBonusConditionFlags.h`, `ObjectScriptStatusBits.h`
//! (upstream commit 0a05454d).
//!
//! References: the header's own text compiled by g++
//! (`differential/wrap_value_types.cpp`, see build.rs) and the unchanged
//! headers and `.cpp` files (`differential/wrap_game_types.cpp`).
//!
//! Inputs: 100,000 seeded random cases per function (`support::Gen::mixed`,
//! `geom`, the special list: zeros, signs, subnormals, inf, NaN, rounding
//! boundaries) plus edge cases. Integer functions get values up to the
//! overflow limits; inputs where upstream is undefined (signed overflow, an
//! out-of-range `double`-to-`Int` conversion, a shift of a negative value,
//! `normalizeAngle` that never terminates) are **not** sent to the C++; the
//! test counts them and checks that the Rust returns an error there.
//! Outputs are compared as raw bits.
//!
//! **Planted bugs.** Every comparison is run a second time against a
//! deliberately wrong variant of the Rust (a plausible slip: a missing clamp,
//! `<=` for `<`, 256 for 255, a swapped sign, ...), which must be caught
//! (`Check::finish_planted`). That shows the inputs can tell the translation
//! from a near miss.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::common::base_type::*;
use rts_lockstep_engine::common::game_common::*;
use rts_lockstep_engine::common::{errors, game_type, special_power_type, thing_sort, trig};
use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::logic::{object_script_status_bits as ossb, powers, weapon_types as wt};
use std::ffi::CStr;
use support::{Check, Gen, N};

fn c3(v: [f32; 3]) -> Coord3D {
    Coord3D {
        x: v[0],
        y: v[1],
        z: v[2],
    }
}

#[test]
fn coord2d_matches() {
    let mut g = Gen::new(101);
    let names = ["Coord2D::length", "Coord2D::normalize", "Coord2D::toAngle"];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        for _ in 0..N {
            let (x, y) = if g.below(4) == 0 {
                (g.mixed(), g.mixed())
            } else {
                (g.geom(), g.geom())
            };
            let c = Coord2D { x, y };
            let mut out = [0.0f32; 2];
            let r = unsafe { cpp::ref_vt_coord2d(op as i32, x, y, out.as_mut_ptr()) };
            let input = || format!("({x:e}, {y:e})");
            match op {
                0 => {
                    ok.f32(c.length(), r, input);
                    // planted: sqrt of the double sum of squares (the sum not rounded to float first)
                    let wrong = petir::Real::sqrt(
                        f64::from(x) * f64::from(x) + f64::from(y) * f64::from(y),
                    ) as f32;
                    bug.f32(wrong, r, input);
                }
                1 => {
                    let mut n = c;
                    n.normalize();
                    ok.f32s(&[n.x, n.y], &out, input);
                    // planted: multiply by the reciprocal instead of dividing
                    let len = c.length();
                    let wrong = if len != 0.0 {
                        [x * (1.0 / len), y * (1.0 / len)]
                    } else {
                        [x, y]
                    };
                    bug.f32s(&wrong, &out, input);
                }
                _ => {
                    ok.f32(c.to_angle(), r, input);
                    // planted: no clamp of x/len before ACos
                    let len = c.length();
                    let wrong = if len == 0.0 {
                        0.0
                    } else if y < 0.0 {
                        -trig::acos(x / len)
                    } else {
                        trig::acos(x / len)
                    };
                    bug.f32(wrong, r, input);
                }
            }
        }
        ok.finish();
        bug.finish_planted();
    }
}

#[test]
fn coord3d_matches() {
    let mut g = Gen::new(102);
    let names = [
        "Coord3D::length",
        "Coord3D::lengthSqr",
        "Coord3D::normalize",
        "Coord3D::crossProduct",
        "Coord3D::add",
        "Coord3D::sub",
        "Coord3D::scale",
        "Coord3D::equals",
        "Coord3D::operator==",
        "Coord3D::set(const Coord3D*)",
        "Coord3D::set(x, y, z)",
        "Coord3D::zero",
    ];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        for _ in 0..N {
            let f = if g.below(4) == 0 {
                Gen::mixed
            } else {
                Gen::geom
            };
            let a: [f32; 3] = g.vec(f);
            let mut b: [f32; 3] = g.vec(f);
            if g.below(8) == 0 {
                b = a; // equal vectors for the comparisons
                if g.below(2) == 0 {
                    b[g.below(3) as usize] *= -1.0; // and signed zeros
                }
            }
            let k = g.mixed();
            let mut out = [0.0f32; 3];
            let r = unsafe {
                cpp::ref_vt_coord3d(op as i32, a.as_ptr(), b.as_ptr(), k, out.as_mut_ptr())
            };
            let (ca, cb) = (c3(a), c3(b));
            let input = || format!("a {a:?} b {b:?} k {k:e}");
            let v = |c: Coord3D| [c.x, c.y, c.z];
            let flag = |t: bool| if t { 1.0f32 } else { 0.0 };
            match op {
                0 => {
                    ok.f32(ca.length(), r, input);
                    // planted: sum in the other order (z*z + y*y + x*x)
                    bug.f32(
                        petir::Real::sqrt(f64::from(a[2] * a[2] + a[1] * a[1] + a[0] * a[0]))
                            as f32,
                        r,
                        input,
                    );
                }
                1 => {
                    ok.f32(ca.length_sqr(), r, input);
                    bug.f32(a[0] * a[0] + (a[1] * a[1] + a[2] * a[2]), r, input);
                }
                2 => {
                    let mut n = ca;
                    n.normalize();
                    ok.f32s(&v(n), &out, input);
                    let len = ca.length();
                    let wrong = if len != 0.0 {
                        [a[0] * (1.0 / len), a[1] * (1.0 / len), a[2] * (1.0 / len)]
                    } else {
                        a
                    };
                    bug.f32s(&wrong, &out, input);
                }
                3 => {
                    ok.f32s(&v(Coord3D::cross_product(&ca, &cb)), &out, input);
                    // planted: y component with the sign swapped
                    let x = Coord3D::cross_product(&ca, &cb);
                    bug.f32s(&[x.x, a[0] * b[2] - a[2] * b[0], x.z], &out, input);
                }
                4 | 5 => {
                    let mut s = ca;
                    if op == 4 {
                        s.add(&cb)
                    } else {
                        s.sub(&cb)
                    }
                    ok.f32s(&v(s), &out, input);
                    // planted: add takes z from a.y (a copy-paste slip); sub is b - a
                    let w = if op == 4 {
                        [a[0] + b[0], a[1] + b[1], a[2] + b[1]]
                    } else {
                        let mut w = cb;
                        w.sub(&ca);
                        v(w)
                    };
                    bug.f32s(&w, &out, input);
                }
                6 => {
                    let mut s = ca;
                    s.scale(k);
                    ok.f32s(&v(s), &out, input);
                    bug.f32s(&[a[0] * k, a[1] * k, a[2] / (1.0 / k)], &out, input);
                }
                7 | 8 => {
                    let e = if op == 7 { ca.equals(&cb) } else { ca == cb };
                    ok.f32(flag(e), r, input);
                    // planted: compare bit patterns (-0 != 0)
                    bug.f32(flag(a.map(f32::to_bits) == b.map(f32::to_bits)), r, input);
                }
                9 => {
                    let s = cb;
                    ok.f32s(&v(s), &out, input);
                    bug.f32s(&a, &out, input);
                }
                10 => {
                    let mut s = ca;
                    s.set(k, k, k);
                    ok.f32s(&v(s), &out, input);
                    bug.f32s(&[k, k, a[2]], &out, input);
                }
                _ => {
                    let mut s = ca;
                    s.zero();
                    ok.f32s(&v(s), &out, input);
                    bug.f32s(&a, &out, input);
                }
            }
        }
        ok.finish();
        bug.finish_planted();
    }
}

#[test]
fn regions_match() {
    let mut g = Gen::new(103);
    let names = [
        "Region2D::width",
        "Region2D::height",
        "Region3D::width",
        "Region3D::height",
        "Region3D::depth",
        "Region3D::isInRegionNoZ",
        "Region3D::isInRegionWithZ",
        "Region3D::zero",
    ];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        for _ in 0..N {
            let lo: [f32; 3] = g.vec(Gen::geom);
            let hi: [f32; 3] = g.vec(Gen::geom);
            let mut q: [f32; 3] = g.vec(Gen::geom);
            // queries on the boundary, where `<` and `<=` differ
            for i in 0..3 {
                match g.below(6) {
                    0 => q[i] = lo[i],
                    1 => q[i] = hi[i],
                    _ => {}
                }
            }
            let mut out = [0.0f32; 6];
            let r = unsafe {
                cpp::ref_vt_region(
                    op as i32,
                    lo.as_ptr(),
                    hi.as_ptr(),
                    q.as_ptr(),
                    out.as_mut_ptr(),
                )
            };
            let r2 = Region2D {
                lo: Coord2D { x: lo[0], y: lo[1] },
                hi: Coord2D { x: hi[0], y: hi[1] },
            };
            let r3 = Region3D {
                lo: c3(lo),
                hi: c3(hi),
            };
            let input = || format!("lo {lo:?} hi {hi:?} q {q:?}");
            let flag = |t: bool| if t { 1.0f32 } else { 0.0 };
            let (good, wrong) = match op {
                0 => (r2.width(), r2.lo.x - r2.hi.x),
                1 => (r2.height(), r2.hi.x - r2.lo.y),
                2 => (r3.width(), r3.hi.y - r3.lo.x),
                3 => (r3.height(), r3.hi.y - r3.lo.x),
                4 => (r3.depth(), r3.hi.z - r3.lo.y),
                5 => (
                    flag(r3.is_in_region_no_z(&c3(q))),
                    flag(lo[0] <= q[0] && q[0] < hi[0] && lo[1] < q[1] && q[1] < hi[1]),
                ),
                6 => (
                    flag(r3.is_in_region_with_z(&c3(q))),
                    flag(
                        lo[0] < q[0]
                            && q[0] < hi[0]
                            && lo[1] < q[1]
                            && q[1] < hi[1]
                            && lo[2] < q[2]
                            && q[2] <= hi[2],
                    ),
                ),
                _ => {
                    let mut z = r3;
                    z.zero();
                    ok.f32s(
                        &[z.lo.x, z.lo.y, z.lo.z, z.hi.x, z.hi.y, z.hi.z],
                        &out,
                        input,
                    );
                    bug.f32s(&[0.0, 0.0, 0.0, hi[0], hi[1], hi[2]], &out, input);
                    continue;
                }
            };
            ok.f32(good, r, input);
            bug.f32(wrong, r, input);
        }
        ok.finish();
        bug.finish_planted();
    }
}

/// A random `Int` biased towards the overflow limits of squares and sums.
fn int_input(g: &mut Gen) -> i32 {
    match g.below(8) {
        0 => g.i32(),
        1 => 46340 - g.below(3) as i32,
        2 => -(46340 - g.below(3) as i32),
        3 => (g.below(70000) as i32) - 35000,
        4 => [0, 1, -1, i32::MAX, i32::MIN, 26754, -26754][g.below(7) as usize],
        _ => (g.below(4001) as i32) - 2000,
    }
}

#[test]
fn integer_types_match_where_defined() {
    let mut g = Gen::new(104);
    let names = [
        "ICoord2D::length",
        "ICoord3D::length",
        "IRegion2D::width",
        "IRegion2D::height",
        "IRegion3D::width",
        "IRegion3D::height",
        "IRegion3D::depth",
        "ICoord3D::zero",
    ];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        let mut undefined = 0u64;
        for _ in 0..N {
            let lo = [int_input(&mut g), int_input(&mut g), int_input(&mut g)];
            let hi = [int_input(&mut g), int_input(&mut g), int_input(&mut g)];
            let a2 = ICoord2D { x: lo[0], y: lo[1] };
            let a3 = ICoord3D {
                x: lo[0],
                y: lo[1],
                z: lo[2],
            };
            let r2 = IRegion2D {
                lo: a2,
                hi: ICoord2D { x: hi[0], y: hi[1] },
            };
            let r3 = IRegion3D {
                lo: a3,
                hi: ICoord3D {
                    x: hi[0],
                    y: hi[1],
                    z: hi[2],
                },
            };
            let rust: Result<i32, BaseTypeError> = match op {
                0 => a2.try_length(),
                1 => a3.try_length(),
                2 => r2.try_width(),
                3 => r2.try_height(),
                4 => r3.try_width(),
                5 => r3.try_height(),
                6 => r3.try_depth(),
                _ => {
                    let mut z = a3;
                    z.zero();
                    Ok(z.x | z.y | z.z)
                }
            };
            // Independent definedness check (i64 arithmetic) before calling C++.
            let wide = |v: &[i32]| v.iter().map(|&c| i64::from(c) * i64::from(c)).sum::<i64>();
            let in_range = |v: i64| v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX);
            let squares_ok = |v: &[i32]| {
                v.iter().all(|&c| in_range(i64::from(c) * i64::from(c)))
                    && (1..=v.len()).all(|n| in_range(wide(&v[..n])))
            };
            let defined = match op {
                0 => squares_ok(&lo[..2]),
                1 => squares_ok(&lo),
                2 | 4 => in_range(i64::from(hi[0]) - i64::from(lo[0])),
                3 | 5 => in_range(i64::from(hi[1]) - i64::from(lo[1])),
                6 => in_range(i64::from(hi[2]) - i64::from(lo[2])),
                _ => true,
            };
            if !defined {
                undefined += 1;
                assert_eq!(
                    rust,
                    Err(BaseTypeError::SignedOverflow),
                    "{name}: lo {lo:?} hi {hi:?}"
                );
                continue;
            }
            let c = unsafe { cpp::ref_vt_int(op as i32, lo.as_ptr(), hi.as_ptr()) };
            let input = || format!("lo {lo:?} hi {hi:?}");
            let v =
                rust.unwrap_or_else(|e| panic!("{name}: {e} on defined input lo {lo:?} hi {hi:?}"));
            ok.eq(v, c, input);
            // planted: lengths rounded instead of truncated; widths off by one; zero leaves z
            let wrong = match op {
                0 => (petir::Real::sqrt(wide(&lo[..2]) as f64) + 0.5) as i32,
                1 => (petir::Real::sqrt(wide(&lo) as f64) + 0.5) as i32,
                7 => lo[2],
                _ => v.wrapping_add(i32::from(g.below(64) == 0)),
            };
            bug.eq(wrong, c, input);
        }
        println!("     {name}: {undefined} inputs left out (Int overflow, undefined upstream; Rust returned Err)");
        ok.finish();
        bug.finish_planted();
    }
}

#[test]
fn rgb_color_matches() {
    let mut g = Gen::new(105);
    let mut ok = Check::new("RGBColor::getAsInt");
    let mut bug = Check::new("RGBColor::getAsInt");
    let mut undefined = 0u64;
    for _ in 0..N {
        let comp = |g: &mut Gen| match g.below(6) {
            0 => g.mixed(),
            1 => g.uniform(-0.01, 0.01),
            2 => g.below(256) as f32 / 255.0,
            3 => g.uniform(0.0, 300.0),
            _ => g.uniform(0.0, 1.0),
        };
        let rgb = [comp(&mut g), comp(&mut g), comp(&mut g)];
        let c = RGBColor {
            red: rgb[0],
            green: rgb[1],
            blue: rgb[2],
        };
        // Independent definedness check: each (Int)(c*255.0) in range and >= 0,
        // and its shifted value within 32 unsigned bits.
        let defined = rgb.iter().zip([16u32, 8, 0]).all(|(&v, sh)| {
            let d = f64::from(v) * 255.0;
            d.is_finite()
                && d.trunc() >= 0.0
                && d.trunc() <= f64::from(i32::MAX)
                && d.trunc() * f64::from(1u32 << sh) <= f64::from(u32::MAX)
        });
        match c.try_get_as_int() {
            Err(_) if !defined => {
                undefined += 1;
                continue;
            }
            Err(e) => panic!("getAsInt: {e} on defined input {rgb:?}"),
            Ok(v) => {
                assert!(defined, "getAsInt: Ok on undefined input {rgb:?}");
                let r = unsafe { cpp::ref_vt_rgb_get_as_int(rgb.as_ptr()) };
                ok.eq(v, r, || format!("{rgb:?}"));
                // planted: scale in float (red * 255.0f) instead of double
                let wrong = ((rgb[0] * 255.0) as i32) << 16
                    | ((rgb[1] * 255.0) as i32) << 8
                    | (rgb[2] * 255.0) as i32;
                bug.eq(wrong, r, || format!("{rgb:?}"));
            }
        }
    }
    println!("     getAsInt: {undefined} inputs left out (undefined conversion or shift; Rust returned Err)");
    ok.finish();
    bug.finish_planted();

    let mut ok = Check::new("RGBColor::setFromInt");
    let mut bug = Check::new("RGBColor::setFromInt");
    for i in 0..N {
        let v = if i < 256 {
            (i as i32) * 0x0001_0101
        } else {
            g.i32()
        };
        let mut c = RGBColor::default();
        c.set_from_int(v);
        let mut out = [0.0f32; 3];
        unsafe { cpp::ref_vt_rgb_set_from_int(v, out.as_mut_ptr()) };
        ok.f32s(&[c.red, c.green, c.blue], &out, || format!("{v:#x}"));
        let w = |b: i32| (b & 0xff) as f32 * (1.0 / 255.0); // planted: reciprocal
        bug.f32s(&[w(v >> 16), w(v >> 8), w(v)], &out, || format!("{v:#x}"));
    }
    ok.finish();
    bug.finish_planted();
}

#[test]
fn templates_match() {
    let mut g = Gen::new(106);
    let names = [
        "sqr<Real>",
        "clamp<Real>",
        "sign<Real>",
        "rad2deg",
        "deg2rad",
        "min",
        "max",
        "RealRange::combine lo",
        "RealRange::combine hi",
    ];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        for _ in 0..N {
            let mut v = [g.mixed(), g.mixed(), g.mixed(), g.mixed()];
            if op == 1 && g.below(2) == 0 {
                v[0] = -1.0;
                v[2] = 1.0;
            }
            let [a, b, c, d] = v;
            let r = unsafe { cpp::ref_vt_real(op as i32, a, b, c, d) };
            let input = || format!("{a:e} {b:e} {c:e} {d:e}");
            let mut rr = RealRange { lo: a, hi: b };
            rr.combine(&RealRange { lo: c, hi: d });
            let (good, wrong) = match op {
                0 => (sqr(a), a * a.abs()),
                1 => (
                    clamp(a, b, c),
                    if b > c {
                        c
                    } else if b < a {
                        a
                    } else {
                        b
                    },
                ),
                2 => (sign(a) as f32, if a >= 0.0 { 1.0 } else { -1.0 }),
                3 => (rad2deg(a), a * 180.0 / PI),
                4 => (deg2rad(a), a * PI / 180.0),
                5 => (min(a, b), a.min(b)),
                6 => (max(a, b), a.max(b)),
                7 => (rr.lo, a.min(c)),
                _ => (rr.hi, b.max(d)),
            };
            ok.f32(good, r, input);
            bug.f32(wrong, r, input);
        }
        ok.finish();
        bug.finish_planted();
    }

    let names = ["sqr<Int>", "clamp<Int>", "sign<Int>"];
    for (op, name) in names.iter().enumerate() {
        let mut ok = Check::new(name);
        let mut bug = Check::new(name);
        let mut undefined = 0u64;
        for _ in 0..N {
            let (a, b, c) = (int_input(&mut g), int_input(&mut g), int_input(&mut g));
            let good = match op {
                0 => match try_sqr_int(a) {
                    Ok(v) => v,
                    Err(_) => {
                        assert!(i64::from(a) * i64::from(a) > i64::from(i32::MAX));
                        undefined += 1;
                        continue;
                    }
                },
                1 => clamp(a, b, c),
                _ => sign(a),
            };
            let r = unsafe { cpp::ref_vt_intfn(op as i32, a, b, c) };
            ok.eq(good, r, || format!("{a} {b} {c}"));
            let wrong = match op {
                0 => good.wrapping_add(i32::from(a % 7 == 3)),
                1 => b.max(a).min(c),
                _ => a.signum() * i32::from(a != 1),
            };
            bug.eq(wrong, r, || format!("{a} {b} {c}"));
        }
        if op == 0 {
            println!("     sqr<Int>: {undefined} inputs left out (Int overflow, undefined upstream; Rust returned Err)");
        }
        ok.finish();
        bug.finish_planted();
    }
}

fn enum_values(which: i32) -> Vec<i64> {
    let mut out = vec![0i64; 256];
    let n = unsafe { cpp::ref_enum_values(which, out.as_mut_ptr(), out.len() as i32) };
    out.truncate(n as usize);
    out
}

fn names(which: i32) -> Vec<String> {
    let mut v = Vec::new();
    for i in 0.. {
        let p = unsafe { cpp::ref_name_table(which, i) };
        if p.is_null() {
            break;
        }
        v.push(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned());
    }
    v
}

#[test]
fn enums_and_name_tables_match() {
    use game_type::*;
    let game_type: Vec<i64> = [
        DEFAULT_WORLD_WIDTH as i64,
        DEFAULT_WORLD_HEIGHT as i64,
        INVALID_ID as i64,
        FORCE_ID_TO_LONG_SIZE as i64,
        INVALID_DRAWABLE_ID as i64,
        FORCE_ID_TO_LONG_SIZE as i64,
        NO_FORMATION_ID as i64,
        FORCE_ID_TO_LONG_SIZE as i64,
        TimeOfDay::Invalid as i64,
        TIME_OF_DAY_FIRST as i64,
        TimeOfDay::Morning as i64,
        TimeOfDay::Afternoon as i64,
        TimeOfDay::Evening as i64,
        TimeOfDay::Night as i64,
        TIME_OF_DAY_COUNT as i64,
        Weather::Normal as i64,
        Weather::Snowy as i64,
        WEATHER_COUNT as i64,
        SCORCH_1 as i64,
        SCORCH_2 as i64,
        SCORCH_3 as i64,
        SCORCH_4 as i64,
        SHADOW_SCORCH as i64,
        SCORCH_COUNT as i64,
        WeaponSlotType::Primary as i64,
        WeaponSlotType::Secondary as i64,
        WeaponSlotType::Tertiary as i64,
        WEAPONSLOT_COUNT as i64,
        LAYER_INVALID as i64,
        LAYER_GROUND as i64,
        LAYER_WALL as i64,
        LAYER_LAST as i64,
    ]
    .to_vec();
    let game_common: Vec<i64> = [
        LOGICFRAMES_PER_SECOND as i64,
        MSEC_PER_SECOND as i64,
        MAX_PLAYER_COUNT as i64,
        std::mem::size_of::<PlayerMaskType>() as i64,
        PLAYERMASK_ALL as i64,
        PLAYERMASK_NONE as i64,
        MAX_GLOBAL_GENERAL_TYPES as i64,
        GLOBAL_GENERAL_BEGIN as i64,
        GLOBAL_GENERAL_END as i64,
        GameDifficulty::Easy as i64,
        GameDifficulty::Normal as i64,
        GameDifficulty::Hard as i64,
        DIFFICULTY_COUNT as i64,
        PlayerType::Human as i64,
        PlayerType::Computer as i64,
        PLAYERTYPE_COUNT as i64,
        CellShroudStatus::Clear as i64,
        CellShroudStatus::Fogged as i64,
        CellShroudStatus::Shrouded as i64,
        CELLSHROUD_COUNT as i64,
        ObjectShroudStatus::Invalid as i64,
        ObjectShroudStatus::Clear as i64,
        ObjectShroudStatus::PartialClear as i64,
        ObjectShroudStatus::Fogged as i64,
        ObjectShroudStatus::Shrouded as i64,
        ObjectShroudStatus::InvalidButPreviousValid as i64,
        OBJECTSHROUD_COUNT as i64,
        GuardMode::Normal as i64,
        GuardMode::GuardWithoutPursuit as i64,
        GuardMode::GuardFlyingUnitsOnly as i64,
        NEVER as i64,
        FOREVER as i64,
        VeterancyLevel::Regular as i64,
        VeterancyLevel::Veteran as i64,
        VeterancyLevel::Elite as i64,
        VeterancyLevel::Heroic as i64,
        LEVEL_COUNT as i64,
        CommandSourceType::FromPlayer as i64,
        CommandSourceType::FromScript as i64,
        CommandSourceType::FromAi as i64,
        CommandSourceType::FromDozer as i64,
        CommandSourceType::DefaultSwitchWeapon as i64,
        ATTACK_FORCED_BIT as i64,
        ATTACK_CONTINUED_BIT as i64,
        ATTACK_TUNNELNETWORK_GUARD_BIT as i64,
        ATTACK_NEW_TARGET as i64,
        ATTACK_NEW_TARGET_FORCED as i64,
        ATTACK_CONTINUED_TARGET as i64,
        ATTACK_CONTINUED_TARGET_FORCED as i64,
        ATTACK_TUNNEL_NETWORK_GUARD as i64,
        TURRET_INVALID as i64,
        TURRET_MAIN as i64,
        TURRET_ALT as i64,
        MAX_TURRETS as i64,
        Relationship::Enemies as i64,
        Relationship::Neutral as i64,
        Relationship::Allies as i64,
    ]
    .to_vec();
    use errors::*;
    let errs: Vec<i64> = [
        ERROR_BASE,
        ERROR_BUG,
        ERROR_OUT_OF_MEMORY,
        ERROR_BAD_ARG,
        ERROR_INVALID_FILE_VERSION,
        ERROR_CORRUPT_FILE_FORMAT,
        ERROR_BAD_INI,
        ERROR_INVALID_D3D,
        ERROR_LAST,
    ]
    .iter()
    .map(|&v| v as i64)
    .collect();
    use thing_sort::*;
    let sort: Vec<i64> = [
        ES_FIRST,
        ES_NONE,
        ES_STRUCTURE,
        ES_INFANTRY,
        ES_VEHICLE,
        ES_SHRUBBERY,
        ES_MISC_MAN_MADE,
        ES_MISC_NATURAL,
        ES_DEBRIS,
        ES_SYSTEM,
        ES_AUDIO,
        ES_TEST,
        ES_FOR_REVIEW,
        ES_ROAD,
        ES_WAYPOINT,
        ES_NUM_SORTING_TYPES,
    ]
    .iter()
    .map(|&v| v as i64)
    .collect();
    use powers::*;
    let pw: Vec<i64> = [
        POWER_NONE,
        POWER_FASTER,
        POWER_DOUBLE_SHOT,
        POWER_SELF_HEALING,
        POWERS_NUM_POWERS,
    ]
    .iter()
    .map(|&v| v as i64)
    .collect();
    let mut weapon: Vec<i64> = [
        wt::WEAPONSET_VETERAN,
        wt::WEAPONSET_ELITE,
        wt::WEAPONSET_HERO,
        wt::WEAPONSET_PLAYER_UPGRADE,
        wt::WEAPONSET_CRATEUPGRADE_ONE,
        wt::WEAPONSET_CRATEUPGRADE_TWO,
        wt::WEAPONSET_VEHICLE_HIJACK,
        wt::WEAPONSET_CARBOMB,
        wt::WEAPONSET_MINE_CLEARING_DETAIL,
        wt::WEAPONSET_RIDER1,
        wt::WEAPONSET_RIDER2,
        wt::WEAPONSET_RIDER3,
        wt::WEAPONSET_RIDER4,
        wt::WEAPONSET_RIDER5,
        wt::WEAPONSET_RIDER6,
        wt::WEAPONSET_RIDER7,
        wt::WEAPONSET_RIDER8,
        wt::WEAPONSET_COUNT,
        wt::WeaponStatus::ReadyToFire as i32,
        wt::WeaponStatus::OutOfAmmo as i32,
        wt::WeaponStatus::BetweenFiringShots as i32,
        wt::WeaponStatus::ReloadingClip as i32,
        wt::WeaponStatus::PreAttack as i32,
        wt::WEAPON_STATUS_COUNT,
        std::mem::size_of::<wt::WeaponBonusConditionFlags>() as i32,
    ]
    .iter()
    .map(|&v| v as i64)
    .collect();
    weapon.extend(
        [
            ossb::OBJECT_STATUS_SCRIPT_DISABLED,
            ossb::OBJECT_STATUS_SCRIPT_UNPOWERED,
            ossb::OBJECT_STATUS_SCRIPT_UNSELLABLE,
            ossb::OBJECT_STATUS_SCRIPT_UNSTEALTHED,
            ossb::OBJECT_STATUS_SCRIPT_TARGETABLE,
        ]
        .iter()
        .map(|&v| v as i64),
    );
    let spt: Vec<i64> = special_power_type::SPECIAL_POWER_TYPE_ENUMERATORS
        .iter()
        .map(|&(_, v)| v as i64)
        .collect();

    let tables: [(&str, Vec<i64>); 7] = [
        ("GameType.h enumerators", game_type),
        ("GameCommon.h enumerators", game_common),
        ("Errors.h enumerators", errs),
        ("ThingSort.h enumerators", sort),
        ("Powers.h enumerators", pw),
        ("weapon headers, ObjectScriptStatusBits.h", weapon),
        ("SpecialPowerType.h enumerators", spt),
    ];
    for (which, (name, rust)) in tables.into_iter().enumerate() {
        let c = enum_values(which as i32);
        let mut ok = Check::new(name);
        ok.eq(rust.len(), c.len(), || "count".into());
        for (i, (&r, &v)) in rust.iter().zip(c.iter()).enumerate() {
            ok.eq(r, v, || format!("entry {i}"));
        }
        ok.finish();
        // planted: the list with two neighbours swapped where their values differ
        let mut bug = Check::new(name);
        let mut w = rust.clone();
        if let Some(i) = (1..w.len()).find(|&i| w[i] != w[i - 1]) {
            w.swap(i, i - 1);
        }
        for (i, (&r, &v)) in w.iter().zip(c.iter()).enumerate() {
            bug.eq(r, v, || format!("entry {i}"));
        }
        bug.finish_planted();
    }

    let tables: [(&str, Vec<&str>); 6] = [
        ("TimeOfDayNames", TIME_OF_DAY_NAMES.to_vec()),
        ("WeatherNames", WEATHER_NAMES.to_vec()),
        ("TheVeterancyNames", VETERANCY_NAMES.to_vec()),
        ("TheRelationshipNames", RELATIONSHIP_NAMES.to_vec()),
        ("EditorSortingNames", EDITOR_SORTING_NAMES.to_vec()),
        ("PowerNames", POWER_NAMES.to_vec()),
    ];
    for (which, (name, rust)) in tables.into_iter().enumerate() {
        let c = names(which as i32);
        let mut ok = Check::new(name);
        ok.eq(rust.len(), c.len(), || "count".into());
        for (i, (r, v)) in rust.iter().zip(c.iter()).enumerate() {
            ok.eq(*r, v.as_str(), || format!("entry {i}"));
        }
        ok.finish();
        let mut bug = Check::new(name);
        let wrong: Vec<String> = rust.iter().map(|s| s.to_lowercase()).collect(); // planted: case
        for (i, (r, v)) in wrong.iter().zip(c.iter()).enumerate() {
            bug.eq(r.as_str(), v.as_str(), || format!("entry {i}"));
        }
        bug.finish_planted();
    }
}

#[test]
fn normalize_angle_and_attack_types_match() {
    let mut g = Gen::new(107);
    let mut ok = Check::new("normalizeAngle");
    let mut bug = Check::new("normalizeAngle");
    let mut ok2 = Check::new("stdAngleDiff");
    let mut bug2 = Check::new("stdAngleDiff");
    let mut hangs = 0u64;
    for i in 0..N {
        let a = match g.below(5) {
            0 => g.mixed(),
            1 => g.uniform(-20.0, 20.0),
            2 => [PI, -PI, 2.0 * PI, -2.0 * PI, 3.0 * PI, 0.0, -0.0][i % 7],
            3 => g.uniform(-1.0e5, 1.0e5),
            _ => g.uniform(-4.0, 4.0),
        };
        let b = g.uniform(-10.0, 10.0);
        let r = match try_normalize_angle(a) {
            Ok(r) => r,
            Err(_) => {
                // Independent check: a step of 2*PI no longer changes it.
                assert!(
                    a.is_infinite() || a - 2.0 * PI == a || a + 2.0 * PI == a,
                    "hang claimed for {a:e}"
                );
                hangs += 1;
                continue;
            }
        };
        // Bound the C++ loop to inputs the Rust finished.
        if a.abs() > 1.0e6 {
            hangs += 1;
            continue;
        }
        let c = unsafe { cpp::ref_game_common_angle(0, a, 0.0) };
        ok.f32(r, c, || format!("{a:e}"));
        // planted: one remainder instead of repeated subtraction
        let wrong = if a.is_nan() {
            0.0
        } else {
            let mut x = a % (2.0 * PI);
            if x > PI {
                x -= 2.0 * PI
            } else if x <= -PI {
                x += 2.0 * PI
            }
            x
        };
        bug.f32(wrong, c, || format!("{a:e}"));
        if let Ok(d) = try_std_angle_diff(a, b) {
            if (a - b).abs() <= 1.0e6 || (a - b).is_nan() {
                let c = unsafe { cpp::ref_game_common_angle(1, a, b) };
                ok2.f32(d, c, || format!("{a:e} {b:e}"));
                bug2.f32(try_normalize_angle(a + b).unwrap_or(0.0), c, || {
                    format!("{a:e} {b:e}")
                }); // planted: a + b
            }
        }
    }
    println!("     normalizeAngle: {hangs} inputs left out (upstream never terminates, or above 1e6 to bound the C++ loop)");
    ok.finish();
    bug.finish_planted();
    ok2.finish();
    bug2.finish_planted();

    let mut ok = Check::new("isForcedAttack/isContinuedAttack");
    let mut bug = Check::new("isForcedAttack/isContinuedAttack");
    for t in -16..64 {
        for op in 0..2 {
            let r = if op == 0 {
                is_forced_attack(t)
            } else {
                is_continued_attack(t)
            };
            let c = unsafe { cpp::ref_attack_type(op, t) } != 0;
            ok.eq(r, c, || format!("op {op} t {t}"));
            let w = if op == 0 {
                t & 3 != 0
            } else {
                is_continued_attack(t)
            };
            bug.eq(w, c, || format!("op {op} t {t}"));
        }
    }
    ok.finish();
    bug.finish_planted();
}

/// `DiscreteCircle` (DiscreteCircle.cpp, `common::discrete_circle`): edges
/// and draw calls for every radius 0..=600 at random centres, and 20,000
/// random (centre, radius) pairs up to radius 5,000 and centres within
/// +-2^20, plus centres near the `Int` limits. Inputs where upstream is
/// undefined (a negative radius throws; `Int` overflow) are not sent to the
/// C++; the Rust must return an error there. Planted bug: duplicate rows
/// keep the first (narrowest) edge instead of the last.
#[test]
fn discrete_circle_matches() {
    use rts_lockstep_engine::common::discrete_circle::*;
    let mut g = Gen::new(108);
    let mut ok = Check::new("DiscreteCircle edges and draw calls");
    let mut bug = Check::new("DiscreteCircle edges and draw calls");
    let mut undefined = 0u32;
    let mut cases: Vec<(i32, i32, i32)> = (0..=600)
        .map(|r| (g.i32() % 1000, g.i32() % 1000, r))
        .collect();
    for _ in 0..20_000 {
        let r = match g.below(4) {
            0 => g.below(5001) as i32,
            1 => -(g.below(5) as i32),
            _ => g.below(64) as i32,
        };
        let (x, y) = match g.below(10) {
            0 => (
                i32::MAX - g.below(100) as i32,
                i32::MIN + g.below(100) as i32,
            ),
            1 => (g.i32(), g.i32()),
            _ => ((g.i32() % (1 << 20)), (g.i32() % (1 << 20))),
        };
        cases.push((x, y, r));
    }
    for (x, y, r) in cases {
        let rust = DiscreteCircle::new(x, y, r).and_then(|c| {
            let mut calls = Vec::new();
            c.draw_circle(|a, b, yy| calls.extend_from_slice(&[a, b, yy]))?;
            Ok((c.get_edges().to_vec(), calls))
        });
        // Independent definedness check, in i64.
        let (x6, y6, r6) = (i64::from(x), i64::from(y), i64::from(r));
        let fits = |v: i64| v >= i64::from(i32::MIN) && v <= i64::from(i32::MAX);
        let defined = r >= 0
            && fits(2 * y6)
            && fits(2 * r6)
            && fits(x6 - r6)
            && fits(x6 + r6)
            && fits(y6 + r6)
            && fits(2 * y6 - (y6 + r6))
            && fits(2 * y6 - y6);
        let (edges, calls) = match rust {
            Ok(v) => v,
            Err(e) => {
                assert!(
                    !defined,
                    "({x}, {y}, {r}): {e} on an input upstream defines"
                );
                undefined += 1;
                continue;
            }
        };
        assert!(
            defined,
            "({x}, {y}, {r}): Ok on an input upstream leaves undefined"
        );
        let mut ce = vec![0i32; 3 * 20_000];
        let mut cc = vec![0i32; 6 * 20_000];
        let mut ncalls = 0;
        let n = unsafe {
            cpp::ref_discrete_circle(
                x,
                y,
                r,
                ce.as_mut_ptr(),
                ce.len() as i32,
                cc.as_mut_ptr(),
                cc.len() as i32,
                &mut ncalls,
            )
        };
        assert!(n >= 0, "buffers");
        let flat: Vec<i32> = edges
            .iter()
            .flat_map(|e| [e.y_pos, e.x_start, e.x_end])
            .collect();
        ok.eq(
            (flat.clone(), calls),
            (
                ce[..3 * n as usize].to_vec(),
                cc[..3 * ncalls as usize].to_vec(),
            ),
            || format!("({x}, {y}, {r})"),
        );
        // planted: keep the first edge of each row
        let mut first: Vec<i32> = Vec::new();
        let mut gen_edges = Vec::new();
        {
            let (mut xx, mut yy, mut d) = (0i64, r6, (1 - r6) * 2);
            while yy >= 0 {
                gen_edges.push([y6 + yy, x6 - xx, x6 + xx]);
                if d + yy > 0 {
                    yy -= 1;
                    d -= yy * 2 - 1;
                }
                if xx > d {
                    xx += 1;
                    d += xx * 2 + 1;
                }
            }
        }
        let mut last_row = None;
        for e in gen_edges {
            if last_row != Some(e[0]) {
                first.extend(e.iter().map(|&v| v as i32));
                last_row = Some(e[0]);
            }
        }
        bug.eq(first, ce[..3 * n as usize].to_vec(), || {
            format!("({x}, {y}, {r})")
        });
    }
    println!("     DiscreteCircle: {undefined} inputs left out (negative radius or Int overflow; Rust returned Err)");
    ok.finish();
    bug.finish_planted();
}
