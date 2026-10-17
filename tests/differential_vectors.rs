// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `ww::vector2`, `ww::vector3`, `ww::vector4`
//! against upstream `WWMath/vector2.h`, `vector3.h`, `vector4.h`
//! (commit 0a05454d), compiled unchanged.
//!
//! Every operation is driven with 100k random operand sets (mostly |v| <= 100,
//! with 20% drawn from the full mixed generator: NaN, inf, subnormals, huge
//! values) and every output component is compared as bits. See
//! `tests/support/mod.rs` for the NaN-payload rule and the libm binding.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::vector2::{self, Vector2};
use rts_lockstep_engine::ww::vector3::{self, Vector3};
use rts_lockstep_engine::ww::vector4::Vector4;
use support::{Check, Gen, N};

fn v2(a: [f32; 2]) -> Vector2 {
    Vector2::new(a[0], a[1])
}
fn a2(v: Vector2) -> [f32; 2] {
    [v.x, v.y]
}
fn v3(a: [f32; 3]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}
fn a3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}
fn v4(a: [f32; 4]) -> Vector4 {
    Vector4::new(a[0], a[1], a[2], a[3])
}
fn a4(v: Vector4) -> [f32; 4] {
    [v.x, v.y, v.z, v.w]
}

const V2_OPS: [&str; 19] = [
    "+", "-", "v*k", "k*v", "v/k", "+=", "-=", "*=", "/=", "neg", "Scale(x,y)", "Scale(v)",
    "Update_Min", "Update_Max", "Normalize", "Rotate(theta)", "Rotate(s,c)", "Lerp", "::Normalize",
];

fn rust_v2_op(op: usize, a: Vector2, b: Vector2, k: f32) -> Vector2 {
    let mut r = a;
    match op {
        0 => a + b,
        1 => a - b,
        2 => a * k,
        3 => k * a,
        4 => a / k,
        5 => {
            r += b;
            r
        }
        6 => {
            r -= b;
            r
        }
        7 => {
            r *= k;
            r
        }
        8 => {
            r /= k;
            r
        }
        9 => -a,
        10 => {
            r.scale(b.x, b.y);
            r
        }
        11 => {
            r.scale_by(&b);
            r
        }
        12 => {
            r.update_min(&b);
            r
        }
        13 => {
            r.update_max(&b);
            r
        }
        14 => {
            r.normalize();
            r
        }
        15 => {
            r.rotate(k);
            r
        }
        16 => {
            r.rotate_sc(b.x, b.y);
            r
        }
        17 => Vector2::lerp(&a, &b, k),
        18 => vector2::normalize(&a),
        _ => unreachable!(),
    }
}

#[test]
fn vector2_matches() {
    let mut g = Gen::new(70);
    for (op, name) in V2_OPS.iter().enumerate() {
        let mut c = Check::new(&format!("Vector2 {name}"));
        for i in 0..N {
            let (a, b, k) = (g.vec::<2>(Gen::geom), g.vec::<2>(Gen::geom), g.geom());
            let r = rust_v2_op(op, v2(a), v2(b), k);
            let mut o = [0.0f32; 2];
            unsafe { cpp::ref_v2_op(op as i32, a.as_ptr(), b.as_ptr(), k, o.as_mut_ptr()) };
            c.f32s(&a2(r), &o, || format!("case {i}: a={a:?} b={b:?} k={k:e}"));
        }
        c.finish();
    }
    let names = ["Dot_Product", "operator*", "Perp_Dot_Product", "Length", "Length2", "Distance", "Quick_Distance", "::Distance", "::Quick_Distance"];
    for (op, name) in names.iter().enumerate() {
        let mut c = Check::new(&format!("Vector2 {name}"));
        for i in 0..N {
            let (a, b) = (g.vec::<2>(Gen::geom), g.vec::<2>(Gen::geom));
            let (va, vb) = (v2(a), v2(b));
            let r = match op {
                0 => Vector2::dot_product(&va, &vb),
                1 => va * vb,
                2 => Vector2::perp_dot_product(&va, &vb),
                3 => va.length(),
                4 => va.length2(),
                5 => Vector2::distance(&va, &vb),
                6 => Vector2::quick_distance(&va, &vb),
                7 => vector2::distance_xy(a[0], a[1], b[0], b[1]),
                _ => vector2::quick_distance_xy(a[0], a[1], b[0], b[1]),
            };
            c.f32(r, unsafe { cpp::ref_v2_scalar(op as i32, a.as_ptr(), b.as_ptr()) }, || format!("case {i}: a={a:?} b={b:?}"));
        }
        c.finish();
    }
    let mut c = Check::new("Vector2 ==, !=, Equal_Within_Epsilon, Is_Valid");
    for i in 0..N {
        let a = g.vec::<2>(Gen::geom);
        let b = if g.below(3) == 0 { a } else { g.vec::<2>(Gen::geom) };
        let k = g.uniform(0.0, 50.0);
        let (va, vb) = (v2(a), v2(b));
        let r = [va == vb, va != vb, vector2::equal_within_epsilon(&va, &vb, k), va.is_valid()];
        let o: Vec<bool> = (0..4).map(|op| unsafe { cpp::ref_v2_pred(op, a.as_ptr(), b.as_ptr(), k) } != 0).collect();
        c.eq(r.to_vec(), o, || format!("case {i}: a={a:?} b={b:?} k={k:e}"));
    }
    c.finish();
    let mut c = Check::new("Vector2 Rotate_Towards_Vector (s,c) and (theta)");
    for i in 0..N {
        let (a, t) = (g.vec::<2>(Gen::geom), g.vec::<2>(Gen::geom));
        let theta = g.uniform(-4.0, 4.0);
        let (s, cc) = (libm::sinf(theta), libm::cosf(theta));
        let mut va = v2(a);
        let (r1, p1) = va.rotate_towards_vector_sc(&v2(t), s, cc);
        let mut o = a;
        let f = unsafe { cpp::ref_v2_rotate_towards(o.as_mut_ptr(), t.as_ptr(), s, cc) };
        c.f32s(&a2(va), &o, || format!("case {i} (s,c)"));
        c.eq((r1, p1), (f & 1 != 0, f & 2 != 0), || format!("case {i} flags"));
        let mut va = v2(a);
        let (r2, p2) = va.rotate_towards_vector(&v2(t), theta);
        let mut o = a;
        let f = unsafe { cpp::ref_v2_rotate_towards_theta(o.as_mut_ptr(), t.as_ptr(), theta) };
        c.f32s(&a2(va), &o, || format!("case {i} theta"));
        c.eq((r2, p2), (f & 1 != 0, f & 2 != 0), || format!("case {i} theta flags"));
    }
    c.finish();
}

const V3_OPS: [&str; 26] = [
    "+", "-", "v*k", "k*v", "v/k", "+=", "-=", "*=", "/=", "neg", "Normalize", "Scale",
    "Rotate_X(angle)", "Rotate_Y(angle)", "Rotate_Z(angle)", "Rotate_X(s,c)", "Rotate_Y(s,c)",
    "Rotate_Z(s,c)", "Cross_Product", "Normalized_Cross_Product", "Add", "Subtract",
    "Update_Min", "Update_Max", "Cap_Absolute_To", "Lerp",
];

fn rust_v3_op(op: usize, a: Vector3, b: Vector3, k: f32) -> Vector3 {
    let mut r = a;
    match op {
        0 => a + b,
        1 => a - b,
        2 => a * k,
        3 => k * a,
        4 => a / k,
        5 => {
            r += b;
            r
        }
        6 => {
            r -= b;
            r
        }
        7 => {
            r *= k;
            r
        }
        8 => {
            r /= k;
            r
        }
        9 => -a,
        10 => {
            r.normalize();
            r
        }
        11 => {
            r.scale(&b);
            r
        }
        12 => {
            r.rotate_x(k);
            r
        }
        13 => {
            r.rotate_y(k);
            r
        }
        14 => {
            r.rotate_z(k);
            r
        }
        15 => {
            r.rotate_x_sc(b.x, b.y);
            r
        }
        16 => {
            r.rotate_y_sc(b.x, b.y);
            r
        }
        17 => {
            r.rotate_z_sc(b.x, b.y);
            r
        }
        18 => Vector3::cross_product(&a, &b),
        19 => Vector3::normalized_cross_product(&a, &b),
        20 => Vector3::add_vectors(&a, &b),
        21 => Vector3::subtract_vectors(&a, &b),
        22 => {
            r.update_min(&b);
            r
        }
        23 => {
            r.update_max(&b);
            r
        }
        24 => {
            r.cap_absolute_to(&b);
            r
        }
        25 => Vector3::lerp(&a, &b, k),
        _ => unreachable!(),
    }
}

#[test]
fn vector3_matches() {
    let mut g = Gen::new(71);
    for (op, name) in V3_OPS.iter().enumerate() {
        let mut c = Check::new(&format!("Vector3 {name}"));
        for i in 0..N {
            let (a, b, k) = (g.vec::<3>(Gen::geom), g.vec::<3>(Gen::geom), g.geom());
            let r = rust_v3_op(op, v3(a), v3(b), k);
            let mut o = [0.0f32; 3];
            unsafe { cpp::ref_v3_op(op as i32, a.as_ptr(), b.as_ptr(), k, o.as_mut_ptr()) };
            c.f32s(&a3(r), &o, || format!("case {i}: a={a:?} b={b:?} k={k:e}"));
        }
        c.finish();
    }
    let names = ["Dot_Product", "Length", "Length2", "Quick_Length", "Distance", "Quick_Distance", "Cross_Product_X", "Cross_Product_Y", "Cross_Product_Z"];
    for (op, name) in names.iter().enumerate() {
        let mut c = Check::new(&format!("Vector3 {name}"));
        for i in 0..N {
            let (a, b) = (g.vec::<3>(Gen::geom), g.vec::<3>(Gen::geom));
            let (va, vb) = (v3(a), v3(b));
            let r = match op {
                0 => Vector3::dot_product(&va, &vb),
                1 => va.length(),
                2 => va.length2(),
                3 => va.quick_length(),
                4 => Vector3::distance(&va, &vb),
                5 => Vector3::quick_distance(&va, &vb),
                6 => Vector3::cross_product_x(&va, &vb),
                7 => Vector3::cross_product_y(&va, &vb),
                _ => Vector3::cross_product_z(&va, &vb),
            };
            c.f32(r, unsafe { cpp::ref_v3_scalar(op as i32, a.as_ptr(), b.as_ptr()) }, || format!("case {i}: a={a:?} b={b:?}"));
        }
        c.finish();
    }
    let names = ["Find_X_At_Y", "Find_X_At_Z", "Find_Y_At_X", "Find_Y_At_Z", "Find_Z_At_X", "Find_Z_At_Y"];
    for (op, name) in names.iter().enumerate() {
        let mut c = Check::new(&format!("Vector3 {name}"));
        for i in 0..N {
            let (p1, p2, v) = (g.vec::<3>(Gen::geom), g.vec::<3>(Gen::geom), g.geom());
            let (a, b) = (v3(p1), v3(p2));
            let r = match op {
                0 => Vector3::find_x_at_y(v, &a, &b),
                1 => Vector3::find_x_at_z(v, &a, &b),
                2 => Vector3::find_y_at_x(v, &a, &b),
                3 => Vector3::find_y_at_z(v, &a, &b),
                4 => Vector3::find_z_at_x(v, &a, &b),
                _ => Vector3::find_z_at_y(v, &a, &b),
            };
            c.f32(r, unsafe { cpp::ref_v3_find(op as i32, v, p1.as_ptr(), p2.as_ptr()) }, || format!("case {i}"));
        }
        c.finish();
    }
    let mut c = Check::new("Vector3 ==, !=, Equal_Within_Epsilon, Is_Valid");
    for i in 0..N {
        let a = g.vec::<3>(Gen::geom);
        let b = if g.below(3) == 0 { a } else { g.vec::<3>(Gen::geom) };
        let k = g.uniform(0.0, 50.0);
        let (va, vb) = (v3(a), v3(b));
        let r = [va == vb, va != vb, vector3::equal_within_epsilon(&va, &vb, k), va.is_valid()];
        let o: Vec<bool> = (0..4).map(|op| unsafe { cpp::ref_v3_pred(op, a.as_ptr(), b.as_ptr(), k) } != 0).collect();
        c.eq(r.to_vec(), o, || format!("case {i}: a={a:?} b={b:?}"));
    }
    c.finish();
}

#[test]
fn vector4_matches() {
    let mut g = Gen::new(72);
    let names = ["+", "-", "v*k", "k*v", "v/k", "+=", "-= (upstream W += v.W)", "*=", "/=", "neg"];
    for (op, name) in names.iter().enumerate() {
        let mut c = Check::new(&format!("Vector4 {name}"));
        for i in 0..N {
            let (a, b, k) = (g.vec::<4>(Gen::geom), g.vec::<4>(Gen::geom), g.geom());
            let (va, vb) = (v4(a), v4(b));
            let mut r = va;
            let r = match op {
                0 => va + vb,
                1 => va - vb,
                2 => va * k,
                3 => k * va,
                4 => va / k,
                5 => {
                    r += vb;
                    r
                }
                6 => {
                    r -= vb;
                    r
                }
                7 => {
                    r *= k;
                    r
                }
                8 => {
                    r /= k;
                    r
                }
                _ => -va,
            };
            let mut o = [0.0f32; 4];
            unsafe { cpp::ref_v4_op(op as i32, a.as_ptr(), b.as_ptr(), k, o.as_mut_ptr()) };
            c.f32s(&a4(r), &o, || format!("case {i}: a={a:?} b={b:?} k={k:e}"));
        }
        c.finish();
    }
    let mut c = Check::new("Vector4 dot, ==, !=");
    for i in 0..N {
        let a = g.vec::<4>(Gen::geom);
        let b = if g.below(3) == 0 { a } else { g.vec::<4>(Gen::geom) };
        let (va, vb) = (v4(a), v4(b));
        c.f32(va * vb, unsafe { cpp::ref_v4_dot(a.as_ptr(), b.as_ptr()) }, || format!("case {i}"));
        let e = unsafe { cpp::ref_v4_eq(a.as_ptr(), b.as_ptr()) };
        c.eq((va == vb, va != vb), (e & 1 != 0, e & 2 != 0), || format!("case {i}: a={a:?} b={b:?}"));
    }
    c.finish();
}
