//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/seglinerenderer.cpp
//!
//! `SegLineRendererClass::getVertexBuffer(number)`: the scratch vertex array (`m_vertexBuffer`) grown on demand to 1.5x the
//! request. The verified `formats::w3d::seglinerenderer` has the vertex type ([`SegLineVertex`] there, upstream
//! `VertexFormatXYZDUV1`); the buffer here is generic over it.
//!
//! [`SegLineVertex`]: crate::formats::w3d::seglinerenderer

/// `m_vertexBuffer` and `m_vertexBufferSize`.
#[derive(Clone, Debug, Default)]
pub struct SegLineVertexBuffer<V> {
    pub vertex_buffer: Vec<V>,
}

impl<V: Clone + Default> SegLineVertexBuffer<V> {
    /// `SegLineRendererClass::getVertexBuffer(number)` (seglinerenderer.cpp:1313): when `number` exceeds the allocated
    /// size, a new array of `number + (number >> 1)` vertices replaces the old (contents not kept). Returns the array.
    pub fn get_vertex_buffer(&mut self, number: u32) -> &mut [V] {
        if number as usize > self.vertex_buffer.len() {
            let number_to_alloc = number.wrapping_add(number >> 1);
            self.vertex_buffer = vec![V::default(); number_to_alloc as usize];
        }
        &mut self.vertex_buffer[..]
    }
}
