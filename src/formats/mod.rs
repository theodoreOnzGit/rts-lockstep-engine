// SPDX-License-Identifier: GPL-3.0-only
//! Readers for the game's data formats. So far: `.big` archives, `.csf`
//! string files, the chunk format of maps (`DataChunk`), W3D files
//! (meshes, hierarchies, animations) and textures (Targa, DDS).

pub mod big;
pub mod csf;
pub mod data_chunk;
pub mod texture;
pub mod w3d;
// --- p1w4-sides ---
pub mod data_chunk_output;
// --- end p1w4-sides ---
