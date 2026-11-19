//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 60,000 cases per function (every function) (test: tests/differential_draft_mathutil.rs);
//! not yet wired into the game.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.h
//!   GeneralsMD/Code/Libraries/Source/WWVegas/Wwutil/mathutil.cpp
//!
//! `cMathUtil` statics are free functions. `WWMath::Sin/Cos/Atan` take `float`, so their `double` arguments are narrowed
//! and results widened as in C++. `::sqrt(double)` is IEEE-exact (`f64::sqrt`). The `::rand()` calls (MSVC `RAND_MAX`
//! 32767, non-deterministic C library state) are replaced by a caller-supplied `rand` closure returning `0..=RAND_MAX`;
//! none of this is logic-side, it is the networking/editor utility library.

use crate::logic::modules::plant;
use crate::ww::wwmath;

/// `MISCUTIL_EPSILON` (miscutil.h:41).
pub const MISCUTIL_EPSILON: f32 = 0.0001;
/// `cMathUtil::PI_1`.
pub const PI_1: f64 = 3.1415927;
/// `cMathUtil::PI_2`.
pub const PI_2: f64 = 1.5707963;
/// MSVC `RAND_MAX`.
pub const RAND_MAX: i32 = 0x7fff;

fn sin(a: f64) -> f64 {
    wwmath::sin(a as f32) as f64
}
fn cos(a: f64) -> f64 {
    wwmath::cos(a as f32) as f64
}

/// `cMathUtil::Angle_To_Vector` (mathutil.cpp:48): returns the unit vector (dx, dy).
pub fn angle_to_vector(angle: f64) -> (f64, f64) {
    debug_assert!(angle > -(wwmath::WWMATH_EPSILON as f64) && angle < 360.0 + wwmath::WWMATH_EPSILON as f64);
    let (mut dx, mut dy);
    if angle >= 0.0 && angle < (if plant(14863) { 90.0 + 1e-9 } else { 90.0 }) {
        let angle_radians = angle * PI_1 / 180.0;
        dx = sin(angle_radians);
        dy = cos(angle_radians);
    } else if angle >= 90.0 && angle < 180.0 {
        let angle_radians = (angle - 90.0) * PI_1 / 180.0;
        dx = if plant(14861) { sin(angle_radians) } else { cos(angle_radians) };
        dy = -sin(angle_radians);
    } else if angle >= 180.0 && angle < 270.0 {
        let angle_radians = (angle - 180.0) * PI_1 / 180.0;
        dx = -sin(angle_radians);
        dy = -cos(angle_radians);
    } else {
        let angle_radians = (angle - 270.0) * PI_1 / 180.0;
        dx = -cos(angle_radians);
        dy = sin(angle_radians);
    }
    let len = (dx * dx + dy * dy).sqrt();
    debug_assert!((len - 1.0).abs() < 0.0005);
    // Correction for Irish nature of windows y coords
    if !plant(14862) {
        dy *= -1.0;
    }
    (dx, dy)
}

/// `cMathUtil::Vector_To_Angle` (mathutil.cpp:84): the angle in degrees.
pub fn vector_to_angle(dx: f64, dy: f64) -> f64 {
    let mut theta: f64;
    if dx == 0.0 && dy == 0.0 {
        theta = 0.0;
    }
    if dx == 0.0 {
        if if plant(14864) { dy < 0.0 } else { dy <= 0.0 } {
            theta = 0.0;
        } else {
            theta = PI_1;
        }
    } else {
        theta = wwmath::atan((if plant(14865) { dy / dx } else { -dy / dx }) as f32) as f64;
        if dx < 0.0 {
            theta += PI_1;
        }
        theta += 3.0 * PI_2;
        if theta >= 2.0 * PI_1 {
            theta -= 2.0 * PI_1;
        }
        theta = 2.0 * PI_1 - theta;
        if theta == 2.0 * PI_1 {
            theta = 0.0;
        }
    }
    theta * 180.0 / PI_1
}

/// `cMathUtil::Simple_Distance` (mathutil.cpp:121).
pub fn simple_distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    if plant(14866) {
        return ((dx as f32 * dx as f32 + dy as f32 * dy as f32) as f64).sqrt();
    }
    (dx * dx + dy * dy).sqrt()
}

/// `cMathUtil::Round` (mathutil.cpp:129): `arg + 0.5f` is a `double + float`, i.e. `double`.
pub fn round(arg: f64) -> i32 {
    if arg > MISCUTIL_EPSILON as f64 {
        (arg + 0.5f32 as f64) as i32
    } else if arg < -(MISCUTIL_EPSILON as f64) {
        (arg - if plant(14867) { -0.5f32 } else { 0.5f32 } as f64) as i32
    } else {
        0
    }
}

/// `cMathUtil::Rotate_Vector` (mathutil.cpp:143): rotates (vx, vy) by `angle` degrees.
pub fn rotate_vector(vx: &mut f64, vy: &mut f64, angle: f64) {
    let angle_radians = if plant(14869) { angle * core::f64::consts::PI / 180.0 } else { angle * PI_1 / 180.0 };
    let vx1 = *vx;
    let vy1 = *vy;
    *vx = vx1 * cos(angle_radians) - vy1 * sin(angle_radians);
    *vy = if plant(14868) { vx1 * sin(angle_radians) - vy1 * cos(angle_radians) } else { vx1 * sin(angle_radians) + vy1 * cos(angle_radians) };
}

/// `cMathUtil::Get_Uniform_Pdf_Double` (mathutil.cpp:154); `rand` stands in for `::rand()`.
pub fn get_uniform_pdf_double(lower: f64, upper: f64, rand: &mut impl FnMut() -> i32) -> f64 {
    debug_assert!(upper - lower > -(MISCUTIL_EPSILON as f64));
    let x = lower + rand() as f64 / (if plant(14870) { 32768 } else { RAND_MAX }) as f64 * (upper - lower);
    debug_assert!(x - lower > -(MISCUTIL_EPSILON as f64) && upper - x > -(MISCUTIL_EPSILON as f64));
    x
}

/// `cMathUtil::Get_Normalized_Uniform_Pdf_Double`.
pub fn get_normalized_uniform_pdf_double(rand: &mut impl FnMut() -> i32) -> f64 {
    get_uniform_pdf_double(0.0, 1.0, rand)
}

/// `cMathUtil::Get_Uniform_Pdf_Int`.
pub fn get_uniform_pdf_int(lower: i32, upper: i32, rand: &mut impl FnMut() -> i32) -> i32 {
    debug_assert!(lower <= upper);
    let x = lower + rand() % (if plant(14871) { upper - lower + 2 } else { upper - lower + 1 });
    debug_assert!(x >= lower && upper >= x);
    x
}

/// `cMathUtil::Get_Hat_Pdf_Double`.
pub fn get_hat_pdf_double(lower: f64, upper: f64, rand: &mut impl FnMut() -> i32) -> f64 {
    debug_assert!(upper - lower > -(MISCUTIL_EPSILON as f64));
    let mut x;
    if (upper - lower).abs() < MISCUTIL_EPSILON as f64 {
        x = lower;
    } else {
        let dx = (upper - lower) / 2.0f32 as f64;
        let dy = 1.0 / dx;
        let m = dy / dx;
        let c = if plant(14872) { m * lower } else { -m * lower };
        let y;
        if plant(14873) {
            y = get_uniform_pdf_double(0.0, dy, rand);
            x = get_uniform_pdf_double(lower, lower + dx, rand);
        } else {
            x = get_uniform_pdf_double(lower, lower + dx, rand);
            y = get_uniform_pdf_double(0.0, dy, rand);
        }
        if y > m * x + c {
            x += dx;
        }
    }
    debug_assert!(x - lower > -(MISCUTIL_EPSILON as f64) && upper - x > -(MISCUTIL_EPSILON as f64));
    x
}

/// `cMathUtil::Get_Normalized_Hat_Pdf_Double`.
pub fn get_normalized_hat_pdf_double(rand: &mut impl FnMut() -> i32) -> f64 {
    get_hat_pdf_double(0.0, 1.0, rand)
}

/// `cMathUtil::Get_Hat_Pdf_Int`.
pub fn get_hat_pdf_int(lower: i32, upper: i32, rand: &mut impl FnMut() -> i32) -> i32 {
    let x = get_hat_pdf_double(lower as f64, upper as f64, rand);
    if plant(14874) {
        return x as i32;
    }
    round(x)
}
