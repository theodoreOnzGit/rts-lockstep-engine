// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DModelDraw.h
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DModelDraw.cpp
//   (W3DAnimationInfo, ModelConditionInfo, W3DModelDrawModuleData: its field
//   parse, parseConditionState and findBestInfo; W3DModelDraw: the constructor,
//   doDrawModule, findTransitionForSig, getCurrentAnimFraction,
//   adjustAnimation, setCurAnimDurationInMsec, getCurAnimDistanceCovered,
//   adjustAnimSpeedToMovementSpeed, setModelState, replaceModelConditionState,
//   setAnimationLoopDuration, setAnimationCompletionTime, setAnimationFrame,
//   setPauseAnimation, isAnimationComplete, turretNamesDiffer;
//   GitHub #27: findSingleBone, findSingleSubObj, doSingleBoneName,
//   ModelConditionInfo::validateStuff, validateCachedBones,
//   validateWeaponBarrelInfo, validateTurretInfo, findPristineBone,
//   WeaponBarrelInfo::setMuzzleFlashHidden, doHideShowBoneSubObjs,
//   W3DModelDraw::showSubObject, doHideShowSubObjs,
//   handleClientTurretPositioning, handleClientRecoil, hideAllHeadlights,
//   hideAllMuzzleFlashes, getBarrelCount, handleWeaponFireFX (without the
//   effect), rebuildWeaponRecoilInfo, updateSubObjects)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/htree.cpp
//   (Get_Bone_Index, Get_Parent_Index), rendobj.cpp (Get_Sub_Object_By_Name),
//   hlod.cpp (Get_Sub_Object_On_Bone), animobj.cpp (Capture_Bone,
//   Control_Bone)
//   GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/
//   W3DTankDraw.cpp, W3DTruckDraw.cpp, W3DTankTruckDraw.cpp, W3DSupplyDraw.cpp,
//   W3DDependencyModelDraw.cpp, W3DScienceModelDraw.cpp (their module data's
//   field parse only; GitHub #41: W3DTankDraw.cpp's constructor (the tread
//   state), updateTreadPositions, updateTreadObjects, onRenderObjRecreated and
//   doDrawModule without the tread debris; the trucks' own parts are in
//   w3d_truck_draw.rs, whose tread loop shares find_tread_objects here)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/animobj.cpp
//   (Animatable3DObjClass: Set_Animation, Single_Anim_Progress,
//   Is_Animation_Complete, Peek_Animation_And_Info,
//   Set_Animation_Frame_Rate_Multiplier)
//   GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/rendobj.h (TheAnimModeNames)
//   GeneralsMD/Code/GameEngine/Source/Common/GameLOD.cpp
//   (INI::parseStaticGameLODLevel, StaticGameLODNames)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (Westwood Studios, WW3D, by Greg Hjelstrom and others; the game's draw
//   modules by Steven Johnson, Colin Day, Mark Lorenzen and others)
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `W3DModelDraw`: the draw module that chooses an object's model and
//! animation from its drawable's model condition flags (GitHub #25, step 5 of
//! #10). No renderer: the render object here is the model's name, whether it
//! is an HLOD, and its animation's playback state; the client draws the
//! meshes (`client/src/model.rs`).
//!
//! # As upstream reads it
//!
//! - **The module data** ([`W3DModelDrawModuleData::parse`]):
//!   `W3DModelDrawModuleData::buildFieldParse` (and the subclasses' tables,
//!   [`DrawModuleExtra`]). `DefaultConditionState` must come first and only
//!   once; every `ConditionState` then starts as a copy of it (its condition
//!   bits cleared, its animations marked "copied", so the state's own first
//!   `Animation` replaces them); `AliasConditionState` adds a condition set to
//!   the previous state; `TransitionState A B` starts from the default too and
//!   goes into the transition map under the pair of lower-cased keys.
//!   Without a default state the first `ConditionState` must be for no bits.
//!   Bits named in `IgnoreConditionStates` may not appear in a later state;
//!   the same condition set may not appear twice. A state needs a `Model`
//!   (`None` for no model); idle and normal animations may not be mixed; idle
//!   animations and transitions play `ONCE` or `ONCE_BACKWARDS`.
//! - **The choice** ([`W3DModelDrawModuleData::find_best_info`]): the
//!   drawable's flags with the ignored bits cleared, through the
//!   [`SparseMatchFinder`] (most bits in common, then fewest extraneous bits,
//!   the first candidate on a tie; cached per flag set).
//! - **The state change** ([`W3DModelDraw::set_model_state`]): a request for
//!   the current state with nothing pending, or for the pending state, is
//!   ignored. A state whose `WaitForStateToFinishIfPossible` key is the
//!   current state's `TransitionKey` waits (pending) while the current
//!   animation runs. Between two states that both have a `TransitionKey`, a
//!   `TransitionState` for the pair plays first, the requested state pending.
//!   A new render object is made only when the model name (or the turret
//!   bones) differ; then the animation is chosen ([`W3DModelDraw::adjust_animation`]):
//!   one of the state's animations at random (with the client's generator;
//!   an idle state never repeats the animation it just played), its start
//!   frame from the mode (the last frame for the backward modes) and the
//!   `Flags` (`RANDOMSTART`, `START_FRAME_FIRST`, `START_FRAME_LAST`, or the
//!   previous state's fraction under a shared `MAINTAIN_FRAME_ACROSS_STATES*`
//!   flag), and a random speed factor in `AnimationSpeedFactorRange`.
//! - **Each frame** ([`W3DModelDraw::do_draw_module`]): pause the animation
//!   when the object should not animate; when the animation is complete,
//!   start the pending state (and its pending duration), and for an idle
//!   animation, or a state with `RESTART_ANIM_WHEN_COMPLETE`, choose again; an
//!   animation with a `DistanceCovered` is sped to the object's speed. The
//!   render then advances the frame (`Single_Anim_Progress` over
//!   `Compute_Current_Frame`, [`RenderObj::render`]) by the sync time.
//!
//! # Upstream quirks that are kept
//!
//! - (#27) A recoil never enters `RECOIL`: from `RECOIL_START` it goes to
//!   `SETTLE` at `MaxRecoil` or a tiny rate, so the muzzle flash shows until
//!   then. The muzzle flashes are hidden on the *old* render object, before a
//!   new one is made: a new render object's flash shows until the next draw.
//!   A sub-object hidden by a state stays hidden when the next state keeps the
//!   render object and names no sub-objects. `doHideShowBoneSubObjs` counts a
//!   sub-object's bone's ancestors, not its own bone, and with bone 0 every
//!   sub-object not on bone 0. Turret, barrel and pristine bones are found only
//!   at a logic time; a state first validated from the client's draw has none.
//! - A `ConditionState` copies the default state's **turret, weapon-bone and
//!   sub-object lists, flags and mode** as well as its model and animations
//!   (the whole `ModelConditionInfo`); only the condition bits are cleared.
//!   A `TransitionState` keeps the default's too, but its keys are cleared.
//! - `W3DAnimationInfo::m_naturalDurationInMsec` starts at -1 and is set the
//!   first time the animation's handle is fetched, in the module data shared
//!   by every drawable of the template; `setAnimationCompletionTime` reads it
//!   whether or not it was ever fetched.
//! - The maintained start frame is `REAL_TO_INT(fraction * NumFrames - 1)`
//!   (`- 1` binds to the product, not to `NumFrames`).
//! - `Is_Animation_Complete` compares the frame with `NumFrames - 1` (or 0)
//!   exactly, and is false for an HLOD without an animation (base pose) and
//!   true when there is no render object or it is not an HLOD (a mesh): a
//!   pending state of a model-less or mesh-only state starts on the next
//!   frame.
//! - `TURRETS_VALID` (which `turretNamesDiffer` needs, or it answers "differ"
//!   and the render object is remade) is set only when the state was
//!   validated "at a valid time to calculate logic stuff": inside a logic
//!   update or a load. Upstream creates drawables inside the logic update, so
//!   the constructor's validation counts; a state first entered from the
//!   client's draw (the dirty flags) does not. [`DrawEnv::logic_time`] says
//!   which.
//! - A transition state's `m_iniReadFlags` keeps `ANIMS_COPIED_FROM_DEFAULT_STATE`
//!   when it names no animation of its own, so a later `Animation` line would
//!   still replace the default's: kept as parsed.
//!
//! # Where this differs from upstream
//!
//! - **Name keys** (`TransitionKey`, `WaitForStateToFinishIfPossible`, the
//!   turret bones, the transition signature) are kept as the lower-cased
//!   names: the client may not allocate the logic's name keys (they are
//!   logic state). Equality is the same: `NAMEKEY` is one key per name and
//!   never `NAMEKEY_INVALID`, which is the empty name here.
//! - **`ParticleSysBone`** keeps the system's name, not a template pointer
//!   (~~the client has no particle systems yet~~ **Corrected 2026-11-06 (GitHub #43):** the client has them,
//!   `crate::client::particle_sys`, but this module does not make its bone systems yet); `RequiredScience` keeps the
//!   science's name.
//! - **Not translated yet** (GitHub #25's remaining work): ~~the bone caches
//!   (`validateCachedBones`, pristine bones, public bones beyond the parse,
//!   weapon barrels), turret and recoil posing, sub-object hide and show,
//!   muzzle flashes, headlights,~~ particle systems, shadows, terrain decals and
//!   tracks, the attach-to-bone offset, `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`,
//!   the weapon fire FX lists themselves, garrison flags, and the subclasses'
//!   runtime (~~treads,~~ tires, supplies, dependencies). ~~`validateStuff` only
//!   marks the turret info valid.~~ **Corrected 2026-11-06 (#27):** the bone
//!   caches, turret and recoil posing, muzzle flashes, sub-object hide and
//!   show and headlights are translated (below). **Corrected 2026-11-06
//!   (#41):** `W3DTankDraw`'s treads are translated
//!   ([`W3DModelDraw::update_tread_objects`], [`TankDraw`]); its tread debris
//!   (particle systems), `W3DTruckDraw`, `W3DTankTruckDraw` and
//!   `W3DPoliceCarDraw` are not (the truck's tires and steering need the
//!   drawable's `calcPhysicsXformWheels`, not translated yet).
//! - **The render object** ([`RenderObj`]) is not the drawn one: it keeps the
//!   bone captures and controls (`Capture_Bone`, `Control_Bone`) and the
//!   sub-objects' hidden flags, and the client puts them on the render model
//!   it draws ([`RenderObj::pose_render_model`]) before posing it. What the
//!   bone functions read of a render object (bone and sub-object names, bone
//!   indices, parents, transforms) is its [`Skeleton`], from the asset source.
//! - **The pristine pose** is a fresh render object's (the asset source's),
//!   even when `validateStuff` is given the live one, whose captured bones
//!   upstream would include. The sub-objects are [`RenderModel`]'s (top LOD,
//!   then aggregates, meshes only); upstream counts every LOD and reads a
//!   sub-object's bone as `Get_Sub_Object_Bone_Index(0, i)`, the same for the
//!   game's one-LOD models.
//! - **The logic's turret angles** are read from the World each draw
//!   ([`DrawFacts::turrets`], `World::ai_turret_rot_and_pitch`, which takes
//!   `&self`); upstream's draw module asks the object's AI itself.
//! - (#41) **The logic's motion** for `W3DTankDraw` ([`DrawFacts::motion`]:
//!   speed, turning, motive force, locomotor speed, direction) is read from
//!   the World in place, nothing written; upstream asks the object's physics
//!   and AI. A tank's render object is told apart from the last one by a serial
//!   ([`W3DModelDraw::render_obj_serial`]), not a pointer. The treads' UV
//!   offsets reach the drawn model as material overrides
//!   ([`RenderObj::material_overrides`], `SubObject::uv_offset`).
//! - (#41, kept) `W3DTankDraw::doDrawModule` returns before the base class's
//!   when there is no render object or no physics, so such a tank's states
//!   and animation pauses are not handled then.
//! - `handleWeaponFireFX` with a barrel equal to the barrel count (upstream
//!   tests `> size()`, then reads past the end: undefined) uses barrel 0.
//! - ~~A pristine-bone validation that found the model's files still being
//!   fetched (the browser) is valid all the same, with no bones, as upstream's
//!   is for a missing model; the module data's validation is once per mode.~~
//!   **Corrected 2026-11-06 (#42):** that left a template's bones empty for
//!   good in the browser. Now (new: upstream's reads never wait) a validation
//!   whose asset source answered "none for now"
//!   ([`W3dAssetSource::deferred_answers`]) leaves `PRISTINE_BONES_VALID`
//!   unset, finds no turret or barrel bones (not valid, as outside a logic
//!   time), and leaves the mode's validated bit unset, so a later validation
//!   at a logic time finds the bones once the bytes are there. A missing
//!   model is still valid with no bones, as upstream's; with the files at
//!   hand nothing changes.
//! - `Peek_Animation_And_Info` on an HLOD with no animation leaves its outputs
//!   uninitialised, which `getCurrentAnimFraction` then reads (undefined
//!   upstream); here the fraction is -1 (none) there.
//!
//! # Representation
//!
//! The module data is shared ([`Arc`]) by every drawable of a template, as
//! upstream's is; its caches (the match finder's, the validation bits, the
//! natural durations) are behind a [`Mutex`] or atomics, as upstream's are
//! `mutable`. A state is named by [`StateRef`]: an index into
//! `condition_states` or a transition's key pair, compared as upstream compares
//! the pointers. The assets come through [`W3dAssetSource`] (the asset
//! manager over the game's files, [`W3dFiles`], or a test's table).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::base_type::{real_to_int, real_to_int_floor};
use crate::common::file_system::{FileSystem, LocalDevice};
use crate::common::game_common::MSEC_PER_LOGICFRAME_REAL;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniError, IniErrorKind, IniLoadType, IniResult, MultiIniFieldParse, UserData};
use crate::common::model_state::{ModelConditionFlags, ModelConditionNames, MODELCONDITION_NIGHT, MODELCONDITION_SNOW};
use crate::common::random_value::RandomValues;
use crate::common::sparse_match_finder::{SparseMatchFinder, SparseMatchable};
use crate::common::thing_template::{ClientModuleData, CLIENT_NESTED_BLOCKS};
use crate::formats::w3d::assetmgr::{AssetManager, Prototype};
use crate::formats::w3d::render::{compute_current_frame, get_frame_rate, AnimMode, RenderModel};
use crate::logic::weapon_set::{SLOTS, WEAPON_SLOT_TYPE_NAMES};
use crate::formats::w3d::vertmaterial::MapperKind;
use crate::ww::matrix3d::Matrix3D;
use crate::client::w3d_truck_draw::TruckKind;

/// `MAX_TURRETS`.
pub const MAX_TURRETS: usize = 2;

/// `NO_NEXT_DURATION`.
pub const NO_NEXT_DURATION: u32 = 0xffff_ffff;

/// `TheAnimModeNames` (rendobj.h), in `RenderObjClass::AnimMode` order.
pub const THE_ANIM_MODE_NAMES: [&str; 6] = ["MANUAL", "LOOP", "ONCE", "LOOP_PINGPONG", "LOOP_BACKWARDS", "ONCE_BACKWARDS"];

/// `RenderObjClass::AnimMode` from its value (`ANIM_MODE_MANUAL = 0` ...).
pub fn anim_mode_from_index(i: i32) -> AnimMode {
    match i {
        0 => AnimMode::Manual,
        1 => AnimMode::Loop,
        2 => AnimMode::Once,
        3 => AnimMode::LoopPingpong,
        4 => AnimMode::LoopBackwards,
        _ => AnimMode::OnceBackwards,
    }
}

/// The value of a `RenderObjClass::AnimMode`.
pub fn anim_mode_index(m: AnimMode) -> i32 {
    match m {
        AnimMode::Manual => 0,
        AnimMode::Loop => 1,
        AnimMode::Once => 2,
        AnimMode::LoopPingpong => 3,
        AnimMode::LoopBackwards => 4,
        AnimMode::OnceBackwards => 5,
    }
}

/// `StaticGameLODNames` (GameLOD.cpp).
pub const STATIC_GAME_LOD_NAMES: [&str; 4] = ["Low", "Medium", "High", "Custom"];
/// `STATIC_GAME_LOD_LOW`, `_MEDIUM`, `_HIGH`.
pub const STATIC_GAME_LOD_LOW: i32 = 0;
pub const STATIC_GAME_LOD_MEDIUM: i32 = 1;
pub const STATIC_GAME_LOD_HIGH: i32 = 2;

// `enum ACBits` (the state's `Flags`).
pub const RANDOMIZE_START_FRAME: u32 = 0;
pub const START_FRAME_FIRST: u32 = 1;
pub const START_FRAME_LAST: u32 = 2;
pub const ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT: u32 = 3;
pub const PRISTINE_BONE_POS_IN_FINAL_FRAME: u32 = 4;
pub const MAINTAIN_FRAME_ACROSS_STATES: u32 = 5;
pub const RESTART_ANIM_WHEN_COMPLETE: u32 = 6;
pub const MAINTAIN_FRAME_ACROSS_STATES2: u32 = 7;
pub const MAINTAIN_FRAME_ACROSS_STATES3: u32 = 8;
pub const MAINTAIN_FRAME_ACROSS_STATES4: u32 = 9;

/// `ACBitsNames`.
pub const AC_BITS_NAMES: [&str; 10] = [
    "RANDOMSTART",
    "START_FRAME_FIRST",
    "START_FRAME_LAST",
    "ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT",
    "PRISTINE_BONE_POS_IN_FINAL_FRAME",
    "MAINTAIN_FRAME_ACROSS_STATES",
    "RESTART_ANIM_WHEN_COMPLETE",
    "MAINTAIN_FRAME_ACROSS_STATES2",
    "MAINTAIN_FRAME_ACROSS_STATES3",
    "MAINTAIN_FRAME_ACROSS_STATES4",
];

/// `ALL_MAINTAIN_FRAME_FLAGS`.
pub const ALL_MAINTAIN_FRAME_FLAGS: u32 =
    (1 << MAINTAIN_FRAME_ACROSS_STATES) | (1 << MAINTAIN_FRAME_ACROSS_STATES2) | (1 << MAINTAIN_FRAME_ACROSS_STATES3) | (1 << MAINTAIN_FRAME_ACROSS_STATES4);

/// `isAnyMaintainFrameFlagSet(flags)`.
pub fn is_any_maintain_frame_flag_set(flags: u32) -> bool {
    flags & ALL_MAINTAIN_FRAME_FLAGS != 0
}

/// `isCommonMaintainFrameFlagSet(a, b)`.
pub fn is_common_maintain_frame_flag_set(a: u32, b: u32) -> bool {
    (a & ALL_MAINTAIN_FRAME_FLAGS) & (b & ALL_MAINTAIN_FRAME_FLAGS) != 0
}

/// `testFlagBit(flags, bit)`.
pub fn test_flag_bit(flags: u32, bit: u32) -> bool {
    flags & (1 << bit) != 0
}

// `enum INIReadFlagsType`.
const ANIMS_COPIED_FROM_DEFAULT_STATE: u32 = 0;
const GOT_NONIDLE_ANIMS: u32 = 1;
const GOT_IDLE_ANIMS: u32 = 2;

// `ModelConditionInfo::m_validStuff` bits.
const PRISTINE_BONES_VALID: u8 = 0x0001;
const TURRETS_VALID: u8 = 0x0002;
const HAS_PROJECTILE_BONES: u8 = 0x0004;
const BARRELS_VALID: u8 = 0x0008;

/// `AsciiString::toLower()` (ASCII).
fn lower(s: &[u8]) -> Vec<u8> {
    s.to_ascii_lowercase()
}

/// `AsciiString::isNone()`: `stricmp(str, "None") == 0`.
fn is_none(s: &[u8]) -> bool {
    s.eq_ignore_ascii_case(b"None")
}

fn invalid() -> IniError {
    IniError::new(IniErrorKind::InvalidData)
}

// ---------------------------------------------------------------------------
// ModelConditionInfo
// ---------------------------------------------------------------------------

/// `W3DAnimationInfo`.
#[derive(Debug)]
pub struct W3DAnimationInfo {
    /// `m_name`, lower-cased.
    pub name: Vec<u8>,
    /// `m_distanceCovered`: if nonzero, the distance one loop covers.
    pub distance_covered: f32,
    /// `m_naturalDurationInMsec` (mutable upstream: -1 until the handle is
    /// first fetched), the bits of an `f32`.
    natural_duration_in_msec: AtomicU32,
    /// `m_isIdleAnim`.
    pub is_idle_anim: bool,
}

impl Clone for W3DAnimationInfo {
    fn clone(&self) -> Self {
        W3DAnimationInfo {
            name: self.name.clone(),
            distance_covered: self.distance_covered,
            natural_duration_in_msec: AtomicU32::new(self.natural_duration_in_msec.load(Ordering::Relaxed)),
            is_idle_anim: self.is_idle_anim,
        }
    }
}

impl PartialEq for W3DAnimationInfo {
    fn eq(&self, o: &Self) -> bool {
        self.name == o.name && self.distance_covered == o.distance_covered && self.is_idle_anim == o.is_idle_anim
    }
}

impl W3DAnimationInfo {
    /// `W3DAnimationInfo(name, isIdle, distanceCovered)` (`NO_RETAIN_ANIM_HANDLES`).
    pub fn new(name: Vec<u8>, is_idle: bool, distance_covered: f32) -> Self {
        W3DAnimationInfo { name, distance_covered, natural_duration_in_msec: AtomicU32::new((-1.0f32).to_bits()), is_idle_anim: is_idle }
    }

    /// `getNaturalDurationInMsec()`.
    pub fn get_natural_duration_in_msec(&self) -> f32 {
        f32::from_bits(self.natural_duration_in_msec.load(Ordering::Relaxed))
    }

    /// `getAnimHandle()`: the animation through the asset manager; the
    /// first time it is found, its natural duration is kept.
    pub fn get_anim_handle<A: W3dAssetSource>(&self, assets: &mut A) -> Option<AnimHandle> {
        let handle = assets.get_hanim(&self.name);
        if let Some(h) = &handle {
            if self.get_natural_duration_in_msec() < 0.0 {
                let d = h.num_frames as f32 * 1000.0 / h.frame_rate;
                self.natural_duration_in_msec.store(d.to_bits(), Ordering::Relaxed);
            }
        }
        handle
    }
}

/// `ModelConditionInfo::HideShowSubObjInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct HideShowSubObjInfo {
    pub sub_obj_name: Vec<u8>,
    pub hide: bool,
}

/// `ModelConditionInfo::TurretInfo`, its INI part.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TurretInfo {
    /// `m_turretAngleNameKey`: the bone's lower-cased name, empty for `NAMEKEY_INVALID`.
    pub turret_angle_name_key: Vec<u8>,
    /// `m_turretPitchNameKey`.
    pub turret_pitch_name_key: Vec<u8>,
    /// `m_turretArtAngle`, radians.
    pub turret_art_angle: f32,
    /// `m_turretArtPitch`, radians.
    pub turret_art_pitch: f32,
}

/// `ParticleSysBoneInfo` (the system's name in place of its template).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSysBoneInfo {
    pub bone_name: Vec<u8>,
    pub particle_system_template: Vec<u8>,
}

/// `PristineBoneInfo`: a bone's transform in the pristine pose and its index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PristineBoneInfo {
    pub mtx: Matrix3D,
    pub bone_index: i32,
}

/// `ModelConditionInfo::WeaponBarrelInfo` (`m_muzzleFlashBoneName` is debug-only upstream and left out).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponBarrelInfo {
    /// `m_recoilBone` (zero: no bone).
    pub recoil_bone: i32,
    /// `m_fxBone`.
    pub fx_bone: i32,
    /// `m_muzzleFlashBone`.
    pub muzzle_flash_bone: i32,
    /// `m_projectileOffsetMtx`.
    pub projectile_offset_mtx: Matrix3D,
}

impl Default for WeaponBarrelInfo {
    /// `WeaponBarrelInfo()`, which calls `clear()`.
    fn default() -> Self {
        WeaponBarrelInfo { recoil_bone: 0, fx_bone: 0, muzzle_flash_bone: 0, projectile_offset_mtx: Matrix3D::IDENTITY }
    }
}

impl WeaponBarrelInfo {
    /// `setMuzzleFlashHidden(fullObject, hide)`: the first sub-object on the muzzle-flash bone
    /// (`Get_Sub_Object_On_Bone(0, bone)`); none is upstream's asset-error debug crash, a no-op here.
    pub fn set_muzzle_flash_hidden(&self, full_object: Option<&mut RenderObj>, hide: bool) {
        if let Some(r) = full_object {
            if let Some(i) = r.get_sub_object_on_bone(0, self.muzzle_flash_bone) {
                r.set_sub_object_hidden(i, hide);
            }
        }
    }
}

/// The `mutable` bone caches of a `ModelConditionInfo` that `validateStuff` fills.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoneCache {
    /// `m_pristineBones`, by lower-cased name (`NAMEKEY` of it).
    pub pristine_bones: BTreeMap<Vec<u8>, PristineBoneInfo>,
    /// `m_turrets[i].m_turretAngleBone`, `.m_turretPitchBone`.
    pub turret_bones: [(i32, i32); MAX_TURRETS],
    /// `m_weaponBarrelInfoVec`.
    pub weapon_barrel_info_vec: [Vec<WeaponBarrelInfo>; SLOTS],
    /// `m_hasRecoilBonesOrMuzzleFlashes`.
    pub has_recoil_bones_or_muzzle_flashes: [bool; SLOTS],
}

/// The transition signature, `buildTransitionSig(src, dst)`: the two keys'
/// names. `NO_TRANSITION` is `None`.
pub type TransitionSig = (Vec<u8>, Vec<u8>);

/// `struct ModelConditionInfo`.
#[derive(Debug)]
pub struct ModelConditionInfo {
    /// `m_conditionsYesVec`.
    pub conditions_yes_vec: Vec<ModelConditionFlags>,
    /// `m_modelName`, lower-cased; empty for `None`.
    pub model_name: Vec<u8>,
    /// `m_hideShowVec`.
    pub hide_show_vec: Vec<HideShowSubObjInfo>,
    /// `m_publicBones` (those the parse adds).
    pub public_bones: Vec<Vec<u8>>,
    pub weapon_fire_fx_bone_name: [Vec<u8>; SLOTS],
    pub weapon_recoil_bone_name: [Vec<u8>; SLOTS],
    pub weapon_muzzle_flash_name: [Vec<u8>; SLOTS],
    pub weapon_projectile_launch_bone_name: [Vec<u8>; SLOTS],
    pub weapon_projectile_hide_show_name: [Vec<u8>; SLOTS],
    /// `m_animations`.
    pub animations: Vec<W3DAnimationInfo>,
    /// `m_transitionKey` (empty: `NAMEKEY_INVALID`).
    pub transition_key: Vec<u8>,
    /// `m_allowToFinishKey`.
    pub allow_to_finish_key: Vec<u8>,
    /// `m_flags` (`ACBits`).
    pub flags: u32,
    /// `m_iniReadFlags`.
    pub ini_read_flags: u32,
    /// `m_mode`.
    pub mode: AnimMode,
    /// `m_particleSysBones`.
    pub particle_sys_bones: Vec<ParticleSysBoneInfo>,
    /// `m_transitionSig`.
    pub transition_sig: Option<TransitionSig>,
    /// `m_animMinSpeedFactor`, `m_animMaxSpeedFactor`.
    pub anim_min_speed_factor: f32,
    pub anim_max_speed_factor: f32,
    /// `m_turrets` (the INI part).
    pub turrets: [TurretInfo; MAX_TURRETS],
    /// `m_validStuff` (mutable upstream).
    valid_stuff: AtomicU8,
    /// `m_pristineBones`, the turrets' bones, the barrels (mutable upstream).
    bones: Mutex<BoneCache>,
}

impl Clone for ModelConditionInfo {
    fn clone(&self) -> Self {
        ModelConditionInfo {
            conditions_yes_vec: self.conditions_yes_vec.clone(),
            model_name: self.model_name.clone(),
            hide_show_vec: self.hide_show_vec.clone(),
            public_bones: self.public_bones.clone(),
            weapon_fire_fx_bone_name: self.weapon_fire_fx_bone_name.clone(),
            weapon_recoil_bone_name: self.weapon_recoil_bone_name.clone(),
            weapon_muzzle_flash_name: self.weapon_muzzle_flash_name.clone(),
            weapon_projectile_launch_bone_name: self.weapon_projectile_launch_bone_name.clone(),
            weapon_projectile_hide_show_name: self.weapon_projectile_hide_show_name.clone(),
            animations: self.animations.clone(),
            transition_key: self.transition_key.clone(),
            allow_to_finish_key: self.allow_to_finish_key.clone(),
            flags: self.flags,
            ini_read_flags: self.ini_read_flags,
            mode: self.mode,
            particle_sys_bones: self.particle_sys_bones.clone(),
            transition_sig: self.transition_sig.clone(),
            anim_min_speed_factor: self.anim_min_speed_factor,
            anim_max_speed_factor: self.anim_max_speed_factor,
            turrets: self.turrets.clone(),
            valid_stuff: AtomicU8::new(self.valid_stuff.load(Ordering::Relaxed)),
            bones: Mutex::new(self.bone_cache()),
        }
    }
}

impl Default for ModelConditionInfo {
    /// `ModelConditionInfo()`, which calls `clear()`.
    fn default() -> Self {
        ModelConditionInfo {
            conditions_yes_vec: Vec::new(),
            model_name: Vec::new(),
            hide_show_vec: Vec::new(),
            public_bones: Vec::new(),
            weapon_fire_fx_bone_name: Default::default(),
            weapon_recoil_bone_name: Default::default(),
            weapon_muzzle_flash_name: Default::default(),
            weapon_projectile_launch_bone_name: Default::default(),
            weapon_projectile_hide_show_name: Default::default(),
            animations: Vec::new(),
            transition_key: Vec::new(),
            allow_to_finish_key: Vec::new(),
            flags: 0,
            ini_read_flags: 0,
            mode: AnimMode::Once,
            particle_sys_bones: Vec::new(),
            transition_sig: None,
            anim_min_speed_factor: 1.0,
            anim_max_speed_factor: 1.0,
            turrets: Default::default(),
            valid_stuff: AtomicU8::new(0),
            bones: Mutex::new(BoneCache::default()),
        }
    }
}

impl SparseMatchable<ModelConditionNames> for ModelConditionInfo {
    fn conditions_yes_count(&self) -> i32 {
        self.conditions_yes_vec.len() as i32
    }
    fn nth_conditions_yes(&self, i: i32) -> &ModelConditionFlags {
        &self.conditions_yes_vec[i as usize]
    }
}

impl ModelConditionInfo {
    /// `addPublicBone(boneName)`: lower-cased, once; not empty or `None`.
    pub fn add_public_bone(&mut self, bone_name: &[u8]) {
        if bone_name.is_empty() || is_none(bone_name) {
            return;
        }
        let tmp = lower(bone_name);
        if !self.public_bones.contains(&tmp) {
            self.public_bones.push(tmp);
        }
    }

    /// `matchesMode(night, snowy)`: one of the condition sets has exactly
    /// these `NIGHT` and `SNOW` bits.
    pub fn matches_mode(&self, night: bool, snowy: bool) -> bool {
        self.conditions_yes_vec.iter().any(|f| f.test(MODELCONDITION_NIGHT) == night && f.test(MODELCONDITION_SNOW) == snowy)
    }

    /// Whether the parse found a projectile launch bone (`HAS_PROJECTILE_BONES`).
    pub fn has_projectile_bones(&self) -> bool {
        self.valid_stuff.load(Ordering::Relaxed) & HAS_PROJECTILE_BONES != 0
    }

    /// Whether `validateTurretInfo` has run at a valid time (`TURRETS_VALID`).
    pub fn turrets_valid(&self) -> bool {
        self.valid_stuff.load(Ordering::Relaxed) & TURRETS_VALID != 0
    }

    /// Whether `validateWeaponBarrelInfo` has run at a valid time (`BARRELS_VALID`).
    pub fn barrels_valid(&self) -> bool {
        self.valid_stuff.load(Ordering::Relaxed) & BARRELS_VALID != 0
    }

    /// Whether `validateCachedBones` has run at a valid time (`PRISTINE_BONES_VALID`).
    pub fn pristine_bones_valid(&self) -> bool {
        self.valid_stuff.load(Ordering::Relaxed) & PRISTINE_BONES_VALID != 0
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, BoneCache> {
        self.bones.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A copy of the bone caches (`m_pristineBones`, the turrets' and barrels' bones).
    pub fn bone_cache(&self) -> BoneCache {
        self.cache().clone()
    }

    /// `m_turrets[t].m_turretAngleBone` and `.m_turretPitchBone`.
    pub fn turret_bones(&self, t: usize) -> (i32, i32) {
        self.cache().turret_bones[t]
    }

    /// `m_weaponBarrelInfoVec[wslot]`.
    pub fn weapon_barrels(&self, wslot: usize) -> Vec<WeaponBarrelInfo> {
        self.cache().weapon_barrel_info_vec[wslot].clone()
    }

    /// `m_hasRecoilBonesOrMuzzleFlashes[wslot]`.
    pub fn has_recoil_bones_or_muzzle_flashes(&self, wslot: usize) -> bool {
        self.cache().has_recoil_bones_or_muzzle_flashes[wslot]
    }

    /// `validateStuff(robj, scale, extraPublicBones)`: `loadAnimations` (nothing, without
    /// `RETAIN_ANIM_HANDLES`), the public bones, `validateCachedBones`, `validateTurretInfo`,
    /// `validateWeaponBarrelInfo`.
    ///
    /// Returns whether the pristine bones' files were still being fetched (GitHub #42, new: see
    /// [`ModelConditionInfo::validate_cached_bones`]); then the turrets and barrels are validated as
    /// outside a logic time (none found, not valid), so a later validation finds them.
    ///
    /// `addPublicBone` of the extra bones (once, at a valid time, `PUBLIC_BONES_VALID`) is not
    /// stored: the extras join the parsed list where the list is read (here only, when the
    /// pristine bones are cached), with `addPublicBone`'s rules; the map is by name, so the order
    /// does not matter.
    pub fn validate_stuff<A: W3dAssetSource>(
        &self,
        robj: Option<&mut RenderObj>,
        scale: f32,
        extra_public_bones: &[Vec<u8>],
        standard_public_bones: &[Vec<u8>],
        env: &mut DrawEnv<'_, A>,
    ) -> bool {
        let fetching = self.validate_cached_bones(robj, scale, extra_public_bones, standard_public_bones, env);
        // (GitHub #42, new) bones looked for in a pose whose files were still being fetched are not looked for:
        // the turrets and barrels stay not valid, as outside a logic time, and are found once the pose is
        let valid_time = env.logic_time && !fetching;
        self.validate_turret_info(valid_time);
        self.validate_weapon_barrel_info(valid_time);
        fetching
    }

    /// `validateCachedBones(robj, scale)`: once, at a valid time, the pristine bones: the
    /// global data's standard public bones and the state's public bones (each also numbered
    /// `01` up to `99`, until one is missing), found as bones (`Get_Bone_Index`, where index 0
    /// counts as not found) or else as sub-objects, with the render object posed at the first
    /// frame of the state's first animation (the last, under `PRISTINE_BONE_POS_IN_FINAL_FRAME`;
    /// without animations, the render object's own) under a transform of `scale`.
    ///
    /// The pose is the asset source's ([`W3dAssetSource::skeleton`]): a fresh render object of
    /// the model, so a bone the live one has captured is not applied (upstream poses the live
    /// render object when one is given). The live one's animation is left as upstream leaves
    /// it: set to the pose's animation in `MANUAL`, then back to the animation it had, if any.
    ///
    /// **Files still being fetched** (GitHub #42; new, not upstream's, whose reads never wait):
    /// when the asset source answered a question of this validation "none for now"
    /// ([`W3dAssetSource::deferred_answers`] moved: the model, its skeleton or the pose's animation),
    /// the bones are cleared and `PRISTINE_BONES_VALID` is left unset, so a later validation at a
    /// valid time looks again once the bytes are there; it returns `true` then. With the files at
    /// hand nothing is answered "for now" and this is upstream's function.
    pub fn validate_cached_bones<A: W3dAssetSource>(
        &self,
        robj: Option<&mut RenderObj>,
        scale: f32,
        extra_public_bones: &[Vec<u8>],
        standard_public_bones: &[Vec<u8>],
        env: &mut DrawEnv<'_, A>,
    ) -> bool {
        let before = env.assets.deferred_answers();
        self.validate_cached_bones_upstream(robj, scale, extra_public_bones, standard_public_bones, env);
        if env.assets.deferred_answers() == before {
            return false;
        }
        self.cache().pristine_bones.clear();
        self.valid_stuff.fetch_and(!PRISTINE_BONES_VALID, Ordering::Relaxed);
        true
    }

    /// `validateCachedBones` as upstream has it (see [`ModelConditionInfo::validate_cached_bones`]).
    fn validate_cached_bones_upstream<A: W3dAssetSource>(
        &self,
        robj: Option<&mut RenderObj>,
        scale: f32,
        extra_public_bones: &[Vec<u8>],
        standard_public_bones: &[Vec<u8>],
        env: &mut DrawEnv<'_, A>,
    ) {
        if self.pristine_bones_valid() {
            return;
        }
        if !env.logic_time {
            self.cache().pristine_bones.clear();
            self.valid_stuff.fetch_and(!PRISTINE_BONES_VALID, Ordering::Relaxed);
            return;
        }
        self.cache().pristine_bones.clear();
        // go ahead and set this here, in case we return early.
        self.valid_stuff.fetch_or(PRISTINE_BONES_VALID, Ordering::Relaxed);

        let (model_name, is_hlod, cur_anim) = match &robj {
            Some(r) => (r.model_name.clone(), r.is_hlod, if r.is_hlod { r.anim.clone() } else { None }),
            None => {
                if self.model_name.is_empty() {
                    return;
                }
                match env.assets.create_render_obj(&self.model_name) {
                    Some(hlod) => (self.model_name.clone(), hlod, None),
                    // "*** ASSET ERROR: Model %s not found!"
                    None => return,
                }
            }
        };

        // if we have any animations in this state, always choose the first, since the
        // animations vary on a per-client basis.
        let anim_to_use = if !self.animations.is_empty() { self.animations[0].get_anim_handle(env.assets) } else { cur_anim.as_ref().map(|a| a.motion.clone()) };
        let which_frame = anim_to_use.as_ref().map(|h| if test_flag_bit(self.flags, PRISTINE_BONE_POS_IN_FINAL_FRAME) { h.num_frames - 1 } else { 0 });
        // Set_Animation does nothing on a render object that is not an HLOD
        let pose = match (&anim_to_use, which_frame) {
            (Some(h), Some(f)) if is_hlod => Some((h.name.as_slice(), f as f32)),
            _ => None,
        };
        let skeleton = env.assets.skeleton(&model_name, pose, scale);

        let mut map = BTreeMap::new();
        for b in standard_public_bones {
            // don't crash here, since these are catch-all global bones and won't be present in most models.
            do_single_bone_name(skeleton.as_ref(), b, &mut map);
        }
        let mut public = self.public_bones.clone();
        for b in extra_public_bones {
            let tmp = lower(b);
            if !b.is_empty() && !is_none(b) && !public.contains(&tmp) {
                public.push(tmp);
            }
        }
        for b in &public {
            // DO crash here (upstream's debug build), since we specifically requested this bone for this model
            do_single_bone_name(skeleton.as_ref(), b, &mut map);
        }
        self.cache().pristine_bones = map;

        if let Some(r) = robj {
            if r.is_hlod {
                if let (Some(h), Some(f)) = (anim_to_use, which_frame) {
                    r.set_animation(Some(h), f as f32, AnimMode::Manual, env.sync_time);
                }
                if let Some(a) = cur_anim {
                    r.set_animation(Some(a.motion), a.frame, a.anim_mode, env.sync_time);
                    r.set_animation_frame_rate_multiplier(a.frame_rate_multiplier);
                }
            }
        }
    }

    /// `findPristineBone(boneName, &boneIndex)`: `None` (bone index 0) when the bones are not
    /// valid, the name is `NAMEKEY_INVALID` (empty), or it is not cached.
    pub fn find_pristine_bone(&self, bone_name: &[u8]) -> Option<PristineBoneInfo> {
        if !self.pristine_bones_valid() || bone_name.is_empty() {
            return None;
        }
        self.cache().pristine_bones.get(bone_name).copied()
    }

    /// `validateTurretInfo()`: once, at a valid time, each turret's angle and pitch bone by its
    /// key through the pristine bones (0 when it has none, or it is not found: upstream's
    /// asset-error debug crash). Outside a valid time, or without a model, the bones are 0 and
    /// the turrets stay not valid.
    pub fn validate_turret_info(&self, logic_time: bool) {
        if self.turrets_valid() {
            return;
        }
        for tslot in 0..MAX_TURRETS {
            let tur = &self.turrets[tslot];
            let bones = if !logic_time || self.model_name.is_empty() {
                (0, 0)
            } else {
                let find = |key: &[u8]| if key.is_empty() { 0 } else { self.find_pristine_bone(key).map_or(0, |b| b.bone_index) };
                (find(&tur.turret_angle_name_key), find(&tur.turret_pitch_name_key))
            };
            self.cache().turret_bones[tslot] = bones;
        }
        if logic_time {
            self.valid_stuff.fetch_or(TURRETS_VALID, Ordering::Relaxed);
        }
    }

    /// `validateWeaponBarrelInfo()`: once, at a valid time, each weapon slot's barrels: for
    /// `i` from 1 to 99 the bones `<name>NN` of the recoil, muzzle-flash, fire-FX and launch
    /// names (a barrel with a muzzle flash but no FX bone of its own uses the previous FX
    /// bone), until a number finds none of them; when no numbered barrel is found, the
    /// unadorned names make one barrel if any is found.
    pub fn validate_weapon_barrel_info(&self, logic_time: bool) {
        if self.barrels_valid() || !logic_time {
            return;
        }
        let bone = |name: &[u8]| self.find_pristine_bone(name).map_or(0, |b| b.bone_index);
        let numbered = |name: &[u8], i: i32| {
            let mut n = name.to_vec();
            n.extend_from_slice(format!("{i:02}").as_bytes());
            n
        };
        let mut vecs: [Vec<WeaponBarrelInfo>; SLOTS] = Default::default();
        let mut has = [false; SLOTS];
        for wslot in 0..SLOTS {
            let fx_bone_name = &self.weapon_fire_fx_bone_name[wslot];
            let recoil_bone_name = &self.weapon_recoil_bone_name[wslot];
            let mf_name = &self.weapon_muzzle_flash_name[wslot];
            let plb_name = &self.weapon_projectile_launch_bone_name[wslot];
            if fx_bone_name.is_empty() && recoil_bone_name.is_empty() && mf_name.is_empty() && plb_name.is_empty() {
                continue;
            }
            let mut prev_fx_bone = 0;
            for i in 1..=99 {
                let mut info = WeaponBarrelInfo::default();
                if !recoil_bone_name.is_empty() {
                    info.recoil_bone = bone(&numbered(recoil_bone_name, i));
                }
                if !mf_name.is_empty() {
                    info.muzzle_flash_bone = bone(&numbered(mf_name, i));
                }
                if !fx_bone_name.is_empty() {
                    info.fx_bone = bone(&numbered(fx_bone_name, i));
                    // special case: if we have multiple muzzleflashes, but only one fxbone, use that fxbone for everything.
                    if info.fx_bone == 0 && info.muzzle_flash_bone != 0 {
                        info.fx_bone = prev_fx_bone;
                    }
                }
                let mut plb_bone_index = 0;
                if !plb_name.is_empty() {
                    if let Some(b) = self.find_pristine_bone(&numbered(plb_name, i)) {
                        plb_bone_index = b.bone_index;
                        info.projectile_offset_mtx = b.mtx;
                    }
                }
                if info.fx_bone == 0 && info.recoil_bone == 0 && info.muzzle_flash_bone == 0 && plb_bone_index == 0 {
                    break;
                }
                vecs[wslot].push(info);
                if info.recoil_bone != 0 || info.muzzle_flash_bone != 0 {
                    has[wslot] = true;
                }
                prev_fx_bone = info.fx_bone;
            }
            if vecs[wslot].is_empty() {
                // try the unadorned names
                let mut info = WeaponBarrelInfo::default();
                if !recoil_bone_name.is_empty() {
                    info.recoil_bone = bone(recoil_bone_name);
                }
                if !mf_name.is_empty() {
                    info.muzzle_flash_bone = bone(mf_name);
                }
                let plb = if plb_name.is_empty() { None } else { self.find_pristine_bone(plb_name) };
                info.projectile_offset_mtx = plb.map_or(Matrix3D::IDENTITY, |b| b.mtx);
                if !fx_bone_name.is_empty() {
                    info.fx_bone = bone(fx_bone_name);
                }
                if info.fx_bone != 0 || info.recoil_bone != 0 || info.muzzle_flash_bone != 0 || plb.is_some() {
                    vecs[wslot].push(info);
                    if info.recoil_bone != 0 || info.muzzle_flash_bone != 0 {
                        has[wslot] = true;
                    }
                }
            }
        }
        {
            let mut c = self.cache();
            c.weapon_barrel_info_vec = vecs;
            c.has_recoil_bones_or_muzzle_flashes = has;
        }
        self.valid_stuff.fetch_or(BARRELS_VALID, Ordering::Relaxed);
    }
}

/// `findSingleBone(robj, boneName, mtx, boneIndex)`: the bone by name, unless its index is 0.
fn find_single_bone(sk: Option<&Skeleton>, bone_name: &[u8]) -> Option<PristineBoneInfo> {
    if is_none(bone_name) || bone_name.is_empty() {
        return None;
    }
    let sk = sk?;
    let bone_index = sk.get_bone_index(bone_name);
    if bone_index != 0 {
        Some(PristineBoneInfo { mtx: sk.bones[bone_index as usize].transform, bone_index })
    } else {
        None
    }
}

/// `findSingleSubObj(robj, boneName, mtx, boneIndex)`: the sub-object by name, its transform
/// and its bone.
fn find_single_sub_obj(sk: Option<&Skeleton>, bone_name: &[u8]) -> Option<PristineBoneInfo> {
    if is_none(bone_name) || bone_name.is_empty() {
        return None;
    }
    let sk = sk?;
    let i = sk.get_sub_object_by_name(bone_name)?;
    let sub = &sk.sub_objects[i];
    Some(PristineBoneInfo { mtx: sub.transform, bone_index: sub.bone_index })
}

/// `doSingleBoneName(robj, boneName, map)`: the bone and its numbered variants as bones, or
/// else as sub-objects, into the map by lower-cased name.
fn do_single_bone_name(sk: Option<&Skeleton>, bone_name: &[u8], map: &mut BTreeMap<Vec<u8>, PristineBoneInfo>) -> bool {
    let mut found_as_bone = false;
    let mut found_as_sub_obj = false;
    let tmp_name = lower(bone_name);
    let numbered = |i: i32| {
        let mut n = tmp_name.clone();
        n.extend_from_slice(format!("{i:02}").as_bytes());
        n
    };
    if let Some(info) = find_single_bone(sk, &tmp_name) {
        map.insert(tmp_name.clone(), info);
        found_as_bone = true;
    }
    for i in 1..=99 {
        let tmp = numbered(i);
        match find_single_bone(sk, &tmp) {
            Some(info) => {
                map.insert(tmp, info);
                found_as_bone = true;
            }
            None => break,
        }
    }
    if !found_as_bone {
        if let Some(info) = find_single_sub_obj(sk, &tmp_name) {
            map.insert(tmp_name.clone(), info);
            found_as_sub_obj = true;
        }
        for i in 1..=99 {
            let tmp = numbered(i);
            match find_single_sub_obj(sk, &tmp) {
                Some(info) => {
                    map.insert(tmp, info);
                    found_as_sub_obj = true;
                }
                None => break,
            }
        }
    }
    found_as_bone || found_as_sub_obj
}

// ---------------------------------------------------------------------------
// The module data and its parse
// ---------------------------------------------------------------------------

/// The draw modules whose data is a `W3DModelDrawModuleData`, and what each
/// subclass adds to it (~~parsed, not used yet~~ **Corrected 2026-11-06 (#41):** the tank's and the trucks' are used by
/// their draws, `W3DModelDraw::tank` and `W3DModelDraw::truck`; the supply, dependency and science fields are parsed,
/// not used yet).
#[derive(Clone, Debug, PartialEq)]
pub enum DrawModuleExtra {
    /// `W3DModelDraw`, `W3DOverlordAircraftDraw`: nothing.
    None,
    /// `W3DTankDrawModuleData` (`W3DTankDraw`, `W3DOverlordTankDraw`).
    Tank(TankFields),
    /// `W3DTruckDrawModuleData` (`W3DTruckDraw`, `W3DOverlordTruckDraw`, `W3DPoliceCarDraw`).
    Truck(TruckFields),
    /// `W3DTankTruckDrawModuleData`.
    TankTruck(TruckFields, TankFields),
    /// `W3DSupplyDrawModuleData`: `m_supplyBonePrefix`.
    Supply { supply_bone_prefix: Vec<u8> },
    /// `W3DDependencyModelDrawModuleData`: `m_attachToDrawableBoneInContainer`.
    Dependency { attach_to_drawable_bone_in_container: Vec<u8> },
    /// `W3DScienceModelDrawModuleData`: `m_requiredScience`, by name.
    Science { required_science: Vec<u8> },
}

/// The tread fields of `W3DTankDrawModuleData` (and `W3DTankTruckDrawModuleData`).
#[derive(Clone, Debug, PartialEq)]
pub struct TankFields {
    pub tread_debris_name_left: Vec<u8>,
    pub tread_debris_name_right: Vec<u8>,
    /// `m_treadAnimationRate`, per frame.
    pub tread_animation_rate: f32,
    pub tread_pivot_speed_fraction: f32,
    pub tread_drive_speed_fraction: f32,
}

impl Default for TankFields {
    /// `W3DTankDrawModuleData()`.
    fn default() -> Self {
        TankFields {
            tread_debris_name_left: b"TrackDebrisDirtLeft".to_vec(),
            tread_debris_name_right: b"TrackDebrisDirtRight".to_vec(),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
        }
    }
}

/// The fields of `W3DTruckDrawModuleData` (the tank truck has all but the cab
/// and trailer ones), as strings and reals. The names are the INI's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TruckFields {
    /// String fields by INI name (`Dust`, `LeftFrontTireBone`, ...).
    pub strings: BTreeMap<&'static str, Vec<u8>>,
    /// Real fields by INI name (`TireRotationMultiplier`, ...).
    pub reals: BTreeMap<&'static str, f32>,
}

/// `W3DTruckDrawModuleData`'s string fields.
const TRUCK_STRINGS: [&str; 15] = [
    "Dust",
    "DirtSpray",
    "PowerslideSpray",
    "LeftFrontTireBone",
    "RightFrontTireBone",
    "LeftRearTireBone",
    "RightRearTireBone",
    "MidLeftFrontTireBone",
    "MidRightFrontTireBone",
    "MidLeftRearTireBone",
    "MidRightRearTireBone",
    "MidLeftMidTireBone",
    "MidRightMidTireBone",
    "CabBone",
    "TrailerBone",
];
/// `W3DTruckDrawModuleData`'s real fields.
const TRUCK_REALS: [&str; 5] = ["TireRotationMultiplier", "PowerslideRotationAddition", "CabRotationMultiplier", "TrailerRotationMultiplier", "RotationDamping"];
/// `W3DTankTruckDrawModuleData`'s string fields (no mid-mid tires, cab or trailer).
const TANK_TRUCK_STRINGS: [&str; 11] = [
    "Dust",
    "DirtSpray",
    "PowerslideSpray",
    "LeftFrontTireBone",
    "RightFrontTireBone",
    "LeftRearTireBone",
    "RightRearTireBone",
    "MidLeftFrontTireBone",
    "MidRightFrontTireBone",
    "MidLeftRearTireBone",
    "MidRightRearTireBone",
];
const TANK_TRUCK_REALS: [&str; 2] = ["TireRotationMultiplier", "PowerslideRotationAddition"];

/// A state of the module data: `&m_conditionStates[i]`, or the transition
/// map's entry for a signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateRef {
    Normal(usize),
    Transition(TransitionSig),
}

/// `class W3DModelDrawModuleData`.
#[derive(Debug)]
pub struct W3DModelDrawModuleData {
    /// The module's class name (`W3DModelDraw`, `W3DTankDraw`, ...).
    pub module_name: Vec<u8>,
    /// `getModuleTagNameKey()`'s name (the `Draw` line's tag, `ModuleTag_01`): what `Drawable::xferDrawableModules` writes
    /// and finds the module by (GitHub #59). Empty when the data was not made from a template's module list.
    pub module_tag: Vec<u8>,
    /// `m_conditionStates`.
    pub condition_states: Vec<ModelConditionInfo>,
    /// `m_conditionStateMap` (mutable upstream).
    condition_state_map: Mutex<SparseMatchFinder>,
    /// `m_transitionMap`.
    pub transition_map: BTreeMap<TransitionSig, ModelConditionInfo>,
    /// `m_extraPublicBones`.
    pub extra_public_bones: Vec<Vec<u8>>,
    /// `m_trackFile`, lower-cased.
    pub track_file: Vec<u8>,
    /// `m_attachToDrawableBone`, lower-cased.
    pub attach_to_drawable_bone: Vec<u8>,
    /// `m_defaultState`: the default state's index, or -1.
    pub default_state: i32,
    /// `m_projectileBoneFeedbackEnabledSlots`.
    pub projectile_bone_feedback_enabled_slots: u32,
    /// `m_initialRecoil` (per frame), `m_maxRecoil`, `m_recoilDamping`, `m_recoilSettle` (per frame).
    pub initial_recoil: f32,
    pub max_recoil: f32,
    pub recoil_damping: f32,
    pub recoil_settle: f32,
    /// `m_minLODRequired`.
    pub min_lod_required: i32,
    /// `m_ignoreConditionStates`.
    pub ignore_condition_states: ModelConditionFlags,
    /// `m_okToChangeModelColor`.
    pub ok_to_change_model_color: bool,
    /// `m_animationsRequirePower`.
    pub animations_require_power: bool,
    /// `m_validated` (mutable upstream).
    validated: AtomicU8,
    /// `m_particlesAttachedToAnimatedBones`.
    pub particles_attached_to_animated_bones: bool,
    /// `m_receivesDynamicLights`.
    pub receives_dynamic_lights: bool,
    /// What the subclass's data adds.
    pub extra: DrawModuleExtra,
}

impl W3DModelDrawModuleData {
    /// `W3DModelDrawModuleData()` (and the subclass's constructor, for `extra`).
    pub fn new(module_name: &[u8], extra: DrawModuleExtra) -> Self {
        W3DModelDrawModuleData {
            module_name: module_name.to_vec(),
            module_tag: Vec::new(),
            condition_states: Vec::new(),
            condition_state_map: Mutex::new(SparseMatchFinder::new()),
            transition_map: BTreeMap::new(),
            extra_public_bones: Vec::new(),
            track_file: Vec::new(),
            attach_to_drawable_bone: Vec::new(),
            default_state: -1,
            projectile_bone_feedback_enabled_slots: 0,
            initial_recoil: 2.0,
            max_recoil: 3.0,
            recoil_damping: 0.4,
            recoil_settle: 0.065,
            min_lod_required: STATIC_GAME_LOD_LOW,
            ignore_condition_states: ModelConditionFlags::new(),
            ok_to_change_model_color: false,
            animations_require_power: true,
            validated: AtomicU8::new(0),
            particles_attached_to_animated_bones: false,
            receives_dynamic_lights: true,
            extra,
        }
    }

    /// The module data's `extra` for a draw module's class name, or `None`
    /// when its data is not a `W3DModelDrawModuleData` (`W3DDefaultDraw`,
    /// `W3DTreeDraw`, `W3DLaserDraw`, ...).
    pub fn extra_for(module_name: &[u8]) -> Option<DrawModuleExtra> {
        Some(match module_name {
            b"W3DModelDraw" | b"W3DOverlordAircraftDraw" => DrawModuleExtra::None,
            b"W3DTankDraw" | b"W3DOverlordTankDraw" => DrawModuleExtra::Tank(TankFields::default()),
            b"W3DTruckDraw" | b"W3DOverlordTruckDraw" | b"W3DPoliceCarDraw" => DrawModuleExtra::Truck(TruckFields::default()),
            b"W3DTankTruckDraw" => DrawModuleExtra::TankTruck(TruckFields::default(), TankFields::default()),
            b"W3DSupplyDraw" => DrawModuleExtra::Supply { supply_bone_prefix: Vec::new() },
            b"W3DDependencyModelDraw" => DrawModuleExtra::Dependency { attach_to_drawable_bone_in_container: Vec::new() },
            b"W3DScienceModelDraw" => DrawModuleExtra::Science { required_science: Vec::new() },
            _ => return None,
        })
    }

    /// The module data of a `Draw` block the logic kept as written
    /// ([`ClientModuleData`]): `newModuleDataFromINI` for the module's class,
    /// i.e. its `buildFieldParse` tables over the block. `Ok(None)` for a
    /// module whose data is not a `W3DModelDrawModuleData`.
    ///
    /// The logic keeps the block's lines as tokens (it reads none of them);
    /// they are written back as INI text, the nested blocks closed with `End`
    /// where their lines end, and read with upstream's tables by the
    /// translated INI reader. A token never holds a separator, so the text
    /// reads back to the same tokens.
    pub fn parse(module_name: &[u8], block: &ClientModuleData) -> IniResult<Option<W3DModelDrawModuleData>> {
        let Some(extra) = Self::extra_for(module_name) else { return Ok(None) };
        let text = client_block_text(block);
        let mut data = W3DModelDrawModuleData::new(module_name, extra);
        let blocks = [BlockParse { token: "Draw", parse: parse_draw_block }];
        Ini::load(b"Draw", &text, IniLoadType::Overwrite, None, &blocks, &mut data)?;
        Ok(Some(data))
    }

    /// `findBestInfo(c)`: the state for the flags `c` with the ignored bits
    /// cleared (the [`SparseMatchFinder`]'s choice, cached).
    pub fn find_best_info(&self, c: &ModelConditionFlags) -> Option<usize> {
        let mut bits = *c;
        bits.clear_mask(&self.ignore_condition_states);
        let mut finder = self.condition_state_map.lock().unwrap_or_else(|e| e.into_inner());
        finder.find_best_info(&self.condition_states, &bits)
    }

    /// `getBestModelNameForWB(c)`.
    pub fn get_best_model_name_for_wb(&self, c: &ModelConditionFlags) -> Vec<u8> {
        self.find_best_info(c).map(|i| self.condition_states[i].model_name.clone()).unwrap_or_default()
    }

    /// The state a [`StateRef`] names.
    pub fn state(&self, s: &StateRef) -> &ModelConditionInfo {
        match s {
            StateRef::Normal(i) => &self.condition_states[*i],
            StateRef::Transition(sig) => &self.transition_map[sig],
        }
    }

    /// `validateStuffForTimeAndWeather(draw, night, snowy)`: once per mode
    /// at a valid time, validate the states of the plain mode or of this one,
    /// and the transitions with a source and a destination in it.
    ///
    /// When a state's validation found its files still being fetched (GitHub #42; new, not
    /// upstream's, whose reads never wait), the mode's validated bit is cleared again after the
    /// loop, so the next validation of the mode at a valid time (the next drawable of the template,
    /// or this one made again when the files arrive) validates the states still not valid. With
    /// the files at hand, upstream's function.
    pub fn validate_stuff_for_time_and_weather<A: W3dAssetSource>(
        &self,
        night: bool,
        snowy: bool,
        scale: f32,
        standard_public_bones: &[Vec<u8>],
        env: &mut DrawEnv<'_, A>,
    ) {
        if !env.logic_time {
            return;
        }
        let mode: u8 = match (night, snowy) {
            (true, true) => 0x8,
            (true, false) => 0x2,
            (false, true) => 0x4,
            (false, false) => 0x1,
        };
        if self.validated.load(Ordering::Relaxed) & mode != 0 {
            return;
        }
        self.validated.fetch_or(mode, Ordering::Relaxed);
        let mut fetching = false;
        for c in &self.condition_states {
            if !c.matches_mode(false, false) && !c.matches_mode(night, snowy) {
                continue;
            }
            fetching |= c.validate_stuff(None, scale, &self.extra_public_bones, standard_public_bones, env);
        }
        for ((src, dst), t) in &self.transition_map {
            let a = self.condition_states.iter().any(|c| &c.transition_key == src && c.matches_mode(night, snowy));
            let b = self.condition_states.iter().any(|c| &c.transition_key == dst && c.matches_mode(night, snowy));
            if a && b {
                // (nope -- transition states don't get public bones; they are validated all the same)
                fetching |= t.validate_stuff(None, scale, &self.extra_public_bones, standard_public_bones, env);
            }
        }
        if fetching {
            // (GitHub #42, new) not validated for this mode until every state's files were at hand
            self.validated.fetch_and(!mode, Ordering::Relaxed);
        }
    }

    /// Whether `validateStuffForTimeAndWeather` has validated this mode (`m_validated`'s bit).
    pub fn validated_for(&self, night: bool, snowy: bool) -> bool {
        let mode: u8 = match (night, snowy) {
            (true, true) => 0x8,
            (true, false) => 0x2,
            (false, true) => 0x4,
            (false, false) => 0x1,
        };
        self.validated.load(Ordering::Relaxed) & mode != 0
    }
}

/// A client module block as INI text: each line `Field arg arg`, each nested
/// block closed by `End` before the first line at its opener's depth or
/// shallower, the module's own `End` last.
pub fn client_block_text(block: &ClientModuleData) -> Vec<u8> {
    let mut out = b"Draw\n".to_vec();
    let mut open: Vec<u32> = Vec::new();
    for l in &block.lines {
        while open.last().is_some_and(|&d| d >= l.depth) {
            open.pop();
            out.extend_from_slice(b"End\n");
        }
        out.extend_from_slice(&l.field);
        for a in &l.args {
            out.push(b' ');
            out.extend_from_slice(a);
        }
        out.push(b'\n');
        if CLIENT_NESTED_BLOCKS.iter().any(|n| n.as_bytes().eq_ignore_ascii_case(&l.field)) {
            open.push(l.depth);
        }
    }
    for _ in open {
        out.extend_from_slice(b"End\n");
    }
    out.extend_from_slice(b"End\n");
    out
}

fn parse_draw_block(ini: &mut Ini<'_>, d: &mut W3DModelDrawModuleData) -> IniResult<()> {
    let base = model_draw_fields();
    let tank = tank_fields();
    let truck = truck_fields();
    let tank_truck = tank_truck_fields();
    let other = other_fields();
    let mut p = MultiIniFieldParse::new();
    p.add(&base)?;
    match d.extra {
        DrawModuleExtra::None => {}
        DrawModuleExtra::Tank(_) => p.add(&tank)?,
        DrawModuleExtra::Truck(_) => p.add(&truck)?,
        DrawModuleExtra::TankTruck(..) => p.add(&tank_truck)?,
        DrawModuleExtra::Supply { .. } | DrawModuleExtra::Dependency { .. } | DrawModuleExtra::Science { .. } => p.add(&other)?,
    }
    ini.init_from_ini_multi(d, &p)
}

/// `parseAsciiStringLC`.
fn parse_ascii_string_lc(ini: &mut Ini<'_>) -> Vec<u8> {
    lower(&ini.get_next_ascii_string())
}

/// `INI::parseStaticGameLODLevel`.
fn parse_static_game_lod_level(ini: &mut Ini<'_>) -> IniResult<i32> {
    let tok = ini.get_next_token(None)?;
    for (i, n) in STATIC_GAME_LOD_NAMES.iter().enumerate() {
        if tok.eq_ignore_ascii_case(n.as_bytes()) {
            return Ok(i as i32);
        }
    }
    Err(invalid())
}

type DataField = FieldParse<W3DModelDrawModuleData>;

/// `W3DModelDrawModuleData::buildFieldParse`'s table (`ModuleData`'s adds nothing).
fn model_draw_fields() -> Vec<DataField> {
    vec![
        FieldParse::new("InitialRecoilSpeed", |i, d, _| { d.initial_recoil = i.parse_velocity_real()?; Ok(()) }, UserData::None),
        FieldParse::new("MaxRecoilDistance", |i, d, _| { d.max_recoil = i.parse_real()?; Ok(()) }, UserData::None),
        FieldParse::new("RecoilDamping", |i, d, _| { d.recoil_damping = i.parse_real()?; Ok(()) }, UserData::None),
        FieldParse::new("RecoilSettleSpeed", |i, d, _| { d.recoil_settle = i.parse_velocity_real()?; Ok(()) }, UserData::None),
        FieldParse::new("OkToChangeModelColor", |i, d, _| { d.ok_to_change_model_color = i.parse_bool()?; Ok(()) }, UserData::None),
        FieldParse::new("AnimationsRequirePower", |i, d, _| { d.animations_require_power = i.parse_bool()?; Ok(()) }, UserData::None),
        FieldParse::new("ParticlesAttachedToAnimatedBones", |i, d, _| { d.particles_attached_to_animated_bones = i.parse_bool()?; Ok(()) }, UserData::None),
        FieldParse::new("MinLODRequired", |i, d, _| { d.min_lod_required = parse_static_game_lod_level(i)?; Ok(()) }, UserData::None),
        FieldParse::new(
            "ProjectileBoneFeedbackEnabledSlots",
            |i, d, _| i.parse_bit_string32(&mut d.projectile_bone_feedback_enabled_slots, &WEAPON_SLOT_TYPE_NAMES),
            UserData::None,
        ),
        FieldParse::new("DefaultConditionState", parse_condition_state, UserData::Mask(PARSE_DEFAULT)),
        FieldParse::new("ConditionState", parse_condition_state, UserData::Mask(PARSE_NORMAL)),
        FieldParse::new("AliasConditionState", parse_condition_state, UserData::Mask(PARSE_ALIAS)),
        FieldParse::new("TransitionState", parse_condition_state, UserData::Mask(PARSE_TRANSITION)),
        FieldParse::new("TrackMarks", |i, d, _| { d.track_file = parse_ascii_string_lc(i); Ok(()) }, UserData::None),
        FieldParse::new("ExtraPublicBone", |i, d, _| i.parse_ascii_string_vector_append(&mut d.extra_public_bones), UserData::None),
        FieldParse::new("AttachToBoneInAnotherModule", |i, d, _| { d.attach_to_drawable_bone = parse_ascii_string_lc(i); Ok(()) }, UserData::None),
        FieldParse::new("IgnoreConditionStates", |i, d, _| d.ignore_condition_states.parse_from_ini(i), UserData::None),
        FieldParse::new("ReceivesDynamicLights", |i, d, _| { d.receives_dynamic_lights = i.parse_bool()?; Ok(()) }, UserData::None),
        FieldParse::end(),
    ]
}

fn tank_of(d: &mut W3DModelDrawModuleData) -> &mut TankFields {
    match &mut d.extra {
        DrawModuleExtra::Tank(t) | DrawModuleExtra::TankTruck(_, t) => t,
        _ => unreachable!("a tank table is only added to a tank's data"),
    }
}

fn truck_of(d: &mut W3DModelDrawModuleData) -> &mut TruckFields {
    match &mut d.extra {
        DrawModuleExtra::Truck(t) | DrawModuleExtra::TankTruck(t, _) => t,
        _ => unreachable!("a truck table is only added to a truck's data"),
    }
}

/// `W3DTankDrawModuleData::buildFieldParse`'s own entries.
fn tank_entries() -> Vec<DataField> {
    vec![
        FieldParse::new("TreadDebrisLeft", |i, d, _| { tank_of(d).tread_debris_name_left = i.parse_ascii_string()?; Ok(()) }, UserData::None),
        FieldParse::new("TreadDebrisRight", |i, d, _| { tank_of(d).tread_debris_name_right = i.parse_ascii_string()?; Ok(()) }, UserData::None),
        FieldParse::new("TreadAnimationRate", |i, d, _| { tank_of(d).tread_animation_rate = i.parse_velocity_real()?; Ok(()) }, UserData::None),
        FieldParse::new("TreadPivotSpeedFraction", |i, d, _| { tank_of(d).tread_pivot_speed_fraction = i.parse_real()?; Ok(()) }, UserData::None),
        FieldParse::new("TreadDriveSpeedFraction", |i, d, _| { tank_of(d).tread_drive_speed_fraction = i.parse_real()?; Ok(()) }, UserData::None),
    ]
}

fn truck_string(i: &mut Ini<'_>, d: &mut W3DModelDrawModuleData, ud: &UserData) -> IniResult<()> {
    let name = ud.names()[0];
    let v = i.parse_ascii_string()?;
    truck_of(d).strings.insert(name, v);
    Ok(())
}

fn truck_real(i: &mut Ini<'_>, d: &mut W3DModelDrawModuleData, ud: &UserData) -> IniResult<()> {
    let name = ud.names()[0];
    let v = i.parse_real()?;
    truck_of(d).reals.insert(name, v);
    Ok(())
}

/// One `FieldParse` per name, each storing under its own name.
fn named_entries(strings: &'static [&'static str], reals: &'static [&'static str]) -> Vec<DataField> {
    let mut v = Vec::new();
    for k in 0..strings.len() {
        v.push(FieldParse::new(strings[k], truck_string, UserData::Names(&strings[k..k + 1])));
    }
    for k in 0..reals.len() {
        v.push(FieldParse::new(reals[k], truck_real, UserData::Names(&reals[k..k + 1])));
    }
    v
}

/// `W3DTankDrawModuleData::buildFieldParse`'s own table.
fn tank_fields() -> Vec<DataField> {
    let mut v = tank_entries();
    v.push(FieldParse::end());
    v
}

/// `W3DTruckDrawModuleData::buildFieldParse`'s own table.
fn truck_fields() -> Vec<DataField> {
    let mut v = named_entries(&TRUCK_STRINGS, &TRUCK_REALS);
    v.push(FieldParse::end());
    v
}

/// `W3DTankTruckDrawModuleData::buildFieldParse`'s own table.
fn tank_truck_fields() -> Vec<DataField> {
    let mut v = named_entries(&TANK_TRUCK_STRINGS, &TANK_TRUCK_REALS);
    v.extend(tank_entries());
    v.push(FieldParse::end());
    v
}

/// The supply, dependency and science draws' one field each (only the
/// matching one stores; the others are not in that module's table, so they
/// refuse).
fn other_fields() -> Vec<DataField> {
    vec![
        FieldParse::new(
            "SupplyBonePrefix",
            |i, d, _| match &mut d.extra {
                DrawModuleExtra::Supply { supply_bone_prefix } => {
                    *supply_bone_prefix = i.parse_ascii_string()?;
                    Ok(())
                }
                _ => Err(IniError::new(IniErrorKind::UnknownToken)),
            },
            UserData::None,
        ),
        FieldParse::new(
            "AttachToBoneInContainer",
            |i, d, _| match &mut d.extra {
                DrawModuleExtra::Dependency { attach_to_drawable_bone_in_container } => {
                    *attach_to_drawable_bone_in_container = i.parse_ascii_string()?;
                    Ok(())
                }
                _ => Err(IniError::new(IniErrorKind::UnknownToken)),
            },
            UserData::None,
        ),
        FieldParse::new(
            "RequiredScience",
            |i, d, _| match &mut d.extra {
                DrawModuleExtra::Science { required_science } => {
                    *required_science = i.get_next_token(None)?;
                    Ok(())
                }
                _ => Err(IniError::new(IniErrorKind::UnknownToken)),
            },
            UserData::None,
        ),
        FieldParse::end(),
    ]
}

// `enum ParseCondStateType`.
const PARSE_NORMAL: u32 = 0;
const PARSE_DEFAULT: u32 = 1;
const PARSE_TRANSITION: u32 = 2;
const PARSE_ALIAS: u32 = 3;

/// `doesStateExist(v, f)`.
fn does_state_exist(v: &[ModelConditionInfo], f: &ModelConditionFlags) -> bool {
    v.iter().any(|it| it.conditions_yes_vec.iter().rev().any(|c| c == f))
}

/// `parseAnimation` (`ANIM_NORMAL` or `ANIM_IDLE`).
fn parse_animation(ini: &mut Ini<'_>, s: &mut ModelConditionInfo, ud: &UserData) -> IniResult<()> {
    let idle = ud.mask() == 1;
    let anim_name = lower(&ini.get_next_ascii_string());
    let distance_covered = match ini.get_next_token_or_null(None) {
        Some(t) => Ini::scan_real(&t)?,
        None => 0.0,
    };
    let mut times_to_repeat = match ini.get_next_token_or_null(None) {
        Some(t) => Ini::scan_int(&t)?,
        None => 1,
    };
    if times_to_repeat < 1 {
        times_to_repeat = 1;
    }
    let anim_info = W3DAnimationInfo::new(anim_name.clone(), idle, distance_covered);
    if s.ini_read_flags & (1 << ANIMS_COPIED_FROM_DEFAULT_STATE) != 0 {
        s.ini_read_flags &= !((1 << ANIMS_COPIED_FROM_DEFAULT_STATE) | (1 << GOT_IDLE_ANIMS) | (1 << GOT_NONIDLE_ANIMS));
        s.animations.clear();
    }
    if anim_info.is_idle_anim {
        s.ini_read_flags |= 1 << GOT_IDLE_ANIMS;
    } else {
        s.ini_read_flags |= 1 << GOT_NONIDLE_ANIMS;
    }
    if !anim_name.is_empty() && !is_none(&anim_name) {
        for _ in 0..times_to_repeat {
            s.animations.push(anim_info.clone());
        }
    }
    Ok(())
}

/// `parseShowHideSubObject` (`userData` 1: hide).
fn parse_show_hide_sub_object(ini: &mut Ini<'_>, s: &mut ModelConditionInfo, ud: &UserData) -> IniResult<()> {
    let hide = ud.mask() != 0;
    let vec = &mut s.hide_show_vec;
    let mut sub_obj_name = lower(&ini.get_next_ascii_string());
    if is_none(&sub_obj_name) {
        vec.clear();
        return Ok(());
    }
    while !sub_obj_name.is_empty() {
        let mut found = false;
        for it in vec.iter_mut() {
            if it.sub_obj_name.eq_ignore_ascii_case(&sub_obj_name) {
                it.hide = hide;
                found = true;
            }
        }
        if !found {
            vec.push(HideShowSubObjInfo { sub_obj_name: sub_obj_name.clone(), hide });
        }
        sub_obj_name = lower(&ini.get_next_ascii_string());
    }
    Ok(())
}

/// `parseWeaponBoneName` into the array `ud.mask()` names (0 fire FX, 1
/// recoil, 2 muzzle flash, 3 launch, 4 hide-show).
fn parse_weapon_bone_name(ini: &mut Ini<'_>, s: &mut ModelConditionInfo, ud: &UserData) -> IniResult<()> {
    let wslot = Ini::scan_index_list(&ini.get_next_token(None)?, &WEAPON_SLOT_TYPE_NAMES)? as usize;
    let mut v = lower(&ini.get_next_ascii_string());
    if is_none(&v) {
        v.clear();
    }
    let arr = match ud.mask() {
        0 => &mut s.weapon_fire_fx_bone_name,
        1 => &mut s.weapon_recoil_bone_name,
        2 => &mut s.weapon_muzzle_flash_name,
        3 => &mut s.weapon_projectile_launch_bone_name,
        _ => &mut s.weapon_projectile_hide_show_name,
    };
    arr[wslot] = v.clone();
    s.add_public_bone(&v);
    Ok(())
}

/// `parseBoneNameKey` into turret `ud.mask() / 2`'s angle (even) or pitch (odd) key.
fn parse_bone_name_key(ini: &mut Ini<'_>, s: &mut ModelConditionInfo, ud: &UserData) -> IniResult<()> {
    let tmp = lower(&ini.get_next_token(None)?);
    s.add_public_bone(&tmp);
    let key = if tmp.is_empty() || is_none(&tmp) { Vec::new() } else { tmp };
    let t = &mut s.turrets[(ud.mask() / 2) as usize];
    if ud.mask().is_multiple_of(2) {
        t.turret_angle_name_key = key;
    } else {
        t.turret_pitch_name_key = key;
    }
    Ok(())
}

/// `INI::parseAngleReal` into turret `ud.mask() / 2`'s art angle (even) or pitch (odd).
fn parse_turret_art(ini: &mut Ini<'_>, s: &mut ModelConditionInfo, ud: &UserData) -> IniResult<()> {
    let v = ini.parse_angle_real()?;
    let t = &mut s.turrets[(ud.mask() / 2) as usize];
    if ud.mask().is_multiple_of(2) {
        t.turret_art_angle = v;
    } else {
        t.turret_art_pitch = v;
    }
    Ok(())
}

/// `parseLowercaseNameKey` (`TransitionKey`, `WaitForStateToFinishIfPossible`).
fn parse_lowercase_name_key(ini: &mut Ini<'_>) -> IniResult<Vec<u8>> {
    Ok(lower(&ini.get_next_token(None)?))
}

/// `parseConditionState`'s `myFieldParse`.
fn condition_fields() -> Vec<FieldParse<ModelConditionInfo>> {
    vec![
        FieldParse::new("Model", |i, s, _| { s.model_name = parse_ascii_string_lc(i); Ok(()) }, UserData::None),
        FieldParse::new("Turret", parse_bone_name_key, UserData::Mask(0)),
        FieldParse::new("TurretArtAngle", parse_turret_art, UserData::Mask(0)),
        FieldParse::new("TurretPitch", parse_bone_name_key, UserData::Mask(1)),
        FieldParse::new("TurretArtPitch", parse_turret_art, UserData::Mask(1)),
        FieldParse::new("AltTurret", parse_bone_name_key, UserData::Mask(2)),
        FieldParse::new("AltTurretArtAngle", parse_turret_art, UserData::Mask(2)),
        FieldParse::new("AltTurretPitch", parse_bone_name_key, UserData::Mask(3)),
        FieldParse::new("AltTurretArtPitch", parse_turret_art, UserData::Mask(3)),
        FieldParse::new("ShowSubObject", parse_show_hide_sub_object, UserData::Mask(0)),
        FieldParse::new("HideSubObject", parse_show_hide_sub_object, UserData::Mask(1)),
        FieldParse::new("WeaponFireFXBone", parse_weapon_bone_name, UserData::Mask(0)),
        FieldParse::new("WeaponRecoilBone", parse_weapon_bone_name, UserData::Mask(1)),
        FieldParse::new("WeaponMuzzleFlash", parse_weapon_bone_name, UserData::Mask(2)),
        FieldParse::new("WeaponLaunchBone", parse_weapon_bone_name, UserData::Mask(3)),
        FieldParse::new("WeaponHideShowBone", parse_weapon_bone_name, UserData::Mask(4)),
        FieldParse::new("Animation", parse_animation, UserData::Mask(0)),
        FieldParse::new("IdleAnimation", parse_animation, UserData::Mask(1)),
        FieldParse::new("AnimationMode", |i, s, _| { s.mode = anim_mode_from_index(i.parse_index_list(&THE_ANIM_MODE_NAMES)?); Ok(()) }, UserData::None),
        FieldParse::new("TransitionKey", |i, s, _| { s.transition_key = parse_lowercase_name_key(i)?; Ok(()) }, UserData::None),
        FieldParse::new("WaitForStateToFinishIfPossible", |i, s, _| { s.allow_to_finish_key = parse_lowercase_name_key(i)?; Ok(()) }, UserData::None),
        FieldParse::new("Flags", |i, s, _| i.parse_bit_string32(&mut s.flags, &AC_BITS_NAMES), UserData::None),
        FieldParse::new(
            "ParticleSysBone",
            |i, s, _| {
                // parseParticleSysBone: the bone, then parseParticleSystemTemplate's token
                let bone_name = lower(&i.get_next_ascii_string());
                let particle_system_template = i.get_next_token(None)?;
                s.particle_sys_bones.push(ParticleSysBoneInfo { bone_name, particle_system_template });
                Ok(())
            },
            UserData::None,
        ),
        FieldParse::new(
            "AnimationSpeedFactorRange",
            |i, s, _| {
                // parseRealRange
                s.anim_min_speed_factor = Ini::scan_real(&i.get_next_token(None)?)?;
                s.anim_max_speed_factor = Ini::scan_real(&i.get_next_token(None)?)?;
                Ok(())
            },
            UserData::None,
        ),
        FieldParse::end(),
    ]
}

/// `W3DModelDrawModuleData::parseConditionState`.
fn parse_condition_state(ini: &mut Ini<'_>, d: &mut W3DModelDrawModuleData, ud: &UserData) -> IniResult<()> {
    let mut info = ModelConditionInfo::default();
    let cst = ud.mask();
    match cst {
        PARSE_DEFAULT => {
            if d.default_state >= 0 {
                // you may have only one default state
                return Err(invalid());
            } else if ini.get_next_token_or_null(None).is_some() {
                // unknown keyword
                return Err(invalid());
            } else {
                if !d.condition_states.is_empty() {
                    // when using DefaultConditionState, it must be the first state listed
                    return Err(invalid());
                }
                d.default_state = d.condition_states.len() as i32;
                info.conditions_yes_vec.clear();
                info.conditions_yes_vec.push(ModelConditionFlags::new());
            }
        }
        PARSE_TRANSITION => {
            let first_nm = lower(&ini.get_next_token(None)?);
            let second_nm = lower(&ini.get_next_token(None)?);
            if first_nm == second_nm {
                // You may not declare a transition between two identical states
                return Err(invalid());
            }
            if d.default_state >= 0 {
                info = d.condition_states[d.default_state as usize].clone();
                info.ini_read_flags |= 1 << ANIMS_COPIED_FROM_DEFAULT_STATE;
                info.transition_key.clear();
                info.allow_to_finish_key.clear();
            }
            info.transition_sig = Some((first_nm, second_nm));
        }
        PARSE_ALIAS => {
            if d.condition_states.is_empty() {
                // AliasConditionState must refer to the previous state
                return Err(invalid());
            }
            let mut conditions_yes = ModelConditionFlags::new();
            conditions_yes.parse(ini, None)?;
            if conditions_yes.any_intersection_with(&d.ignore_condition_states) {
                return Err(invalid());
            }
            if does_state_exist(&d.condition_states, &conditions_yes) {
                // duplicate condition states are not currently allowed
                return Err(invalid());
            }
            if !conditions_yes.any() && d.default_state >= 0 {
                // you may not specify both a Default state and a Conditions=None state
                return Err(invalid());
            }
            let n = d.condition_states.len();
            d.condition_states[n - 1].conditions_yes_vec.push(conditions_yes);
            // yes, return, NOT break!
            return Ok(());
        }
        _ => {
            if d.default_state >= 0 {
                info = d.condition_states[d.default_state as usize].clone();
                info.ini_read_flags |= 1 << ANIMS_COPIED_FROM_DEFAULT_STATE;
                info.conditions_yes_vec.clear();
            }
            let mut conditions_yes = ModelConditionFlags::new();
            conditions_yes.parse(ini, None)?;
            if conditions_yes.any_intersection_with(&d.ignore_condition_states) {
                return Err(invalid());
            }
            if d.default_state < 0 && d.condition_states.is_empty() && conditions_yes.any() {
                // when not using DefaultConditionState, the first ConditionState must be for NONE
                return Err(invalid());
            }
            if !conditions_yes.any() && d.default_state >= 0 {
                return Err(invalid());
            }
            if does_state_exist(&d.condition_states, &conditions_yes) {
                return Err(invalid());
            }
            info.conditions_yes_vec.clear();
            info.conditions_yes_vec.push(conditions_yes);
        }
    }

    ini.init_from_ini(&mut info, &condition_fields())?;

    if info.model_name.is_empty() {
        // you must specify a model name
        return Err(invalid());
    } else if is_none(&info.model_name) {
        info.model_name.clear();
    }
    if info.ini_read_flags & (1 << GOT_IDLE_ANIMS) != 0 && info.ini_read_flags & (1 << GOT_NONIDLE_ANIMS) != 0 {
        // you should not specify both Animations and IdleAnimations for the same state
        return Err(invalid());
    }
    let once = |m: AnimMode| m == AnimMode::Once || m == AnimMode::OnceBackwards;
    if info.ini_read_flags & (1 << GOT_IDLE_ANIMS) != 0 && !once(info.mode) {
        // Idle Anims should always use ONCE or ONCE_BACKWARDS
        return Err(invalid());
    }
    let mut vs = info.valid_stuff.load(Ordering::Relaxed) & !HAS_PROJECTILE_BONES;
    if info.weapon_projectile_launch_bone_name.iter().any(|n| !n.is_empty()) {
        vs |= HAS_PROJECTILE_BONES;
    }
    info.valid_stuff.store(vs, Ordering::Relaxed);

    if cst == PARSE_TRANSITION {
        if info.ini_read_flags & (1 << GOT_IDLE_ANIMS) != 0 {
            // Transition States should not specify Idle anims
            return Err(invalid());
        }
        if !once(info.mode) {
            // Transition States should always use ONCE or ONCE_BACKWARDS
            return Err(invalid());
        }
        if !info.transition_key.is_empty() || !info.allow_to_finish_key.is_empty() {
            // Transition States must not have transition keys or m_allowToFinishKey
            return Err(invalid());
        }
        let sig = info.transition_sig.clone().expect("set above");
        d.transition_map.insert(sig, info);
    } else {
        d.condition_states.push(info);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The render object's side (Animatable3DObjClass) and the assets
// ---------------------------------------------------------------------------

/// An animation as the asset manager hands it out (`HAnimClass*`): what the
/// draw module and `Animatable3DObjClass` read of it.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimHandle {
    /// `Get_Name()`.
    pub name: Vec<u8>,
    /// `Get_Num_Frames()`.
    pub num_frames: i32,
    /// `Get_Frame_Rate()`.
    pub frame_rate: f32,
}

/// What `W3DDisplay::m_assetManager` answers the draw module.
pub trait W3dAssetSource {
    /// `Create_Render_Obj(name, scale, color)`: `None` when there is no such
    /// render object; `Some(true)` for an HLOD (an `Animatable3DObjClass`),
    /// `Some(false)` for another kind (a mesh), which ignores animations.
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool>;
    /// `Get_HAnim(name)`.
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle>;
    /// The hierarchy and sub-objects of a new render object of `name` (`Create_Render_Obj`),
    /// posed at `pose`'s animation and frame (the base pose without one, or when the animation
    /// is not found) under a transform of `scale` (`Matrix3D(true).Scale(scale)`): what
    /// `Get_Bone_Index`, `Get_Bone_Transform`, `Get_Sub_Object_By_Name` and the hide-show
    /// functions read of it. `None` when it has none to give (no such render object, or a
    /// source without hierarchies); a mesh gives an empty [`Skeleton`].
    fn skeleton(&mut self, _name: &[u8], _pose: Option<(&[u8], f32)>, _scale: f32) -> Option<Skeleton> {
        None
    }
    /// How many questions this source has answered "none for now" because the files that decide the answer are still
    /// being fetched (the browser's ranged reads, GitHub #24), as opposed to "none": the validations compare it before
    /// and after asking, and stay not valid when it moved (GitHub #42). New, not upstream's: upstream's reads never
    /// wait. A source whose files are at hand never answers "for now": 0.
    fn deferred_answers(&self) -> usize {
        0
    }
}

/// A render object's bone: `Pivot[i]` of its `HTreeClass` as the draw module reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonBone {
    /// `Pivot[i].Name`, up to its NUL.
    pub name: Vec<u8>,
    /// `Get_Parent_Index(i)`: the parent's index, 0 for none.
    pub parent_index: i32,
    /// `Get_Transform(i)` in the pose the skeleton was made for.
    pub transform: Matrix3D,
}

/// A render object's sub-object (`Get_Sub_Object(i)`).
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonSubObject {
    /// `Get_Name()` (`CONTAINER.MESH`).
    pub name: Vec<u8>,
    /// The bone it is bound to (`Get_Sub_Object_Bone_Index`).
    pub bone_index: i32,
    /// `Is_Hidden()` as made (`W3D_MESH_FLAG_HIDDEN`).
    pub hidden: bool,
    /// `Get_Transform()` in the pose: its bone's transform.
    pub transform: Matrix3D,
    /// How many of its `Get_Material_Info()`'s vertex materials have a stage-0 mapper
    /// (`Peek_Mapper()`) of `MAPPER_ID_LINEAR_OFFSET`: what `W3DTankDraw::updateTreadObjects` looks
    /// for (GitHub #41). Every sub-object here is a mesh (`CLASSID_MESH`).
    pub linear_offset_materials: usize,
}

/// What the draw module reads of a render object's hierarchy and sub-objects (see
/// [`W3dAssetSource::skeleton`]). The sub-objects are [`RenderModel`]'s: the top LOD's meshes,
/// then the aggregates' (upstream counts every LOD and non-mesh sub-objects too; the same list
/// for the game's one-LOD models made only of meshes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    /// The pivots; empty for a render object without a hierarchy (a mesh).
    pub bones: Vec<SkeletonBone>,
    pub sub_objects: Vec<SkeletonSubObject>,
}

impl Skeleton {
    /// The skeleton of a posed [`RenderModel`].
    pub fn of(m: &RenderModel) -> Skeleton {
        let Some(tree) = (if m.is_hlod { m.htree.as_ref() } else { None }) else {
            return Skeleton::default();
        };
        let mut bones = Vec::new();
        for i in 0..tree.num_pivots().max(0) as usize {
            let p = tree.pivot(i).unwrap_or_default();
            let end = p.name.iter().position(|&c| c == 0).unwrap_or(p.name.len());
            let parent_index = p.parent.and_then(|q| tree.pivot(q as usize)).map_or(0, |q| q.index);
            bones.push(SkeletonBone { name: p.name[..end].to_vec(), parent_index, transform: tree.get_transform(i as i32).unwrap_or(Matrix3D::IDENTITY) });
        }
        let sub_objects = m
            .sub_objects
            .iter()
            .map(|s| SkeletonSubObject {
                name: s.name.clone(),
                bone_index: s.bone_index,
                hidden: s.hidden,
                transform: tree.get_transform(s.bone_index).unwrap_or(Matrix3D::IDENTITY),
                linear_offset_materials: s.model.vertex_materials.iter().filter(|v| v.mapper[0] == MapperKind::LinearOffset).count(),
            })
            .collect();
        Skeleton { bones, sub_objects }
    }

    /// `Get_HTree() != NULL`.
    pub fn has_htree(&self) -> bool {
        !self.bones.is_empty()
    }

    /// `Get_Bone_Index(name)`: the first pivot of that name (ignoring case), else 0.
    pub fn get_bone_index(&self, name: &[u8]) -> i32 {
        self.bones.iter().position(|b| b.name.eq_ignore_ascii_case(name)).map_or(0, |i| i as i32)
    }

    /// `Get_Parent_Index(bone)` (0 out of range: upstream asserts).
    pub fn get_parent_index(&self, bone: i32) -> i32 {
        usize::try_from(bone).ok().and_then(|b| self.bones.get(b)).map_or(0, |b| b.parent_index)
    }

    /// `Get_Sub_Object_By_Name(name, &index)`: the first sub-object of that name (ignoring
    /// case), else the first whose name after its first `.` is it.
    pub fn get_sub_object_by_name(&self, name: &[u8]) -> Option<usize> {
        if let Some(i) = self.sub_objects.iter().position(|s| s.name.eq_ignore_ascii_case(name)) {
            return Some(i);
        }
        self.sub_objects.iter().position(|s| {
            let suffix = match s.name.iter().position(|&c| c == b'.') {
                Some(d) => &s.name[d + 1..],
                None => &s.name[..],
            };
            suffix.eq_ignore_ascii_case(name)
        })
    }
}

/// The asset manager over the game's files.
pub struct W3dFiles<'a, D: LocalDevice> {
    pub assets: &'a mut AssetManager,
    pub fs: &'a FileSystem<D>,
}

impl<D: LocalDevice> W3dAssetSource for W3dFiles<'_, D> {
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
        match self.assets.create_render_obj(self.fs, name) {
            Ok(Some(Prototype::HLod(_))) => Some(true),
            Ok(Some(_)) => Some(false),
            _ => None,
        }
    }
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
        match self.assets.get_hanim(self.fs, name) {
            Ok(Some(a)) => Some(AnimHandle { name: a.name().to_vec(), num_frames: a.get_num_frames(), frame_rate: get_frame_rate(a) }),
            _ => None,
        }
    }
    /// The render model as drawn ([`RenderModel::create`]), posed by `Anim_Update` (or
    /// `Base_Update`) under the scaled identity, as `Validate_Transform` poses it.
    fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let mut m = RenderModel::create(self.assets, self.fs, name).ok()??;
        let mut root = Matrix3D::IDENTITY;
        root.scale(scale);
        let anim = match pose {
            Some((anim, frame)) => match self.assets.get_hanim(self.fs, anim) {
                Ok(Some(a)) => Some((a.clone(), frame)),
                _ => None,
            },
            None => None,
        };
        match &anim {
            Some((a, frame)) => m.update(&root, Some((a, &*self.assets, *frame))).ok()?,
            None => m.update::<AssetManager>(&root, None).ok()?,
        }
        Some(Skeleton::of(&m))
    }
}

/// `Animatable3DObjClass::ModeAnim` (in `SINGLE_ANIM`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModeAnim {
    pub motion: AnimHandle,
    pub frame: f32,
    pub prev_frame: f32,
    /// `LastSyncTime`, milliseconds.
    pub last_sync_time: u32,
    pub frame_rate_multiplier: f32,
    pub anim_direction: f32,
    pub anim_mode: AnimMode,
}

/// Where a handled `handleWeaponFireFX` plays its FX list (GitHub #43).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFireFxAt {
    /// The FX bone's transform in the model's space (its pose as made); `None` when the asset source gave no skeleton.
    pub bone_transform: Option<Matrix3D>,
    /// `m_renderObject->Is_Hidden()`: upstream then plays at the logic object's position and transform.
    pub render_hidden: bool,
}

/// A render object as the draw module sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderObj {
    /// The name it was created by.
    pub model_name: Vec<u8>,
    /// `Class_ID() == CLASSID_HLOD`: only these play animations.
    pub is_hlod: bool,
    /// `CurMotionMode == SINGLE_ANIM`'s `ModeAnim`; `None` in `BASE_POSE`.
    pub anim: Option<ModeAnim>,
    /// `ModeAnim.Frame` kept across `Set_Animation` (its `PrevFrame`).
    last_frame: f32,
    /// `Is_Hidden()`.
    pub hidden: bool,
    /// Its hierarchy and sub-objects as made ([`W3dAssetSource::skeleton`] in the base pose);
    /// `None` when the asset source gives none.
    pub skeleton: Option<Arc<Skeleton>>,
    /// Its `HTreeClass`'s bone captures and controls (`Capture_Bone`, `Control_Bone`), by bone;
    /// a bone not listed is as made (not captured, the identity).
    pub bone_controls: BTreeMap<i32, BoneControl>,
    /// Each sub-object's `Is_Hidden()` (the skeleton's sub-objects), as `Set_Hidden` left it.
    pub sub_object_hidden: Vec<bool>,
    /// The sub-objects given a `Material_Override` as user data (`Set_User_Data`), by sub-object
    /// index: its `customUVOffset` (GitHub #41, `W3DTankDraw`'s treads). Upstream the sub-object
    /// points at the draw module's settings; here the module writes the value each draw.
    pub material_overrides: BTreeMap<usize, [f32; 2]>,
}

/// One pivot's capture state (`PivotClass::IsCaptured`, `CapTransform`, `WorldSpaceTranslation`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneControl {
    pub captured: bool,
    pub transform: Matrix3D,
    pub world_space_translation: bool,
}

impl Default for BoneControl {
    fn default() -> Self {
        BoneControl { captured: false, transform: Matrix3D::IDENTITY, world_space_translation: false }
    }
}

impl RenderObj {
    /// A new render object of `name` (`Create_Render_Obj`).
    pub fn new(name: &[u8], is_hlod: bool) -> Self {
        RenderObj {
            model_name: name.to_vec(),
            is_hlod,
            anim: None,
            last_frame: 0.0,
            hidden: false,
            skeleton: None,
            bone_controls: BTreeMap::new(),
            sub_object_hidden: Vec::new(),
            material_overrides: BTreeMap::new(),
        }
    }

    /// The render object with its hierarchy and sub-objects (their hidden flags as made).
    pub fn with_skeleton(mut self, skeleton: Option<Skeleton>) -> Self {
        self.sub_object_hidden = skeleton.as_ref().map_or_else(Vec::new, |k| k.sub_objects.iter().map(|s| s.hidden).collect());
        self.skeleton = skeleton.map(Arc::new);
        self
    }

    /// `Get_HTree()` is not `NULL`: an HLOD with a hierarchy.
    fn has_htree(&self) -> bool {
        self.is_hlod && self.skeleton.as_ref().is_some_and(|k| k.has_htree())
    }

    /// `Capture_Bone(boneindex)` (`Animatable3DObjClass`: nothing without a hierarchy; a bone
    /// out of range is an upstream assertion, ignored).
    pub fn capture_bone(&mut self, boneindex: i32) {
        if self.has_htree() && self.skeleton.as_ref().is_some_and(|k| boneindex >= 0 && (boneindex as usize) < k.bones.len()) {
            self.bone_controls.entry(boneindex).or_default().captured = true;
        }
    }

    /// `Control_Bone(bindex, objtm, world_space_translation = false)`.
    pub fn control_bone(&mut self, bindex: i32, objtm: &Matrix3D) {
        if self.has_htree() && self.skeleton.as_ref().is_some_and(|k| bindex >= 0 && (bindex as usize) < k.bones.len()) {
            let c = self.bone_controls.entry(bindex).or_default();
            c.transform = *objtm;
            c.world_space_translation = false;
        }
    }

    /// `Get_Sub_Object_On_Bone(index, boneindex)`: the `index`th sub-object on the bone.
    pub fn get_sub_object_on_bone(&self, index: usize, boneindex: i32) -> Option<usize> {
        let k = self.skeleton.as_ref()?;
        k.sub_objects.iter().enumerate().filter(|(_, s)| s.bone_index == boneindex).map(|(i, _)| i).nth(index)
    }

    /// `Get_Sub_Object(i)->Set_Hidden(hide)`.
    pub fn set_sub_object_hidden(&mut self, i: usize, hide: bool) {
        if let Some(h) = self.sub_object_hidden.get_mut(i) {
            *h = hide;
        }
    }

    /// Whether sub-object `i` is hidden (`Get_Sub_Object(i)->Is_Hidden()`).
    pub fn is_sub_object_hidden(&self, i: usize) -> bool {
        self.sub_object_hidden.get(i).copied().unwrap_or(false)
    }

    /// The render object's bone controls and sub-object visibility put on a render model made
    /// for the same name (the client's): every pivot of its hierarchy captured and controlled
    /// as this object's are (the others released, their control the identity), and each
    /// sub-object hidden as this object's is, and given this object's material overrides' UV
    /// offsets (GitHub #41; none on the others). Call before posing it ([`RenderModel::update`]).
    /// New (upstream's render object is the drawn one).
    pub fn pose_render_model(&self, m: &mut RenderModel) {
        if let Some(tree) = &mut m.htree {
            for i in 0..tree.num_pivots().max(0) {
                let c = self.bone_controls.get(&i).copied().unwrap_or_default();
                let as_made = tree.pivot(i as usize).is_some_and(|p| !p.is_captured && p.cap_transform == Matrix3D::IDENTITY && !p.world_space_translation);
                if c == BoneControl::default() && as_made {
                    continue;
                }
                let _ = tree.control_bone(i, &c.transform, c.world_space_translation);
                let _ = if c.captured { tree.capture_bone(i) } else { tree.release_bone(i) };
            }
        }
        if m.sub_objects.len() == self.sub_object_hidden.len() {
            for (s, &h) in m.sub_objects.iter_mut().zip(&self.sub_object_hidden) {
                s.hidden = h;
            }
        }
        for (i, s) in m.sub_objects.iter_mut().enumerate() {
            s.uv_offset = self.material_overrides.get(&i).copied();
        }
    }

    /// `Set_Animation(motion, frame, mode)` at sync time `now` (nothing on a
    /// render object that is not an HLOD).
    pub fn set_animation(&mut self, motion: Option<AnimHandle>, frame: f32, mode: AnimMode, now: u32) {
        if !self.is_hlod {
            return;
        }
        match motion {
            Some(motion) => {
                let prev_frame = self.anim.as_ref().map_or(self.last_frame, |a| a.frame);
                let anim_direction = if anim_mode_index(mode) < anim_mode_index(AnimMode::LoopBackwards) { 1.0 } else { -1.0 };
                self.anim = Some(ModeAnim { motion, frame, prev_frame, last_sync_time: now, frame_rate_multiplier: 1.0, anim_direction, anim_mode: mode });
                self.last_frame = frame;
            }
            None => self.anim = None,
        }
    }

    /// `Single_Anim_Progress()` at sync time `now` (what `Render` does
    /// before it draws, unless the mode is `MANUAL`).
    pub fn render(&mut self, now: u32) {
        let Some(a) = &mut self.anim else { return };
        if a.anim_mode == AnimMode::Manual {
            return;
        }
        let oldprev = a.prev_frame;
        a.prev_frame = a.frame;
        let diff = now.wrapping_sub(a.last_sync_time) as f32;
        let (f, d) = compute_current_frame(a.frame, a.motion.num_frames, a.motion.frame_rate, a.frame_rate_multiplier, a.anim_direction, diff, a.anim_mode);
        a.frame = f;
        a.anim_direction = d;
        a.last_sync_time = now;
        if a.frame == a.prev_frame {
            a.prev_frame = oldprev;
        }
        self.last_frame = a.frame;
    }

    /// `Is_Animation_Complete()`: on the last frame of a `ONCE`, or the
    /// first of a `ONCE_BACKWARDS`.
    pub fn is_animation_complete_hlod(&self) -> bool {
        match &self.anim {
            Some(a) if a.anim_mode == AnimMode::Once => a.frame == (a.motion.num_frames - 1) as f32,
            Some(a) if a.anim_mode == AnimMode::OnceBackwards => a.frame == 0.0,
            _ => false,
        }
    }

    /// `Set_Animation_Frame_Rate_Multiplier(multiplier)`.
    pub fn set_animation_frame_rate_multiplier(&mut self, multiplier: f32) {
        if let Some(a) = &mut self.anim {
            a.frame_rate_multiplier = multiplier;
        }
    }
}

/// `isAnimationComplete(r)`: an HLOD's own answer; true without a render
/// object or for another kind.
pub fn is_animation_complete(r: Option<&RenderObj>) -> bool {
    match r {
        Some(r) if r.is_hlod => r.is_animation_complete_hlod(),
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// W3DModelDraw
// ---------------------------------------------------------------------------

/// What a draw module call needs from the client around it.
pub struct DrawEnv<'a, A: W3dAssetSource> {
    pub assets: &'a mut A,
    /// The client's random generator (`GameClientRandomValue`).
    pub random: &'a mut RandomValues,
    /// `WW3D::Get_Sync_Time()`, milliseconds.
    pub sync_time: u32,
    /// `isValidTimeToCalcLogicStuff()` (see the module docs).
    pub logic_time: bool,
}

/// What `doDrawModule` reads from the drawable and its object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawFacts {
    /// `getDrawable()->getShouldAnimate(m_animationsRequirePower)`, with and
    /// without power considered.
    pub should_animate_with_power: bool,
    pub should_animate_without_power: bool,
    /// The object's `getPhysics()->getVelocityMagnitude()`, distance per
    /// logic frame; `None` without an object or physics. Read in place
    /// (`PhysicsBehavior::peek_velocity_magnitude`, GitHub #45), the cache not filled.
    pub speed: Option<f32>,
    /// Each turret's `ai->getTurretRotAndPitch(tslot, &angle, &pitch)` (radians), read from
    /// the World; `None` when the object has no AI or the call answers false (the angles stay 0).
    pub turrets: [Option<(f32, f32)>; MAX_TURRETS],
    /// What the subclasses' `doDrawModule` read of the object's physics and AI (GitHub #41);
    /// `None` without an object or without physics.
    pub motion: Option<MotionFacts>,
    /// `TheGlobalData->m_showClientPhysics` (the truck draws do nothing of their own without it).
    pub show_client_physics: bool,
    /// `getDrawable()->getWheelInfo()`, after this draw's `applyPhysicsXform` (GitHub #41,
    /// `crate::client::drawable_loco`); `None` until the drawable's first physics model ran.
    pub wheel_info: Option<crate::client::drawable_loco::TWheelInfo>,
}

/// The logic's state a subclass draw module reads (`W3DTankDraw::doDrawModule`), read only: see
/// `ClientDrawables::draw`, which fills it with `World::physics_peek`'s `peek_*`,
/// `World::peek_cur_locomotor_speed` and `Thing::peek_unit_direction_vector_2d`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionFacts {
    /// `physics->getVelocityMagnitude()`.
    pub velocity_magnitude: f32,
    /// `physics->getTurning()`: `TURN_NEGATIVE` (-1), `TURN_NONE` (0), `TURN_POSITIVE` (1).
    pub turning: i32,
    /// `physics->isMotive()`.
    pub is_motive: bool,
    /// `obj->getAIUpdateInterface()->getCurLocomotorSpeed()`; `None` without an AI (upstream
    /// dereferences it all the same).
    pub cur_locomotor_speed: Option<f32>,
    /// `obj->getUnitDirectionVector2D()` (x, y).
    pub unit_direction_2d: (f32, f32),
    /// `physics->getVelocity()`.
    pub velocity: crate::common::base_type::Coord3D,
    /// `ai->getCurLocomotor()->isMovingBackwards()`; false without an AI or a current locomotor.
    pub moving_backwards: bool,
    /// `ThePartitionManager->getRelativeAngle2D(obj, &pointOnPath)` for the AI path's
    /// `peekCachedPointOnPath`; `None` without an AI or a path (`ai->getPath()`).
    pub angle_to_path_point: Option<f32>,
}

/// `PhysicsTurningType::TURN_NEGATIVE`.
pub const TURN_NEGATIVE: i32 = -1;
/// `PhysicsTurningType::TURN_NONE`.
pub const TURN_NONE: i32 = 0;
/// `PhysicsTurningType::TURN_POSITIVE`.
pub const TURN_POSITIVE: i32 = 1;

/// `W3DTankDraw::MAX_TREADS_PER_TANK`.
pub const MAX_TREADS_PER_TANK: usize = 4;

/// `W3DTankDraw::TreadType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreadType {
    Left,
    Right,
    Middle,
}

/// `W3DTankDraw::TreadObjectInfo`: the sub-object (by index, for `m_robj`), its type, and its
/// material settings' `customUVOffset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreadObjectInfo {
    pub sub_object: usize,
    pub kind: TreadType,
    pub custom_uv_offset: [f32; 2],
}

/// `W3DTankDraw`'s own state (GitHub #41): `m_treads`, `m_treadCount` (the length),
/// `m_prevRenderObj` (by the render object's serial, see [`W3DModelDraw::render_obj_serial`]),
/// `m_lastDirection`. The tread debris particle systems are not made (~~no particle systems yet~~ **Corrected
/// 2026-11-06 (GitHub #44):** the client's particle systems exist, `crate::client::particle_sys`; the draw modules do
/// not reach them yet).
#[derive(Clone, Debug, PartialEq)]
pub struct TankDraw {
    pub treads: Vec<TreadObjectInfo>,
    pub prev_render_obj: Option<u32>,
    pub last_direction: (f32, f32),
}

impl TankDraw {
    /// `W3DTankDraw(thing, moduleData)`: no treads; "assume all things face along x axis when
    /// created".
    pub fn new() -> Self {
        TankDraw { treads: Vec::new(), prev_render_obj: None, last_direction: (1.0, 0.0) }
    }

    /// `updateTreadPositions(uvDelta)`: each tread's U offset moved by `uv_delta` (a left tread
    /// forwards, a right one backwards) and wrapped into [0, 1) (`offset - WWMath::Floor(offset)`).
    /// A middle tread: upstream's `offset_u` is not set for it (the previous tread's value, or
    /// uninitialised for the first: undefined); here its offset is wrapped unchanged.
    pub fn update_tread_positions(&mut self, uv_delta: f32) {
        for t in &mut self.treads {
            let mut offset_u = match t.kind {
                TreadType::Left => t.custom_uv_offset[0] + uv_delta,
                TreadType::Right => t.custom_uv_offset[0] - uv_delta,
                TreadType::Middle => t.custom_uv_offset[0],
            };
            // ensure coordinates of offset are in [0, 1] range:
            offset_u -= crate::ww::wwmath::floor(offset_u);
            t.custom_uv_offset = [offset_u, 0.0];
        }
    }
}

impl Default for TankDraw {
    fn default() -> Self {
        Self::new()
    }
}

/// The tread loop of `W3DTankDraw::updateTreadObjects` and `W3DTankTruckDraw::updateTreadObjects`, for render object
/// `r` and module data rate `rate` (none when 0 or without a render object): each sub-object (up to
/// `MAX_TREADS_PER_TANK` treads) that is a mesh named `<container>.TREADS*` (ignoring case) gives one tread per vertex
/// material whose stage-0 mapper is a linear-offset mapper, at offset (0, 0); with `typed` (the tank's), `TREADSL*` is a
/// left tread, `TREADSR*` a right one and any other a middle one; without (the tank truck's, whose switch is commented
/// out upstream: "on vehicles with wheels, it makes no sense to turn with treads") every tread is a middle one. The
/// treads, and the render object's material overrides that go with them.
pub(crate) fn find_tread_objects(r: Option<&RenderObj>, rate: f32, typed: bool) -> (Vec<TreadObjectInfo>, BTreeMap<usize, [f32; 2]>) {
    let mut treads = Vec::new();
    let mut overrides = BTreeMap::new();
    if let Some(r) = r {
        //Make sure this object has defined a speed for tread scrolling.
        if rate != 0.0 {
            let subs = r.skeleton.as_ref().map(|k| k.sub_objects.as_slice()).unwrap_or(&[]);
            for (i, sub) in subs.iter().enumerate() {
                if treads.len() >= MAX_TREADS_PER_TANK {
                    break;
                }
                //Check if subobject name starts with "TREADS".
                let Some(dot) = sub.name.iter().position(|&c| c == b'.') else { continue };
                let mesh_name = &sub.name[dot + 1..];
                if mesh_name.len() < 6 || !mesh_name[..6].eq_ignore_ascii_case(b"TREADS") {
                    continue;
                }
                //check if sub-object has the correct material to do texture scrolling.
                for _ in 0..sub.linear_offset_materials {
                    if treads.len() >= MAX_TREADS_PER_TANK {
                        break;
                    }
                    //check next character after 'TREADS'
                    let kind = match mesh_name.get(6) {
                        Some(b'L' | b'l') if typed => TreadType::Left,
                        Some(b'R' | b'r') if typed => TreadType::Right,
                        _ => TreadType::Middle,
                    };
                    treads.push(TreadObjectInfo { sub_object: i, kind, custom_uv_offset: [0.0, 0.0] });
                    overrides.insert(i, [0.0, 0.0]);
                }
            }
        }
    }
    (treads, overrides)
}

/// `W3DModelDraw::WeaponRecoilInfo::RecoilState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoilState {
    Idle,
    RecoilStart,
    Recoil,
    Settle,
}

/// `W3DModelDraw::WeaponRecoilInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRecoilInfo {
    /// `m_state`: what state this gun is in.
    pub state: RecoilState,
    /// `m_shift`: how far the gun barrel has recoiled.
    pub shift: f32,
    /// `m_recoilRate`: how fast the gun barrel is recoiling.
    pub recoil_rate: f32,
}

impl Default for WeaponRecoilInfo {
    /// `WeaponRecoilInfo()`, which calls `clear()`.
    fn default() -> Self {
        WeaponRecoilInfo { state: RecoilState::Idle, shift: 0.0, recoil_rate: 0.0 }
    }
}

/// `class W3DModelDraw` (the parts listed in the module docs).
#[derive(Clone, Debug)]
pub struct W3DModelDraw {
    pub data: Arc<W3DModelDrawModuleData>,
    /// `m_curState`.
    cur_state: Option<StateRef>,
    /// `m_nextState`.
    next_state: Option<StateRef>,
    /// `m_nextStateAnimLoopDuration`.
    next_state_anim_loop_duration: u32,
    /// `m_hexColor`.
    pub hex_color: i32,
    /// `m_whichAnimInCurState`.
    which_anim_in_cur_state: i32,
    /// `m_renderObject`.
    pub render_object: Option<RenderObj>,
    /// `m_hideHeadlights`.
    pub hide_headlights: bool,
    /// `m_pauseAnimation`.
    pause_animation: bool,
    /// `m_animationMode`.
    animation_mode: AnimMode,
    /// The drawable is effectively hidden (`isDrawableEffectivelyHidden`),
    /// carried to each new render object.
    pub drawable_hidden: bool,
    /// `getDrawable()->getScale()` (`m_instanceScale`, the template's `Scale`).
    pub scale: f32,
    /// `TheGlobalData->m_standardPublicBones`.
    pub standard_public_bones: Arc<Vec<Vec<u8>>>,
    /// `m_subObjectVec`: the sub-objects shown or hidden by `showSubObject`.
    pub sub_object_vec: Vec<HideShowSubObjInfo>,
    /// `m_weaponRecoilInfoVec`.
    pub weapon_recoil_info_vec: [Vec<WeaponRecoilInfo>; SLOTS],
    /// Counts the render objects made, the current one's number being its identity: what a
    /// subclass's `m_prevRenderObj` compares (a pointer upstream). New (GitHub #41).
    pub render_obj_serial: u32,
    /// `W3DTankDraw`'s state, for a module whose data is `W3DTankDrawModuleData` (GitHub #41).
    pub tank: Option<TankDraw>,
    /// The truck draws' state (GitHub #41, [`crate::client::w3d_truck_draw`]): `W3DTruckDraw` (and
    /// `W3DOverlordTruckDraw`, `W3DPoliceCarDraw`), or `W3DTankTruckDraw`.
    pub truck: Option<TruckKind>,
    /// `W3DDependencyModelDraw::m_dependencyCleared` (`FALSE` at construction), which its xfer saves (GitHub #59). Kept
    /// but not used: the dependency draw's own behaviour (drawn only once its container says so) is not translated.
    pub dependency_cleared: bool,
}

impl W3DModelDraw {
    /// `W3DModelDraw(thing, moduleData)`: validate the states for the time
    /// of day and weather, then take the state for no flags; a module with
    /// none is an error ("all draw modules must have an IDLE state").
    ///
    /// The drawable's scale is 1 and the global data has no standard public bones; see
    /// [`W3DModelDraw::new_scaled`].
    pub fn new<A: W3dAssetSource>(data: Arc<W3DModelDrawModuleData>, hex_color: i32, night: bool, snowy: bool, env: &mut DrawEnv<'_, A>) -> IniResult<Self> {
        Self::new_scaled(data, hex_color, night, snowy, 1.0, Arc::new(Vec::new()), env)
    }

    /// [`W3DModelDraw::new`] for a drawable of `scale` (`getScale()`), with the global data's
    /// standard public bones.
    pub fn new_scaled<A: W3dAssetSource>(
        data: Arc<W3DModelDrawModuleData>,
        hex_color: i32,
        night: bool,
        snowy: bool,
        scale: f32,
        standard_public_bones: Arc<Vec<Vec<u8>>>,
        env: &mut DrawEnv<'_, A>,
    ) -> IniResult<Self> {
        data.validate_stuff_for_time_and_weather(night, snowy, scale, &standard_public_bones, env);
        let info = data.find_best_info(&ModelConditionFlags::new()).ok_or_else(invalid)?;
        let data_extra = data.extra.clone();
        let mut d = W3DModelDraw {
            data,
            cur_state: None,
            next_state: None,
            next_state_anim_loop_duration: NO_NEXT_DURATION,
            hex_color,
            which_anim_in_cur_state: -1,
            render_object: None,
            hide_headlights: true,
            pause_animation: false,
            animation_mode: AnimMode::Loop,
            drawable_hidden: false,
            scale,
            standard_public_bones,
            sub_object_vec: Vec::new(),
            weapon_recoil_info_vec: Default::default(),
            render_obj_serial: 0,
            tank: matches!(data_extra, DrawModuleExtra::Tank(_)).then(TankDraw::new),
            truck: None,
            dependency_cleared: false,
        };
        d.set_model_state(StateRef::Normal(info), env);
        // the subclass's own members are made after the base class's constructor (whose `setModelState` calls the
        // base class's `onRenderObjRecreated`, C++'s dispatch during construction): the trucks find their bones on
        // their first draw
        d.truck = TruckKind::new_for(&d.data.module_name, &d.data.extra, env.random);
        Ok(d)
    }

    /// `getCurState()`.
    pub fn get_cur_state(&self) -> Option<&ModelConditionInfo> {
        self.cur_state.as_ref().map(|s| self.data.state(s))
    }

    /// The current state's reference.
    pub fn cur_state_ref(&self) -> Option<&StateRef> {
        self.cur_state.as_ref()
    }

    /// The pending state's reference (`m_nextState`).
    pub fn next_state_ref(&self) -> Option<&StateRef> {
        self.next_state.as_ref()
    }

    /// `m_whichAnimInCurState`.
    pub fn which_anim_in_cur_state(&self) -> i32 {
        self.which_anim_in_cur_state
    }

    /// The name of the animation playing (the render object's), if any.
    pub fn current_animation(&self) -> Option<&AnimHandle> {
        self.render_object.as_ref().and_then(|r| r.anim.as_ref()).map(|a| &a.motion)
    }

    /// `findBestInfo(c)`.
    pub fn find_best_info(&self, c: &ModelConditionFlags) -> Option<usize> {
        self.data.find_best_info(c)
    }

    /// `findTransitionForSig(sig)`.
    pub fn find_transition_for_sig(&self, sig: &TransitionSig) -> Option<StateRef> {
        self.data.transition_map.contains_key(sig).then(|| StateRef::Transition(sig.clone()))
    }

    /// `replaceModelConditionState(c)`.
    pub fn replace_model_condition_state<A: W3dAssetSource>(&mut self, c: &ModelConditionFlags, env: &mut DrawEnv<'_, A>) {
        self.hide_headlights = !c.test(MODELCONDITION_NIGHT);
        if let Some(info) = self.find_best_info(c) {
            self.set_model_state(StateRef::Normal(info), env);
        }
    }

    /// `turretNamesDiffer(a, b)`.
    fn turret_names_differ(a: &ModelConditionInfo, b: &ModelConditionInfo) -> bool {
        if !a.turrets_valid() || !b.turrets_valid() {
            return true;
        }
        (0..MAX_TURRETS).any(|i| {
            a.turrets[i].turret_angle_name_key != b.turrets[i].turret_angle_name_key || a.turrets[i].turret_pitch_name_key != b.turrets[i].turret_pitch_name_key
        })
    }

    /// `nukeCurrentRender(xform)`.
    fn nuke_current_render(&mut self) {
        self.pause_animation = false;
        self.render_object = None;
    }

    /// `setModelState(newState)`.
    pub fn set_model_state<A: W3dAssetSource>(&mut self, new_state: StateRef, env: &mut DrawEnv<'_, A>) {
        let mut new_state = new_state;
        let mut next_state = None;
        if let Some(cur) = self.cur_state.clone() {
            if (cur == new_state && self.next_state.is_none()) || self.next_state.as_ref() == Some(&new_state) {
                // he's already got one, you see
                return;
            }
            let data = self.data.clone();
            let ns = data.state(&new_state);
            let cs = data.state(&cur);
            if new_state != cur
                && !ns.allow_to_finish_key.is_empty()
                && ns.allow_to_finish_key == cs.transition_key
                && self.render_object.is_some()
                && !is_animation_complete(self.render_object.as_ref())
            {
                self.next_state = Some(new_state);
                self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                return;
            } else if new_state != cur && !cs.transition_key.is_empty() && !ns.transition_key.is_empty() {
                let sig = (cs.transition_key.clone(), ns.transition_key.clone());
                if let Some(trans) = self.find_transition_for_sig(&sig) {
                    next_state = Some(new_state);
                    new_state = trans;
                }
            }
        }

        let prev_anim_fraction = self.get_current_anim_fraction();

        let data = self.data.clone();
        let ns = data.state(&new_state);

        // and always hide muzzle flashes, in case modelstate and model have crossed at the same time
        Self::hide_all_muzzle_flashes(Some(ns), self.render_object.as_mut());

        let remake = match &self.cur_state {
            None => true,
            Some(cur) => {
                let cs = data.state(cur);
                ns.model_name != cs.model_name || Self::turret_names_differ(ns, cs)
            }
        };
        if remake {
            self.nuke_current_render();
            if !ns.model_name.is_empty() {
                if let Some(hlod) = env.assets.create_render_obj(&ns.model_name) {
                    let skeleton = env.assets.skeleton(&ns.model_name, None, self.scale);
                    let mut r = RenderObj::new(&ns.model_name, hlod).with_skeleton(skeleton);
                    // preserve the visibility state of the previous render object
                    r.hidden = self.drawable_hidden;
                    self.render_object = Some(r);
                    self.render_obj_serial = self.render_obj_serial.wrapping_add(1);
                }
            }
        }
        let standard = self.standard_public_bones.clone();
        ns.validate_stuff(self.render_object.as_mut(), self.scale, &data.extra_public_bones, &standard, env);
        self.rebuild_weapon_recoil_info(Some(ns));
        self.do_hide_show_sub_objs(&ns.hide_show_vec);
        if remake && self.render_object.is_some() {
            self.on_render_obj_recreated();
        }
        self.hide_all_headlights(self.hide_headlights);

        let prev_state = self.cur_state.take();
        self.cur_state = Some(new_state);
        self.next_state = next_state;
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        self.adjust_animation(prev_state.as_ref(), prev_anim_fraction, env);
    }

    /// `getCurrentAnimFraction()`.
    pub fn get_current_anim_fraction(&self) -> f32 {
        let Some(cur) = self.get_cur_state() else { return -1.0 };
        if !is_any_maintain_frame_flag_set(cur.flags) {
            return -1.0;
        }
        let Some(r) = &self.render_object else { return -1.0 };
        if !r.is_hlod {
            return -1.0;
        }
        let Some(a) = &r.anim else { return -1.0 };
        let num_frames = a.motion.num_frames;
        if a.frame < 0.0 {
            0.0
        } else if a.frame >= num_frames as f32 {
            1.0
        } else {
            a.frame / (num_frames as f32 - 1.0)
        }
    }

    /// `adjustAnimation(prevState, prevAnimFraction)`.
    pub fn adjust_animation<A: W3dAssetSource>(&mut self, prev_state: Option<&StateRef>, prev_anim_fraction: f32, env: &mut DrawEnv<'_, A>) {
        let Some(cur_ref) = self.cur_state.clone() else { return };
        let data = self.data.clone();
        let cur = data.state(&cur_ref);
        let num_anims = cur.animations.len() as i32;
        if num_anims > 0 {
            if num_anims == 1 {
                self.which_anim_in_cur_state = 0;
            } else if prev_state == Some(&cur_ref) {
                // select an idle anim different than the currently playing one
                let anim_to_avoid = self.which_anim_in_cur_state;
                while self.which_anim_in_cur_state == anim_to_avoid {
                    self.which_anim_in_cur_state = env.random.game_client_random_value(0, num_anims - 1);
                }
            } else {
                self.which_anim_in_cur_state = env.random.game_client_random_value(0, num_anims - 1);
            }
            let anim_info = &cur.animations[self.which_anim_in_cur_state as usize];
            let anim_handle = anim_info.get_anim_handle(env.assets);
            if let (Some(r), Some(h)) = (&mut self.render_object, anim_handle) {
                let mut start_frame = 0;
                if cur.mode == AnimMode::OnceBackwards || cur.mode == AnimMode::LoopBackwards {
                    start_frame = h.num_frames - 1;
                }
                let prev = prev_state.map(|p| data.state(p));
                if test_flag_bit(cur.flags, RANDOMIZE_START_FRAME) {
                    start_frame = env.random.game_client_random_value(0, h.num_frames - 1);
                } else if test_flag_bit(cur.flags, START_FRAME_FIRST) {
                    start_frame = 0;
                } else if test_flag_bit(cur.flags, START_FRAME_LAST) {
                    start_frame = h.num_frames - 1;
                } else if let Some(p) = prev {
                    // order is important here: MAINTAIN_FRAME_ACROSS_STATES is overridden by the other bits, above.
                    if is_any_maintain_frame_flag_set(cur.flags)
                        && prev_state != Some(&cur_ref)
                        && is_any_maintain_frame_flag_set(p.flags)
                        && is_common_maintain_frame_flag_set(cur.flags, p.flags)
                        && prev_anim_fraction >= 0.0
                    {
                        start_frame = real_to_int(prev_anim_fraction * h.num_frames as f32 - 1.0);
                    }
                }
                r.set_animation(Some(h), start_frame as f32, cur.mode, env.sync_time);
                if r.is_hlod {
                    let factor = env.random.game_client_random_value_real(cur.anim_min_speed_factor, cur.anim_max_speed_factor);
                    r.set_animation_frame_rate_multiplier(factor);
                }
            }
        } else {
            self.which_anim_in_cur_state = -1;
        }
    }

    /// `setCurAnimDurationInMsec(desiredDurationInMsec)`.
    pub fn set_cur_anim_duration_in_msec(&mut self, desired_duration_in_msec: f32) -> bool {
        let Some(r) = &mut self.render_object else { return false };
        if !r.is_hlod {
            return false;
        }
        let Some(a) = &mut r.anim else { return false };
        let natural_duration_in_msec = a.motion.num_frames as f32 * 1000.0 / a.motion.frame_rate;
        if natural_duration_in_msec > 0.0 && desired_duration_in_msec > 0.0 {
            a.frame_rate_multiplier = natural_duration_in_msec / desired_duration_in_msec;
            return true;
        }
        false
    }

    /// `getCurAnimDistanceCovered()`.
    pub fn get_cur_anim_distance_covered(&self) -> f32 {
        match (&self.cur_state, self.which_anim_in_cur_state) {
            (Some(s), w) if w >= 0 => self.data.state(s).animations[w as usize].distance_covered,
            _ => 0.0,
        }
    }

    /// `adjustAnimSpeedToMovementSpeed()`.
    pub fn adjust_anim_speed_to_movement_speed(&mut self, speed: Option<f32>) {
        let dist = self.get_cur_anim_distance_covered();
        if dist > 0.0 {
            if let Some(speed) = speed {
                if speed > 0.0 {
                    let desired_duration_in_msec = dist / speed * MSEC_PER_LOGICFRAME_REAL;
                    self.set_cur_anim_duration_in_msec(desired_duration_in_msec);
                }
            }
        }
    }

    /// `setAnimationLoopDuration(numFrames)`.
    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.next_state_anim_loop_duration = NO_NEXT_DURATION;
        let desired_duration_in_msec = (num_frames as f32 * MSEC_PER_LOGICFRAME_REAL).ceil();
        self.set_cur_anim_duration_in_msec(desired_duration_in_msec);
    }

    /// `setAnimationCompletionTime(numFrames)`.
    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        let data = self.data.clone();
        let cur = self.cur_state.as_ref().map(|s| data.state(s));
        let next = self.next_state.as_ref().map(|s| data.state(s));
        if let (Some(c), Some(n)) = (cur, next) {
            if c.transition_sig.is_some() && !c.animations.is_empty() && n.transition_sig.is_none() && !n.animations.is_empty() {
                let t1 = c.animations[0].get_natural_duration_in_msec();
                let t2 = n.animations[0].get_natural_duration_in_msec();
                let trans_time = real_to_int_floor((num_frames as f32 * t1) / (t1 + t2)) as u32;
                self.set_animation_loop_duration(trans_time);
                self.next_state_anim_loop_duration = num_frames.wrapping_sub(trans_time);
                return;
            }
        }
        self.set_animation_loop_duration(num_frames);
    }

    /// `setAnimationFrame(frame)`: the current animation at `frame`, in the
    /// render object's default mode (`ANIM_MODE_ONCE`).
    pub fn set_animation_frame<A: W3dAssetSource>(&mut self, frame: i32, env: &mut DrawEnv<'_, A>) {
        if self.render_object.is_some() && self.which_anim_in_cur_state >= 0 {
            let data = self.data.clone();
            let Some(cur) = self.cur_state.as_ref().map(|s| data.state(s)) else { return };
            let h = cur.animations[self.which_anim_in_cur_state as usize].get_anim_handle(env.assets);
            if let Some(r) = &mut self.render_object {
                r.set_animation(h, frame as f32, AnimMode::Once, env.sync_time);
            }
        }
    }

    /// `setPauseAnimation(pauseAnim)`.
    pub fn set_pause_animation(&mut self, pause_anim: bool, now: u32) {
        if self.pause_animation == pause_anim {
            return;
        }
        self.pause_animation = pause_anim;
        let Some(r) = &mut self.render_object else { return };
        if !r.is_hlod {
            return;
        }
        if let Some(a) = r.anim.clone() {
            if self.pause_animation {
                self.animation_mode = a.anim_mode;
                r.set_animation(Some(a.motion), a.frame, AnimMode::Manual, now);
            } else {
                r.set_animation(Some(a.motion), a.frame, self.animation_mode, now);
            }
        }
    }

    /// `doDrawModule(transformMtx)`, the parts that choose and time the
    /// animation (the transform is the client's; see the module docs).
    pub fn do_draw_module<A: W3dAssetSource>(&mut self, facts: &DrawFacts, env: &mut DrawEnv<'_, A>) {
        if self.tank.is_some() && !self.tank_do_draw_module(facts) {
            return;
        }
        if self.truck.is_some() {
            self.truck_kind_do_draw_module(facts, env);
            return;
        }
        self.model_draw_do_draw_module(facts, env);
    }

    /// `W3DModelDraw::doDrawModule` (see [`W3DModelDraw::do_draw_module`]).
    pub(crate) fn model_draw_do_draw_module<A: W3dAssetSource>(&mut self, facts: &DrawFacts, env: &mut DrawEnv<'_, A>) {
        let should_animate = if self.data.animations_require_power { facts.should_animate_with_power } else { facts.should_animate_without_power };
        self.set_pause_animation(!should_animate, env.sync_time);

        if is_animation_complete(self.render_object.as_ref()) {
            if self.cur_state.is_some() {
                if let Some(next_state) = self.next_state.take() {
                    let next_duration = self.next_state_anim_loop_duration;
                    self.next_state_anim_loop_duration = NO_NEXT_DURATION;
                    self.set_model_state(next_state, env);
                    if next_duration != NO_NEXT_DURATION {
                        self.set_animation_loop_duration(next_duration);
                    }
                }
            }
            if self.render_object.is_some() && self.which_anim_in_cur_state != -1 {
                if let Some(cur_ref) = self.cur_state.clone() {
                    let data = self.data.clone();
                    let cur = data.state(&cur_ref);
                    if cur.animations[self.which_anim_in_cur_state as usize].is_idle_anim {
                        // (yes, that's right: pass curState for prevState)
                        self.adjust_animation(Some(&cur_ref), -1.0, env);
                    } else if test_flag_bit(cur.flags, RESTART_ANIM_WHEN_COMPLETE) {
                        self.adjust_animation(Some(&cur_ref), -1.0, env);
                    }
                }
            }
        }

        self.adjust_anim_speed_to_movement_speed(facts.speed);

        self.handle_client_turret_positioning(&facts.turrets);
        self.handle_client_recoil();
    }

    /// The identity of the current render object (`getRenderObject()` as a pointer).
    pub(crate) fn render_obj_id(&self) -> Option<u32> {
        self.render_object.as_ref().map(|_| self.render_obj_serial)
    }

    /// `onRenderObjRecreated()`: `W3DTankDraw`'s finds its treads again
    /// ([`W3DModelDraw::update_tread_objects`]), the trucks' their bones (and the tank truck's its treads,
    /// `crate::client::w3d_truck_draw`); the base class's does nothing.
    fn on_render_obj_recreated(&mut self) {
        if self.tank.is_some() {
            self.update_tread_objects();
        }
        if self.truck.is_some() {
            self.truck_on_render_obj_recreated();
        }
    }

    /// `W3DTankDraw::updateTreadObjects()`: the treads forgotten, then, when the module data has a
    /// `TreadAnimationRate` and there is a render object, each sub-object (up to
    /// `MAX_TREADS_PER_TANK` treads) that is a mesh named `<container>.TREADS*` (ignoring case)
    /// gives one tread per vertex material whose stage-0 mapper is a linear-offset mapper (its
    /// automatic scrolling turned off, `Set_UV_Offset_Delta(0, 0)`; the sub-object's user data set to
    /// the tread's settings, offset (0, 0)): `TREADSL*` a left tread, `TREADSR*` a right one, any
    /// other a middle one. `m_prevRenderObj` is the render object either way.
    ///
    /// Upstream checks the tread count against the maximum only between sub-objects, so a
    /// sub-object with more such materials than are left would write past `m_treads` (undefined);
    /// here the count stops at the maximum. The automatic scrolling is not drawn here at all (the
    /// client draws no mapper; `client/src/model.rs`), so turning it off is implicit.
    pub fn update_tread_objects(&mut self) {
        let rate = match &self.data.extra {
            DrawModuleExtra::Tank(t) => t.tread_animation_rate,
            _ => 0.0,
        };
        let id = self.render_obj_id();
        let (treads, overrides) = find_tread_objects(self.render_object.as_ref(), rate, true);
        if let Some(r) = &mut self.render_object {
            r.material_overrides = overrides;
        }
        if let Some(t) = &mut self.tank {
            t.treads = treads;
            t.prev_render_obj = id;
        }
    }

    /// `W3DTankDraw::doDrawModule(transformMtx)` up to its call of the base class's: `false` when
    /// it returns before (no render object, no object, no physics), so the base class's does not
    /// run either (kept: upstream's tank draw does not pause or advance its states then).
    ///
    /// - A render object other than the last one seen: [`W3DModelDraw::update_tread_objects`].
    /// - The tread debris (`startMoveDebris`, `stopMoveDebris`, the velocity and burst
    ///   multipliers) is not made: ~~no particle systems yet~~ **Corrected 2026-11-06 (GitHub #44):** the draw
    ///   modules do not reach the client's particle systems yet.
    /// - With treads: while turning and slower than `TreadPivotSpeedFraction` of the current
    ///   locomotor's speed, the treads move by the `TreadAnimationRate` (negative when turning
    ///   right, `TURN_NEGATIVE`) when the direction changed since the last draw (the cosine between
    ///   them more than 1e-5 from 1), and the direction is kept; else, moving under a motive force
    ///   at `TreadDriveSpeedFraction` or more of it, every tread's U offset goes down by the rate,
    ///   wrapped into [0, 1). The rate is per draw, not per unit of speed ("tread scrolling speed
    ///   was not directly tied into tank velocity because it looked odd").
    /// - Time frozen (`isTimeFrozen`, the script engine's freezes): the client has no frozen time
    ///   yet; not checked.
    /// - Without an AI upstream dereferences null for the locomotor speed; here the treads do not
    ///   move then.
    /// - The values sent to the render object: the treads' offsets, as the sub-objects' material
    ///   overrides (upstream's sub-objects point at them).
    fn tank_do_draw_module(&mut self, facts: &DrawFacts) -> bool {
        if self.render_object.is_none() {
            return false;
        }
        if self.render_obj_id() != self.tank.as_ref().and_then(|t| t.prev_render_obj) {
            self.update_tread_objects();
        }
        // get object from logic; get object physics state
        let Some(m) = facts.motion else { return false };
        let DrawModuleExtra::Tank(fields) = &self.data.extra else { return true };
        let (scroll, pivot, drive) = (fields.tread_animation_rate, fields.tread_pivot_speed_fraction, fields.tread_drive_speed_fraction);
        let Some(t) = &mut self.tank else { return true };
        //Update movement of treads
        if !t.treads.is_empty() {
            if let Some(max_speed) = m.cur_locomotor_speed {
                //For optimization sake, we only do complex tread scrolling when tank
                //is mostly stationary and turning
                if m.turning != TURN_NONE && m.velocity_magnitude / max_speed < pivot {
                    //Check if we have turned enough since last draw to require animation
                    let dir = m.unit_direction_2d;
                    let angle_to_goal = dir.0 * t.last_direction.0 + dir.1 * t.last_direction.1;
                    if crate::ww::wwmath::fabs(1.0 - angle_to_goal) > 0.00001 {
                        if m.turning == TURN_NEGATIVE {
                            //turning right
                            t.update_tread_positions(-scroll);
                        } else {
                            //turning left
                            t.update_tread_positions(scroll);
                        }
                    }
                    t.last_direction = dir; //update for next frame
                } else if m.is_motive && m.velocity_magnitude / max_speed >= drive {
                    //do simple scrolling based only on speed when tank is moving straight at high speed.
                    for tr in &mut t.treads {
                        let mut offset_u = tr.custom_uv_offset[0] - scroll;
                        // ensure coordinates of offset are in [0, 1] range:
                        offset_u -= crate::ww::wwmath::floor(offset_u);
                        tr.custom_uv_offset = [offset_u, 0.0];
                    }
                }
            }
            let offsets: Vec<(usize, [f32; 2])> = t.treads.iter().map(|tr| (tr.sub_object, tr.custom_uv_offset)).collect();
            if let Some(r) = &mut self.render_object {
                for (i, o) in offsets {
                    r.material_overrides.insert(i, o);
                }
            }
        }
        true
    }

    /// `handleClientTurretPositioning()`: each turret of the current state (valid at a logic
    /// time) with a bone: its angle bone captured and turned about Z by the logic's angle plus
    /// `TurretArtAngle`, its pitch bone about Y by minus the pitch plus `TurretArtPitch`.
    /// `turrets` is what the object's AI answers (see [`DrawFacts::turrets`]): read, never
    /// written ("This function must not EVER do ANYTHING which can in any way, shape, or form
    /// affect GameLogic").
    pub fn handle_client_turret_positioning(&mut self, turrets: &[Option<(f32, f32)>; MAX_TURRETS]) {
        let Some(cur_ref) = self.cur_state.clone() else { return };
        let data = self.data.clone();
        let cur = data.state(&cur_ref);
        if !cur.turrets_valid() {
            return;
        }
        for (tslot, logic) in turrets.iter().enumerate() {
            let tur = &cur.turrets[tslot];
            let (angle_bone, pitch_bone) = cur.turret_bones(tslot);
            if angle_bone != 0 || pitch_bone != 0 {
                let (mut turret_angle, mut turret_pitch) = logic.unwrap_or((0.0, 0.0));
                // do turret, if any
                if angle_bone != 0 {
                    turret_angle += tur.turret_art_angle;
                    let mut turret_xfrm = Matrix3D::IDENTITY;
                    turret_xfrm.rotate_z(turret_angle);
                    if let Some(r) = &mut self.render_object {
                        r.capture_bone(angle_bone);
                        r.control_bone(angle_bone, &turret_xfrm);
                    }
                }
                // do turret pitch, if any
                if pitch_bone != 0 {
                    turret_pitch += tur.turret_art_pitch;
                    let mut turret_pitch_xfrm = Matrix3D::IDENTITY;
                    turret_pitch_xfrm.rotate_y(-turret_pitch);
                    if let Some(r) = &mut self.render_object {
                        r.capture_bone(pitch_bone);
                        r.control_bone(pitch_bone, &turret_pitch_xfrm);
                    }
                }
            }
        }
    }

    /// `handleClientRecoil()`: each barrel of the current state's weapons (valid at a logic
    /// time): its muzzle flash shown only while the recoil starts; its recoil bone moved back
    /// along X by the shift, which grows by the rate (the rate damped by `RecoilDamping`) up to
    /// `MaxRecoil`, or until the rate is tiny, then settles back by `RecoilSettle` a draw. A
    /// barrel without a recoil bone goes idle. Upstream's own note: it assumes one call a frame
    /// but is called each frame the drawable is drawn.
    pub fn handle_client_recoil(&mut self) {
        let Some(cur_ref) = self.cur_state.clone() else { return };
        let data = self.data.clone();
        let cur = data.state(&cur_ref);
        let d = &*data;
        if !cur.barrels_valid() {
            return;
        }
        const TINY_RECOIL: f32 = 0.01;
        for wslot in 0..SLOTS {
            if !cur.has_recoil_bones_or_muzzle_flashes(wslot) {
                continue;
            }
            let barrels = cur.weapon_barrels(wslot);
            let count = barrels.len().min(self.weapon_recoil_info_vec[wslot].len());
            for (i, barrel) in barrels.iter().enumerate().take(count) {
                let recoil = &mut self.weapon_recoil_info_vec[wslot][i];
                if barrel.muzzle_flash_bone != 0 {
                    let hidden = recoil.state != RecoilState::RecoilStart;
                    barrel.set_muzzle_flash_hidden(self.render_object.as_mut(), hidden);
                }
                if barrel.recoil_bone != 0 {
                    match recoil.state {
                        RecoilState::Idle => {}
                        RecoilState::RecoilStart | RecoilState::Recoil => {
                            recoil.shift += recoil.recoil_rate;
                            recoil.recoil_rate *= d.recoil_damping; // recoil decelerates
                            if recoil.shift >= d.max_recoil {
                                recoil.shift = d.max_recoil;
                                recoil.state = RecoilState::Settle;
                            } else if recoil.recoil_rate.abs() < TINY_RECOIL {
                                recoil.state = RecoilState::Settle;
                            }
                        }
                        RecoilState::Settle => {
                            recoil.shift -= d.recoil_settle;
                            if recoil.shift <= 0.0 {
                                recoil.shift = 0.0;
                                recoil.state = RecoilState::Idle;
                            }
                        }
                    }
                    let mut gun_xfrm = Matrix3D::IDENTITY;
                    gun_xfrm.translate_x(-recoil.shift);
                    if let Some(r) = &mut self.render_object {
                        r.capture_bone(barrel.recoil_bone);
                        r.control_bone(barrel.recoil_bone, &gun_xfrm);
                    }
                } else {
                    recoil.state = RecoilState::Idle;
                }
            }
        }
    }

    /// `rebuildWeaponRecoilInfo(state)`: one cleared recoil per barrel of each slot (none
    /// without a state).
    pub fn rebuild_weapon_recoil_info(&mut self, state: Option<&ModelConditionInfo>) {
        for wslot in 0..SLOTS {
            match state {
                None => self.weapon_recoil_info_vec[wslot].clear(),
                Some(st) => {
                    let ncount = st.weapon_barrels(wslot).len();
                    let v = &mut self.weapon_recoil_info_vec[wslot];
                    v.resize(ncount, WeaponRecoilInfo::default());
                    for it in v.iter_mut() {
                        *it = WeaponRecoilInfo::default();
                    }
                }
            }
        }
    }

    /// `hideAllMuzzleFlashes(state, renderObject)`: every muzzle flash of the state's barrels
    /// (valid at a logic time) hidden.
    pub fn hide_all_muzzle_flashes(state: Option<&ModelConditionInfo>, render_object: Option<&mut RenderObj>) {
        let (Some(state), Some(r)) = (state, render_object) else { return };
        if !state.barrels_valid() {
            return;
        }
        for wslot in 0..SLOTS {
            if !state.has_recoil_bones_or_muzzle_flashes(wslot) {
                continue;
            }
            for it in state.weapon_barrels(wslot) {
                if it.muzzle_flash_bone != 0 {
                    it.set_muzzle_flash_hidden(Some(&mut *r), true);
                }
            }
        }
    }

    /// `getBarrelCount(wslot)`.
    pub fn get_barrel_count(&self, wslot: usize) -> usize {
        match self.get_cur_state() {
            Some(c) if c.barrels_valid() => c.weapon_barrels(wslot).len(),
            _ => 0,
        }
    }

    /// `handleWeaponFireFX(wslot, specificBarrelToUse, fxl, ...)`'s answer, whether it was handled
    /// (see [`W3DModelDraw::handle_weapon_fire_fx_at`] for where the effect plays).
    pub fn handle_weapon_fire_fx(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool) -> bool {
        self.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl).is_some()
    }

    /// `handleWeaponFireFX(wslot, specificBarrelToUse, fxl, ...)`: the barrel's recoil started and its muzzle flash
    /// shown; when it is handled (an FX list, the barrel's FX bone and a render object), where upstream's
    /// `FXList::doFXPos` plays: ~~not drawn yet~~ **since GitHub #43 (2026-11-06)** the caller plays it
    /// (`crate::client::client_fx`) at the bone the answer names, or at the logic object when the render object is
    /// hidden.
    pub fn handle_weapon_fire_fx_at(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool) -> Option<WeaponFireFxAt> {
        let cur = self.get_cur_state()?;
        if !cur.barrels_valid() || wslot >= SLOTS {
            return None;
        }
        let wbvec = cur.weapon_barrels(wslot);
        if wbvec.is_empty() {
            // some other module of our drawable may have handled it.
            return None;
        }
        let mut handled = None;
        // upstream tests `> size()`, so a barrel equal to the count is read past the end (undefined); here it is the first.
        let barrel = if specific_barrel_to_use < 0 || specific_barrel_to_use as usize >= wbvec.len() { 0 } else { specific_barrel_to_use as usize };
        let info = wbvec[barrel];
        if has_fxl && info.fx_bone != 0 {
            if let Some(r) = &self.render_object {
                // `m_renderObject->Get_Bone_Transform(info.m_fxBone)` unless hidden; the bone's pose as made (the
                // skeleton's), in the model's space
                let bone_transform = r.skeleton.as_ref().and_then(|k| k.bones.get(info.fx_bone as usize)).map(|b| b.transform);
                handled = Some(WeaponFireFxAt { bone_transform, render_hidden: r.hidden });
            }
        }
        if info.recoil_bone != 0 || info.muzzle_flash_bone != 0 {
            if let Some(recoil) = self.weapon_recoil_info_vec[wslot].get_mut(barrel) {
                recoil.state = RecoilState::RecoilStart;
                recoil.recoil_rate = self.data.initial_recoil;
            }
            if info.muzzle_flash_bone != 0 {
                info.set_muzzle_flash_hidden(self.render_object.as_mut(), false);
            }
        }
        handled
    }

    /// `doHideShowSubObjs(vec)`: each named sub-object hidden or shown, with every sub-object
    /// whose bone descends from its bone; then the `showSubObject` list over them
    /// ([`W3DModelDraw::update_sub_objects`]). A name not found is upstream's asset-error debug
    /// crash, skipped.
    pub fn do_hide_show_sub_objs(&mut self, vec: &[HideShowSubObjInfo]) {
        let Some(r) = &mut self.render_object else { return };
        for it in vec {
            Self::hide_show_one(r, &it.sub_obj_name, it.hide);
        }
        // This is really important as it allows a person to override modelcondition show/hide objects.
        if !self.sub_object_vec.is_empty() {
            self.update_sub_objects();
        }
    }

    /// One `Get_Sub_Object_By_Name` and its `Set_Hidden`, then `doHideShowBoneSubObjs` under its bone.
    fn hide_show_one(r: &mut RenderObj, name: &[u8], hide: bool) {
        let Some(k) = r.skeleton.clone() else { return };
        let Some(obj_index) = k.get_sub_object_by_name(name) else { return };
        r.set_sub_object_hidden(obj_index, hide);
        if r.has_htree() {
            // get the bone of this subobject so we can hide all other child objects that use this bone as a parent.
            let bone_idx = k.sub_objects[obj_index].bone_index;
            Self::do_hide_show_bone_sub_objs(hide, bone_idx, r, &k);
        }
    }

    /// `doHideShowBoneSubObjs(state, numSubObjects, boneIdx, fullObject, htree)`: every
    /// sub-object one of whose bone's ancestors (not its own bone) is `bone_idx`. The walk stops
    /// at bone 0, after testing it: with `bone_idx` 0, every sub-object not on bone 0 matches.
    fn do_hide_show_bone_sub_objs(state: bool, bone_idx: i32, r: &mut RenderObj, k: &Skeleton) {
        for (i, s) in k.sub_objects.iter().enumerate() {
            let mut is_child = false;
            let mut parent_bone_index = s.bone_index;
            let mut steps = 0;
            while parent_bone_index != 0 && steps <= k.bones.len() {
                parent_bone_index = k.get_parent_index(parent_bone_index);
                steps += 1;
                if parent_bone_index == bone_idx {
                    is_child = true;
                    break;
                }
            }
            if is_child {
                r.set_sub_object_hidden(i, state);
            }
        }
    }

    /// (#165) `isVisible()` (W3DModelDraw.cpp:3916-3919): a render object that `Is_Really_Visible()`. Here: one that is
    /// not hidden (`Set_Hidden`); the scene's view-frustum flag (`IS_VISIBLE`) is not kept (#165's gaps), so a render
    /// object off screen counts as visible.
    pub fn is_visible(&self) -> bool {
        self.render_object.as_ref().is_some_and(|r| !r.hidden)
    }

    /// (#165) `updateBonesForClientParticleSystems()` (W3DModelDraw.cpp:2677-2722): each bone particle system the module
    /// made (`m_particleSystemIDs`) found and moved to its bone; always true. The module makes no bone particle systems
    /// yet (the module docs: `ParticleSysBone`), so the list is empty and nothing moves.
    pub fn update_bones_for_client_particle_systems(&mut self) -> bool {
        true
    }

    /// (#165) `getCurrentWorldspaceClientBonePositions(boneName, transform)` (W3DModelDraw.cpp:3555-3566): with a render
    /// object, its `Get_Bone_Index(boneName)` (0: none, false) and `Get_Bone_Transform(index)`. Here the bone's transform
    /// is the skeleton's pose as made (as [`W3DModelDraw::handle_weapon_fire_fx_at`] reads it), put in the world by the
    /// drawable's transform `world`: the animation's current pose is not applied (#165's gaps).
    pub fn get_current_worldspace_client_bone_positions(&self, bone_name: &[u8], world: &Matrix3D) -> Option<Matrix3D> {
        let r = self.render_object.as_ref()?;
        let k = r.skeleton.as_ref()?;
        let bone_index = k.get_bone_index(bone_name);
        if bone_index == 0 {
            return None;
        }
        Some(Matrix3D::multiply(world, &k.bones[bone_index as usize].transform))
    }

    /// `showSubObject(name, show)`: kept in the list (`m_subObjectVec`), by name ignoring case;
    /// applied by [`W3DModelDraw::update_sub_objects`].
    pub fn show_sub_object(&mut self, name: &[u8], show: bool) {
        if name.is_empty() {
            return;
        }
        let mut found = false;
        for it in &mut self.sub_object_vec {
            if it.sub_obj_name.eq_ignore_ascii_case(name) {
                it.hide = !show;
                found = true;
            }
        }
        if !found {
            self.sub_object_vec.push(HideShowSubObjInfo { sub_obj_name: name.to_vec(), hide: !show });
        }
    }

    /// `updateSubObjects()`: the `showSubObject` list applied to the render object, each as
    /// `doHideShowSubObjs` applies a state's.
    pub fn update_sub_objects(&mut self) {
        let Some(r) = &mut self.render_object else { return };
        for it in &self.sub_object_vec {
            Self::hide_show_one(r, &it.sub_obj_name, it.hide);
        }
    }

    /// `hideAllHeadlights(hide)`: every sub-object whose name holds `HEADLIGHT` (case matters,
    /// `strstr`).
    pub fn hide_all_headlights(&mut self, hide: bool) {
        let Some(r) = &mut self.render_object else { return };
        let Some(k) = r.skeleton.clone() else { return };
        for (i, s) in k.sub_objects.iter().enumerate() {
            if s.name.windows(9).any(|w| w == b"HEADLIGHT") {
                r.set_sub_object_hidden(i, hide);
            }
        }
    }

    /// The render's own step (`Animatable3DObjClass::Render`): advance the
    /// animation to sync time `now`, unless hidden.
    pub fn render(&mut self, now: u32) {
        if let Some(r) = &mut self.render_object {
            if !r.hidden {
                r.render(now);
            }
        }
    }

    /// `setHidden(h)`: the render object's visibility.
    pub fn set_hidden(&mut self, h: bool) {
        self.drawable_hidden = h;
        if let Some(r) = &mut self.render_object {
            r.hidden = h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::model_state::*;
    use crate::common::thing_template::ClientLine;

    /// A `Draw` block as the logic keeps it, from `(depth, field, args)`.
    fn block(lines: &[(u32, &str, &str)]) -> ClientModuleData {
        ClientModuleData {
            module_tag_name_key: Default::default(),
            lines: lines
                .iter()
                .map(|(d, f, a)| ClientLine { depth: *d, field: f.as_bytes().to_vec(), args: a.split_whitespace().map(|t| t.as_bytes().to_vec()).collect() })
                .collect(),
        }
    }

    fn parse(lines: &[(u32, &str, &str)]) -> IniResult<W3DModelDrawModuleData> {
        W3DModelDrawModuleData::parse(b"W3DModelDraw", &block(lines)).map(|d| d.expect("a model draw"))
    }

    fn flags(bits: &[i32]) -> ModelConditionFlags {
        ModelConditionFlags::from_indices(bits)
    }

    const TANK: &[(u32, &str, &str)] = &[
        (0, "OkToChangeModelColor", "Yes"),
        (0, "DefaultConditionState", ""),
        (1, "Model", "AVTank"),
        (1, "Animation", "AVTank.Idle"),
        (1, "Turret", "Turret01"),
        (1, "TurretArtAngle", "90"),
        (1, "TransitionKey", "Up"),
        (0, "ConditionState", "MOVING"),
        (1, "Animation", "AVTank.Move 30"),
        (1, "AnimationMode", "LOOP"),
        (1, "TransitionKey", "Down"),
        (0, "ConditionState", "DAMAGED"),
        (1, "Flags", "RANDOMSTART MAINTAIN_FRAME_ACROSS_STATES"),
        (0, "AliasConditionState", "REALLYDAMAGED"),
        (0, "ConditionState", "FIRING_A"),
        (1, "Model", "None"),
        (1, "Animation", "AVTank.Fire 0 3"),
        (1, "WeaponLaunchBone", "PRIMARY Muzzle"),
        (1, "ParticleSysBone", "Smoke01 SmokeTrail"),
        (1, "AnimationSpeedFactorRange", "0.5 1.5"),
        (0, "TransitionState", "Up Down"),
        (1, "Animation", "AVTank.Lower"),
        (1, "AnimationMode", "ONCE_BACKWARDS"),
        (0, "IgnoreConditionStates", "NIGHT"),
        (0, "ExtraPublicBone", "Bone01 Bone02"),
        (0, "TrackMarks", "EXTnkTrack.tga"),
    ];

    /// Predictions, written before the first run (rung 1):
    /// - default state 0; four states (default, MOVING, DAMAGED+alias, FIRING_A) and one transition (`up`, `down`);
    /// - names are lower-cased: models `avtank`, animations `avtank.idle` ...; `OkToChangeModelColor` is on;
    /// - MOVING copies the default's model and turret (`turret01` at 90 degrees, in radians) but its own animation
    ///   replaces the default's: one `avtank.move`, distance 30, mode LOOP, key `down`;
    /// - DAMAGED keeps the default's animation (copied, so `avtank.idle`), its flags are bits 0 and 5, and it matches both
    ///   DAMAGED and REALLY_DAMAGED (two condition sets);
    /// - FIRING_A has no model (`None`), three copies of `avtank.fire`, a projectile bone (`muzzle`, also public, as is
    ///   `turret01` copied from the default), one particle bone `smoke01` with system `SmokeTrail`, speed range 0.5..1.5;
    /// - the transition starts from the default (model `avtank`, turret) with its keys cleared, one animation, ONCE_BACKWARDS;
    /// - the ignore mask is NIGHT; two extra public bones as written; track `extnktrack.tga`.
    ///
    /// **Missed on the first run (2026-11-05):** the block was refused, `INI_INVALID_DATA` at the alias. The test's data
    /// was wrong: the bit's INI name is `REALLYDAMAGED` (`ModelConditionFlags::s_bitNameList`), not `REALLY_DAMAGED`,
    /// and an unknown name is invalid data. Bisecting it found a real defect first: the logic's loader read
    /// `AliasConditionState` as a nested block (see `common::thing_template::CLIENT_NESTED_BLOCKS`), fixed with it.
    #[test]
    fn the_module_data_parses_as_upstream_reads_it() {
        let d = parse(TANK).unwrap();
        assert_eq!(d.default_state, 0);
        assert_eq!(d.condition_states.len(), 4);
        assert!(d.ok_to_change_model_color);
        let s = &d.condition_states;
        assert_eq!(s[0].model_name, b"avtank");
        assert_eq!(s[0].animations[0].name, b"avtank.idle");
        assert_eq!(s[0].conditions_yes_vec, [ModelConditionFlags::new()]);
        assert_eq!(s[1].model_name, b"avtank");
        assert_eq!(s[1].turrets[0].turret_angle_name_key, b"turret01");
        assert!((s[1].turrets[0].turret_art_angle - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(s[1].animations.len(), 1);
        assert_eq!((s[1].animations[0].name.as_slice(), s[1].animations[0].distance_covered), (&b"avtank.move"[..], 30.0));
        assert_eq!((s[1].mode, s[1].transition_key.as_slice()), (AnimMode::Loop, &b"down"[..]));
        assert_eq!(s[2].animations[0].name, b"avtank.idle");
        assert_eq!(s[2].flags, (1 << RANDOMIZE_START_FRAME) | (1 << MAINTAIN_FRAME_ACROSS_STATES));
        assert_eq!(s[2].conditions_yes_vec, [flags(&[MODELCONDITION_DAMAGED]), flags(&[MODELCONDITION_REALLY_DAMAGED])]);
        assert!(s[3].model_name.is_empty());
        assert_eq!(s[3].animations.len(), 3);
        assert!(s[3].has_projectile_bones());
        assert_eq!(s[3].weapon_projectile_launch_bone_name[0], b"muzzle");
        assert!(s[3].public_bones.contains(&b"muzzle".to_vec()) && s[3].public_bones.contains(&b"turret01".to_vec()));
        assert_eq!(s[3].particle_sys_bones, [ParticleSysBoneInfo { bone_name: b"smoke01".to_vec(), particle_system_template: b"SmokeTrail".to_vec() }]);
        assert_eq!((s[3].anim_min_speed_factor, s[3].anim_max_speed_factor), (0.5, 1.5));
        let t = &d.transition_map[&(b"up".to_vec(), b"down".to_vec())];
        assert_eq!(d.transition_map.len(), 1);
        assert_eq!((t.model_name.as_slice(), t.turrets[0].turret_angle_name_key.as_slice()), (&b"avtank"[..], &b"turret01"[..]));
        assert!(t.transition_key.is_empty() && t.allow_to_finish_key.is_empty());
        assert_eq!((t.animations.len(), t.mode), (1, AnimMode::OnceBackwards));
        assert_eq!(d.ignore_condition_states, flags(&[MODELCONDITION_NIGHT]));
        assert_eq!(d.extra_public_bones, [b"Bone01".to_vec(), b"Bone02".to_vec()]);
        assert_eq!(d.track_file, b"extnktrack.tga");
    }

    /// Predictions: each of upstream's asset errors refuses the block with `INI_INVALID_DATA`: a second default state,
    /// a default after a state, a first state with bits when there is no default, an ignored bit in a later state, a
    /// duplicate condition set (also through an alias), an alias first, no `Model`, idle and normal animations together,
    /// an idle animation that loops, a transition that loops or has a key, a transition between a state and itself. An
    /// unknown field is `INI_UNKNOWN_TOKEN`; a module that is not a model draw gives no data.
    #[test]
    fn asset_errors_refuse_the_block() {
        let kind = |lines: &[(u32, &str, &str)]| parse(lines).map(|_| ()).map_err(|e| e.kind);
        let def = (0, "DefaultConditionState", "");
        let m = (1, "Model", "M");
        let bad = [
            vec![def, m, def, m],
            vec![(0, "ConditionState", ""), m, def, m],
            vec![(0, "ConditionState", "MOVING"), m],
            vec![(0, "IgnoreConditionStates", "MOVING"), def, m, (0, "ConditionState", "MOVING"), m],
            vec![def, m, (0, "ConditionState", "MOVING"), m, (0, "ConditionState", "MOVING"), m],
            vec![def, m, (0, "ConditionState", "MOVING"), m, (0, "AliasConditionState", "MOVING")],
            vec![(0, "AliasConditionState", "MOVING")],
            vec![def],
            vec![def, m, (1, "Animation", "A"), (1, "IdleAnimation", "B")],
            vec![def, m, (1, "IdleAnimation", "B"), (1, "AnimationMode", "LOOP")],
            vec![def, m, (0, "TransitionState", "A B"), (1, "AnimationMode", "LOOP")],
            vec![def, m, (0, "TransitionState", "A B"), (1, "TransitionKey", "C")],
            vec![def, m, (0, "TransitionState", "A a")],
        ];
        for (i, b) in bad.iter().enumerate() {
            assert_eq!(kind(b), Err(IniErrorKind::InvalidData), "case {i}");
        }
        assert_eq!(kind(&[def, m, (1, "NoSuchField", "1")]), Err(IniErrorKind::UnknownToken));
        assert_eq!(kind(&[def, m, (0, "TreadAnimationRate", "1")]), Err(IniErrorKind::UnknownToken), "a tank field in a plain model draw");
        assert!(W3DModelDrawModuleData::parse(b"W3DDefaultDraw", &block(&[])).unwrap().is_none());
        // a state with no bits and no default is fine, as is "Model = None"
        assert!(parse(&[(0, "ConditionState", ""), (1, "Model", "None")]).unwrap().condition_states[0].model_name.is_empty());
    }

    /// Predictions: `W3DTankDraw` reads its tread fields after the base table's (a rate of 30 per second is 1 per frame;
    /// unset fields keep the constructor's, `TrackDebrisDirtLeft`); `W3DTruckDraw` its tire bones and multipliers;
    /// `W3DScienceModelDraw` its science by name; `W3DPoliceCarDraw` is a truck.
    #[test]
    fn the_subclasses_read_their_own_fields() {
        let def = [(0, "DefaultConditionState", ""), (1, "Model", "M")];
        let mut l = def.to_vec();
        l.extend([(0, "TreadAnimationRate", "30"), (0, "TreadDebrisRight", "Mud")]);
        let d = W3DModelDrawModuleData::parse(b"W3DTankDraw", &block(&l)).unwrap().unwrap();
        let DrawModuleExtra::Tank(t) = &d.extra else { panic!("{:?}", d.extra) };
        assert_eq!((t.tread_animation_rate, t.tread_debris_name_left.as_slice(), t.tread_debris_name_right.as_slice()), (1.0, &b"TrackDebrisDirtLeft"[..], &b"Mud"[..]));
        let mut l = def.to_vec();
        l.extend([(0, "LeftFrontTireBone", "Tire01"), (0, "TireRotationMultiplier", "0.2")]);
        let d = W3DModelDrawModuleData::parse(b"W3DPoliceCarDraw", &block(&l)).unwrap().unwrap();
        let DrawModuleExtra::Truck(t) = &d.extra else { panic!("{:?}", d.extra) };
        assert_eq!((t.strings["LeftFrontTireBone"].as_slice(), t.reals["TireRotationMultiplier"]), (&b"Tire01"[..], 0.2));
        let mut l = def.to_vec();
        l.push((0, "RequiredScience", "SCIENCE_Foo"));
        let d = W3DModelDrawModuleData::parse(b"W3DScienceModelDraw", &block(&l)).unwrap().unwrap();
        assert_eq!(d.extra, DrawModuleExtra::Science { required_science: b"SCIENCE_Foo".to_vec() });
    }

    /// Predictions: through the logic's own loader (the gate world with a template carrying a `Draw` block), the block
    /// kept as tokens parses to the same states: two states, the moving one's model `tankmove`; the logic's CRC after
    /// loading is that of the gate map without the client reading the block (reading changes nothing).
    #[test]
    fn a_template_s_draw_block_from_the_logic_s_loader() {
        let extra = "Object Drawn\n KindOf = VEHICLE\n Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = Tank\n  End\n  \
ConditionState = MOVING\n   Model = TankMove\n   Animation = TankMove.Roll\n   AnimationMode = LOOP\n  End\n End\nEnd\n";
        let mut w = crate::logic::world::gate_scenario::gate_map_world_with(extra);
        let before = w.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).unwrap();
        let t = w.statics.thing_factory.find_template(b"Drawn").expect("the template").data.clone();
        let n = &t.get_draw_module_info().nuggets()[0];
        let crate::common::thing_template::ModuleInfoData::Client(b) = &n.second else { panic!("client data") };
        let d = W3DModelDrawModuleData::parse(&n.first, b).unwrap().unwrap();
        assert_eq!(d.condition_states.len(), 2);
        assert_eq!(d.condition_states[1].model_name, b"tankmove");
        assert_eq!(d.condition_states[1].animations[0].name, b"tankmove.roll");
        assert_eq!(w.logic_crc(crate::logic::world::game_logic_update::CRC_RECALC, None).unwrap(), before);
    }

    // ---- rung 2: the choice and the animation

    /// The assets of the tests: HLODs by name, animations by name with their frame count and rate.
    struct Fake {
        anims: Vec<(&'static str, i32, f32)>,
    }

    impl W3dAssetSource for Fake {
        fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
            (!name.eq_ignore_ascii_case(b"missing")).then_some(true)
        }
        fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
            self.anims.iter().find(|a| a.0.as_bytes().eq_ignore_ascii_case(name)).map(|a| AnimHandle { name: a.0.as_bytes().to_vec(), num_frames: a.1, frame_rate: a.2 })
        }
    }

    fn fake() -> Fake {
        Fake {
            anims: vec![("t.idle", 10, 10.0), ("t.move", 20, 10.0), ("t.lower", 5, 10.0), ("t.a", 4, 10.0), ("t.b", 4, 10.0), ("t.c", 11, 10.0), ("t.d", 30, 30.0)],
        }
    }

    fn draw_of(lines: &[(u32, &str, &str)], assets: &mut Fake, random: &mut RandomValues) -> W3DModelDraw {
        let d = Arc::new(parse(lines).unwrap());
        let mut env = DrawEnv { assets, random, sync_time: 0, logic_time: true };
        W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
    }

    const FACTS: DrawFacts = DrawFacts { should_animate_with_power: true, should_animate_without_power: true, speed: None, turrets: [None, None], motion: None, show_client_physics: true, wheel_info: None };

    /// One client frame at `t` ms: draw, then render.
    fn frame(m: &mut W3DModelDraw, assets: &mut Fake, random: &mut RandomValues, t: u32) {
        let mut env = DrawEnv { assets, random, sync_time: t, logic_time: false };
        m.do_draw_module(&FACTS, &mut env);
        m.render(t);
    }

    fn set_flags(m: &mut W3DModelDraw, assets: &mut Fake, random: &mut RandomValues, t: u32, bits: &[i32]) {
        let mut env = DrawEnv { assets, random, sync_time: t, logic_time: false };
        m.replace_model_condition_state(&flags(bits), &mut env);
    }

    fn anim(m: &W3DModelDraw) -> (String, f32) {
        let a = m.render_object.as_ref().and_then(|r| r.anim.as_ref()).expect("an animation");
        (String::from_utf8_lossy(&a.motion.name).into_owned(), a.frame)
    }

    /// Predictions, written before the first run, for `findBestInfo` (the ignore mask is NIGHT):
    /// no bits: the default (0); MOVING: 1; MOVING+NIGHT: 1 (NIGHT cleared); DAMAGED+MOVING: 3; REALLYDAMAGED+MOVING:
    /// 1 (MOVING and the DAMAGED state's alias both match one bit with no extra, and the first found wins); FIRING_A+
    /// MOVING+ATTACKING: 4 (4 and 5 tie at two bits, the first wins); FIRING_A alone: 4 (one bit beats none, though one
    /// extraneous); SNOW alone: the default (no state matches, the default has no extraneous bit).
    #[test]
    fn find_best_info_matches_as_upstream() {
        let m = |s: &'static str| (1, "Model", s);
        let d = parse(&[
            (0, "IgnoreConditionStates", "NIGHT"),
            (0, "DefaultConditionState", ""),
            m("D"),
            (0, "ConditionState", "MOVING"),
            (0, "ConditionState", "DAMAGED"),
            (0, "AliasConditionState", "REALLYDAMAGED"),
            (0, "ConditionState", "DAMAGED MOVING"),
            (0, "ConditionState", "MOVING FIRING_A"),
            (0, "ConditionState", "MOVING ATTACKING"),
        ])
        .unwrap();
        let f = |b: &[i32]| d.find_best_info(&flags(b));
        assert_eq!(f(&[]), Some(0));
        assert_eq!(f(&[MODELCONDITION_MOVING]), Some(1));
        assert_eq!(f(&[MODELCONDITION_MOVING, MODELCONDITION_NIGHT]), Some(1));
        assert_eq!(f(&[MODELCONDITION_DAMAGED, MODELCONDITION_MOVING]), Some(3));
        assert_eq!(f(&[MODELCONDITION_REALLY_DAMAGED, MODELCONDITION_MOVING]), Some(1));
        assert_eq!(f(&[MODELCONDITION_FIRING_A, MODELCONDITION_MOVING, MODELCONDITION_ATTACKING]), Some(4));
        assert_eq!(f(&[MODELCONDITION_FIRING_A]), Some(4));
        assert_eq!(f(&[MODELCONDITION_SNOW]), Some(0));
        // the cache answers the same the second time
        assert_eq!(f(&[MODELCONDITION_FIRING_A]), Some(4));
        assert_eq!(d.get_best_model_name_for_wb(&flags(&[MODELCONDITION_MOVING])), b"d");
    }

    /// Predictions: from the default (key `up`, `t.idle` looping), MOVING (key `down`) plays the transition `up -> down`
    /// first (`t.lower`, 5 frames at 10 a second, ONCE), MOVING pending; 250 ms in it is at frame 2.5 and still
    /// running; at 1000 ms it has stopped at frame 4, and the next draw starts MOVING (`t.move` from frame 0, nothing
    /// pending). Back to no bits: no transition `down -> up` exists, so `t.idle` at once. Asking for the current state
    /// again changes nothing (the frame keeps running).
    #[test]
    fn a_transition_state_plays_between_two_keyed_states() {
        let (mut a, mut r) = (fake(), RandomValues::new(0));
        let mut m = draw_of(
            &[
                (0, "DefaultConditionState", ""),
                (1, "Model", "T"),
                (1, "Animation", "T.Idle"),
                (1, "AnimationMode", "LOOP"),
                (1, "TransitionKey", "Up"),
                (0, "ConditionState", "MOVING"),
                (1, "Animation", "T.Move"),
                (1, "TransitionKey", "Down"),
                (0, "TransitionState", "Up Down"),
                (1, "Animation", "T.Lower"),
                (1, "AnimationMode", "ONCE"),
            ],
            &mut a,
            &mut r,
        );
        assert_eq!(m.cur_state_ref(), Some(&StateRef::Normal(0)));
        assert_eq!(anim(&m).0, "t.idle");
        set_flags(&mut m, &mut a, &mut r, 0, &[MODELCONDITION_MOVING]);
        let sig = (b"up".to_vec(), b"down".to_vec());
        assert_eq!(m.cur_state_ref(), Some(&StateRef::Transition(sig)));
        assert_eq!(m.next_state_ref(), Some(&StateRef::Normal(1)));
        assert_eq!(anim(&m), ("t.lower".into(), 0.0));
        frame(&mut m, &mut a, &mut r, 250);
        assert_eq!(anim(&m), ("t.lower".into(), 2.5));
        frame(&mut m, &mut a, &mut r, 1000);
        assert_eq!(anim(&m), ("t.lower".into(), 4.0));
        assert!(is_animation_complete(m.render_object.as_ref()));
        frame(&mut m, &mut a, &mut r, 1033);
        assert_eq!(m.cur_state_ref(), Some(&StateRef::Normal(1)));
        assert_eq!(m.next_state_ref(), None);
        assert_eq!(anim(&m).0, "t.move");
        set_flags(&mut m, &mut a, &mut r, 1100, &[MODELCONDITION_MOVING]);
        assert_eq!(m.cur_state_ref(), Some(&StateRef::Normal(1)), "the same state again: ignored");
        set_flags(&mut m, &mut a, &mut r, 1100, &[]);
        assert_eq!((m.cur_state_ref(), anim(&m).0.as_str()), (Some(&StateRef::Normal(0)), "t.idle"));
    }

    /// Predictions: a state with two idle animations (ONCE, 4 frames at 10 a second) picks one at random; each time it
    /// completes, the next draw picks the other (never the one that just played), so six completions alternate. A state
    /// with `RESTART_ANIM_WHEN_COMPLETE` restarts its one animation from frame 0; one without stays on its last frame.
    #[test]
    fn idle_animations_alternate_and_restart() {
        let (mut a, mut r) = (fake(), RandomValues::new(7));
        let mut m = draw_of(&[(0, "DefaultConditionState", ""), (1, "Model", "T"), (1, "IdleAnimation", "T.A"), (1, "IdleAnimation", "T.B")], &mut a, &mut r);
        let mut seen = vec![anim(&m).0];
        let mut t = 0;
        for _ in 0..6 {
            t += 300; // 3 frames: complete
            frame(&mut m, &mut a, &mut r, t);
            t += 33;
            frame(&mut m, &mut a, &mut r, t);
            seen.push(anim(&m).0);
        }
        for w in seen.windows(2) {
            assert_ne!(w[0], w[1], "{seen:?}");
        }
        for (flags_line, restarted) in [("RESTART_ANIM_WHEN_COMPLETE", true), ("NONE", false)] {
            let mut m = draw_of(&[(0, "DefaultConditionState", ""), (1, "Model", "T"), (1, "Animation", "T.A"), (1, "Flags", flags_line)], &mut a, &mut r);
            frame(&mut m, &mut a, &mut r, 300);
            assert_eq!(anim(&m).1, 3.0);
            frame(&mut m, &mut a, &mut r, 333);
            assert_eq!(anim(&m).1, if restarted { 0.0 } else { 3.0 }, "{flags_line}");
        }
    }

    /// Predictions, 10 frames at 10 a second (last frame 9): ONCE_BACKWARDS starts at 9 and after 1000 ms is at 0,
    /// complete; LOOP from 0 after 1100 ms is at 11 - 9 = 2; MANUAL stays at 0; LOOP_PINGPONG after 1200 ms is at
    /// 2 * 9 - 12 = 6, going backwards, and after 700 ms more at 6 - 7 = -1, reflected to 1, forwards again;
    /// LOOP_BACKWARDS starts at 9 and after 1100 ms is at -2 + 9 = 7.
    ///
    /// **Missed on the first run (2026-11-05), in the test's exactness only:** LOOP gave 2.000001, not 2: the step is
    /// `10 * 1 * 1 * 1100 * 0.001` in `f32` (upstream's own sum, `Compute_Current_Frame`), which is 11.000001. The
    /// looping cases are compared to 1e-4 since.
    #[test]
    fn animation_modes_advance_as_compute_current_frame() {
        let (mut a, mut r) = (fake(), RandomValues::new(0));
        let mut run = |mode: &str, steps: &[u32]| {
            let mut m = draw_of(&[(0, "DefaultConditionState", ""), (1, "Model", "T"), (1, "Animation", "T.Idle"), (1, "AnimationMode", mode)], &mut a, &mut r);
            let mut out = vec![anim(&m).1];
            for &t in steps {
                m.render(t);
                out.push(anim(&m).1);
            }
            (out, m)
        };
        let close = |a: &[f32], b: &[f32]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
        let (f, m) = run("ONCE_BACKWARDS", &[1000]);
        assert_eq!(f, [9.0, 0.0]);
        assert!(is_animation_complete(m.render_object.as_ref()));
        let f = run("LOOP", &[1100]).0;
        assert!(close(&f, &[0.0, 2.0]), "{f:?}");
        assert_eq!(run("MANUAL", &[1100]).0, [0.0, 0.0]);
        let (f, m) = run("LOOP_PINGPONG", &[1200, 1900]);
        assert!(close(&f, &[0.0, 6.0, 1.0]), "{f:?}");
        assert_eq!(m.render_object.as_ref().unwrap().anim.as_ref().unwrap().anim_direction, 1.0);
        let f = run("LOOP_BACKWARDS", &[1100]).0;
        assert!(close(&f, &[9.0, 7.0]), "{f:?}");
    }

    /// Predictions: a state whose `WaitForStateToFinishIfPossible` is the current state's `TransitionKey` waits while the
    /// current ONCE animation runs (pending), then starts when it completes; requested after completion it starts at
    /// once. `MAINTAIN_FRAME_ACROSS_STATES` on both states carries the fraction: at frame 5 of 11 (5/10 = 0.5) the next
    /// state's 11-frame animation starts at `REAL_TO_INT(0.5 * 11 - 1)` = 4.
    #[test]
    fn waiting_states_and_maintained_frames() {
        let (mut a, mut r) = (fake(), RandomValues::new(0));
        let lines = [
            (0, "DefaultConditionState", ""),
            (1, "Model", "T"),
            (1, "Animation", "T.A"),
            (1, "TransitionKey", "Busy"),
            (0, "ConditionState", "MOVING"),
            (1, "Animation", "T.B"),
            (1, "WaitForStateToFinishIfPossible", "Busy"),
        ];
        let mut m = draw_of(&lines, &mut a, &mut r);
        frame(&mut m, &mut a, &mut r, 100);
        set_flags(&mut m, &mut a, &mut r, 100, &[MODELCONDITION_MOVING]);
        assert_eq!((m.cur_state_ref(), m.next_state_ref()), (Some(&StateRef::Normal(0)), Some(&StateRef::Normal(1))));
        frame(&mut m, &mut a, &mut r, 400); // frame 3 of 4: complete after this render
        frame(&mut m, &mut a, &mut r, 433);
        assert_eq!((m.cur_state_ref(), anim(&m).0.as_str()), (Some(&StateRef::Normal(1)), "t.b"));

        let lines = [
            (0, "DefaultConditionState", ""),
            (1, "Model", "T"),
            (1, "Animation", "T.C"),
            (1, "AnimationMode", "LOOP"),
            (1, "Flags", "MAINTAIN_FRAME_ACROSS_STATES"),
            (0, "ConditionState", "MOVING"),
        ];
        let mut m = draw_of(&lines, &mut a, &mut r);
        m.render(500); // frame 5
        assert_eq!(anim(&m).1, 5.0);
        set_flags(&mut m, &mut a, &mut r, 500, &[MODELCONDITION_MOVING]);
        assert_eq!(anim(&m), ("t.c".into(), 4.0));
    }

    /// Predictions: an animation of 30 frames at 30 a second (1000 ms) covering 20 units, at 2 units a logic frame,
    /// should last 10 logic frames (333.33 ms): multiplier 3. Paused (the object should not animate), the mode becomes
    /// MANUAL and the frame stops; resumed, the mode it had (LOOP) comes back.
    #[test]
    fn speed_follows_the_distance_covered_and_pause_freezes() {
        let (mut a, mut r) = (fake(), RandomValues::new(0));
        let lines = [(0, "DefaultConditionState", ""), (1, "Model", "T"), (1, "Animation", "T.D 20"), (1, "AnimationMode", "LOOP")];
        let mut m = draw_of(&lines, &mut a, &mut r);
        let mut env = DrawEnv { assets: &mut a, random: &mut r, sync_time: 0, logic_time: false };
        m.do_draw_module(&DrawFacts { speed: Some(2.0), ..FACTS }, &mut env);
        let mult = m.render_object.as_ref().unwrap().anim.as_ref().unwrap().frame_rate_multiplier;
        assert!((mult - 3.0).abs() < 1e-5, "{mult}");
        let paused = DrawFacts { should_animate_with_power: false, ..FACTS };
        m.do_draw_module(&paused, &mut env);
        assert_eq!(m.render_object.as_ref().unwrap().anim.as_ref().unwrap().anim_mode, AnimMode::Manual);
        m.render(500);
        assert_eq!(anim(&m).1, 0.0);
        m.do_draw_module(&FACTS, &mut env);
        assert_eq!(m.render_object.as_ref().unwrap().anim.as_ref().unwrap().anim_mode, AnimMode::Loop);
    }

    /// Predictions: a draw module with no state for no flags (the first state needs bits, refused at parse) cannot be
    /// built; one whose model is missing from the assets has no render object, so its animation is "complete" at once
    /// and a pending state starts on the next draw.
    #[test]
    fn missing_models_and_idle_state() {
        let (mut a, mut r) = (fake(), RandomValues::new(0));
        let m = draw_of(&[(0, "DefaultConditionState", ""), (1, "Model", "Missing"), (1, "Animation", "T.A")], &mut a, &mut r);
        assert!(m.render_object.is_none());
        assert_eq!(m.which_anim_in_cur_state(), 0);
        assert!(is_animation_complete(m.render_object.as_ref()));
    }

    // ---- GitHub #27: the bone caches, turrets, recoil, muzzle flashes, sub-objects

    mod bones {
        use super::*;
        use crate::common::file_system::{FileSystem, MemoryDevice};
        use crate::formats::w3d::assetmgr::W3dFileFactory;
        use crate::formats::w3d::synth::*;
        use crate::ww::vector3::Vector3;

        /// A turreted gun `GUNT`: pivots ROOTTRANSFORM, TURRET (0, 0, 2) under the root, BARREL (1, 0, 0) under the
        /// turret, RECOIL01 (2, 0, 0) under the barrel, MUZZLEFX01 (1, 0, 0) under the recoil bone; a cube on each
        /// (CHASSIS on the root, TURRET, BARREL, GUN on RECOIL01, MUZZLEFX01), and ANTENNA on the turret's bone and
        /// HEADLIGHT on the root.
        fn gunt() -> Vec<u8> {
            let ident = [0.0, 0.0, 0.0, 1.0];
            let p = |n: &[u8], parent: i32, t: [f32; 3]| PivotSpec { name: n.to_vec(), parent, translation: t, rotation: ident };
            let mut f = htree_w3d(b"GUNT", &[
                p(b"ROOTTRANSFORM", -1, [0.0; 3]),
                p(b"TURRET", 0, [0.0, 0.0, 2.0]),
                p(b"BARREL", 1, [1.0, 0.0, 0.0]),
                p(b"RECOIL01", 2, [2.0, 0.0, 0.0]),
                p(b"MUZZLEFX01", 3, [1.0, 0.0, 0.0]),
            ]);
            let subs: [(&[u8], u32); 7] =
                [(b"CHASSIS", 0), (b"TURRET", 1), (b"BARREL", 2), (b"GUN", 3), (b"MUZZLEFX01", 4), (b"ANTENNA", 1), (b"HEADLIGHT", 0)];
            let mut names = Vec::new();
            for (n, _) in subs {
                let mut m = MeshSpec::cuboid(b"GUNT", n, [-0.5; 3], [0.5; 3]);
                m.stage0.texture = None;
                f.extend(m.to_w3d());
                let mut full = b"GUNT.".to_vec();
                full.extend_from_slice(n);
                names.push(full);
            }
            let hl: Vec<(&[u8], u32)> = names.iter().zip(subs).map(|(n, (_, b))| (n.as_slice(), b)).collect();
            f.extend(hlod_w3d(b"GUNT", b"GUNT", &hl));
            f
        }

        fn files() -> FileSystem<MemoryDevice> {
            let mut d = MemoryDevice::new();
            d.insert(b"Art/W3D/gunt.w3d", gunt());
            FileSystem::new(d)
        }

        const GUN_DRAW: &[(u32, &str, &str)] = &[
            (0, "DefaultConditionState", ""),
            (1, "Model", "GUNT"),
            (1, "Turret", "Turret"),
            (1, "TurretArtAngle", "90"),
            (1, "TurretPitch", "Barrel"),
            (1, "WeaponRecoilBone", "PRIMARY Recoil"),
            (1, "WeaponMuzzleFlash", "PRIMARY MuzzleFX"),
            (1, "WeaponFireFXBone", "PRIMARY MuzzleFX"),
            (0, "ConditionState", "DAMAGED"),
            (1, "HideSubObject", "Turret"),
            (0, "ConditionState", "NIGHT"),
            (1, "Model", "GUNT"),
        ];

        /// Sub-object indices of [`gunt`].
        const CHASSIS: usize = 0;
        const TURRET: usize = 1;
        const BARREL: usize = 2;
        const GUN: usize = 3;
        const MUZZLE: usize = 4;
        const ANTENNA: usize = 5;
        const HEADLIGHT: usize = 6;

        struct Rig {
            fs: FileSystem<MemoryDevice>,
            am: AssetManager,
            random: RandomValues,
        }

        impl Rig {
            fn new() -> Self {
                Rig { fs: files(), am: AssetManager::new_game(W3dFileFactory::default()), random: RandomValues::new(0) }
            }
            fn draw(&mut self, logic_time: bool) -> W3DModelDraw {
                let d = Arc::new(parse(GUN_DRAW).unwrap());
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time };
                W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
            }
            fn frame(&mut self, m: &mut W3DModelDraw, turrets: [Option<(f32, f32)>; 2]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false };
                m.do_draw_module(&DrawFacts { turrets, ..FACTS }, &mut env);
            }
            fn set_flags(&mut self, m: &mut W3DModelDraw, bits: &[i32]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false };
                m.replace_model_condition_state(&flags(bits), &mut env);
            }
            /// The render model the client draws for `m`, with `m`'s controls and visibility, posed.
            fn posed(&mut self, m: &W3DModelDraw) -> RenderModel {
                let mut model = RenderModel::create(&mut self.am, &self.fs, b"gunt").unwrap().unwrap();
                m.render_object.as_ref().unwrap().pose_render_model(&mut model);
                model.update::<AssetManager>(&Matrix3D::IDENTITY, None).unwrap();
                model
            }
        }

        fn hidden(m: &W3DModelDraw) -> Vec<bool> {
            m.render_object.as_ref().unwrap().sub_object_hidden.clone()
        }

        fn near(a: Vector3, b: [f32; 3]) -> bool {
            (a.x - b[0]).abs() < 1e-5 && (a.y - b[1]).abs() < 1e-5 && (a.z - b[2]).abs() < 1e-5
        }

        /// Predictions, written before the first run:
        /// - the pristine bones (made in the constructor, a logic time) hold `turret` (1), `barrel` (2, at (1, 0, 2)),
        ///   `recoil01` (3) and `muzzlefx01` (4), and not the unnumbered `recoil` or `muzzlefx` (no such bones, and
        ///   numbered ones were found);
        /// - turret 0's bones are (1, 2), turret 1's (0, 0); the primary slot has one barrel: recoil bone 3, muzzle
        ///   flash and FX bone 4, projectile offset the identity; it has recoil bones or muzzle flashes;
        /// - `Get_Sub_Object_By_Name("turret")` finds `GUNT.TURRET` by its suffix.
        #[test]
        fn the_pristine_bones_turrets_and_barrels_are_cached_at_a_logic_time() {
            let mut rig = Rig::new();
            let m = rig.draw(true);
            let s = m.get_cur_state().unwrap();
            assert!(s.pristine_bones_valid() && s.turrets_valid() && s.barrels_valid());
            let c = s.bone_cache();
            let keys: Vec<&[u8]> = c.pristine_bones.keys().map(|k| k.as_slice()).collect();
            assert_eq!(keys, vec![&b"barrel"[..], b"muzzlefx01", b"recoil01", b"turret"]);
            assert_eq!(c.pristine_bones[&b"recoil01"[..]].bone_index, 3);
            assert!(near(c.pristine_bones[&b"barrel"[..]].mtx.get_translation(), [1.0, 0.0, 2.0]));
            assert_eq!(c.turret_bones, [(1, 2), (0, 0)]);
            assert_eq!(c.weapon_barrel_info_vec[0], vec![WeaponBarrelInfo { recoil_bone: 3, fx_bone: 4, muzzle_flash_bone: 4, projectile_offset_mtx: Matrix3D::IDENTITY }]);
            assert_eq!(c.has_recoil_bones_or_muzzle_flashes, [true, false, false]);
            let k = m.render_object.as_ref().unwrap().skeleton.clone().unwrap();
            assert_eq!(k.get_sub_object_by_name(b"turret"), Some(TURRET));
            assert_eq!(k.get_bone_index(b"recoil01"), 3);
            assert_eq!(m.weapon_recoil_info_vec[0], vec![WeaponRecoilInfo::default()]);
        }

        /// Predictions, written before the first run:
        /// - the logic's turret at angle 0.5 and pitch 0.25: bone 1 captured and turned about Z by 0.5 + pi/2 (the art
        ///   angle of 90 degrees), bone 2 captured and turned about Y by -0.25; posed, the barrel's bone is at
        ///   (0, 0, 2) + Rz(0.5 + pi/2) (1, 0, 0) = (-0.479426, 0.877583, 2);
        /// - with no AI answer the art angle alone turns it: the barrel at (0, 1, 2);
        /// - a module made outside a logic time has no pristine bones, its turrets are not valid, nothing is captured.
        #[test]
        fn the_turret_bones_follow_the_logic_s_angle_and_pitch() {
            let mut rig = Rig::new();
            let mut m = rig.draw(true);
            rig.frame(&mut m, [Some((0.5, 0.25)), None]);
            let r = m.render_object.as_ref().unwrap();
            let mut rz = Matrix3D::IDENTITY;
            rz.rotate_z(0.5 + std::f32::consts::FRAC_PI_2);
            let mut ry = Matrix3D::IDENTITY;
            ry.rotate_y(-0.25);
            assert_eq!(r.bone_controls[&1], BoneControl { captured: true, transform: rz, world_space_translation: false });
            assert_eq!(r.bone_controls[&2], BoneControl { captured: true, transform: ry, world_space_translation: false });
            let model = rig.posed(&m);
            assert!(near(model.bone_transform(2).unwrap().get_translation(), [-0.479426, 0.877583, 2.0]), "{:?}", model.bone_transform(2).unwrap().get_translation());

            rig.frame(&mut m, [None, None]);
            let model = rig.posed(&m);
            assert!(near(model.bone_transform(2).unwrap().get_translation(), [0.0, 1.0, 2.0]), "{:?}", model.bone_transform(2).unwrap().get_translation());

            let mut rig = Rig::new();
            let mut m = rig.draw(false);
            rig.frame(&mut m, [Some((0.5, 0.25)), None]);
            let s = m.get_cur_state().unwrap();
            assert!(!s.pristine_bones_valid() && !s.turrets_valid() && !s.barrels_valid());
            assert!(m.render_object.as_ref().unwrap().bone_controls.is_empty());
        }

        /// Predictions, written before the first run (`InitialRecoil` 2, `RecoilDamping` 0.4, `MaxRecoil` 3,
        /// `RecoilSettle` 0.065, the defaults):
        /// - as made, the muzzle flash is shown (the muzzle flashes are hidden on the *old* render object, before the
        ///   new one is made: upstream's order) and the headlight hidden (`m_hideHeadlights` starts true); after one
        ///   draw the flash is hidden (idle recoil);
        /// - a shot without an FX list is not handled (false) but starts the recoil and shows the flash;
        /// - the shift goes 2, 2.8, then 3 (capped, settling); the state stays `RECOIL_START` until then (upstream never
        ///   enters `RECOIL`), so the flash shows for those three draws and hides on the fourth, when the shift is
        ///   2.935; the recoil bone is moved back along X by the shift;
        /// - it settles to 0 and idle on draw 50.
        #[test]
        fn a_shot_recoils_the_barrel_and_flashes_the_muzzle() {
            let mut rig = Rig::new();
            let mut m = rig.draw(true);
            assert!(!hidden(&m)[MUZZLE] && hidden(&m)[HEADLIGHT]);
            rig.frame(&mut m, [None, None]);
            assert!(hidden(&m)[MUZZLE]);
            assert_eq!(m.get_barrel_count(0), 1);

            assert!(!m.handle_weapon_fire_fx(0, 0, false));
            assert_eq!(m.weapon_recoil_info_vec[0][0], WeaponRecoilInfo { state: RecoilState::RecoilStart, shift: 0.0, recoil_rate: 2.0 });
            assert!(!hidden(&m)[MUZZLE]);
            let mut shifts = Vec::new();
            let mut flash = Vec::new();
            let mut idle_at = None;
            for f in 1..=60 {
                rig.frame(&mut m, [None, None]);
                let rc = m.weapon_recoil_info_vec[0][0];
                shifts.push(rc.shift);
                flash.push(!hidden(&m)[MUZZLE]);
                if rc.state == RecoilState::Idle && idle_at.is_none() {
                    idle_at = Some(f);
                }
            }
            assert_eq!(&flash[..5], &[true, true, true, false, false]);
            assert!((shifts[0] - 2.0).abs() < 1e-6 && (shifts[1] - 2.8).abs() < 1e-6 && shifts[2] == 3.0 && (shifts[3] - 2.935).abs() < 1e-6, "{shifts:?}");
            let mut back = Matrix3D::IDENTITY;
            back.translate_x(-shifts[59]);
            assert_eq!(idle_at, Some(50));
            assert_eq!(m.render_object.as_ref().unwrap().bone_controls[&3].transform, back);
            // with an FX list, the FX bone and a render object, it is handled
            assert!(m.handle_weapon_fire_fx(0, 0, true));
        }

        /// The browser's asset source in small (GitHub #42): the files of [`gunt`], answering every question "none for
        /// now" (counted, as `Deferred` counts them) until `arrived`, or only the skeleton when `skeleton_only`.
        struct Fetching<'a> {
            files: W3dFiles<'a, MemoryDevice>,
            arrived: bool,
            skeleton_only: bool,
            deferred: usize,
        }

        impl W3dAssetSource for Fetching<'_> {
            fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
                if !self.arrived && !self.skeleton_only {
                    self.deferred += 1;
                    return None;
                }
                self.files.create_render_obj(name)
            }
            fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle> {
                if !self.arrived && !self.skeleton_only {
                    self.deferred += 1;
                    return None;
                }
                self.files.get_hanim(name)
            }
            fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
                if !self.arrived {
                    self.deferred += 1;
                    return None;
                }
                self.files.skeleton(name, pose, scale)
            }
            fn deferred_answers(&self) -> usize {
                self.deferred
            }
        }

        /// GitHub #42: bones validated while the model's files are being fetched are looked for again once they are
        /// there. Each case makes a drawable of [`GUN_DRAW`] at a logic time while fetching, then (as the client remakes
        /// a waiting drawable) another of the same shared module data at a logic time after the files arrived.
        ///
        /// *Predictions, written before the first run:*
        /// - while fetching (everything, or only the skeleton): the default state's pristine bones, turrets and barrels
        ///   are not valid, its bone cache is empty with turret bones `[(0, 0), (0, 0)]`, and the module data is not
        ///   validated for the plain mode; with everything fetched there is no render object;
        /// - after arrival: all three valid, the mode validated, and the bone cache exactly that of a drawable made with
        ///   the files at hand from the start (the 4 pristine bones, turret bones `[(1, 2), (0, 0)]`, one primary
        ///   barrel 3/4/4), which is itself what [`the_pristine_bones_turrets_and_barrels_are_cached_at_a_logic_time`]
        ///   checks against upstream;
        /// - a draw outside a logic time after arrival validates nothing (upstream's rule, unchanged): a drawable made
        ///   with `logic_time` false after a fetched one still finds the bones not valid.
        ///
        /// *Results (2026-11-06), first run:* all held, in both cases.
        #[test]
        fn bones_validated_while_fetching_are_found_after_the_files_arrive() {
            let mut rig = Rig::new();
            let at_hand = rig.draw(true).get_cur_state().unwrap().bone_cache();
            for skeleton_only in [false, true] {
                let data = Arc::new(parse(GUN_DRAW).unwrap());
                let mut src = Fetching { files: W3dFiles { assets: &mut rig.am, fs: &rig.fs }, arrived: false, skeleton_only, deferred: 0 };
                let m = {
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true };
                    W3DModelDraw::new(data.clone(), 0, false, false, &mut env).unwrap()
                };
                let s = m.get_cur_state().unwrap();
                assert!(src.deferred > 0);
                assert!(!s.pristine_bones_valid() && !s.turrets_valid() && !s.barrels_valid(), "skeleton only {skeleton_only}");
                let c = s.bone_cache();
                assert!(c.pristine_bones.is_empty());
                assert_eq!(c.turret_bones, [(0, 0), (0, 0)]);
                assert!(c.weapon_barrel_info_vec[0].is_empty());
                assert!(!data.validated_for(false, false));
                assert_eq!(m.render_object.is_none(), !skeleton_only);

                // made outside a logic time once the files are there: upstream's rule, nothing validated
                src.arrived = true;
                let m = {
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: false };
                    W3DModelDraw::new(data.clone(), 0, false, false, &mut env).unwrap()
                };
                assert!(!m.get_cur_state().unwrap().pristine_bones_valid());
                assert!(!data.validated_for(false, false));

                // made again at a logic time (the client's remake): found
                let before = src.deferred;
                let m = {
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true };
                    W3DModelDraw::new(data.clone(), 0, false, false, &mut env).unwrap()
                };
                assert_eq!(src.deferred, before);
                let s = m.get_cur_state().unwrap();
                assert!(s.pristine_bones_valid() && s.turrets_valid() && s.barrels_valid());
                assert!(data.validated_for(false, false));
                assert_eq!(s.bone_cache(), at_hand);
                assert_eq!(s.bone_cache().turret_bones, [(1, 2), (0, 0)]);
                assert!(m.render_object.is_some());
            }
        }

        /// With the files at hand nothing is answered "for now": a source that never defers gives the module data the
        /// same validity bits and caches as before GitHub #42 (the at-hand drawable above), and is validated for its mode.
        #[test]
        fn with_the_files_at_hand_the_validation_is_upstream_s() {
            let mut rig = Rig::new();
            let m = rig.draw(true);
            let s = m.get_cur_state().unwrap();
            assert!(s.pristine_bones_valid() && s.turrets_valid() && s.barrels_valid());
            assert!(m.data.validated_for(false, false));
            let mut src = Fetching { files: W3dFiles { assets: &mut rig.am, fs: &rig.fs }, arrived: true, skeleton_only: false, deferred: 0 };
            let data = Arc::new(parse(GUN_DRAW).unwrap());
            let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true };
            let m2 = W3DModelDraw::new(data, 0, false, false, &mut env).unwrap();
            assert_eq!(src.deferred, 0);
            assert_eq!(m2.get_cur_state().unwrap().bone_cache(), s.bone_cache());
        }

        /// Predictions, written before the first run:
        /// - `DAMAGED` (same model, same turret names: no new render object) hides `TURRET` and every sub-object whose
        ///   bone descends from the turret's bone (BARREL, GUN, MUZZLEFX01), but not ANTENNA, which is on the turret's
        ///   own bone, nor CHASSIS;
        /// - back to the default state nothing is shown again (it names no sub-objects, and the render object is
        ///   kept): TURRET stays hidden;
        /// - `showSubObject("Turret", true)` then `updateSubObjects` shows it and its descendants again;
        /// - `NIGHT` (its own state) shows the headlight (`hideAllHeadlights(false)`);
        /// - posed by the client, the hidden flags are the drawn model's.
        #[test]
        fn sub_objects_are_hidden_and_shown_with_their_bones_children() {
            let mut rig = Rig::new();
            let mut m = rig.draw(true);
            rig.frame(&mut m, [None, None]);
            rig.set_flags(&mut m, &[MODELCONDITION_DAMAGED]);
            let h = hidden(&m);
            assert!(h[TURRET] && h[BARREL] && h[GUN] && h[MUZZLE] && !h[ANTENNA] && !h[CHASSIS], "{h:?}");
            rig.set_flags(&mut m, &[]);
            assert!(hidden(&m)[TURRET]);
            m.show_sub_object(b"Turret", true);
            m.update_sub_objects();
            let h = hidden(&m);
            assert!(!h[TURRET] && !h[BARREL] && !h[GUN] && !h[MUZZLE], "{h:?}");
            assert!(h[HEADLIGHT]);
            rig.set_flags(&mut m, &[MODELCONDITION_NIGHT]);
            assert!(!hidden(&m)[HEADLIGHT]);
            let model = rig.posed(&m);
            assert_eq!(model.sub_objects.iter().map(|s| s.hidden).collect::<Vec<_>>(), hidden(&m));
        }
    }

    // ---- GitHub #41: W3DTankDraw's treads

    mod treads {
        use super::*;
        use crate::common::file_system::{FileSystem, MemoryDevice};
        use crate::formats::w3d::assetmgr::W3dFileFactory;
        use crate::formats::w3d::synth::*;

        /// A tank `TANKT` (ROOTTRANSFORM and BODY pivots) with five cubes on the body: CHASSIS (plain), TREADSL, TREADSR
        /// and TREADSX (each a linear-offset mapper on stage 0), and TREADSNOMAP (named as a tread, no mapper).
        fn tankt(name: &[u8]) -> Vec<u8> {
            let ident = [0.0, 0.0, 0.0, 1.0];
            let mut f = htree_w3d(name, &[
                PivotSpec { name: b"ROOTTRANSFORM".to_vec(), parent: -1, translation: [0.0; 3], rotation: ident },
                PivotSpec { name: b"BODY".to_vec(), parent: 0, translation: [0.0, 0.0, 1.0], rotation: ident },
            ]);
            let subs: [(&[u8], u32); 5] = [(b"CHASSIS", 0), (b"TREADSL", 4), (b"TREADSR", 4), (b"TREADSX", 4), (b"TREADSNOMAP", 0)];
            let mut names = Vec::new();
            for (n, mapping) in subs {
                let mut m = MeshSpec::cuboid(name, n, [-0.5; 3], [0.5; 3]);
                m.stage0.texture = None;
                m.stage0.mapping = mapping;
                f.extend(m.to_w3d());
                let mut full = name.to_vec();
                full.push(b'.');
                full.extend_from_slice(n);
                names.push(full);
            }
            let hl: Vec<(&[u8], u32)> = names.iter().map(|n| (n.as_slice(), 1)).collect();
            f.extend(hlod_w3d(name, name, &hl));
            f
        }

        const CHASSIS: usize = 0;
        const LEFT: usize = 1;
        const RIGHT: usize = 2;
        const MIDDLE: usize = 3;
        const NOMAP: usize = 4;

        struct Rig {
            fs: FileSystem<MemoryDevice>,
            am: AssetManager,
            random: RandomValues,
        }

        impl Rig {
            fn new() -> Self {
                let mut d = MemoryDevice::new();
                d.insert(b"Art/W3D/tankt.w3d", tankt(b"TANKT"));
                d.insert(b"Art/W3D/tanku.w3d", tankt(b"TANKU"));
                Rig { fs: FileSystem::new(d), am: AssetManager::new_game(W3dFileFactory::default()), random: RandomValues::new(0) }
            }
            /// A `W3DTankDraw` of `TANKT` (`TANKU` when `DAMAGED`) with `TreadAnimationRate = rate` (per second).
            fn draw(&mut self, rate: &str) -> W3DModelDraw {
                let l = [
                    (0, "DefaultConditionState", ""),
                    (1, "Model", "TANKT"),
                    (0, "ConditionState", "DAMAGED"),
                    (1, "Model", "TANKU"),
                    (0, "TreadAnimationRate", rate),
                ];
                let d = Arc::new(W3DModelDrawModuleData::parse(b"W3DTankDraw", &block(&l)).unwrap().unwrap());
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: true };
                W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
            }
            fn frame(&mut self, m: &mut W3DModelDraw, facts: &DrawFacts) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false };
                m.do_draw_module(facts, &mut env);
            }
            fn set_flags(&mut self, m: &mut W3DModelDraw, bits: &[i32]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false };
                m.replace_model_condition_state(&flags(bits), &mut env);
            }
        }

        fn moving(v: f32, turning: i32, motive: bool, dir_angle: f32) -> DrawFacts {
            let motion = MotionFacts {
                velocity_magnitude: v,
                turning,
                is_motive: motive,
                cur_locomotor_speed: Some(4.0),
                unit_direction_2d: (crate::ww::wwmath::cos(dir_angle), crate::ww::wwmath::sin(dir_angle)),
                velocity: Default::default(),
                moving_backwards: false,
                angle_to_path_point: None,
            };
            DrawFacts { motion: Some(motion), ..FACTS }
        }

        fn offsets(m: &W3DModelDraw) -> Vec<(usize, TreadType, f32)> {
            m.tank.as_ref().unwrap().treads.iter().map(|t| (t.sub_object, t.kind, t.custom_uv_offset[0])).collect()
        }

        /// `offset - floor(offset)` after `n` steps of `-d` from 0, as `doDrawModule` computes it.
        fn scrolled(n: usize, d: f32) -> f32 {
            let mut o = 0.0f32;
            for _ in 0..n {
                o -= d;
                o -= crate::ww::wwmath::floor(o);
            }
            o
        }

        /// *Predictions, written before the first run* (rate 3 a second is 0.1 a frame; the pivot and drive fractions
        /// are the defaults, 0.6 and 0.3; the locomotor's speed is 4):
        /// - the render object made in the constructor gives three treads: TREADSL left (sub-object 1), TREADSR right
        ///   (2), TREADSX middle (3), offsets 0; not CHASSIS, and not TREADSNOMAP (no linear-offset mapper);
        /// - driving straight at speed v = 2 (half the locomotor's speed, above 0.3) under a motive force, every tread's
        ///   U goes down by 0.1 a draw, wrapped: 0.9, 0.8, 0.7 after three draws (to within one ulp of the same sum); the
        ///   client's render model gets those offsets on sub-objects 1 to 3 and none on 0 and 4;
        /// - at v = 1 (0.25 of the locomotor's speed, under 0.3), or at v = 2 without a motive force, nothing moves;
        /// - pivoting right (`TURN_NEGATIVE`, v = 0) with the direction turned 0.1 rad since the last draw: the left
        ///   tread goes down by 0.1, the right one up by 0.1, the middle one not at all; a second draw facing the same
        ///   way moves nothing; pivoting left moves them the other way;
        /// - without physics (no motion facts) `doDrawModule` returns before the base class's: an object that should not
        ///   animate is not paused;
        /// - a new render object (the `DAMAGED` state's model) finds its treads again, at offset 0;
        /// - with `TreadAnimationRate` 0 there are no treads.
        ///
        /// *Results (2026-11-06):* all held but one. **Miss:** the step is not 0.1 but 0.10000001: `parseVelocityReal`
        /// multiplies 3 by `SECONDS_PER_LOGICFRAME_REAL` (1/30 in f32). The test now takes the parsed rate and checks
        /// it is 0.10000001; with it every offset held exactly (three drive steps within 1e-6 of 0.7).
        #[test]
        fn treads_scroll_with_the_tank_s_motion() {
            let mut rig = Rig::new();
            let mut m = rig.draw("3");
            assert_eq!(offsets(&m), vec![(LEFT, TreadType::Left, 0.0), (RIGHT, TreadType::Right, 0.0), (MIDDLE, TreadType::Middle, 0.0)]);
            // (the first run's miss: `parseVelocityReal` gives 3 * (1/30) = 0.10000001 in f32, not 0.1)
            let DrawModuleExtra::Tank(t) = &m.data.extra else { panic!("a tank") };
            let rate = t.tread_animation_rate;
            assert_eq!(rate, 0.10000001);

            let drive = moving(2.0, TURN_NONE, true, 0.0);
            for _ in 0..3 {
                rig.frame(&mut m, &drive);
            }
            let want = scrolled(3, rate);
            assert!((want - 0.7).abs() < 1e-6, "{want}");
            assert_eq!(offsets(&m), vec![(LEFT, TreadType::Left, want), (RIGHT, TreadType::Right, want), (MIDDLE, TreadType::Middle, want)]);
            let mut model = RenderModel::create(&mut rig.am, &rig.fs, b"tankt").unwrap().unwrap();
            m.render_object.as_ref().unwrap().pose_render_model(&mut model);
            let uv: Vec<Option<[f32; 2]>> = model.sub_objects.iter().map(|s| s.uv_offset).collect();
            assert_eq!(uv, vec![None, Some([want, 0.0]), Some([want, 0.0]), Some([want, 0.0]), None]);
            let _ = (CHASSIS, NOMAP);

            rig.frame(&mut m, &moving(1.0, TURN_NONE, true, 0.0));
            rig.frame(&mut m, &moving(2.0, TURN_NONE, false, 0.0));
            assert_eq!(offsets(&m).iter().map(|o| o.2).collect::<Vec<_>>(), vec![want; 3]);

            let mut m = rig.draw("3");
            rig.frame(&mut m, &moving(0.0, TURN_NEGATIVE, false, 0.1));
            let down = -rate - crate::ww::wwmath::floor(-rate);
            assert_eq!(offsets(&m).iter().map(|o| o.2).collect::<Vec<_>>(), vec![down, rate, 0.0]);
            rig.frame(&mut m, &moving(0.0, TURN_NEGATIVE, false, 0.1));
            assert_eq!(offsets(&m).iter().map(|o| o.2).collect::<Vec<_>>(), vec![down, rate, 0.0]);
            rig.frame(&mut m, &moving(0.0, TURN_POSITIVE, false, 0.0));
            let l = down + rate - crate::ww::wwmath::floor(down + rate);
            let r = rate - rate - crate::ww::wwmath::floor(rate - rate);
            assert_eq!(offsets(&m).iter().map(|o| o.2).collect::<Vec<_>>(), vec![l, r, 0.0]);

            rig.frame(&mut m, &DrawFacts { should_animate_with_power: false, should_animate_without_power: false, ..FACTS });
            assert!(!m.pause_animation, "the base class's doDrawModule did not run");

            rig.set_flags(&mut m, &[MODELCONDITION_DAMAGED]);
            assert_eq!(m.render_object.as_ref().unwrap().model_name, b"tanku");
            assert_eq!(offsets(&m), vec![(LEFT, TreadType::Left, 0.0), (RIGHT, TreadType::Right, 0.0), (MIDDLE, TreadType::Middle, 0.0)]);

            let m = rig.draw("0");
            assert!(offsets(&m).is_empty());
        }
    }
}
