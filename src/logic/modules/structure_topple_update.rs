// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StructureToppleUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureToppleUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StructureToppleUpdate`: a building that, when it dies (as the die mux allows), topples over away from its killer: a dramatic pause with
//! bursts at its base, then a fall that accelerates as its structural integrity decays, crushing what is under it with a weapon fired along
//! the strip it covers, FX at angles of the fall, and at the end the rubble.
//!
//! # Where the world comes in
//!
//! - The FX are [`ObjectClientEvent::FxListDoFxPos`] / `FxListDoFxObj`; the object creation lists are [`CutCall::OclCreateAtPrimary`]
//!   (`INVALID_ANGLE`); the crushing weapon is `CutCall::WeaponCreateAndFireTemp` (the weapon store's template by name; none, no crushing);
//!   the ground is [`World::terrain_get_ground_height`]; the script's topple direction is [`World::script_adjust_topple_direction`].
//! - The AI's `markAsDead`, the body's last damage and its `updateBodyParticleSystems`, and the `BoneFXUpdate`'s `stopAllBoneFX` are module
//!   calls; the deselection is `CutCall::GameLogicDeselectObject`.
//!
//! # Upstream facts kept
//!
//! - The burst delays are drawn from the client's random values (`GameClientRandomValue`); the topple delay and the angle from the logic's.
//! - `explosionRadius = averageRadius * 0.90` and `1.0 - m_structuralIntegrity`, `1.0 - Sin(theta)` are computed in `double`.
//! - `fxbones` (particle systems at bones) and `m_oclCount` have no INI field: no bone FX, and one OCL per phase.
//! - `applyCrushingDamage` keeps the loop's `j` after it (MSVC's `for` scope): the last crushed location is the first step at or past the
//!   reach, and the reach itself is crushed once more each call.
//! - `doToppleDoneStuff` calls `setOrientation(getOrientation())` (which rebuilds the transform upright) and then turns it about z.
//! - `xfer` saves the version, the base and the state but not `m_buildingHeight` (the constructor's, from the geometry then).
//!
//! # Where upstream is undefined
//!
//! No body (`getBodyModule()->getLastDamageInfo()`, [`Undefined::NullBodyModule`]); the end of the fall with no drawable
//! ([`Undefined::NullDrawable`]). The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R36, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{Coord2D, Coord3D, PI};
use crate::common::game_common::PLAYERMASK_ALL;
use crate::common::game_type::INVALID_ANGLE;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_POST_COLLAPSE, MODELCONDITION_RUBBLE};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::trig::{cos, sin};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageTypeFlags, DAMAGE_TYPE_FLAGS_ALL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::structure_collapse_update::{build_non_dup_random_index_list, MAX_IDX};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `STPHASE_INITIAL`.
pub const STPHASE_INITIAL: usize = 0;
/// `STPHASE_DELAY`.
pub const STPHASE_DELAY: usize = 1;
/// `STPHASE_FINAL`.
pub const STPHASE_FINAL: usize = 2;
/// `ST_PHASE_COUNT`.
pub const ST_PHASE_COUNT: usize = 3;
const PHASE_NAMES: [&str; 3] = ["INITIAL", "DELAY", "FINAL"];

/// `TOPPLE_ACCELERATION_FACTOR`.
const TOPPLE_ACCELERATION_FACTOR: f32 = 0.02;
/// `THETA_CEILING`: "this weapon won't do any damage until theta is less than this value".
const THETA_CEILING: f32 = PI / 6.0;
/// `WEAPON_SPACING_PERPENDICULAR`.
const WEAPON_SPACING_PERPENDICULAR: f32 = 25.0;
/// `WEAPON_SPACING_PARALLEL`.
const WEAPON_SPACING_PARALLEL: f32 = 25.0;

/// `enum StructureToppleStateType`, by value (saved as its four bytes).
pub const TOPPLESTATE_STANDING: i32 = 0;
/// `TOPPLESTATE_WAITINGFORTOPPLESTART`.
pub const TOPPLESTATE_WAITINGFORTOPPLESTART: i32 = 1;
/// `TOPPLESTATE_TOPPLING`.
pub const TOPPLESTATE_TOPPLING: i32 = 2;
/// `TOPPLESTATE_WAITINGFORDONE`.
pub const TOPPLESTATE_WAITINGFORDONE: i32 = 3;
/// `TOPPLESTATE_DONE`.
pub const TOPPLESTATE_DONE: i32 = 4;

/// `struct AngleFXInfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct AngleFxInfo {
    /// `angle`, in radians (the INI's degrees `* PI / 180.0f`).
    pub angle: f32,
    /// `fxList`, by name (`None` for `NULL`).
    pub fx_list: Option<Vec<u8>>,
}

/// `class StructureToppleUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct StructureToppleUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_dieMuxData`, in a `DieModuleData` (the same pair; the behaviour base of it is unused).
    pub die: DieModuleData,
    /// `m_minToppleDelay`.
    pub min_topple_delay: i32,
    /// `m_maxToppleDelay`.
    pub max_topple_delay: i32,
    /// `m_structuralIntegrity`.
    pub structural_integrity: f32,
    /// `m_structuralDecay`.
    pub structural_decay: f32,
    /// `m_damageFXTypes`: "flags used to play or not play the effects".
    pub damage_fx_types: DamageTypeFlags,
    /// `m_toppleStartFXList`.
    pub topple_start_fx_list: Option<Vec<u8>>,
    /// `m_toppleDelayFXList`.
    pub topple_delay_fx_list: Option<Vec<u8>>,
    /// `m_toppleFXList` (read, never played).
    pub topple_fx_list: Option<Vec<u8>>,
    /// `m_toppleDoneFXList`.
    pub topple_done_fx_list: Option<Vec<u8>>,
    /// `m_crushingFXList`.
    pub crushing_fx_list: Option<Vec<u8>>,
    /// `m_crushingWeaponName`.
    pub crushing_weapon_name: Vec<u8>,
    /// `m_minToppleBurstDelay`.
    pub min_topple_burst_delay: i32,
    /// `m_maxToppleBurstDelay`.
    pub max_topple_burst_delay: i32,
    /// `m_ocls[phase]`: the object creation list names (found in the store when used; "could be null! this is OK!").
    pub ocls: [Vec<Vec<u8>>; ST_PHASE_COUNT],
    /// `m_oclCount[phase]` (1, which nothing changes).
    pub ocl_count: [u32; ST_PHASE_COUNT],
    /// `angleFX`.
    pub angle_fx: Vec<AngleFxInfo>,
}

impl Default for StructureToppleUpdateModuleData {
    /// `StructureToppleUpdateModuleData()`.
    fn default() -> Self {
        StructureToppleUpdateModuleData {
            update: UpdateModuleData::new(),
            die: DieModuleData::new(),
            min_topple_delay: 0,
            max_topple_delay: 0,
            structural_integrity: 0.1,
            structural_decay: 0.0,
            damage_fx_types: DAMAGE_TYPE_FLAGS_ALL,
            topple_start_fx_list: None,
            topple_delay_fx_list: None,
            topple_fx_list: None,
            topple_done_fx_list: None,
            crushing_fx_list: None,
            crushing_weapon_name: Vec::new(),
            min_topple_burst_delay: 0,
            max_topple_burst_delay: 0,
            ocls: Default::default(),
            ocl_count: [1; ST_PHASE_COUNT],
            angle_fx: Vec::new(),
        }
    }
}

/// `INI::parseFXList`: the next token, `None` for `NULL`.
fn parse_fx(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// Lifts entry `k` of the layer-5 die field table onto `StructureToppleUpdateModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut StructureToppleUpdateModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `dataFieldParse`, then `DieMuxData::getFieldParse()`.
static FIELD_PARSE: [FieldParse<StructureToppleUpdateModuleData>; 20] = [
    FieldParse::new("MinToppleDelay", |i, d, _| { d.min_topple_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MaxToppleDelay", |i, d, _| { d.max_topple_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MinToppleBurstDelay", |i, d, _| { d.min_topple_burst_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MaxToppleBurstDelay", |i, d, _| { d.max_topple_burst_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("StructuralIntegrity", |i, d, _| { d.structural_integrity = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("StructuralDecay", |i, d, _| { d.structural_decay = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DamageFXTypes", |i, d, _| { d.damage_fx_types = i.parse_damage_type_flags()?; Ok(()) }, UserData::None),
    FieldParse::new("TopplingFX", |i, d, _| { d.topple_fx_list = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ToppleDelayFX", |i, d, _| { d.topple_delay_fx_list = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ToppleStartFX", |i, d, _| { d.topple_start_fx_list = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("ToppleDoneFX", |i, d, _| { d.topple_done_fx_list = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("CrushingFX", |i, d, _| { d.crushing_fx_list = parse_fx(i)?; Ok(()) }, UserData::None),
    FieldParse::new("CrushingWeaponName", |i, d, _| { d.crushing_weapon_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OCL", |i, d, _| {
        // `INI::scanIndexList(ini->getNextToken(), TheStructureTopplePhaseNames)`
        let token = i.get_next_token(None)?;
        let phase = Ini::scan_index_list(&token, &PHASE_NAMES)? as usize;
        // `for (token = ini->getNextToken(); token != NULL; token = ini->getNextTokenOrNull())`: the first token is required
        let mut token = Some(i.get_next_token(None)?);
        while let Some(t) = token {
            d.ocls[phase].push(t);
            token = i.get_next_token_or_null(None);
        }
        Ok(())
    }, UserData::None),
    FieldParse::new("AngleFX", |i, d, _| {
        // `parseAngleFX`: the angle in degrees, then the FX list
        let degrees = i.parse_real()?;
        let angle = degrees * PI / 180.0;
        let fx_list = parse_fx(i)?;
        d.angle_fx.push(AngleFxInfo { angle, fx_list });
        Ok(())
    }, UserData::None),
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::end(),
];

impl StructureToppleUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields and the die mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, StructureToppleUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<StructureToppleUpdateModuleData> {
        let mut data = StructureToppleUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StructureToppleUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StructureToppleUpdate : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct StructureToppleUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getStructureToppleUpdateModuleData()`.
    data: Arc<StructureToppleUpdateModuleData>,
    /// `m_toppleFrame`.
    topple_frame: u32,
    /// `m_toppleDirection`.
    topple_direction: Coord2D,
    /// `m_toppleState` (`StructureToppleStateType` by value).
    topple_state: i32,
    /// `m_toppleVelocity`.
    topple_velocity: f32,
    /// `m_accumulatedAngle`.
    accumulated_angle: f32,
    /// `m_structuralIntegrity`.
    structural_integrity: f32,
    /// `m_lastCrushedLocation`.
    last_crushed_location: f32,
    /// `m_nextBurstFrame`.
    next_burst_frame: i32,
    /// `m_delayBurstLocation`.
    delay_burst_location: Coord3D,
    /// `m_buildingHeight` (the geometry's at construction).
    building_height: f32,
}

impl PartialEq for StructureToppleUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl StructureToppleUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<StructureToppleUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(StructureToppleUpdate {
            update,
            data,
            topple_frame: 0,
            topple_direction: Coord2D { x: 0.0, y: 0.0 },
            topple_state: TOPPLESTATE_STANDING,
            topple_velocity: 0.0,
            // "Need to give it a little nudge in the right direction"
            accumulated_angle: 0.001,
            structural_integrity: 0.0,
            last_crushed_location: 0.0,
            next_burst_frame: -1,
            delay_burst_location: Coord3D::default(),
            building_height: 0.0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getStructureToppleUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<StructureToppleUpdateModuleData> {
        &self.data
    }

    /// The state the test compares, as words.
    pub fn state_words(&self) -> [u32; 13] {
        [
            self.topple_frame,
            self.topple_direction.x.to_bits(),
            self.topple_direction.y.to_bits(),
            self.topple_state as u32,
            self.topple_velocity.to_bits(),
            self.accumulated_angle.to_bits(),
            self.structural_integrity.to_bits(),
            self.last_crushed_location.to_bits(),
            self.next_burst_frame as u32,
            self.delay_burst_location.x.to_bits(),
            self.delay_burst_location.y.to_bits(),
            self.delay_burst_location.z.to_bits(),
            self.building_height.to_bits(),
        ]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::xfer_with(self, update, xfer)
    }

    /// `crc` with the scheduling half the slot holds: the base's.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds: the base's.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `lastDamageInfo == NULL || getDamageTypeFlag(m_damageFXTypes, lastDamageInfo->in.m_damageType)`, on the body's last damage (the body
    /// read unchecked).
    fn fx_allowed(&self, world: &mut World, id: ObjectId) -> Result<bool, Undefined> {
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        Ok(match world.module_call(id, body, ModuleCall::GetLastDamageInfo) {
            ModuleReply::Damage(last) => self.data.damage_fx_types.test(last.input.damage_type),
            _ => true,
        })
    }

    fn fx_pos(world: &mut World, fx: &Option<Vec<u8>>, position: Coord3D) {
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxPos { fx: fx.clone(), position });
        }
    }

    fn fx_obj(world: &mut World, fx: &Option<Vec<u8>>, object_id: ObjectId) {
        if let Some(fx) = fx {
            world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id });
        }
    }

    /// `beginStructureTopple(damageInfo)`.
    fn begin_structure_topple(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        let d = self.data.clone();
        let id = me.object;
        let now = world.frame();
        self.topple_frame = now.wrapping_add_signed(world.random.game_logic_random_value(d.min_topple_delay, d.max_topple_delay));
        let attacker = world.objects.find(info.input.source_id).map(|o| o.get_position());
        let (building_pos, geom) = match world.objects.find(id) {
            Some(o) => (o.get_position(), o.geometry_info.clone()),
            None => return Ok(()),
        };
        let topple_angle = match attacker {
            None => world.random.game_logic_random_value_real(0.0, 2.0 * PI),
            Some(attacker_pos) => {
                self.topple_direction = Coord2D { x: building_pos.x - attacker_pos.x, y: building_pos.y - attacker_pos.y };
                let a = self.topple_direction.to_angle();
                a + world.random.game_logic_random_value_real(-PI / 8.0, PI / 8.0)
            }
        };
        self.topple_direction = Coord2D { x: cos(topple_angle), y: sin(topple_angle) };
        // `adjustToppleDirection(object, Coord2D *)`: through a Coord3D whose z is never read back
        let mut dir = Coord3D { x: self.topple_direction.x, y: self.topple_direction.y, z: 0.0 };
        if !crate::logic::modules::plant(2069) {
            world.script_adjust_topple_direction(id, &mut dir);
        }
        self.topple_direction = Coord2D { x: dir.x, y: dir.y };
        let average_radius = (geom.get_major_radius() + geom.get_minor_radius()) / 2.0;
        let explosion_radius = (f64::from(average_radius) * if crate::logic::modules::plant(2061) { 0.95 } else { 0.90 }) as f32;
        self.delay_burst_location.x = building_pos.x + explosion_radius * cos(topple_angle);
        self.delay_burst_location.y = building_pos.y + explosion_radius * sin(topple_angle);
        self.delay_burst_location.z = world.terrain_get_ground_height(self.delay_burst_location.x, self.delay_burst_location.y);
        self.do_topple_start_fx(world, me, building_pos)?;
        let delay = if crate::logic::modules::plant(2067) { world.random.game_logic_random_value(d.min_topple_burst_delay, d.max_topple_burst_delay) } else { world.random.game_client_random_value(d.min_topple_burst_delay, d.max_topple_burst_delay) };
        self.next_burst_frame = (now as i32).wrapping_add(delay);
        self.topple_state = TOPPLESTATE_WAITINGFORTOPPLESTART;
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        Ok(())
    }

    /// `onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        let id = me.object;
        let Some(o) = world.objects.find(id) else { return Ok(()) };
        if !self.data.die.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status) {
            return Ok(());
        }
        if let Some(ai) = o.ai {
            world.module_call(id, ai, ModuleCall::AiMarkAsDead);
        }
        world.cut(CutCall::GameLogicDeselectObject { object: id, player_mask: PLAYERMASK_ALL, affect_client: true });
        self.begin_structure_topple(world, me, info)
    }

    /// `doToppleStartFX(building, damageInfo)`.
    fn do_topple_start_fx(&self, world: &mut World, me: ModuleHandle, building_pos: Coord3D) -> Result<(), Undefined> {
        if self.fx_allowed(world, me.object)? {
            Self::fx_pos(world, &self.data.topple_start_fx_list, building_pos);
        }
        self.do_phase_stuff(world, me, STPHASE_INITIAL, &building_pos);
        Ok(())
    }

    /// `doToppleDelayBurstFX()` (the bone particle systems: `fxbones` is never filled).
    fn do_topple_delay_burst_fx(&self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        if self.fx_allowed(world, me.object)? {
            Self::fx_pos(world, &self.data.topple_delay_fx_list, self.delay_burst_location);
        }
        self.do_phase_stuff(world, me, STPHASE_DELAY, &self.delay_burst_location);
        Ok(())
    }

    /// `doPhaseStuff(stphase, target)`: `m_oclCount` different entries of the phase's object creation lists, at random.
    fn do_phase_stuff(&self, world: &mut World, me: ModuleHandle, stphase: usize, target: &Coord3D) {
        let d = self.data.clone();
        let list_size = d.ocls[stphase].len();
        if list_size == 0 {
            return;
        }
        let mut idx_list = [0i32; MAX_IDX];
        let count = d.ocl_count[stphase] as usize;
        build_non_dup_random_index_list(world, list_size as i32, count, &mut idx_list);
        for &idx in &idx_list[..count] {
            if let Some(ocl) = d.ocls[stphase].get(idx as usize) {
                // `ObjectCreationList::create(NULL, ...)` makes nothing
                if world.statics.clone().object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some() {
                    world.cut(CutCall::OclCreateAtPrimary { primary: me.object, ocl: ocl.clone(), primary_pos: *target, angle: INVALID_ANGLE });
                }
            }
        }
    }

    /// `doAngleFX(curAngle, newAngle)`: the FX of the angles the fall passes this frame.
    fn do_angle_fx(&self, world: &mut World, id: ObjectId, cur_angle: f32, new_angle: f32) -> Result<(), Undefined> {
        let allowed = self.fx_allowed(world, id)?;
        for it in &self.data.angle_fx {
            if (it.angle > cur_angle || (crate::logic::modules::plant(2064) && it.angle == cur_angle) || crate::logic::modules::plant(2070)) && it.angle <= new_angle && allowed {
                Self::fx_obj(world, &it.fx_list, id);
            }
        }
        Ok(())
    }

    /// `applyCrushingDamage(theta)`.
    fn apply_crushing_damage(&mut self, world: &mut World, me: ModuleHandle, theta: f32) -> Result<(), Undefined> {
        if theta > THETA_CEILING {
            return Ok(());
        }
        let id = me.object;
        let Some((orientation_angle, geom)) = world.objects.find(id).map(|o| (o.thing.orientation(), o.geometry_info.clone())) else { return Ok(()) };
        let topple_angle = self.topple_direction.to_angle();
        let angle = orientation_angle - topple_angle;
        let minor_component = geom.get_minor_radius() * cos(angle);
        let major_component = geom.get_major_radius() * sin(angle);
        let facing_width = Coord3D { x: major_component, y: minor_component, z: 0.0 }.length() / 2.0;
        let name = &self.data.crushing_weapon_name;
        let Some(wt) = world.statics.weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(name)).map(|t| t.name.clone()) else {
            return Ok(());
        };
        let max_distance = (f64::from(self.building_height) * (1.0 - f64::from(sin(theta)))) as f32;
        // "Fire explosions at regular intervals across the area that the building is currently crushing."
        let mut j = self.last_crushed_location;
        while j < max_distance {
            let (jcos, jsin) = (j * cos(topple_angle), j * sin(topple_angle));
            self.do_damage_line(world, me, &wt, jcos, jsin, facing_width, topple_angle)?;
            j += if crate::logic::modules::plant(2065) { 20.0 } else { WEAPON_SPACING_PERPENDICULAR };
        }
        let (jcos, jsin) = (max_distance * cos(topple_angle), max_distance * sin(topple_angle));
        self.do_damage_line(world, me, &wt, jcos, jsin, facing_width, topple_angle)?;
        // (the loop's `j`, kept after it)
        self.last_crushed_location = if crate::logic::modules::plant(2066) { max_distance } else { j };
        Ok(())
    }

    /// `doDamageLine(building, wt, jcos, jsin, facingWidth, toppleAngle)`.
    #[allow(clippy::too_many_arguments)]
    fn do_damage_line(&self, world: &mut World, me: ModuleHandle, wt: &[u8], jcos: f32, jsin: f32, facing_width: f32, topple_angle: f32) -> Result<(), Undefined> {
        let id = me.object;
        let allowed = self.fx_allowed(world, id)?;
        let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
        let fire = |world: &mut World, x: f32, y: f32| {
            let target = Coord3D { x, y, z: world.terrain_get_ground_height(x, y) };
            world.cut(CutCall::WeaponCreateAndFireTemp { weapon: wt.to_vec(), source: id, position: target });
            if allowed {
                Self::fx_pos(world, &self.data.crushing_fx_list, target);
            }
        };
        let mut i = -facing_width;
        while i < facing_width {
            fire(world, pos.x + jcos + (i * sin(topple_angle)), pos.y + jsin + (i * cos(topple_angle)));
            i += WEAPON_SPACING_PARALLEL;
        }
        fire(world, pos.x + jcos + (facing_width * sin(topple_angle)), pos.y + jsin + (facing_width * cos(topple_angle)));
        let (x, y) = (pos.x + jcos, pos.y + jsin);
        let target = Coord3D { x, y, z: world.terrain_get_ground_height(x, y) };
        self.do_phase_stuff(world, me, STPHASE_FINAL, &target);
        Ok(())
    }

    /// `doToppleDoneStuff()`.
    fn do_topple_done_stuff(&self, world: &mut World, id: ObjectId) {
        if let Some(bfxu) = world.find_module(id, b"BoneFXUpdate") {
            world.module_call(id, bfxu, ModuleCall::BoneFxStopAll);
        }
        let orig_angle = world.objects.find(id).map_or(0.0, |o| o.thing.orientation());
        if !crate::logic::modules::plant(2068) {
            world.set_orientation(id, orig_angle);
        }
        let topple_angle = self.topple_direction.to_angle();
        let Some(mut xfrm) = world.objects.find(id).map(|o| *o.thing.transform_matrix()) else { return };
        xfrm.in_place_pre_rotate_z(topple_angle - orig_angle);
        world.set_transform_matrix(id, &xfrm);
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let d = self.data.clone();
        let id = me.object;
        if self.topple_state == TOPPLESTATE_STANDING {
            // DEBUG_CRASH(("hmm, what?"));
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // `getObject()->getBodyModule()->getLastDamageInfo()` (read here, used below)
        let done_fx = self.fx_allowed(world, id)?;
        if self.topple_state == TOPPLESTATE_WAITINGFORTOPPLESTART {
            let now = world.frame();
            if now >= self.next_burst_frame as u32 {
                self.do_topple_delay_burst_fx(world, me)?;
                self.next_burst_frame = (now as i32).wrapping_add(world.random.game_client_random_value(d.min_topple_burst_delay, d.max_topple_burst_delay));
            }
            if now >= self.topple_frame {
                self.topple_state = TOPPLESTATE_TOPPLING;
                self.structural_integrity = d.structural_integrity;
            }
        }
        if self.topple_state == TOPPLESTATE_TOPPLING {
            let now = world.frame();
            let topple_acceleration = (f64::from(if crate::logic::modules::plant(2062) { 0.03 } else { TOPPLE_ACCELERATION_FACTOR }) * (f64::from(sin(self.accumulated_angle)) * (1.0 - f64::from(self.structural_integrity)))) as f32;
            self.topple_velocity += topple_acceleration;
            if self.structural_integrity > 0.0 && !crate::logic::modules::plant(2063) {
                self.structural_integrity *= d.structural_decay;
                if self.structural_integrity < 0.0 {
                    self.structural_integrity = 0.0;
                }
            }
            self.do_angle_fx(world, id, self.accumulated_angle, self.accumulated_angle + self.topple_velocity)?;
            self.accumulated_angle += self.topple_velocity;
            self.apply_crushing_damage(world, me, PI / 2.0 - self.accumulated_angle)?;
            if self.accumulated_angle >= PI / 2.0 {
                self.topple_velocity -= self.accumulated_angle - PI / 2.0;
                self.accumulated_angle = PI / 2.0;
                self.topple_state = TOPPLESTATE_WAITINGFORDONE;
                self.apply_crushing_damage(world, me, 0.0)?;
                let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
                self.do_phase_stuff(world, me, STPHASE_FINAL, &pos);
                if done_fx {
                    Self::fx_obj(world, &d.topple_done_fx_list, id);
                }
                self.topple_frame = world.frame();
            }
            if now >= self.next_burst_frame as u32 {
                self.do_topple_delay_burst_fx(world, me)?;
                self.next_burst_frame = (now as i32).wrapping_add(world.random.game_client_random_value(d.min_topple_burst_delay, d.max_topple_burst_delay));
            }
            if let Some(mut xfrm) = world.objects.find(id).map(|o| *o.thing.transform_matrix()) {
                xfrm.in_place_pre_rotate_x(-self.topple_velocity * self.topple_direction.y);
                xfrm.in_place_pre_rotate_y(self.topple_velocity * self.topple_direction.x);
                world.set_transform_matrix(id, &xfrm);
            }
        }
        if self.topple_state == TOPPLESTATE_WAITINGFORDONE && self.topple_frame <= world.frame() {
            if !world.has_drawable(id) {
                return Err(Undefined::NullDrawable);
            }
            world.clear_model_condition_state(id, MODELCONDITION_RUBBLE);
            world.set_model_condition_state(id, MODELCONDITION_POST_COLLAPSE);
            let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
            world.module_call(id, body, ModuleCall::UpdateBodyParticleSystems);
            self.do_topple_done_stuff(world, id);
            self.topple_state = TOPPLESTATE_DONE;
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for StructureToppleUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(s) => s,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The constructor's world half: asleep, and the building's height from its geometry.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
        self.building_height = world.objects.find(me.object).map_or(0.0, |o| o.geometry_info.get_max_height_above_position());
    }
    /// `xfer`'s fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.topple_frame)?;
        xfer.xfer_coord2d(&mut self.topple_direction)?;
        let mut state = self.topple_state.to_le_bytes();
        xfer.xfer_user(&mut state)?;
        self.topple_state = i32::from_le_bytes(state);
        xfer.xfer_real(&mut self.topple_velocity)?;
        xfer.xfer_real(&mut self.accumulated_angle)?;
        xfer.xfer_real(&mut self.structural_integrity)?;
        xfer.xfer_real(&mut self.last_crushed_location)?;
        xfer.xfer_int(&mut self.next_burst_frame)?;
        xfer.xfer_coord3d(&mut self.delay_burst_location)
    }
    /// The die interface: `onDie` (its own die mux).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                if let Err(u) = self.on_die(world, me, &info) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::StructureToppleUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StructureToppleUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StructureToppleUpdate {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base, then the fields.
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

/// The factory's `NewModuleDataProc` for `StructureToppleUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StructureToppleUpdate(Arc::new(StructureToppleUpdateModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = StructureToppleUpdateModuleData::default();
        assert_eq!(d.structural_integrity, 0.1);
        assert_eq!(d.ocl_count, [1; ST_PHASE_COUNT]);
        let m = StructureToppleUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[5], 0.001f32.to_bits());
        assert_eq!(m.state_words()[8], u32::MAX);
    }
}
