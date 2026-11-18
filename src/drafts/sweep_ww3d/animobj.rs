//! UNTESTED DRAFT (#202, #214): compiles; not yet tested against the upstream C++.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.cpp
//!   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.h
//!
//! `Animatable3DObjClass::Simple_Evaluate_Bone` (both overloads) and `::Set_HTree`. The verified port animates a model
//! through `formats::w3d::render::RenderModel` (`compute_current_frame` is a free function there) and the client's
//! `w3d_model_draw`; neither is the runtime `Animatable3DObjClass`, so the parts these functions read are
//! [`Animatable3DState`] and the [`AnimatableHost`] contract (the current frame, the pivot evaluators of
//! `formats::w3d::htree::HTree`, and the lazy hierarchy update).

use crate::logic::modules::plant;
use crate::formats::w3d::htree::HTree;
use crate::ww::matrix3d::Matrix3D;

/// `Animatable3DObjClass::CurMotionMode` (animobj.h): `NONE`, `BASE_POSE`, `SINGLE_ANIM`, `DOUBLE_ANIM`, `MULTIPLE_ANIM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    None,
    BasePose,
    SingleAnim,
    DoubleAnim,
    MultipleAnim,
}

/// What `Simple_Evaluate_Bone` reads of the object.
#[derive(Clone, Debug)]
pub struct Animatable3DState {
    /// `HTree` (NULL when `None`).
    pub htree: Option<HTree>,
    /// `CurMotionMode`.
    pub cur_motion_mode: MotionMode,
    /// `Transform` (also `Get_Transform()`).
    pub transform: Matrix3D,
}

/// The calls out of `Simple_Evaluate_Bone`.
pub trait AnimatableHost {
    /// `Compute_Current_Frame()` (with its default `newDirection = NULL`).
    fn compute_current_frame(&self) -> f32;
    /// `HTree->Simple_Evaluate_Pivot(ModeAnim.Motion, boneindex, frame, Get_Transform(), tm)`.
    fn htree_simple_evaluate_pivot_anim(&self, boneindex: i32, frame: f32, obj_tm: &Matrix3D, tm: &mut Matrix3D) -> bool;
    /// `HTree->Simple_Evaluate_Pivot(boneindex, Get_Transform(), tm)`.
    fn htree_simple_evaluate_pivot(&self, boneindex: i32, obj_tm: &Matrix3D, tm: &mut Matrix3D) -> bool;
    /// `const_cast<Animatable3DObjClass *>(this)->Update_Sub_Object_Transforms()`.
    fn update_sub_object_transforms(&mut self);
    /// `HTree->Get_Transform(boneindex)`.
    fn htree_get_transform(&self, boneindex: i32) -> Matrix3D;
}

/// `Animatable3DObjClass::Simple_Evaluate_Bone(boneindex, tm)` (animobj.cpp:870): for the simple modes (`NONE`,
/// `BASE_POSE`, `SINGLE_ANIM`) evaluate the bone at the current frame without updating the hierarchy; otherwise bring the
/// hierarchy up to date and read the bone's transform.
pub fn simple_evaluate_bone<H: AnimatableHost>(state: &Animatable3DState, host: &mut H, boneindex: i32, tm: &mut Matrix3D) -> bool {
    let mut retval = false;

    // Only do this for simple animations
    if state.cur_motion_mode == MotionMode::None
        || state.cur_motion_mode == MotionMode::BasePose
        || state.cur_motion_mode == MotionMode::SingleAnim
        || (plant(11110) && state.cur_motion_mode == MotionMode::DoubleAnim)
    {
        // Determine which frame we should be on, then use this information to determine the bone's transform.
        let curr_frame = host.compute_current_frame();
        retval = simple_evaluate_bone_frame(state, host, boneindex, if plant(11116) { 0.0 } else { curr_frame }, tm);
    } else {
        if !plant(11114) {
            host.update_sub_object_transforms();
        }
        *tm = host.htree_get_transform(boneindex);
    }

    retval
}

/// `Animatable3DObjClass::Simple_Evaluate_Bone(boneindex, frame, tm)` (animobj.cpp:912): with a hierarchy, `SINGLE_ANIM`
/// evaluates against the motion at `frame`, `NONE` / `BASE_POSE` against the base pose, any other mode gives the object's
/// own `Transform` (and false); without a hierarchy also `Transform`.
pub fn simple_evaluate_bone_frame<H: AnimatableHost>(
    state: &Animatable3DState,
    host: &H,
    boneindex: i32,
    frame: f32,
    tm: &mut Matrix3D,
) -> bool {
    let mut retval = false;

    if state.htree.is_some() {
        if state.cur_motion_mode == MotionMode::SingleAnim {
            retval = host.htree_simple_evaluate_pivot_anim(boneindex, if plant(11111) { 0.0 } else { frame }, &state.transform, tm);
        } else if state.cur_motion_mode == MotionMode::None || state.cur_motion_mode == MotionMode::BasePose {
            retval = host.htree_simple_evaluate_pivot(boneindex, &state.transform, tm);
        } else if !plant(11112) {
            *tm = state.transform;
        }
    } else if !plant(11113) {
        *tm = state.transform;
    }

    retval
}

/// `Animatable3DObjClass::Set_HTree(new_htree)` (animobj.cpp:1131): "(gth) TESTING DYNAMICALLY SWAPPING SKELETONS!". The
/// old hierarchy is deleted and replaced by a copy of the new one (`new HTreeClass(*new_htree)`); the structure should have
/// the same pivot count (`WWASSERT`, a debug assertion; upstream also dereferences `HTree` in it, so it must be non-NULL).
pub fn set_htree(state: &mut Animatable3DState, new_htree: &HTree) {
    debug_assert!(state.htree.as_ref().map_or(true, |h| h.num_pivots == new_htree.num_pivots));

    // just assign it...
    if !plant(11115) || state.htree.is_none() {
        state.htree = Some(new_htree.clone());
    }
}
