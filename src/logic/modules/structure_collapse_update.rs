// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StructureCollapseUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StructureCollapseUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StructureCollapseUpdate`: how a building dies by falling into the ground. When the building dies (`onDie`, if the die mux applies), the AI is
//! marked dead, the building is deselected, the `INITIAL` phase plays, and after a random delay the collapse starts: the building shudders, plays
//! the `BURST` phase, then sinks (its height falls by a velocity that gravity, damped, speeds up), with a `BURST` or a `DELAY` phase at random
//! intervals, until its template's height is under the ground: the `FINAL` phase, the bone FX stop, the model becomes `POST_COLLAPSE`.
//!
//! # Where the world comes in
//!
//! - The module is an update module (asleep for ever until the building dies) and a die module ([`ModuleCall::OnDie`]).
//! - The shudder and the sinking are the drawable's instance matrix, which the client keeps: [`ObjectClientEvent::DrawableSetInstanceTranslation`]
//!   gives the translation upstream sets on it (the shudder is drawn from the client's random numbers, as upstream draws it).
//! - `FXList::doFXPos` is [`ObjectClientEvent::FxListDoFxPos`]; `ObjectCreationList::create(ocl, building, target, NULL, angle)` is
//!   [`CutCall::OclCreateAtPrimary`]; `deselectObject` is [`CutCall::GameLogicDeselectObject`]; `TheGlobalData->m_gravity` is
//!   `cuts.globals.gravity`.
//! - The AI's `markAsDead` is [`ModuleCall::AiMarkAsDead`]; the body's `updateBodyParticleSystems` is [`ModuleCall::UpdateBodyParticleSystems`];
//!   the object's `BoneFXUpdate` (`findUpdateModule`) gets [`ModuleCall::BoneFxStopAll`].
//!
//! # Upstream facts kept
//!
//! - `m_oclCount` and `m_fxCount` are never read from the INI: each phase plays exactly one entry of each list.
//! - The delays are `parseDurationUnsignedInt` into `Int` fields; the frames are `UnsignedInt + Int` (modular).
//! - `m_collapseVelocity -= m_gravity * (1.0 - m_collapseDamping)` is `double` arithmetic; `m_collapseVelocity` is never reset (a second death
//!   starts at the speed the first one ended at).
//! - The done test reads the template's geometry, not the object's; the collapse state is saved as the four bytes of the enum.
//! - The shudder's two random numbers are the arguments of one call (`Vector3::Set`), whose order of evaluation C++ leaves unspecified; this
//!   translation draws them in the order the reference build (clang) does: left to right, `x` first. They are the client's random numbers, so the game logic is the same
//!   in either order.
//!
//! # Where upstream is undefined
//!
//! `m_maxBurstDelay` is the one field the data's constructor leaves alone: reading it when the INI did not give it is
//! [`Undefined::UninitialisedModuleData`]. A building without a drawable (`getDrawable()->getInstanceMatrix()`, [`Undefined::NullDrawable`]) or,
//! when it is down, without a body ([`Undefined::NullBodyModule`]). The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R29, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::PLAYERMASK_ALL;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::model_state::{MODELCONDITION_POST_COLLAPSE, MODELCONDITION_RUBBLE};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::lend_clone;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `enum StructureCollapsePhaseType`: `SCPHASE_INITIAL`.
pub const SCPHASE_INITIAL: usize = 0;
/// `SCPHASE_DELAY`.
pub const SCPHASE_DELAY: usize = 1;
/// `SCPHASE_BURST`.
pub const SCPHASE_BURST: usize = 2;
/// `SCPHASE_FINAL`.
pub const SCPHASE_FINAL: usize = 3;
/// `SC_PHASE_COUNT`.
pub const SC_PHASE_COUNT: usize = 4;
/// `TheStructureCollapsePhaseNames`.
const PHASE_NAMES: [&str; 4] = ["INITIAL", "DELAY", "BURST", "FINAL"];
/// `MAX_IDX`: the size of `doPhaseStuff`'s index list.
pub(crate) const MAX_IDX: usize = 32;

/// `enum StructureCollapseStateType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CollapseState {
    /// `COLLAPSESTATE_STANDING`.
    #[default]
    Standing = 0,
    /// `COLLAPSESTATE_WAITINGFORCOLLAPSESTART`.
    WaitingForCollapseStart = 1,
    /// `COLLAPSESTATE_COLLAPSING`.
    Collapsing = 2,
    /// `COLLAPSESTATE_DONE`.
    Done = 3,
}

/// `class StructureCollapseUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct StructureCollapseUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_dieMuxData`, in a `DieModuleData` (the same pair; the behaviour base of it is unused).
    pub die: DieModuleData,
    /// `m_minCollapseDelay`.
    pub min_collapse_delay: i32,
    /// `m_maxCollapseDelay`.
    pub max_collapse_delay: i32,
    /// `m_minBurstDelay`.
    pub min_burst_delay: i32,
    /// `m_maxBurstDelay` (`None`: the constructor leaves it alone, and reading it is undefined).
    pub max_burst_delay: Option<i32>,
    /// `m_bigBurstFrequency`.
    pub big_burst_frequency: i32,
    /// `m_collapseDamping`.
    pub collapse_damping: f32,
    /// `m_maxShudder`.
    pub max_shudder: f32,
    /// `m_ocls[phase]`: the object creation list names, every entry the line gave (found in the store when used; "could be null! this is OK!").
    pub ocls: [Vec<Vec<u8>>; SC_PHASE_COUNT],
    /// `m_fxs[phase]`: the FX list names (`None` for `NULL`).
    pub fxs: [Vec<Option<Vec<u8>>>; SC_PHASE_COUNT],
    /// `m_oclCount[phase]` (1, which nothing changes).
    pub ocl_count: [u32; SC_PHASE_COUNT],
    /// `m_fxCount[phase]` (1, which nothing changes).
    pub fx_count: [u32; SC_PHASE_COUNT],
}

impl Default for StructureCollapseUpdateModuleData {
    /// `StructureCollapseUpdateModuleData()`.
    fn default() -> Self {
        StructureCollapseUpdateModuleData {
            update: UpdateModuleData::new(),
            die: DieModuleData::new(),
            min_collapse_delay: 0,
            max_collapse_delay: 0,
            min_burst_delay: 9999,
            max_burst_delay: None,
            big_burst_frequency: 0,
            collapse_damping: 0.0,
            max_shudder: 0.0,
            ocls: Default::default(),
            fxs: Default::default(),
            // "init to one, so that if these are omitted, we choose exactly one of each."
            ocl_count: [1; SC_PHASE_COUNT],
            fx_count: [1; SC_PHASE_COUNT],
        }
    }
}

/// The phase a list line starts with: `INI::scanIndexList(ini->getNextToken(), TheStructureCollapsePhaseNames)`.
fn parse_phase(ini: &mut Ini<'_>) -> IniResult<usize> {
    let token = ini.get_next_token(None)?;
    Ok(Ini::scan_index_list(&token, &PHASE_NAMES)? as usize)
}

/// Lifts entry `k` of the layer-5 die field table onto `StructureCollapseUpdateModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut StructureCollapseUpdateModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `dataFieldParse`, then `DieMuxData::getFieldParse()`.
static FIELD_PARSE: [FieldParse<StructureCollapseUpdateModuleData>; 14] = [
    FieldParse::new("MinCollapseDelay", |i, d, _| { d.min_collapse_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MaxCollapseDelay", |i, d, _| { d.max_collapse_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MinBurstDelay", |i, d, _| { d.min_burst_delay = i.parse_duration_unsigned_int()? as i32; Ok(()) }, UserData::None),
    FieldParse::new("MaxBurstDelay", |i, d, _| { d.max_burst_delay = Some(i.parse_duration_unsigned_int()? as i32); Ok(()) }, UserData::None),
    FieldParse::new("CollapseDamping", |i, d, _| { d.collapse_damping = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxShudder", |i, d, _| { d.max_shudder = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("BigBurstFrequency", |i, d, _| { d.big_burst_frequency = i.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("OCL", |i, d, _| {
        let phase = parse_phase(i)?;
        // `for (token = ini->getNextToken(); token != NULL; token = ini->getNextTokenOrNull())`: the first token is required
        let mut token = Some(i.get_next_token(None)?);
        while let Some(t) = token {
            d.ocls[phase].push(t);
            token = i.get_next_token_or_null(None);
        }
        Ok(())
    }, UserData::None),
    FieldParse::new("FXList", |i, d, _| {
        let phase = parse_phase(i)?;
        let mut token = Some(i.get_next_token(None)?);
        while let Some(t) = token {
            // `findFXList(token)`: "could be null! this is OK!"
            d.fxs[phase].push(if t.eq_ignore_ascii_case(b"None") { None } else { Some(t) });
            token = i.get_next_token_or_null(None);
        }
        Ok(())
    }, UserData::None),
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::end(),
];

impl StructureCollapseUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields and the die mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, StructureCollapseUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<StructureCollapseUpdateModuleData> {
        let mut data = StructureCollapseUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StructureCollapseUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StructureCollapseUpdate : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct StructureCollapseUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getStructureCollapseUpdateModuleData()`.
    data: Arc<StructureCollapseUpdateModuleData>,
    /// `m_collapseFrame`.
    collapse_frame: u32,
    /// `m_burstFrame`.
    burst_frame: u32,
    /// `m_collapseState`.
    collapse_state: CollapseState,
    /// `m_collapseVelocity`.
    collapse_velocity: f32,
    /// `m_currentHeight`.
    current_height: f32,
}

impl PartialEq for StructureCollapseUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

/// `buildNonDupRandomIndexList(range, count, idxList)`: `count` different indices below `range`, each drawn until it is new (also
/// `StructureToppleUpdate.cpp`'s copy, batch R36).
pub(crate) fn build_non_dup_random_index_list(world: &mut World, range: i32, count: usize, idx_list: &mut [i32; MAX_IDX]) {
    for i in 0..count {
        let idx = loop {
            let idx = world.random.game_logic_random_value(0, range - 1);
            if !idx_list[..i].contains(&idx) {
                break idx;
            }
        };
        idx_list[i] = idx;
    }
}

impl StructureCollapseUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<StructureCollapseUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(StructureCollapseUpdate {
            update,
            data,
            collapse_frame: 0,
            burst_frame: 0,
            collapse_state: CollapseState::Standing,
            collapse_velocity: 0.0,
            current_height: 0.0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getStructureCollapseUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<StructureCollapseUpdateModuleData> {
        &self.data
    }

    /// The state `xfer` writes, as words: the two frames, the state, the velocity and the height.
    pub fn state_words(&self) -> [u32; 5] {
        [self.collapse_frame, self.burst_frame, self.collapse_state as u32, self.collapse_velocity.to_bits(), self.current_height.to_bits()]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: asleep until the building dies.
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

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// Records that upstream is undefined here.
    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    /// `isDieApplicable(damageInfo)`: the die mux's, of this object's death.
    fn is_die_applicable(&self, world: &World, me: ModuleHandle, info: &DamageInfo) -> bool {
        let Some(o) = world.objects.find(me.object) else { return false };
        self.data.die.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status)
    }

    /// `GameLogicRandomValue(m_minBurstDelay, m_maxBurstDelay)`.
    fn random_burst_delay(&self, world: &mut World) -> Result<i32, Undefined> {
        let max = self.data.max_burst_delay.ok_or(Undefined::UninitialisedModuleData)?;
        Ok(world.random.game_logic_random_value(self.data.min_burst_delay, max))
    }

    /// `beginStructureCollapse(damageInfo)`.
    fn begin_structure_collapse(&mut self, world: &mut World, me: ModuleHandle) {
        let d = self.data.clone();
        let now = world.frame();
        // "This has to use a game logic random value since the bursts can spawn debris, and debris is sync'd."
        self.collapse_frame = now.wrapping_add_signed(if crate::logic::modules::plant(1992) { d.min_collapse_delay } else { world.random.game_logic_random_value(d.min_collapse_delay, d.max_collapse_delay) });
        let pos = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
        self.do_phase_stuff(world, me, if crate::logic::modules::plant(1999) { SCPHASE_BURST } else { SCPHASE_INITIAL }, &pos);
        self.collapse_state = CollapseState::WaitingForCollapseStart;
        self.current_height = 0.0;
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }

    /// `onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.is_die_applicable(world, me, info) {
            return;
        }
        let obj = me.object;
        if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai).filter(|_| !crate::logic::modules::plant(1991)) {
            world.module_call(obj, ai, ModuleCall::AiMarkAsDead);
        }
        // deselect this object for all players.
        world.cut(CutCall::GameLogicDeselectObject { object: obj, player_mask: PLAYERMASK_ALL, affect_client: true });
        self.begin_structure_collapse(world, me);
    }

    /// `doPhaseStuff(scphase, target)`: `m_fxCount` entries of the FX lists and `m_oclCount` of the object creation lists, different ones, at random.
    fn do_phase_stuff(&self, world: &mut World, me: ModuleHandle, scphase: usize, target: &Coord3D) {
        let d = self.data.clone();
        let mut idx_list = [0i32; MAX_IDX];
        let list_size = d.fxs[scphase].len();
        if list_size > 0 {
            let count = d.fx_count[scphase] as usize;
            build_non_dup_random_index_list(world, list_size as i32, count, &mut idx_list);
            for &idx in &idx_list[..count] {
                if let Some(Some(fx)) = d.fxs[scphase].get(idx as usize) {
                    world.emit(ObjectClientEvent::FxListDoFxPos { fx: fx.clone(), position: *target });
                }
            }
        }
        let list_size = d.ocls[scphase].len();
        if list_size > 0 {
            let count = d.ocl_count[scphase] as usize;
            build_non_dup_random_index_list(world, list_size as i32, count, &mut idx_list);
            for &idx in &idx_list[..count] {
                if let Some(ocl) = d.ocls[scphase].get(idx as usize) {
                    // `ObjectCreationList::create(NULL, ...)` makes nothing
                    let found = world.statics.clone().object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some();
                    if found {
                        let angle = world.objects.find(me.object).map_or(0.0, |o| o.thing.orientation());
                        world.cut(CutCall::OclCreateAtPrimary { primary: me.object, ocl: ocl.clone(), primary_pos: *target, angle });
                    }
                }
            }
        }
    }

    /// `doCollapseDoneStuff()`: the object's `BoneFXUpdate` stops its bone FX.
    fn do_collapse_done_stuff(world: &mut World, obj: ObjectId) {
        if let Some(bfxu) = world.find_module(obj, b"BoneFXUpdate").filter(|_| !crate::logic::modules::plant(1994)) {
            world.module_call(obj, bfxu, ModuleCall::BoneFxStopAll);
        }
    }

    /// The shudder: `GameClientRandomValueReal(-m_maxShudder, m_maxShudder)` for `x` and `y` (drawn `x` first, see the module docs), `z` given.
    fn shudder(world: &mut World, max_shudder: f32, z: f32) -> Coord3D {
        let x = world.random.game_client_random_value_real(-max_shudder, max_shudder);
        let y = world.random.game_client_random_value_real(-max_shudder, max_shudder);
        if crate::logic::modules::plant(1998) {
            return Coord3D { x: y, y: x, z };
        }
        Coord3D { x, y, z }
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        // static const Real COLLAPSE_ACCELERATION_FACTOR = 0.02f; (unused)
        let d = self.data.clone();
        let id = me.object;
        if self.collapse_state == CollapseState::Standing {
            // DEBUG_CRASH(("hmm, what?"));
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // "We are in the dramatic pause between when the building has lost all its hit points and when it starts toppling over."
        if self.collapse_state == CollapseState::WaitingForCollapseStart {
            let now = world.frame();
            let current_position = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
            let shudder = Self::shudder(world, d.max_shudder, 0.0);
            if !world.has_drawable(id) {
                return Err(Undefined::NullDrawable);
            }
            world.emit(ObjectClientEvent::DrawableSetInstanceTranslation { object_id: id, translation: shudder });
            if now >= self.collapse_frame {
                self.collapse_state = CollapseState::Collapsing;
                self.do_phase_stuff(world, me, SCPHASE_BURST, &current_position);
                self.burst_frame = now.wrapping_add_signed(self.random_burst_delay(world)?);
            }
        }
        // "The building is in the process of falling over."
        if self.collapse_state == CollapseState::Collapsing {
            let now = world.frame();
            self.current_height -= self.collapse_velocity;
            self.collapse_velocity = (f64::from(self.collapse_velocity) - f64::from(world.cuts.globals.gravity) * (1.0 - if crate::logic::modules::plant(1993) { 0.0 } else { f64::from(d.collapse_damping) })) as f32;
            let current_position = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
            let shudder = Self::shudder(world, d.max_shudder, self.current_height);
            if !world.has_drawable(id) {
                return Err(Undefined::NullDrawable);
            }
            world.emit(ObjectClientEvent::DrawableSetInstanceTranslation { object_id: id, translation: shudder });
            if now >= self.burst_frame {
                if (world.random.game_logic_random_value(1, d.big_burst_frequency) == 1) != crate::logic::modules::plant(1995) {
                    self.do_phase_stuff(world, me, SCPHASE_BURST, &current_position);
                } else {
                    self.do_phase_stuff(world, me, SCPHASE_DELAY, &current_position);
                }
                self.burst_frame = if crate::logic::modules::plant(1996) { now } else { self.burst_frame }.wrapping_add_signed(self.random_burst_delay(world)?);
            }
            let max_height = world.objects.find(id).map_or(0.0, |o| o.thing.template().get_template_geometry_info().get_max_height_above_position());
            if self.current_height + max_height <= 0.0 {
                self.collapse_state = CollapseState::Done;
                let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
                self.do_phase_stuff(world, me, SCPHASE_FINAL, &pos);
                Self::do_collapse_done_stuff(world, id);
                world.clear_model_condition_state(id, MODELCONDITION_RUBBLE);
                world.set_model_condition_state(id, MODELCONDITION_POST_COLLAPSE);
                let angle = world.objects.find(id).map_or(0.0, |o| o.thing.orientation());
                world.set_orientation(id, angle);
                // "Need to update body particle systems, now"
                let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
                world.module_call(id, body, ModuleCall::UpdateBodyParticleSystems);
                world.emit(ObjectClientEvent::DrawableSetInstanceTranslation { object_id: id, translation: Coord3D::default() });
                return Ok(UPDATE_SLEEP_FOREVER);
            }
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for StructureCollapseUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.update_inner(world, me) {
            Ok(s) => s,
            Err(u) => {
                Self::undefined(world, me, u);
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// The fields after the base, in order; the state as the four bytes of the enum.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.collapse_frame)?;
        xfer.xfer_unsigned_int(&mut self.burst_frame)?;
        let mut state = (self.collapse_state as i32).to_le_bytes();
        xfer.xfer_user(&mut state)?;
        self.collapse_state = match i32::from_le_bytes(state) {
            0 => CollapseState::Standing,
            1 => CollapseState::WaitingForCollapseStart,
            2 => CollapseState::Collapsing,
            _ => CollapseState::Done,
        };
        xfer.xfer_real(&mut self.collapse_velocity)?;
        if crate::logic::modules::plant(1997) {
            return Ok(());
        }
        xfer.xfer_real(&mut self.current_height)
    }
    /// The die interface.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                lend_clone(world, me, self, UpdateBody::StructureCollapseUpdate, Self::from_body, |w, m| m.on_die(w, me, &info));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::StructureCollapseUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StructureCollapseUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StructureCollapseUpdate {
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

/// The factory's `NewModuleDataProc` for `StructureCollapseUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StructureCollapseUpdate(Arc::new(StructureCollapseUpdateModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = StructureCollapseUpdateModuleData::default();
        assert_eq!((d.min_burst_delay, d.max_burst_delay, d.ocl_count, d.fx_count), (9999, None, [1; 4], [1; 4]));
        let m = StructureCollapseUpdate::unconstructed(3, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), [0, 0, 0, 0, 0]);
        assert_eq!(StructureCollapseUpdate::unconstructed(3, None).unwrap_err().kind, IniErrorKind::InvalidData);
    }
}
