//! DIFFERENTIAL-TESTED (#202, #215): matches the upstream C++ on 110,224 calls (test: tests/differential_draft_lists.rs);
//! not yet wired into the game.
//! Not covered: `peek_camera` (`return Camera;`).
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/visrasterizer.cpp
//!
//! `VisRasterizerClass::Peek_Camera` and `::Get_Temp_Vertex_Buffer`, which the first pass
//! (`drafts::ww3d::visrasterizer`, where the camera is passed to each call as a [`VisCamera`] and `TempVertexBuffer` is
//! private) left out. The camera is a host handle here (`Camera`, `None` is NULL).
//!
//! [`VisCamera`]: crate::drafts::ww3d::visrasterizer::VisCamera

use crate::ww::vector3::Vector3;

use crate::logic::modules::plant;
/// `VisRasterizerClass::Peek_Camera()` (visrasterizer.cpp:188): the camera without a reference.
pub fn peek_camera(camera: Option<usize>) -> Option<usize> {
    camera
}

/// `VisRasterizerClass::Get_Temp_Vertex_Buffer(count)` (visrasterizer.cpp:215): `TempVertexBuffer.Uninitialised_Grow(count)`
/// (grow, never shrink, keeping the old contents; the new elements are uninitialised upstream, zero here) and a pointer to
/// element 0, here the first `count` elements.
pub fn get_temp_vertex_buffer(temp_vertex_buffer: &mut Vec<Vector3>, count: i32) -> &mut [Vector3] {
    let count = if count < 0 { 0 } else { count as usize };
    if plant(11676) {
        temp_vertex_buffer.resize(count, Vector3::new(0.0, 0.0, 0.0));
    } else if temp_vertex_buffer.len() < count {
        temp_vertex_buffer.resize(if plant(11675) { count + 1 } else { count }, Vector3::new(0.0, 0.0, 0.0));
    }
    &mut temp_vertex_buffer[..count]
}
