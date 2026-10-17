// SPDX-License-Identifier: GPL-3.0-only
//! Readers for the game's data formats. So far: `.big` archives, `.csf`
//! string files, the chunk format of maps (`DataChunk`) and W3D files
//! (meshes, hierarchies, animations).

pub mod big;
pub mod csf;
pub mod data_chunk;
pub mod w3d;
