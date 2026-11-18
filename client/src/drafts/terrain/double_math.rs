//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! The C `double` maths the terrain, shadow and view drafts call (`cos`, `sin`, `tan`, `asin`, `sqrt` on a `Real`, the
//! result stored back in a `Real`). The client crate has no `petir` dependency of its own and `client/Cargo.toml` is outside
//! the compile-fix brief, so these go through the verified logic crate's functions:
//!
//! - `asin` and `sqrt` are the verified double routes (`ww::wwmath::asin`, `common::base_type::sqrt_via_double`): bit-exact
//!   with `(Real)asin(double)` and `(Real)sqrt(double)`.
//! - `cos`, `sin`, `tan` have NO verified double route public (`common::trig` is the `f32` `cosf`/`sinf`/`tanf`;
//!   `geometry::c_cos` is private). They are the `f32` route here, so a result can differ from `(Real)cos(double)` by an ulp.
//!   GAP: add `petir = { workspace-version }` to `client/Cargo.toml` and call `petir::Real::cos(f64)` here (one file).

use rts_lockstep_engine::common::base_type::sqrt_via_double;
use rts_lockstep_engine::common::trig;
use rts_lockstep_engine::ww::wwmath;

/// `(Real)cos(x)`.
pub fn c_cos(x: f32) -> f32 {
    trig::cos(x)
}

/// `(Real)sin(x)`.
pub fn c_sin(x: f32) -> f32 {
    trig::sin(x)
}

/// `(Real)tan(x)`.
pub fn c_tan(x: f32) -> f32 {
    trig::tan(x)
}

/// `(Real)asin(x)`.
pub fn c_asin(x: f32) -> f32 {
    wwmath::asin(x)
}

/// `(Real)sqrt(x)`.
pub fn c_sqrt(x: f32) -> f32 {
    sqrt_via_double(x)
}
