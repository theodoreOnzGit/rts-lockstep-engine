// SPDX-License-Identifier: GPL-3.0-only
//! Readers for the game's data formats. So far: `.big` archives, `.csf`
//! string files and the chunk format of maps (`DataChunk`).

pub mod big;
pub mod csf;
pub mod data_chunk;
