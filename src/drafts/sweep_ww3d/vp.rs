//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vp.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WWMath/vp.h
//!
//! `VectorProcessorClass::CopyIndexed` (six overloads) and `::Prefetch`. The Intel-compiler (`__ICL`) SSE paths of the
//! neighbouring functions are x87/SSE assembly and are not part of this sweep; the plain C loops below are what every
//! other compiler (and the shipped game's non-ICL build) runs. The six overloads differ only in element type, so one
//! generic function covers them; each overload is named in its doc comment.

use crate::ww::vector2::Vector2;
use crate::ww::vector3::Vector3;
use crate::ww::vector4::Vector4;

/// `VectorProcessorClass::Prefetch(void* address)` (vp.cpp:66): the body is `#if defined (__ICL)` with its instructions
/// commented out, so for every other compiler it does nothing.
pub fn prefetch() {
    // upstream: prefetch instruction — device-only, not translated
}

/// `if (count<=0) return; for (i=0; i<count; i++) dst[i]=src[index[i]];` shared by all overloads. `count` is the C
/// `int`; a non-positive count copies nothing.
pub fn copy_indexed<T: Copy>(dst: &mut [T], src: &[T], index: &[u32], count: i32) {
    if count <= 0 {
        return;
    }
    for i in 0..count as usize {
        dst[i] = src[index[i] as usize];
    }
}

/// `VectorProcessorClass::CopyIndexed(unsigned *dst, const unsigned *src, const unsigned int *index, int count)`
/// (vp.cpp:393).
pub fn copy_indexed_u32(dst: &mut [u32], src: &[u32], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}

/// `VectorProcessorClass::CopyIndexed(Vector2 *dst, ...)` (vp.cpp:404).
pub fn copy_indexed_vector2(dst: &mut [Vector2], src: &[Vector2], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}

/// `VectorProcessorClass::CopyIndexed(Vector3 *dst, ...)` (vp.cpp:415).
pub fn copy_indexed_vector3(dst: &mut [Vector3], src: &[Vector3], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}

/// `VectorProcessorClass::CopyIndexed(Vector4 *dst, ...)` (vp.cpp:426).
pub fn copy_indexed_vector4(dst: &mut [Vector4], src: &[Vector4], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}

/// `VectorProcessorClass::CopyIndexed(unsigned char* dst, ...)` (vp.cpp:437).
pub fn copy_indexed_u8(dst: &mut [u8], src: &[u8], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}

/// `VectorProcessorClass::CopyIndexed(float* dst, float* src, ...)` (vp.cpp:448).
pub fn copy_indexed_f32(dst: &mut [f32], src: &[f32], index: &[u32], count: i32) {
    copy_indexed(dst, src, index, count);
}
