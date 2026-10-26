// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BridgeBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BridgeBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BridgeBehavior`: the object of a bridge on the terrain. Damage or healing done to it is passed on, as the same fraction of each one's
//! maximum health, to its four towers (unless it came from a tower); a change of its body's damage state plays the bridge template's
//! transition sound and scatters its effects over the bridge's surface; its death kills the towers, drops (or kills) what stood on it and
//! starts the timed death effects of its data, which `update()` fires frame by frame. A dozer repairing it raises scaffolding: rows of
//! `BridgeScaffoldBehavior` objects rising out of the ground at both ends to meet in the middle, which tear down again when it is done.
//!
//! The interface (`BridgeBehaviorInterface`) is the module calls [`ModuleCall::BridgeSetTower`], [`ModuleCall::GetBridgeTowerId`],
//! `BridgeCreateScaffolding`, `BridgeRemoveScaffolding`, `BridgeIsScaffoldInMotion` and `BridgeIsScaffoldPresent`;
//! [`ModuleCall::HasBridgeBehaviorInterface`] answers `getBridgeBehaviorInterface()`.
//!
//! # Upstream facts kept
//!
//! - The data: `LateralScaffoldSpeed` and `VerticalScaffoldSpeed` (velocities, per frame; 1 by default, not converted), `BridgeDieFX` and
//!   `BridgeDieOCL`, each `FX:<list> Delay:<duration> [Bone:<name>]` (`OCL:`), the labels case-insensitive; a missing or wrong label throws
//!   `INI_INVALID_DATA`. A list the store does not find is `NULL` (a debug crash only), and is then never played.
//! - The bridge (`TheTerrainLogic->findBridgeAt(position)`) and its template (`TheTerrainRoads->findBridge(name)`) are looked up again at
//!   every use. The transition effects are resolved from the template once, at the first damage state change (`resolveFX`; not saved), and
//!   only if both are found then.
//! - `onBodyDamageStateChange`: the death frame clears unless the new state is rubble; with the effects resolved and a bridge, the sound of
//!   the new state (repaired to it when the old state was worse) and `getNumFXPerType()` of each of the three FX and OCL entries of that
//!   state at random spots of the bridge (`getRandomSurfacePosition`: two random fractions of the bridge's sides, then a random height up
//!   to `getTransitionEffectsHeight()`; one spot per FX and one per OCL, FX first); then `updateBridgeDamageStates()`, and a terrain
//!   refresh of the radar when either state is rubble.
//! - `update()` never sleeps; once dead, each death FX and OCL whose delay is exactly the frames since the death plays: at its bone, else at
//!   a random spot of the bridge (when the bridge and its template are found), else at the object; an OCL whose bone is `ParentObject`
//!   is made from the object alone (`create(ocl, us, NULL)`). The spot is picked even when the list is `NULL`.
//! - `onDie`: `kill()` each tower that exists, then everything the partition finds within the distance from the bridge's centre to its
//!   `toLeft` corner (2D) that is not a bridge or tower, is not above the terrain, is not below the bridge's lowest corner, is inside the
//!   bridge's polygon (even-odd) and is on the bridge's layer: put on the ground layer, and allowed to fall (its physics) or killed; then
//!   the death frame.
//! - `createScaffolding` (once until removed): the scaffold and support templates of the bridge template (none: nothing); the objects tile
//!   from the middle of each end to the middle of the bridge, `ceil(length / spacing) + 1` of them (the spacing twice the scaffold's major
//!   radius), alternating ends, each with columns of supports below it until the support layer passes zero; each starts sunk by its height
//!   plus 8 below its rise-to point, its lateral speed scaled by the ratio of its build distance to its distance to the bridge's centre.
//!   The bridge's pathfinding layer is then closed. The right end's angle is the left's plus `TWO_PI` (sic).
//! - `removeScaffolding`: each scaffold object that exists reverses; the list is emptied, and the layer opened again unless the body is
//!   rubble. `isScaffoldInMotion`: any listed object whose interface's motion is not still.
//! - The xfer (version 1): the version, `UpdateModule`'s, (on load: the bridge told this is its object), the four tower ids, (on load: the
//!   bridge told its towers), the scaffold flag, the list's size (an `UnsignedShort`) and ids (a load appends to the list), the death
//!   frame. The CRC is `UpdateModule`'s.
//! - `onDelete` empties the scaffold list. `setTower` and `getTowerID` ignore a tower type out of range (a debug crash).
//!
//! # Where upstream is undefined
//!
//! Damage or healing to an object without a body, or passed to a tower without one; `kill()` of a tower or of an object on the bridge
//! without a body; `removeScaffolding` on an object without a body: [`Undefined::NullBodyModule`]. Effects of a state change to play with
//! a bridge whose template is not found: [`Undefined::NullBridgeTemplate`]. `createScaffolding` with no bridge or no template there, or a
//! load with no bridge: [`Undefined::NullBridge`] / `NullBridgeTemplate`. A scaffold object (made or kept) without the scaffold interface:
//! [`Undefined::NullScaffoldInterface`]. A damage state outside the four: [`Undefined::ArrayIndexOutOfRange`]. Scaffolding whose support
//! template has no height, at a bridge end at or above zero: the column never ends ([`Undefined::EndlessLoop`]). The case is recorded in
//! `world.cuts.module_errors` and the call returns.
//!
//! # Deviation
//!
//! The destructor destroys the four towers when they still exist; the world has no hook for a module's destruction, so it is not run here
//! (as `ParticleUplinkCannonUpdate`'s). The FX lists are names: the client's store resolves them (empty or `None` is `NULL`).
//!
//! `p1w13-sonnet`, batch R77, 2026-10-26.

use std::sync::Arc;

use crate::common::base_type::{real_to_int_ceil, Coord2D, Coord3D, TWO_PI};
use crate::common::game_type::{ObjectId, INVALID_ANGLE, INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData, SEPS_COLON};
use crate::common::kind_of::{KINDOF_BRIDGE, KINDOF_BRIDGE_TOWER};
use crate::common::module::{ModuleDataKind, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::body_module::{is_condition_worse, BODY_RUBBLE};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::bridge_scaffold_behavior::{STM_RISE, STM_STILL};
use crate::logic::modules::lend_clone;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::terrain_logic::BRIDGE_MAX_TOWERS;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::{BridgeDetails, BridgeTemplateDetails, CutCall};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::partition::filters::point_inside_area_2d;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `MAX_BRIDGE_BODY_FX` (`TerrainRoads.h`): the FX and OCL entries per damage state of a bridge template.
pub const MAX_BRIDGE_BODY_FX: usize = 3;
/// `BODYDAMAGETYPE_COUNT`.
const BODY_STATES: usize = 4;
/// `FROM_CENTER_2D` (`PartitionManager.h`'s `DistanceCalculationType`).
const FROM_CENTER_2D: i32 = 0;

type D = BridgeBehaviorModuleData;

/// `struct TimeAndLocationInfo`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimeAndLocationInfo {
    /// `delay`: "how long to wait to execute this" (frames).
    pub delay: u32,
    /// `boneName`: "which bone to execute at" (empty: none).
    pub bone_name: Vec<u8>,
}

/// `struct BridgeFXInfo` / `BridgeOCLInfo`: the list's name (`None`: `NULL`) and when and where.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BridgeEffectInfo {
    pub list: Option<Vec<u8>>,
    pub time_and_location_info: TimeAndLocationInfo,
}

/// `class BridgeBehaviorModuleData : public BehaviorModuleData` (the update base's scheduling data as `update`).
#[derive(Clone, Debug)]
pub struct BridgeBehaviorModuleData {
    pub update: UpdateModuleData,
    /// `m_lateralScaffoldSpeed`, `m_verticalScaffoldSpeed`.
    pub lateral_scaffold_speed: f32,
    pub vertical_scaffold_speed: f32,
    /// `m_fx`: "list of FX lists to execute"; `m_ocl`: "list of OCL to execute".
    pub fx: Vec<BridgeEffectInfo>,
    pub ocl: Vec<BridgeEffectInfo>,
}

impl Default for BridgeBehaviorModuleData {
    /// The constructor's: both speeds 1, no lists.
    fn default() -> Self {
        BridgeBehaviorModuleData { update: UpdateModuleData::new(), lateral_scaffold_speed: 1.0, vertical_scaffold_speed: 1.0, fx: Vec::new(), ocl: Vec::new() }
    }
}

/// `parseTimeAndLocationInfo`: `Delay:<duration>`, then optionally `Bone:<name>`.
fn parse_time_and_location_info(ini: &mut Ini<'_>) -> IniResult<TimeAndLocationInfo> {
    // delay label
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if !token.eq_ignore_ascii_case(b"Delay") {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let delay = ini.parse_duration_unsigned_int()?;
    // get optional bone label
    let mut bone_name = Vec::new();
    if let Some(token) = ini.get_next_token_or_null(Some(SEPS_COLON)) {
        // "token must be a label for bone location"
        if !token.eq_ignore_ascii_case(b"Bone") {
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
        bone_name = ini.get_next_ascii_string();
    }
    Ok(TimeAndLocationInfo { delay, bone_name })
}

/// `parseFX` / `parseOCL`: the label (`FX` or `OCL`), the list (`INI::parseFXList`, `parseObjectCreationList`: the next token; `None` is
/// `NULL`), the timing.
fn parse_effect(ini: &mut Ini<'_>, label: &[u8]) -> IniResult<BridgeEffectInfo> {
    let token = ini.get_next_token(Some(SEPS_COLON))?;
    if !token.eq_ignore_ascii_case(label) {
        return Err(IniError::new(IniErrorKind::InvalidData));
    }
    let name = ini.get_next_token(None)?;
    let list = (!name.eq_ignore_ascii_case(b"None")).then_some(name);
    let time_and_location_info = parse_time_and_location_info(ini)?;
    Ok(BridgeEffectInfo { list, time_and_location_info })
}

static FIELD_PARSE: [FieldParse<D>; 5] = [
    FieldParse::new("LateralScaffoldSpeed", |i, d: &mut D, _| { d.lateral_scaffold_speed = i.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("VerticalScaffoldSpeed", |i, d: &mut D, _| { d.vertical_scaffold_speed = i.parse_velocity_real()?; Ok(()) }, UserData::None),
    FieldParse::new("BridgeDieFX", |i, d: &mut D, _| { let e = parse_effect(i, b"FX")?; d.fx.push(e); Ok(()) }, UserData::None),
    FieldParse::new("BridgeDieOCL", |i, d: &mut D, _| { let e = parse_effect(i, b"OCL")?; d.ocl.push(e); Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl BridgeBehaviorModuleData {
    /// `buildFieldParse(p)`: `BehaviorModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, D>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<D> {
        let mut data = D::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, D::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `BridgeBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::BridgeBehavior(Arc::new(D::friend_new_module_data(ini)?)))
}

/// The test's view of one set of resolved effects: the three OCLs then the three FXs (`None`: `NULL`), the sound's name and object.
pub type ResolvedEffects = (Vec<Option<Vec<u8>>>, Vec<u8>, ObjectId);

/// An `AudioEventRTS` of the module's: its event name and object (the default: none, `INVALID_ID`).
#[derive(Clone, Debug, Default, PartialEq)]
struct Sound {
    name: Vec<u8>,
    object: ObjectId,
}

/// What a load leaves to do with the world once the xfer is done: tell the bridge its object, then its towers.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LoadStep {
    BridgeObject,
    Towers,
}

/// `class BridgeBehavior : public UpdateModule, public BridgeBehaviorInterface, public DamageModuleInterface, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct BridgeBehavior {
    update: UpdateModule,
    data: Arc<D>,
    /// `m_towerID`: "the towers that are a part of us".
    tower_id: [ObjectId; BRIDGE_MAX_TOWERS],
    /// `m_damageToOCL`, `m_damageToFX`, `m_damageToSound`; `m_repairToOCL`, `m_repairToFX`, `m_repairToSound` (the lists by name, `None`:
    /// `NULL`).
    damage_to_ocl: [[Option<Vec<u8>>; MAX_BRIDGE_BODY_FX]; BODY_STATES],
    damage_to_fx: [[Option<Vec<u8>>; MAX_BRIDGE_BODY_FX]; BODY_STATES],
    damage_to_sound: [Sound; BODY_STATES],
    repair_to_ocl: [[Option<Vec<u8>>; MAX_BRIDGE_BODY_FX]; BODY_STATES],
    repair_to_fx: [[Option<Vec<u8>>; MAX_BRIDGE_BODY_FX]; BODY_STATES],
    repair_to_sound: [Sound; BODY_STATES],
    /// `m_fxResolved`.
    fx_resolved: bool,
    /// `m_scaffoldPresent`: "TRUE when we have repair scaffolding visible".
    scaffold_present: bool,
    /// `m_scaffoldObjectIDList`.
    scaffold_object_ids: Vec<ObjectId>,
    /// `m_deathFrame`: "frame we died on".
    death_frame: u32,
    /// The load's steps that need the world (see [`BridgeBehavior::finish_load`]).
    pending_load: Vec<LoadStep>,
}

impl PartialEq for BridgeBehavior {
    /// The same object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && self.tower_id == other.tower_id
            && self.damage_to_ocl == other.damage_to_ocl
            && self.damage_to_fx == other.damage_to_fx
            && self.damage_to_sound == other.damage_to_sound
            && self.repair_to_ocl == other.repair_to_ocl
            && self.repair_to_fx == other.repair_to_fx
            && self.repair_to_sound == other.repair_to_sound
            && self.fx_resolved == other.fx_resolved
            && self.scaffold_present == other.scaffold_present
            && self.scaffold_object_ids == other.scaffold_object_ids
            && self.death_frame == other.death_frame
            && self.pending_load == other.pending_load
    }
}

impl BridgeBehavior {
    /// `getInterfaceMask()`: damage, die and update.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE | UpdateModule::get_interface_mask()
    }

    /// The constructor (it touches no world): no towers, nothing resolved, no scaffolding, never died (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<D>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(BridgeBehavior {
            update,
            data,
            tower_id: [INVALID_ID; BRIDGE_MAX_TOWERS],
            damage_to_ocl: Default::default(),
            damage_to_fx: Default::default(),
            damage_to_sound: Default::default(),
            repair_to_ocl: Default::default(),
            repair_to_fx: Default::default(),
            repair_to_sound: Default::default(),
            fx_resolved: false,
            scaffold_present: false,
            scaffold_object_ids: Vec::new(),
            death_frame: 0,
            pending_load: Vec::new(),
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<D> {
        &self.data
    }

    /// The state the test compares, as words: the tower ids, the flags, the death frame, the scaffold list.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.tower_id.iter().map(|&id| id as u32).collect();
        v.extend([u32::from(self.fx_resolved), u32::from(self.scaffold_present), self.death_frame, self.scaffold_object_ids.len() as u32]);
        v.extend(self.scaffold_object_ids.iter().map(|&id| id as u32));
        v
    }

    /// The resolved effects, the test's view: for each state, damaged-to then repaired-to, the three OCLs, the three FXs and the sound (its
    /// name and object).
    pub fn resolved_effects(&self) -> Vec<ResolvedEffects> {
        let mut v = Vec::new();
        for s in 0..BODY_STATES {
            for (ocl, fx, sound) in [(&self.damage_to_ocl, &self.damage_to_fx, &self.damage_to_sound), (&self.repair_to_ocl, &self.repair_to_fx, &self.repair_to_sound)] {
                let lists = ocl[s].iter().chain(fx[s].iter()).cloned().collect();
                v.push((lists, sound[s].name.clone(), sound[s].object));
            }
        }
        v
    }

    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The object's position.
    fn position(world: &World, id: ObjectId) -> Coord3D {
        world.objects.find(id).map_or_else(Coord3D::default, |o| o.get_position())
    }

    /// `getBodyModule()->getMaxHealth()` of `id`; [`Undefined::NullBodyModule`] without a body.
    fn max_health(world: &mut World, id: ObjectId) -> Result<f32, Undefined> {
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        Ok(world.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0))
    }

    /// `TheTerrainLogic->findBridgeAt(us->getPosition())`.
    fn find_bridge(world: &mut World, us: ObjectId) -> Option<BridgeDetails> {
        let pos = Self::position(world, us);
        world.terrain_find_bridge_details(pos)
    }

    /// `setTower(towerType, tower)`: the tower's id (`INVALID_ID` for none); a type out of range is ignored.
    fn set_tower(&mut self, world: &World, tower_type: i32, tower: ObjectId) {
        // sanity
        if tower_type < 0 || tower_type >= BRIDGE_MAX_TOWERS as i32 {
            return;
        }
        self.tower_id[tower_type as usize] = if world.objects.find(tower).is_some() { tower } else { INVALID_ID };
    }

    /// `getTowerID(towerType)`: `INVALID_ID` for a type out of range.
    pub fn get_tower_id(&self, tower_type: i32) -> ObjectId {
        if tower_type < 0 || tower_type >= BRIDGE_MAX_TOWERS as i32 {
            return INVALID_ID;
        }
        self.tower_id[tower_type as usize]
    }

    /// `resolveFX()`: the bridge template's effects by state, found in the stores; nothing without a bridge or its template.
    fn resolve_fx(&mut self, world: &mut World, us: ObjectId) {
        let Some(bridge) = Self::find_bridge(world, us) else { return };
        let Some(t) = world.terrain_roads_find_bridge(&bridge.template_name) else { return };
        let ocl = |world: &mut World, name: &[u8]| -> Option<Vec<u8>> {
            let statics = world.statics.clone();
            statics.object_creation_list_store.find_object_creation_list(&mut world.names, name).map(|_| name.to_vec())
        };
        // FXListStore::findFXList: `None` is NULL; the client resolves the rest by name (an empty name is no list)
        let fx = |name: &[u8]| -> Option<Vec<u8>> { (!name.is_empty() && !name.eq_ignore_ascii_case(b"None")).then(|| name.to_vec()) };
        for s in 0..BODY_STATES {
            for i in 0..MAX_BRIDGE_BODY_FX {
                self.damage_to_ocl[s][i] = ocl(world, &t.damage_to_ocl[s][i]);
                self.damage_to_fx[s][i] = fx(&t.damage_to_fx[s][i]);
                self.repair_to_ocl[s][i] = ocl(world, &t.repaired_to_ocl[s][i]);
                self.repair_to_fx[s][i] = fx(&t.repaired_to_fx[s][i]);
            }
            // audio sounds
            self.damage_to_sound[s] = Sound { name: t.damage_to_sound[s].clone(), object: us };
            self.repair_to_sound[s] = Sound { name: t.repaired_to_sound[s].clone(), object: us };
        }
        // fx are now "resolved"
        self.fx_resolved = true;
    }

    /// `onDamage(damageInfo)`: the same fraction of each tower's maximum health, from this bridge, unless a tower did it.
    fn on_damage(&self, world: &mut World, us: ObjectId, info: &DamageInfo) -> Result<(), Undefined> {
        let damage_percentage = info.input.amount / Self::max_health(world, us)?;
        let from_tower = world.objects.find(info.input.source_id).is_some_and(|s| s.is_kind_of(KINDOF_BRIDGE_TOWER));
        if !from_tower || crate::logic::modules::plant(2490) {
            for &tower in &self.tower_id {
                if world.objects.find(tower).is_none() {
                    continue;
                }
                let mut tower_damage = DamageInfo::default();
                tower_damage.input.amount = damage_percentage * Self::max_health(world, tower)?;
                tower_damage.input.source_id = us; // we're now the source
                tower_damage.input.damage_type = info.input.damage_type;
                tower_damage.input.death_type = info.input.death_type;
                world.attempt_damage(tower, &mut tower_damage);
            }
        }
        Ok(())
    }

    /// `onHealing(damageInfo)`: the same fraction of each tower's maximum health, from this bridge, unless a tower did it.
    fn on_healing(&self, world: &mut World, us: ObjectId, info: &DamageInfo) -> Result<(), Undefined> {
        let healing_percentage = info.input.amount / Self::max_health(world, us)?;
        let from_tower = world.objects.find(info.input.source_id).is_some_and(|s| s.is_kind_of(KINDOF_BRIDGE_TOWER));
        if !from_tower {
            for &tower in &self.tower_id {
                if world.objects.find(tower).is_none() {
                    continue;
                }
                let amount = healing_percentage * Self::max_health(world, tower)?;
                world.attempt_healing(tower, amount, us);
            }
        }
        Ok(())
    }

    /// `getRandomSurfacePosition(bridgeTemplate, bridgeInfo, &pos)`: a random spot of the bridge's area, at a random height up to the
    /// template's transition effects height.
    fn get_random_surface_position(world: &mut World, t: &BridgeTemplateDetails, b: &BridgeDetails) -> Coord3D {
        let to_side = if crate::logic::modules::plant(2491) { b.to_right } else { b.from_right };
        let scale = world.random.game_logic_random_value_real(0.0, 1.0);
        let v1 = Coord3D { x: (b.to_left.x - b.from_left.x) * scale, y: (b.to_left.y - b.from_left.y) * scale, z: (b.to_left.z - b.from_left.z) * scale };
        let scale = world.random.game_logic_random_value_real(0.0, 1.0);
        let v2 = Coord3D { x: (to_side.x - b.from_left.x) * scale, y: (to_side.y - b.from_left.y) * scale, z: (to_side.z - b.from_left.z) * scale };
        let mut pos = Coord3D { x: v1.x + v2.x + b.from_left.x, y: v1.y + v2.y + b.from_left.y, z: v1.z + v2.z + b.from_left.z };
        // "add in an additional Z component so that effects can be created in a 'cube' area on and above the bridge"
        pos.z += world.random.game_logic_random_value_real(0.0, t.transition_effects_height);
        pos
    }

    /// `doAreaEffects(bridgeTemplate, bridge, ocl, fx)`: `getNumFXPerType()` of each at random spots of the bridge.
    fn do_area_effects(world: &mut World, us: ObjectId, t: Option<&BridgeTemplateDetails>, b: &BridgeDetails, ocl: &Option<Vec<u8>>, fx: &Option<Vec<u8>>) -> Result<(), Undefined> {
        // if no effects, don't bother
        if ocl.is_none() && fx.is_none() {
            return Ok(());
        }
        let t = t.ok_or(Undefined::NullBridgeTemplate)?;
        for _ in 0..t.num_fx_per_type {
            if let Some(fx) = fx {
                let pos = Self::get_random_surface_position(world, t, b);
                world.emit(ObjectClientEvent::FxListDoFxPos { fx: fx.clone(), position: pos });
            }
            if let Some(ocl) = ocl {
                let pos = Self::get_random_surface_position(world, t, b);
                world.cut(CutCall::OclCreateAtPrimary { primary: us, ocl: ocl.clone(), primary_pos: pos, angle: INVALID_ANGLE });
            }
        }
        Ok(())
    }

    /// `onBodyDamageStateChange(damageInfo, oldState, newState)`.
    fn on_body_damage_state_change(&mut self, world: &mut World, us: ObjectId, old_state: i32, new_state: i32) -> Result<(), Undefined> {
        // "check for coming back from the dead"
        if new_state != BODY_RUBBLE {
            self.death_frame = 0;
        }
        // first resolve any fx stuff if we need to
        if !self.fx_resolved {
            self.resolve_fx(world, us);
        }
        if !self.fx_resolved {
            return Ok(());
        }
        // sanity (a debug crash)
        let Some(bridge) = Self::find_bridge(world, us) else { return Ok(()) };
        let template = world.terrain_roads_find_bridge(&bridge.template_name);
        let state = usize::try_from(new_state).ok().filter(|&s| s < BODY_STATES).ok_or(Undefined::ArrayIndexOutOfRange)?;
        // "given the old state and the new state, did we get worse (damaged) or did we get better (repaired)?"
        let got_repaired = is_condition_worse(old_state, new_state) != crate::logic::modules::plant(2492);
        let (sound, ocls, fxs) = if got_repaired {
            (&self.repair_to_sound[state], &self.repair_to_ocl[state], &self.repair_to_fx[state])
        } else {
            (&self.damage_to_sound[state], &self.damage_to_ocl[state], &self.damage_to_fx[state])
        };
        // play the sound
        world.audio_add_event(sound.name.clone(), sound.object);
        for i in 0..MAX_BRIDGE_BODY_FX {
            Self::do_area_effects(world, us, template.as_ref(), &bridge, &ocls[i], &fxs[i])?;
        }
        // update bridge damage states
        world.terrain_update_bridge_damage_states();
        // "if this bridge has switched from rubble, to usable, or from usable to rubble, we should reflect the change on the radar"
        if old_state == BODY_RUBBLE || new_state == BODY_RUBBLE {
            world.radar_queue_terrain_refresh();
        }
        Ok(())
    }

    /// `update()`: the death effects whose delay has come.
    fn do_update(&mut self, world: &mut World, us: ObjectId) {
        // if we're dead, we need to possibly throw off some effects
        if self.death_frame == 0 {
            return;
        }
        let bridge = Self::find_bridge(world, us);
        let template = bridge.as_ref().and_then(|b| world.terrain_roads_find_bridge(&b.template_name));
        // how much time has passed between now and our destruction frame
        let death_time = world.frame().wrapping_sub(self.death_frame).wrapping_sub(u32::from(crate::logic::modules::plant(2493)));
        let surface = |world: &mut World| -> Coord3D {
            match (&bridge, &template) {
                (Some(b), Some(t)) => Self::get_random_surface_position(world, t, b),
                _ => Self::position(world, us),
            }
        };
        let data = self.data.clone();
        for fx in &data.fx {
            if death_time != fx.time_and_location_info.delay {
                continue;
            }
            let bone = &fx.time_and_location_info.bone_name;
            let pos = if !bone.is_empty() { world.object_get_single_logical_bone_transform(us, bone).1 } else { surface(world) };
            // launch the fx list
            if let Some(name) = &fx.list {
                world.emit(ObjectClientEvent::FxListDoFxPos { fx: name.clone(), position: pos });
            }
        }
        let found = |world: &mut World, name: &Option<Vec<u8>>| -> Option<Vec<u8>> {
            let name = name.as_ref()?;
            let statics = world.statics.clone();
            statics.object_creation_list_store.find_object_creation_list(&mut world.names, name).map(|_| name.clone())
        };
        for ocl in &data.ocl {
            if death_time != ocl.time_and_location_info.delay {
                continue;
            }
            let bone = &ocl.time_and_location_info.bone_name;
            if bone.as_slice() == b"ParentObject" {
                // "launch the effects just using the parent object for location info"
                if let Some(name) = found(world, &ocl.list) {
                    world.cut(CutCall::OclCreate { object: us, ocl: name });
                }
                continue;
            }
            let pos = if !bone.is_empty() { world.object_get_single_logical_bone_transform(us, bone).1 } else { surface(world) };
            if let Some(name) = found(world, &ocl.list) {
                world.cut(CutCall::OclCreateAtPrimary { primary: us, ocl: name, primary_pos: pos, angle: INVALID_ANGLE });
            }
        }
    }

    /// `onDie(damageInfo)`: kill the towers, handle what stood on the bridge, record the death frame.
    fn on_die(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        for &tower in &self.tower_id {
            if world.objects.find(tower).is_some() {
                // Object::kill reads the body unchecked
                world.get_body_module(tower).ok_or(Undefined::NullBodyModule)?;
                world.kill(tower, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
            }
        }
        Self::handle_objects_on_bridge_on_die(world, us)?;
        // we have now died, record the death frame
        self.death_frame = world.frame();
        Ok(())
    }

    /// `handleObjectsOnBridgeOnDie()`: what stood on the bridge goes to the ground layer and falls (or dies).
    fn handle_objects_on_bridge_on_die(world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        let bridge_pos = Self::position(world, us);
        let Some(b) = Self::find_bridge(world, us) else { return Ok(()) };
        let bridge_layer = b.layer;
        let polygon = [b.from_left, b.from_right, b.to_right, b.to_left];
        // "find the lowest Z point of the bridge area"
        let mut low_bridge_z = polygon[0].z;
        for p in &polygon {
            if p.z < low_bridge_z {
                low_bridge_z = p.z;
            }
        }
        let radius = Coord2D { x: b.to_left.x - bridge_pos.x, y: b.to_left.y - bridge_pos.y }.length();
        for other in world.partition_iterate_objects_in_range_at(bridge_pos, radius, FROM_CENTER_2D) {
            let Some(o) = world.objects.find(other) else { continue };
            if o.is_kind_of(KINDOF_BRIDGE) || o.is_kind_of(KINDOF_BRIDGE_TOWER) {
                continue;
            }
            let (pos, layer, physics) = (o.get_position(), o.layer, o.physics);
            // ignore airborne objects
            if world.is_above_terrain(other) {
                continue;
            }
            // ignore objects that were not actually on the bridge
            if pos.z < low_bridge_z && !crate::logic::modules::plant(2494) {
                continue;
            }
            if !point_inside_area_2d(&pos, &polygon) {
                continue;
            }
            // if object not on same layer as bridge do nothing
            if bridge_layer != layer {
                continue;
            }
            world.set_layer(other, LAYER_GROUND);
            // "if they have physics, let 'em fall, otherwise just kill 'em"
            match physics {
                Some(p) => {
                    world.module_call(other, p, ModuleCall::PhysicsSetAllowToFall(true));
                }
                None => {
                    world.get_body_module(other).ok_or(Undefined::NullBodyModule)?;
                    world.kill(other, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                }
            }
        }
        Ok(())
    }

    /// `setScaffoldData(obj, angle, sunkenHeight, riseToPos, buildPos, bridgeCenter)`.
    #[allow(clippy::too_many_arguments)]
    fn set_scaffold_data(&self, world: &mut World, obj: ObjectId, angle: f32, sunken_height: f32, rise_to_pos: Coord3D, build_pos: Coord3D, bridge_center: Coord3D) -> Result<(), Undefined> {
        let scaffold = world.bridge_scaffold_interface(obj);
        // "compute the sunken position that the object will initially start at"
        let fudge = 8.0f32;
        let mut sunken_pos = rise_to_pos;
        sunken_pos.z = sunken_pos.z - sunken_height - fudge;
        world.set_position(obj, &sunken_pos);
        let scaffold = scaffold.ok_or(Undefined::NullScaffoldInterface)?;
        world.module_call(obj, scaffold, ModuleCall::ScaffoldSetPositions { create_pos: sunken_pos, rise_to_pos, build_pos });
        world.module_call(obj, scaffold, ModuleCall::ScaffoldSetMotion(STM_RISE));
        world.set_orientation(obj, angle);
        // "set the speed of the scaffold 'animation'"
        let lateral_speed = self.data.lateral_scaffold_speed;
        let to_build = Coord3D { x: build_pos.x - rise_to_pos.x, y: build_pos.y - rise_to_pos.y, z: build_pos.z - rise_to_pos.z };
        let to_center = Coord3D { x: bridge_center.x - rise_to_pos.x, y: bridge_center.y - rise_to_pos.y, z: bridge_center.z - rise_to_pos.z };
        let (dist_build, dist_center) = (to_build.length(), to_center.length());
        world.module_call(obj, scaffold, ModuleCall::ScaffoldSetLateralSpeed(lateral_speed * (dist_build / dist_center)));
        // rising speed is always the same for all objects
        world.module_call(obj, scaffold, ModuleCall::ScaffoldSetVerticalSpeed(self.data.vertical_scaffold_speed));
        Ok(())
    }

    /// A scaffold object of `template` on our team, its id kept, its data set (`newObject` and `setScaffoldData`).
    #[allow(clippy::too_many_arguments)]
    fn make_scaffold(&mut self, world: &mut World, us: ObjectId, template: u16, angle: f32, height: f32, rise_to_pos: Coord3D, build_pos: Coord3D, center: Coord3D) -> Result<(), IniError> {
        let team = world.get_team(us);
        let obj = world.thing_factory_new_object(template, team, Default::default()).map_err(|_| IniError::new(IniErrorKind::BadArg))?;
        self.set_scaffold_data(world, obj, angle, height, rise_to_pos, build_pos, center).map_err(IniError::undefined)?;
        self.scaffold_object_ids.push(obj);
        Ok(())
    }

    /// The scaffold object and its columns of supports for one side of one step of `createScaffolding`.
    #[allow(clippy::too_many_arguments)]
    fn make_scaffold_column(&mut self, world: &mut World, us: ObjectId, t: [(u16, f32); 2], angle: f32, rise_to_pos: Coord3D, destination_pos: Coord3D, center: Coord3D) -> Result<(), IniError> {
        let [(scaffold, scaffold_height), (support, support_height)] = t;
        self.make_scaffold(world, us, scaffold, angle, scaffold_height, rise_to_pos, destination_pos, center)?;
        // "create support object layers under the scaffold object"
        let mut offset = rise_to_pos.z;
        // (a support of no height never takes the layer below zero: upstream loops making supports for ever)
        if offset >= 0.0 && (support_height <= 0.0 || support_height.is_nan()) {
            return Err(IniError::undefined(Undefined::EndlessLoop));
        }
        let (mut support_rise_to_pos, mut support_destination_pos, mut support_bridge_center) = (rise_to_pos, destination_pos, center);
        while offset >= 0.0 {
            support_rise_to_pos.z -= support_height;
            support_destination_pos.z -= support_height;
            support_bridge_center.z -= support_height;
            self.make_scaffold(world, us, support, angle, support_height, support_rise_to_pos, support_destination_pos, support_bridge_center)?;
            // off to the next layer
            offset -= support_height;
        }
        Ok(())
    }

    /// `createScaffolding()`: "Start the bridge repair scaffolding. If we already have scaffolding this call is ignored".
    fn create_scaffolding(&mut self, world: &mut World, us: ObjectId) -> Result<(), IniError> {
        if self.scaffold_present {
            return Ok(());
        }
        let center = Self::position(world, us);
        let bridge = Self::find_bridge(world, us).ok_or_else(|| IniError::undefined(Undefined::NullBridge))?;
        let t = world.terrain_roads_find_bridge(&bridge.template_name).ok_or_else(|| IniError::undefined(Undefined::NullBridgeTemplate))?;
        let factory = &world.statics.thing_factory;
        let Some(scaffold) = factory.find_template(&t.scaffold_object_name).map(|x| &x.data) else { return Ok(()) };
        let Some(support) = factory.find_template(&t.scaffold_support_object_name).map(|x| &x.data) else { return Ok(()) };
        let (g, sg) = (scaffold.get_template_geometry_info(), support.get_template_geometry_info());
        // "how much space is going to be between each of the scaffold objects at their final positions"
        let spacing = g.get_major_radius() * 2.0;
        let scaffold_height = g.get_max_height_above_position() + g.get_max_height_below_position();
        let support_height = sg.get_max_height_above_position() + sg.get_max_height_below_position();
        let templates = [(scaffold.get_template_id(), scaffold_height), (support.get_template_id(), support_height)];
        let mid = |a: Coord3D, b: Coord3D| Coord3D { x: ((a.x - b.x) / 2.0) + b.x, y: ((a.y - b.y) / 2.0) + b.y, z: ((a.z - b.z) / 2.0) + b.z };
        let left_start = mid(bridge.from_left, bridge.from_right);
        let right_start = mid(bridge.to_left, bridge.to_right);
        let left_angle = Coord2D { x: right_start.x - left_start.x, y: right_start.y - left_start.y }.to_angle();
        let right_angle = left_angle + TWO_PI;
        let mut left_vector = Coord3D { x: right_start.x - left_start.x, y: right_start.y - left_start.y, z: right_start.z - left_start.z };
        let mut right_vector = Coord3D { x: left_start.x - right_start.x, y: left_start.y - right_start.y, z: left_start.z - right_start.z };
        // "how many of these scaffold objects will take to tile from each of the endpoints to the center area of the bridge"
        let tile_distance = left_vector.length();
        let num_objects = real_to_int_ceil(tile_distance / spacing) + 1;
        let num_iterations = real_to_int_ceil(num_objects as f32 / 2.0);
        left_vector.normalize();
        right_vector.normalize();
        let nudge = if crate::logic::modules::plant(2495) { 0.0 } else { 0.1 };
        let destination = |v: Coord3D, rise: Coord3D, i: i32| Coord3D {
            x: v.x * (spacing * i as f32) + rise.x + nudge,
            y: v.y * (spacing * i as f32) + rise.y,
            z: v.z * (spacing * i as f32) + rise.z,
        };
        let mut scaffold_objects_created = 0;
        for i in 0..num_iterations {
            // this object is from the "left" side of the bridge
            self.make_scaffold_column(world, us, templates, left_angle, left_start, destination(left_vector, left_start, i), center)?;
            scaffold_objects_created += 1;
            // "we don't do this on the last iteration through this loop when we have an odd number of objects"
            if scaffold_objects_created < num_objects {
                self.make_scaffold_column(world, us, templates, right_angle, right_start, destination(right_vector, right_start, i), center)?;
                scaffold_objects_created += 1;
            }
        }
        // scaffolding is now present
        self.scaffold_present = true;
        // when scaffolding is present, a bridge cannot be used
        world.pathfinder_change_bridge_state(bridge.layer, false);
        Ok(())
    }

    /// `removeScaffolding()`: "Remove the bridge scaffolding. If we don't have any then this call is ignored".
    fn remove_scaffolding(&mut self, world: &mut World, us: ObjectId) -> Result<(), Undefined> {
        if !self.scaffold_present {
            return Ok(());
        }
        // go through each object and tell them to reverse course
        for &id in &self.scaffold_object_ids {
            if world.objects.find(id).is_none() {
                continue;
            }
            let scaffold = world.bridge_scaffold_interface(id).ok_or(Undefined::NullScaffoldInterface)?;
            world.module_call(id, scaffold, ModuleCall::ScaffoldReverseMotion);
        }
        self.scaffold_object_ids.clear();
        self.scaffold_present = false;
        // "when scaffolding is gone, a bridge can be used again if we're not in a rubble state"
        let body = world.get_body_module(us).ok_or(Undefined::NullBodyModule)?;
        let state = world.module_call(us, body, ModuleCall::GetDamageState).as_int(0);
        if state != BODY_RUBBLE || crate::logic::modules::plant(2496) {
            if let Some(bridge) = Self::find_bridge(world, us) {
                world.pathfinder_change_bridge_state(bridge.layer, true);
            }
        }
        Ok(())
    }

    /// `isScaffoldInMotion()`: any scaffold object whose motion is not still.
    fn is_scaffold_in_motion(&self, world: &mut World) -> bool {
        for &id in &self.scaffold_object_ids {
            if world.objects.find(id).is_none() {
                continue;
            }
            let Some(scaffold) = world.bridge_scaffold_interface(id) else { continue };
            if world.module_call(id, scaffold, ModuleCall::ScaffoldGetCurrentMotion).as_int(STM_STILL) != STM_STILL {
                return true;
            }
        }
        false
    }

    /// The load's world half (after the xfer): `bridge->setBridgeObjectID(us)` once the base was read, `bridge->setTowerObjectID(id, i)`
    /// once the towers were; the error for no bridge there (it is dereferenced).
    pub fn finish_load(&mut self, world: &mut World, us: ObjectId) -> Option<IniError> {
        for step in std::mem::take(&mut self.pending_load) {
            let pos = Self::position(world, us);
            if world.terrain_find_bridge_details(pos).is_none() {
                return Some(IniError::undefined(Undefined::NullBridge));
            }
            match step {
                LoadStep::BridgeObject => world.bridge_set_bridge_object_id(pos, us),
                LoadStep::Towers => {
                    for (i, &id) in self.tower_id.iter().enumerate() {
                        world.bridge_set_tower_object_id(pos, id, i as i32);
                    }
                }
            }
        }
        None
    }

    fn record(world: &mut World, me: ModuleHandle, r: Result<(), Undefined>) {
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
    }
}

impl UpdateBehavior for BridgeBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// `update()`: the death effects; never sleeps.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.do_update(world, me.object);
        UPDATE_SLEEP_NONE
    }
    /// The fields after the version and the base: the tower ids, the scaffold flag, the list (its size an `UnsignedShort`; a load appends),
    /// the death frame. A load records the steps [`BridgeBehavior::finish_load`] takes with the world.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        let loading = xfer.get_xfer_mode() == XferMode::Load;
        if loading {
            self.pending_load.push(LoadStep::BridgeObject);
        }
        for id in &mut self.tower_id {
            xfer.xfer_object_id(id)?;
        }
        if loading {
            self.pending_load.push(LoadStep::Towers);
        }
        xfer.xfer_bool(&mut self.scaffold_present)?;
        let mut count = self.scaffold_object_ids.len() as u16;
        xfer.xfer_unsigned_short(&mut count)?;
        if xfer.get_xfer_mode() == XferMode::Save {
            for id in &self.scaffold_object_ids {
                let mut id = *id;
                xfer.xfer_object_id(&mut id)?;
            }
        } else {
            for _ in 0..count {
                let mut id = INVALID_ID;
                xfer.xfer_object_id(&mut id)?;
                self.scaffold_object_ids.push(id);
            }
        }
        if !crate::logic::modules::plant(2497) {
            xfer.xfer_unsigned_int(&mut self.death_frame)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::BridgeBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::BridgeBehavior(m) => Some(m),
            _ => None,
        }
    }
    /// The damage and die interfaces, `onDelete` and `BridgeBehaviorInterface`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let us = me.object;
        match call {
            ModuleCall::OnDamage(info) => {
                let r = lend_clone(world, me, self, UpdateBody::BridgeBehavior, Self::from_body, |w, m| m.on_damage(w, us, &info));
                Self::record(world, me, r);
            }
            ModuleCall::OnHealing(info) => {
                let r = lend_clone(world, me, self, UpdateBody::BridgeBehavior, Self::from_body, |w, m| m.on_healing(w, us, &info));
                Self::record(world, me, r);
            }
            ModuleCall::OnBodyDamageStateChange { old_state, new_state, .. } => {
                let r = self.on_body_damage_state_change(world, us, old_state, new_state);
                Self::record(world, me, r);
            }
            ModuleCall::OnDie(_) => {
                let r = lend_clone(world, me, self, UpdateBody::BridgeBehavior, Self::from_body, |w, m| m.on_die(w, us));
                Self::record(world, me, r);
            }
            ModuleCall::OnDelete => self.scaffold_object_ids.clear(),
            ModuleCall::HasBridgeBehaviorInterface => return ModuleReply::Bool(true),
            ModuleCall::GetBridgeTowerId { tower } => return ModuleReply::Id(self.get_tower_id(tower)),
            ModuleCall::BridgeSetTower { tower, object } => self.set_tower(world, tower, object),
            ModuleCall::BridgeCreateScaffolding => {
                if let Err(e) = self.create_scaffolding(world, us) {
                    world.cuts.module_errors.push((me, e));
                }
            }
            ModuleCall::BridgeRemoveScaffolding => {
                let r = self.remove_scaffolding(world, us);
                Self::record(world, me, r);
            }
            ModuleCall::BridgeIsScaffoldInMotion => return ModuleReply::Bool(self.is_scaffold_in_motion(world)),
            ModuleCall::BridgeIsScaffoldPresent => return ModuleReply::Bool(self.scaffold_present),
            _ => return ModuleReply::Unhandled,
        }
        ModuleReply::Done
    }
}

impl Snapshot for BridgeBehavior {
    /// `crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer` (version 1): the version, `UpdateModule::xfer`, the fields.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = D::default();
        assert_eq!((d.lateral_scaffold_speed, d.vertical_scaffold_speed), (1.0, 1.0));
        let m = BridgeBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.tower_id, [INVALID_ID; BRIDGE_MAX_TOWERS]);
        assert!(!m.fx_resolved && !m.scaffold_present && m.death_frame == 0);
        assert_eq!(m.get_tower_id(4), INVALID_ID);
    }
}
