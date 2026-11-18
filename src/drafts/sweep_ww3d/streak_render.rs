//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/streakRender.cpp
//!
//! `StreakRendererClass::getVertexBuffer(number)`: the scratch vertex array (`m_vertexBuffer`, `VertexFormatXYZUV1`: position
//! and one texture coordinate) grown on demand to 1.5x the request. `_INTERNAL` (an internal-build define, not set in the
//! release) would fill it with `(float)0xdeadbeef`; that block is not translated.

/// `VertexFormatXYZUV1` (dx8fvf.h:174).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VertexFormatXYZUV1 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub u1: f32,
    pub v1: f32,
}

/// `m_vertexBuffer` and `m_vertexBufferSize`.
#[derive(Clone, Debug, Default)]
pub struct StreakVertexBuffer {
    pub vertex_buffer: Vec<VertexFormatXYZUV1>,
}

impl StreakVertexBuffer {
    /// `StreakRendererClass::getVertexBuffer(number)` (streakRender.cpp:1411): if `number` exceeds the allocated size, the
    /// old array is deleted and a new one of `number + (number >> 1)` vertices allocated (the old contents are NOT kept).
    /// Returns the array.
    pub fn get_vertex_buffer(&mut self, number: u32) -> &mut [VertexFormatXYZUV1] {
        if number as usize > self.vertex_buffer.len() {
            let number_to_alloc = number.wrapping_add(number >> 1);
            self.vertex_buffer = vec![VertexFormatXYZUV1::default(); number_to_alloc as usize];
        }
        &mut self.vertex_buffer[..]
    }
}
