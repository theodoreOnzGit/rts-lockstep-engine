// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/ObjectCreationList.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/ObjectCreationList.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Steven Johnson, December 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectCreationList` and its store (`TheObjectCreationListStore`), wave 7, `p1w10-sonnet`: the INI-defined recipes that make objects,
//! debris, fired weapons, payload deliveries and physics kicks ("a way of creating a particular set of Objects"), used by weapons, deaths,
//! upgrades, special powers and scripts.
//!
//! # The shape
//!
//! An [`ObjectCreationList`] is a list of [`Nugget`]s; the nuggets are owned by the [`ObjectCreationListStore`] and shared (`Arc`), as
//! upstream's raw pointers share them. Upstream's abstract `ObjectCreationNugget` with its five subclasses is the [`Nugget`] enum
//! (`FireWeaponNugget`, `AttackNugget`, `DeliverPayloadNugget`, `ApplyRandomForceNugget`, `GenericObjectCreationNugget`), and the three virtual
//! `create` overloads are [`Nugget::create_object`] (`create(primary, secondary, life)`), [`Nugget::create_with_owner`]
//! (`create(primaryObj, primary, secondary, Bool createOwner, life)`) and [`Nugget::create_at`] (`create(primaryObj, primary, secondary,
//! Real angle, life)`), each with the base class's default where the subclass does not override it.
//!
//! The objects the file makes and the modules it finds in them are another unit's (the object system, `DeliverPayloadAIUpdate`,
//! `PhysicsBehavior`, `LifetimeUpdate`, `RadiusDecalUpdate`, the drawable, the terrain, the pathfinder, the partition manager, the weapon store,
//! the particle systems, the audio): everything the creation reaches is an [`OclHost`] call, keyed by object id, which the layer-12 adapter
//! answers from the `World`. Each call is named after the upstream call it stands for.
//!
//! The INI side is a [`FieldParse`] table per nugget class, composed with `MultiIniFieldParse` where upstream does. What the parsers look up
//! (`INI::parseWeaponTemplate`, `INI::parseFXList`, `TheGlobalData->m_preloadAssets`) is an [`OclParseEnv`].
//!
//! # Upstream quirks kept
//!
//! - **`parseDebrisObjectNames` keeps every other token.** Its loop reads the next token in the body and again in the `for` increment, so
//!   `ObjectNames = A B C D` makes the names `A` and `C`. (With `m_preloadAssets` the kept names are also appended to
//!   `debrisModelNamesGlobalHack`, a global nothing reads.)
//! - **The formation offset is truncated to `Int`:** `Int offsetMultiplier = (formationIndex + 1) / 2 * m_formationSpacing`, so a spacing of
//!   `25.5` places the planes `25`, `25`, `51`, `51`, ... apart.
//! - **`reallyCreate` caches the `GenericDebris` template in a function-local `static`:** the first lookup of the process is kept for good.
//!   [`OclHost::debris_template`] is that lookup, and a host must memoise it the way the static does.
//! - **`MinLODRequired` and `ObjectCount` are parsed and never read;** the LOD gate is `TheGameLODManager->isDebrisSkipped()`.
//! - **A redefined list keeps its old nuggets in the store** (`ObjectCreationList::clear()` "does NOT delete the nuggets -- they're owned
//!   by the Store"): the store only grows, and a block that fails half way leaves the nuggets completed before the error in the list.
//! - **`createInternal` returns the first non-`NULL` object of its nuggets,** whichever nugget made it.
//! - **`FireWeapon`, `Attack` and `DeliverPayload` reached through the object overload** (`create(primary, secondary, life)`) get the
//!   positions of the two objects and `INVALID_ANGLE`; `ApplyRandomForce` reached through either position overload does nothing.
//! - **A fade-in and a fade-out can both apply** to one object, each with its own audio event.
//! - **`ThingFactory::newObject` can return `NULL`** for the payload and its container in `DeliverPayloadNugget`, and the nugget then writes
//!   through the null pointer (`payload->setPosition`).
//!
//! # Where upstream is undefined
//!
//! A null pointer is dereferenced by: `DeliverPayloadNugget` with a primary object that has no controlling player
//! (`getControllingPlayer()->getDefaultTeam()`), or with `StartAtMaxSpeed` and a transport that has physics but no body module; a payload
//! or container that `newObject` could not make; `CreateDebris` (a model name, not an object) on an object with no drawable; a fade of an
//! object with no drawable, or a fade with no source object; `ContainInsideSourceObject` with no source object. Each is
//! [`OclError::Undefined`] here, and the differential test gives the C++ the same inputs under a `SIGSEGV` handler and compares where
//! the two stop. A float-to-`Int` conversion out of range (a formation spacing or a health percentage in the billions, or not a number)
//! is undefined too and is not given to either side.
//!
//! Verified by `tests/differential_object_creation_list.rs` against the unchanged `ObjectCreationList.cpp`; results in its header.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::common::base_type::{real_to_int, Coord3D, PI};
use crate::common::damage_fx::FxListName;
use crate::common::disabled_types::{DisabledType, DISABLED_DEFAULT};
use crate::common::game_common::{CommandSourceType, VeterancyLevel, SECONDS_PER_LOGICFRAME_REAL};
use crate::common::game_type::{ObjectId, PathfindLayer, INVALID_ANGLE, INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniResult, MultiIniFieldParse, UserData};
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};
use crate::common::thing_template::SHADOW_NAMES;
use crate::common::trig::{cos, sin};
use crate::logic::damage::{DamageInfo, DamageType, DeathType, DAMAGE_WATER, HUGE_DAMAGE_AMOUNT};
use crate::logic::modules::deliver_payload_data::{
    DeliverPayloadData, DeliverPayloadDataParse, WeaponTemplateRef, WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST,
};
use crate::logic::object_script_status_bits::OBJECT_STATUS_SCRIPT_TARGETABLE;
use crate::logic::radius_decal::RadiusDecalTemplate;
use crate::logic::world::build_assistant::PathfindCellType;
use crate::logic::world::ids::{PlayerIndex, TeamId};
use crate::logic::world::team_ops::TemplateRef;
use crate::ww::matrix3d::Matrix3D;
use crate::ww::vector3::Vector3;

/// (test-only) the planted bug that is switched on by `OCL_PLANT`.
#[doc(hidden)]
pub fn plant(k: u32) -> bool {
    use std::sync::OnceLock;
    static P: OnceLock<u32> = OnceLock::new();
    *P.get_or_init(|| std::env::var("OCL_PLANT").ok().and_then(|s| s.parse().ok()).unwrap_or(0)) == k
}

/// Where upstream is undefined: see the module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OclError {
    /// A null pointer upstream dereferences; the text says where.
    Undefined(&'static str),
}

impl OclError {
    /// Upstream is undefined here.
    pub fn is_undefined(&self) -> bool {
        matches!(self, OclError::Undefined(_))
    }
}

/// The result of a creation.
pub type OclResult<T> = Result<T, OclError>;

/// `DEATH_FLOODED` (`Damage.h`): index 7 of `DEATH_NAMES`.
pub const DEATH_FLOODED: DeathType = 7;
/// `LOCKED_TEMPORARILY` (`WeaponSet.h`).
pub const LOCKED_TEMPORARILY: i32 = 1;
/// `PRIMARY_WEAPON`.
pub const PRIMARY_WEAPON: i32 = 0;
/// `PATHFIND_CELL_SIZE` (`AIPathfind.h`).
pub const PATHFIND_CELL_SIZE: i32 = 10;
/// `STATIC_GAME_LOD_LOW`.
pub const STATIC_GAME_LOD_LOW: i32 = 0;
/// `StaticGameLODNames[]` (`GameLOD.cpp`), indexed by `StaticGameLODLevel`.
pub const STATIC_GAME_LOD_NAMES: [&str; 4] = ["Low", "Medium", "High", "Custom"];
/// `FPF_USE_HIGHEST_LAYER` (`PartitionManager.h`).
pub const FPF_USE_HIGHEST_LAYER: u32 = 0x80;
/// `RANDOM_START_ANGLE` (`PartitionManager.h`).
pub const RANDOM_START_ANGLE: f32 = 99999.9;

/// `enum DebrisDisposition`.
pub const LIKE_EXISTING: u32 = 0x0000_0001;
/// `ON_GROUND_ALIGNED`.
pub const ON_GROUND_ALIGNED: u32 = 0x0000_0002;
/// `SEND_IT_FLYING`.
pub const SEND_IT_FLYING: u32 = 0x0000_0004;
/// `SEND_IT_UP`.
pub const SEND_IT_UP: u32 = 0x0000_0008;
/// `SEND_IT_OUT`.
pub const SEND_IT_OUT: u32 = 0x0000_0010;
/// `RANDOM_FORCE`.
pub const RANDOM_FORCE: u32 = 0x0000_0020;
/// `FLOATING`.
pub const FLOATING: u32 = 0x0000_0040;
/// `INHERIT_VELOCITY`.
pub const INHERIT_VELOCITY: u32 = 0x0000_0080;
/// `WHIRLING`.
pub const WHIRLING: u32 = 0x0000_0100;

/// `DebrisDispositionNames[]`, without the terminator.
pub const DEBRIS_DISPOSITION_NAMES: [&str; 9] = [
    "LIKE_EXISTING",
    "ON_GROUND_ALIGNED",
    "SEND_IT_FLYING",
    "SEND_IT_UP",
    "SEND_IT_OUT",
    "RANDOM_FORCE",
    "FLOATING",
    "INHERIT_VELOCITY",
    "WHIRLING",
];

/// The modules of an object that `ObjectCreationList.cpp` finds with `findUpdateModule(NAMEKEY(...))`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateModuleKind {
    /// `RadiusDecalUpdate`.
    RadiusDecal,
    /// `DeliverPayloadAIUpdate`.
    DeliverPayloadAi,
    /// `LifetimeUpdate`.
    Lifetime,
    /// `FloatUpdate`.
    Float,
}

/// `FindPositionOptions` (`PartitionManager.h`), the fields `ObjectCreationList.cpp` can set or leave at their defaults (the three object
/// pointers are always `NULL` here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FindPositionOptions {
    /// `flags` (`FindPositionFlags`).
    pub flags: u32,
    /// `minRadius`.
    pub min_radius: f32,
    /// `maxRadius`.
    pub max_radius: f32,
    /// `startAngle`.
    pub start_angle: f32,
    /// `maxZDelta`.
    pub max_z_delta: f32,
}

impl Default for FindPositionOptions {
    /// `FindPositionOptions()`.
    fn default() -> Self {
        FindPositionOptions { flags: 0, min_radius: 0.0, max_radius: 0.0, start_angle: RANDOM_START_ANGLE, max_z_delta: 1e10 }
    }
}

/// What `ObjectCreationList.cpp` reaches and another unit owns. Every method is the upstream call named in its comment, `o` being the
/// object the call is made on; an object that does not exist, or a module it does not have, is the caller's error (upstream
/// dereferences it).
pub trait OclHost {
    // ---- global state ---------------------------------------------------------------------------------------------
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLODManager->isDebrisSkipped()`.
    fn is_debris_skipped(&self) -> bool;
    /// `GameLogicRandomValue(lo, hi)`.
    fn random_value(&mut self, lo: i32, hi: i32) -> i32;
    /// `GameLogicRandomValueReal(lo, hi)`.
    fn random_value_real(&mut self, lo: f32, hi: f32) -> f32;
    /// `TheGameLogic->destroyObject(o)`.
    fn destroy_object(&mut self, o: ObjectId);
    /// `ThePlayerList->getNeutralPlayer() ? ThePlayerList->getNeutralPlayer()->getDefaultTeam() : NULL`.
    fn neutral_player_default_team(&self) -> Option<TeamId>;
    /// `player->getDefaultTeam()` (`None` for `NULL`).
    fn player_default_team(&self, player: PlayerIndex) -> Option<TeamId>;
    /// `player->isPlayerActive()`.
    fn player_is_active(&self, player: PlayerIndex) -> bool;
    /// `TheScriptEngine->transferObjectName(name, o)`.
    fn transfer_object_name(&mut self, name: &[u8], o: ObjectId);
    // ---- the thing factory ----------------------------------------------------------------------------------------
    /// `TheThingFactory->findTemplate("GenericDebris")`, cached by the host as upstream's function-local `static` caches it.
    fn debris_template(&mut self) -> Option<TemplateRef>;
    /// `TheThingFactory->findTemplate(name)` (`None` for `NULL`).
    fn find_template(&mut self, name: &[u8]) -> Option<TemplateRef>;
    /// `TheThingFactory->newObject(tmplate, team)` (`None` for `NULL`, also for a `NULL` template).
    fn new_object(&mut self, tmplate: Option<TemplateRef>, team: Option<TeamId>) -> Option<ObjectId>;
    // ---- an object -------------------------------------------------------------------------------------------------
    /// `*o->getPosition()`.
    fn obj_position(&self, o: ObjectId) -> Coord3D;
    /// `o->getOrientation()`.
    fn obj_orientation(&self, o: ObjectId) -> f32;
    /// `*o->getTransformMatrix()`.
    fn obj_transform(&self, o: ObjectId) -> Matrix3D;
    /// `o->getControllingPlayer()`'s index (`None` for `NULL`).
    fn obj_controlling_player(&self, o: ObjectId) -> Option<PlayerIndex>;
    /// `o->getLayer()`.
    fn obj_layer(&self, o: ObjectId) -> PathfindLayer;
    /// `o->isAboveTerrain()`.
    fn obj_is_above_terrain(&self, o: ObjectId) -> bool;
    /// `o->isSignificantlyAboveTerrain()`.
    fn obj_is_significantly_above_terrain(&self, o: ObjectId) -> bool;
    /// `o->isOffMap()`.
    fn obj_is_off_map(&self, o: ObjectId) -> bool;
    /// `o->isKindOf(KINDOF_STRUCTURE)`.
    fn obj_is_structure(&self, o: ObjectId) -> bool;
    /// `o->getVeterancyLevel()`.
    fn obj_veterancy_level(&self, o: ObjectId) -> VeterancyLevel;
    /// `o->getIndicatorColor()`.
    fn obj_indicator_color(&self, o: ObjectId) -> i32;
    /// `o->getName()`.
    fn obj_name(&self, o: ObjectId) -> Vec<u8>;
    /// `*o->getUnitDirectionVector2D()`.
    fn obj_unit_direction_vector_2d(&self, o: ObjectId) -> Coord3D;
    /// `o->setProducer(producer)` (`None` for `NULL`).
    fn obj_set_producer(&mut self, o: ObjectId, producer: Option<ObjectId>);
    /// `o->setPosition(pos)`.
    fn obj_set_position(&mut self, o: ObjectId, pos: &Coord3D);
    /// `o->setOrientation(angle)`.
    fn obj_set_orientation(&mut self, o: ObjectId, angle: f32);
    /// `o->setTransformMatrix(mtx)`.
    fn obj_set_transform_matrix(&mut self, o: ObjectId, mtx: &Matrix3D);
    /// `o->setLayer(layer)`.
    fn obj_set_layer(&mut self, o: ObjectId, layer: PathfindLayer);
    /// `o->setScriptStatus(bit)` (with the default `set = true`).
    fn obj_set_script_status(&mut self, o: ObjectId, bit: u8);
    /// `o->setDisabledUntil(type, frame)`.
    fn obj_set_disabled_until(&mut self, o: ObjectId, disabled: DisabledType, frame: u32);
    /// `o->setWeaponLock(slot, lock)`.
    fn obj_set_weapon_lock(&mut self, o: ObjectId, slot: i32, lock: i32);
    /// `o->goInvulnerable(frames)`.
    fn obj_go_invulnerable(&mut self, o: ObjectId, frames: u32);
    /// `o->kill()`.
    fn obj_kill(&mut self, o: ObjectId);
    /// `o->attemptDamage(&info)`.
    fn obj_attempt_damage(&mut self, o: ObjectId, info: &DamageInfo);
    /// `o->findUpdateModule(NAMEKEY(...)) != NULL`.
    fn obj_has_update_module(&self, o: ObjectId, kind: UpdateModuleKind) -> bool;
    /// `o->findSpecialPowerCompletionDie() != NULL`.
    fn obj_has_special_power_completion_die(&self, o: ObjectId) -> bool;
    /// `o->getAIUpdateInterface() != NULL`.
    fn obj_has_ai(&self, o: ObjectId) -> bool;
    /// `o->getPhysics() != NULL`.
    fn obj_has_physics(&self, o: ObjectId) -> bool;
    /// `o->getBodyModule() != NULL`.
    fn obj_has_body(&self, o: ObjectId) -> bool;
    /// `o->getContain() != NULL`.
    fn obj_has_contain(&self, o: ObjectId) -> bool;
    /// `o->getDrawable() != NULL`.
    fn obj_has_drawable(&self, o: ObjectId) -> bool;
    /// `o->getExperienceTracker()->isTrainable()`.
    fn obj_is_trainable(&self, o: ObjectId) -> bool;
    /// `o->getExperienceTracker()->setVeterancyLevel(level)`.
    fn obj_set_veterancy_level(&mut self, o: ObjectId, level: VeterancyLevel);
    // ---- the modules of an object ---------------------------------------------------------------------------------
    /// `ai->aiAttackPosition(pos, maxShots, cmd)`.
    fn ai_attack_position(&mut self, o: ObjectId, pos: &Coord3D, max_shots: i32, cmd: CommandSourceType);
    /// `rd->createRadiusDecal(tmpl, radius, pos)`.
    fn radius_decal_create(&mut self, o: ObjectId, tmpl: &RadiusDecalTemplate, radius: f32, pos: &Coord3D);
    /// `rd->killWhenNoLongerAttacking(TRUE)`.
    fn radius_decal_kill_when_no_longer_attacking(&mut self, o: ObjectId);
    /// `dpai->deliverPayload(moveToPos, targetPos, data)`.
    fn deliver_payload(&mut self, o: ObjectId, move_to: &Coord3D, target: &Coord3D, data: &DeliverPayloadData);
    /// `dpai->getCurLocomotor()->getMaxSpeedForCondition(damageState)`.
    fn deliver_payload_max_speed_for_condition(&self, o: ObjectId, damage_state: i32) -> f32;
    /// `dpai->getCurLocomotor()->getPreferredHeight()`.
    fn deliver_payload_preferred_height(&self, o: ObjectId) -> f32;
    /// `o->getBodyModule()->getDamageState()`.
    fn body_damage_state(&self, o: ObjectId) -> i32;
    /// `body->setInitialHealth(percent)`.
    fn body_set_initial_health(&mut self, o: ObjectId, percent: i32);
    /// `lup->setLifetimeRange(min, max)`.
    fn lifetime_set_range(&mut self, o: ObjectId, min_frames: u32, max_frames: u32);
    /// `floatUpdate->setEnabled(TRUE)`.
    fn float_set_enabled(&mut self, o: ObjectId);
    /// `die->setCreator(id)`.
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, creator: ObjectId);
    /// The number of entries of `o->getBehaviorModules()` (up to the `NULL`).
    fn behavior_module_count(&self, o: ObjectId) -> usize;
    /// `o->getBehaviorModules()[i]->getSlavedUpdateInterface() != NULL`.
    fn behavior_module_has_slaved_update(&self, o: ObjectId, i: usize) -> bool;
    /// `o->getBehaviorModules()[i]->getSlavedUpdateInterface()->onEnslave(slaver)`.
    fn behavior_module_on_enslave(&mut self, o: ObjectId, i: usize, slaver: Option<ObjectId>);
    // ---- physics ----------------------------------------------------------------------------------------------------
    /// `physics->getVelocity()`.
    fn physics_velocity(&self, o: ObjectId) -> Coord3D;
    /// `physics->getMass()`.
    fn physics_mass(&self, o: ObjectId) -> f32;
    /// `physics->applyMotiveForce(&force)`.
    fn physics_apply_motive_force(&mut self, o: ObjectId, force: &Coord3D);
    /// `physics->applyForce(&force)`.
    fn physics_apply_force(&mut self, o: ObjectId, force: &Coord3D);
    /// `physics->setMass(mass)`.
    fn physics_set_mass(&mut self, o: ObjectId, mass: f32);
    /// `physics->setExtraFriction(f)`.
    fn physics_set_extra_friction(&mut self, o: ObjectId, f: f32);
    /// `physics->setExtraBounciness(b)`.
    fn physics_set_extra_bounciness(&mut self, o: ObjectId, b: f32);
    /// `physics->setAllowBouncing(b)`.
    fn physics_set_allow_bouncing(&mut self, o: ObjectId, b: bool);
    /// `physics->setAllowToFall(b)`.
    fn physics_set_allow_to_fall(&mut self, o: ObjectId, b: bool);
    /// `physics->setBounceSound(&event)`, the event named `name`.
    fn physics_set_bounce_sound(&mut self, o: ObjectId, name: &[u8]);
    /// `physics->setIgnoreCollisionsWith(other)` (`None` for `NULL`).
    fn physics_set_ignore_collisions_with(&mut self, o: ObjectId, other: Option<ObjectId>);
    /// `physics->setAngles(yaw, pitch, roll)`.
    fn physics_set_angles(&mut self, o: ObjectId, yaw: f32, pitch: f32, roll: f32);
    /// `physics->setYawRate(r)`.
    fn physics_set_yaw_rate(&mut self, o: ObjectId, r: f32);
    /// `physics->setRollRate(r)`.
    fn physics_set_roll_rate(&mut self, o: ObjectId, r: f32);
    /// `physics->setPitchRate(r)`.
    fn physics_set_pitch_rate(&mut self, o: ObjectId, r: f32);
    // ---- containment ------------------------------------------------------------------------------------------------
    /// `container->getContain()->isValidContainerFor(obj, checkCapacity)`.
    fn contain_is_valid_container_for(&self, container: ObjectId, obj: ObjectId, check_capacity: bool) -> bool;
    /// `container->getContain()->addToContain(obj)`.
    fn contain_add(&mut self, container: ObjectId, obj: ObjectId);
    // ---- the drawable -----------------------------------------------------------------------------------------------
    /// `o->getDrawable()->isDrawableEffectivelyHidden()`.
    fn drawable_is_effectively_hidden(&self, o: ObjectId) -> bool;
    /// `o->getDrawable()->setDrawableHidden(h)`.
    fn drawable_set_hidden(&mut self, o: ObjectId, hidden: bool);
    /// `o->getDrawable()->fadeIn(frames)`.
    fn drawable_fade_in(&mut self, o: ObjectId, frames: u32);
    /// `o->getDrawable()->fadeOut(frames)`.
    fn drawable_fade_out(&mut self, o: ObjectId, frames: u32);
    /// The number of draw modules of `o`'s drawable that have a `DebrisDrawInterface`, in module order.
    fn drawable_debris_interface_count(&self, o: ObjectId) -> usize;
    /// `di->setModelName(name, color, shadowType)` on the `i`-th debris interface.
    fn debris_set_model_name(&mut self, o: ObjectId, i: usize, name: &[u8], color: i32, shadow_type: u32);
    /// `di->setAnimNames(initial, flying, final, finalFX)` on the `i`-th debris interface.
    fn debris_set_anim_names(&mut self, o: ObjectId, i: usize, initial: &[u8], flying: &[u8], fin: &[u8], final_fx: &FxListName);
    // ---- the world --------------------------------------------------------------------------------------------------
    /// `TheTerrainLogic->getGroundHeight(x, y)`.
    fn terrain_ground_height(&self, x: f32, y: f32) -> f32;
    /// `TheTerrainLogic->getHighestLayerForDestination(&pos)`.
    fn terrain_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayer;
    /// `TheTerrainLogic->getLayerHeight(x, y, layer)`.
    fn terrain_layer_height(&self, x: f32, y: f32, layer: PathfindLayer) -> f32;
    /// `TheTerrainLogic->flattenTerrain(o)`.
    fn terrain_flatten(&mut self, o: ObjectId);
    /// `TheTerrainLogic->isUnderwater(x, y, &waterZ, &terrainZ)`: the water and terrain heights if the point is underwater.
    fn terrain_is_underwater(&self, x: f32, y: f32) -> Option<(f32, f32)>;
    /// `TheAI->pathfinder()->getCell(layer, x, y)->getType()` (`None` for `NULL`).
    fn pathfinder_cell_type(&self, layer: PathfindLayer, x: i32, y: i32) -> Option<PathfindCellType>;
    /// `TheAI->pathfinder()->addObjectToPathfindMap(o)`.
    fn pathfinder_add_object(&mut self, o: ObjectId);
    /// `ThePartitionManager->findPositionAround(center, &options, &result)`.
    fn partition_find_position_around(&mut self, center: &Coord3D, options: &FindPositionOptions) -> Coord3D;
    /// `TheWeaponStore->createAndFireTempWeapon(weapon, source, &pos)`.
    fn weapon_create_and_fire_temp(&mut self, weapon: WeaponTemplateRef, source: ObjectId, pos: &Coord3D);
    /// `TheParticleSystemManager->findTemplate(name)` (`None` for `NULL`).
    fn particle_system_find_template(&self, name: &[u8]) -> Option<u32>;
    /// `TheParticleSystemManager->createParticleSystem(tmpl)`, then `->attachToObject(o)`.
    fn particle_system_create_attached(&mut self, tmpl: u32, o: ObjectId);
    /// `TheAudio->addAudioEvent(&event)` for an event named `name` with `setObjectID(object)`.
    fn audio_add_event(&mut self, name: &[u8], object: ObjectId);
}

// -------------------------------------------------------------------------------------------------------------------------------
// the nuggets
// -------------------------------------------------------------------------------------------------------------------------------

/// `adjustVector(vec, mtx)`: `vec` rotated by the matrix's 3x3 part, if there is a matrix.
fn adjust_vector(vec: &mut Coord3D, mtx: Option<&Matrix3D>) {
    if let Some(m) = mtx {
        if plant(30) {
            return;
        }
        let v = m.rotate_vector(&Vector3::new(vec.x, vec.y, vec.z));
        vec.x = v.x;
        vec.y = v.y;
        vec.z = v.z;
    }
}

/// `calcRandomForce(minMag, maxMag, minPitch, maxPitch, &force)`.
fn calc_random_force<H: OclHost>(host: &mut H, min_mag: f32, max_mag: f32, min_pitch: f32, max_pitch: f32) -> Coord3D {
    let (angle, pitch, mag);
    if plant(31) {
        mag = host.random_value_real(min_mag, max_mag);
        pitch = host.random_value_real(min_pitch, max_pitch);
        angle = host.random_value_real(0.0, 2.0 * PI);
    } else {
        angle = host.random_value_real(0.0, 2.0 * PI);
        pitch = host.random_value_real(min_pitch, max_pitch);
        mag = host.random_value_real(min_mag, max_mag);
    }
    let mut mtx = Matrix3D::IDENTITY;
    mtx.scale(mag);
    mtx.rotate_z(angle);
    mtx.rotate_y(if plant(32) { pitch } else { -pitch });
    let v = mtx.get_x_vector();
    Coord3D { x: v.x, y: v.y, z: v.z }
}

/// `class FireWeaponNugget`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FireWeaponNugget {
    /// `m_weapon`.
    pub weapon: Option<WeaponTemplateRef>,
}

/// `class AttackNugget`.
#[derive(Clone, Debug, PartialEq)]
pub struct AttackNugget {
    /// `m_deliveryDecalTemplate`.
    pub delivery_decal_template: RadiusDecalTemplate,
    /// `m_deliveryDecalRadius`.
    pub delivery_decal_radius: f32,
    /// `m_numberOfShots`.
    pub number_of_shots: i32,
    /// `m_weaponSlot`.
    pub weapon_slot: i32,
}

impl Default for AttackNugget {
    /// `AttackNugget()`.
    fn default() -> Self {
        AttackNugget {
            delivery_decal_template: RadiusDecalTemplate::default(),
            delivery_decal_radius: 0.0,
            number_of_shots: if plant(1) { 2 } else { 1 },
            weapon_slot: if plant(2) { 1 } else { PRIMARY_WEAPON },
        }
    }
}

/// `DeliverPayloadNugget::Payload`.
#[derive(Clone, Debug, PartialEq)]
pub struct Payload {
    /// `m_payloadName`.
    pub payload_name: Vec<u8>,
    /// `m_payloadCount`.
    pub payload_count: i32,
}

/// `class DeliverPayloadNugget`.
#[derive(Clone, Debug, PartialEq)]
pub struct DeliverPayloadNugget {
    /// `m_transportName`.
    pub transport_name: Vec<u8>,
    /// `m_putInContainerName`.
    pub put_in_container_name: Vec<u8>,
    /// `m_payload`.
    pub payload: Vec<Payload>,
    /// `m_formationSpacing`.
    pub formation_spacing: f32,
    /// `m_convergenceFactor`.
    pub convergence_factor: f32,
    /// `m_errorRadius`.
    pub error_radius: f32,
    /// `m_delayDeliveryFramesMax`.
    pub delay_delivery_frames_max: u32,
    /// `m_formationSize`.
    pub formation_size: u32,
    /// `m_startAtPreferredHeight`.
    pub start_at_preferred_height: bool,
    /// `m_startAtMaxSpeed`.
    pub start_at_max_speed: bool,
    /// `m_data`.
    pub data: DeliverPayloadData,
}

impl Default for DeliverPayloadNugget {
    /// `DeliverPayloadNugget()`.
    fn default() -> Self {
        DeliverPayloadNugget {
            transport_name: Vec::new(),
            put_in_container_name: Vec::new(),
            payload: Vec::new(),
            formation_spacing: if plant(3) { 20.0 } else { 25.0 },
            convergence_factor: 0.0,
            error_radius: 0.0,
            delay_delivery_frames_max: 0,
            formation_size: if plant(4) { 2 } else { 1 },
            start_at_preferred_height: !plant(5),
            start_at_max_speed: plant(6),
            data: DeliverPayloadData::default(),
        }
    }
}

/// `class ApplyRandomForceNugget`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ApplyRandomForceNugget {
    /// `m_spinRate`.
    pub spin_rate: f32,
    /// `m_minMag`.
    pub min_mag: f32,
    /// `m_maxMag`.
    pub max_mag: f32,
    /// `m_minPitch`.
    pub min_pitch: f32,
    /// `m_maxPitch`.
    pub max_pitch: f32,
}

/// `GenericObjectCreationNugget::AnimSet`.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimSet {
    /// `m_animInitial`.
    pub anim_initial: Vec<u8>,
    /// `m_animFlying`.
    pub anim_flying: Vec<u8>,
    /// `m_animFinal`.
    pub anim_final: Vec<u8>,
}

/// `class GenericObjectCreationNugget` (`CreateObject` when `name_are_objects`, `CreateDebris` when not).
#[derive(Clone, Debug, PartialEq)]
pub struct GenericObjectCreationNugget {
    /// `m_names`: thing template names (objects) or model names (debris).
    pub names: Vec<Vec<u8>>,
    /// `m_putInContainer`.
    pub put_in_container: Vec<u8>,
    /// `m_animSets`.
    pub anim_sets: Vec<AnimSet>,
    /// `m_fxFinal`.
    pub fx_final: FxListName,
    /// `m_particleSysName`.
    pub particle_sys_name: Vec<u8>,
    /// `m_debrisToGenerate`.
    pub debris_to_generate: i32,
    /// `m_mass`.
    pub mass: f32,
    /// `m_extraBounciness`.
    pub extra_bounciness: f32,
    /// `m_extraFriction`.
    pub extra_friction: f32,
    /// `m_offset`.
    pub offset: Coord3D,
    /// `m_disposition` (a `DebrisDisposition` bit set).
    pub disposition: u32,
    /// `m_dispositionIntensity`.
    pub disposition_intensity: f32,
    /// `m_spinRate`.
    pub spin_rate: f32,
    /// `m_yawRate`.
    pub yaw_rate: f32,
    /// `m_rollRate`.
    pub roll_rate: f32,
    /// `m_pitchRate`.
    pub pitch_rate: f32,
    /// `m_minMag`.
    pub min_mag: f32,
    /// `m_maxMag`.
    pub max_mag: f32,
    /// `m_minPitch`.
    pub min_pitch: f32,
    /// `m_maxPitch`.
    pub max_pitch: f32,
    /// `m_minFrames`.
    pub min_frames: u32,
    /// `m_maxFrames`.
    pub max_frames: u32,
    /// `m_shadowType`.
    pub shadow_type: u32,
    /// `m_minLODRequired`.
    pub min_lod_required: i32,
    /// `m_invulnerableTime`.
    pub invulnerable_time: u32,
    /// `m_minHealth`.
    pub min_health: f32,
    /// `m_maxHealth`.
    pub max_health: f32,
    /// `m_fadeFrames`.
    pub fade_frames: u32,
    /// `m_fadeSoundName`.
    pub fade_sound_name: Vec<u8>,
    /// `m_minDistanceAFormation`.
    pub min_distance_a_formation: f32,
    /// `m_minDistanceBFormation`.
    pub min_distance_b_formation: f32,
    /// `m_maxDistanceFormation`.
    pub max_distance_formation: f32,
    /// `m_objectCount`.
    pub object_count: i32,
    /// `m_bounceSound`, by event name.
    pub bounce_sound: Vec<u8>,
    /// `m_requiresLivePlayer`.
    pub requires_live_player: bool,
    /// `m_containInsideSourceObject`.
    pub contain_inside_source_object: bool,
    /// `m_preserveLayer`.
    pub preserve_layer: bool,
    /// `m_nameAreObjects`.
    pub name_are_objects: bool,
    /// `m_okToChangeModelColor`.
    pub ok_to_change_model_color: bool,
    /// `m_orientInForceDirection`.
    pub orient_in_force_direction: bool,
    /// `m_spreadFormation`.
    pub spread_formation: bool,
    /// `m_fadeIn`.
    pub fade_in: bool,
    /// `m_fadeOut`.
    pub fade_out: bool,
    /// `m_ignorePrimaryObstacle`.
    pub ignore_primary_obstacle: bool,
    /// `m_inheritsVeterancy`.
    pub inherits_veterancy: bool,
    /// `m_diesOnBadLand`.
    pub dies_on_bad_land: bool,
    /// `m_skipIfSignificantlyAirborne`.
    pub skip_if_significantly_airborne: bool,
}

impl Default for GenericObjectCreationNugget {
    /// `GenericObjectCreationNugget()`.
    fn default() -> Self {
        GenericObjectCreationNugget {
            names: Vec::new(),
            put_in_container: Vec::new(),
            anim_sets: Vec::new(),
            fx_final: None,
            particle_sys_name: Vec::new(),
            debris_to_generate: if plant(7) { 2 } else { 1 },
            mass: 0.0,
            extra_bounciness: 0.0,
            extra_friction: 0.0,
            offset: Coord3D::default(),
            disposition: if plant(8) { LIKE_EXISTING } else { ON_GROUND_ALIGNED },
            disposition_intensity: 0.0,
            spin_rate: if plant(9) { 0.0 } else { -1.0 },
            yaw_rate: -1.0,
            roll_rate: if plant(10) { 0.0 } else { -1.0 },
            pitch_rate: -1.0,
            min_mag: 0.0,
            max_mag: 0.0,
            min_pitch: 0.0,
            max_pitch: 0.0,
            min_frames: 0,
            max_frames: 0,
            shadow_type: 0,
            min_lod_required: if plant(13) { 1 } else { STATIC_GAME_LOD_LOW },
            invulnerable_time: 0,
            min_health: if plant(11) { 0.5 } else { 1.0 },
            max_health: 1.0,
            fade_frames: 0,
            fade_sound_name: Vec::new(),
            min_distance_a_formation: 0.0,
            min_distance_b_formation: 0.0,
            max_distance_formation: 0.0,
            object_count: 0,
            bounce_sound: Vec::new(),
            requires_live_player: false,
            contain_inside_source_object: false,
            preserve_layer: !plant(12),
            name_are_objects: true,
            ok_to_change_model_color: false,
            orient_in_force_direction: false,
            spread_formation: false,
            fade_in: false,
            fade_out: false,
            ignore_primary_obstacle: false,
            inherits_veterancy: false,
            dies_on_bad_land: false,
            skip_if_significantly_airborne: false,
        }
    }
}

/// `class ObjectCreationNugget` and its five implementations. The nuggets live in `Arc`s, one allocation each, so the variants'
/// different sizes cost nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Nugget {
    /// `FireWeaponNugget`.
    FireWeapon(FireWeaponNugget),
    /// `AttackNugget`.
    Attack(AttackNugget),
    /// `DeliverPayloadNugget`.
    DeliverPayload(DeliverPayloadNugget),
    /// `ApplyRandomForceNugget`.
    ApplyRandomForce(ApplyRandomForceNugget),
    /// `GenericObjectCreationNugget`.
    Generic(GenericObjectCreationNugget),
}

/// `atan2(y, x)` on `Real`s: the C `double` function, stored in a `Real`.
fn atan2_d(y: f32, x: f32) -> f32 {
    <f64 as petir::Real>::atan2(f64::from(y), f64::from(x)) as f32
}

/// `BitTest(x, i)`.
fn bit_test(x: u32, i: u32) -> bool {
    x & i != 0
}

impl FireWeaponNugget {
    /// `create(primaryObj, primary, secondary, angle, life)`.
    fn create_at<H: OclHost>(&self, host: &mut H, primary_obj: Option<ObjectId>, primary: Option<&Coord3D>, secondary: Option<&Coord3D>) -> OclResult<Option<ObjectId>> {
        let (Some(po), Some(_), Some(s)) = (primary_obj, primary, secondary) else {
            return Ok(None); // "You must have a primary and secondary source for this effect"
        };
        if let Some(w) = self.weapon {
            host.weapon_create_and_fire_temp(w, po, if plant(33) { primary.expect("checked above") } else { s });
        }
        Ok(None)
    }
}

impl AttackNugget {
    /// `create(primaryObj, primary, secondary, angle, life)`.
    fn create_at<H: OclHost>(&self, host: &mut H, primary_obj: Option<ObjectId>, primary: Option<&Coord3D>, secondary: Option<&Coord3D>) -> OclResult<Option<ObjectId>> {
        let (Some(po), Some(_), Some(s)) = (primary_obj, primary, secondary) else {
            return Ok(None);
        };
        if host.obj_has_ai(po) {
            // lock merely fires till the weapon is empty or the attack is "done"
            if !plant(34) {
                host.obj_set_weapon_lock(po, self.weapon_slot, LOCKED_TEMPORARILY);
            }
            host.ai_attack_position(po, s, if plant(35) { 1 } else { self.number_of_shots }, CommandSourceType::FromAi);
        }
        if host.obj_has_update_module(po, UpdateModuleKind::RadiusDecal) {
            host.radius_decal_create(po, &self.delivery_decal_template, self.delivery_decal_radius, s);
            if !plant(36) {
                host.radius_decal_kill_when_no_longer_attacking(po);
            }
        }
        Ok(None)
    }
}

impl DeliverPayloadNugget {
    /// `create(primaryObj, primary, secondary, createOwner, life)`.
    fn create<H: OclHost>(
        &self,
        host: &mut H,
        primary_obj: Option<ObjectId>,
        primary: Option<&Coord3D>,
        secondary: Option<&Coord3D>,
        create_owner: bool,
    ) -> OclResult<Option<ObjectId>> {
        let (Some(po), Some(primary), Some(secondary)) = (primary_obj, primary, secondary) else {
            return Ok(None);
        };
        let owner_player = host.obj_controlling_player(po).ok_or(OclError::Undefined("DeliverPayloadNugget: the primary object has no controlling player"))?;
        let owner = host.player_default_team(owner_player);

        // The relative positions of each member of the formation: the vector from the target to the lead plane, normalised, rotated 90
        // degrees (CW and CCW); adding the results to the initial vectors gives the delta positions.
        let (mut ccw_x, mut ccw_y, mut cw_x, mut cw_y) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        if self.formation_size > 1 {
            let mut dx = primary.x - secondary.x;
            let mut dy = primary.y - secondary.y;
            let length = (dx * dx + dy * dy).sqrt();
            dx /= length;
            dy /= length;
            if plant(37) {
                std::mem::swap(&mut dx, &mut dy);
            }
            let radians = if plant(38) { 45.0f32 * PI / 180.0f32 } else { 90.0f32 * PI / 180.0f32 };
            let mut s = sin(radians);
            let mut c = cos(radians);
            ccw_x = dx * c + dy * -s + dx;
            ccw_y = dx * s + dy * c + dy;
            if plant(39) {
                ccw_x -= dx;
            }
            s = sin(-radians);
            c = cos(-radians);
            cw_x = dx * c + dy * -s + dx;
            cw_y = dx * s + dy * c + dy;
            if plant(40) {
                cw_y = dx * s + dy * c;
            }
        }

        let mut first_transport: Option<ObjectId> = None;
        let mut formation_index: i32 = 0;
        while (formation_index as u32) < self.formation_size {
            let mut offset = Coord3D::default();
            // `Int offsetMultiplier = (formationIndex + 1) / 2 * m_formationSpacing;`: truncated
            let offset_multiplier: i32 = if plant(41) {
                ((((formation_index + 1) / 2) as f32 * self.formation_spacing) + 0.5) as i32
            } else if plant(42) {
                ((formation_index / 2) as f32 * self.formation_spacing) as i32
            } else {
                (((formation_index + 1) / 2) as f32 * self.formation_spacing) as i32
            };
            if (formation_index % 2 != 0) != plant(43) {
                offset.x = ccw_x * offset_multiplier as f32;
                offset.y = ccw_y * offset_multiplier as f32;
            } else {
                offset.x = cw_x * offset_multiplier as f32;
                offset.y = cw_y * offset_multiplier as f32;
            }

            let mut start_pos = *primary;
            let mut move_to_pos = *secondary;
            start_pos.x += offset.x;
            start_pos.y += offset.y;
            start_pos.z += offset.z;
            if !plant(44) {
                move_to_pos.x += offset.x;
                move_to_pos.y += offset.y;
                move_to_pos.z += offset.z;
            }

            let mut target_pos = *secondary;
            let conv = if plant(45) { self.convergence_factor } else { 1.0 - self.convergence_factor };
            target_pos.x += offset.x * conv;
            target_pos.y += if plant(46) { offset.x } else { offset.y } * conv;

            // the first guy in each formation is always spot-on
            if (self.error_radius > 1.0 || plant(47) && self.error_radius > 0.0) && (formation_index > 0 || plant(48)) {
                let random_radius = host.random_value_real(0.0, self.error_radius);
                let random_angle = host.random_value_real(0.0, if plant(49) { PI } else { PI * 2.0 });
                target_pos.x += random_radius * cos(random_angle);
                target_pos.y += random_radius * if plant(50) { cos(random_angle) } else { sin(random_angle) };
            }

            let orient = if plant(51) {
                atan2_d(move_to_pos.x - start_pos.x, move_to_pos.y - start_pos.y)
            } else {
                atan2_d(move_to_pos.y - start_pos.y, move_to_pos.x - start_pos.x)
            };
            if self.data.dist_to_target > 0.0 || plant(52) && self.data.dist_to_target < 0.0 {
                let slop: f32 = if plant(53) { 1.0 } else { 1.5 };
                start_pos.x -= cos(orient) * self.data.dist_to_target * slop;
                start_pos.y -= sin(orient) * self.data.dist_to_target * slop;
            }

            let transport: ObjectId;
            if create_owner {
                let ttn = host.find_template(&self.transport_name);
                let Some(t) = host.new_object(ttn, owner) else {
                    return Ok(None);
                };
                transport = t;
                if first_transport.is_none() || plant(67) {
                    first_transport = Some(t);
                }
                host.obj_set_position(t, &start_pos);
                host.obj_set_orientation(t, orient);
                if !plant(54) {
                    host.obj_set_producer(t, Some(po));
                }
                // this flag lets enemy players target it manually with weapons
                if !plant(55) {
                    host.obj_set_script_status(t, OBJECT_STATUS_SCRIPT_TARGETABLE);
                }
                if self.delay_delivery_frames_max > 0 {
                    let delay = host.random_value(0, self.delay_delivery_frames_max as i32);
                    let until = if plant(56) { delay as u32 } else { host.frame().wrapping_add(delay as u32) };
                    host.obj_set_disabled_until(t, DISABLED_DEFAULT, until);
                }
            } else {
                transport = po;
            }

            // notify special power tracking
            if host.obj_has_special_power_completion_die(transport) {
                host.special_power_completion_die_set_creator(transport, if (formation_index == 0) != plant(57) { po } else { INVALID_ID });
            }

            if host.obj_has_update_module(transport, UpdateModuleKind::DeliverPayloadAi) {
                if self.start_at_max_speed && (create_owner || plant(58)) && host.obj_has_physics(transport) {
                    let mut starting_force = host.obj_unit_direction_vector_2d(transport);
                    if !host.obj_has_body(transport) {
                        return Err(OclError::Undefined("DeliverPayloadNugget: StartAtMaxSpeed on a transport with physics and no body module"));
                    }
                    let damage_state = host.body_damage_state(transport);
                    let max_speed = host.deliver_payload_max_speed_for_condition(transport, damage_state);
                    let factor = if plant(59) { max_speed } else { max_speed * host.physics_mass(transport) };
                    starting_force.x *= factor;
                    starting_force.y *= factor;
                    starting_force.z *= factor;
                    host.physics_apply_motive_force(transport, &starting_force);
                }

                // only the first guy in each formation gets a delivery decal
                let mut data = self.data.clone();
                if (formation_index != 0) != plant(60) {
                    data.delivery_decal_radius = 0.0;
                }
                host.deliver_payload(transport, &move_to_pos, &target_pos, &data);
                if self.start_at_preferred_height && create_owner {
                    start_pos.z = host.terrain_ground_height(start_pos.x, start_pos.y) + if plant(61) { 0.0 } else { host.deliver_payload_preferred_height(transport) };
                    host.obj_set_position(transport, &start_pos);
                }

                let put_in_container_tmpl = if self.put_in_container_name.is_empty() { None } else { host.find_template(&self.put_in_container_name) };
                for it in &self.payload {
                    let Some(payload_tmpl) = host.find_template(&it.payload_name) else {
                        if plant(62) {
                            continue;
                        }
                        return Ok(None); // "couldn't create (template not found)"
                    };
                    for i in 0..it.payload_count + i32::from(plant(63)) {
                        let mut payload = host
                            .new_object(Some(payload_tmpl), owner)
                            .ok_or(OclError::Undefined("DeliverPayloadNugget: the payload could not be made and is written through"))?;
                        host.obj_set_position(payload, &start_pos);
                        host.obj_set_producer(payload, Some(transport));
                        if host.obj_has_special_power_completion_die(payload) {
                            host.special_power_completion_die_set_creator(payload, if formation_index == 0 && i == 0 { po } else { INVALID_ID });
                        }
                        if let Some(ct) = put_in_container_tmpl {
                            let container = host
                                .new_object(Some(ct), owner)
                                .ok_or(OclError::Undefined("DeliverPayloadNugget: the container could not be made and is written through"))?;
                            host.obj_set_position(container, &start_pos);
                            host.obj_set_producer(container, if plant(64) { Some(po) } else { Some(transport) });
                            if host.obj_has_special_power_completion_die(container) {
                                host.special_power_completion_die_set_creator(container, if formation_index == 0 && i == 0 { po } else { INVALID_ID });
                            }
                            if host.obj_has_contain(container) && (host.contain_is_valid_container_for(container, payload, true) || plant(65)) {
                                host.contain_add(container, payload);
                                payload = container;
                            }
                            // else: "PutInContainer is full, or not valid for the payload"
                        }
                        if host.obj_has_contain(transport) && (host.contain_is_valid_container_for(transport, payload, true) || plant(66)) {
                            host.contain_add(transport, payload);
                        }
                        // else: "transport is full, or not valid for the payload"
                    }
                }
            }
            // else: "You should really have a DeliverPayloadAIUpdate here"
            formation_index += 1;
        }
        Ok(first_transport)
    }
}

impl ApplyRandomForceNugget {
    /// `create(primary, secondary, life)`: a random kick and spin on the primary object's physics.
    fn create_object<H: OclHost>(&self, host: &mut H, primary: Option<ObjectId>) -> OclResult<Option<ObjectId>> {
        if let Some(p) = primary {
            if host.obj_has_physics(p) {
                let force = calc_random_force(host, self.min_mag, self.max_mag, self.min_pitch, self.max_pitch);
                host.physics_apply_force(p, &force);
                let yaw = host.random_value_real(-self.spin_rate, self.spin_rate);
                let (roll, pitch);
                if plant(69) {
                    pitch = host.random_value_real(-self.spin_rate, self.spin_rate);
                    roll = host.random_value_real(-self.spin_rate, self.spin_rate);
                } else {
                    roll = host.random_value_real(-self.spin_rate, self.spin_rate);
                    pitch = host.random_value_real(-self.spin_rate, self.spin_rate);
                }
                host.physics_set_yaw_rate(p, yaw);
                host.physics_set_roll_rate(p, roll);
                host.physics_set_pitch_rate(p, pitch);
            }
            // else: "You must have a Physics module source for this effect"
        }
        // else: "You must have a primary source for this effect"
        Ok(None)
    }
}

impl GenericObjectCreationNugget {
    /// `create(primary, secondary, life)`.
    fn create_object<H: OclHost>(&self, host: &mut H, primary: Option<ObjectId>, life: u32) -> OclResult<Option<ObjectId>> {
        let Some(p) = primary else {
            return Ok(None);
        };
        if self.skip_if_significantly_airborne && (host.obj_is_significantly_above_terrain(p) != plant(70)) {
            return Ok(None);
        }
        let pos = host.obj_position(p);
        let mtx = host.obj_transform(p);
        let orientation = host.obj_orientation(p);
        self.really_create(host, &pos, Some(&mtx), orientation, Some(p), life)
    }

    /// `create(primaryObj, primary, secondary, angle, life)`.
    fn create_at<H: OclHost>(&self, host: &mut H, primary_obj: Option<ObjectId>, primary: Option<&Coord3D>, angle: f32, life: u32) -> OclResult<Option<ObjectId>> {
        let Some(pos) = primary else {
            return Ok(None);
        };
        // the vast majority of OCLs don't care about the angle, so if it comes in invalid, default it to 0
        let angle = if angle == INVALID_ANGLE && !plant(71) { 0.0 } else { angle };
        self.really_create(host, pos, if plant(72) { primary_obj.map(|o| host.obj_transform(o)) } else { None }.as_ref(), angle, primary_obj, life)
    }

    /// `doStuffToObj(obj, modelName, pos, mtx, orientation, sourceObj, lifetimeFrames)`.
    #[allow(clippy::too_many_arguments)]
    fn do_stuff_to_obj<H: OclHost>(
        &self,
        host: &mut H,
        obj: ObjectId,
        model_name: &[u8],
        pos: &Coord3D,
        mtx: Option<&Matrix3D>,
        mut orientation: f32,
        source: Option<ObjectId>,
        lifetime_frames: u32,
    ) -> OclResult<()> {
        host.obj_set_producer(obj, source);

        if host.obj_has_update_module(obj, UpdateModuleKind::Lifetime) {
            if lifetime_frames != 0 && !plant(73) {
                // passed in override: use this value for a specific lifetime
                host.lifetime_set_range(obj, lifetime_frames, lifetime_frames);
            } else if self.max_frames > 0 || plant(74) && self.min_frames > 0 {
                // both are zero if no lifetime was specified in the OCL (it could be in the Object): the OCL overrides only if it has one
                host.lifetime_set_range(obj, if plant(75) { self.max_frames } else { self.min_frames }, self.max_frames);
            }
        }

        if !self.name_are_objects {
            if !host.obj_has_drawable(obj) {
                return Err(OclError::Undefined("CreateDebris: the object has no drawable"));
            }
            for i in 0..host.drawable_debris_interface_count(obj) {
                let color = if self.ok_to_change_model_color != plant(76) { host.obj_indicator_color(obj) } else { 0 };
                host.debris_set_model_name(obj, i, model_name, color, self.shadow_type);
                if !self.anim_sets.is_empty() {
                    let which = host.random_value(0, self.anim_sets.len() as i32 - i32::from(!plant(77))) as usize % self.anim_sets.len();
                    let a = &self.anim_sets[which];
                    host.debris_set_anim_names(obj, i, &a.anim_initial, &a.anim_flying, &a.anim_final, &self.fx_final);
                }
            }
        }

        let mut offset = self.offset;
        adjust_vector(&mut offset, mtx);

        let mut chunk_pos = Coord3D { x: pos.x + offset.x, y: pos.y + offset.y, z: if plant(78) { pos.z } else { pos.z + offset.z } };

        if !self.particle_sys_name.is_empty() && !plant(79) {
            if let Some(t) = host.particle_system_find_template(&self.particle_sys_name) {
                host.particle_system_create_attached(t, obj);
            }
        }

        if self.ignore_primary_obstacle != plant(80) && host.obj_has_physics(obj) {
            host.physics_set_ignore_collisions_with(obj, source);
        }

        // set its beginning health
        let health_percent = if plant(82) { self.max_health } else { host.random_value_real(self.min_health, self.max_health) };
        if host.obj_has_body(obj) {
            host.body_set_initial_health(obj, if plant(81) { (health_percent * 100.0 + 0.5) as i32 } else { (health_percent * 100.0) as i32 });
        }

        // If they have a SlavedUpdate, then I have to tell them who their daddy is from now on.
        for i in 0..host.behavior_module_count(obj) {
            if host.behavior_module_has_slaved_update(obj, i) {
                host.behavior_module_on_enslave(obj, i, source);
                if !plant(83) {
                    break;
                }
            }
        }

        if self.inherits_veterancy {
            if let Some(s) = source {
                if host.obj_is_trainable(obj) || plant(84) {
                    let v = host.obj_veterancy_level(s);
                    host.obj_set_veterancy_level(obj, v);
                    // the unit name goes to the ejected thing, so the designer can control the pilot with the scripts
                    let name = host.obj_name(s);
                    if !plant(85) {
                        host.transfer_object_name(&name, obj);
                    }
                }
            }
        }

        if self.invulnerable_time > 0 || plant(86) {
            host.obj_go_invulnerable(obj, self.invulnerable_time);
        }

        if bit_test(self.disposition, INHERIT_VELOCITY) {
            if let Some(s) = source {
                if host.obj_has_physics(s) && host.obj_has_physics(obj) {
                    let v = host.physics_velocity(s);
                    host.physics_apply_force(obj, &if plant(87) { Coord3D { x: v.x, y: v.y, z: 0.0 } } else { v });
                }
            }
        }

        if bit_test(self.disposition, LIKE_EXISTING) {
            match mtx {
                Some(m) if !plant(88) => host.obj_set_transform_matrix(obj, m),
                _ => host.obj_set_orientation(obj, orientation),
            }
            host.obj_set_position(obj, &chunk_pos);
            if let Some(s) = source {
                if (host.obj_is_above_terrain(s) != plant(89)) && host.obj_has_physics(obj) {
                    host.physics_set_allow_to_fall(obj, true);
                }
            }
            // Since the sneak attack is a structure created with an OCL, it bypasses dozerAI::build: stamp it into the pathfind map here.
            if host.obj_is_structure(obj) {
                // flatten the terrain underneath, then adjust to the flattened height; add to the map AFTER flattening
                host.terrain_flatten(obj);
                let mut adjusted_pos = host.obj_position(obj);
                adjusted_pos.z = if plant(90) { host.terrain_ground_height(adjusted_pos.x, adjusted_pos.y) } else { host.terrain_ground_height(pos.x, pos.y) };
                host.obj_set_position(obj, &adjusted_pos);
                host.pathfinder_add_object(obj);
            }
        }

        if bit_test(self.disposition, ON_GROUND_ALIGNED) {
            chunk_pos.z = if plant(92) { 0.0 } else { 99999.0 };
            let layer = host.terrain_highest_layer_for_destination(&chunk_pos);
            let angle = host.random_value_real(0.0, 2.0 * PI);
            host.obj_set_orientation(obj, angle);
            chunk_pos.z = host.terrain_layer_height(chunk_pos.x, chunk_pos.y, layer);
            // ensure we are slightly above the bridge, to account for fudge and sloppy art
            if layer != LAYER_GROUND || plant(91) {
                chunk_pos.z += 1.0;
            }
            host.obj_set_layer(obj, layer);
            host.obj_set_position(obj, &chunk_pos);
        }

        if bit_test(self.disposition, SEND_IT_OUT) {
            let angle = host.random_value_real(0.0, 2.0 * PI);
            host.obj_set_orientation(obj, angle);
            chunk_pos.z = host.terrain_ground_height(chunk_pos.x, chunk_pos.y);
            host.obj_set_position(obj, &chunk_pos);
            if host.obj_has_physics(obj) {
                if !self.name_are_objects || plant(93) {
                    host.physics_set_mass(obj, self.mass);
                }
                host.physics_set_extra_friction(obj, self.extra_friction);

                let horiz_force = if plant(94) { 3.0 } else { 4.0 } * self.disposition_intensity;
                let x = host.random_value_real(-horiz_force, horiz_force);
                let y = host.random_value_real(-horiz_force, horiz_force);
                let force = Coord3D { x, y, z: 0.0 };

                host.physics_apply_force(obj, &force);
                if self.orient_in_force_direction && !plant(95) {
                    orientation = atan2_d(force.y, force.x);
                }
            }
        }

        if bit_test(self.disposition, SEND_IT_FLYING | SEND_IT_UP | RANDOM_FORCE) {
            if let Some(m) = mtx {
                if !plant(96) {
                    host.obj_set_transform_matrix(obj, m);
                }
            }
            host.obj_set_position(obj, &chunk_pos);
            if host.obj_has_physics(obj) {
                if !self.name_are_objects {
                    host.physics_set_mass(obj, self.mass);
                }
                host.physics_set_extra_bounciness(obj, if plant(97) { self.extra_friction } else { self.extra_bounciness });
                host.physics_set_extra_friction(obj, self.extra_friction);
                host.physics_set_allow_bouncing(obj, !plant(98));
                host.physics_set_bounce_sound(obj, &self.bounce_sound);

                // if omitted from INI, calc it based on intensity
                let spin_rate = if self.spin_rate >= 0.0 && !plant(99) { self.spin_rate } else if plant(100) { (PI / 16.0) * self.disposition_intensity } else { (PI / 32.0) * self.disposition_intensity };

                // treat these as overrides
                let yaw_rate = if self.yaw_rate >= 0.0 { self.yaw_rate } else { spin_rate };
                let roll_rate = if self.roll_rate > 0.0 || !plant(101) && self.roll_rate == 0.0 { self.roll_rate } else { spin_rate };
                let pitch_rate = if self.pitch_rate >= 0.0 { self.pitch_rate } else if plant(102) { self.yaw_rate.max(0.0) } else { spin_rate };

                let yaw = host.random_value_real(-yaw_rate, yaw_rate);
                let roll = host.random_value_real(-roll_rate, roll_rate);
                let pitch = host.random_value_real(-pitch_rate, pitch_rate);

                let force;
                if bit_test(self.disposition, SEND_IT_FLYING) {
                    let horiz_force = 4.0 * self.disposition_intensity;
                    let vert_force = if plant(103) { 4.0 } else { 3.0 } * self.disposition_intensity;
                    let x = host.random_value_real(-horiz_force, horiz_force);
                    let y = host.random_value_real(-horiz_force, horiz_force);
                    let z = host.random_value_real(vert_force * if plant(104) { 0.5 } else { 0.33 }, vert_force);
                    force = Coord3D { x, y, z };
                } else if bit_test(self.disposition, SEND_IT_UP) {
                    let horiz_force = if plant(105) { 4.0 } else { 2.0 } * self.disposition_intensity;
                    let vert_force = 4.0 * self.disposition_intensity;
                    let x = host.random_value_real(-horiz_force, horiz_force);
                    let y = host.random_value_real(-horiz_force, horiz_force);
                    let z = host.random_value_real(vert_force * if plant(106) { 0.5 } else { 0.75 }, vert_force);
                    force = Coord3D { x, y, z };
                } else {
                    force = calc_random_force(host, self.min_mag, self.max_mag, self.min_pitch, self.max_pitch);
                }
                host.physics_apply_force(obj, &force);
                if self.orient_in_force_direction {
                    orientation = if plant(107) { atan2_d(force.x, force.y) } else { atan2_d(force.y, force.x) };
                }
                if plant(108) {
                    host.physics_set_angles(obj, 0.0, orientation, 0.0);
                } else {
                    host.physics_set_angles(obj, orientation, 0.0, 0.0);
                }
                host.physics_set_yaw_rate(obj, yaw);
                host.physics_set_roll_rate(obj, roll);
                host.physics_set_pitch_rate(obj, pitch);
            }
        }

        if bit_test(self.disposition, WHIRLING) && host.obj_has_physics(obj) {
            let yaw = host.random_value_real(-self.disposition_intensity, self.disposition_intensity);
            let roll = host.random_value_real(-self.disposition_intensity, self.disposition_intensity);
            let pitch = if plant(109) { yaw } else { host.random_value_real(-self.disposition_intensity, self.disposition_intensity) };
            host.physics_set_yaw_rate(obj, yaw);
            host.physics_set_roll_rate(obj, roll);
            host.physics_set_pitch_rate(obj, pitch);
        }

        if bit_test(self.disposition, FLOATING) && (host.obj_has_update_module(obj, UpdateModuleKind::Float) || plant(110)) {
            host.float_set_enabled(obj);
        }

        if self.contain_inside_source_object {
            // The Obj has been totally made, so stuff it inside ourselves if desired.
            let s = source.ok_or(OclError::Undefined("ContainInsideSourceObject without a source object"))?;
            if host.obj_has_contain(s) && host.contain_is_valid_container_for(s, obj, true) {
                host.contain_add(s, obj);
                // need to hide if they are hidden
                if host.obj_has_drawable(s) && host.obj_has_drawable(obj) && (host.drawable_is_effectively_hidden(s) != plant(111)) {
                    host.drawable_set_hidden(obj, true);
                }
            } else if !plant(112) {
                // If we fail to contain it, we can't just leave it. Stillborn it.
                host.destroy_object(obj);
            }
        }

        if self.dies_on_bad_land {
            // if we land in the water, we die
            let rider_pos = host.obj_position(obj);
            if let Some((water_z, _terrain_z)) = host.terrain_is_underwater(rider_pos.x, rider_pos.y) {
                if rider_pos.z <= water_z + if plant(113) { 5.0 } else { 10.0 } && (host.obj_layer(obj) == LAYER_GROUND || plant(114)) {
                    // don't call kill(); do it manually, so we can specify DEATH_FLOODED
                    let mut damage_info = DamageInfo::default();
                    damage_info.input.damage_type = DAMAGE_WATER as DamageType; // instead of UNRESISTABLE, so there is no dusty damage effect
                    damage_info.input.death_type = if plant(115) { 0 } else { DEATH_FLOODED };
                    damage_info.input.source_id = INVALID_ID;
                    damage_info.input.amount = HUGE_DAMAGE_AMOUNT;
                    host.obj_attempt_damage(obj, &damage_info);
                }
            }

            // Kill if materialized on impassable ground
            let pos_now = host.obj_position(obj);
            let cell_x = real_to_int(pos_now.x / PATHFIND_CELL_SIZE as f32);
            let cell_y = real_to_int(pos_now.y / if plant(116) { 8.0 } else { PATHFIND_CELL_SIZE as f32 });
            let layer = host.obj_layer(obj);
            let cell_type = host.pathfinder_cell_type(layer, cell_x, cell_y).unwrap_or(if plant(119) { PathfindCellType::Clear } else { PathfindCellType::Impassable });

            // If we land outside the map, we die too. Otherwise we exist outside the PartitionManager like a cheater.
            if host.obj_is_off_map(obj) || matches!(cell_type, PathfindCellType::Cliff | PathfindCellType::Impassable) || !plant(117) && cell_type == PathfindCellType::Water || plant(118) && cell_type == PathfindCellType::Obstacle {
                host.obj_kill(obj);
            }
        }
        Ok(())
    }

    /// `reallyCreate(pos, mtx, orientation, sourceObj, lifetimeFrames)`.
    fn really_create<H: OclHost>(
        &self,
        host: &mut H,
        pos: &Coord3D,
        mtx: Option<&Matrix3D>,
        orientation: f32,
        source: Option<ObjectId>,
        life: u32,
    ) -> OclResult<Option<ObjectId>> {
        let debris_template = host.debris_template();

        if self.names.is_empty() {
            return Ok(None);
        }

        if self.requires_live_player {
            let live = source.and_then(|s| host.obj_controlling_player(s)).is_some_and(|p| host.player_is_active(p) || plant(120));
            if !live && !plant(121) {
                return Ok(None); // don't spawn useful objects for dead players
            }
        }

        // Object type debris might need this information to process visual UpgradeModules.
        let mut debris_owner = host.neutral_player_default_team();
        if let Some(p) = source.and_then(|s| host.obj_controlling_player(s)).filter(|_| !plant(122)) {
            debris_owner = host.player_default_team(p);
        }

        let mut container: Option<ObjectId> = None;
        let mut first_object: Option<ObjectId> = None;
        if !self.put_in_container.is_empty() {
            if let Some(container_tmpl) = host.find_template(&self.put_in_container) {
                let Some(c) = host.new_object(Some(container_tmpl), debris_owner) else {
                    return Ok(first_object);
                };
                container = Some(c);
                first_object = Some(c);
                if !plant(123) {
                    host.obj_set_producer(c, source);
                }
            }
        }

        for _nn in 0..self.debris_to_generate {
            let pick = host.random_value(0, self.names.len() as i32 - i32::from(!plant(124))) as usize % self.names.len();

            let tmpl = if self.name_are_objects {
                host.find_template(&self.names[pick])
            } else {
                // this is using the generic debris type so it's probably safe to remove if requested by the GameLOD manager
                if host.is_debris_skipped() != plant(125) {
                    continue;
                }
                debris_template
            };
            let Some(tmpl) = tmpl else {
                continue;
            };

            let Some(debris) = host.new_object(Some(tmpl), debris_owner) else {
                return Ok(first_object);
            };
            if first_object.is_none() || plant(126) {
                first_object = Some(debris);
            }
            host.obj_set_producer(debris, source);
            if self.preserve_layer && (container.is_none() || plant(127)) {
                if let Some(s) = source {
                    let layer = host.obj_layer(s);
                    if layer != LAYER_GROUND {
                        host.obj_set_layer(debris, layer);
                    }
                }
            }

            if let Some(c) = container {
                if host.obj_has_contain(c) && host.contain_is_valid_container_for(c, debris, true) {
                    host.contain_add(c, debris);
                }
            }

            // if we want the objects being created to appear in a spread formation
            if self.spread_formation {
                let mut fp_options = FindPositionOptions {
                    min_radius: if plant(128) { host.random_value_real(self.min_distance_b_formation, self.min_distance_a_formation) } else { host.random_value_real(self.min_distance_a_formation, self.min_distance_b_formation) },
                    ..FindPositionOptions::default()
                };
                fp_options.max_radius = self.max_distance_formation;
                fp_options.flags = if plant(129) { 0 } else { FPF_USE_HIGHEST_LAYER };
                let result_pos = host.partition_find_position_around(pos, &fp_options);
                self.do_stuff_to_obj(host, debris, &self.names[pick], &result_pos, mtx, orientation, source, life)?;
            } else {
                // do stuff to contained objects too
                self.do_stuff_to_obj(host, debris, &self.names[pick], pos, mtx, orientation, source, life)?;
            }

            if self.fade_in != plant(130) {
                let s = source.ok_or(OclError::Undefined("a faded object without a source object"))?;
                host.audio_add_event(&self.fade_sound_name, s);
                if !host.obj_has_drawable(debris) {
                    return Err(OclError::Undefined("a faded object has no drawable"));
                }
                host.drawable_fade_in(debris, if plant(131) { self.fade_frames + 1 } else { self.fade_frames });
            }

            if self.fade_out && !plant(132) {
                let s = source.ok_or(OclError::Undefined("a faded object without a source object"))?;
                host.audio_add_event(&self.fade_sound_name, s);
                if !host.obj_has_drawable(debris) {
                    return Err(OclError::Undefined("a faded object has no drawable"));
                }
                host.drawable_fade_out(debris, self.fade_frames);
            }
        }

        if let Some(c) = container.filter(|_| !plant(133)) {
            self.do_stuff_to_obj(host, c, b"", pos, mtx, orientation, source, life)?;
        }

        Ok(first_object)
    }
}

impl Nugget {
    /// `create(primary, secondary, lifetimeFrames)`: the object-based version; by default (`FireWeapon`, `Attack`, `DeliverPayload`) the
    /// location-based one with the two objects' positions and `INVALID_ANGLE`.
    pub fn create_object<H: OclHost>(&self, host: &mut H, primary: Option<ObjectId>, secondary: Option<ObjectId>, life: u32) -> OclResult<Option<ObjectId>> {
        match self {
            Nugget::ApplyRandomForce(n) => n.create_object(host, primary),
            Nugget::Generic(n) => n.create_object(host, primary, life),
            _ => {
                let p = primary.map(|o| host.obj_position(o));
                let s = secondary.map(|o| host.obj_position(o));
                self.create_at(host, primary, p.as_ref(), s.as_ref(), INVALID_ANGLE, life)
            }
        }
    }

    /// `create(primaryObj, primary, secondary, createOwner, lifetimeFrames)`: only `DeliverPayload` cares about `createOwner`; the base class
    /// calls the angle overload with `INVALID_ANGLE`.
    pub fn create_with_owner<H: OclHost>(
        &self,
        host: &mut H,
        primary_obj: Option<ObjectId>,
        primary: Option<&Coord3D>,
        secondary: Option<&Coord3D>,
        create_owner: bool,
        life: u32,
    ) -> OclResult<Option<ObjectId>> {
        match self {
            Nugget::DeliverPayload(n) => n.create(host, primary_obj, primary, secondary, create_owner),
            _ => self.create_at(host, primary_obj, primary, secondary, INVALID_ANGLE, life),
        }
    }

    /// `create(primaryObj, primary, secondary, angle, lifetimeFrames)`.
    pub fn create_at<H: OclHost>(
        &self,
        host: &mut H,
        primary_obj: Option<ObjectId>,
        primary: Option<&Coord3D>,
        secondary: Option<&Coord3D>,
        angle: f32,
        life: u32,
    ) -> OclResult<Option<ObjectId>> {
        match self {
            Nugget::FireWeapon(n) => n.create_at(host, primary_obj, primary, secondary),
            Nugget::Attack(n) => n.create_at(host, primary_obj, primary, secondary),
            Nugget::DeliverPayload(n) => n.create(host, primary_obj, primary, secondary, true),
            // "You must call this effect with an object, not a location"
            Nugget::ApplyRandomForce(_) => Ok(None),
            Nugget::Generic(n) => n.create_at(host, primary_obj, primary, angle, life),
        }
    }
}

// -------------------------------------------------------------------------------------------------------------------------------
// the list and the store
// -------------------------------------------------------------------------------------------------------------------------------

/// `class ObjectCreationList`: a list of the store's nuggets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectCreationList {
    /// `m_nuggets`: "this list doesn't own the nuggets; all nuggets are owned by the Store".
    pub nuggets: Vec<Arc<Nugget>>,
}

impl ObjectCreationList {
    /// `clear()`: toss the contents (the nuggets stay in the store).
    pub fn clear(&mut self) {
        self.nuggets.clear();
    }

    /// `createInternal(primaryObj, primary, secondary, createOwner, life)` (through the static `create(ocl, ...)`, which has the
    /// `NULL`-list check: here the list exists). All OCLs return the first object that is created (`None` if not applicable).
    pub fn create_with_owner<H: OclHost>(
        &self,
        host: &mut H,
        primary_obj: Option<ObjectId>,
        primary: Option<&Coord3D>,
        secondary: Option<&Coord3D>,
        create_owner: bool,
        life: u32,
    ) -> OclResult<Option<ObjectId>> {
        let mut first = None;
        for n in &self.nuggets {
            let cur = n.create_with_owner(host, primary_obj, primary, secondary, create_owner, life)?;
            if first.is_none() || plant(134) && cur.is_some() {
                first = cur;
            }
        }
        Ok(first)
    }

    /// `createInternal(primaryObj, primary, secondary, angle, life)`.
    pub fn create_at<H: OclHost>(
        &self,
        host: &mut H,
        primary_obj: Option<ObjectId>,
        primary: Option<&Coord3D>,
        secondary: Option<&Coord3D>,
        angle: f32,
        life: u32,
    ) -> OclResult<Option<ObjectId>> {
        let mut first = None;
        for n in &self.nuggets {
            let cur = n.create_at(host, primary_obj, primary, secondary, angle, life)?;
            if first.is_none() && !plant(135) {
                first = cur;
            }
        }
        Ok(first)
    }

    /// `createInternal(primary, secondary, life)`.
    pub fn create_object<H: OclHost>(&self, host: &mut H, primary: Option<ObjectId>, secondary: Option<ObjectId>, life: u32) -> OclResult<Option<ObjectId>> {
        let mut first = None;
        for n in &self.nuggets {
            let cur = n.create_object(host, primary, secondary, life)?;
            if first.is_none() || plant(136) {
                first = cur;
            }
        }
        Ok(first)
    }
}

/// What the parsers look up: `TheGlobalData->m_preloadAssets`, and the weapon store (`INI::parseWeaponTemplate`). An FX list is its name
/// (see [`DeliverPayloadDataParse::find_fx_list`]).
#[derive(Clone, Debug, Default)]
pub struct OclParseEnv {
    /// `TheGlobalData->m_preloadAssets`.
    pub preload_assets: bool,
    /// The weapon templates by name (`TheWeaponStore->findWeaponTemplate`).
    pub weapon_templates: BTreeMap<Vec<u8>, WeaponTemplateRef>,
}

impl OclParseEnv {
    /// `TheWeaponStore->findWeaponTemplate(name)`.
    fn find_weapon_template(&self, name: &[u8]) -> Option<WeaponTemplateRef> {
        if plant(26) {
            return self.weapon_templates.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| *v);
        }
        self.weapon_templates.get(name).copied()
    }
    /// `TheFXListStore->findFXList(name)`: `None` for `NULL` ("None" is not a list).
    fn find_fx_list(&self, name: &[u8]) -> FxListName {
        (!name.eq_ignore_ascii_case(b"None")).then(|| name.to_vec())
    }
}

/// The target of one nugget's parse: the nugget being filled, the lookups, and what the parsers add to the store's globals.
struct NuggetParse<N> {
    nugget: N,
    env: Arc<OclParseEnv>,
    debris_hack: Vec<Vec<u8>>,
}

impl<N> NuggetParse<N> {
    fn new(nugget: N, env: &Arc<OclParseEnv>) -> Self {
        NuggetParse { nugget, env: env.clone(), debris_hack: Vec::new() }
    }
}

impl DeliverPayloadDataParse for NuggetParse<DeliverPayloadNugget> {
    fn deliver_payload_data_mut(&mut self) -> &mut DeliverPayloadData {
        &mut self.nugget.data
    }
    fn find_weapon_template(&self, name: &[u8]) -> Option<WeaponTemplateRef> {
        self.env.find_weapon_template(name)
    }
    fn find_fx_list(&self, name: &[u8]) -> FxListName {
        self.env.find_fx_list(name)
    }
}

/// The target of an `ObjectCreationList` block: the nuggets completed so far, and the debris names kept for preloading.
struct OclParse {
    env: Arc<OclParseEnv>,
    nuggets: Vec<Arc<Nugget>>,
    debris_hack: Vec<Vec<u8>>,
}

type FireWeaponParse = NuggetParse<FireWeaponNugget>;
type AttackParse = NuggetParse<AttackNugget>;
type DeliverParse = NuggetParse<DeliverPayloadNugget>;
type ForceParse = NuggetParse<ApplyRandomForceNugget>;
type GenericParse = NuggetParse<GenericObjectCreationNugget>;

/// `FireWeaponNugget::parse`'s `myFieldParse`.
static FIRE_WEAPON_FIELDS: [FieldParse<FireWeaponParse>; 2] = [
    FieldParse::new(
        "Weapon",
        |ini, p, _| {
            let token = ini.get_next_token(None)?;
            p.nugget.weapon = p.env.find_weapon_template(&token);
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

/// `AttackNugget::parse`'s `myFieldParse`.
static ATTACK_FIELDS: [FieldParse<AttackParse>; 5] = [
    FieldParse::new("NumberOfShots", |ini, p, _| { p.nugget.number_of_shots = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "WeaponSlot",
        |ini, p, _| { p.nugget.weapon_slot = ini.parse_lookup_list(&WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST)?; Ok(()) },
        UserData::Lookup(&WEAPON_SLOT_TYPE_NAMES_LOOKUP_LIST),
    ),
    FieldParse::new("DeliveryDecal", |ini, p, _| RadiusDecalTemplate::parse_radius_decal_template(ini, &mut p.nugget.delivery_decal_template), UserData::None),
    FieldParse::new("DeliveryDecalRadius", |ini, p, _| { p.nugget.delivery_decal_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `DeliverPayloadNugget::parsePayload`: `Payload = <template> [<count>]`.
fn parse_payload(ini: &mut Ini<'_>, p: &mut DeliverParse, _: &UserData) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let count_str = ini.get_next_token_or_null(None);
    let count = match count_str {
        Some(c) => Ini::scan_int(&c)?,
        None => i32::from(!plant(23)),
    };
    p.nugget.payload.push(Payload { payload_name: name, payload_count: count });
    Ok(())
}

/// `DeliverPayloadNugget::parse`'s `myFieldParse` (the data's fields come after it, from [`DeliverPayloadData::field_parse`]).
static DELIVER_PAYLOAD_FIELDS: [FieldParse<DeliverParse>; 11] = [
    FieldParse::new("Transport", |ini, p, _| { p.nugget.transport_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("StartAtPreferredHeight", |ini, p, _| { p.nugget.start_at_preferred_height = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("StartAtMaxSpeed", |ini, p, _| { p.nugget.start_at_max_speed = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FormationSize", |ini, p, _| { p.nugget.formation_size = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FormationSpacing", |ini, p, _| { p.nugget.formation_spacing = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("WeaponConvergenceFactor", |ini, p, _| { p.nugget.convergence_factor = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("WeaponErrorRadius", |ini, p, _| { p.nugget.error_radius = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DelayDeliveryMax", |ini, p, _| { p.nugget.delay_delivery_frames_max = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("Payload", parse_payload, UserData::None),
    FieldParse::new("PutInContainer", |ini, p, _| { p.nugget.put_in_container_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `ApplyRandomForceNugget::parse`'s `myFieldParse`.
static APPLY_RANDOM_FORCE_FIELDS: [FieldParse<ForceParse>; 6] = [
    FieldParse::new("SpinRate", |ini, p, _| { p.nugget.spin_rate = ini.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinForceMagnitude", |ini, p, _| { p.nugget.min_mag = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxForceMagnitude", |ini, p, _| { p.nugget.max_mag = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinForcePitch", |ini, p, _| { p.nugget.min_pitch = ini.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxForcePitch", |ini, p, _| { p.nugget.max_pitch = ini.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `parseFrictionPerSec`: a per-second friction as a per-frame one.
fn parse_friction_per_sec(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let fric_per_sec = Ini::scan_real(&ini.get_next_token(None)?)?;
    p.nugget.extra_friction = if plant(22) { fric_per_sec } else { fric_per_sec * SECONDS_PER_LOGICFRAME_REAL };
    Ok(())
}

/// `GenericObjectCreationNugget::parseDebrisObjectNames`: every other token is kept (see the module docs).
fn parse_debris_object_names(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let mut debris_name = Some(ini.get_next_token(None)?);
    while let Some(name) = debris_name {
        if p.env.preload_assets != plant(16) {
            p.debris_hack.push(name.clone());
        }
        p.nugget.names.push(name);
        if plant(14) {
            // keep every token
            debris_name = ini.get_next_token_or_null(None);
            continue;
        }
        let _skipped = ini.get_next_token_or_null(None);
        debris_name = ini.get_next_token_or_null(None);
        if plant(15) {
            // keep the other half: the first token is dropped, not the second
            if let Some(n) = p.nugget.names.pop() {
                p.nugget.names.push(debris_name.clone().unwrap_or(n));
            }
        }
    }
    Ok(())
}

/// `GenericObjectCreationNugget::parseAnimSet`: `AnimationSet = <initial> <flying> <final>`.
fn parse_anim_set(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let anim_initial = ini.get_next_ascii_string();
    let anim_flying = ini.get_next_ascii_string();
    let anim_final = ini.get_next_ascii_string();
    p.nugget.anim_sets.push(AnimSet { anim_initial, anim_flying, anim_final });
    Ok(())
}

/// `INI::parseStaticGameLODLevel`.
fn parse_static_game_lod_level(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let tok = ini.get_next_token(None)?;
    for (i, name) in STATIC_GAME_LOD_NAMES.iter().enumerate() {
        if if plant(27) { tok == name.as_bytes() } else { tok.eq_ignore_ascii_case(name.as_bytes()) } {
            p.nugget.min_lod_required = i as i32;
            return Ok(());
        }
    }
    Err(crate::common::ini::IniError::new(crate::common::ini::IniErrorKind::InvalidData))
}

/// `INI::parseAudioEventRTS` on `m_bounceSound`: `NoSound` leaves the event as it is, any other token names it.
fn parse_bounce_sound(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    if !token.eq_ignore_ascii_case(b"NoSound") {
        p.nugget.bounce_sound = token;
    } else if plant(28) {
        p.nugget.bounce_sound.clear();
    }
    Ok(())
}

/// `INI::parseFXList` on `m_fxFinal`.
fn parse_fx_final(ini: &mut Ini<'_>, p: &mut GenericParse, _: &UserData) -> IniResult<()> {
    let token = ini.get_next_token(None)?;
    p.nugget.fx_final = if plant(29) { Some(token) } else { p.env.find_fx_list(&token) };
    Ok(())
}

/// `GenericObjectCreationNugget::getCommonFieldParse()`.
static GENERIC_COMMON_FIELDS: [FieldParse<GenericParse>; 31] = [
    FieldParse::new("PutInContainer", |ini, p, _| { p.nugget.put_in_container = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("ParticleSystem", |ini, p, _| { p.nugget.particle_sys_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("Count", |ini, p, _| { p.nugget.debris_to_generate = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("IgnorePrimaryObstacle", |ini, p, _| { p.nugget.ignore_primary_obstacle = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("OrientInForceDirection", |ini, p, _| { p.nugget.orient_in_force_direction = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ExtraBounciness", |ini, p, _| { p.nugget.extra_bounciness = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("ExtraFriction", parse_friction_per_sec, UserData::None),
    FieldParse::new("Offset", |ini, p, _| { p.nugget.offset = ini.parse_coord3d()?; Ok(()) }, UserData::None),
    FieldParse::new(
        "Disposition",
        |ini, p, _| ini.parse_bit_string32(&mut p.nugget.disposition, &DEBRIS_DISPOSITION_NAMES),
        UserData::Names(&DEBRIS_DISPOSITION_NAMES),
    ),
    FieldParse::new("DispositionIntensity", |ini, p, _| { p.nugget.disposition_intensity = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("SpinRate", |ini, p, _| { p.nugget.spin_rate = ini.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("YawRate", |ini, p, _| { p.nugget.yaw_rate = ini.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("RollRate", |ini, p, _| { p.nugget.roll_rate = ini.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PitchRate", |ini, p, _| { p.nugget.pitch_rate = ini.parse_angular_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinForceMagnitude", |ini, p, _| { p.nugget.min_mag = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxForceMagnitude", |ini, p, _| { p.nugget.max_mag = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinForcePitch", |ini, p, _| { p.nugget.min_pitch = if plant(25) { ini.parse_real()? } else { ini.parse_angle_real()? }; Ok(()) }, UserData::None),
    FieldParse::new("MaxForcePitch", |ini, p, _| { p.nugget.max_pitch = ini.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinLifetime", |ini, p, _| { p.nugget.min_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxLifetime", |ini, p, _| { p.nugget.max_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("SpreadFormation", |ini, p, _| { p.nugget.spread_formation = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("MinDistanceAFormation", |ini, p, _| { p.nugget.min_distance_a_formation = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinDistanceBFormation", |ini, p, _| { p.nugget.min_distance_b_formation = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxDistanceFormation", |ini, p, _| { p.nugget.max_distance_formation = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("FadeIn", |ini, p, _| { p.nugget.fade_in = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FadeOut", |ini, p, _| { p.nugget.fade_out = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("FadeTime", |ini, p, _| { p.nugget.fade_frames = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("FadeSound", |ini, p, _| { p.nugget.fade_sound_name = ini.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("PreserveLayer", |ini, p, _| { p.nugget.preserve_layer = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DiesOnBadLand", |ini, p, _| { p.nugget.dies_on_bad_land = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `GenericObjectCreationNugget::parseObject`'s `myFieldParse`.
static GENERIC_OBJECT_FIELDS: [FieldParse<GenericParse>; 10] = [
    FieldParse::new("ContainInsideSourceObject", |ini, p, _| { p.nugget.contain_inside_source_object = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("ObjectNames", parse_debris_object_names, UserData::None),
    FieldParse::new("ObjectCount", |ini, p, _| { p.nugget.object_count = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("InheritsVeterancy", |ini, p, _| { p.nugget.inherits_veterancy = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("SkipIfSignificantlyAirborne", |ini, p, _| { p.nugget.skip_if_significantly_airborne = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("InvulnerableTime", |ini, p, _| { p.nugget.invulnerable_time = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MinHealth", |ini, p, _| { p.nugget.min_health = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxHealth", |ini, p, _| { if plant(24) { p.nugget.min_health = ini.parse_percent_to_real()?; } else { p.nugget.max_health = ini.parse_percent_to_real()?; } Ok(()) }, UserData::None),
    FieldParse::new("RequiresLivePlayer", |ini, p, _| { p.nugget.requires_live_player = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `GenericObjectCreationNugget::parseDebris`'s `myFieldParse`.
static GENERIC_DEBRIS_FIELDS: [FieldParse<GenericParse>; 9] = [
    FieldParse::new("ModelNames", parse_debris_object_names, UserData::None),
    FieldParse::new("Mass", |ini, p, _| { p.nugget.mass = ini.parse_positive_non_zero_real()?; Ok(()) }, UserData::None),
    FieldParse::new("AnimationSet", parse_anim_set, UserData::None),
    FieldParse::new("FXFinal", parse_fx_final, UserData::None),
    FieldParse::new("OkToChangeModelColor", |ini, p, _| { p.nugget.ok_to_change_model_color = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("MinLODRequired", parse_static_game_lod_level, UserData::None),
    FieldParse::new("Shadow", |ini, p, _| ini.parse_bit_string32(&mut p.nugget.shadow_type, &SHADOW_NAMES), UserData::Names(&SHADOW_NAMES)),
    FieldParse::new("BounceSound", parse_bounce_sound, UserData::None),
    FieldParse::end(),
];

/// Runs one nugget's block: the field lines up to `End` into a fresh nugget of the class, whose debris names (kept even if the block
/// fails, as the global is upstream's) and which (only if the block succeeds) joins the list.
fn finish_nugget<N>(this: &mut OclParse, p: NuggetParse<N>, r: IniResult<()>, wrap: fn(N) -> Nugget) -> IniResult<()> {
    if !(plant(17) && r.is_err()) {
        this.debris_hack.extend(p.debris_hack);
    }
    if plant(18) && r.is_err() {
        this.nuggets.push(Arc::new(wrap(p.nugget)));
        return r;
    }
    r?;
    this.nuggets.push(Arc::new(wrap(p.nugget)));
    Ok(())
}

/// `FireWeaponNugget::parse`.
fn parse_fire_weapon(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    let mut p = NuggetParse::new(FireWeaponNugget::default(), &this.env);
    let r = ini.init_from_ini(&mut p, &FIRE_WEAPON_FIELDS);
    finish_nugget(this, p, r, Nugget::FireWeapon)
}

/// `AttackNugget::parse`.
fn parse_attack(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    let mut p = NuggetParse::new(AttackNugget::default(), &this.env);
    let r = ini.init_from_ini(&mut p, &ATTACK_FIELDS);
    finish_nugget(this, p, r, Nugget::Attack)
}

/// `DeliverPayloadNugget::parse`: the nugget's own fields, then the data's (`MultiIniFieldParse`).
fn parse_deliver_payload(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    let mut p = NuggetParse::new(DeliverPayloadNugget::default(), &this.env);
    let data_fields = DeliverPayloadData::field_parse::<DeliverParse>();
    let r = (|| {
        let mut multi = MultiIniFieldParse::new();
        multi.add(&DELIVER_PAYLOAD_FIELDS)?;
        multi.add(&data_fields)?;
        ini.init_from_ini_multi(&mut p, &multi)
    })();
    finish_nugget(this, p, r, Nugget::DeliverPayload)
}

/// `ApplyRandomForceNugget::parse`.
fn parse_apply_random_force(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    let mut p = NuggetParse::new(ApplyRandomForceNugget::default(), &this.env);
    let r = ini.init_from_ini(&mut p, &APPLY_RANDOM_FORCE_FIELDS);
    finish_nugget(this, p, r, Nugget::ApplyRandomForce)
}

/// `GenericObjectCreationNugget::parseObject` (`name_are_objects`) and `parseDebris` (not): the common fields, then the class's.
fn parse_generic(ini: &mut Ini<'_>, this: &mut OclParse, name_are_objects: bool) -> IniResult<()> {
    let generic = GenericObjectCreationNugget { name_are_objects, ..GenericObjectCreationNugget::default() };
    let mut p = NuggetParse::new(generic, &this.env);
    let r = (|| {
        let mut multi = MultiIniFieldParse::new();
        multi.add(&GENERIC_COMMON_FIELDS)?;
        multi.add(if name_are_objects { &GENERIC_OBJECT_FIELDS } else { &GENERIC_DEBRIS_FIELDS })?;
        ini.init_from_ini_multi(&mut p, &multi)
    })();
    // "Zero masses are not allowed for debris" is a debug assertion only.
    finish_nugget(this, p, r, Nugget::Generic)
}

/// `GenericObjectCreationNugget::parseObject`.
fn parse_create_object(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    parse_generic(ini, this, true)
}

/// `GenericObjectCreationNugget::parseDebris`.
fn parse_create_debris(ini: &mut Ini<'_>, this: &mut OclParse, _: &UserData) -> IniResult<()> {
    parse_generic(ini, this, false)
}

/// `TheObjectCreationListFieldParse`.
static OBJECT_CREATION_LIST_FIELDS: [FieldParse<OclParse>; 7] = [
    FieldParse::new("CreateObject", parse_create_object, UserData::None),
    FieldParse::new("CreateDebris", parse_create_debris, UserData::None),
    FieldParse::new("ApplyRandomForce", parse_apply_random_force, UserData::None),
    FieldParse::new("DeliverPayload", parse_deliver_payload, UserData::None),
    FieldParse::new("FireWeapon", parse_fire_weapon, UserData::None),
    FieldParse::new("Attack", parse_attack, UserData::None),
    FieldParse::end(),
];

/// `class ObjectCreationListStore` (`TheObjectCreationListStore`): "the 'store' used to hold all the ObjectCreationLists in existence".
#[derive(Clone, Debug, Default)]
pub struct ObjectCreationListStore {
    /// `m_nuggets`: every nugget ever parsed, in order (the lists share them).
    pub nuggets: Vec<Arc<Nugget>>,
    /// `m_ocls`.
    pub ocls: BTreeMap<NameKeyType, ObjectCreationList>,
    /// `debrisModelNamesGlobalHack`: the debris names parsed while `m_preloadAssets` was set (a global upstream; nothing reads it).
    pub debris_model_names_global_hack: Vec<Vec<u8>>,
}

impl ObjectCreationListStore {
    /// `ObjectCreationListStore()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `findObjectCreationList(name)`: the list with the given name, `None` if there is none or the name is `None`. The name gets a key
    /// (`NAMEKEY(name)`) whether or not there is such a list.
    pub fn find_object_creation_list(&self, names: &mut NameKeyGenerator, name: &[u8]) -> Option<&ObjectCreationList> {
        if if plant(21) { name == b"None" } else { name.eq_ignore_ascii_case(b"None") } {
            return None;
        }
        self.ocls.get(&names.name_to_key(name))
    }

    /// `addObjectCreationNugget(nugget)`.
    pub fn add_object_creation_nugget(&mut self, nugget: Arc<Nugget>) {
        self.nuggets.push(nugget);
    }

    /// `parseObjectCreationListDefinition(ini)` (also `INI::parseObjectCreationListDefinition`): the list's name, then its nugget blocks up to
    /// `End`. The list is cleared first; the nuggets completed before an error stay in the list and the store.
    pub fn parse_object_creation_list_definition(&mut self, ini: &mut Ini<'_>, names: &mut NameKeyGenerator, env: &Arc<OclParseEnv>) -> IniResult<()> {
        let c = ini.get_next_token(None)?;
        let key = names.name_to_key(&c);
        let ocl = self.ocls.entry(key).or_default();
        if !plant(19) {
            ocl.clear();
        }
        let mut target = OclParse { env: env.clone(), nuggets: Vec::new(), debris_hack: Vec::new() };
        let r = ini.init_from_ini(&mut target, &OBJECT_CREATION_LIST_FIELDS);
        for n in target.nuggets {
            ocl.nuggets.push(n.clone());
            if !plant(20) {
                self.nuggets.push(n);
            }
        }
        self.debris_model_names_global_hack.extend(target.debris_hack);
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::ini::{BlockParse, IniLoadType};

    /// A host that answers every question with a fixed, plausible answer and records the calls that have an effect, named
    /// after the method and the object they are made on.
    #[derive(Default)]
    struct Rec {
        next: ObjectId,
        log: Vec<String>,
        /// The primary object has no controlling player.
        no_player: bool,
    }

    impl OclHost for Rec {
    fn frame(&self) -> u32 {
        0
    }
    fn is_debris_skipped(&self) -> bool {
        false
    }
    fn random_value(&mut self, _lo: i32, hi: i32) -> i32 {
        self.log.push("random_value".to_string());
        hi
    }
    fn random_value_real(&mut self, _lo: f32, hi: f32) -> f32 {
        self.log.push("random_value_real".to_string());
        hi
    }
    fn destroy_object(&mut self, o: ObjectId) {
        self.log.push(format!("destroy_object {}", o));
    }
    fn neutral_player_default_team(&self) -> Option<TeamId> {
        Some(2)
    }
    fn player_default_team(&self, _player: PlayerIndex) -> Option<TeamId> {
        Some(3)
    }
    fn player_is_active(&self, _player: PlayerIndex) -> bool {
        true
    }
    fn transfer_object_name(&mut self, _name: &[u8], _o: ObjectId) {
        self.log.push("transfer_object_name".to_string());
    }
    fn debris_template(&mut self) -> Option<TemplateRef> {
        self.log.push("debris_template".to_string());
        Some(7)
    }
    fn find_template(&mut self, _name: &[u8]) -> Option<TemplateRef> {
        self.log.push("find_template".to_string());
        Some(7)
    }
    fn new_object(&mut self, _tmplate: Option<TemplateRef>, _team: Option<TeamId>) -> Option<ObjectId> {
        self.log.push("new_object".to_string());
        {
            self.next += 1;
            Some(self.next)
        }
    }
    fn obj_position(&self, _o: ObjectId) -> Coord3D {
        Coord3D::default()
    }
    fn obj_orientation(&self, _o: ObjectId) -> f32 {
        0.0
    }
    fn obj_transform(&self, _o: ObjectId) -> Matrix3D {
        Matrix3D::IDENTITY
    }
    fn obj_controlling_player(&self, _o: ObjectId) -> Option<PlayerIndex> {
        (!self.no_player).then_some(1)
    }
    fn obj_layer(&self, _o: ObjectId) -> PathfindLayer {
        LAYER_GROUND
    }
    fn obj_is_above_terrain(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_is_significantly_above_terrain(&self, _o: ObjectId) -> bool {
        false
    }
    fn obj_is_off_map(&self, _o: ObjectId) -> bool {
        false
    }
    fn obj_is_structure(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_veterancy_level(&self, _o: ObjectId) -> VeterancyLevel {
        VeterancyLevel::Elite
    }
    fn obj_indicator_color(&self, _o: ObjectId) -> i32 {
        0
    }
    fn obj_name(&self, _o: ObjectId) -> Vec<u8> {
        Vec::new()
    }
    fn obj_unit_direction_vector_2d(&self, _o: ObjectId) -> Coord3D {
        Coord3D { x: 1.0, y: 0.0, z: 0.0 }
    }
    fn obj_set_producer(&mut self, o: ObjectId, _producer: Option<ObjectId>) {
        self.log.push(format!("obj_set_producer {}", o));
    }
    fn obj_set_position(&mut self, o: ObjectId, _pos: &Coord3D) {
        self.log.push(format!("obj_set_position {}", o));
    }
    fn obj_set_orientation(&mut self, o: ObjectId, _angle: f32) {
        self.log.push(format!("obj_set_orientation {}", o));
    }
    fn obj_set_transform_matrix(&mut self, o: ObjectId, _mtx: &Matrix3D) {
        self.log.push(format!("obj_set_transform_matrix {}", o));
    }
    fn obj_set_layer(&mut self, o: ObjectId, _layer: PathfindLayer) {
        self.log.push(format!("obj_set_layer {}", o));
    }
    fn obj_set_script_status(&mut self, o: ObjectId, _bit: u8) {
        self.log.push(format!("obj_set_script_status {}", o));
    }
    fn obj_set_disabled_until(&mut self, o: ObjectId, _disabled: DisabledType, _frame: u32) {
        self.log.push(format!("obj_set_disabled_until {}", o));
    }
    fn obj_set_weapon_lock(&mut self, o: ObjectId, _slot: i32, _lock: i32) {
        self.log.push(format!("obj_set_weapon_lock {}", o));
    }
    fn obj_go_invulnerable(&mut self, o: ObjectId, _frames: u32) {
        self.log.push(format!("obj_go_invulnerable {}", o));
    }
    fn obj_kill(&mut self, o: ObjectId) {
        self.log.push(format!("obj_kill {}", o));
    }
    fn obj_attempt_damage(&mut self, o: ObjectId, _info: &DamageInfo) {
        self.log.push(format!("obj_attempt_damage {}", o));
    }
    fn obj_has_update_module(&self, _o: ObjectId, _kind: UpdateModuleKind) -> bool {
        true
    }
    fn obj_has_special_power_completion_die(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_has_ai(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_has_physics(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_has_body(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_has_contain(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_has_drawable(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_is_trainable(&self, _o: ObjectId) -> bool {
        true
    }
    fn obj_set_veterancy_level(&mut self, o: ObjectId, _level: VeterancyLevel) {
        self.log.push(format!("obj_set_veterancy_level {}", o));
    }
    fn ai_attack_position(&mut self, o: ObjectId, _pos: &Coord3D, _max_shots: i32, _cmd: CommandSourceType) {
        self.log.push(format!("ai_attack_position {}", o));
    }
    fn radius_decal_create(&mut self, o: ObjectId, _tmpl: &RadiusDecalTemplate, _radius: f32, _pos: &Coord3D) {
        self.log.push(format!("radius_decal_create {}", o));
    }
    fn radius_decal_kill_when_no_longer_attacking(&mut self, o: ObjectId) {
        self.log.push(format!("radius_decal_kill_when_no_longer_attacking {}", o));
    }
    fn deliver_payload(&mut self, o: ObjectId, _move_to: &Coord3D, _target: &Coord3D, _data: &DeliverPayloadData) {
        self.log.push(format!("deliver_payload {}", o));
    }
    fn deliver_payload_max_speed_for_condition(&self, _o: ObjectId, _damage_state: i32) -> f32 {
        10.0
    }
    fn deliver_payload_preferred_height(&self, _o: ObjectId) -> f32 {
        30.0
    }
    fn body_damage_state(&self, _o: ObjectId) -> i32 {
        1
    }
    fn body_set_initial_health(&mut self, o: ObjectId, _percent: i32) {
        self.log.push(format!("body_set_initial_health {}", o));
    }
    fn lifetime_set_range(&mut self, o: ObjectId, _min_frames: u32, _max_frames: u32) {
        self.log.push(format!("lifetime_set_range {}", o));
    }
    fn float_set_enabled(&mut self, o: ObjectId) {
        self.log.push(format!("float_set_enabled {}", o));
    }
    fn special_power_completion_die_set_creator(&mut self, o: ObjectId, _creator: ObjectId) {
        self.log.push(format!("special_power_completion_die_set_creator {}", o));
    }
    fn behavior_module_count(&self, _o: ObjectId) -> usize {
        2
    }
    fn behavior_module_has_slaved_update(&self, _o: ObjectId, i: usize) -> bool {
        i == 1
    }
    fn behavior_module_on_enslave(&mut self, o: ObjectId, _i: usize, _slaver: Option<ObjectId>) {
        self.log.push(format!("behavior_module_on_enslave {}", o));
    }
    fn physics_velocity(&self, _o: ObjectId) -> Coord3D {
        Coord3D::default()
    }
    fn physics_mass(&self, _o: ObjectId) -> f32 {
        2.0
    }
    fn physics_apply_motive_force(&mut self, o: ObjectId, _force: &Coord3D) {
        self.log.push(format!("physics_apply_motive_force {}", o));
    }
    fn physics_apply_force(&mut self, o: ObjectId, _force: &Coord3D) {
        self.log.push(format!("physics_apply_force {}", o));
    }
    fn physics_set_mass(&mut self, o: ObjectId, _mass: f32) {
        self.log.push(format!("physics_set_mass {}", o));
    }
    fn physics_set_extra_friction(&mut self, o: ObjectId, _f: f32) {
        self.log.push(format!("physics_set_extra_friction {}", o));
    }
    fn physics_set_extra_bounciness(&mut self, o: ObjectId, _b: f32) {
        self.log.push(format!("physics_set_extra_bounciness {}", o));
    }
    fn physics_set_allow_bouncing(&mut self, o: ObjectId, _b: bool) {
        self.log.push(format!("physics_set_allow_bouncing {}", o));
    }
    fn physics_set_allow_to_fall(&mut self, o: ObjectId, _b: bool) {
        self.log.push(format!("physics_set_allow_to_fall {}", o));
    }
    fn physics_set_bounce_sound(&mut self, o: ObjectId, _name: &[u8]) {
        self.log.push(format!("physics_set_bounce_sound {}", o));
    }
    fn physics_set_ignore_collisions_with(&mut self, o: ObjectId, _other: Option<ObjectId>) {
        self.log.push(format!("physics_set_ignore_collisions_with {}", o));
    }
    fn physics_set_angles(&mut self, o: ObjectId, _yaw: f32, _pitch: f32, _roll: f32) {
        self.log.push(format!("physics_set_angles {}", o));
    }
    fn physics_set_yaw_rate(&mut self, o: ObjectId, _r: f32) {
        self.log.push(format!("physics_set_yaw_rate {}", o));
    }
    fn physics_set_roll_rate(&mut self, o: ObjectId, _r: f32) {
        self.log.push(format!("physics_set_roll_rate {}", o));
    }
    fn physics_set_pitch_rate(&mut self, o: ObjectId, _r: f32) {
        self.log.push(format!("physics_set_pitch_rate {}", o));
    }
    fn contain_is_valid_container_for(&self, _container: ObjectId, _obj: ObjectId, _check_capacity: bool) -> bool {
        true
    }
    fn contain_add(&mut self, container: ObjectId, _obj: ObjectId) {
        self.log.push(format!("contain_add {}", container));
    }
    fn drawable_is_effectively_hidden(&self, _o: ObjectId) -> bool {
        false
    }
    fn drawable_set_hidden(&mut self, o: ObjectId, _hidden: bool) {
        self.log.push(format!("drawable_set_hidden {}", o));
    }
    fn drawable_fade_in(&mut self, o: ObjectId, _frames: u32) {
        self.log.push(format!("drawable_fade_in {}", o));
    }
    fn drawable_fade_out(&mut self, o: ObjectId, _frames: u32) {
        self.log.push(format!("drawable_fade_out {}", o));
    }
    fn drawable_debris_interface_count(&self, _o: ObjectId) -> usize {
        1
    }
    fn debris_set_model_name(&mut self, o: ObjectId, _i: usize, _name: &[u8], _color: i32, _shadow_type: u32) {
        self.log.push(format!("debris_set_model_name {}", o));
    }
    fn debris_set_anim_names(&mut self, o: ObjectId, _i: usize, _initial: &[u8], _flying: &[u8], _fin: &[u8], _final_fx: &FxListName) {
        self.log.push(format!("debris_set_anim_names {}", o));
    }
    fn terrain_ground_height(&self, _x: f32, _y: f32) -> f32 {
        0.0
    }
    fn terrain_highest_layer_for_destination(&self, _pos: &Coord3D) -> PathfindLayer {
        LAYER_GROUND
    }
    fn terrain_layer_height(&self, _x: f32, _y: f32, _layer: PathfindLayer) -> f32 {
        0.0
    }
    fn terrain_flatten(&mut self, o: ObjectId) {
        self.log.push(format!("terrain_flatten {}", o));
    }
    fn terrain_is_underwater(&self, _x: f32, _y: f32) -> Option<(f32, f32)> {
        Some((50.0, 40.0))
    }
    fn pathfinder_cell_type(&self, _layer: PathfindLayer, _x: i32, _y: i32) -> Option<PathfindCellType> {
        Some(PathfindCellType::Clear)
    }
    fn pathfinder_add_object(&mut self, o: ObjectId) {
        self.log.push(format!("pathfinder_add_object {}", o));
    }
    fn partition_find_position_around(&mut self, _center: &Coord3D, _options: &FindPositionOptions) -> Coord3D {
        self.log.push("partition_find_position_around".to_string());
        Coord3D::default()
    }
    fn weapon_create_and_fire_temp(&mut self, _weapon: WeaponTemplateRef, _source: ObjectId, _pos: &Coord3D) {
        self.log.push("weapon_create_and_fire_temp".to_string());
    }
    fn particle_system_find_template(&self, _name: &[u8]) -> Option<u32> {
        Some(1)
    }
    fn particle_system_create_attached(&mut self, _tmpl: u32, _o: ObjectId) {
        self.log.push("particle_system_create_attached".to_string());
    }
    fn audio_add_event(&mut self, _name: &[u8], _object: ObjectId) {
        self.log.push("audio_add_event".to_string());
    }
    }

    struct Ctx {
        store: ObjectCreationListStore,
        names: NameKeyGenerator,
        env: Arc<OclParseEnv>,
    }

    fn block(ini: &mut Ini<'_>, c: &mut Ctx) -> IniResult<()> {
        c.store.parse_object_creation_list_definition(ini, &mut c.names, &c.env)
    }

    fn load(text: &str) -> (Ctx, IniResult<()>) {
        let mut names = NameKeyGenerator::new();
        names.reset();
        let mut weapon_templates = BTreeMap::new();
        weapon_templates.insert(b"W1".to_vec(), 5);
        let mut c = Ctx { store: ObjectCreationListStore::new(), names, env: Arc::new(OclParseEnv { preload_assets: true, weapon_templates }) };
        let blocks = [BlockParse { token: "ObjectCreationList", parse: block }];
        let r = Ini::load(b"t.ini", text.as_bytes(), IniLoadType::Overwrite, None, &blocks, &mut c);
        (c, r)
    }

    const SAMPLE: &str = "ObjectCreationList OCL_Mix\n\
        CreateObject\n ObjectNames = T0 T1 T2 T3\n Disposition = SEND_IT_UP LIKE_EXISTING\n DispositionIntensity = 2\n Count = 2\n ParticleSystem = P0\n MinHealth = 50%\n InheritsVeterancy = Yes\nEnd\n\
        CreateDebris\n ModelNames = M_a\n Mass = 3\n AnimationSet = A B C\n Shadow = SHADOW_DECAL\n FadeIn = Yes\nEnd\n\
        ApplyRandomForce\n SpinRate = 90\n MaxForceMagnitude = 4\nEnd\n\
        DeliverPayload\n Transport = T9\n FormationSize = 3\n FormationSpacing = 25.5\n Payload = T4 2\n DeliveryDistance = 10\n StartAtMaxSpeed = Yes\nEnd\n\
        FireWeapon\n Weapon = W1\nEnd\n\
        Attack\n NumberOfShots = 3\n WeaponSlot = SECONDARY\n DeliveryDecal\n Texture = Ring\n End\nEnd\n\
        End\n";

    #[test]
    fn parses_every_nugget_class_and_keeps_every_other_debris_name() {
        let (mut c, r) = load(SAMPLE);
        r.unwrap();
        assert_eq!(c.store.nuggets.len(), 6);
        assert_eq!(c.store.debris_model_names_global_hack, vec![b"T0".to_vec(), b"T2".to_vec(), b"M_a".to_vec()]);
        let ocl = c.store.find_object_creation_list(&mut c.names, b"OCL_Mix").unwrap();
        assert_eq!(ocl.nuggets.len(), 6);
        match &*ocl.nuggets[0] {
            Nugget::Generic(g) => {
                assert_eq!(g.names, vec![b"T0".to_vec(), b"T2".to_vec()]);
                assert_eq!(g.disposition, SEND_IT_UP | LIKE_EXISTING);
                assert_eq!((g.min_health, g.debris_to_generate, g.inherits_veterancy, g.name_are_objects), (0.5, 2, true, true));
            }
            n => panic!("{n:?}"),
        }
        match &*ocl.nuggets[3] {
            Nugget::DeliverPayload(d) => assert_eq!((d.formation_size, d.formation_spacing, d.payload[0].payload_count, d.data.dist_to_target), (3, 25.5, 2, 10.0)),
            n => panic!("{n:?}"),
        }
        assert!(matches!(&*ocl.nuggets[4], Nugget::FireWeapon(FireWeaponNugget { weapon: Some(5) })));
        assert!(c.store.find_object_creation_list(&mut c.names, b"none").is_none());
        assert!(c.store.find_object_creation_list(&mut c.names, b"OCL_Nope").is_none());
    }

    #[test]
    fn a_redefinition_clears_the_list_but_the_store_keeps_the_nuggets() {
        let (mut c, r) = load("ObjectCreationList A\n FireWeapon\n End\nEnd\nObjectCreationList A\n Attack\n End\nEnd\n");
        r.unwrap();
        assert_eq!(c.store.nuggets.len(), 2);
        assert_eq!(c.store.find_object_creation_list(&mut c.names, b"A").unwrap().nuggets.len(), 1);
    }

    #[test]
    fn a_failed_block_keeps_the_nuggets_completed_before_the_error() {
        let (mut c, r) = load("ObjectCreationList A\n FireWeapon\n End\n CreateObject\n Count = x\n End\nEnd\n");
        assert!(r.is_err());
        assert_eq!(c.store.find_object_creation_list(&mut c.names, b"A").unwrap().nuggets.len(), 1);
        assert_eq!(c.store.nuggets.len(), 1);
    }

    fn created(text: &str, f: impl Fn(&ObjectCreationList, &mut Rec) -> OclResult<Option<ObjectId>>) -> (Option<ObjectId>, Vec<String>) {
        let (mut c, r) = load(text);
        r.unwrap();
        let ocl = c.store.find_object_creation_list(&mut c.names, b"A").unwrap().clone();
        let mut host = Rec { next: 100, ..Rec::default() };
        let first = f(&ocl, &mut host).unwrap();
        (first, host.log)
    }

    #[test]
    fn create_object_makes_the_objects_and_sets_them_up() {
        let text = "ObjectCreationList A\n CreateObject\n ObjectNames = T0\n Disposition = LIKE_EXISTING\n Count = 2\n End\nEnd\n";
        let (first, log) = created(text, |o, h| o.create_object(h, Some(9), Some(8), 0));
        assert_eq!(first, Some(101));
        for want in ["new_object", "obj_set_producer 101", "obj_set_transform_matrix 101", "obj_set_position 102", "terrain_flatten 101", "pathfinder_add_object 102"] {
            assert!(log.iter().any(|l| l.starts_with(want)), "{want} in {log:?}");
        }
    }

    #[test]
    fn create_at_fire_weapon_attack_and_force() {
        let text = "ObjectCreationList A\n FireWeapon\n Weapon = W1\n End\n Attack\n End\n ApplyRandomForce\n End\n CreateDebris\n ModelNames = M\n End\nEnd\n";
        let pos = Coord3D { x: 1.0, y: 2.0, z: 3.0 };
        let (first, log) = created(text, |o, h| o.create_at(h, Some(9), Some(&pos), Some(&pos), INVALID_ANGLE, 5));
        assert_eq!(first, Some(101));
        for want in ["weapon_create_and_fire_temp", "obj_set_weapon_lock 9", "radius_decal_create 9", "debris_set_model_name 101"] {
            assert!(log.iter().any(|l| l.starts_with(want)), "{want} in {log:?}");
        }
        // through the object overload the force applies, and nothing else changes the first object
        let (_, log) = created(text, |o, h| o.create_object(h, Some(9), Some(8), 0));
        assert!(log.iter().any(|l| l.starts_with("physics_apply_force 9")), "{log:?}");
    }

    #[test]
    fn deliver_payload_makes_a_formation() {
        let text = "ObjectCreationList A\n DeliverPayload\n Transport = T1\n FormationSize = 2\n FormationSpacing = 10\n Payload = T2 2\n PutInContainer = T3\n \
                    StartAtMaxSpeed = Yes\n WeaponErrorRadius = 5\n DelayDeliveryMax = 300\n End\nEnd\n";
        let from = Coord3D { x: 0.0, y: 0.0, z: 0.0 };
        let to = Coord3D { x: 100.0, y: 50.0, z: 0.0 };
        let (first, log) = created(text, |o, h| o.create_with_owner(h, Some(9), Some(&from), Some(&to), true, 0));
        assert_eq!(first, Some(101));
        assert_eq!(log.iter().filter(|l| l.starts_with("deliver_payload")).count(), 2);
        assert!(log.iter().any(|l| l.starts_with("physics_apply_motive_force 101")), "{log:?}");
        assert!(log.iter().any(|l| l.starts_with("contain_add")), "{log:?}");
    }

    #[test]
    fn undefined_inputs_are_errors() {
        let text = "ObjectCreationList A\n CreateObject\n ObjectNames = T0\n FadeIn = Yes\n End\nEnd\n";
        let (mut c, r) = load(text);
        r.unwrap();
        let ocl = c.store.find_object_creation_list(&mut c.names, b"A").unwrap().clone();
        let pos = Coord3D::default();
        let mut host = Rec { next: 100, ..Rec::default() };
        // a fade without a source object writes through a null pointer
        assert!(ocl.create_at(&mut host, None, Some(&pos), Some(&pos), 0.0, 0).unwrap_err().is_undefined());
        // so does a payload delivery by an object that has no controlling player
        let (mut c, r) = load("ObjectCreationList A\n DeliverPayload\n Transport = T1\n End\nEnd\n");
        r.unwrap();
        let ocl = c.store.find_object_creation_list(&mut c.names, b"A").unwrap().clone();
        let mut host = Rec { next: 100, no_player: true, ..Rec::default() };
        assert!(ocl.create_at(&mut host, Some(9), Some(&pos), Some(&pos), 0.0, 0).unwrap_err().is_undefined());
        host.no_player = false;
        assert!(ocl.create_at(&mut host, Some(9), Some(&pos), Some(&pos), 0.0, 0).is_ok());
    }
}
