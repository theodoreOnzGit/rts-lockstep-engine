// SPDX-License-Identifier: GPL-3.0-only
//! Shared helpers for the differential tests: a seeded input generator and a
//! bit-for-bit comparison recorder.
//!
//! # What counts as a match
//!
//! Every comparison is on raw bits (`f32::to_bits`); nothing is compared with
//! a tolerance. A result pair is a **mismatch** when the bits differ and at
//! least one side is not NaN. When **both** sides are NaN but with different
//! sign or payload bits, the pair is counted separately (`NaN-payload-only`)
//! and reported, but does not fail the test: Rust's float semantics leave the
//! sign and payload of a NaN result unspecified (any input NaN, quieted, or
//! the preferred NaN), and so do C++ and IEEE 754 when an operation has two
//! NaN operands, so equality there would test LLVM's and GCC's choice of
//! operand order rather than the translation. (This rule was adopted after
//! the first run showed `Lerp(NaN, NaN, x)` propagating `a`'s payload in Rust
//! and `b`'s in C++.) Engine logic must therefore never depend on NaN bits.
//!
//! # Elementary functions
//!
//! The C++ reference would call glibc's elementary functions, while the
//! translation calls the owner's `petir` crate (`petir::Real`, which forwards
//! to the pure-Rust `libm` crate), and the two libraries round differently in
//! the last bit for some inputs. So that the gated tests verify the
//! *translation*, each test binary defines those symbols itself (below),
//! forwarding to the same `petir::Real` functions the translation calls; the
//! linker binds the upstream objects' calls to them in preference to the
//! shared glibc. Two families:
//!
//! - `float` functions upstream calls explicitly (`sinf`, `cosf`, `sincosf`,
//!   `acosf`, `asinf`, `atanf`, `atan2f`, `tanf`, and `sqrtf`, which g++
//!   substitutes for `(float)sqrt((double)f)`): petir's `f32` `Real`;
//! - C `double` functions (`sin`, `cos`, `sincos`, `tan`, `acos`, `asin`,
//!   `atan`, `atan2`, `sqrt`), which upstream calls on `float` arguments and
//!   casts back (`(float)acos(val)`): petir's `f64` `Real`. The differential
//!   build's shim `math.h` keeps g++ from resolving those calls to `float`
//!   overloads instead (see `differential/shim/math.h`).
//!
//! `floor`/`ceil` are exact and are not interposed (LLVM may lower the
//! `libm` crate's own `floorf` to a call to the `floorf` symbol, which an
//! interposer would turn into infinite recursion). The petir/`libm`-vs-glibc
//! difference is MEASURED separately, by looking the real glibc functions up
//! with `dlsym` (`tests/differential_libm_vs_glibc.rs`), and reported, not
//! hidden.
#![allow(dead_code)]

use petir::Real;

/// Interposed for the C++ reference: see the module docs.
#[no_mangle]
pub extern "C" fn sinf(x: f32) -> f32 {
    Real::sin(x)
}
#[no_mangle]
pub extern "C" fn cosf(x: f32) -> f32 {
    Real::cos(x)
}
/// g++ fuses `sinf(x)` and `cosf(x)` of one argument into `sincosf`.
/// # Safety
/// `s` and `c` must be valid for writes (they are, from the C++ callers).
#[no_mangle]
pub unsafe extern "C" fn sincosf(x: f32, s: *mut f32, c: *mut f32) {
    *s = Real::sin(x);
    *c = Real::cos(x);
}
#[no_mangle]
pub extern "C" fn tanf(x: f32) -> f32 {
    Real::tan(x)
}
#[no_mangle]
pub extern "C" fn acosf(x: f32) -> f32 {
    Real::acos(x)
}
#[no_mangle]
pub extern "C" fn asinf(x: f32) -> f32 {
    Real::asin(x)
}
#[no_mangle]
pub extern "C" fn atanf(x: f32) -> f32 {
    Real::atan(x)
}
#[no_mangle]
pub extern "C" fn atan2f(y: f32, x: f32) -> f32 {
    Real::atan2(y, x)
}
#[no_mangle]
pub extern "C" fn sin(x: f64) -> f64 {
    Real::sin(x)
}
#[no_mangle]
pub extern "C" fn cos(x: f64) -> f64 {
    Real::cos(x)
}
/// g++ fuses `sin(x)` and `cos(x)` of one argument into `sincos`.
/// # Safety
/// `s` and `c` must be valid for writes (they are, from the C++ callers).
#[no_mangle]
pub unsafe extern "C" fn sincos(x: f64, s: *mut f64, c: *mut f64) {
    *s = Real::sin(x);
    *c = Real::cos(x);
}
#[no_mangle]
pub extern "C" fn tan(x: f64) -> f64 {
    Real::tan(x)
}
#[no_mangle]
pub extern "C" fn acos(x: f64) -> f64 {
    Real::acos(x)
}
#[no_mangle]
pub extern "C" fn asin(x: f64) -> f64 {
    Real::asin(x)
}
#[no_mangle]
pub extern "C" fn atan(x: f64) -> f64 {
    Real::atan(x)
}
#[no_mangle]
pub extern "C" fn atan2(y: f64, x: f64) -> f64 {
    Real::atan2(y, x)
}
#[no_mangle]
pub extern "C" fn sqrt(x: f64) -> f64 {
    Real::sqrt(x)
}
/// g++ folds `(float)sqrt((double)f)` into `sqrtf(f)` (the same value).
#[no_mangle]
pub extern "C" fn sqrtf(x: f32) -> f32 {
    Real::sqrt(x)
}

/// SplitMix64: a small, seedable, platform-independent generator for test
/// inputs (not the engine RNG under test).
pub struct Gen(u64);

impl Gen {
    pub fn new(seed: u64) -> Self {
        Gen(seed)
    }
    pub fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn u32(&mut self) -> u32 {
        (self.u64() >> 32) as u32
    }
    pub fn i32(&mut self) -> i32 {
        self.u32() as i32
    }
    pub fn below(&mut self, n: u32) -> u32 {
        (self.u64() % u64::from(n)) as u32
    }
    /// Uniform in [lo, hi).
    pub fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        let t = (self.u64() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * t
    }
    pub fn special(&mut self) -> f32 {
        SPECIALS[self.below(SPECIALS.len() as u32) as usize]
    }
    /// A mixture: ordinary magnitudes, unit range, small integers and
    /// half-integers (rounding boundaries), log-uniform magnitudes, raw bit
    /// patterns (NaN, inf, subnormals included) and the special list.
    pub fn mixed(&mut self) -> f32 {
        match self.below(100) {
            0..=29 => self.uniform(-1000.0, 1000.0),
            30..=49 => self.uniform(-1.0, 1.0),
            50..=59 => (self.i32() % 2000) as f32 * 0.5,
            60..=74 => {
                let e = self.uniform(-40.0, 40.0);
                let m = Real::exp2(e);
                if self.below(2) == 0 { m } else { -m }
            }
            75..=89 => f32::from_bits(self.u32()),
            _ => self.special(),
        }
    }
    /// Like `mixed` but never NaN or infinite.
    pub fn finite(&mut self) -> f32 {
        loop {
            let v = self.mixed();
            if v.is_finite() {
                return v;
            }
        }
    }
    /// Mostly well-scaled values for geometry (|v| <= 100), with a share of
    /// `mixed` to reach the edges.
    pub fn geom(&mut self) -> f32 {
        if self.below(10) < 8 { self.uniform(-100.0, 100.0) } else { self.mixed() }
    }
    pub fn vec<const N: usize>(&mut self, f: fn(&mut Gen) -> f32) -> [f32; N] {
        let mut v = [0.0f32; N];
        for x in v.iter_mut() {
            *x = f(self);
        }
        v
    }
}

// Upstream literals (WWMATH_PI and friends) are deliberately among the inputs.
#[allow(clippy::approx_constant, clippy::excessive_precision)]
pub const SPECIALS: &[f32] = &[
    0.0, -0.0, 1.0, -1.0, 0.5, -0.5, 1.5, -1.5, 2.5, -2.5, 3.5, -3.5,
    0.975, -0.975, 0.97500002, 0.99999994, -0.99999994, 1.0000001, -1.0000001,
    f32::MIN_POSITIVE, -f32::MIN_POSITIVE, 1.0e-45, -1.0e-45,
    f32::MAX, f32::MIN, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -f32::NAN,
    f32::EPSILON, 255.0, 255.5, 256.0, -1.0e-7,
    8388608.0, 8388607.5, 16777216.0, 16777215.0, -16777216.0,
    2147483520.0, 2147483648.0, -2147483648.0, -2147483904.0, 4294967296.0, 4294967040.0,
    3.141592654, -3.141592654, 6.283185307, 1.5707964, 100.0, -100.0, 1.0e10, -1.0e10, 1.0e30,
];

/// Map f32 bits to a monotonic integer line for ULP distances.
fn ordered(bits: u32) -> i64 {
    if bits & 0x8000_0000 != 0 { -i64::from(bits & 0x7fff_ffff) } else { i64::from(bits) }
}

pub fn ulp_distance(a: f32, b: f32) -> u64 {
    (ordered(a.to_bits()) - ordered(b.to_bits())).unsigned_abs()
}

/// Records comparisons for one function.
pub struct Check {
    pub name: String,
    pub compared: u64,
    pub mismatches: u64,
    pub nan_only: u64,
    pub max_ulp: u64,
    pub examples: Vec<String>,
}

impl Check {
    pub fn new(name: &str) -> Self {
        Check { name: name.to_string(), compared: 0, mismatches: 0, nan_only: 0, max_ulp: 0, examples: Vec::new() }
    }

    fn miss(&mut self, what: String) {
        self.mismatches += 1;
        if self.examples.len() < 8 {
            self.examples.push(what);
        }
    }

    /// Compare two f32 results bit for bit. `input` is formatted only on a miss.
    pub fn f32(&mut self, rust: f32, cpp: f32, input: impl FnOnce() -> String) {
        self.compared += 1;
        if rust.to_bits() != cpp.to_bits() {
            if rust.is_nan() && cpp.is_nan() {
                self.nan_only += 1;
                return;
            }
            if !rust.is_nan() && !cpp.is_nan() {
                self.max_ulp = self.max_ulp.max(ulp_distance(rust, cpp));
            }
            self.miss(format!(
                "{}: rust {:e} ({:#010x}) != c++ {:e} ({:#010x})",
                input(), rust, rust.to_bits(), cpp, cpp.to_bits()
            ));
        }
    }

    pub fn f32s(&mut self, rust: &[f32], cpp: &[f32], input: impl Fn() -> String) {
        for (i, (&r, &c)) in rust.iter().zip(cpp.iter()).enumerate() {
            self.f32(r, c, || format!("{} [component {i}]", input()));
        }
    }

    pub fn f64(&mut self, rust: f64, cpp: f64, input: impl FnOnce() -> String) {
        self.compared += 1;
        if rust.to_bits() != cpp.to_bits() {
            if rust.is_nan() && cpp.is_nan() {
                self.nan_only += 1;
                return;
            }
            self.miss(format!("{}: rust {:e} != c++ {:e}", input(), rust, cpp));
        }
    }

    pub fn eq<T: PartialEq + std::fmt::Debug>(&mut self, rust: T, cpp: T, input: impl FnOnce() -> String) {
        self.compared += 1;
        if rust != cpp {
            self.miss(format!("{}: rust {:?} != c++ {:?}", input(), rust, cpp));
        }
    }

    /// Print the summary line and fail on any mismatch.
    pub fn finish(self) {
        println!(
            "DIFF {:<44} compared {:>9}  mismatches {:>6}  (NaN-payload-only {}, max ULP {})",
            self.name, self.compared, self.mismatches, self.nan_only, self.max_ulp
        );
        assert!(self.compared > 0, "{}: nothing compared", self.name);
        assert_eq!(
            self.mismatches, 0,
            "{}: {} of {} outputs differ from the C++ reference; first: {:#?}",
            self.name, self.mismatches, self.compared, self.examples
        );
    }

    /// Print the summary line without failing; returns the mismatch count.
    /// Used only for measurements that are reported, not gated.
    pub fn report(self) -> u64 {
        println!(
            "MEASURED {:<40} compared {:>9}  mismatches {:>6}  (NaN-payload-only {}, max ULP {})",
            self.name, self.compared, self.mismatches, self.nan_only, self.max_ulp
        );
        for e in &self.examples {
            println!("    e.g. {e}");
        }
        self.mismatches
    }
}

/// Number of random inputs per function.
pub const N: usize = 100_000;
