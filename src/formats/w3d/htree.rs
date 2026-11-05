// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp
//   (HTreeClass::Load_W3D, read_pivots, Free; Base_Update, both
//   Anim_Update, Blend_Update, both Simple_Evaluate_Pivot, Capture_Bone,
//   Release_Bone, Is_Bone_Captured, Control_Bone, Get_Bone_Control) and
//   htree.h (Get_Transform, Get_Visibility, Get_Root_Transform)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/pivot.h, pivot.cpp
//   (PivotClass's data and constructor, Capture_Update)
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
//! transforms, ~~Only the loading; the animation blending of `htree.cpp` is
//! not translated.~~ **Corrected 2026-11-05 (#10, step 3):** and the pose
//! updates that compute every pivot's transform: `Base_Update`,
//! `Anim_Update` (the virtual one and the raw-animation fast path),
//! `Blend_Update`, `Simple_Evaluate_Pivot`, and bone capture.
//! `Combo_Update` (`HAnimComboClass`, `hanim.cpp`) is not translated yet.
//!
//! # Pose updates, as upstream computes them
//!
//! `Pivot[0].Transform = root`; then for each pivot after the first, in
//! index order, `Transform = Parent->Transform * BaseTransform`, and if the
//! animation has the pivot (`index < Get_Num_Pivots()`), post-multiplied by
//! the animation: `Translate(trans * ScaleFactor)` then `postMul(
//! Build_Matrix3D(q))`, and the visibility from the animation; a captured
//! bone then applies its control transform (`PivotClass::Capture_Update`)
//! and is visible. `Blend_Update` blends two animations per pivot
//! (`(1.0 - percentage) * trans0 + percentage * trans1`, with the factor a
//! `double` converted to `float`; `Fast_Slerp` of the orientations;
//! visible if the first is, else if the second is).
//!
//! The raw fast path (`Anim_Update(root, HRawAnimClass *, frame)`, "for
//! use by 'Generals'") samples one integer frame,
//! `WWMath::Float_To_Long(frame)` (0 at or past `NumFrames`), without
//! interpolation, reading the channels directly
//! (`Get_Vector_As_Quat` for the orientation); `Translate(trans)` when
//! `ScaleFactor` is 1. `Float_To_Long` is the portable truncation here;
//! the shipped game's `fistp` rounds to nearest, so it shows frame
//! `round(frame)` where this shows `floor(frame)` (see [`super::hanim`]).
//!
//! # Upstream quirks that are kept (pose updates)
//!
//! - A parent later in the array contributes the transform it got in the
//!   *previous* update (identity before the first), as upstream reads it.
//! - `Anim_Update` and `Blend_Update` use the array index as the
//!   animation's pivot; `Simple_Evaluate_Pivot` uses each pivot's `Index`.
//! - `Simple_Evaluate_Pivot` walks the parents up to a pivot with none (the
//!   root's own animation is never applied) and returns `false` with an
//!   identity for a pivot index out of range.
//! - The order of the animation calls is upstream's, since the channels'
//!   caches depend on it (`Blend_Update` asks the second animation's
//!   visibility only when the first is invisible).
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
//! Pose updates ([`W3dUndefined`]): a tree with no pivots
//! ([`W3dUndefined::NoPivots`], `Pivot[0]` of a null array); a pivot after
//! the first with no parent (null `Parent`, [`W3dUndefined::NullPointer`];
//! also every pivot after the first of a tree whose pivots were never read);
//! a parent one past the array ([`W3dUndefined::IndexOutOfRange`]); an
//! animation call upstream leaves undefined (see [`super::hanim`] and
//! [`super::hmorphanim`]), including an orientation or transform left
//! uninitialised by a compressed animation of an unknown flavor
//! ([`W3dUndefined::UninitialisedRead`]); a bone index out of range; and a
//! parent chain with a cycle in `Simple_Evaluate_Pivot`
//! ([`super::W3dError::NonTerminating`]).
//!
//! Loading ([`W3dUndefined`]): a `W3D_CHUNK_PIVOTS` chunk when no pivots were
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
//! uninitialised, here they are 0). The pose updates keep each pivot's
//! computed `Transform` and `IsVisible` and its capture state in the
//! [`Pivot`].

use super::assetmgr::HAnim;
use super::hanim::HRawAnim;
use super::hmorphanim::PoseLookup;
use super::w3d_file::*;
use super::{f32_to_i32, W3dError, W3dResult, W3dUndefined};
use crate::ww::chunkio::ChunkLoad;
use crate::ww::wwmath;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::quat::{build_matrix3d, fast_slerp, Quaternion};
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
    /// `Transform`: the computed transform (identity until an update).
    pub transform: Matrix3D,
    /// `IsVisible`: the result of the visibility channel.
    pub is_visible: bool,
    /// `CapTransform`: the control transform of a captured bone.
    pub cap_transform: Matrix3D,
    /// `IsCaptured`.
    pub is_captured: bool,
    /// `WorldSpaceTranslation`.
    pub world_space_translation: bool,
}

impl Default for Pivot {
    /// `PivotClass()`.
    fn default() -> Self {
        Pivot::new([0; W3D_NAME_LEN], None, Matrix3D::IDENTITY, 0)
    }
}

impl Pivot {
    /// A pivot as `read_pivots` sets it, the rest as `PivotClass()` does.
    pub fn new(name: [u8; W3D_NAME_LEN], parent: Option<u32>, base_transform: Matrix3D, index: i32) -> Self {
        Pivot {
            name,
            parent,
            base_transform,
            index,
            transform: Matrix3D::IDENTITY,
            is_visible: true,
            cap_transform: Matrix3D::IDENTITY,
            is_captured: false,
            world_space_translation: false,
        }
    }

    /// `PivotClass::Capture_Update()`: apply the control transform.
    pub fn capture_update(&mut self) {
        let ct = self.cap_transform;
        if self.world_space_translation {
            // The Translation of CapTransform is meant to be in world space,
            // so remove before applying orientation
            let mut cap_orientation = ct;
            cap_orientation.set_translation(&Vector3::new(0.0, 0.0, 0.0));
            self.transform.post_mul(&cap_orientation);
            // Now apply translation in world space
            self.transform.adjust_translation(&ct.get_translation());
        } else {
            self.transform.post_mul(&ct);
        }
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

    /// The pivots for an update: `Pivot[0]` must exist; a tree whose pivots
    /// were never read gets its one default pivot (with more, pivot 1 has a
    /// null parent).
    fn begin_update(&mut self, root: &Matrix3D) -> W3dResult<()> {
        if self.num_pivots <= 0 {
            return Err(W3dUndefined::NoPivots.into());
        }
        if self.pivots.is_empty() {
            if self.num_pivots > 1 {
                return Err(W3dUndefined::NullPointer.into());
            }
            self.pivots.push(Pivot::default());
        }
        self.pivots[0].transform = *root;
        self.pivots[0].is_visible = true;
        Ok(())
    }

    /// `Matrix3D::Multiply(pivot->Parent->Transform, pivot->BaseTransform,
    /// &pivot->Transform)`: the base pose of pivot `i`.
    fn base_pose(&mut self, i: usize) -> W3dResult<()> {
        let parent = self.pivots[i].parent.ok_or(W3dUndefined::NullPointer)?;
        let ptm = self.pivots.get(parent as usize).ok_or(W3dUndefined::IndexOutOfRange)?.transform;
        let p = &mut self.pivots[i];
        p.transform = Matrix3D::multiply(&ptm, &p.base_transform);
        Ok(())
    }

    /// `if (pivot->Is_Captured()) { Capture_Update(); IsVisible = true; }`
    fn captured(&mut self, i: usize) {
        let p = &mut self.pivots[i];
        if p.is_captured {
            p.capture_update();
            p.is_visible = true;
        }
    }

    /// `Base_Update(root)`: the base pose.
    pub fn base_update(&mut self, root: &Matrix3D) -> W3dResult<()> {
        self.begin_update(root)?;
        for i in 1..self.num_pivots as usize {
            self.base_pose(i)?;
            self.pivots[i].is_visible = true;
            if self.pivots[i].is_captured {
                self.pivots[i].capture_update();
            }
        }
        Ok(())
    }

    /// `Anim_Update(root, motion, frame)` (the `HAnimClass *` overload).
    pub fn anim_update<P: PoseLookup>(&mut self, root: &Matrix3D, motion: &HAnim, poses: &P, frame: f32) -> W3dResult<()> {
        self.begin_update(root)?;
        let num_anim_pivots = motion.get_num_pivots();
        for i in 1..self.num_pivots as usize {
            // base pose
            self.base_pose(i)?;
            // Don't update this pivot if the HTree doesn't have animation data for it...
            if (i as i32) < num_anim_pivots {
                // animation
                let trans = motion.get_translation(poses, i as i32, frame)?;
                let scaled = trans * self.scale_factor;
                self.pivots[i].transform.translate_v(&scaled);
                let q = motion.get_orientation(poses, i as i32, frame)?.ok_or(W3dUndefined::UninitialisedRead)?;
                self.pivots[i].transform.post_mul(&build_matrix3d(&q));
                // visibility
                self.pivots[i].is_visible = motion.get_visibility(i as i32, frame)?;
            }
            self.captured(i);
        }
        Ok(())
    }

    /// `Anim_Update(root, HRawAnimClass *motion, frame)`: the raw fast path
    /// (one integer frame, no interpolation).
    pub fn anim_update_raw(&mut self, root: &Matrix3D, motion: &HRawAnim, frame: f32) -> W3dResult<()> {
        self.begin_update(root)?;
        let num_anim_pivots = motion.get_num_pivots();
        // Get integer frame (WWMath::Float_To_Long, a 32-bit long)
        f32_to_i32(frame)?;
        let mut iframe = wwmath::float_to_long(frame);
        if iframe >= motion.get_num_frames() {
            iframe = 0;
        }
        for i in 1..self.num_pivots as usize {
            // base pose
            self.base_pose(i)?;
            if (i as i32) < num_anim_pivots {
                let m = motion.node_motion_at(i as i32)?;
                let mut t = [0.0f32; 3];
                if let Some(x) = &m.x {
                    x.get_vector(iframe, &mut t[0..])?;
                }
                if let Some(y) = &m.y {
                    y.get_vector(iframe, &mut t[1..])?;
                }
                if let Some(z) = &m.z {
                    z.get_vector(iframe, &mut t[2..])?;
                }
                let trans = Vector3::new(t[0], t[1], t[2]);
                let scale = self.scale_factor;
                let xform = &mut self.pivots[i].transform;
                if scale == 1.0f32 {
                    xform.translate_v(&trans);
                } else {
                    xform.translate_v(&(trans * scale));
                }
                if let Some(q) = &m.q {
                    let q = q.get_vector_as_quat(iframe)?;
                    xform.post_mul(&build_matrix3d(&q));
                }
                // visibility
                self.pivots[i].is_visible = match &m.vis {
                    Some(v) => v.get_bit(iframe)? == 1,
                    None => true,
                };
            }
            self.captured(i);
        }
        Ok(())
    }

    /// `Blend_Update(root, motion0, frame0, motion1, frame1, percentage)`
    /// (0 is `motion0`, 1 is `motion1`).
    #[allow(clippy::too_many_arguments)]
    pub fn blend_update<P: PoseLookup>(
        &mut self,
        root: &Matrix3D,
        motion0: &HAnim,
        frame0: f32,
        motion1: &HAnim,
        frame1: f32,
        percentage: f32,
        poses: &P,
    ) -> W3dResult<()> {
        self.begin_update(root)?;
        let num_anim_pivots = motion0.get_num_pivots().min(motion1.get_num_pivots());
        for i in 1..self.num_pivots as usize {
            self.base_pose(i)?;
            if (i as i32) < num_anim_pivots {
                let pi = i as i32;
                // interpolated translation
                let trans0 = motion0.get_translation(poses, pi, frame0)?;
                let trans1 = motion1.get_translation(poses, pi, frame1)?;
                let k0 = (1.0f64 - f64::from(percentage)) as f32;
                let lerped = k0 * trans0 + percentage * trans1;
                let scaled = lerped * self.scale_factor;
                self.pivots[i].transform.translate_v(&scaled);
                // interpolated rotation
                let q0 = motion0.get_orientation(poses, pi, frame0)?.ok_or(W3dUndefined::UninitialisedRead)?;
                let q1 = motion1.get_orientation(poses, pi, frame1)?.ok_or(W3dUndefined::UninitialisedRead)?;
                let q = fast_slerp(&q0, &q1, percentage);
                self.pivots[i].transform.post_mul(&build_matrix3d(&q));
                self.pivots[i].is_visible = motion0.get_visibility(pi, frame0)? || motion1.get_visibility(pi, frame1)?;
            }
            self.captured(i);
        }
        Ok(())
    }

    /// The parent of pivot `p` in `Simple_Evaluate_Pivot`'s walk (a parent
    /// one past the array is read: undefined).
    fn walk_parent(&self, p: &Pivot) -> W3dResult<Option<Pivot>> {
        match p.parent {
            None => Ok(None),
            Some(i) => self.pivot(i as usize).map(Some).ok_or_else(|| W3dUndefined::IndexOutOfRange.into()),
        }
    }

    /// `Simple_Evaluate_Pivot(motion, pivot_index, frame, obj_tm, &end_tm)`:
    /// the pivot's world transform at the frame, without updating the tree.
    /// `(false, identity)` for a pivot index out of range.
    pub fn simple_evaluate_pivot_anim<P: PoseLookup>(
        &self,
        motion: &HAnim,
        poses: &P,
        pivot_index: i32,
        frame: f32,
        obj_tm: &Matrix3D,
    ) -> W3dResult<(bool, Matrix3D)> {
        let mut end_tm = Matrix3D::IDENTITY;
        if pivot_index < 0 || pivot_index >= self.num_pivots {
            return Ok((false, end_tm));
        }
        let mut pivot = self.pivot(pivot_index as usize).ok_or(W3dUndefined::NoPivots)?;
        let mut steps = 0i64;
        while let Some(parent) = self.walk_parent(&pivot)? {
            steps += 1;
            if steps > i64::from(self.num_pivots) {
                return Err(W3dError::NonTerminating);
            }
            // Build a matrix that represents the animation for this pivot
            let mut anim_tm =
                motion.get_transform(poses, pivot.index, frame)?.ok_or(W3dUndefined::UninitialisedRead)?;
            let trans = anim_tm.get_translation();
            anim_tm.set_translation(&(trans * self.scale_factor));
            // Transform the animation transform by the 'relative-to-parent' transform.
            let curr_tm = Matrix3D::multiply(&pivot.base_transform, &anim_tm);
            end_tm.pre_mul(&curr_tm);
            pivot = parent;
        }
        end_tm.pre_mul(obj_tm);
        Ok((true, end_tm))
    }

    /// `Simple_Evaluate_Pivot(pivot_index, obj_tm, &end_tm)`: the base pose
    /// of the pivot in world space. `(false, identity)` out of range.
    pub fn simple_evaluate_pivot(&self, pivot_index: i32, obj_tm: &Matrix3D) -> W3dResult<(bool, Matrix3D)> {
        let mut end_tm = Matrix3D::IDENTITY;
        if pivot_index < 0 || pivot_index >= self.num_pivots {
            return Ok((false, end_tm));
        }
        let mut pivot = self.pivot(pivot_index as usize).ok_or(W3dUndefined::NoPivots)?;
        let mut steps = 0i64;
        while let Some(parent) = self.walk_parent(&pivot)? {
            steps += 1;
            if steps > i64::from(self.num_pivots) {
                return Err(W3dError::NonTerminating);
            }
            let anim_tm = Matrix3D::IDENTITY;
            let curr_tm = Matrix3D::multiply(&pivot.base_transform, &anim_tm);
            end_tm = Matrix3D::multiply(&curr_tm, &end_tm);
            pivot = parent;
        }
        end_tm = Matrix3D::multiply(obj_tm, &end_tm);
        Ok((true, end_tm))
    }

    /// Pivot `i` for the bone functions (upstream only asserts the range).
    fn bone(&mut self, i: i32) -> W3dResult<&mut Pivot> {
        if i < 0 || i >= self.num_pivots {
            return Err(W3dUndefined::IndexOutOfRange.into());
        }
        if self.pivots.is_empty() {
            // the defaults, made explicit so that the bone can keep state
            let n = self.num_pivots as usize;
            self.pivots = vec![Pivot::default(); n];
        }
        Ok(&mut self.pivots[i as usize])
    }

    /// `Capture_Bone(boneindex)`.
    pub fn capture_bone(&mut self, boneindex: i32) -> W3dResult<()> {
        self.bone(boneindex)?.is_captured = true;
        Ok(())
    }

    /// `Release_Bone(boneindex)`.
    pub fn release_bone(&mut self, boneindex: i32) -> W3dResult<()> {
        self.bone(boneindex)?.is_captured = false;
        Ok(())
    }

    /// `Is_Bone_Captured(boneindex)`.
    pub fn is_bone_captured(&self, boneindex: i32) -> W3dResult<bool> {
        let p = self.pivot(usize::try_from(boneindex).unwrap_or(usize::MAX)).ok_or(W3dUndefined::IndexOutOfRange)?;
        Ok(p.is_captured)
    }

    /// `Control_Bone(boneindex, relative_tm, world_space_translation)`
    /// (whether or not the bone is captured: upstream only asserts it).
    pub fn control_bone(&mut self, boneindex: i32, relative_tm: &Matrix3D, world_space_translation: bool) -> W3dResult<()> {
        let p = self.bone(boneindex)?;
        p.world_space_translation = world_space_translation;
        p.cap_transform = *relative_tm;
        Ok(())
    }

    /// `Get_Bone_Control(boneindex, relative_tm)`: the control transform of
    /// a captured bone, else identity.
    pub fn get_bone_control(&self, boneindex: i32) -> W3dResult<Matrix3D> {
        let p = self.pivot(usize::try_from(boneindex).unwrap_or(usize::MAX)).ok_or(W3dUndefined::IndexOutOfRange)?;
        Ok(if p.is_captured { p.cap_transform } else { Matrix3D::IDENTITY })
    }

    /// `Get_Transform(pivot)`: the transform the last update computed.
    pub fn get_transform(&self, pivot: i32) -> W3dResult<Matrix3D> {
        let p = self.pivot(usize::try_from(pivot).unwrap_or(usize::MAX)).ok_or(W3dUndefined::IndexOutOfRange)?;
        Ok(p.transform)
    }

    /// `Get_Visibility(pivot)`.
    pub fn get_visibility(&self, pivot: i32) -> W3dResult<bool> {
        let p = self.pivot(usize::try_from(pivot).unwrap_or(usize::MAX)).ok_or(W3dUndefined::IndexOutOfRange)?;
        Ok(p.is_visible)
    }

    /// `Get_Root_Transform()`: `Pivot[0].Transform`.
    pub fn get_root_transform(&self) -> W3dResult<Matrix3D> {
        self.get_transform(0)
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
        pivots.push(Pivot::new(piv.name, parent, base, pidx as i32));
    }
    // Pivot[0].Transform = identity, IsVisible = true: as constructed.
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

    use crate::formats::w3d::assetmgr::HAnim;
    use crate::formats::w3d::hanim::tests::raw_anim;
    use crate::formats::w3d::hmorphanim::NoPoses;

    /// Two pivots: the root, and pivot 1 at (1, 0, 0) under it.
    fn two_pivots() -> HTree {
        let mut base = Matrix3D::IDENTITY;
        base.translate(1.0, 0.0, 0.0);
        HTree {
            name: [0; W3D_NAME_LEN],
            num_pivots: 2,
            pivots: vec![Pivot::new([0; W3D_NAME_LEN], None, Matrix3D::IDENTITY, 0), Pivot::new([0; W3D_NAME_LEN], Some(0), base, 1)],
            scale_factor: 1.0,
        }
    }

    /// The animation of `hanim::tests::raw_anim` at frame 2.0 (frames 1/2,
    /// ratio 1 under the portable truncation): translation x = 40 exactly,
    /// orientation the 90-degree turn about Z exactly (`Get_Orientation`
    /// returns q1 at ratio 1). Predictions: pivot 1 = translate(1,0,0) *
    /// translate(40,0,0) * Rz(90): translation (41, 0, 0) exactly (the
    /// rotation adds 0 to it), rotation [0][1] = -1 and [1][0] = 1 within
    /// float rounding (`Build_Matrix3D` of sqrt(1/2) components gives
    /// 1 - 2 * 0.49999997, not 0, on the diagonal). The raw fast path at
    /// frame 2.0 reads frame 2 itself: the same pose. `Blend_Update` of the
    /// animation with itself at 0.5: the same translation (0.5 * 40 + 0.5 *
    /// 40) ~~and the same quaternion (the slerp's linear branch), so the same
    /// matrix, bit for bit.~~ **Prediction missed (2026-11-05):** the
    /// quaternion's dot with itself is `2 * 0.70710677^2 = 0.99999994` in
    /// `float`, and `1 - 0.99999994 = 6e-8` is not below `WWMATH_EPSILON^2 =
    /// 1e-8`, so `Fast_Slerp` takes the table branch: the rotation entries
    /// come out 1.8e-7 / -0.9999998 instead of 6e-8 / -0.99999994. Same
    /// translation, rotation equal within 2e-7.
    #[test]
    fn two_pivot_anim_update() {
        let anim = HAnim::Raw(raw_anim());
        let mut t = two_pivots();
        t.anim_update(&Matrix3D::IDENTITY, &anim, &NoPoses, 2.0).unwrap();
        let m = t.get_transform(1).unwrap();
        assert_eq!(m.get_translation(), Vector3::new(41.0, 0.0, 0.0));
        assert!((m[0][1] + 1.0).abs() < 1e-6 && (m[1][0] - 1.0).abs() < 1e-6 && m[0][0].abs() < 1e-6, "{m:?}");
        assert!(t.get_visibility(1).unwrap());

        let mut r = two_pivots();
        let HAnim::Raw(raw) = &anim else { unreachable!() };
        r.anim_update_raw(&Matrix3D::IDENTITY, raw, 2.0).unwrap();
        assert_eq!(r.get_transform(1).unwrap(), m);

        let mut b = two_pivots();
        b.blend_update(&Matrix3D::IDENTITY, &anim, 2.0, &anim, 2.0, 0.5, &NoPoses).unwrap();
        let bm = b.get_transform(1).unwrap();
        assert_eq!(bm.get_translation(), m.get_translation());
        assert_ne!(bm, m, "the table slerp, not the linear branch");
        for (x, y) in bm.to_array().iter().zip(m.to_array().iter()) {
            assert!((x - y).abs() < 2e-7, "{bm:?}");
        }
    }

    /// Predictions: the base pose puts pivot 1 at (1, 0, 0) under a root at
    /// (0, 0, 2): (1, 0, 2). Captured with a control transform of
    /// translate(0, 0, 5): (1, 0, 7). `Simple_Evaluate_Pivot(1)` under an
    /// object at (10, 0, 0): (11, 0, 0); pivot 5 is out of range: false
    /// and the identity. A tree with no pivots cannot be updated.
    #[test]
    fn base_pose_capture_and_simple_evaluation() {
        let mut t = two_pivots();
        let mut root = Matrix3D::IDENTITY;
        root.translate(0.0, 0.0, 2.0);
        t.base_update(&root).unwrap();
        assert_eq!(t.get_transform(1).unwrap().get_translation(), Vector3::new(1.0, 0.0, 2.0));
        t.capture_bone(1).unwrap();
        let mut ctl = Matrix3D::IDENTITY;
        ctl.translate(0.0, 0.0, 5.0);
        t.control_bone(1, &ctl, false).unwrap();
        t.base_update(&root).unwrap();
        assert_eq!(t.get_transform(1).unwrap().get_translation(), Vector3::new(1.0, 0.0, 7.0));
        assert_eq!(t.get_bone_control(1).unwrap(), ctl);
        let mut obj = Matrix3D::IDENTITY;
        obj.translate(10.0, 0.0, 0.0);
        let (ok, m) = t.simple_evaluate_pivot(1, &obj).unwrap();
        assert!(ok);
        assert_eq!(m.get_translation(), Vector3::new(11.0, 0.0, 0.0));
        assert_eq!(t.simple_evaluate_pivot(5, &obj).unwrap(), (false, Matrix3D::IDENTITY));
        let mut empty = HTree { name: [0; W3D_NAME_LEN], num_pivots: 0, pivots: Vec::new(), scale_factor: 1.0 };
        assert!(empty.base_update(&root).is_err());
    }
}
