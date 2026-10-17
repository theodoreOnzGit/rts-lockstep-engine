// SPDX-License-Identifier: GPL-3.0-only
//
// rts-lockstep-engine: a modified Rust translation of the engine source code
// that Electronic Arts released under the GPL-3.0 (with additional terms, see
// NOTICE.md) at https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590. Not affiliated with or
// endorsed by Electronic Arts. Translated to Rust and modified; not the
// original program.

//! A pure-Rust, wasm-compatible translation of a released real-time-strategy
//! engine, aimed at deterministic lockstep simulation. See `SCOPE.md` for the
//! plan; this is the first batch of self-contained foundations.
//!
//! | module | upstream |
//! |---|---|
//! | [`common::random_value`] | `GameEngine/Source/Common/RandomValue.cpp` |
//! | [`common::crc`] | `GameEngine/Source/Common/crc.cpp`, `Include/Common/crc.h` |
//! | [`common::base_type`] | `Libraries/Include/Lib/BaseType.h` (fast float helpers) |
//! | [`common::trig`] | `GameEngine/Source/Common/System/Trig.cpp`, `Libraries/Include/Lib/trig.h` |
//! | [`common::quick_trig`] | `GameEngine/Include/Common/QuickTrig.h`, `Source/Common/System/QuickTrig.cpp` |
//! | [`ww::wwmath`] | `Libraries/Source/WWVegas/WWMath/wwmath.h`, `wwmath.cpp` |
//! | [`ww::vector2`], [`ww::vector3`], [`ww::vector4`] | `WWMath/vector2.h`, `vector3.h`, `vector4.h` |
//! | [`ww::matrix3d`] | `WWMath/matrix3d.h`, `matrix3d.cpp` |
//!
//! Floating point: all `f32` arithmetic is plain IEEE-754 binary32, one
//! rounding per operation, no fused multiply-add, and every elementary
//! function comes from the owner's `petir` crate (`petir::Real`, pure Rust,
//! forwarding to the `libm` crate), never `std`, in upstream's precision (a C
//! double function is evaluated in `f64`, an explicit `sinf` in `f32`), so
//! native and wasm builds agree bit for bit. The `differential` feature checks the translation against the
//! original C++ compiled for SSE; it does **not** establish agreement with the
//! original x87 build (see `SCOPE.md` section 3).

pub mod common;
pub mod ww;

#[cfg(feature = "differential")]
#[doc(hidden)]
pub mod differential;
