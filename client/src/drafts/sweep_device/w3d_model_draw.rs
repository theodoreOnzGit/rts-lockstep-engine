//! UNTESTED DRAFT (#202, #213): written without compiling or testing. Not part of the verified port until it is
//! compiled, differential-tested against the upstream C++ and moved out of `drafts`.
//!
//! Translated from (EA's GPLv3 release, upstream commit 0a05454d):
//!   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//!
//! Sweep 2 (#213): the members of `W3DModelDraw` whose names were absent from the port, as functions on the state they use.
//!
//! Drafted: `setTerrainDecalOpacity`, `replaceIndicatorColor`, `clientOnly_getRenderObjInfo`,
//! `clientOnly_getRenderObjBoundBox`, `clientOnly_getRenderObjBoneTransform`, `updateProjectileClipStatus`,
//! `updateDrawModuleSupplyStatus`, `doHideShowProjectileObjects`.
//!
//! Excluded (listed in the #213 comment):
//! - `LogClass::dumpMatrix3D` / `dumpReal` (W3DModelDraw.cpp:164, 176): inside `#if defined(DEBUG_CRC) && (defined(_DEBUG) ||
//!   defined(_INTERNAL))` (line 88), else the `#else // DEBUG_CRC` stubs (line 194). Debug only.
//! - `getRenderCostRecursive` (1900): inside `#if defined(_DEBUG) || defined(_INTERNAL)` (1899-1936). Debug only.
//! - `hideGarrisonFlags` (2822): `#if defined(_DEBUG) || defined(_INTERNAL)` ("art wants to see buildings without flags").
//! - `getAnimationScrubScalar` (3867): inside `#ifdef ALLOW_ANIM_INQUIRIES`, which no header defines.

use rts_lockstep_engine::common::base_type::Coord3D;
use rts_lockstep_engine::ww::matrix3d::Matrix3D;
use rts_lockstep_engine::ww::vector3::Vector3;

/// `NO_NEXT_DURATION` (W3DModelDraw.h): `m_nextStateAnimLoopDuration` when there is none.
pub const NO_NEXT_DURATION: u32 = 0xffff_ffff;

/// `ModelConditionInfo::HideShowSubObjInfo`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HideShowSubObjInfo {
    pub sub_obj_name: Vec<u8>,
    pub hide: bool,
}

/// `MODELCONDITION_CARRYING` set or cleared by `W3DModelDraw::updateDrawModuleSupplyStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupplyCondition {
    /// `setModelConditionState(MODELCONDITION_CARRYING)`.
    SetCarrying,
    /// `clearModelConditionState(MODELCONDITION_CARRYING)`.
    ClearCarrying,
}

/// `W3DModelDraw::updateDrawModuleSupplyStatus` (W3DModelDraw.cpp:3931): carrying while any supply is held.
pub fn update_draw_module_supply_status(_max_supply: i32, current_supply: i32) -> SupplyCondition {
    if current_supply > 0 {
        SupplyCondition::SetCarrying
    } else {
        SupplyCondition::ClearCarrying
    }
}

/// `W3DModelDraw::setTerrainDecalOpacity` (W3DModelDraw.cpp:2777): the integer opacity for the decal (`(Int)(255 * o)`),
/// or None without a terrain decal.
pub fn set_terrain_decal_opacity(have_terrain_decal: bool, o: f32) -> Option<i32> {
    if have_terrain_decal {
        return Some((255.0 * o) as i32);
    }
    None
}

/// What `replaceIndicatorColor` decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplaceIndicatorColor {
    /// Not allowed to change the model colour, or no render object, or the colour is unchanged.
    Nothing,
    /// `m_hexColor = newColor`, `m_curState = NULL`, `m_nextState = NULL`, `m_nextStateAnimLoopDuration =
    /// NO_NEXT_DURATION`, then `setModelState(oldCurState)`.
    RebuildState { new_hex_color: i32 },
}

/// `W3DModelDraw::replaceIndicatorColor` (W3DModelDraw.cpp:3212): `Color` 0 means no tint, any other gets full alpha.
pub fn replace_indicator_color(ok_to_change_model_color: bool, have_render_object: bool, cur_hex_color: i32, color: u32) -> ReplaceIndicatorColor {
    if !ok_to_change_model_color {
        return ReplaceIndicatorColor::Nothing;
    }
    if have_render_object {
        let new_color: i32 = if color == 0 { 0 } else { (color | 0xFF00_0000) as i32 };
        if new_color != cur_hex_color {
            return ReplaceIndicatorColor::RebuildState { new_hex_color: new_color };
        }
    }
    ReplaceIndicatorColor::Nothing
}

/// `W3DModelDraw::clientOnly_getRenderObjInfo` (W3DModelDraw.cpp:3236): position, bounding sphere radius and transform of
/// the render object. `render_object` is `(position, transform, bounding_sphere_radius)`.
pub fn client_only_get_render_obj_info(render_object: Option<(Vector3, Matrix3D, f32)>) -> Option<(Coord3D, f32, Matrix3D)> {
    let (obj_pos, transform, radius) = render_object?;
    Some((Coord3D { x: obj_pos.x, y: obj_pos.y, z: obj_pos.z }, radius, transform))
}

/// An oriented box (`OBBoxClass`).
#[derive(Clone, Copy, Debug)]
pub struct OBBox {
    pub center: Vector3,
    pub extent: Vector3,
    pub basis: [[f32; 3]; 3],
}

/// `W3DModelDraw::clientOnly_getRenderObjBoundBox` (W3DModelDraw.cpp:3516): the object-space AABox as an `OBBoxClass`,
/// transformed by the render object's transform (`OBBoxClass::Transform(tm, box0, boundbox)`: the centre is transformed,
/// the basis is the transform's rotation, the extent is kept). `aabox` is `(center, extent)`.
pub fn client_only_get_render_obj_bound_box(render_object: Option<((Vector3, Vector3), Matrix3D)>) -> Option<OBBox> {
    let ((center, extent), tm) = render_object?;
    let c = Matrix3D::transform_vector(&tm, &center);
    // basis: the columns of the rotation part (X, Y, Z axes)
    let x = tm.rotate_vector(&Vector3 { x: 1.0, y: 0.0, z: 0.0 });
    let y = tm.rotate_vector(&Vector3 { x: 0.0, y: 1.0, z: 0.0 });
    let z = tm.rotate_vector(&Vector3 { x: 0.0, y: 0.0, z: 1.0 });
    Some(OBBox { center: c, extent, basis: [[x.x, x.y, x.z], [y.x, y.y, y.z], [z.x, z.y, z.z]] })
}

/// `W3DModelDraw::clientOnly_getRenderObjBoneTransform` (W3DModelDraw.cpp:3537): the bone's transform, `(false, identity)`
/// for bone index 0 ("not found"), `(false, _)` without a render object (the output is untouched upstream: None).
/// `bone_index` is `Get_Bone_Index(boneName)`, `bone_transform(idx)` is `Get_Bone_Transform(idx)`.
pub fn client_only_get_render_obj_bone_transform(
    have_render_object: bool,
    bone_index: i32,
    bone_transform: &impl Fn(i32) -> Matrix3D,
    identity: Matrix3D,
) -> (bool, Option<Matrix3D>) {
    if !have_render_object {
        return (false, None);
    }
    if bone_index == 0 {
        (false, Some(identity)) // set_tm->Make_Identity()
    } else {
        (true, Some(bone_transform(bone_index)))
    }
}

/// `W3DModelDraw::updateProjectileClipStatus` (W3DModelDraw.cpp:3922): only for slots flagged in
/// `m_projectileBoneFeedbackEnabledSlots`; shows `shotsRemaining` of `maxShots`.
pub fn update_projectile_clip_status(
    projectile_bone_feedback_enabled_slots: u32,
    shots_remaining: u32,
    max_shots: u32,
    slot: u32,
    launch_bone_name: &[u8],
    hide_show_name: &[u8],
) -> Option<Vec<HideShowSubObjInfo>> {
    if (projectile_bone_feedback_enabled_slots & (1 << slot)) == 0 {
        return None; // Only if specified.
    }
    do_hide_show_projectile_objects(shots_remaining, max_shots, launch_bone_name, hide_show_name) // Means effectively, show m of n.
}

/// `W3DModelDraw::doHideShowProjectileObjects` (W3DModelDraw.cpp:3945): the sub-object hide/show list for showing
/// `showCount` of `maxCount` projectiles. With a `m_weaponProjectileHideShowName` the one named object is hidden when any
/// are hidden; otherwise bones `<launchBone>01..` are hidden for the first `hideCount` and shown for the rest. None when
/// `maxCount < showCount` (upstream `DEBUG_CRASH`es and returns). The caller runs `doHideShowSubObjs` on the list.
pub fn do_hide_show_projectile_objects(show_count: u32, max_count: u32, launch_bone_name: &[u8], hide_show_name: &[u8]) -> Option<Vec<HideShowSubObjInfo>> {
    if max_count < show_count {
        return None; // Someone is trying to show more projectiles than they have.
    }
    let hide_count = (max_count - show_count) as i32;
    let mut show_hide_vector = Vec::new();
    if hide_show_name.is_empty() {
        for projectile_index in 0..max_count as i32 {
            let mut name = launch_bone_name.to_vec();
            name.extend_from_slice(format!("{:02}", projectile_index + 1).as_bytes());
            show_hide_vector.push(HideShowSubObjInfo { sub_obj_name: name, hide: (projectile_index + 1) <= hide_count });
        }
    } else {
        show_hide_vector.push(HideShowSubObjInfo { sub_obj_name: hide_show_name.to_vec(), hide: 0 < hide_count });
    }
    Some(show_hide_vector)
}
