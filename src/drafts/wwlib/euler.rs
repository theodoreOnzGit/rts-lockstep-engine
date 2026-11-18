//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/euler.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/euler.cpp
//!
//! Notes: the `extern int EulerOrder..` globals are `pub const`s (their values never change). `double` arithmetic is
//! `f64`; `sqrt(double)` is IEEE-exact so `f64::sqrt` is bit-identical to libm's. `WWMath::Atan2/Cos/Sin` take `float`,
//! so their double arguments are narrowed to `f32` as in C++. `EULER_AXIS_*` macros and `_euler_axis_*` helpers
//! (unused by upstream's functions except via `_euler_unpack_order`) are translated as functions.

use crate::ww::matrix3d::Matrix3D;
use crate::ww::wwmath;

const EULER_FRAME_STATIC: i32 = 0;
const EULER_FRAME_ROTATING: i32 = 1;
const EULER_REPEAT_YES: i32 = 1;
const EULER_PARITY_ODD: i32 = 1;

const FLT_EPSILON: f64 = f32::EPSILON as f64;

/// `EULER_FRAME(order)`.
pub fn euler_frame(order: i32) -> i32 {
    ((order as u32) & 1) as i32
}
/// `EULER_REPEAT(order)`.
pub fn euler_repeat(order: i32) -> i32 {
    (((order as u32) >> 1) & 1) as i32
}
/// `EULER_PARITY(order)`.
pub fn euler_parity(order: i32) -> i32 {
    (((order as u32) >> 2) & 1) as i32
}
/// `EULER_BUILD_ORDER(i,p,r,f)`.
pub const fn euler_build_order(i: i32, p: i32, r: i32, f: i32) -> i32 {
    (((((i << 1) + p) << 1) + r) << 1) + f
}

/// static axes
pub const EULER_ORDER_XYZS: i32 = euler_build_order(0, 0, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_XYXS: i32 = euler_build_order(0, 0, 1, EULER_FRAME_STATIC);
pub const EULER_ORDER_XZYS: i32 = euler_build_order(0, 1, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_XZXS: i32 = euler_build_order(0, 1, 1, EULER_FRAME_STATIC);
pub const EULER_ORDER_YZXS: i32 = euler_build_order(1, 0, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_YZYS: i32 = euler_build_order(1, 0, 1, EULER_FRAME_STATIC);
pub const EULER_ORDER_YXZS: i32 = euler_build_order(1, 1, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_YXYS: i32 = euler_build_order(1, 1, 1, EULER_FRAME_STATIC);
pub const EULER_ORDER_ZXYS: i32 = euler_build_order(2, 0, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_ZXZS: i32 = euler_build_order(2, 0, 1, EULER_FRAME_STATIC);
pub const EULER_ORDER_ZYXS: i32 = euler_build_order(2, 1, 0, EULER_FRAME_STATIC);
pub const EULER_ORDER_ZYZS: i32 = euler_build_order(2, 1, 1, EULER_FRAME_STATIC);
/// rotating axes (note upstream's table: the names are as upstream wrote them)
pub const EULER_ORDER_ZYXR: i32 = euler_build_order(0, 0, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_XYXR: i32 = euler_build_order(0, 0, 1, EULER_FRAME_ROTATING);
pub const EULER_ORDER_YZXR: i32 = euler_build_order(0, 1, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_XZXR: i32 = euler_build_order(0, 1, 1, EULER_FRAME_ROTATING);
pub const EULER_ORDER_XZYR: i32 = euler_build_order(1, 0, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_YZYR: i32 = euler_build_order(1, 0, 1, EULER_FRAME_ROTATING);
pub const EULER_ORDER_ZXYR: i32 = euler_build_order(1, 1, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_YXYR: i32 = euler_build_order(1, 1, 1, EULER_FRAME_ROTATING);
pub const EULER_ORDER_YXZR: i32 = euler_build_order(2, 0, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_ZXZR: i32 = euler_build_order(2, 0, 1, EULER_FRAME_ROTATING);
pub const EULER_ORDER_XYZR: i32 = euler_build_order(2, 1, 0, EULER_FRAME_ROTATING);
pub const EULER_ORDER_ZYZR: i32 = euler_build_order(2, 1, 1, EULER_FRAME_ROTATING);

const EULER_SAFE: [i32; 4] = [0, 1, 2, 0];
const EULER_NEXT: [i32; 4] = [1, 2, 0, 1];

/// `_euler_axis_i` (euler.cpp).
pub fn euler_axis_i(order: i32) -> i32 {
    EULER_SAFE[((order >> 3) & 3) as usize]
}

/// `_euler_axis_j`.
pub fn euler_axis_j(order: i32) -> i32 {
    let mut index = euler_axis_i(order);
    if euler_parity(order) == 1 {
        index += 1;
    }
    EULER_NEXT[index as usize]
}

/// `_euler_axis_k`.
pub fn euler_axis_k(order: i32) -> i32 {
    let mut index = euler_axis_i(order);
    if euler_parity(order) != 1 {
        index += 1;
    }
    EULER_NEXT[index as usize]
}

/// `_euler_axis_h`.
pub fn euler_axis_h(order: i32) -> i32 {
    if euler_repeat(order) == 1 {
        euler_axis_k(order)
    } else {
        euler_axis_i(order)
    }
}

/// `_euler_unpack_order` (euler.cpp): (i, j, k, h, n, s, f).
pub fn euler_unpack_order(order: i32) -> (usize, usize, usize, i32, i32, i32, i32) {
    let mut order = order;
    let f = order & 1;
    order >>= 1;
    let s = order & 1;
    order >>= 1;
    let n = order & 1;
    order >>= 1;
    let i = EULER_SAFE[(order & 3) as usize];
    let j = EULER_NEXT[(i + n) as usize];
    let k = EULER_NEXT[(i + 1 - n) as usize];
    let h = if s != 0 { k } else { i };
    (i as usize, j as usize, k as usize, h, n, s, f)
}

/// `class EulerAnglesClass`.
#[derive(Clone, Copy, Default)]
pub struct EulerAnglesClass {
    angle: [f64; 3],
    order: i32,
}

impl EulerAnglesClass {
    /// `EulerAnglesClass()`.
    pub fn new() -> Self {
        EulerAnglesClass { angle: [0.0; 3], order: 0 }
    }

    /// `EulerAnglesClass(const Matrix3D & M, int order)`.
    pub fn from_matrix_order(m: &Matrix3D, order: i32) -> Self {
        let mut e = Self::new();
        e.from_matrix(m, order);
        e
    }

    /// `EulerAnglesClass::Get_Angle`.
    pub fn get_angle(&self, i: usize) -> f64 {
        self.angle[i]
    }

    /// `EulerAnglesClass::From_Matrix` (euler.cpp:140).
    pub fn from_matrix(&mut self, m: &Matrix3D, order: i32) {
        self.order = order;
        let (i, j, k, _h, n, s, f) = euler_unpack_order(order);
        let a = |r: usize, c: usize| m[r][c] as f64;
        // WWMath::Atan2(float,float): the arguments are narrowed to float.
        let atan2 = |y: f64, x: f64| wwmath::atan2(y as f32, x as f32) as f64;

        if s == EULER_REPEAT_YES {
            let sy = (a(i, j) * a(i, j) + a(i, k) * a(i, k)).sqrt();
            if sy > 16.0 * FLT_EPSILON {
                self.angle[0] = atan2(a(i, j), a(i, k));
                self.angle[1] = atan2(sy, a(i, i));
                self.angle[2] = atan2(a(j, i), -a(k, i));
            } else {
                self.angle[0] = atan2(-a(j, k), a(j, j));
                self.angle[1] = atan2(sy, a(i, i));
                self.angle[2] = 0.0;
            }
        } else {
            let cy = (a(i, i) * a(i, i) + a(j, i) * a(j, i)).sqrt();
            if cy > 16.0 * FLT_EPSILON {
                self.angle[0] = atan2(a(k, j), a(k, k));
                self.angle[1] = atan2(-a(k, i), cy);
                self.angle[2] = atan2(a(j, i), a(i, i));
            } else {
                self.angle[0] = atan2(-a(j, k), a(j, j));
                self.angle[1] = atan2(-a(k, i), cy);
                self.angle[2] = 0.0;
            }
        }

        if n == EULER_PARITY_ODD {
            self.angle[0] = -self.angle[0];
            self.angle[1] = -self.angle[1];
            self.angle[2] = -self.angle[2];
        }
        if f == EULER_FRAME_ROTATING {
            let t = self.angle[0];
            self.angle[0] = self.angle[2];
            self.angle[2] = t;
        }

        // Trying to "clean" up the eulers, special cased for XYZr
        if order == EULER_ORDER_XYZR {
            let pi: f64 = 3.141592654;
            let mut x2 = pi + self.angle[0];
            let mut y2 = pi - self.angle[1];
            let mut z2 = pi + self.angle[2];
            if x2 > pi {
                x2 -= 2.0 * pi;
            }
            if y2 > pi {
                y2 -= 2.0 * pi;
            }
            if z2 > pi {
                z2 -= 2.0 * pi;
            }
            let mag0 = self.angle[0] * self.angle[0] + self.angle[1] * self.angle[1] + self.angle[2] * self.angle[2];
            let mag1 = x2 * x2 + y2 * y2 + z2 * z2;
            if mag1 < mag0 {
                self.angle[0] = x2;
                self.angle[1] = y2;
                self.angle[2] = z2;
            }
        }
    }

    /// `EulerAnglesClass::To_Matrix` (euler.cpp:215).
    pub fn to_matrix(&self, m: &mut Matrix3D) {
        m.make_identity();
        let mut a0 = self.angle[0];
        let mut a1 = self.angle[1];
        let mut a2 = self.angle[2];
        let (i, j, k, _h, n, s, f) = euler_unpack_order(self.order);
        if f == EULER_FRAME_ROTATING {
            let t = a0;
            a0 = a2;
            a2 = t;
        }
        if n == EULER_PARITY_ODD {
            a0 = -a0;
            a1 = -a1;
            a2 = -a2;
        }
        let (ti, tj, th) = (a0, a1, a2);
        let ci = wwmath::cos(ti as f32) as f64;
        let cj = wwmath::cos(tj as f32) as f64;
        let ch = wwmath::cos(th as f32) as f64;
        let si = wwmath::sin(ti as f32) as f64;
        let sj = wwmath::sin(tj as f32) as f64;
        let sh = wwmath::sin(th as f32) as f64;
        let cc = ci * ch;
        let cs = ci * sh;
        let sc = si * ch;
        let ss = si * sh;

        if s == EULER_REPEAT_YES {
            m[i][i] = cj as f32;
            m[i][j] = (sj * si) as f32;
            m[i][k] = (sj * ci) as f32;
            m[j][i] = (sj * sh) as f32;
            m[j][j] = (-cj * ss + cc) as f32;
            m[j][k] = (-cj * cs - sc) as f32;
            m[k][i] = (-sj * ch) as f32;
            m[k][j] = (cj * sc + cs) as f32;
            m[k][k] = (cj * cc - ss) as f32;
        } else {
            m[i][i] = (cj * ch) as f32;
            m[i][j] = (sj * sc - cs) as f32;
            m[i][k] = (sj * cc + ss) as f32;
            m[j][i] = (cj * sh) as f32;
            m[j][j] = (sj * ss + cc) as f32;
            m[j][k] = (sj * cs - sc) as f32;
            m[k][i] = (-sj) as f32;
            m[k][j] = (cj * si) as f32;
            m[k][k] = (cj * ci) as f32;
        }
    }
}
