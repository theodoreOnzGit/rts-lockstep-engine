// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp
//   (HTreeClass::Load_W3D, read_pivots, Free) and htree.h
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/pivot.h, pivot.cpp
//   (PivotClass's data and constructor)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, htree by Greg Hjelstrom)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! W3D hierarchies (`W3D_CHUNK_HIERARCHY`): `HTreeClass::Load_W3D`, which
//! reads a header and the pivots (bones) into a tree of base-pose
//! transforms. Only the loading; the animation blending of `htree.cpp` is
//! not translated.
//!
//! # As upstream reads it
//!
//! Inside the hierarchy chunk: a first chunk that must be
//! `W3D_CHUNK_HIERARCHY_HEADER` with a whole [`W3dHierarchyStruct`], then any
//! chunks; each `W3D_CHUNK_PIVOTS` is read as `NumPivots`
//! [`W3dPivotStruct`]s, the others are skipped. A file older than version
//! 3.0 gets an extra root pivot `"RootTransform"` at index 0 (and all parent
//! indices + 1). Each pivot's base transform is
//! `Identity.Translate(Translation) * Build_Matrix3D(Rotation)` (`postMul`);
//! the Euler angles are not used.
//!
//! # Upstream quirks that are kept
//!
//! - On a failure the chunks opened so far stay open (the caller's
//!   `Close_Chunk` then closes the wrong one); the header chunk is not
//!   closed when the header read fails, nor is a pivots chunk when a pivot
//!   read fails.
//! - Every `W3D_CHUNK_PIVOTS` chunk is read from pivot 0 again.
//! - Names are copied as 16 bytes (`memcpy`), so a 16-character name has
//!   no terminator.
//! - `NumPivots` is the header's `uint32` (+1, wrapping, before 3.0) as an
//!   `int`: a value of 2^31 or more is a negative count, for which no
//!   pivots are allocated.
//! - A parent index of `0xFFFFFFFF` (after the +1 of old files) is "no
//!   parent", for any pivot, not just the first.
//!
//! # Where this differs from upstream (only where upstream is undefined)
//!
//! [`W3dUndefined`]: a `W3D_CHUNK_PIVOTS` chunk when no pivots were
//! allocated (a count of 0 or less: `Pivot[0]` of a null array); a parent
//! index above `NumPivots` (a pointer out of the array); more pivots than
//! the 32-bit original could allocate (`NumPivots * sizeof(PivotClass)`
//! over 2^31 - 1 bytes, with the Win32 size of 172 bytes).
//!
//! # Representation
//!
//! [`HTree::pivots`] holds the pivots as read; it is empty when no pivots
//! chunk was read, in which case all [`HTree::num_pivots`] pivots are the
//! default ([`Pivot::default`], as `PivotClass()` makes them: empty name, no
//! parent, identity, index 0; upstream leaves the name bytes after the first
//! uninitialised, here they are 0).

use super::w3d_file::*;
use super::{W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::{build_matrix3d, Quaternion};
use crate::ww::vector3::Vector3;

/// `sizeof(PivotClass)` on Win32 (without `LAZY_CAP_MTX_ALLOC`).
pub const WIN32_SIZEOF_PIVOT: u64 = 172;
/// The largest single allocation the 32-bit original can make.
pub const WIN32_MAX_ALLOCATION: u64 = 0x7FFF_FFFF;

/// `struct PivotClass`: the members `Load_W3D` sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pivot {
    /// `Name` (16 bytes, as copied).
    pub name: [u8; W3D_NAME_LEN],
    /// The index of `Parent` in the tree (`None` for a null parent). May be
    /// `num_pivots` (one past the end), which upstream forms as a pointer.
    pub parent: Option<u32>,
    /// `BaseTransform`, relative to the parent.
    pub base_transform: Matrix3D,
    /// `Index`.
    pub index: i32,
}

impl Default for Pivot {
    /// `PivotClass()`.
    fn default() -> Self {
        let mut m = Matrix3D::default();
        m.make_identity();
        Pivot { name: [0; W3D_NAME_LEN], parent: None, base_transform: m, index: 0 }
    }
}

/// `class HTreeClass`: what `Load_W3D` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct HTree {
    /// `Name` (16 bytes, as copied).
    pub name: [u8; W3D_NAME_LEN],
    /// `NumPivots`.
    pub num_pivots: i32,
    /// The pivots as read, or empty if no pivots chunk was read (see the
    /// module docs).
    pub pivots: Vec<Pivot>,
    /// `ScaleFactor` (1 after loading).
    pub scale_factor: f32,
}

impl HTree {
    /// `Num_Pivots()`.
    pub fn num_pivots(&self) -> i32 {
        self.num_pivots
    }

    /// Pivot `i`: as read, or the default pivot when none were read.
    pub fn pivot(&self, i: usize) -> Option<Pivot> {
        if i >= self.num_pivots.max(0) as usize {
            return None;
        }
        Some(self.pivots.get(i).copied().unwrap_or_default())
    }
}

/// `HTreeClass::Load_W3D(cload)`, with `cload` inside the hierarchy chunk.
/// `Ok(None)` is upstream's `LOAD_ERROR`.
pub fn load_w3d(cload: &mut ChunkLoad<'_>) -> W3dResult<Option<HTree>> {
    // Read the first chunk, it should be the hierarchy header
    if !cload.open_chunk()? {
        return Ok(None);
    }
    if cload.cur_chunk_id()? != W3D_CHUNK_HIERARCHY_HEADER {
        return Ok(None);
    }
    let mut hb = [0u8; W3dHierarchyStruct::SIZE];
    if cload.read_into(&mut hb)? != W3dHierarchyStruct::SIZE as u32 {
        return Ok(None);
    }
    let mut header = W3dHierarchyStruct::from_bytes(&hb);
    cload.close_chunk()?;

    // Check the version, if < 3.0 add a root node
    let mut pre30 = false;
    if header.version < w3d_make_version(3, 0) {
        header.num_pivots = header.num_pivots.wrapping_add(1);
        pre30 = true;
    }

    let num_pivots = header.num_pivots as i32;
    if num_pivots > 0 && num_pivots as u64 * WIN32_SIZEOF_PIVOT > WIN32_MAX_ALLOCATION {
        return Err(W3dUndefined::Allocation.into());
    }
    let mut tree = HTree { name: header.name, num_pivots, pivots: Vec::new(), scale_factor: 1.0 };

    while cload.open_chunk()? {
        if cload.cur_chunk_id()? == W3D_CHUNK_PIVOTS && !read_pivots(cload, &mut tree, pre30)? {
            // Error: Free() and return LOAD_ERROR, the chunk left open
            return Ok(None);
        }
        cload.close_chunk()?;
    }
    Ok(Some(tree))
}

/// `HTreeClass::read_pivots(cload, pre30)`.
fn read_pivots(cload: &mut ChunkLoad<'_>, tree: &mut HTree, pre30: bool) -> W3dResult<bool> {
    if tree.num_pivots <= 0 {
        // Pivot is NULL: Pivot[0] is dereferenced below
        return Err(W3dUndefined::NoPivots.into());
    }
    let n = tree.num_pivots as usize;
    // Every pivot is rewritten (or the load fails and Free() discards them),
    // so the pivots are collected afresh; memory grows only with the bytes
    // actually read.
    let mut pivots = Vec::new();
    let mut first_piv = 0;
    if pre30 {
        let mut root = Pivot::default();
        root.name[..13].copy_from_slice(b"RootTransform");
        pivots.push(root);
        first_piv = 1;
    }
    for pidx in first_piv..n {
        let mut pb = [0u8; W3dPivotStruct::SIZE];
        if cload.read_into(&mut pb)? != W3dPivotStruct::SIZE as u32 {
            return Ok(false);
        }
        let mut piv = W3dPivotStruct::from_bytes(&pb);

        let mut base = Matrix3D::default();
        base.make_identity();
        base.translate_v(&Vector3::new(piv.translation.x, piv.translation.y, piv.translation.z));
        let q = piv.rotation.q;
        base.post_mul(&build_matrix3d(&Quaternion::new(q[0], q[1], q[2], q[3])));

        if pre30 {
            piv.parent_idx = piv.parent_idx.wrapping_add(1);
        }
        let parent = if piv.parent_idx == 0xFFFF_FFFF {
            None
        } else {
            if piv.parent_idx as u64 > n as u64 {
                return Err(W3dUndefined::PointerOutOfRange.into());
            }
            Some(piv.parent_idx)
        };
        pivots.push(Pivot { name: piv.name, parent, base_transform: base, index: pidx as i32 });
    }
    // Pivot[0].Transform = identity, IsVisible = true: not represented.
    tree.pivots = pivots;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ww::chunkio::GameFile;

    fn chunk(id: u32, body: &[u8]) -> Vec<u8> {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    #[test]
    fn pre30_hierarchy_gets_a_root() {
        let mut h = W3dHierarchyStruct::from_bytes(&[0; 36]);
        h.version = w3d_make_version(2, 0);
        h.num_pivots = 1;
        h.name[..3].copy_from_slice(b"SKL");
        let mut p = W3dPivotStruct::from_bytes(&[0; 60]);
        p.name[..4].copy_from_slice(b"BONE");
        p.parent_idx = 0xFFFF_FFFF;
        p.rotation.q = [0.0, 0.0, 0.0, 1.0];
        p.translation.x = 2.0;
        let mut body = chunk(W3D_CHUNK_HIERARCHY_HEADER, &h.to_bytes());
        body.extend(chunk(W3D_CHUNK_PIVOTS, &p.to_bytes()));
        let data = chunk(W3D_CHUNK_HIERARCHY, &body);
        let mut c = ChunkLoad::new(GameFile::new(&data).unwrap());
        assert!(c.open_chunk().unwrap());
        let t = load_w3d(&mut c).unwrap().unwrap();
        assert_eq!(t.num_pivots, 2);
        assert_eq!(&t.pivots[0].name[..13], b"RootTransform");
        assert_eq!(t.pivots[1].parent, Some(0), "-1 + 1 = the new root");
        assert_eq!(t.pivots[1].base_transform[0][3], 2.0);
        assert_eq!(t.pivots[1].index, 1);
    }
}
