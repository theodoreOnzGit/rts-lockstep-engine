// SPDX-License-Identifier: GPL-3.0-only
//! Code-to-code verification of `ww::matrix3d` against upstream
//! `WWMath/matrix3d.h` + `matrix3d.cpp` (commit 0a05454d); `matrix3d.cpp` is
//! compiled unchanged as its own translation unit.
//!
//! Every operation gets 100k random matrices and parameter sets (entries
//! mostly in [-100, 100], 20% from the full mixed generator), plus
//! orthonormal matrices built by rotations for the operations that assume
//! one; all 12 output entries (or 3/6 vector components) are compared as bits.
//! Not covered because not translated: `Get_Inverse` (DirectX SDK),
//! `Set(Matrix3x3|Quaternion)`, `Set_Rotation`, `Lerp`.
#![cfg(feature = "differential")]

mod support;

use rts_lockstep_engine::differential as cpp;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;
use support::{Check, Gen, N};
use petir::Real;

fn mat(g: &mut Gen) -> [f32; 12] {
    if g.below(4) == 0 {
        // An orthonormal rotation with a translation, as most game matrices are.
        let mut m = Matrix3D::IDENTITY;
        m.rotate_z(g.uniform(-4.0, 4.0));
        m.rotate_y(g.uniform(-4.0, 4.0));
        m.rotate_x(g.uniform(-4.0, 4.0));
        m.set_translation(&Vector3::new(g.geom(), g.geom(), g.geom()));
        m.to_array()
    } else {
        g.vec::<12>(Gen::geom)
    }
}

fn v3(a: &[f32]) -> Vector3 {
    Vector3::new(a[0], a[1], a[2])
}
fn a3(v: Vector3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

const OPS: [&str; 42] = [
    "Make_Identity", "Translate(x,y,z)", "Translate(v)", "Translate_X", "Translate_Y", "Translate_Z",
    "Rotate_X(theta)", "Rotate_Y(theta)", "Rotate_Z(theta)", "Rotate_X(s,c)", "Rotate_Y(s,c)", "Rotate_Z(s,c)",
    "Scale(k)", "Scale(x,y,z)", "Scale(v)", "Pre_Rotate_X(theta)", "Pre_Rotate_Y(theta)", "Pre_Rotate_Z(theta)",
    "Pre_Rotate_X(s,c)", "Pre_Rotate_Y(s,c)", "Pre_Rotate_Z(s,c)", "In_Place_Pre_Rotate_X(theta)",
    "In_Place_Pre_Rotate_Y(theta)", "In_Place_Pre_Rotate_Z(theta)", "In_Place_Pre_Rotate_X(s,c)",
    "In_Place_Pre_Rotate_Y(s,c)", "In_Place_Pre_Rotate_Z(s,c)", "Re_Orthogonalize", "Set(axis,angle)",
    "Set(axis,s,c)", "Set(position)", "Set(x,y,z,pos)", "Look_At", "Obj_Look_At", "buildTransformMatrix",
    "Set_Translation", "Adjust_Translation", "Adjust_X/Y/Z_Translation", "Copy_3x3_Matrix",
    "Get_Orthogonal_Inverse", "postMul", "preMul",
];

fn rust_op(op: usize, m: &mut Matrix3D, p: &[f32; 12]) {
    let a = v3(&p[0..3]);
    let b = v3(&p[3..6]);
    match op {
        0 => m.make_identity(),
        1 => m.translate(p[0], p[1], p[2]),
        2 => m.translate_v(&a),
        3 => m.translate_x(p[0]),
        4 => m.translate_y(p[0]),
        5 => m.translate_z(p[0]),
        6 => m.rotate_x(p[0]),
        7 => m.rotate_y(p[0]),
        8 => m.rotate_z(p[0]),
        9 => m.rotate_x_sc(p[0], p[1]),
        10 => m.rotate_y_sc(p[0], p[1]),
        11 => m.rotate_z_sc(p[0], p[1]),
        12 => m.scale(p[0]),
        13 => m.scale_xyz(p[0], p[1], p[2]),
        14 => m.scale_v(&a),
        15 => m.pre_rotate_x(p[0]),
        16 => m.pre_rotate_y(p[0]),
        17 => m.pre_rotate_z(p[0]),
        18 => m.pre_rotate_x_sc(p[0], p[1]),
        19 => m.pre_rotate_y_sc(p[0], p[1]),
        20 => m.pre_rotate_z_sc(p[0], p[1]),
        21 => m.in_place_pre_rotate_x(p[0]),
        22 => m.in_place_pre_rotate_y(p[0]),
        23 => m.in_place_pre_rotate_z(p[0]),
        24 => m.in_place_pre_rotate_x_sc(p[0], p[1]),
        25 => m.in_place_pre_rotate_y_sc(p[0], p[1]),
        26 => m.in_place_pre_rotate_z_sc(p[0], p[1]),
        27 => m.re_orthogonalize(),
        28 => m.set_axis_angle(&a, p[3]),
        29 => m.set_axis_sin_cos(&a, p[3], p[4]),
        30 => m.set_position(&a),
        31 => m.set_axes(&a, &b, &v3(&p[6..9]), &v3(&p[9..12])),
        32 => m.look_at(&a, &b, p[6]),
        33 => m.obj_look_at(&a, &b, p[6]),
        34 => m.build_transform_matrix(&a, &b),
        35 => m.set_translation(&a),
        36 => m.adjust_translation(&a),
        37 => {
            m.adjust_x_translation(p[0]);
            m.adjust_y_translation(p[1]);
            m.adjust_z_translation(p[2]);
        }
        38 => {
            let t = [[p[0], p[1], p[2]], [p[3], p[4], p[5]], [p[6], p[7], p[8]]];
            m.copy_3x3_matrix(&t);
        }
        39 => *m = m.get_orthogonal_inverse(),
        40 => m.post_mul(&Matrix3D::from_array(*p)),
        41 => m.pre_mul(&Matrix3D::from_array(*p)),
        _ => unreachable!(),
    }
}

/// Parameters suited to each op: angles in [-4, 4] for theta ops, a unit
/// axis half of the time for Set(axis, ...), a unit direction half of the
/// time for buildTransformMatrix, a matrix for postMul/preMul.
fn params(op: usize, g: &mut Gen) -> [f32; 12] {
    let mut p = g.vec::<12>(Gen::geom);
    match op {
        6..=8 | 15..=17 | 21..=23 => p[0] = if g.below(5) == 0 { g.mixed() } else { g.uniform(-4.0, 4.0) },
        9..=11 | 18..=20 | 24..=26 => {
            if g.below(2) == 0 {
                let t = g.uniform(-4.0, 4.0);
                p[0] = Real::sin(t);
                p[1] = Real::cos(t);
            }
        }
        28 | 29 => {
            if g.below(2) == 0 {
                let mut v = Vector3::new(p[0], p[1], p[2]);
                v.normalize();
                p[0] = v.x;
                p[1] = v.y;
                p[2] = v.z;
            }
            p[3] = g.uniform(-4.0, 4.0);
            if op == 29 {
                let t = p[3];
                p[3] = Real::sin(t);
                p[4] = Real::cos(t);
            }
        }
        32 | 33 => p[6] = g.uniform(-4.0, 4.0),
        34 => {
            if g.below(2) == 0 {
                let mut v = Vector3::new(p[3], p[4], p[5]);
                v.normalize();
                p[3] = v.x;
                p[4] = v.y;
                p[5] = v.z;
            }
        }
        40 | 41 => p = mat(g),
        _ => {}
    }
    p
}

#[test]
fn matrix3d_in_place_ops_match() {
    let mut g = Gen::new(80);
    for (op, name) in OPS.iter().enumerate() {
        let mut c = Check::new(&format!("Matrix3D {name}"));
        for i in 0..N {
            let m0 = mat(&mut g);
            let p = params(op, &mut g);
            let mut r = Matrix3D::from_array(m0);
            rust_op(op, &mut r, &p);
            let mut o = m0;
            unsafe { cpp::ref_m3_op(op as i32, o.as_mut_ptr(), p.as_ptr()) };
            c.f32s(&r.to_array(), &o, || format!("case {i}: m={m0:?} p={p:?}"));
        }
        c.finish();
    }
}

#[test]
fn matrix3d_multiply_matches() {
    let mut g = Gen::new(81);
    let mut c = Check::new("Matrix3D::Multiply (incl. aliased A and B)");
    for i in 0..N {
        let (a, b) = (mat(&mut g), mat(&mut g));
        let r = Matrix3D::multiply(&Matrix3D::from_array(a), &Matrix3D::from_array(b));
        let alias = (i % 3) as i32;
        let mut o = [0.0f32; 12];
        unsafe { cpp::ref_m3_multiply(a.as_ptr(), b.as_ptr(), alias, o.as_mut_ptr()) };
        c.f32s(&r.to_array(), &o, || format!("case {i} alias {alias}"));
    }
    c.finish();
}

#[test]
fn matrix3d_vector_ops_match() {
    let mut g = Gen::new(82);
    let names = [
        "Transform_Vector", "static Rotate_Vector", "Inverse_Transform_Vector", "static Inverse_Rotate_Vector",
        "Rotate_Vector", "Inverse_Rotate_Vector", "mulVector3(in,out)", "mulVector3(inout)",
        "mulVector3Array(in,out)", "mulVector3Array(inout)", "Transform_Vector (aliased)",
        "Get_X_Vector", "Get_Y_Vector", "Get_Z_Vector", "Get_Translation",
    ];
    for (op, name) in names.iter().enumerate() {
        let mut c = Check::new(&format!("Matrix3D {name}"));
        for i in 0..N {
            let m = mat(&mut g);
            let v = g.vec::<3>(Gen::geom);
            let mm = Matrix3D::from_array(m);
            let vv = v3(&v);
            let r = match op {
                0 | 10 => Matrix3D::transform_vector(&mm, &vv),
                1 => Matrix3D::rotate_vector_by(&mm, &vv),
                2 => Matrix3D::inverse_transform_vector(&mm, &vv),
                3 => Matrix3D::inverse_rotate_vector_by(&mm, &vv),
                4 => mm.rotate_vector(&vv),
                5 => mm.inverse_rotate_vector(&vv),
                6 | 7 => mm.mul_vector3(&vv),
                8 => {
                    let mut out = [Vector3::default()];
                    mm.mul_vector3_array(&[vv], &mut out);
                    out[0]
                }
                9 => {
                    let mut io = [vv];
                    mm.mul_vector3_array_in_place(&mut io);
                    io[0]
                }
                11 => mm.get_x_vector(),
                12 => mm.get_y_vector(),
                13 => mm.get_z_vector(),
                _ => mm.get_translation(),
            };
            let mut o = [0.0f32; 3];
            unsafe { cpp::ref_m3_vec(op as i32, m.as_ptr(), v.as_ptr(), o.as_mut_ptr()) };
            c.f32s(&a3(r), &o, || format!("case {i}: m={m:?} v={v:?}"));
        }
        c.finish();
    }
}

#[test]
fn matrix3d_scalar_box_solve_and_statics_match() {
    let mut g = Gen::new(83);
    let mut cr = Check::new("Matrix3D Get_X/Y/Z_Rotation");
    let mut co = Check::new("Matrix3D Is_Orthogonal");
    let mut cb = Check::new("Matrix3D Transform_Min_Max_AABox");
    let mut ce = Check::new("Matrix3D Transform_Center_Extent_AABox");
    let mut cs = Check::new("Matrix3D::Solve_Linear_System");
    let mut cq = Check::new("Matrix3D ==, !=");
    for i in 0..N {
        let m = mat(&mut g);
        let mm = Matrix3D::from_array(m);
        unsafe {
            cr.f32(mm.get_x_rotation(), cpp::ref_m3_scalar(0, m.as_ptr()), || format!("case {i} X"));
            cr.f32(mm.get_y_rotation(), cpp::ref_m3_scalar(1, m.as_ptr()), || format!("case {i} Y"));
            cr.f32(mm.get_z_rotation(), cpp::ref_m3_scalar(2, m.as_ptr()), || format!("case {i} Z"));
            co.eq(mm.is_orthogonal(), cpp::ref_m3_scalar(3, m.as_ptr()) != 0.0, || format!("case {i}"));
            let (a, b) = (g.vec::<3>(Gen::geom), g.vec::<3>(Gen::geom));
            let (r0, r1) = mm.transform_min_max_aabox(&v3(&a), &v3(&b));
            let mut o = [0.0f32; 6];
            cpp::ref_m3_aabox(0, m.as_ptr(), a.as_ptr(), b.as_ptr(), o.as_mut_ptr());
            cb.f32s(&[a3(r0), a3(r1)].concat(), &o, || format!("case {i}"));
            let (r0, r1) = mm.transform_center_extent_aabox(&v3(&a), &v3(&b));
            cpp::ref_m3_aabox(1, m.as_ptr(), a.as_ptr(), b.as_ptr(), o.as_mut_ptr());
            ce.f32s(&[a3(r0), a3(r1)].concat(), &o, || format!("case {i}"));
            // Solve: sometimes plant a zero pivot.
            let mut sm = m;
            if g.below(10) == 0 {
                sm[[0, 5, 10][g.below(3) as usize]] = 0.0;
            }
            let mut rs = Matrix3D::from_array(sm);
            let ok = Matrix3D::solve_linear_system(&mut rs);
            let mut o12 = sm;
            let okc = cpp::ref_m3_solve(o12.as_mut_ptr()) != 0;
            cs.eq(ok, okc, || format!("case {i} return"));
            cs.f32s(&rs.to_array(), &o12, || format!("case {i}: m={sm:?}"));
            let m2 = if g.below(3) == 0 { m } else { mat(&mut g) };
            let e = cpp::ref_m3_eq(m.as_ptr(), m2.as_ptr());
            let m2m = Matrix3D::from_array(m2);
            cq.eq((mm == m2m, mm != m2m), (e & 1 != 0, e & 2 != 0), || format!("case {i}"));
        }
    }
    cr.finish();
    co.finish();
    cb.finish();
    ce.finish();
    cs.finish();
    cq.finish();

    let mut c = Check::new("Matrix3D static matrices (Identity, RotateX90 ...)");
    let rust = [
        Matrix3D::IDENTITY, Matrix3D::ROTATE_X90, Matrix3D::ROTATE_X180, Matrix3D::ROTATE_X270,
        Matrix3D::ROTATE_Y90, Matrix3D::ROTATE_Y180, Matrix3D::ROTATE_Y270, Matrix3D::ROTATE_Z90,
        Matrix3D::ROTATE_Z180, Matrix3D::ROTATE_Z270,
    ];
    for (i, m) in rust.iter().enumerate() {
        let mut o = [0.0f32; 12];
        unsafe { cpp::ref_m3_static(i as i32, o.as_mut_ptr()) };
        c.f32s(&m.to_array(), &o, || format!("static {i}"));
    }
    c.finish();
}
