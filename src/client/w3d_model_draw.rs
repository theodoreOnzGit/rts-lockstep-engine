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
//   (GitHub #171) W3DModelDraw.cpp: doStartOrStopParticleSys, setHidden's call of it,
//   setFullyObscuredByShroud, stopClientParticleSystems, recalcBonesForClientParticleSystems,
//   updateBonesForClientParticleSystems, and the constructor's, setModelState's and doDrawModule's
//   parts for them
//   (GitHub #172) GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DAssetManager.cpp
//   (Create_Render_Obj's scaling: ident_scale, scale_epsilon, reallyscale, rendobj->Scale), hlod.cpp
//   (HLodClass::Scale's hierarchy part), animobj.cpp (Get_Bone_Transform, Update_Sub_Object_Transforms'
//   BASE_POSE), htree.cpp (Base_Update, through formats::w3d::htree)
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
//!   `crate::client::particle_sys`, ~~but this module does not make its bone systems yet~~ **Corrected 2026-11-12
//!   (#171):** and this module makes, moves, stops and destroys its bone systems, see below); `RequiredScience` keeps
//!   the science's name.
//! - (#171) **The bone particle systems** ([`W3DModelDraw::recalc_bones_for_client_particle_systems`],
//!   [`W3DModelDraw::update_bones_for_client_particle_systems`], [`W3DModelDraw::stop_client_particle_systems`]) reach
//!   `TheParticleSystemManager` as an argument ([`DrawEnv::particles`], or `Option<&mut ParticleSystemManager>`);
//!   without one (`None`: tests, a save's load) no system is found or made, as a manager with none would answer. A
//!   bone's transform is the render object's [`Skeleton`] as made (the base pose under the drawable's scale), not the
//!   animated pose upstream's `Get_Bone_Transform` evaluates, and without the turret and recoil bone captures; the
//!   live transform `updateBonesForClientParticleSystems` reads is [`DrawFacts::transform`] (the drawable's, without
//!   the scale, which the skeleton carries). ~~For a drawable scale that is not a power of two the port's
//!   `world * (Scale * bone)` rounds differently from upstream's `(world * Scale) * bone` (measured,
//!   `tests/differential_model_draw_particles.rs`).~~ **Corrected 2026-11-12 (#172):** that was not upstream's
//!   convention, nor only a rounding: see the next item. `attachToDrawable` takes [`W3DModelDraw::drawable_id`] (the
//!   port's drawable id for particles is its object's).
//! - (#172) **The drawable's scale is in the render object's hierarchy, not in a matrix.** Upstream makes the render
//!   object with `Create_Render_Obj(name, getScale(), color)` (W3DModelDraw.cpp:623, 3034), which when
//!   `WWMath::Fabs(scale - 1) > 0.01` (W3DAssetManager.cpp:84-85, 738) calls `rendobj->Scale(scale)` (858-859):
//!   `HLodClass::Scale` -> `HTreeClass::Scale`, which multiplies every pivot's base *translation* by the scale
//!   (htree.cpp:982-996; [`crate::formats::w3d::htree::HTree::scale`], [`scale_render_obj_hierarchy`]). The live
//!   render object's transform is the drawable's ~~(2094-2100: no template scale)~~ **Corrected 2026-11-13 (#174):**
//!   scaled by `getInstanceScale()` when that is not 1 (2045-2054), and `getInstanceScale()` is `m_instanceScale`, which
//!   `getScale()` answers too (Drawable.h:404, Drawable.cpp:1093-1097): the template's scale *is* in the draw's root,
//!   on top of the hierarchy's (then adjusted by `adjustTransformMtx`, 2094-2100: [`RenderObj::transform`],
//!   [`W3DModelDraw::adjust_transform_mtx`]). Outside a draw it is `reactToTransformChange`'s, unscaled (3647-3659). A
//!   bone is `Base_Update`'s chain
//!   from that root, `((root * B'1) * B'2) ...` (htree.cpp:531-548, animobj.cpp:658-675, 780-802). So there are two
//!   roots: **(A)** `Set_Transform(Matrix3D(true).Scale(getScale()))` then `Get_Bone_Transform` (the pristine bones,
//!   669-672, and the bone systems' recalc, 2611-2617): the chain from `S` over the scaled pivots, translations scaled
//!   twice, rotations once: [`SkeletonBone::transform`], [`RenderObj::get_bone_transform_scaled`]; **(B)** the live
//!   transform (the bone systems' update 2698, `getCurrentWorldspaceClientBonePositions` 3564, `handleWeaponFireFX`
//!   3727): the chain from the ~~drawable's transform, rotations not scaled~~ **Corrected 2026-11-13 (#174):** render
//!   object's transform ([`RenderObj::transform`]: in a draw the drawable's scaled by the instance scale, so at a
//!   template scale the live bones are the pristine bones in the world): [`Skeleton::bone_transform`],
//!   [`RenderObj::get_bone_transform`]. Both are computed as upstream computes them, in its order, so they agree
//!   with it bit for bit at any scale (`tests/differential_model_draw_particles.rs`, gated). (#174) Both apply the
//!   render object's captured bones (`Capture_Bone`, `Control_Bone`: the turrets and the recoil,
//!   [`Skeleton::bone_transform_controlled`]); ~~neither applies the animation's current pose (#174's gaps)~~
//!   **Corrected 2026-11-13 (#179):** root (B) is the animation's current pose ([`RenderObj::get_bone_transform`]); root
//!   (A), the pristine bones, is the asset source's pose (a fresh render object's, at the state's first animation's frame).
//! - (#174) ~~**Not translated:** the animation's current pose in the bones (`Get_Bone_Transform` ->
//!   `Update_Sub_Object_Transforms` -> `Anim_Update`/`Blend_Update`, animobj.cpp:780-860): the render object keeps the
//!   animation's name, frame count, rate and frame ([`ModeAnim`]) but not its channels nor a pose source, and upstream's
//!   evaluation also advances the frame (`Single_Anim_Progress`) when the hierarchy is not valid.~~ **Corrected
//!   2026-11-13 (#179):** translated. The render object keeps the motion's data ([`AnimHandle::data`], [`AnimData`]: the
//!   asset manager's shared animation, a morph's poses), its own live hierarchy and `IsTreeValid`; `Get_Bone_Transform`
//!   (animobj.cpp:658-675) evaluates the pose when the hierarchy is not valid (`Update_Sub_Object_Transforms`, 780-860:
//!   `Single_Anim_Progress` unless `MANUAL`, then the inline `Anim_Update` with the raw fast path, or `Blend_Update`,
//!   `Combo_Update`, `Base_Update`), and answers the last pose while it is valid. So **a bone query moves the animation
//!   on** (frame, `PrevFrame`, `LastSyncTime`, a ping-pong's direction) as upstream's does; every bone query of the port
//!   takes the sync time (`WW3D::Get_Sync_Time()`) for it. `Set_Transform`, `Set_Animation` and `Control_Bone`
//!   invalidate the hierarchy, `Capture_Bone` and `Release_Bone` do not ([`RenderObj::set_transform`] and the rest);
//!   `Render` progresses and then evaluates ([`RenderObj::render`]). Where the port may only read a render object (another
//!   drawable's bone for a laser, the FX lists' bones), it answers without writing the state upstream's query writes
//!   ([`RenderObj::peek_bone_transform`]: #179's gaps). Checked against animobj.cpp compiled verbatim
//!   (`tests/differential_w3d.rs`, `animatable_bones_match_upstream`). `applyPhysicsXform`'s
//!   pitch, roll and bounce and the drawable's instance matrix (Drawable.cpp:2663-2674) are not in the draw's root
//!   either ([`crate::client::drawable::Drawable::physics_xform`] is kept apart for the renderer).
//! - **Not translated yet** (GitHub #25's remaining work): ~~the bone caches
//!   (`validateCachedBones`, pristine bones, public bones beyond the parse,
//!   weapon barrels), turret and recoil posing, sub-object hide and show,
//!   muzzle flashes, headlights, particle systems,~~ shadows, terrain decals and
//!   tracks, ~~the attach-to-bone offset, `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`,~~
//!   (**Corrected 2026-11-13 (#174):** both translated, [`W3DModelDraw::adjust_transform_mtx`])
//!   the weapon fire FX lists themselves, garrison flags, and the subclasses'
//!   runtime (~~treads,~~ tires, supplies, dependencies). ~~`validateStuff` only
//!   marks the turret info valid.~~ **Corrected 2026-11-06 (#27):** the bone
//!   caches, turret and recoil posing, muzzle flashes, sub-object hide and
//!   show and headlights are translated (below). **Corrected 2026-11-12 (#171):** the bone particle systems are
//!   translated (above). **Corrected 2026-11-06
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
use crate::formats::w3d::assetmgr::{AssetManager, HAnim, Prototype};
use crate::formats::w3d::hanim_combo::HAnimCombo;
use crate::formats::w3d::hmorphanim::PoseLookup;
use crate::formats::w3d::htree::{HTree, Pivot};
use crate::formats::w3d::W3dResult;
use crate::formats::w3d::w3d_file::W3D_NAME_LEN;
use crate::formats::w3d::render::{compute_current_frame, get_frame_rate, AnimMode, RenderModel};
use crate::logic::weapon_set::{SLOTS, WEAPON_SLOT_TYPE_NAMES};
use crate::formats::w3d::vertmaterial::MapperKind;
use crate::ww::matrix3d::Matrix3D;
use crate::client::w3d_truck_draw::TruckKind;
use crate::client::particle_sys::{ParticleSystemId, ParticleSystemManager};
use crate::common::base_type::Coord3D;
use crate::common::game_type::ObjectId;
use crate::logic::modules::plant;

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
        let which_frame = anim_to_use.as_ref().map(|h| if test_flag_bit(self.flags, PRISTINE_BONE_POS_IN_FINAL_FRAME) && !plant(9739) { h.num_frames - 1 } else { 0 });
        // Set_Animation does nothing on a render object that is not an HLOD
        let pose = match (&anim_to_use, which_frame) {
            (Some(h), Some(f)) if is_hlod => Some((h.name.as_slice(), f as f32)),
            _ => None,
        };
        let skeleton = env.assets.skeleton(&model_name, pose, scale);

        let mut map = BTreeMap::new();
        for b in standard_public_bones.iter().filter(|_| !plant(9749)) {
            // don't crash here, since these are catch-all global bones and won't be present in most models.
            do_single_bone_name(skeleton.as_ref(), b, &mut map);
        }
        let mut public = self.public_bones.clone();
        for b in extra_public_bones.iter().filter(|_| !plant(9748)) {
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
            // (#179) upstream poses the live render object itself: `Set_Transform(tmp)` (W3DModelDraw.cpp:672), its bones
            // read, `Set_Transform(originalTransform)` (702). The bones are the fresh skeleton's here (above); what remains
            // on the live one is the hierarchy left not valid by the two `Set_Transform`s.
            let original_transform = r.transform;
            let mut tmp = Matrix3D::IDENTITY;
            tmp.scale(scale);
            r.set_transform(&tmp);
            r.set_transform(&original_transform);
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
            if vecs[wslot].is_empty() && !plant(9742) {
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
    if bone_index != 0 || (plant(9744) && sk.has_htree()) {
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
    if !found_as_bone && !plant(9743) {
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
    /// (#174) `m_attachToDrawableBoneOffset` and `m_attachToDrawableBoneOffsetValid` (`CACHE_ATTACH_BONE`, which
    /// W3DModelDraw.h:278 defines; mutable upstream): `None` until [`W3DModelDrawModuleData::get_attach_to_drawable_bone_offset`]
    /// first fills it, then the offset, kept for every drawable of the template.
    attach_to_drawable_bone_offset: Mutex<Option<crate::ww::vector3::Vector3>>,
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
            attach_to_drawable_bone_offset: Mutex::new(None),
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

    /// (#174) `W3DModelDrawModuleData::getAttachToDrawableBoneOffset(draw)` (W3DModelDraw.cpp:1148-1174,
    /// `CACHE_ATTACH_BONE`): `None` (`NULL`) without an `AttachToBoneInAnotherModule` bone; else the first call fills the
    /// cache from `pristine` (the drawable's `getPristineBonePositions(m_attachToDrawableBone, 0, NULL, &boneMtx, 1)`:
    /// the bone's transform when exactly one was found, its translation kept; else zero) and every call answers the
    /// cached offset. Upstream's comment: "must use pristine bone here since the result is used by logic". `pristine`
    /// is asked only to fill the cache.
    pub fn get_attach_to_drawable_bone_offset(&self, pristine: impl FnOnce() -> Option<Matrix3D>) -> Option<crate::ww::vector3::Vector3> {
        if self.attach_to_drawable_bone.is_empty() {
            return None;
        }
        let mut cache = self.attach_to_drawable_bone_offset.lock().unwrap_or_else(|e| e.into_inner());
        if cache.is_none() {
            *cache = Some(match pristine() {
                Some(bone_mtx) if !plant(9753) => bone_mtx.get_translation(),
                _ => crate::ww::vector3::Vector3::new(0.0, 0.0, 0.0),
            });
        }
        *cache
    }

    /// (#174) The cached `m_attachToDrawableBoneOffset` without filling it ([`W3DModelDraw::adjust_transform_mtx`]'s
    /// read): `None` without an `AttachToBoneInAnotherModule` bone; the zero offset while not filled yet (the port's
    /// drawable fills it before its modules draw, [`crate::client::drawable::Drawable::validate_attach_to_drawable_bone_offsets`];
    /// a module drawn without its drawable has no pristine bones to fill it from).
    pub fn attach_to_drawable_bone_offset_cached(&self) -> Option<crate::ww::vector3::Vector3> {
        if self.attach_to_drawable_bone.is_empty() {
            return None;
        }
        let cache = self.attach_to_drawable_bone_offset.lock().unwrap_or_else(|e| e.into_inner());
        Some(cache.unwrap_or(crate::ww::vector3::Vector3::new(0.0, 0.0, 0.0)))
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

    /// (#175) `W3DModelDraw::getPristineBonePositionsForConditionState(condition, boneNamePrefix, startIndex, positions,
    /// transforms, maxBones)` (W3DModelDraw.cpp:3403-3506) as the logic asks it: "this recalcs the state every time,
    /// rather than using m_curState, because m_curState could be a transition state, and we really need to get the
    /// pristine bone(s) for the state that logic believes to be current" (`findBestInfo(condition)`), then
    /// `validateStuff(stateToUse == m_curState ? m_renderObject : NULL, getDrawable()->getScale(), m_extraPublicBones)`
    /// with no current render object (the logic's module has none: the `NULL` branch), and the pristine bones `prefix`
    /// (for `startIndex` 0) or `prefix01`, `prefix02`, ... from `startIndex` to 99, lower-cased, at most `maxBones`
    /// (clamped to `MAX_BONE_GET`, 64), written into `transforms` from its start. A bone not found ends the list, and
    /// writes the object's transform (`getDrawable()->getObject()->getTransformMatrix()`, `object_transform`; the
    /// identity without an object) at the place the next bone would have gone, as upstream writes it into the caller's
    /// array (a slot beyond `transforms` is upstream's write past the caller's array: not made). Returns the count; the
    /// positions are the transforms' translations (`positions[i]`, the caller's). No state for the flags: 0, nothing
    /// written.
    #[allow(clippy::too_many_arguments)]
    pub fn get_pristine_bone_positions_for_condition_state<A: W3dAssetSource>(
        &self,
        condition: &ModelConditionFlags,
        bone_name_prefix: &[u8],
        start_index: i32,
        transforms: &mut [Matrix3D],
        max_bones: i32,
        scale: f32,
        standard_public_bones: &[Vec<u8>],
        object_transform: Option<&Matrix3D>,
        env: &mut DrawEnv<'_, A>,
    ) -> i32 {
        let flags = if plant(9721) { ModelConditionFlags::new() } else { *condition };
        let Some(i) = self.find_best_info(&flags) else { return 0 };
        let state_to_use = &self.condition_states[i];
        state_to_use.validate_stuff(None, scale, &self.extra_public_bones, standard_public_bones, env);

        const MAX_BONE_GET: i32 = 64;
        let max_bones = if max_bones > MAX_BONE_GET { MAX_BONE_GET } else { max_bones };

        let mut pos_count: i32 = 0;
        let end_index = if start_index == 0 && !plant(9724) { 0 } else { 99 };
        let mut i = start_index;
        while i <= end_index {
            let mut buffer = bone_name_prefix.to_vec();
            if i != 0 {
                buffer.extend_from_slice(format!("{i:02}").as_bytes());
            }
            // convert to all-lowercase since that's how we filled in the map
            if !plant(9723) {
                buffer.make_ascii_lowercase();
            }
            let slot = usize::try_from(pos_count).ok().and_then(|p| transforms.get_mut(p));
            match state_to_use.find_pristine_bone(&buffer) {
                Some(b) => {
                    if let Some(t) = slot {
                        *t = b.mtx;
                    }
                }
                None => {
                    // (`*obj->getTransformMatrix()`, else `Make_Identity()`)
                    if let Some(t) = slot {
                        *t = match object_transform {
                            Some(m) if !plant(9722) => *m,
                            _ => Matrix3D::IDENTITY,
                        };
                    }
                    break;
                }
            }
            pos_count += 1;
            if pos_count >= max_bones + i32::from(plant(9725)) {
                break;
            }
            i += 1;
        }
        pos_count
    }

    /// (#175) `W3DModelDraw::getProjectileLaunchOffset(condition, wslot, specificBarrelToUse, launchPos, tur,
    /// turretRotPos, turretPitchPos)` (W3DModelDraw.cpp:3253-3398), ~~`CACHE_ATTACH_BONE` undefined as upstream builds
    /// it~~ **Corrected 2026-11-13 (#178):** `CACHE_ATTACH_BONE` defined, as W3DModelDraw.h:278 defines it (#175
    /// translated the undefined branch, 3306-3319 and 3346-3349: a tech offset, the attach bone's pristine transform
    /// turned by the drawable's orientation, added to the launch position; upstream does not build it). The state the
    /// logic believes current (`findBestInfo(condition)`; none: false, nothing written), validated with no render
    /// object at the drawable's scale; the barrel's `m_projectileOffsetMtx` (barrel 0 when out of range), turned by the
    /// turret's art angle and pitch, into `launch_pos` (no barrels: `launchPos = NULL`, false), with no offset added.
    /// The turret's angle and pitch bones' pristine positions (`findPristineBonePos`, zero when not found) into
    /// `turret_rot_pos` and `turret_pitch_pos`, which are zeroed first. With a turret, the module data's cached
    /// attach-bone offset (`d->getAttachToDrawableBoneOffset(getDrawable())`, 3362-3364:
    /// [`W3DModelDrawModuleData::get_attach_to_drawable_bone_offset`], `None` without an `AttachToBoneInAnotherModule`
    /// bone) is added, as it is (model space, not turned), to each position asked for, whether or not its bone was
    /// found (3373-3379, 3389-3395). `attach_pivot` is the drawable's `getPristineBonePositions(bone, 0, NULL, &mtx, 1)
    /// == 1`, asked only when the cache is empty (the logic fills it at load,
    /// [`crate::logic::world::drawable_bones::fill_attach_to_drawable_bone_offsets`]). Returns whether `launch_pos` was
    /// given and written (`launchPos != NULL`).
    #[allow(clippy::too_many_arguments)]
    pub fn get_projectile_launch_offset<A: W3dAssetSource>(
        &self,
        condition: &ModelConditionFlags,
        wslot: usize,
        specific_barrel_to_use: i32,
        launch_pos: Option<&mut Matrix3D>,
        tur: i32,
        turret_rot_pos: Option<&mut Coord3D>,
        turret_pitch_pos: Option<&mut Coord3D>,
        scale: f32,
        standard_public_bones: &[Vec<u8>],
        attach_pivot: &mut impl FnMut(&mut DrawEnv<'_, A>) -> Option<Matrix3D>,
        env: &mut DrawEnv<'_, A>,
    ) -> bool {
        let Some(si) = self.find_best_info(condition) else { return false };
        let state_to_use = &self.condition_states[si];
        state_to_use.validate_stuff(None, scale, &self.extra_public_bones, standard_public_bones, env);

        // (`#ifdef CACHE_ATTACH_BONE #else` ... `#endif`: no tech offset on the defined branch)

        let turret = usize::try_from(tur).ok().filter(|&t| t < MAX_TURRETS);
        let wbvec = state_to_use.weapon_barrels(wslot);
        let mut launch_pos = launch_pos;
        if wbvec.is_empty() {
            // Can't find the launch pos, but they might still want the other info they asked for
            launch_pos = None;
        } else {
            let mut barrel = specific_barrel_to_use;
            if barrel < 0 || barrel as usize >= wbvec.len() {
                barrel = if plant(9731) { wbvec.len() as i32 - 1 } else { 0 };
            }
            if let Some(lp) = launch_pos.as_deref_mut() {
                *lp = wbvec[barrel as usize].projectile_offset_mtx;
                if let Some(t) = turret {
                    if !plant(9728) {
                        lp.pre_rotate_z(state_to_use.turrets[t].turret_art_angle);
                    }
                    let pitch = state_to_use.turrets[t].turret_art_pitch;
                    lp.pre_rotate_y(if plant(9729) { pitch } else { -pitch });
                }
                // (#178 plant: the undefined branch's adjust kept, the cached offset added to the launch position too)
                if plant(9784) {
                    if let Some(o) = self.get_attach_to_drawable_bone_offset(|| attach_pivot(env)) {
                        lp.adjust_translation(&o);
                    }
                }
            }
        }

        let mut turret_rot_pos = turret_rot_pos;
        let mut turret_pitch_pos = turret_pitch_pos;
        if let Some(p) = turret_rot_pos.as_deref_mut() {
            *p = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        }
        if let Some(p) = turret_pitch_pos.as_deref_mut() {
            *p = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        }

        if let Some(t) = turret {
            // `const Vector3* offset = d->getAttachToDrawableBoneOffset(getDrawable());`
            let offset = if plant(9783) {
                // (#178 plant: no cache, the asking drawable's bone read at every call)
                (!self.attach_to_drawable_bone.is_empty()).then(|| attach_pivot(env).map_or(crate::ww::vector3::Vector3::new(0.0, 0.0, 0.0), |m| m.get_translation()))
            } else {
                self.get_attach_to_drawable_bone_offset(|| attach_pivot(env))
            };
            // `if (offset) { pos->x += offset->X; ... }`
            let add = |p: &mut Coord3D, o: Option<crate::ww::vector3::Vector3>| {
                if let Some(o) = o {
                    p.x += o.x;
                    p.y += o.y;
                    p.z += o.z;
                }
            };
            let tur_info = &state_to_use.turrets[t];
            // (`findPristineBonePos(key, pos)`: the bone's translation, else zero; "*** ASSET ERROR: TurretBone %s not found!")
            let bone_pos = |key: &[u8]| -> Coord3D {
                match state_to_use.find_pristine_bone(key) {
                    Some(b) => {
                        let v = b.mtx.get_translation();
                        Coord3D { x: v.x, y: v.y, z: v.z }
                    }
                    None => Coord3D { x: 0.0, y: 0.0, z: 0.0 },
                }
            };
            if let Some(p) = turret_rot_pos {
                if !tur_info.turret_angle_name_key.is_empty() {
                    *p = bone_pos(&tur_info.turret_angle_name_key);
                    if plant(9785) {
                        add(p, offset);
                    }
                }
                if !plant(9781) && !plant(9785) {
                    add(p, offset);
                }
            }
            if let Some(p) = turret_pitch_pos {
                if !tur_info.turret_pitch_name_key.is_empty() {
                    *p = bone_pos(&tur_info.turret_pitch_name_key);
                }
                if !plant(9782) {
                    add(p, offset);
                }
            }
        }
        // return if LaunchPos is valid or not
        launch_pos.is_some()
    }

    /// (#175) `W3DModelDraw::getBarrelCount(wslot)` (W3DModelDraw.cpp:3689-3693) as the logic asks it: upstream reads
    /// `m_curState`, the client's state, which is a transition state while one plays (its timing is the client's); the
    /// logic reads the state it believes current, `findBestInfo(condition)` (what `m_curState` is outside a transition),
    /// validated with no render object at the drawable's scale (as `setModelState` validates the state it enters at a
    /// logic time). The barrels of the slot when they are valid (`BARRELS_VALID`), else 0; no state: 0.
    pub fn get_barrel_count_for_condition_state<A: W3dAssetSource>(
        &self,
        condition: &ModelConditionFlags,
        wslot: usize,
        scale: f32,
        standard_public_bones: &[Vec<u8>],
        env: &mut DrawEnv<'_, A>,
    ) -> i32 {
        let Some(si) = self.find_best_info(condition) else { return 0 };
        let s = &self.condition_states[si];
        s.validate_stuff(None, scale, &self.extra_public_bones, standard_public_bones, env);
        if s.barrels_valid() { s.weapon_barrels(wslot).len() as i32 } else { 0 }
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
    /// (#179) The motion itself: what `Animatable3DObjClass` evaluates the pose from (`ModeAnim.Motion`, an Add_Ref'd
    /// `HAnimClass*`). `None` from a source that gives only the name, frame count and rate (a test's): a render object
    /// playing such a handle evaluates the base pose (`Base_Update`), as every render object did before #179.
    pub data: Option<Arc<AnimData>>,
}

/// (#179) An animation as a render object holds it: the motion (shared with the asset manager, as upstream's
/// `HAnimClass*` is reference counted) and, for a morph animation, the pose animations its channels name. Upstream
/// resolves a morph channel's pose with `WW3DAssetManager::Get_HAnim` once, when the morph animation is loaded
/// (hmorphanim.cpp); here the poses are resolved by name ([`AssetManager::peek_anim_shared`], the newest of the name)
/// when the handle is made, transitively (a pose that is itself a morph animation brings its own). They agree unless an
/// animation of the same name is added in between.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimData {
    pub motion: Arc<HAnim>,
    pub poses: Vec<Arc<HAnim>>,
}

impl AnimData {
    /// The data of `motion` with its poses resolved through `assets` (see [`AnimData`]).
    pub fn new(motion: Arc<HAnim>, assets: &AssetManager) -> Self {
        let mut poses: Vec<Arc<HAnim>> = Vec::new();
        let mut todo = vec![motion.clone()];
        while let Some(a) = todo.pop() {
            if let HAnim::Morph(m) = &*a {
                for name in m.channels.iter().filter_map(|c| c.pose_name.as_deref()) {
                    if let Some(p) = assets.peek_anim_shared(name) {
                        if !poses.iter().any(|q| Arc::ptr_eq(q, &p)) && !Arc::ptr_eq(&p, &motion) {
                            poses.push(p.clone());
                            todo.push(p);
                        }
                    }
                }
            }
        }
        AnimData { motion, poses }
    }
}

/// (#179) A morph animation's poses as [`AnimData`] resolved them: the first of the name, ignoring case and anything
/// after a NUL (`Peek_Anim`'s comparison).
impl PoseLookup for AnimData {
    fn pose(&self, name: &[u8]) -> Option<&HAnim> {
        let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(name.len())];
        self.poses.iter().map(|p| &**p).find(|p| {
            let n = p.name();
            n[..n.iter().position(|&c| c == 0).unwrap_or(n.len())].eq_ignore_ascii_case(name)
        })
    }
}

/// (#179) Two animations' poses (`Blend_Update`'s and `Combo_Update`'s motions): the first's, then the second's.
struct PosesOf<'a>(&'a [&'a AnimData]);

impl PoseLookup for PosesOf<'_> {
    fn pose(&self, name: &[u8]) -> Option<&HAnim> {
        self.0.iter().find_map(|d| d.pose(name))
    }
}

/// What `W3DDisplay::m_assetManager` answers the draw module.
pub trait W3dAssetSource {
    /// `Create_Render_Obj(name, scale, color)`: `None` when there is no such
    /// render object; `Some(true)` for an HLOD (an `Animatable3DObjClass`),
    /// `Some(false)` for another kind (a mesh), which ignores animations.
    fn create_render_obj(&mut self, name: &[u8]) -> Option<bool>;
    /// `Get_HAnim(name)`.
    fn get_hanim(&mut self, name: &[u8]) -> Option<AnimHandle>;
    /// The hierarchy and sub-objects of a new render object of `name` ~~(`Create_Render_Obj`)~~
    /// **Corrected 2026-11-12 (#172):** made at the drawable's `scale` (`Create_Render_Obj(name, scale, color)`, whose
    /// hierarchy is then scaled as [`scale_render_obj_hierarchy`] scales it), posed at `pose`'s animation and frame (the
    /// base pose without one, or when the animation is not found) under a transform of `scale`
    /// (`Matrix3D(true).Scale(scale)`): what `Get_Bone_Index`, `Get_Bone_Transform`, `Get_Sub_Object_By_Name` and the
    /// hide-show functions read of it, with each bone's scaled base transform ([`SkeletonBone::base`]) for the chain
    /// from another root. `None` when it has none to give (no such render object, or a source without hierarchies); a
    /// mesh gives an empty [`Skeleton`].
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
    /// (#168) `WW3DAssetManager::Get_Texture(name)`'s top level `(Width, Height)` (`Get_Level_Description`), `None` when
    /// this source cannot tell (upstream's missing texture is then assumed: 128 x 128, aspect 1). New: the W3D textures'
    /// headers are the source's to read.
    fn texture_size(&mut self, _name: &[u8]) -> Option<(u32, u32)> {
        None
    }
}

/// A render object's bone: `Pivot[i]` of its `HTreeClass` as the draw module reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct SkeletonBone {
    /// `Pivot[i].Name`, up to its NUL.
    pub name: Vec<u8>,
    /// `Get_Parent_Index(i)`: the parent's index, 0 for none.
    pub parent_index: i32,
    /// `Get_Transform(i)` in the pose the skeleton was made for ~~.~~ **Corrected 2026-11-12 (#172):** under the root
    /// `Matrix3D(true).Scale(scale)`, over the pivots as the render object made at that scale holds them (root (A) of
    /// the module docs: what upstream reads after `Set_Transform(Matrix3D(true).Scale(getScale()))`).
    pub transform: Matrix3D,
    /// (#172) `Pivot[i].BaseTransform` as the render object holds it: as loaded, its translation scaled by
    /// `HTreeClass::Scale` when the render object was made at a scale ([`scale_render_obj_hierarchy`]). What
    /// [`Skeleton::bone_transform`] chains from another root.
    pub base: Matrix3D,
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
#[derive(Clone, Debug, PartialEq)]
pub struct Skeleton {
    /// The pivots; empty for a render object without a hierarchy (a mesh).
    pub bones: Vec<SkeletonBone>,
    pub sub_objects: Vec<SkeletonSubObject>,
    /// (#179) The hierarchy's `ScaleFactor` (1 as loaded; times the scale `HTreeClass::Scale` was given when the render
    /// object was made at one, [`scale_render_obj_hierarchy`]): what the animation updates multiply the animated
    /// translations by.
    pub scale_factor: f32,
}

impl Default for Skeleton {
    fn default() -> Self {
        Skeleton { bones: Vec::new(), sub_objects: Vec::new(), scale_factor: 1.0 }
    }
}

impl Skeleton {
    /// The skeleton of a posed [`RenderModel`].
    pub fn of(m: &RenderModel) -> Skeleton {
        let Some(tree) = (if m.is_hlod { m.htree.as_ref() } else { None }) else {
            return Skeleton::default();
        };
        let bones = Skeleton::bones_of(tree);
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
        Skeleton { bones, sub_objects, scale_factor: tree.scale_factor }
    }

    /// The bones of a posed hierarchy: each pivot's name, parent, posed transform (`Get_Transform(i)`) and (#172) base
    /// transform (`BaseTransform`).
    pub fn bones_of(tree: &HTree) -> Vec<SkeletonBone> {
        let mut bones = Vec::new();
        for i in 0..tree.num_pivots().max(0) as usize {
            let p = tree.pivot(i).unwrap_or_default();
            let end = p.name.iter().position(|&c| c == 0).unwrap_or(p.name.len());
            let parent_index = p.parent.and_then(|q| tree.pivot(q as usize)).map_or(0, |q| q.index);
            bones.push(SkeletonBone {
                name: p.name[..end].to_vec(),
                parent_index,
                transform: tree.get_transform(i as i32).unwrap_or(Matrix3D::IDENTITY),
                base: p.base_transform,
            });
        }
        bones
    }

    /// (#172) Bone `bone`'s transform under the render object's transform `root`, in the base pose: what
    /// `Animatable3DObjClass::Get_Bone_Transform(bone)` (animobj.cpp:658-675) gives after
    /// `Update_Sub_Object_Transforms` (780-802) runs `HTreeClass::Base_Update(Transform)` (htree.cpp:531-548): pivot 0
    /// is `root`, each other pivot `Multiply(Parent->Transform, BaseTransform)` in index order, so the chain is
    /// computed *from the root* (`((root * B1) * B2) ...`), never as `root * (B1 * B2 ...)`. Run by the port's own
    /// [`HTree::base_update`] over the bones' [`SkeletonBone::base`] and parents (~~no bone is captured here: the turret
    /// and recoil captures are not applied, as before~~ **Corrected 2026-11-13 (#174):** no bone is captured in this
    /// one; [`Skeleton::bone_transform_controlled`] applies a render object's captures). Without bones, or a bone out of
    /// range (an upstream assertion), or a hierarchy `Base_Update` cannot run (a parent out of range): `root`.
    ///
    /// (#179) No longer the render object's path: [`RenderObj::get_bone_transform`] evaluates its own live hierarchy (the
    /// animation's pose, cached while valid). Kept as the base-pose chain from a root.
    pub fn bone_transform(&self, root: &Matrix3D, bone: i32) -> Matrix3D {
        self.bone_transform_controlled(root, bone, &BTreeMap::new())
    }

    /// (#174) [`Skeleton::bone_transform`] with the render object's bone captures and controls (`Capture_Bone`,
    /// `Control_Bone`: [`RenderObj::bone_controls`]) on its pivots: `Base_Update` (htree.cpp:531-548) gives a captured
    /// pivot, after its base pose, `PivotClass::Capture_Update()` (pivot.cpp:137-166: `Transform.postMul(CapTransform)`,
    /// or with `WorldSpaceTranslation` the rotation part post-multiplied and the translation added), so the turret's turn
    /// reaches every bone under it (a muzzle bone under a turret follows the turret).
    pub fn bone_transform_controlled(&self, root: &Matrix3D, bone: i32, controls: &BTreeMap<i32, BoneControl>) -> Matrix3D {
        let n = self.bones.len();
        if n == 0 || usize::try_from(bone).map_or(true, |b| b >= n) {
            return *root;
        }
        if plant(9694) {
            let made = Skeleton { bones: self.bones.clone(), sub_objects: Vec::new(), scale_factor: self.scale_factor };
            return Matrix3D::multiply(root, &made.bone_transform_base_update(&Matrix3D::IDENTITY, bone, controls));
        }
        self.bone_transform_base_update(root, bone, controls)
    }

    /// [`Skeleton::bone_transform_controlled`]'s `Base_Update(root)` and `Get_Transform(bone)`.
    fn bone_transform_base_update(&self, root: &Matrix3D, bone: i32, controls: &BTreeMap<i32, BoneControl>) -> Matrix3D {
        let pivots = self
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let parent = if i == 0 { None } else if plant(9698) { Some(0) } else { u32::try_from(b.parent_index).ok() };
                let mut p = Pivot::new([0; W3D_NAME_LEN], parent, b.base, i as i32);
                // (#174) the pivot's `IsCaptured`, `CapTransform`, `WorldSpaceTranslation` as the render object's
                // `Capture_Bone` and `Control_Bone` left them
                if let Some(c) = controls.get(&(i as i32)).filter(|_| !plant(9751)) {
                    p.is_captured = c.captured;
                    p.cap_transform = c.transform;
                    p.world_space_translation = c.world_space_translation;
                }
                p
            })
            .collect();
        let mut tree = HTree { name: [0; W3D_NAME_LEN], num_pivots: self.bones.len() as i32, pivots, scale_factor: 1.0 };
        match tree.base_update(root) {
            Ok(()) => tree.get_transform(bone).unwrap_or(*root),
            Err(_) => *root,
        }
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

/// (#172) `ident_scale` (W3DAssetManager.cpp:84).
pub const IDENT_SCALE: f32 = 1.0;
/// (#172) `scale_epsilon` (W3DAssetManager.cpp:85).
pub const SCALE_EPSILON: f32 = 0.01;

/// (#172) What `W3DAssetManager::Create_Render_Obj(name, scale, color)` (W3DAssetManager.cpp:719-895) does to the new
/// render object's hierarchy: `reallyscale = (WWMath::Fabs(scale - ident_scale) > scale_epsilon)` (738), and when it
/// really scales, `rendobj->Scale(scale)` (858-859), whose `HLodClass::Scale` (hlod.cpp:3264-3290) ends in
/// `HTree->Scale(scale)` ([`HTree::scale`]: the pivots' translations). A scale within 0.01 of 1 leaves the hierarchy as
/// loaded (and only the root's `Matrix3D(true).Scale(scale)` of the (A) reads applies it). The meshes' own `Scale` (the
/// vertices) is the renderer's, not the skeleton's.
pub fn scale_render_obj_hierarchy(tree: &mut HTree, scale: f32) {
    let reallyscale = crate::ww::wwmath::fabs(scale - IDENT_SCALE) > SCALE_EPSILON;
    if (reallyscale || plant(9693)) && !plant(9692) {
        tree.scale(scale);
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
            Ok(Some(a)) => {
                let handle = AnimHandle { name: a.name().to_vec(), num_frames: a.get_num_frames(), frame_rate: get_frame_rate(a), data: None };
                // (#179) the motion itself, shared (`Get_HAnim` returns an Add_Ref'd pointer)
                let data = self.assets.peek_anim_shared(&handle.name).map(|m| Arc::new(AnimData::new(m, self.assets)));
                Some(AnimHandle { data, ..handle })
            }
            _ => None,
        }
    }
    /// The render model as drawn ([`RenderModel::create`]), ~~posed~~ **Corrected 2026-11-12 (#172):** its
    /// hierarchy scaled as `Create_Render_Obj(name, scale, color)` scales it ([`scale_render_obj_hierarchy`]), then
    /// posed by `Anim_Update` (or `Base_Update`) under the scaled identity, as `Validate_Transform` poses it. (#180) The
    /// `Anim_Update` is the virtual one ([`RenderModel::update`]), where upstream's render object takes animobj.h's inline
    /// one (the raw fast path for a raw animation, [`RenderModel::update_render_obj`]); kept, because this feeds the
    /// logic's art bones (#175) and its callers ask only integer frames (0 or `NumFrames - 1`), where the two agree bit
    /// for bit under the default `Float_To_Long` (the lib test `integer_frames_agree_on_both_paths_in_win32`).
    fn skeleton(&mut self, name: &[u8], pose: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
        let mut m = RenderModel::create(self.assets, self.fs, name).ok()??;
        if let Some(tree) = m.htree.as_mut() {
            scale_render_obj_hierarchy(tree, scale);
        }
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
    /// (#168) The texture's size, read by decoding it ([`crate::formats::w3d::render::load_texture_image`]); `None` for a
    /// texture that is not there or does not decode (upstream's missing texture).
    fn texture_size(&mut self, name: &[u8]) -> Option<(u32, u32)> {
        match crate::formats::w3d::render::load_texture_image(&self.assets.file_factory, self.fs, name) {
            Ok(Ok(img)) => Some((img.width, img.height)),
            _ => None,
        }
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

/// (#179) `Animatable3DObjClass::ModeInterp` (in `DOUBLE_ANIM`, `Set_Animation(motion0, frame0, motion1, frame1,
/// percentage)`): a blend of two animations. A motion may be `NULL` upstream (its `Blend_Update` then dereferences it:
/// undefined). `PrevFrame0`/`PrevFrame1` share their bytes with `ModeAnim`'s (a union) upstream; here they are the
/// previous blend's frames (0 at first): only the embedded sounds, not translated, read them.
#[derive(Clone, Debug, PartialEq)]
pub struct ModeInterp {
    pub motion0: Option<AnimHandle>,
    pub motion1: Option<AnimHandle>,
    pub frame0: f32,
    pub frame1: f32,
    pub prev_frame0: f32,
    pub prev_frame1: f32,
    pub percentage: f32,
}

/// (#179) `Animatable3DObjClass::ModeCombo` (in `MULTIPLE_ANIM`, `Set_Animation(HAnimComboClass *)`): the combination,
/// which upstream keeps as the caller's pointer (no reference is added: the caller may change it later), with its
/// entries' motions as indices into `motions`. Compared as upstream compares the pointers.
#[derive(Clone, Debug)]
pub struct ModeCombo {
    pub combo: Arc<HAnimCombo<usize>>,
    pub motions: Arc<Vec<Arc<AnimData>>>,
}

impl PartialEq for ModeCombo {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.combo, &other.combo) && Arc::ptr_eq(&self.motions, &other.motions)
    }
}

/// Where a handled `handleWeaponFireFX` plays its FX list (GitHub #43).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFireFxAt {
    /// The FX bone's transform ~~in the model's space (its pose as made)~~ **Corrected 2026-11-12 (#172):** in the world,
    /// as `Get_Bone_Transform` gives it (the chain from the render object's transform over the scaled pivots, base
    /// pose); `None` when the asset source gave no skeleton.
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
    /// `CurMotionMode == SINGLE_ANIM`'s `ModeAnim`; `None` in `BASE_POSE` ~~.~~ (#179) and in the two modes below.
    pub anim: Option<ModeAnim>,
    /// (#179) `CurMotionMode == DOUBLE_ANIM`'s `ModeInterp` (`None` otherwise).
    pub mode_interp: Option<ModeInterp>,
    /// (#179) `CurMotionMode == MULTIPLE_ANIM`'s `ModeCombo` (`None` otherwise). With none of the three set the mode is
    /// `BASE_POSE`; at most one is set.
    pub mode_combo: Option<ModeCombo>,
    /// (#179) `IsTreeValid` (`Is_Hierarchy_Valid()`): whether the hierarchy's pivots hold the pose for the current
    /// transform and animation state. 0 as made; see [`RenderObj::get_bone_transform`].
    is_tree_valid: bool,
    /// (#179) `HTree`: the render object's own copy of its hierarchy (made from [`RenderObj::skeleton`] the first time it
    /// is updated), whose pivots keep the transforms of the last update (`Get_Bone_Transform` answers them while the tree
    /// is valid, and a parent later in the array contributes its previous transform, as upstream's). Its capture state is
    /// [`RenderObj::bone_controls`]'s, put on before each update.
    live_tree: Option<HTree>,
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
    /// (#174) `Transform` (`Set_Transform`, `Get_Transform`): what `doDrawModule` (W3DModelDraw.cpp:2094-2100),
    /// `reactToTransformChange` (3647-3660) and `setModelState` (3155) set it to. The identity as made.
    pub transform: Matrix3D,
    /// (#174) `ObjectScale` (`Set_ObjectScale`, rendobj.h:454): the drawable's instance scale, which `doDrawModule` gives
    /// it when it is not 1 (W3DModelDraw.cpp:2052-2053); 1 as made. Kept for the renderer (upstream folds it into the
    /// transform it is given; the bones do not read it).
    pub object_scale: f32,
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
            mode_interp: None,
            mode_combo: None,
            is_tree_valid: false,
            live_tree: None,
            last_frame: 0.0,
            hidden: false,
            skeleton: None,
            bone_controls: BTreeMap::new(),
            sub_object_hidden: Vec::new(),
            material_overrides: BTreeMap::new(),
            transform: Matrix3D::IDENTITY,
            object_scale: 1.0,
        }
    }

    /// The render object with its hierarchy and sub-objects (their hidden flags as made).
    pub fn with_skeleton(mut self, skeleton: Option<Skeleton>) -> Self {
        self.sub_object_hidden = skeleton.as_ref().map_or_else(Vec::new, |k| k.sub_objects.iter().map(|s| s.hidden).collect());
        self.skeleton = skeleton.map(Arc::new);
        // (#179) a new hierarchy: made again from it at the next update
        self.live_tree = None;
        self.is_tree_valid = false;
        self
    }

    /// `Get_HTree()` is not `NULL`: an HLOD with a hierarchy.
    fn has_htree(&self) -> bool {
        self.is_hlod && self.skeleton.as_ref().is_some_and(|k| k.has_htree())
    }

    /// (#171) `Get_Bone_Index(bonename)`: an HLOD's hierarchy's (`HTreeClass::Get_Bone_Index`, htree.cpp:922-930: the
    /// first pivot of that name ignoring case, else 0); 0 for any other render object (`RenderObjClass`'s, rendobj.h:354)
    /// or without a hierarchy (`Animatable3DObjClass`'s).
    pub fn get_bone_index(&self, bonename: &[u8]) -> i32 {
        if !self.has_htree() {
            return 0;
        }
        self.skeleton.as_ref().map_or(0, |k| k.get_bone_index(bonename))
    }

    /// (#171) `Get_Bone_Transform(boneindex)` under `Set_Transform(Matrix3D(true).Scale(scale))` (the drawable's scale):
    /// the skeleton's pose as made (animobj.cpp:658-675 evaluates the current animation; here the base pose, the
    /// module docs). (#172) That is root (A) of the module docs: the chain from the scale over the scaled pivots
    /// ([`SkeletonBone::transform`]). Without a hierarchy, or out of range, the render object's own transform: the
    /// identity here.
    pub fn get_bone_transform_scaled(&self, boneindex: i32) -> Matrix3D {
        if !self.has_htree() {
            return Matrix3D::IDENTITY;
        }
        let k = self.skeleton.as_ref().expect("a hierarchy");
        usize::try_from(boneindex).ok().and_then(|b| k.bones.get(b)).map_or(Matrix3D::IDENTITY, |b| b.transform)
    }

    /// (#179) `Set_Transform(m)` (`Animatable3DObjClass::Set_Transform`, animobj.cpp:347-351, after
    /// `HLodClass::Set_Transform`, hlod.cpp:2222-2226): the transform, and the hierarchy no longer valid
    /// (`Set_Hierarchy_Valid(false)`): the next bone query evaluates the pose again.
    pub fn set_transform(&mut self, m: &Matrix3D) {
        self.transform = *m;
        if !plant(9802) {
            self.is_tree_valid = false;
        }
    }

    /// (#179) `Is_Hierarchy_Valid()` (animobj.h:157).
    pub fn is_hierarchy_valid(&self) -> bool {
        self.is_tree_valid
    }

    /// (#179) The live hierarchy, made from the skeleton the first time (`new HTreeClass(*source)`: the pivots as the
    /// asset source scaled them, their transforms the identity), with the render object's bone captures and controls on
    /// its pivots (`Capture_Bone`/`Control_Bone`/`Release_Bone` wrote them to the pivots upstream). `None` without a
    /// skeleton with bones (an HLOD whose source gave none, a mesh).
    fn prepared_tree(&mut self) -> Option<&mut HTree> {
        let k = self.skeleton.clone().filter(|k| !k.bones.is_empty())?;
        let stale = self.live_tree.as_ref().is_none_or(|t| t.num_pivots as usize != k.bones.len()) || plant(9819);
        if stale {
            let pivots = k
                .bones
                .iter()
                .enumerate()
                .map(|(i, b)| {
                    let parent = if i == 0 { None } else if plant(9698) { Some(0) } else { u32::try_from(b.parent_index).ok() };
                    Pivot::new([0; W3D_NAME_LEN], parent, b.base, i as i32)
                })
                .collect();
            let scale_factor = if plant(9813) { 1.0 } else { k.scale_factor };
            self.live_tree = Some(HTree { name: [0; W3D_NAME_LEN], num_pivots: k.bones.len() as i32, pivots, scale_factor });
        }
        let tree = self.live_tree.as_mut().expect("made above");
        for (i, p) in tree.pivots.iter_mut().enumerate() {
            // (#174) the pivot's `IsCaptured`, `CapTransform`, `WorldSpaceTranslation` as the render object's
            // `Capture_Bone`, `Control_Bone` and `Release_Bone` left them
            let c = if plant(9751) { BoneControl::default() } else { self.bone_controls.get(&(i as i32)).copied().unwrap_or_default() };
            p.is_captured = c.captured;
            p.cap_transform = c.transform;
            p.world_space_translation = c.world_space_translation;
        }
        Some(tree)
    }

    /// (#179) One of `HTreeClass`'s updates of the live hierarchy under `root` (nothing without one). (#172 plant 9694:
    /// the chain from the identity, then the root put before each pivot.)
    fn tree_update(&mut self, root: &Matrix3D, f: impl FnOnce(&mut HTree, &Matrix3D) -> W3dResult<()>) -> W3dResult<()> {
        let Some(tree) = self.prepared_tree() else { return Ok(()) };
        if plant(9694) {
            let r = f(tree, &Matrix3D::IDENTITY);
            for p in &mut tree.pivots {
                p.transform = Matrix3D::multiply(root, &p.transform);
            }
            return r;
        }
        f(tree, root)
    }

    /// (#179) `Single_Anim_Progress()` (animobj.cpp:1048-1075) at sync time `now` (`WW3D::Get_Sync_Time()`): in
    /// `SINGLE_ANIM`, the frame and direction moved on by the time since `LastSyncTime` (`Compute_Current_Frame`,
    /// [`compute_current_frame`]: `float sync_time_diff = WW3D::Get_Sync_Time() - ModeAnim.LastSyncTime`, an unsigned
    /// difference, so a sync time earlier than the last wraps), `PrevFrame` the old frame (or kept when the frame did not
    /// move: "This function was somehow called twice per frame"), `LastSyncTime` now, and the hierarchy no longer valid.
    /// Its callers skip it in `ANIM_MODE_MANUAL`.
    fn single_anim_progress(&mut self, now: u32) {
        let Some(a) = &mut self.anim else { return };
        let oldprev = a.prev_frame;
        a.prev_frame = a.frame;
        let diff = if plant(9818) { (i64::from(now) - i64::from(a.last_sync_time)) as f32 } else { now.wrapping_sub(a.last_sync_time) as f32 };
        let (f, d) = compute_current_frame(a.frame, a.motion.num_frames, a.motion.frame_rate, a.frame_rate_multiplier, a.anim_direction, diff, a.anim_mode);
        a.frame = f;
        a.anim_direction = d;
        a.last_sync_time = now;
        if a.frame == a.prev_frame && !plant(9814) {
            // This function was somehow called twice per frame.
            a.prev_frame = oldprev;
        }
        self.last_frame = a.frame;
        // Force the heirarchy to be recalculated
        if !plant(9806) {
            self.is_tree_valid = false;
        }
    }

    /// (#179) `Update_Sub_Object_Transforms()` (animobj.cpp:780-860, through `HLodClass`'s, hlod.cpp:3351-3388, whose
    /// sub-object part is the renderer's) at sync time `now`: by the motion mode, `BASE_POSE` `Base_Update(Transform)`;
    /// `SINGLE_ANIM` `Single_Anim_Progress()` unless `ANIM_MODE_MANUAL`, then the inline `Anim_Update(Transform, Motion,
    /// Frame)` (animobj.h:256-268), which takes `HTreeClass`'s raw fast path (`Anim_Update(Transform, (HRawAnimClass *)
    /// Motion, Frame)`: one integer frame, [`HTree::anim_update_raw`]) for a raw animation (`CLASSID_HRAWANIM`) and the
    /// virtual one ([`HTree::anim_update`]) for the others ([`HTree::anim_update_animobj`], #180); `DOUBLE_ANIM` `Blend_Update(Transform, Motion0, Frame0,
    /// Motion1, Frame1, Percentage)`; `MULTIPLE_ANIM` `Combo_Update(Transform, AnimCombo)`; then the hierarchy is valid.
    /// The embedded sounds that follow each update upstream (`AnimatedSoundMgrClass::Trigger_Sound`) are not translated:
    /// a motion has embedded sounds only when the animated-sound manager named a bone for it, and the port has none.
    /// A handle without data ([`AnimHandle::data`]): the base pose. Undefined upstream (a `NULL` motion of a blend or a
    /// `NULL` combo dereferenced, or an evaluation the [`HTree`] updates report): the error, with the hierarchy valid and
    /// its pivots as far as the update got.
    pub fn update_sub_object_transforms(&mut self, now: u32) -> W3dResult<()> {
        // `CompositeRenderObjClass::Update_Sub_Object_Transforms()`: `RenderObjClass`'s, nothing
        let root = if plant(9810) { Matrix3D::IDENTITY } else { self.transform };
        let result = if self.anim.is_some() {
            if self.anim.as_ref().is_some_and(|a| a.anim_mode != AnimMode::Manual || plant(9804)) && !plant(9803) {
                self.single_anim_progress(now);
            }
            let a = self.anim.as_ref().expect("SINGLE_ANIM");
            let (data, frame) = (a.motion.data.clone(), a.frame);
            match data {
                None => self.tree_update(&root, |t, r| t.base_update(r)),
                // (#180) the inline `Anim_Update`'s dispatch, shared with the client's drawn pose
                Some(d) => self.tree_update(&root, |t, r| t.anim_update_animobj(r, &d.motion, &*d, frame)),
            }
        } else if let Some(i) = self.mode_interp.clone() {
            match (&i.motion0, &i.motion1) {
                (Some(m0), Some(m1)) => match (&m0.data, &m1.data) {
                    (Some(d0), Some(d1)) => {
                        let pct = if plant(9811) { 1.0 - i.percentage } else { i.percentage };
                        let poses = PosesOf(&[&**d0, &**d1]);
                        self.tree_update(&root, |t, r| t.blend_update(r, &d0.motion, i.frame0, &d1.motion, i.frame1, pct, &poses))
                    }
                    _ => self.tree_update(&root, |t, r| t.base_update(r)),
                },
                // `ModeInterp.Motion0->...` of a NULL motion (htree.cpp's Blend_Update reads its pivot count)
                _ => Err(crate::formats::w3d::W3dUndefined::NullPointer.into()),
            }
        } else if let Some(c) = self.mode_combo.clone() {
            if plant(9812) {
                self.tree_update(&root, |t, r| t.base_update(r))
            } else {
                let motions = c.motions.clone();
                let datas: Vec<&AnimData> = motions.iter().map(|d| &**d).collect();
                let poses = PosesOf(&datas);
                self.tree_update(&root, |t, r| t.combo_update(r, &c.combo, &poses, |&k: &usize| &*motions[k].motion))
            }
        } else {
            self.tree_update(&root, |t, r| t.base_update(r))
        };
        self.is_tree_valid = true;
        result
    }

    /// (#179) `Get_Bone_Transform(boneindex)` (animobj.cpp:658-675) at sync time `now`: `Validate_Transform()` (nothing:
    /// the render object is in no container), then, with a hierarchy (an HLOD, `Animatable3DObjClass`), when it is not
    /// valid `Update_Sub_Object_Transforms()` ([`RenderObj::update_sub_object_transforms`]: the animation moved on to `now`
    /// unless `MANUAL`, and its pose evaluated under the render object's transform with the captured bones), and the
    /// pivot's transform; while it is valid, the pose of the last update (a later query at a later sync time answers the
    /// same pose until something invalidates it: `Set_Transform`, `Set_Animation`, `Control_Bone`, `Render`'s progress).
    /// So the query **changes the animation state** as upstream's does (the frame, `PrevFrame`, `LastSyncTime`, the
    /// direction of a ping-pong). Another render object (`RenderObjClass::Get_Bone_Transform`, rendobj.h): its
    /// transform. A bone out of range (an upstream assertion) or an update undefined upstream: the transform.
    pub fn get_bone_transform(&mut self, boneindex: i32, now: u32) -> Matrix3D {
        let fallback = self.transform;
        self.try_get_bone_transform(boneindex, now).unwrap_or(fallback)
    }

    /// (#179) [`RenderObj::get_bone_transform`] with what upstream leaves undefined as an error.
    pub fn try_get_bone_transform(&mut self, boneindex: i32, now: u32) -> W3dResult<Matrix3D> {
        if !self.is_hlod {
            return Ok(self.transform);
        }
        // If our hierarchy isn't valid, we just need to evaluate our animation state.
        if !self.is_tree_valid || plant(9801) {
            self.update_sub_object_transforms(now)?;
        }
        match &self.live_tree {
            Some(t) => t.get_transform(boneindex),
            // an HLOD whose source gave no hierarchy (upstream's default one: pivot 0, the transform)
            None => Ok(self.transform),
        }
    }

    /// (#179) `Set_Transform(root)`, `Get_Bone_Transform(boneindex)`, `Set_Transform(original)`: how
    /// `recalcBonesForClientParticleSystems` reads a bone "in modelspace, not world space" (W3DModelDraw.cpp:2611-2621,
    /// and `validateCachedBones`, 672-702): the pose under `root`, the transform restored, and the hierarchy left not
    /// valid by the restore.
    pub fn get_bone_transform_under(&mut self, root: &Matrix3D, boneindex: i32, now: u32) -> Matrix3D {
        let original_transform = self.transform;
        self.set_transform(root);
        let bone_transform = self.get_bone_transform(boneindex, now);
        if plant(9816) {
            self.transform = original_transform;
        } else {
            self.set_transform(&original_transform);
        }
        bone_transform
    }

    /// (#179) What [`RenderObj::get_bone_transform`] would answer at `now`, after `Set_Transform(root)` when `root` is
    /// given, with nothing written: for a caller that may only read the render object (another drawable's, or a
    /// `const` path of the port). Upstream's query writes the animation state it moves on and validates the tree; a
    /// later query of this render object can then answer differently from upstream's (#179's gaps).
    pub fn peek_bone_transform(&self, root: Option<&Matrix3D>, boneindex: i32, now: u32) -> Matrix3D {
        let mut r = self.clone();
        if let Some(root) = root {
            r.set_transform(root);
        }
        r.get_bone_transform(boneindex, now)
    }

    /// `Capture_Bone(boneindex)` (`Animatable3DObjClass`: nothing without a hierarchy; a bone
    /// out of range is an upstream assertion, ignored). (#179) The hierarchy stays valid (upstream's does not invalidate
    /// it: the capture shows at the next update).
    pub fn capture_bone(&mut self, boneindex: i32) {
        if self.has_htree() && self.skeleton.as_ref().is_some_and(|k| boneindex >= 0 && (boneindex as usize) < k.bones.len()) {
            self.bone_controls.entry(boneindex).or_default().captured = true;
        }
    }

    /// (#179) `Release_Bone(boneindex)` (animobj.cpp:710-715, htree.cpp:1016-1031: `IsCaptured = false`, the control kept;
    /// the hierarchy stays valid).
    pub fn release_bone(&mut self, boneindex: i32) {
        if self.has_htree() && self.skeleton.as_ref().is_some_and(|k| boneindex >= 0 && (boneindex as usize) < k.bones.len()) && !plant(9817) {
            self.bone_controls.entry(boneindex).or_default().captured = false;
        }
    }

    /// `Control_Bone(bindex, objtm, world_space_translation = false)`.
    pub fn control_bone(&mut self, bindex: i32, objtm: &Matrix3D) {
        self.control_bone_world(bindex, objtm, false);
    }

    /// (#179) `Control_Bone(bindex, objtm, world_space_translation)` (animobj.cpp:752-765): the pivot's control, and
    /// with a hierarchy (an HLOD) the hierarchy no longer valid.
    pub fn control_bone_world(&mut self, bindex: i32, objtm: &Matrix3D, world_space_translation: bool) {
        if self.has_htree() && self.skeleton.as_ref().is_some_and(|k| bindex >= 0 && (bindex as usize) < k.bones.len()) {
            let c = self.bone_controls.entry(bindex).or_default();
            c.transform = *objtm;
            c.world_space_translation = world_space_translation;
        }
        if self.is_hlod && !plant(9807) {
            self.is_tree_valid = false;
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
    /// render object that is not an HLOD). (#179) `Release()` of the previous mode, and the hierarchy no longer valid
    /// (animobj.cpp:472-506); `None` is `BASE_POSE`.
    pub fn set_animation(&mut self, motion: Option<AnimHandle>, frame: f32, mode: AnimMode, now: u32) {
        if !self.is_hlod {
            return;
        }
        self.mode_interp = None;
        self.mode_combo = None;
        match motion {
            Some(motion) => {
                let prev_frame = self.anim.as_ref().map_or(self.last_frame, |a| a.frame);
                let anim_direction = if anim_mode_index(mode) < anim_mode_index(AnimMode::LoopBackwards) { 1.0 } else { -1.0 };
                self.anim = Some(ModeAnim { motion, frame, prev_frame, last_sync_time: now, frame_rate_multiplier: 1.0, anim_direction, anim_mode: mode });
                self.last_frame = frame;
            }
            None => self.anim = None,
        }
        if !plant(9808) {
            self.is_tree_valid = false;
        }
    }

    /// (#179) `Set_Animation()` (animobj.cpp:452-457): `Release()`, `BASE_POSE`, the hierarchy no longer valid.
    pub fn set_animation_base_pose(&mut self) {
        if !self.is_hlod {
            return;
        }
        if !plant(9815) {
            self.anim = None;
            self.mode_interp = None;
            self.mode_combo = None;
        }
        self.is_tree_valid = false;
    }

    /// (#179) `Set_Animation(motion0, frame0, motion1, frame1, percentage)` (animobj.cpp:519-560): `DOUBLE_ANIM`, a blend
    /// of two animations (each may be `None`, upstream's `NULL`), the hierarchy no longer valid.
    pub fn set_animation_blend(&mut self, motion0: Option<AnimHandle>, frame0: f32, motion1: Option<AnimHandle>, frame1: f32, percentage: f32) {
        if !self.is_hlod {
            return;
        }
        let (prev_frame0, prev_frame1) = self.mode_interp.as_ref().map_or((0.0, 0.0), |i| (i.frame0, i.frame1));
        if let Some(a) = &self.anim {
            self.last_frame = a.frame;
        }
        self.anim = None;
        self.mode_combo = None;
        self.mode_interp = Some(ModeInterp { motion0, motion1, frame0, frame1, prev_frame0, prev_frame1, percentage });
        self.is_tree_valid = false;
    }

    /// (#179) `Set_Animation(HAnimComboClass *anim_combo)` (animobj.cpp:572-597): `MULTIPLE_ANIM` (`None` is upstream's
    /// `NULL` combo, which the next update dereferences: undefined), the hierarchy no longer valid.
    pub fn set_animation_combo(&mut self, combo: Option<ModeCombo>) {
        if !self.is_hlod {
            return;
        }
        self.anim = None;
        self.mode_interp = None;
        self.mode_combo = combo;
        self.is_tree_valid = false;
    }

    /// ~~`Single_Anim_Progress()` at sync time `now` (what `Render` does before it draws, unless the mode is `MANUAL`).~~
    /// **Corrected 2026-11-13 (#179):** `Animatable3DObjClass::Render` (animobj.cpp:288-305, after `HLodClass::Render`'s
    /// `Is_Not_Hidden_At_All()`, hlod.cpp:2146-2154) at sync time `now`: nothing for a render object that is not an HLOD
    /// (no `HTree`) or that is hidden; `Single_Anim_Progress()` unless `MANUAL`; then, the hierarchy not valid,
    /// `Update_Sub_Object_Transforms()` (which moves the animation on again at the same sync time: by nothing, except a
    /// frame left on a wrap point). `Are_Sub_Object_Transforms_Dirty()` is not kept: every call that sets it
    /// (`HLodClass::Set_Transform`) also invalidates the hierarchy, and the update clears both.
    pub fn render(&mut self, now: u32) {
        let _ = self.try_render(now);
    }

    /// (#179) [`RenderObj::render`] with what upstream leaves undefined as an error.
    pub fn try_render(&mut self, now: u32) -> W3dResult<()> {
        if !self.is_hlod || self.hidden {
            return Ok(());
        }
        if self.anim.as_ref().is_some_and(|a| a.anim_mode != AnimMode::Manual) {
            self.single_anim_progress(now);
        }
        if !self.is_tree_valid && !plant(9809) {
            self.update_sub_object_transforms(now)?;
        }
        Ok(())
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
    /// (#171) `TheParticleSystemManager`, for the bone particle systems (`None`: no system is found or made).
    pub particles: Option<DrawParticles<'a>>,
}

/// (#171) The client's particle systems as a draw module reaches them: `TheParticleSystemManager`, and the frame a new
/// system's `m_startTimestamp` takes (`TheGameClient->getFrame()`, the logic's frame in the port, as the other makers').
pub struct DrawParticles<'a> {
    pub manager: &'a mut ParticleSystemManager,
    pub frame: u32,
}

/// (#171) `W3DModelDraw::ParticleSysTrackerType` (W3DModelDraw.h:481-485): a bone particle system the module made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParticleSysTrackerType {
    /// `id`.
    pub id: ParticleSystemId,
    /// `boneIndex`: the render object's bone it was made at (0: none).
    pub bone_index: i32,
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
    /// (#171) The render object's transform as `doDrawModule` sets it (`m_renderObject->Set_Transform(mtx)`), without
    /// the drawable's scale (which ~~the port's [`Skeleton`] carries~~ **Corrected 2026-11-12 (#172):** upstream's
    /// render object carries in its hierarchy's translations, as the port's [`Skeleton`] does): what
    /// `updateBonesForClientParticleSystems` chains the bones from. The object's transform matrix ([`crate::client::drawable::ClientDrawables::draw_facts`]).
    /// **Corrected 2026-11-13 (#174):** this is the drawable's transform (`transformMtx`); the render object's is made
    /// from it by `doDrawModule` (the instance scale and `adjustTransformMtx`, [`RenderObj::transform`]), which the
    /// bones chain from.
    pub transform: Matrix3D,
    /// (#174) The object's `getConstructionPercent()` and `getGeometryInfo().getMaxHeightAbovePosition()`, which
    /// `adjustTransformMtx` reads for a state flagged `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`; `None` without an object.
    pub construction: Option<(f32, f32)>,
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
/// not reach them yet; **Corrected 2026-11-12 (#171):** they reach them (`DrawEnv::particles`, for the bone particle
/// systems), but the tread debris is still not made).
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
    /// (#171) `m_needRecalcBoneParticleSystems`.
    pub need_recalc_bone_particle_systems: bool,
    /// (#171) `m_fullyObscuredByShroud`.
    pub fully_obscured_by_shroud: bool,
    /// (#171) `m_particleSystemIDs`: the bone particle systems made for the current state.
    pub particle_system_ids: Vec<ParticleSysTrackerType>,
    /// (#171) `getDrawable()->getID()`, what `attachToDrawable` keeps: the port's drawable id for particles is its
    /// object's ([`crate::client::particle_sys::ParticleSystem::attached_to_drawable`]); set by the drawable that makes
    /// the module (0 until then: none).
    pub drawable_id: ObjectId,
    /// (#171) `getDrawable()->testDrawableStatus(DRAWABLE_STATUS_NO_STATE_PARTICLES)`: no bone particle systems. Upstream
    /// sets it only on the in-game UI's placement previews (InGameUI.cpp:1551, 3040), which the port's drawables are not:
    /// false unless a caller sets it.
    pub drawable_no_state_particles: bool,
    /// (#174) `*getDrawable()->getTransformMatrix()` as the module was last given it ([`W3DModelDraw::do_draw_module`]'s
    /// [`DrawFacts::transform`], [`W3DModelDraw::react_to_transform_change`]): what `nukeCurrentRender(&transform)`
    /// (W3DModelDraw.cpp:2811-2816) hands a new render object when there was no old one. Upstream reads the drawable's
    /// current transform there; the module has no drawable to read, so the last one it was given (the identity before
    /// any).
    pub drawable_transform: Matrix3D,
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
            // `m_needRecalcBoneParticleSystems = false; m_fullyObscuredByShroud = false;` (W3DModelDraw.cpp:1740-1741)
            need_recalc_bone_particle_systems: false,
            fully_obscured_by_shroud: false,
            particle_system_ids: Vec::new(),
            drawable_id: 0,
            drawable_no_state_particles: false,
            drawable_transform: Matrix3D::IDENTITY,
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
    /// (#174) and its `xform`: the old render object's transform (`Get_Transform()`), or without one the drawable's
    /// ([`W3DModelDraw::drawable_transform`]).
    fn nuke_current_render(&mut self) -> Matrix3D {
        self.pause_animation = false;
        // save the transform for the new model
        let xform = match &self.render_object {
            Some(r) => r.transform,
            None => self.drawable_transform,
        };
        self.render_object = None;
        xform
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

        // (#171, W3DModelDraw.cpp:2997-3006) Set up any particle effects for the new model. Also stop the old ones;
        // none are placed for a drawable whose status says not to
        if !self.drawable_no_state_particles || plant(9681) {
            self.need_recalc_bone_particle_systems = true;
        }
        // always stop particle systems now
        if !plant(9682) {
            self.stop_client_particle_systems(env.particles.as_mut().map(|p| &mut *p.manager));
        }

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
            let transform = self.nuke_current_render();
            if !ns.model_name.is_empty() {
                if let Some(hlod) = env.assets.create_render_obj(&ns.model_name) {
                    let skeleton = env.assets.skeleton(&ns.model_name, None, self.scale);
                    let mut r = RenderObj::new(&ns.model_name, hlod).with_skeleton(skeleton);
                    // preserve the visibility state of the previous render object
                    r.hidden = self.drawable_hidden;
                    // (#174, W3DModelDraw.cpp:3150-3155) "set the transform for the new model to that we saved before, we do
                    // this so that the model transition is smooth"
                    if !plant(9754) {
                        r.set_transform(&transform);
                    }
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
    ///
    /// (#174) The render object's transform (W3DModelDraw.cpp:2045-2054, 2094-2100): the drawable's transform
    /// ([`DrawFacts::transform`]) scaled by `getDrawable()->getInstanceScale()` when that is not 1 (the module's
    /// [`W3DModelDraw::scale`]: upstream's `getScale()` and `getInstanceScale()` both answer `m_instanceScale`; the render
    /// object then also gets `Set_ObjectScale`), adjusted by [`W3DModelDraw::adjust_transform_mtx`], and set
    /// (`Set_Transform`, [`RenderObj::transform`]) before the turret, the bone particle systems and the recoil read it.
    pub(crate) fn model_draw_do_draw_module<A: W3dAssetSource>(&mut self, facts: &DrawFacts, env: &mut DrawEnv<'_, A>) {
        let should_animate = if self.data.animations_require_power { facts.should_animate_with_power } else { facts.should_animate_without_power };
        self.set_pause_animation(!should_animate, env.sync_time);

        self.drawable_transform = facts.transform;
        let mut transform_mtx = facts.transform;
        if self.scale != 1.0 && !plant(9755) {
            // do custom scaling of the W3D model.
            transform_mtx.scale(self.scale);
            if let Some(r) = &mut self.render_object {
                r.object_scale = self.scale;
            }
        }

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

        // set our position in the render object to our position of the drawable
        if self.render_object.is_some() {
            let mut mtx = transform_mtx;
            self.adjust_transform_mtx(&mut mtx, facts.construction);
            if let Some(r) = &mut self.render_object {
                r.set_transform(&mtx);
            }
        }

        self.handle_client_turret_positioning(&facts.turrets);
        // (#171, W3DModelDraw.cpp:2103-2107) the bone particle systems: made when the state changed, and moved to their
        // bones each draw when the module data says they follow animated bones
        if self.render_object.is_some() || !plant(9688) {
            self.recalc_bones_for_client_particle_systems(env.particles.as_mut(), env.random, env.sync_time);
        }
        if (self.data.particles_attached_to_animated_bones || plant(9686)) && !plant(9687) {
            // (#174) the render object's transform, as just set (#179: its own, as upstream's call reads it)
            self.update_bones_for_client_particle_systems(env.particles.as_mut().map(|p| &mut *p.manager), env.sync_time);
        }
        self.handle_client_recoil();
    }

    /// (#174) `adjustTransformMtx(mtx)` (W3DModelDraw.cpp:1993-2036), `CACHE_ATTACH_BONE` defined as upstream builds it
    /// (W3DModelDraw.h:278): with an `AttachToBoneInAnotherModule` bone, its cached pristine offset
    /// ([`W3DModelDrawModuleData::attach_to_drawable_bone_offset_cached`]) turned by the matrix (`Rotate_Vector`) and
    /// added to its translation; then, for a current state flagged `ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT`, an object
    /// with a construction percent of 0 or more is lowered along its Z by the part not yet built
    /// (`Translate_Z(-height + (height * pct / 100.0f))`). `construction` is the object's
    /// (`getConstructionPercent()`, `getGeometryInfo().getMaxHeightAbovePosition()`; [`DrawFacts::construction`],
    /// `None` without an object). Upstream reads `m_curState->m_flags` unguarded (its callers have a render object, so a
    /// state); without a state nothing is lowered.
    pub fn adjust_transform_mtx(&self, mtx: &mut Matrix3D, construction: Option<(f32, f32)>) {
        if let Some(offset) = self.data.attach_to_drawable_bone_offset_cached() {
            let tmp = if plant(9756) { offset } else { mtx.rotate_vector(&offset) };
            mtx.adjust_x_translation(tmp.x);
            mtx.adjust_y_translation(tmp.y);
            mtx.adjust_z_translation(tmp.z);
        }
        let adjust_height = self.get_cur_state().is_some_and(|c| test_flag_bit(c.flags, ADJUST_HEIGHT_BY_CONSTRUCTION_PERCENT));
        if adjust_height && !plant(9757) {
            if let Some((pct, height)) = construction {
                if pct >= 0.0 {
                    if plant(9758) {
                        mtx.adjust_z_translation(-height + (height * pct / 100.0));
                    } else {
                        mtx.translate_z(-height + (height * pct / 100.0));
                    }
                }
            }
        }
    }

    /// (#174) `reactToTransformChange(oldMtx, oldPos, oldAngle)` (W3DModelDraw.cpp:3647-3659): the render object's
    /// transform set to the drawable's (`transform`, its object's) adjusted by [`W3DModelDraw::adjust_transform_mtx`], no
    /// instance scale. Upstream calls it each time the logic sets the object's transform; the port's client is not told
    /// (the logic sends no event), so the client calls it before a bone query made outside a draw (the weapon's fire
    /// FX, the laser's and the animated particle bones' client updates): exact for an object the logic moved since the
    /// last draw, and for any object whose instance scale is 1 and which did not move (#174's gaps). The tracks
    /// (3661-3677) are not translated (no track render objects).
    pub fn react_to_transform_change(&mut self, transform: &Matrix3D, construction: Option<(f32, f32)>) {
        self.drawable_transform = *transform;
        // set the position of our render object
        if self.render_object.is_some() {
            let mtx = self.reacted_transform(transform, construction);
            if let Some(r) = &mut self.render_object {
                r.set_transform(&mtx);
            }
        }
    }

    /// (#174) The render object's transform [`W3DModelDraw::react_to_transform_change`] would set for the drawable's
    /// `transform`, nothing written: for a bone query of another drawable, which the asker may only read.
    pub fn reacted_transform(&self, transform: &Matrix3D, construction: Option<(f32, f32)>) -> Matrix3D {
        let mut mtx = *transform;
        self.adjust_transform_mtx(&mut mtx, construction);
        mtx
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
    ///   modules do not reach the client's particle systems yet (**Corrected 2026-11-12 (#171):** they do now, through
    ///   `DrawEnv::particles`; the debris itself is still not made).
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
    pub fn handle_weapon_fire_fx(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, sync_time: u32) -> bool {
        // where it plays is not asked: any render object transform
        self.handle_weapon_fire_fx_at(wslot, specific_barrel_to_use, has_fxl, sync_time).is_some()
    }

    /// `handleWeaponFireFX(wslot, specificBarrelToUse, fxl, ...)`: the barrel's recoil started and its muzzle flash
    /// shown; when it is handled (an FX list, the barrel's FX bone and a render object), where upstream's
    /// `FXList::doFXPos` plays: ~~not drawn yet~~ **since GitHub #43 (2026-11-06)** the caller plays it
    /// (`crate::client::client_fx`) at the bone the answer names, or at the logic object when the render object is
    /// hidden. (#172) ~~`transform` is the render object's (the drawable's, as `doDrawModule` sets it)~~ **Corrected
    /// 2026-11-13 (#174):** the render object's own transform ([`RenderObj::transform`]): the FX bone is
    /// `Get_Bone_Transform(info.m_fxBone)` (W3DModelDraw.cpp:3727), the chain from it with the captured bones
    /// ~~([`RenderObj::get_bone_transform_live`])~~ **Corrected 2026-11-13 (#179):** ([`RenderObj::get_bone_transform`] at
    /// `sync_time`, `WW3D::Get_Sync_Time()`: the animation's current pose, which the query moves on).
    pub fn handle_weapon_fire_fx_at(&mut self, wslot: usize, specific_barrel_to_use: i32, has_fxl: bool, sync_time: u32) -> Option<WeaponFireFxAt> {
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
            if let Some(r) = &mut self.render_object {
                // `m_renderObject->Get_Bone_Transform(info.m_fxBone)` unless hidden; ~~the bone's base pose (the
                // skeleton's)~~ (#179) the animation's current pose, (#172) in the world: the chain from the render
                // object's transform
                let pristine = r.skeleton.as_ref().and_then(|k| k.bones.get(info.fx_bone as usize)).map(|b| b.transform);
                // (#179) upstream asks the bone only when the render object is not hidden (or the drawable has no object:
                // the port's firing drawables always have one), so a hidden one's animation state is not moved on: its
                // answer here is read without writing (the caller plays at the logic object then)
                let bone_transform = pristine.map(|b| {
                    if plant(9697) {
                        b
                    } else if r.hidden {
                        r.peek_bone_transform(None, info.fx_bone, sync_time)
                    } else {
                        r.get_bone_transform(info.fx_bone, sync_time)
                    }
                });
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
    /// made (`m_particleSystemIDs`) found and moved to its bone; always true. ~~The module makes no bone particle systems
    /// yet (the module docs: `ParticleSysBone`), so the list is empty and nothing moves.~~ **Corrected 2026-11-12
    /// (#171):** the module makes them ([`W3DModelDraw::recalc_bones_for_client_particle_systems`]) and this moves them.
    ///
    /// (#171) With a current state and a render object, each system that is still there and was made at a bone (index
    /// not 0) is given the bone's transform in the world: its position (`setPosition`), a turn by the bone's Z rotation
    /// (`rotateLocalTransformZ`, which the next call overwrites, kept as upstream does it), the whole transform as its
    /// local transform (`setLocalTransform`) and `setSkipParentXfrm(true)`, so the drawable's transform is not applied
    /// again. The bone is ~~`transform` (the render object's, [`DrawFacts::transform`]) times the skeleton's bone under
    /// the drawable's scale (the module docs)~~ **Corrected 2026-11-12 (#172):** the chain from `transform` (the render
    /// object's, ~~[`DrawFacts::transform`]~~ **Corrected 2026-11-13 (#174):** its [`RenderObj::transform`] as
    /// `doDrawModule` or `reactToTransformChange` set it) over the scaled pivots, with the captured bones,
    /// [`RenderObj::get_bone_transform`] (root (B) of the module docs). DANGER WARNING READ ME (upstream's): this must never touch the logic, and
    /// does not: it writes the client's particle systems only.
    ///
    /// **Corrected 2026-11-13 (#179):** no `transform` argument: upstream reads the render object's own transform (its
    /// `Set_Transform` is commented out, 2681-2688, 2719), and each bone is `Get_Bone_Transform(boneIndex)` at
    /// `sync_time` ([`RenderObj::get_bone_transform`]): the animation's current pose, the first query moving the
    /// animation on and validating the hierarchy, the next ones answering that pose ("just a little worried about state
    /// changes", upstream's comment).
    pub fn update_bones_for_client_particle_systems(&mut self, particles: Option<&mut ParticleSystemManager>, sync_time: u32) -> bool {
        // `drawable != NULL` always holds for a module of a drawable
        if self.cur_state.is_some() {
            if let (Some(r), Some(manager)) = (&mut self.render_object, particles) {
                let transform = r.transform;
                let first_bone = self.particle_system_ids.first().map_or(0, |t| t.bone_index);
                for it in &self.particle_system_ids {
                    let bone_index = if plant(9677) { first_bone } else { it.bone_index };
                    let Some(sys) = manager.find_particle_system_mut(it.id) else { continue };
                    if bone_index != 0 || plant(9673) {
                        // `m_renderObject->Get_Bone_Transform(boneIndex)` in the world: (#172) the chain from the render
                        // object's transform
                        let bone_transform = if plant(9676) {
                            r.get_bone_transform_scaled(bone_index)
                        } else if plant(9691) {
                            Matrix3D::multiply(&transform, &r.get_bone_transform_scaled(bone_index))
                        } else {
                            r.get_bone_transform(bone_index, sync_time)
                        };
                        let vpos = bone_transform.get_translation();
                        let pos = Coord3D { x: vpos.x, y: vpos.y, z: vpos.z };
                        sys.set_position(&pos);
                        let orientation = bone_transform.get_z_rotation();
                        sys.rotate_local_transform_z(orientation);
                        if !plant(9675) {
                            sys.set_local_transform(&bone_transform);
                        }
                        // `setSkipParentXfrm(true)` (ParticleSys.h:547)
                        sys.skip_parent_xfrm = !plant(9674);
                    }
                }
            }
        }
        true
    }

    /// (#171) `recalcBonesForClientParticleSystems()` (W3DModelDraw.cpp:2589-2656): once the state changed
    /// (`m_needRecalcBoneParticleSystems`), unless the drawable's status forbids it, a system for each of the state's
    /// `ParticleSysBone`s whose template exists: at its bone in model space under the drawable's scale (the origin, no
    /// turn, when the render object has no such bone, or there is no render object), turned by the bone's Z rotation,
    /// attached to the drawable, never saved (`setSaveable(FALSE)`: re-made after a load), stopped when the drawable is
    /// hidden or fully obscured by the shroud, and listed with its bone. The flag is then cleared. DANGER WARNING READ
    /// ME (upstream's): client only.
    ///
    /// (#179) The bone is read as upstream reads it, at `sync_time`: `Set_Transform(Scale(getScale()))`,
    /// `Get_Bone_Transform`, `Set_Transform(original)` ([`RenderObj::get_bone_transform_under`]): the animation's
    /// current pose, the animation moved on, and the hierarchy left not valid.
    pub fn recalc_bones_for_client_particle_systems(&mut self, particles: Option<&mut DrawParticles<'_>>, random: &mut RandomValues, sync_time: u32) {
        if self.need_recalc_bone_particle_systems {
            // `drawable != NULL` always holds for a module of a drawable
            if let Some(cur) = self.cur_state.clone() {
                if !self.drawable_no_state_particles || plant(9662) {
                    let data = self.data.clone();
                    let mut particles = particles;
                    for it in &data.state(&cur).particle_sys_bones {
                        // `TheParticleSystemManager->createParticleSystem(it->particleSystemTemplate)`: none without the
                        // template (or without a manager)
                        let Some(p) = particles.as_deref_mut() else { continue };
                        let Some(id) = p.manager.create_particle_system(&it.particle_system_template, true, random, p.frame) else { continue };
                        let mut pos = Coord3D::default();
                        let mut rotation = 0.0f32;
                        let bone_index = match &self.render_object {
                            Some(r) if !plant(9663) => r.get_bone_index(&it.bone_name),
                            _ => 0,
                        };
                        if bone_index != 0 {
                            // ugh... kill the mtx so we get it in modelspace, not world space (upstream sets the render
                            // object's transform to the drawable's scale, reads the bone and restores it: ~~the skeleton
                            // as made~~ (#174) the chain from `Matrix3D(true).Scale(getScale())` with the render object's
                            // captured bones, e.g. the turret `handleClientTurretPositioning` just turned)
                            let scale = self.scale;
                            let r = self.render_object.as_mut().expect("a render object");
                            let bone_transform = if plant(9759) {
                                r.get_bone_transform_scaled(bone_index)
                            } else {
                                let mut tmp = Matrix3D::IDENTITY;
                                tmp.scale(scale);
                                r.get_bone_transform_under(&tmp, bone_index, sync_time)
                            };
                            let vpos = bone_transform.get_translation();
                            rotation = bone_transform.get_z_rotation();
                            pos.x = vpos.x;
                            pos.y = vpos.y;
                            pos.z = if plant(9690) { 0.0 } else { vpos.z };
                            if plant(9665) {
                                std::mem::swap(&mut pos.x, &mut pos.y);
                            }
                        }
                        let hidden = if plant(9670) {
                            self.fully_obscured_by_shroud
                        } else if plant(9669) {
                            self.drawable_hidden
                        } else {
                            self.drawable_hidden || self.fully_obscured_by_shroud
                        };
                        let sys = p.manager.find_particle_system_mut(id).expect("just made");
                        // got the bone position...
                        sys.set_position(&pos);
                        // and the direction, so that the system is oriented like the bone is
                        if !plant(9664) {
                            sys.rotate_local_transform_z(rotation);
                        }
                        // Attach it to the object...
                        if !plant(9666) {
                            sys.attach_to_drawable(self.drawable_id);
                        }
                        // important: mark it as do-not-save, since we'll just re-create it when we reload.
                        if !plant(9667) {
                            p.manager.set_saveable(id, false);
                        }
                        if hidden != plant(9672) {
                            // don't start the systems for drawables that are hidden.
                            p.manager.find_particle_system_mut(id).expect("just made").stop();
                        }
                        // store the particle system id so we can kill it later.
                        self.particle_system_ids.push(ParticleSysTrackerType { id, bone_index: if plant(9671) { 0 } else { bone_index } });
                    }
                }
            }
            if !plant(9661) {
                self.need_recalc_bone_particle_systems = false;
            }
        }
    }

    /// (#171) `stopClientParticleSystems()` (W3DModelDraw.cpp:2385-2397): each listed system that is still there
    /// destroyed (`ParticleSystem::destroy`, its slave too), then the list cleared. Without a manager (`None`) none is
    /// found: the list is cleared.
    pub fn stop_client_particle_systems(&mut self, particles: Option<&mut ParticleSystemManager>) {
        if let Some(manager) = particles {
            for it in &self.particle_system_ids {
                if manager.find_particle_system(it.id).is_some() {
                    // this can be NULL
                    if plant(9680) {
                        manager.find_particle_system_mut(it.id).expect("found").stop();
                    } else {
                        manager.destroy(it.id);
                    }
                }
            }
        }
        if !plant(9679) {
            self.particle_system_ids.clear();
        }
    }

    /// (#171) `doStartOrStopParticleSys()` (W3DModelDraw.cpp:1803-1818): each listed system that is still there stopped
    /// while the drawable is effectively hidden or fully obscured by the shroud, else started.
    pub fn do_start_or_stop_particle_sys(&mut self, particles: Option<&mut ParticleSystemManager>) {
        let hidden = (self.drawable_hidden && !plant(9678)) || (self.fully_obscured_by_shroud && !plant(9683));
        let Some(manager) = particles else { return };
        for it in &self.particle_system_ids {
            if let Some(sys) = manager.find_particle_system_mut(it.id) {
                // this can be NULL
                if hidden {
                    sys.stop();
                } else if !plant(9684) {
                    sys.start();
                }
            }
        }
    }

    /// (#171) `setFullyObscuredByShroud(fullyObscured)` (W3DModelDraw.cpp:1939-1952): when it changed, kept, and the
    /// particle systems stopped or started (the shadow's and the terrain decal's `enableShadowInvisible` have nothing to
    /// switch: neither is made yet).
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool, particles: Option<&mut ParticleSystemManager>) {
        if self.fully_obscured_by_shroud != fully_obscured {
            if !plant(9685) {
                self.fully_obscured_by_shroud = fully_obscured;
            }
            self.do_start_or_stop_particle_sys(particles);
        }
    }

    /// (#165) `getCurrentWorldspaceClientBonePositions(boneName, transform)` (W3DModelDraw.cpp:3555-3566): with a render
    /// object, its `Get_Bone_Index(boneName)` (0: none, false) and `Get_Bone_Transform(index)`. Here the bone's transform
    /// is the skeleton's pose as made (as [`W3DModelDraw::handle_weapon_fire_fx_at`] reads it), put in the world by the
    /// drawable's transform `world` ~~(`world` times the bone as made)~~ (**Corrected 2026-11-12 (#172):** the chain
    /// from `world` over the scaled pivots, [`RenderObj::get_bone_transform`]): the animation's current pose is not
    /// applied (#165's gaps). **Corrected 2026-11-13 (#174):** `world` is the render object's transform the caller has it
    /// hold (its [`RenderObj::transform`], or [`W3DModelDraw::reacted_transform`] of the drawable's), and the chain
    /// applies the render object's captured bones (the turret's turn, the recoil). **Corrected 2026-11-13 (#179):** no
    /// `world` argument: the render object's own transform (the caller puts it at the drawable's,
    /// [`W3DModelDraw::react_to_transform_change`]), and the bone is `Get_Bone_Transform` at `sync_time`
    /// ([`RenderObj::get_bone_transform`]): the animation's current pose, the query moving the animation on.
    pub fn get_current_worldspace_client_bone_positions(&mut self, bone_name: &[u8], sync_time: u32) -> Option<Matrix3D> {
        Self::worldspace_bone(self.render_object.as_mut()?, bone_name, sync_time)
    }

    /// (#179) [`W3DModelDraw::get_current_worldspace_client_bone_positions`] with the render object's transform put at
    /// `world` (`reactToTransformChange`'s), nothing written: for another drawable's question (a laser's start bone,
    /// [`crate::client::laser_update`]), which may only read it (#179's gaps).
    pub fn peek_current_worldspace_client_bone_positions(&self, bone_name: &[u8], world: &Matrix3D, sync_time: u32) -> Option<Matrix3D> {
        let mut r = self.render_object.clone()?;
        r.set_transform(world);
        Self::worldspace_bone(&mut r, bone_name, sync_time)
    }

    /// (#179) `getCurrentWorldspaceClientBonePositions`' body (W3DModelDraw.cpp:3557-3565) on the render object.
    fn worldspace_bone(r: &mut RenderObj, bone_name: &[u8], sync_time: u32) -> Option<Matrix3D> {
        let k = r.skeleton.clone()?;
        let bone_index = k.get_bone_index(bone_name);
        if bone_index == 0 {
            return None;
        }
        if plant(9696) {
            return Some(Matrix3D::multiply(&r.transform, &k.bones[bone_index as usize].transform));
        }
        Some(r.get_bone_transform(bone_index, sync_time))
    }

    /// (#174) `getCurrentBonePositions(boneNamePrefix, startIndex, positions, transforms, maxBones)`
    /// (W3DModelDraw.cpp:3569-3646, the compiled `DO_INV` branch): the render object's current bones in model space.
    /// `maxBones` is clamped to `MAX_BONE_GET` (64); a `NULL` `transforms` is the function's own `tmpMtx`; without a
    /// render object, 0. The inverse is the render object's transform's `Get_Orthogonal_Inverse` scaled by
    /// `getDrawable()->getScale()` ([`W3DModelDraw::scale`]; upstream's own note: "kill the mtx so we get it in
    /// modelspace, not world space"). The names are `boneNamePrefix` (for `startIndex` 0 only it) or `prefix01`,
    /// `prefix02`, ... from `startIndex` to 99, not lower-cased (`Get_Bone_Index` ignores case); a bone not found (index 0)
    /// ends the list. Each bone is `Get_Bone_Transform(boneIndex)` (the chain from the render object's own transform with
    /// its captures, [`RenderObj::get_bone_transform_live`]) pre-multiplied by the inverse (`preMul`). With `positions`,
    /// each found bone's translation. Returns the count. A slot beyond the caller's array is upstream's write past it:
    /// not made (the count still counts it). **Corrected 2026-11-13 (#179):** each bone at `sync_time`
    /// ([`RenderObj::get_bone_transform`]): the animation's current pose, the first query moving the animation on (also
    /// for a slot past the caller's array: upstream asks before it writes).
    pub fn get_current_bone_positions(&mut self, bone_name_prefix: &[u8], start_index: i32, positions: Option<&mut [Coord3D]>, transforms: Option<&mut [Matrix3D]>, max_bones: i32, sync_time: u32) -> i32 {
        let scale = self.scale;
        match self.render_object.as_mut() {
            Some(r) => Self::current_bone_positions(r, scale, bone_name_prefix, start_index, positions, transforms, max_bones, sync_time),
            None => 0,
        }
    }

    /// (#179) [`W3DModelDraw::get_current_bone_positions`] with nothing written to the render object: for a caller that
    /// may only read the drawable (the FX lists' `doFxAtBones`, [`crate::client::client_fx`]; #179's gaps).
    pub fn peek_current_bone_positions(&self, bone_name_prefix: &[u8], start_index: i32, positions: Option<&mut [Coord3D]>, transforms: Option<&mut [Matrix3D]>, max_bones: i32, sync_time: u32) -> i32 {
        match self.render_object.clone() {
            Some(mut r) => Self::current_bone_positions(&mut r, self.scale, bone_name_prefix, start_index, positions, transforms, max_bones, sync_time),
            None => 0,
        }
    }

    /// (#179) `getCurrentBonePositions`' body after its render-object test, on `r` with the drawable's scale `scale`.
    #[allow(clippy::too_many_arguments)]
    fn current_bone_positions(r: &mut RenderObj, scale: f32, bone_name_prefix: &[u8], start_index: i32, positions: Option<&mut [Coord3D]>, transforms: Option<&mut [Matrix3D]>, max_bones: i32, sync_time: u32) -> i32 {
        const MAX_BONE_GET: i32 = 64;
        let mut tmp_mtx = [Matrix3D::IDENTITY; MAX_BONE_GET as usize];

        let max_bones = if max_bones > MAX_BONE_GET { MAX_BONE_GET } else { max_bones };

        let transforms: &mut [Matrix3D] = match transforms {
            Some(t) => t,
            None => &mut tmp_mtx,
        };

        // ugh... kill the mtx so we get it in modelspace, not world space
        let original_transform = r.transform; // save the transform
        let mut inverse = original_transform.get_orthogonal_inverse();
        if !plant(9762) {
            inverse.scale(scale);
        }

        let mut pos_count: i32 = 0;
        let end_index = if start_index == 0 && !plant(9763) { 0 } else { 99 };
        let mut i = start_index;
        while i <= end_index {
            let mut buffer = bone_name_prefix.to_vec();
            if i != 0 {
                buffer.extend_from_slice(format!("{i:02}").as_bytes());
            }

            let bone_index = r.get_bone_index(&buffer);
            if bone_index == 0 {
                break;
            }

            let bone = if plant(9764) { r.get_bone_transform_under(&Matrix3D::IDENTITY, bone_index, sync_time) } else { r.get_bone_transform(bone_index, sync_time) };
            if let Some(t) = usize::try_from(pos_count).ok().and_then(|p| transforms.get_mut(p)) {
                *t = bone;
                if plant(9765) {
                    t.post_mul(&inverse);
                } else {
                    t.pre_mul(&inverse);
                }
            }

            pos_count += 1;
            if pos_count >= max_bones {
                break;
            }
            i += 1;
        }

        if let Some(positions) = positions {
            for i in 0..pos_count.max(0) as usize {
                if let (Some(p), Some(t)) = (positions.get_mut(i), transforms.get(i)) {
                    let pos = t.get_translation();
                    p.x = pos.x;
                    p.y = pos.y;
                    p.z = if plant(9766) { 0.0 } else { pos.z };
                }
            }
        }

        pos_count
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
    /// animation to sync time `now`, unless hidden (#179: and evaluate its pose when the hierarchy is not valid,
    /// [`RenderObj::render`]).
    pub fn render(&mut self, now: u32) {
        if let Some(r) = &mut self.render_object {
            if !r.hidden {
                r.render(now);
            }
        }
    }

    /// `setHidden(h)`: the render object's visibility (W3DModelDraw.cpp:1823-1841; the shadow, terrain decal and
    /// track parts have nothing to switch: none is made yet). (#171) Then `doStartOrStopParticleSys()`, which reads the
    /// drawable's `isDrawableEffectivelyHidden()`: `h` as its `updateHiddenStatus` gives it.
    pub fn set_hidden(&mut self, h: bool, particles: Option<&mut ParticleSystemManager>) {
        self.drawable_hidden = h;
        if let Some(r) = &mut self.render_object {
            r.hidden = h;
        }
        if !plant(9689) {
            self.do_start_or_stop_particle_sys(particles);
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
            self.anims.iter().find(|a| a.0.as_bytes().eq_ignore_ascii_case(name)).map(|a| AnimHandle { name: a.0.as_bytes().to_vec(), num_frames: a.1, frame_rate: a.2, data: None })
        }
    }

    fn fake() -> Fake {
        Fake {
            anims: vec![("t.idle", 10, 10.0), ("t.move", 20, 10.0), ("t.lower", 5, 10.0), ("t.a", 4, 10.0), ("t.b", 4, 10.0), ("t.c", 11, 10.0), ("t.d", 30, 30.0)],
        }
    }

    fn draw_of(lines: &[(u32, &str, &str)], assets: &mut Fake, random: &mut RandomValues) -> W3DModelDraw {
        let d = Arc::new(parse(lines).unwrap());
        let mut env = DrawEnv { assets, random, sync_time: 0, logic_time: true, particles: None };
        W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
    }

    const FACTS: DrawFacts = DrawFacts { should_animate_with_power: true, should_animate_without_power: true, speed: None, turrets: [None, None], motion: None, show_client_physics: true, wheel_info: None, transform: Matrix3D::IDENTITY, construction: None };

    /// One client frame at `t` ms: draw, then render.
    fn frame(m: &mut W3DModelDraw, assets: &mut Fake, random: &mut RandomValues, t: u32) {
        let mut env = DrawEnv { assets, random, sync_time: t, logic_time: false, particles: None };
        m.do_draw_module(&FACTS, &mut env);
        m.render(t);
    }

    fn set_flags(m: &mut W3DModelDraw, assets: &mut Fake, random: &mut RandomValues, t: u32, bits: &[i32]) {
        let mut env = DrawEnv { assets, random, sync_time: t, logic_time: false, particles: None };
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
        let mut env = DrawEnv { assets: &mut a, random: &mut r, sync_time: 0, logic_time: false, particles: None };
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
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time, particles: None };
                W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
            }
            fn frame(&mut self, m: &mut W3DModelDraw, turrets: [Option<(f32, f32)>; 2]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
                m.do_draw_module(&DrawFacts { turrets, ..FACTS }, &mut env);
            }
            fn set_flags(&mut self, m: &mut W3DModelDraw, bits: &[i32]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
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

            assert!(!m.handle_weapon_fire_fx(0, 0, false, 0));
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
            assert!(m.handle_weapon_fire_fx(0, 0, true, 0));
        }

        /// (#172) The drawable's scale as upstream's render object carries it, on [`gunt`] (MUZZLEFX01 is four pivots
        /// below the root: TURRET (0, 0, 2), BARREL (1, 0, 0), RECOIL01 (2, 0, 0), MUZZLEFX01 (1, 0, 0)), through the
        /// game's asset source ([`W3dFiles::skeleton`]).
        ///
        /// Predictions, written before the first run:
        /// - scale 2 is "really" a scale: the pivots' translations are doubled (`HTreeClass::Scale`), so MUZZLEFX01's
        ///   chain from the identity is at (8, 0, 4) with the rotations unscaled;
        /// - root (A), the pristine bone (`Set_Transform(Scale(2))`): at (16, 0, 8) with the rotation part 2 I (the old
        ///   skeleton gave (8, 0, 4));
        /// - root (B), the live bone under a world translated by (100, 50, 0):
        ///   `getCurrentWorldspaceClientBonePositions` at (108, 50, 4) with the rotation part I (the old code gave 2 I),
        ///   and `handleWeaponFireFX`'s FX bone exactly the same matrix;
        /// - scale 1.005 is within `scale_epsilon` of 1: the hierarchy is not scaled, the pristine bone is
        ///   `Scale(1.005)` of (4, 0, 2) = (4.02, 0, 2.01), and the live bone under the identity is at (4, 0, 2).
        #[test]
        fn a_scaled_gun_s_bones_are_upstream_s() {
            let mut rig = Rig::new();
            let draw_scaled = |rig: &mut Rig, scale: f32| {
                let d = Arc::new(parse(GUN_DRAW).unwrap());
                let mut a = W3dFiles { assets: &mut rig.am, fs: &rig.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
                W3DModelDraw::new_scaled(d, 0, false, false, scale, Arc::new(Vec::new()), &mut env).unwrap()
            };
            let mut m = draw_scaled(&mut rig, 2.0);
            let pristine = m.get_cur_state().unwrap().bone_cache().pristine_bones[&b"muzzlefx01"[..]].mtx;
            assert!(near(pristine.get_translation(), [16.0, 0.0, 8.0]), "{:?}", pristine.get_translation());
            assert_eq!((pristine.row[0][0], pristine.row[1][1], pristine.row[2][2]), (2.0, 2.0, 2.0));
            let mut world = Matrix3D::IDENTITY;
            world.set_translation(&Vector3::new(100.0, 50.0, 0.0));
            let live = m.peek_current_worldspace_client_bone_positions(b"MUZZLEFX01", &world, 0).unwrap();
            assert!(near(live.get_translation(), [108.0, 50.0, 4.0]), "{:?}", live.get_translation());
            assert_eq!((live.row[0][0], live.row[1][1], live.row[2][2]), (1.0, 1.0, 1.0));
            // (#174) the FX bone is chained from the render object's own transform: `reactToTransformChange` puts it at the
            // drawable's (no instance scale)
            m.react_to_transform_change(&world, None);
            let at = m.handle_weapon_fire_fx_at(0, 0, true, 0).unwrap();
            assert_eq!(at, WeaponFireFxAt { bone_transform: Some(live), render_hidden: false });
            // (#174) the draw's render object transform is the drawable's scaled by the instance scale (W3DModelDraw.cpp:
            // 2045-2054, `m_instanceScale` = the template's 2): the live muzzle bone in the base pose is then the pristine
            // bone in the world, (116, 50, 8) with the rotation part 2 I. Predicted before the run
            // (issue174_predictions.md). ~~The live bone is that.~~ **Corrected after the first run (2026-11-13):** a
            // prediction miss: the draw also runs `handleClientTurretPositioning` (TURRET captured at its
            // `TurretArtAngle` 90) and `handleClientRecoil` (the recoil the fire FX above started: RECOIL01 shifted 2
            // back along its X, 4 in the world at scale 2), and the live bone follows both: (100, 50 + 16 - 4, 8).
            {
                let mut a = W3dFiles { assets: &mut rig.am, fs: &rig.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut rig.random, sync_time: 0, logic_time: false, particles: None };
                m.do_draw_module(&DrawFacts { transform: world, ..FACTS }, &mut env);
            }
            let r = m.render_object.as_ref().unwrap();
            let mut scaled = world;
            scaled.scale(2.0);
            assert_eq!((r.transform, r.object_scale), (scaled, 2.0));
            let mut bare = r.clone();
            let live2 = m.get_current_worldspace_client_bone_positions(b"MUZZLEFX01", 0).unwrap();
            assert!(near(live2.get_translation(), [100.0, 62.0, 8.0]), "{:?}", live2.get_translation());
            bare.bone_controls.clear();
            let root = bare.transform;
            let base = bare.peek_bone_transform(Some(&root), bare.get_bone_index(b"MUZZLEFX01"), 0);
            assert_eq!(base, Matrix3D::multiply(&world, &pristine));
            assert!(near(base.get_translation(), [116.0, 50.0, 8.0]), "{:?}", base.get_translation());
            // (#174) `getCurrentBonePositions` (DO_INV): `Get_Orthogonal_Inverse` of the render object's transform
            // M = [2 I | (100, 50, 0)], scaled by the drawable's scale: [4 I | (-200, -100, 0)], times the live bone
            // ~~[2 I | (116, 50, 8)]: [8 I | (264, 100, 32)]~~ **Corrected after the first run:** [2 Rz(90) | (100, 62, 8)]
            // (the turret and the recoil, above): translation 4 (100, 62, 8) - (200, 100, 0) = (200, 148, 32). Not the
            // model-space bone the `#else` branch (`Set_Transform(Scale(s))`) gives: the orthogonal inverse of a scaled
            // transform is not its inverse. Upstream's, kept (at scale 1 the two agree).
            let mut t = [Matrix3D::IDENTITY; 2];
            let mut p = [Coord3D::default(); 2];
            assert_eq!(m.get_current_bone_positions(b"MUZZLEFX", 1, Some(&mut p), Some(&mut t), 2, 0), 1);
            assert!(near(t[0].get_translation(), [200.0, 148.0, 32.0]), "{:?}", t[0]);
            assert!((t[0].row[1][0] - 8.0).abs() < 1e-5 && (t[0].row[2][2] - 8.0).abs() < 1e-5, "{:?}", t[0]);
            assert_eq!(p[0], Coord3D { x: t[0].row[0][3], y: t[0].row[1][3], z: t[0].row[2][3] });

            // (#174) a captured turret: TURRET turned a quarter about Z (`Capture_Bone`, `Control_Bone`, as
            // `handleClientTurretPositioning` does) turns every bone under it. Predicted (issue174_predictions.md): under
            // the render object's transform [2 I | (100, 50, 0)], MUZZLEFX01 moves from (116, 50, 8) to (100, 66, 8): the
            // turret at 2 * (0, 0, 4) and the barrel's 2 * (8, 0, 0) turned onto +Y. **Corrected after the first run:**
            // the draw had already turned it a quarter (its art angle) and the recoil holds RECOIL01 4 back, so the test
            // turns it a half instead: 2 * (8 - 2, 0, 0) turned onto -X, (100 - 12, 50, 8).
            let turret = m.render_object.as_ref().unwrap().get_bone_index(b"TURRET");
            assert_ne!(turret, 0);
            let mut quarter = Matrix3D::IDENTITY;
            quarter.rotate_z(std::f32::consts::PI);
            let r = m.render_object.as_mut().unwrap();
            r.capture_bone(turret);
            r.control_bone(turret, &quarter);
            let turned = m.get_current_worldspace_client_bone_positions(b"MUZZLEFX01", 0).unwrap();
            assert!(near(turned.get_translation(), [88.0, 50.0, 8.0]), "{:?}", turned.get_translation());

            let m = draw_scaled(&mut rig, 1.005);
            let pristine = m.get_cur_state().unwrap().bone_cache().pristine_bones[&b"muzzlefx01"[..]].mtx;
            assert!(near(pristine.get_translation(), [4.02, 0.0, 2.01]), "{:?}", pristine.get_translation());
            let live = m.peek_current_worldspace_client_bone_positions(b"MUZZLEFX01", &Matrix3D::IDENTITY, 0).unwrap();
            assert_eq!(live.get_translation(), Vector3::new(4.0, 0.0, 2.0));
        }

        /// (#175) The logic's pristine bones in a game made by the ordinary path, against upstream's
        /// (`W3DModelDraw::getPristineBonePositionsForConditionState` and `getProjectileLaunchOffset`, which this client
        /// reproduces since #172) for the same art, at `Scale` 2 and 1.
        ///
        /// ~~Predictions, written before the first run (`issue175_predictions.md`): the logic's art-bone tables are
        /// empty, so the logic finds no `MUZZLEFX` bone, no launch offset, 0 barrels, and
        /// `getSingleLogicalBonePosition` falls back to the object's own position~~ (the investigation's measurement,
        /// all held; it is now the ablation, `live_bones` off). **Corrected 2026-11-12 (#175):** the World loads its
        /// art's bones (`World::load_art_bones`, GUNT through `W3dFiles`) before its first frame and, with `live_bones`
        /// (on in a game), answers from them. Predicted (`issue175b_predictions.md`): the logic's `MUZZLEFX01` is
        /// upstream's, (16, 0, 8) with rotation part 2 I at scale 2 and (4, 0, 2) with I at scale 1; its world position
        /// (116, 300, 8) and (104, 300, 2); the launch offset is the pristine matrix; one primary barrel. With
        /// `live_bones` off the old answers come back (none found, the object's own position).
        #[test]
        fn issue175_the_logic_s_bones_against_upstream_s() {
            use crate::logic::weapon_host::WeaponHost;
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::team_ops::factory_find_team;
            for (scale, art) in [(2.0f32, [16.0f32, 0.0, 8.0]), (1.0, [4.0, 0.0, 2.0])] {
                // the logic: a template with the art's draw module and its scale, made as the game makes objects
                let ini = format!(
                    "Object ScaledGun\n Scale = {scale:.1}\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                     Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = GUNT\n   WeaponFireFXBone = PRIMARY MuzzleFX\n\
                     \x20  WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n End\n\
                     Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n"
                );
                let mut w = gate_world_with(&ini);
                let mut rig = Rig::new();
                let report = w.load_art_bones(&mut W3dFiles { assets: &mut rig.am, fs: &rig.fs }).expect("the art at hand");
                assert_eq!(report.templates, 1);
                assert!(w.cuts.live_bones);
                let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
                let id = make(&mut w, b"ScaledGun", team, Coord3D { x: 100.0, y: 300.0, z: 0.0 });
                assert_eq!(w.find_object(id).unwrap().template().get_asset_scale(), scale);
                for _ in 0..30 {
                    w.logic_update().unwrap();
                }

                // upstream's, through the client's translation of W3DModelDraw (#172) on the same art
                let d = Arc::new(
                    parse(&[(0, "DefaultConditionState", ""), (1, "Model", "GUNT"), (1, "WeaponFireFXBone", "PRIMARY MuzzleFX"), (1, "WeaponLaunchBone", "PRIMARY MuzzleFX")])
                        .unwrap(),
                );
                let mut a = W3dFiles { assets: &mut rig.am, fs: &rig.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
                let m = W3DModelDraw::new_scaled(d, 0, false, false, scale, Arc::new(Vec::new()), &mut env).unwrap();
                let cache = m.get_cur_state().unwrap().bone_cache();
                let pristine = cache.pristine_bones[&b"muzzlefx01"[..]].mtx;
                assert!(near(pristine.get_translation(), art), "{:?}", pristine.get_translation());
                assert_eq!((pristine.row[0][0], pristine.row[1][1], pristine.row[2][2]), (scale, scale, scale));
                assert_eq!(cache.weapon_barrel_info_vec[0][0].projectile_offset_mtx, pristine);

                // the logic's, from the art (`live_bones`)
                let p = Coord3D { x: art[0], y: art[1], z: art[2] };
                let (upstream_world, upstream_mtx) = w.object_convert_bone_pos_to_world_pos(id, &p, &pristine);
                assert_eq!(w.drawable_pristine_bone_transforms(id, b"MUZZLEFX", 1, 4), vec![pristine]);
                let logic_p = w.drawable_pristine_bones(id, b"MuzzleFX", 1, 4);
                assert_eq!(logic_p.len(), 1);
                assert_eq!((logic_p[0].x, logic_p[0].y, logic_p[0].z), (p.x, p.y, p.z));
                let lo = WeaponHost::projectile_launch_offset(&mut w, id, 0, 0, -1).expect("the launch bone");
                assert_eq!(lo.attach_transform, pristine);
                assert_eq!(WeaponHost::barrel_count(&mut w, id, 0), Some(1));
                let (found, logic_pos, logic_mtx) = w.object_get_single_logical_bone_transform(id, b"MUZZLEFX01");
                assert!(found);
                assert_eq!(((logic_pos.x, logic_pos.y, logic_pos.z), logic_mtx), ((upstream_world.x, upstream_world.y, upstream_world.z), upstream_mtx));
                assert_eq!((upstream_world.x, upstream_world.y, upstream_world.z), (100.0 + art[0], 300.0 + art[1], art[2]));
                let multi = w.object_get_multi_logical_bone_transforms(id, b"MUZZLEFX", 4);
                assert_eq!(multi.len(), 1);
                assert_eq!(multi[0].1, pristine, "FALSE: not converted");
                eprintln!(
                    "#175 scale {scale}: logic getSingleLogicalBonePosition -> ({}, {}, {}), upstream ({}, {}, {}), diag {}",
                    logic_pos.x, logic_pos.y, logic_pos.z, upstream_world.x, upstream_world.y, upstream_world.z, logic_mtx.row[0][0]
                );

                // the ablation: the tables (empty in a game)
                w.cuts.live_bones = false;
                assert!(w.drawable_pristine_bones(id, b"MUZZLEFX", 1, 4).is_empty());
                assert_eq!(WeaponHost::projectile_launch_offset(&mut w, id, 0, 0, -1), None);
                assert_eq!(WeaponHost::barrel_count(&mut w, id, 0), Some(0));
                let (found, logic_pos, _) = w.object_get_single_logical_bone_transform(id, b"MUZZLEFX01");
                assert!(!found);
                assert_eq!((logic_pos.x, logic_pos.y, logic_pos.z), (100.0, 300.0, 0.0));
            }
        }

        /// (#175) End to end: a game World (the gate's, every switch on) with the GUNT art loaded through `W3dFiles`
        /// before its first frame, and `ArtGun` (`Scale` 2, a projectile weapon launched from `MuzzleFX`) at
        /// (100, 300, 0) facing an enemy depot straight ahead. Its AI acquires the depot and fires; the shell's
        /// `DumbProjectileBehavior` starts its flight at the launch position `Weapon::calcProjectileLaunchPosition`
        /// gives (`m_flightPathStart`, `projectileFireAtObjectOrPosition` reads the projectile's position after
        /// `positionProjectileForLaunch`).
        ///
        /// Predicted (`issue175b_predictions.md`): the first shell starts at upstream's muzzle bone pushed through the
        /// gun's transform, `s² t` = (16, 0, 8) from (100, 300, 0): (116, 300, 8); with `live_bones` off (the
        /// ablation) at the gun's own position.
        #[test]
        fn issue175_a_game_s_weapon_fires_from_the_muzzle_bone() {
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::team_ops::factory_find_team;
            const INI: &str = "Weapon ArtGunShot\n PrimaryDamage = 1\n PrimaryDamageRadius = 0\n AttackRange = 150\n DamageType = ARMOR_PIERCING\n\
                 DeathType = NORMAL\n WeaponSpeed = 20\n ProjectileObject = ArtShell\n DelayBetweenShots = 500\nEnd\n\
                 Object ArtShell\n KindOf = PROJECTILE\n Geometry = SPHERE\n GeometryMajorRadius = 1\n\
                 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 1\n  InitialHealth = 1\n End\n\
                 Behavior = DumbProjectileBehavior ModuleTag_Proj\n  MaxLifespan = 10000\n End\nEnd\n\
                 Object ArtGun\n Scale = 2.0\n KindOf = VEHICLE\n VisionRange = 200\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                 WeaponSet\n  Conditions = None\n  Weapon = PRIMARY ArtGunShot\n End\n\
                 Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = GUNT\n   WeaponFireFXBone = PRIMARY MuzzleFX\n\
                 \x20  WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n End\n\
                 Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                 Behavior = AIUpdateInterface ModuleTag_AI\n  AutoAcquireEnemiesWhenIdle = Yes\n End\nEnd\n";
            let mut starts = Vec::new();
            for live in [true, false] {
                let mut w = gate_world_with(INI);
                let mut rig = Rig::new();
                w.load_art_bones(&mut W3dFiles { assets: &mut rig.am, fs: &rig.fs }).expect("the art at hand");
                w.cuts.live_bones = live;
                let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
                let enemy = factory_find_team(&mut w, b"team2").unwrap().unwrap();
                let gun = make(&mut w, b"ArtGun", team, Coord3D { x: 100.0, y: 300.0, z: 0.0 });
                make(&mut w, b"Depot", enemy, Coord3D { x: 180.0, y: 300.0, z: 0.0 });
                let mut start = None;
                for _ in 0..90 {
                    w.logic_update().unwrap();
                    let shells: Vec<ObjectId> = w.objects.ids_in_order().into_iter().filter(|&o| w.find_object(o).is_some_and(|x| x.template().get_name() == b"ArtShell")).collect();
                    if let Some(&s) = shells.first() {
                        let index = w.find_module(s, b"DumbProjectileBehavior").expect("its projectile module");
                        let h = crate::logic::world::module_call::ModuleHandle { object: s, index };
                        if let Some(crate::logic::world::update_body::UpdateBody::DumbProjectileBehavior(p)) = w.peek_module_body(h) {
                            let v = p.state_words();
                            start = Some(Coord3D { x: f32::from_bits(v[6]), y: f32::from_bits(v[7]), z: f32::from_bits(v[8]) });
                        }
                        break;
                    }
                }
                let start = start.expect("the gun fired a shell");
                let g = *w.find_object(gun).unwrap().thing.transform_matrix();
                let expected = if live {
                    crate::logic::thing::convert_bone_pos_to_world_pos(&g, &Coord3D { x: 16.0, y: 0.0, z: 8.0 }, &Matrix3D::IDENTITY).0
                } else {
                    let t = g.get_translation();
                    Coord3D { x: t.x, y: t.y, z: t.z }
                };
                eprintln!("#175 live_bones {live}: the shell starts at ({}, {}, {}), the gun's transform {:?}", start.x, start.y, start.z, g.to_array());
                assert_eq!((start.x.to_bits(), start.y.to_bits(), start.z.to_bits()), (expected.x.to_bits(), expected.y.to_bits(), expected.z.to_bits()));
                starts.push(start);
            }
            assert_eq!((starts[0].x, starts[0].y, starts[0].z), (116.0, 300.0, 8.0));
        }

        /// (#175) The World's other bone answers, each reached here:
        /// - `OpenContain`'s fire points written into its own array (`contain_get_multi_logical_bone_position_into`).
        ///   Upstream's miss writes the object's transform after the bones found
        ///   (`getPristineBonePositionsForConditionState`), and the rest is left as it was;
        /// - the World as `ObjectBoneHost`: `Object::getCommandSetString` (the template's, then an override, `GateDozerSet`) and
        ///   `canProduceUpgrade` over the control bar's sets; the turret position without `live_bones` (the table's
        ///   reading) and with it.
        ///
        /// Predicted: one fire point, (104, 300, 2) at scale 1 for the gun at (100, 300, 0); the next entry is the
        /// object's transform; the third is untouched. The dozer's set string is its override once set (first run: the
        /// test assumed the gate set it, which only a gate test does; the test's error). The turret query of an
        /// AI without a turret, the bone found, is undefined upstream (the rotation is never written), an error here.
        /// Without `live_bones` it finds no bone.
        #[test]
        fn issue175_the_world_s_other_bone_answers() {
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::object_weapons;
            use crate::logic::world::team_ops::factory_find_team;
            let ini = "Object ArtAiGun\n Scale = 1.0\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = GUNT\n   WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n End\n\
                       Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\n\
                       Behavior = AIUpdateInterface ModuleTag_AI\n End\nEnd\n";
            let mut w = gate_world_with(ini);
            let mut rig = Rig::new();
            w.load_art_bones(&mut W3dFiles { assets: &mut rig.am, fs: &rig.fs }).expect("the art at hand");
            let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
            let id = make(&mut w, b"ArtAiGun", team, Coord3D { x: 100.0, y: 300.0, z: 0.0 });
            let mut sentinel = Matrix3D::IDENTITY;
            sentinel.set_translation(&Vector3::new(7.0, 7.0, 7.0));
            let mut points = [sentinel; 3];
            assert_eq!(w.contain_get_multi_logical_bone_position_into(id, b"MUZZLEFX", &mut points), 1);
            assert_eq!(points[0].get_translation(), Vector3::new(104.0, 300.0, 2.0));
            assert_eq!(points[1], *w.find_object(id).unwrap().thing.transform_matrix(), "the miss's write");
            assert_eq!(points[2], sentinel);
            assert_eq!(w.drawable_condition_flags(id), w.drawable_condition[&id]);

            let dozer = w.objects.ids_in_order().into_iter().find(|&o| w.find_object(o).is_some_and(|x| x.template().get_name() == b"Dozer")).unwrap();
            assert_eq!(object_weapons::get_command_set_string(&w, dozer), w.find_object(dozer).unwrap().template().friend_get_command_set_string().to_vec());
            w.objects.find_mut(dozer).unwrap().command_set_string_override = b"GateDozerSet".to_vec();
            assert_eq!(object_weapons::get_command_set_string(&w, dozer), b"GateDozerSet".to_vec());
            let can = object_weapons::can_produce_upgrade(&w, dozer, 1);
            assert_eq!(can.is_ok(), w.control_bar_find_command_set(b"GateDozerSet").is_some(), "{can:?}");
            assert_eq!(can.ok(), w.control_bar_find_command_set(b"GateDozerSet").map(|_| false));

            let live = object_weapons::get_single_logical_bone_position_on_turret(&mut w, id, 0, b"MuzzleFX01", true, true);
            assert!(live.is_err(), "{live:?}");
            w.cuts.live_bones = false;
            let table = object_weapons::get_single_logical_bone_position_on_turret(&mut w, id, 0, b"MuzzleFX01", true, true).expect("defined");
            assert!(!table.found);
        }

        /// A dock `DOCK` (#175): pivots ROOTTRANSFORM and DOCKWAITING07 (10, 4, 0), a cube on each.
        fn dock() -> Vec<u8> {
            let ident = [0.0, 0.0, 0.0, 1.0];
            let p = |n: &[u8], parent: i32, t: [f32; 3]| PivotSpec { name: n.to_vec(), parent, translation: t, rotation: ident };
            let mut f = htree_w3d(b"DOCK", &[p(b"ROOTTRANSFORM", -1, [0.0; 3]), p(b"DOCKWAITING07", 0, [10.0, 4.0, 0.0])]);
            let mut names = Vec::new();
            for (n, _) in [(&b"BASE"[..], 0u32), (&b"PAD"[..], 1)] {
                let mut m = MeshSpec::cuboid(b"DOCK", n, [-0.5; 3], [0.5; 3]);
                m.stage0.texture = None;
                f.extend(m.to_w3d());
                let mut full = b"DOCK.".to_vec();
                full.extend_from_slice(n);
                names.push(full);
            }
            let hl: Vec<(&[u8], u32)> = names.iter().zip([0u32, 1]).map(|(n, b)| (n.as_slice(), b)).collect();
            f.extend(hlod_w3d(b"DOCK", b"DOCK", &hl));
            f
        }

        /// (#175) `RailedTransportDockUpdate`'s unload end asks `getPristineBonePositions("DOCKWAITING07", 0, ...)` and
        /// read the position uninitialised where the art has no such bone (railed_transport_dock_update.rs, the
        /// `Undefined::UninitialisedLocal` there): in a game it always took that path (the empty table). Predicted: with
        /// the art loaded and `live_bones`, the bone is found at (10, 4, 0) (scale 1), so the expression the module
        /// evaluates gives a position; without art, or with the switch off, it finds none (the undefined path, as
        /// upstream for art without the bone). (`Scale = 1.0`: a template without one has the port's asset scale 0, the
        /// constructor's zero-filled member; a game's `DefaultThingTemplate` sets 1. At scale 0 the bones are all zero, as
        /// upstream's `HTreeClass::Scale(0)` and `Matrix3D(true).Scale(0)` make them: the first run found the bone at
        /// (0, 0, 0) for that reason, a fixture error.) Also, `ExtraPublicBone = DockWaiting07`: `doSingleBoneName("DockWaiting")`
        /// stops at the first missing number (01).
        #[test]
        fn issue175_the_railed_dock_finds_its_waiting_bone() {
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::team_ops::factory_find_team;
            let ini = "Object ArtDock\n Scale = 1.0\n KindOf = STRUCTURE\n Geometry = BOX\n GeometryMajorRadius = 20\n GeometryMinorRadius = 20\n GeometryHeight = 10\n\
                       Draw = W3DModelDraw ModuleTag_Draw\n  ExtraPublicBone = DockWaiting07\n  DefaultConditionState\n   Model = DOCK\n  End\n End\n\
                       Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";
            let mut d = MemoryDevice::new();
            d.insert(b"Art/W3D/dock.w3d", dock());
            let fs = FileSystem::new(d);
            let mut am = AssetManager::new_game(W3dFileFactory::default());
            for (load, live, expected) in [(true, true, Some((10.0, 4.0, 0.0))), (false, true, None), (true, false, None)] {
                let mut w = gate_world_with(ini);
                if load {
                    w.load_art_bones(&mut W3dFiles { assets: &mut am, fs: &fs }).expect("the art at hand");
                }
                w.cuts.live_bones = live;
                let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
                let us = make(&mut w, b"ArtDock", team, Coord3D { x: 200.0, y: 100.0, z: 0.0 });
                assert!(w.has_drawable(us));
                // the module's expression (railed_transport_dock_update.rs, `unloadNext`'s caller)
                let bone = w.drawable_pristine_bones(us, b"DOCKWAITING07", 0, 1).first().copied().ok_or(crate::common::ini::Undefined::UninitialisedLocal);
                assert_eq!(bone.as_ref().ok().map(|b| (b.x, b.y, b.z)), expected, "load {load}, live {live}");
                if let Ok(b) = bone {
                    let p = w.contain_convert_bone_pos_to_world_pos(us, &b);
                    assert_eq!((p.x, p.y, p.z), (210.0, 104.0, 0.0));
                }
            }
        }

        /// (#175) The logic's copy of the drawable's flags decides the state the bones are read from. `FLAGGED` has
        /// GUNT by default and no model when `DAMAGED`. Predicted: no flags, the muzzle bone; the body's damage state
        /// (`DrawableReactToBodyDamageStateChange` 1: DAMAGED) none; back to 0, found; `setModelConditionState(DAMAGED)`
        /// none, and its clear found; a contain module's `clearAndSetModelConditionFlags` (DAMAGED) none and its
        /// `replaceModelConditionFlags` (no flags) found. A missing model (`NOSUCH`) answers as no render object: no bone,
        /// one model reported missing.
        #[test]
        fn issue175_the_logic_s_flags_choose_the_state() {
            use crate::common::model_state::MODELCONDITION_DAMAGED;
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::object_events::ObjectClientEvent;
            use crate::logic::world::team_ops::factory_find_team;
            let ini = "Object Flagged\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = GUNT\n   WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n\
                       \x20 ConditionState = DAMAGED\n   Model = None\n  End\n End\n\
                       Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n\
                       Object Missing\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = NOSUCH\n   WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n End\n\
                       Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";
            let mut w = gate_world_with(ini);
            let mut rig = Rig::new();
            let report = w.load_art_bones(&mut W3dFiles { assets: &mut rig.am, fs: &rig.fs }).expect("the art at hand");
            assert_eq!((report.templates, report.models_missing), (2, 1));
            let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
            let id = make(&mut w, b"Flagged", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
            let found = |w: &crate::logic::world::game_logic::World| !w.drawable_pristine_bones(id, b"MUZZLEFX", 1, 4).is_empty();
            assert!(found(&w));
            w.emit(ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id: id, state: 1 });
            assert!(!found(&w), "damaged: no model");
            w.emit(ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id: id, state: 0 });
            assert!(found(&w));
            w.set_model_condition_state(id, MODELCONDITION_DAMAGED);
            assert!(!found(&w));
            w.clear_model_condition_state(id, MODELCONDITION_DAMAGED);
            assert!(found(&w));
            w.contain_clear_and_set_model_condition_flags(id, &[], &[MODELCONDITION_DAMAGED]);
            assert!(!found(&w));
            w.contain_drawable_replace_model_condition_flags(id, &ModelConditionFlags::new());
            assert!(found(&w));
            let missing = make(&mut w, b"Missing", team, Coord3D { x: 120.0, y: 100.0, z: 0.0 });
            assert!(w.drawable_pristine_bones(missing, b"MUZZLEFX", 1, 4).is_empty());
            assert_eq!(crate::logic::weapon_host::WeaponHost::projectile_launch_offset(&mut w, missing, 0, 0, -1), None);
            // destroyed: the copy goes with the drawable
            w.send_object_destroyed(id);
            assert!(!w.drawable_condition.contains_key(&id));
        }

        /// (#178) `CACHE_ATTACH_BONE` (W3DModelDraw.h:278) on the logic's side: a template whose second draw module is
        /// attached to the first module's `MUZZLEFX01` bone (`AttachToBoneInAnotherModule`; a public bone of the first,
        /// `ExtraPublicBone`, as only public bones are cached) and has a turret but no model.
        /// Predictions, written before the first run (`issue178_predictions.md`): the logic's cache is filled at load by a
        /// drawable with no flags, so the bone's pristine translation at scale 1, (4, 0, 2), is added, unturned, to both
        /// turret positions (both bones missing: zero), whatever the flags at the query (DAMAGED, whose chassis has no
        /// model, asked first) and whatever the object's orientation; the launch position has no barrel (false). A second
        /// World with the same art answers the same, asked in the other order.
        #[test]
        fn issue178_the_logic_s_attach_offset_is_filled_at_load() {
            use crate::common::model_state::MODELCONDITION_DAMAGED;
            use crate::logic::world::gate_scenario::{gate_world_with, make};
            use crate::logic::world::team_ops::factory_find_team;
            let ini = "Object Technical\n Scale = 1.0\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       Draw = W3DModelDraw ModuleTag_Chassis\n  ExtraPublicBone = MuzzleFX01\n  DefaultConditionState\n   Model = GUNT\n  End\n\
                       \x20 ConditionState = DAMAGED\n   Model = None\n  End\n End\n\
                       Draw = W3DModelDraw ModuleTag_Gun\n  AttachToBoneInAnotherModule = MuzzleFX01\n  DefaultConditionState\n   Model = None\n\
                       \x20  Turret = Turret\n   TurretPitch = Barrel\n  End\n End\n\
                       Body = ActiveBody ModuleTag_Body\n  MaxHealth = 100\n  InitialHealth = 100\n End\nEnd\n";
            let offset = Coord3D { x: 4.0, y: 0.0, z: 2.0 };
            for damaged_first in [true, false] {
                let mut w = gate_world_with(ini);
                let mut rig = Rig::new();
                let report = w.load_art_bones(&mut W3dFiles { assets: &mut rig.am, fs: &rig.fs }).expect("the art at hand");
                assert_eq!(report.templates, 1);
                let team = factory_find_team(&mut w, b"team1").unwrap().unwrap();
                let id = make(&mut w, b"Technical", team, Coord3D { x: 100.0, y: 100.0, z: 0.0 });
                let mut turned = *w.find_object(id).unwrap().thing.transform_matrix();
                turned.rotate_z(1.0);
                w.set_transform_matrix(id, &turned);
                let ask = |w: &mut crate::logic::world::game_logic::World, damaged: bool| {
                    if damaged {
                        w.set_model_condition_state(id, MODELCONDITION_DAMAGED);
                    } else {
                        w.clear_model_condition_state(id, MODELCONDITION_DAMAGED);
                    }
                    let (found, lo) = w.live_projectile_launch_offset(id, 0, 0, 0);
                    assert!(!found, "no barrel");
                    assert_eq!((lo.turret_rot_pos, lo.turret_pitch_pos), (offset, offset), "damaged {damaged}");
                    assert_eq!(w.live_turret_rot_pos(id, 0), Some(offset));
                    // no turret: no offset
                    let (_, lo) = w.live_projectile_launch_offset(id, 0, 0, -1);
                    assert_eq!(lo.turret_rot_pos, Coord3D::default());
                };
                ask(&mut w, damaged_first);
                ask(&mut w, !damaged_first);
            }
        }

        /// (#175) `World::load_art_bones` is complete or refused: while a file is being fetched (the browser's source,
        /// [`Fetching`]) it refuses and leaves the World without bones; once the files are there it loads them all.
        #[test]
        fn issue175_the_art_bones_load_is_complete_or_refused() {
            use crate::logic::world::gate_scenario::gate_world_with;
            let ini = "Object ScaledGun\n KindOf = VEHICLE\n Geometry = CYLINDER\n GeometryMajorRadius = 5\n GeometryHeight = 5\n\
                       Draw = W3DModelDraw ModuleTag_Draw\n  DefaultConditionState\n   Model = GUNT\n   WeaponLaunchBone = PRIMARY MuzzleFX\n  End\n End\nEnd\n";
            let mut w = gate_world_with(ini);
            let mut rig = Rig::new();
            let mut src = Fetching { files: W3dFiles { assets: &mut rig.am, fs: &rig.fs }, arrived: false, skeleton_only: false, deferred: 0 };
            let r = w.load_art_bones(&mut src);
            assert!(matches!(r, Err(crate::logic::world::drawable_bones::ArtBonesDeferred(n)) if n > 0), "{r:?}");
            assert!(w.art_bones.templates.is_empty());
            src.arrived = true;
            let r = w.load_art_bones(&mut src).expect("arrived");
            assert_eq!((r.templates, r.states_with_bones), (1, 1));
            assert!(w.art_bones.templates.contains_key(&b"ScaledGun"[..]));
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
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
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
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: false, particles: None };
                    W3DModelDraw::new(data.clone(), 0, false, false, &mut env).unwrap()
                };
                assert!(!m.get_cur_state().unwrap().pristine_bones_valid());
                assert!(!data.validated_for(false, false));

                // made again at a logic time (the client's remake): found
                let before = src.deferred;
                let m = {
                    let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
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
            let mut env = DrawEnv { assets: &mut src, random: &mut rig.random, sync_time: 0, logic_time: true, particles: None };
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
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: true, particles: None };
                W3DModelDraw::new(d, 0, false, false, &mut env).unwrap()
            }
            fn frame(&mut self, m: &mut W3DModelDraw, facts: &DrawFacts) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
                m.do_draw_module(facts, &mut env);
            }
            fn set_flags(&mut self, m: &mut W3DModelDraw, bits: &[i32]) {
                let mut a = W3dFiles { assets: &mut self.am, fs: &self.fs };
                let mut env = DrawEnv { assets: &mut a, random: &mut self.random, sync_time: 0, logic_time: false, particles: None };
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

    /// (#171) The bone particle systems, end to end on a module with one HLOD (`RootTransform`, `Smoke01` at (1, 2, 3)
    /// turned a quarter about Z) and a `PS0` template with a slave `PS1`.
    ///
    /// Predictions, written before the first run: the constructor flags a recalc and makes nothing; the first draw makes
    /// `PS0` (id 1, with slave id 2) for `smoke01` (bone 1) at (2, 4, 6) under the drawable's scale 2, unsaveable
    /// (its slave too), attached to drawable 7, running, with the flag cleared, and `PS0` for `nosuch` (id 3, slave 4) at
    /// the origin with bone 0; `NoSuch` makes nothing. Hidden: both masters stopped; shown again: started. The update
    /// (`ParticlesAttachedToAnimatedBones`) moves only the bone-1 system, to the world's translation plus (2, 4, 6),
    /// skipping the parent's transform. The shroud stops them; a new state destroys all four (slaves with their masters)
    /// and empties the list; with `NO_STATE_PARTICLES` the next state flags nothing and the draw makes nothing.
    ///
    /// (#172) The asset source now makes the skeleton as upstream's render object made at scale 2 holds it (the pivot's
    /// translation scaled to (2, 4, 6), [`scale_render_obj_hierarchy`], posed under `Scale(2)`). Predictions, written
    /// before the run: every position above unchanged; the update's local transform has the bone's rotation *unscaled*
    /// (its Z row (0, 0, 1, 6); the old `world * (Scale * bone)` gave (0, 0, 2, 6)).
    #[test]
    fn bone_particle_systems_are_made_moved_and_destroyed() {
        struct Bones;
        impl W3dAssetSource for Bones {
            fn create_render_obj(&mut self, name: &[u8]) -> Option<bool> {
                name.eq_ignore_ascii_case(b"tank").then_some(true)
            }
            fn get_hanim(&mut self, _: &[u8]) -> Option<AnimHandle> {
                None
            }
            fn skeleton(&mut self, _: &[u8], _: Option<(&[u8], f32)>, scale: f32) -> Option<Skeleton> {
                let mut smoke = Matrix3D::IDENTITY;
                smoke.rotate_z(crate::common::base_type::PI / 2.0);
                smoke.set_translation(&crate::ww::vector3::Vector3::new(1.0, 2.0, 3.0));
                let name = |n: &[u8]| {
                    let mut a = [0u8; W3D_NAME_LEN];
                    a[..n.len()].copy_from_slice(n);
                    a
                };
                // (#172) the render object made at `scale` (`Create_Render_Obj`), posed under `Scale(scale)`
                let mut tree = HTree {
                    name: name(b"TANK"),
                    num_pivots: 2,
                    pivots: vec![Pivot::new(name(b"RootTransform"), None, Matrix3D::IDENTITY, 0), Pivot::new(name(b"Smoke01"), Some(0), smoke, 1)],
                    scale_factor: 1.0,
                };
                scale_render_obj_hierarchy(&mut tree, scale);
                let mut root = Matrix3D::IDENTITY;
                root.scale(scale);
                tree.base_update(&root).unwrap();
                Some(Skeleton { bones: Skeleton::bones_of(&tree), sub_objects: Vec::new(), scale_factor: tree.scale_factor })
            }
        }
        let lines: &[(u32, &str, &str)] = &[
            (0, "ParticlesAttachedToAnimatedBones", "Yes"),
            (0, "DefaultConditionState", ""),
            (1, "Model", "Tank"),
            (1, "ParticleSysBone", "Smoke01 PS0"),
            (1, "ParticleSysBone", "NoSuch PS0"),
            (1, "ParticleSysBone", "Smoke01 NoSuch"),
            (0, "ConditionState", "DAMAGED"),
            (1, "Model", "Tank"),
            (0, "ConditionState", "RUBBLE"),
            (1, "Model", "Tank"),
            (1, "ParticleSysBone", "Smoke01 PS0"),
        ];
        let mut mgr = ParticleSystemManager::new();
        mgr.new_template(b"PS0").info.slave_system_name = b"PS1".to_vec();
        mgr.new_template(b"PS1");
        let mut random = RandomValues::new(1);
        let mut assets = Bones;
        let d = Arc::new(parse(lines).unwrap());
        let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 5 }) };
        let mut m = W3DModelDraw::new_scaled(d.clone(), 0, false, false, 2.0, Arc::new(Vec::new()), &mut env).unwrap();
        m.drawable_id = 7;
        assert!(m.need_recalc_bone_particle_systems && m.particle_system_ids.is_empty());
        let mut world = Matrix3D::IDENTITY;
        world.set_translation(&crate::ww::vector3::Vector3::new(100.0, 0.0, 0.0));
        let facts = DrawFacts { transform: world, ..FACTS };
        m.do_draw_module(&facts, &mut env);
        drop(env);
        assert!(!m.need_recalc_bone_particle_systems);
        assert_eq!(m.particle_system_ids, vec![ParticleSysTrackerType { id: 1, bone_index: 1 }, ParticleSysTrackerType { id: 3, bone_index: 0 }]);
        assert_eq!(mgr.systems.len(), 4);
        let sys = |mgr: &ParticleSystemManager, id: u32| mgr.find_particle_system(id).unwrap().clone();
        assert!(mgr.systems.iter().all(|s| !s.is_saveable && !s.is_stopped));
        assert_eq!((sys(&mgr, 1).attached_to_drawable, sys(&mgr, 2).attached_to_drawable, sys(&mgr, 1).slave), (7, 0, Some(2)));
        // the draw's update moved the bone-1 system into the world ~~(100 + 2, 4, 6)~~ **Corrected 2026-11-13 (#174):** the
        // draw's root is the drawable's transform scaled by the instance scale (2, the module's `getScale()`), so the
        // bone is the pristine one in the world, (100 + 4, 8, 12); and it skips the parent's transform
        let p = sys(&mgr, 1).get_position();
        assert_eq!((p.x, p.y, p.z, sys(&mgr, 1).skip_parent_xfrm), (104.0, 8.0, 12.0, true));
        // ~~(#172) upstream's live bone: the rotation is not scaled~~ (#174) the rotation is scaled too: the draw's root
        let z = sys(&mgr, 1).local_transform.row[2];
        assert_eq!([z[0], z[1], z[2], z[3]], [0.0, 0.0, 2.0, 12.0]);
        let p = sys(&mgr, 3).get_position();
        assert_eq!((p.x, p.y, p.z, sys(&mgr, 3).skip_parent_xfrm, sys(&mgr, 3).is_local_identity), (0.0, 0.0, 0.0, false, false));
        m.set_hidden(true, Some(&mut mgr));
        assert!(sys(&mgr, 1).is_stopped && sys(&mgr, 3).is_stopped && !sys(&mgr, 2).is_stopped);
        m.set_hidden(false, Some(&mut mgr));
        assert!(!sys(&mgr, 1).is_stopped && !sys(&mgr, 3).is_stopped);
        m.set_fully_obscured_by_shroud(true, Some(&mut mgr));
        assert!(m.fully_obscured_by_shroud && sys(&mgr, 1).is_stopped);
        // (#179) the render object's own transform, put at the identity
        m.render_object.as_mut().unwrap().set_transform(&Matrix3D::IDENTITY);
        assert!(m.update_bones_for_client_particle_systems(Some(&mut mgr), 0));
        let p = sys(&mgr, 1).get_position();
        assert_eq!((p.x, p.y, p.z), (2.0, 4.0, 6.0));
        let damaged = d.find_best_info(&flags(&[crate::common::model_state::MODELCONDITION_DAMAGED])).unwrap();
        let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 6 }) };
        m.set_model_state(StateRef::Normal(damaged), &mut env);
        drop(env);
        assert!(m.particle_system_ids.is_empty() && m.need_recalc_bone_particle_systems);
        assert!(mgr.systems.iter().all(|s| s.is_destroyed));
        // a drawable whose status forbids state particles: no flag, nothing made
        m.drawable_no_state_particles = true;
        m.need_recalc_bone_particle_systems = false;
        let rubble = d.find_best_info(&flags(&[crate::common::model_state::MODELCONDITION_RUBBLE])).unwrap();
        let mut env = DrawEnv { assets: &mut assets, random: &mut random, sync_time: 0, logic_time: false, particles: Some(DrawParticles { manager: &mut mgr, frame: 7 }) };
        m.set_model_state(StateRef::Normal(rubble), &mut env);
        m.do_draw_module(&facts, &mut env);
        drop(env);
        assert!(!m.need_recalc_bone_particle_systems && m.particle_system_ids.is_empty() && mgr.systems.len() == 4);
        // without a manager, nothing is found or made and the list is cleared
        m.particle_system_ids.push(ParticleSysTrackerType { id: 1, bone_index: 1 });
        m.stop_client_particle_systems(None);
        assert!(m.particle_system_ids.is_empty());
    }
    /// (#179) A render object's live bones follow its animation, as `Animatable3DObjClass::Get_Bone_Transform` poses
    /// them. Two pivots (pivot 1 at (1, 0, 0) under the root), `hanim::tests::raw_anim` (pivot 1's X = 0, 10, 40 at
    /// frames 0, 1, 2; 30 frames a second) under the portable `Float_To_Long`. Predictions (issue179_predictions.md):
    /// LOOP from frame 0 at sync time 0: bone 1 at x = 1, the tree valid; asked again at 50 ms with nothing invalidating it,
    /// the same pose and frame 0 (the cached pose); after `Set_Transform` at 50 ms, the frame moves to 1.5 (measured:
    /// 1.5000001) and the raw
    /// fast path reads frame 1 (truncated): x = 11 (the virtual `Anim_Update` would interpolate 25: x = 26); `MANUAL` at
    /// frame 2 stays at x = 41 whatever the time; a blend of frames 0 and 2 at 0.25: x = 1 + 0.25 * 40 = 11; a combo of
    /// the animation alone at frame 2, weight 1: x = 41; under `Scale(2)` the combo's bone at 2 * 41 = 82, and
    /// `get_bone_transform_under` leaves the tree not valid and the transform restored.
    #[test]
    fn a_render_object_s_bones_follow_its_animation() {
        use crate::formats::w3d::hanim_combo::HAnimCombo;
        let raw = crate::formats::w3d::hanim::tests::raw_anim().with_frame_rounding(crate::ww::wwmath::FloatToLongMode::PortableTruncate);
        let data = Arc::new(AnimData { motion: Arc::new(HAnim::Raw(raw)), poses: Vec::new() });
        let handle = AnimHandle { name: b"SKL.WALK".to_vec(), num_frames: 3, frame_rate: 30.0, data: Some(data.clone()) };
        let mut base = Matrix3D::IDENTITY;
        base.translate(1.0, 0.0, 0.0);
        let bone = |name: &[u8], parent, base| SkeletonBone { name: name.to_vec(), parent_index: parent, transform: Matrix3D::IDENTITY, base };
        let k = Skeleton { bones: vec![bone(b"ROOT", 0, Matrix3D::IDENTITY), bone(b"B1", 0, base)], sub_objects: Vec::new(), scale_factor: 1.0 };
        let mut r = RenderObj::new(b"SKL", true).with_skeleton(Some(k));
        let x = |r: &mut RenderObj, now| r.get_bone_transform(1, now).get_translation().x;

        r.set_animation(Some(handle.clone()), 0.0, AnimMode::Loop, 0);
        assert!(!r.is_hierarchy_valid());
        assert_eq!(x(&mut r, 0), 1.0);
        assert!(r.is_hierarchy_valid());
        assert_eq!((x(&mut r, 50), r.anim.as_ref().unwrap().frame), (1.0, 0.0));
        r.set_transform(&Matrix3D::IDENTITY);
        // ~~frame 1.5~~ **Corrected after the first run (a prediction miss):** `30 * 50 * 0.001f` is 1.5000001 in `float`
        assert_eq!((x(&mut r, 50), r.anim.as_ref().unwrap().frame), (11.0, 30.0f32 * 1.0 * 1.0 * 50.0 * 0.001));
        assert_eq!(r.anim.as_ref().unwrap().frame, 1.500_000_1);

        r.set_animation(Some(handle.clone()), 2.0, AnimMode::Manual, 100);
        r.set_transform(&Matrix3D::IDENTITY);
        assert_eq!((x(&mut r, 1000), r.anim.as_ref().unwrap().frame), (41.0, 2.0));

        r.set_animation_blend(Some(handle.clone()), 0.0, Some(handle.clone()), 2.0, 0.25);
        assert!(r.anim.is_none());
        assert_eq!(x(&mut r, 1000), 11.0);

        let combo: HAnimCombo<usize> = HAnimCombo::with_anims(1);
        combo.set_motion(0, Some(0)).unwrap();
        combo.set_frame(0, 2.0).unwrap();
        combo.set_weight(0, 1.0).unwrap();
        r.set_animation_combo(Some(ModeCombo { combo: Arc::new(combo), motions: Arc::new(vec![data]) }));
        assert_eq!(x(&mut r, 1000), 41.0);

        let mut s2 = Matrix3D::IDENTITY;
        s2.scale(2.0);
        let t = r.get_bone_transform_under(&s2, 1, 1000);
        assert_eq!(t.get_translation().x, 82.0);
        assert!(!r.is_hierarchy_valid());
        assert_eq!(r.transform, Matrix3D::IDENTITY);
    }
}
