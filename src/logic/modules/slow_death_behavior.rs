// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SlowDeathBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SlowDeathBehavior`: how an object dies when it takes its time over it. When the object dies (`onDie`), the slow death modules of its object roll between
//! themselves (each has a probability modifier, raised by the overkill) and the winner begins its slow death: it sinks into the ground after a delay, plays
//! FX lists, makes object creation lists and fires temporary weapons at the start, the midpoint and the end, and destroys the object when its time is up. A
//! module with a fling force throws the corpse into the air with physics first (and the unit bounces, tumbles and is destroyed after its delay).
//!
//! # Where the world comes in
//!
//! - The module is an update module (asleep for ever until the slow death begins), a die module and the `SlowDeathBehaviorInterface`
//!   ([`ModuleCall::BeginSlowDeath`], [`ModuleCall::GetProbabilityModifier`], [`ModuleCall::IsDieApplicable`], which the modules of one object send each other).
//! - `Object::setDisabled` and `TheGameLogic->destroyObject` are `World::body_set_disabled` and `World::body_destroy_object` (the logged cuts of `body_cuts`, which
//!   run the real calls unless the test ablates them).
//! - `TheGameLODManager->getSlowDeathScale()` is `cuts.globals.slow_death_scale`; `TheGameLogic->getHulkMaxLifetimeOverride()` is
//!   `cuts.globals.hulk_max_lifetime_override`; `deselectObject` is [`CutCall::GameLogicDeselectObject`].
//! - The physics module's calls are [`ModuleCall::PhysicsSetAllowToFall`], `PhysicsApplyForce`, `PhysicsSetExtraBounciness`, `PhysicsSetExtraFriction`,
//!   `PhysicsSetAllowBouncing`, `PhysicsSetAngles` and `PhysicsGetLastCollidee`; the AI's, [`ModuleCall::AiIsAiInDeadState`] and `AiMarkAsDead`; the slaver's,
//!   `SlavedOnSlaverDie`. `ObjectCreationList::create` and `createAndFireTempWeapon` are the cut points `OclCreate` and `WeaponCreateAndFireTemp`.
//! - The drawable's shadows are [`ObjectClientEvent::DrawableSetShadowsEnabled`].
//!
//! # Deviation: the lists are found when the object is made
//!
//! Upstream's INI parsers look each FX list, object creation list and weapon template up in its store as the data is read, keep the pointer (`NULL` for a name
//! the store does not have: "could be null! this is OK!") and set `m_maskOfLoadedEffects` for each non-null one. This translation keeps the names (the stores
//! are not at the data parser's reach) and finds them in [`GameStatics`](crate::logic::world::statics::GameStatics) when the module is made
//! ([`SlowDeathBehavior::world_construct`]): the mask is the module's, not the data's. The stores do not change while a game runs, so the result is the same. An
//! FX list is found when its name is not `None` (the client owns the FX list store; see `docs/world-design.md`).
//!
//! # Upstream facts kept
//!
//! - `ProbabilityModifier < 1` throws `INI_INVALID_DATA` from the constructor.
//! - The frame arithmetic is `UnsignedInt` / `Real` arithmetic: `m_sinkFrame = timeScale * (sinkDelay + GameLogicRandomValue(0, variance))` adds an `Int` to an
//!   `UnsignedInt` (modular), converts the sum to `Real`, multiplies and converts back (truncating). The midpoint is `GameLogicRandomValue(Int(0.35 * dest), Int(0.65 * dest))`
//!   of a `Real` truncated to `Int`.
//! - `whenToWakeTime` is an `Int` compared to `UnsignedInt`s: the comparison is unsigned.
//! - The random draws of `doPhaseStuff` happen for a `NULL` entry as for any other.
//! - `beginSlowDeath` returns before it sets anything when the slow death scale is 0 and the module has no OCL or weapon: the object is destroyed on the spot.
//! - `update()` of a flung object adds one to each of the three frames every frame it is in the air, so that its timers only run once it has bounced.
//! - `m_acceleratedTimeScale` is only changed by `update()` for a module that has no OCL or weapon.
//!
//! # Where upstream is undefined
//!
//! A frame computed from a `Real` outside `UnsignedInt` ([`Undefined::FloatToUnsignedOutOfRange`]) or an `Int` ([`Undefined::FloatToIntOutOfRange`]); the drawable
//! of a flung object that has no drawable (`obj->getDrawable()->setModelConditionState`, [`Undefined::NullDrawable`]). The case is recorded in
//! `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, 2026-10-22.

use std::sync::Arc;

use crate::common::base_type::{Coord3D, PI};
use crate::common::damage_fx::FxListName;
use crate::common::disabled_types::DISABLED_HELD;
use crate::common::game_common::{LOGICFRAMES_PER_SECOND, SECONDS_PER_LOGICFRAME_REAL};
use crate::common::game_type::ObjectId;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_HULK, KINDOF_INFANTRY, KINDOF_SHRUBBERY};
use crate::common::model_state::{MODELCONDITION_EXPLODED_BOUNCING, MODELCONDITION_EXPLODED_FLAILING, MODELCONDITION_PARACHUTING};
use crate::common::module::{ModuleDataKind, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::module::update_module::{update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::lend_clone;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::ww::matrix3d::Matrix3D;

/// `BEGIN_MIDPOINT_RATIO`.
const BEGIN_MIDPOINT_RATIO: f32 = 0.35;
/// `END_MIDPOINT_RATIO`.
const END_MIDPOINT_RATIO: f32 = 0.65;

/// `enum SlowDeathPhaseType`.
pub const SDPHASE_INITIAL: usize = 0;
/// `SDPHASE_MIDPOINT`.
pub const SDPHASE_MIDPOINT: usize = 1;
/// `SDPHASE_FINAL`.
pub const SDPHASE_FINAL: usize = 2;
/// `SD_PHASE_COUNT`.
pub const SD_PHASE_COUNT: usize = 3;
/// `TheSlowDeathPhaseNames`.
const PHASE_NAMES: [&str; 3] = ["INITIAL", "MIDPOINT", "FINAL"];

/// The bits of `m_flags`: `SLOW_DEATH_ACTIVATED`, `MIDPOINT_EXECUTED`, `FLUNG_INTO_AIR`, `BOUNCED`.
const SLOW_DEATH_ACTIVATED: u32 = 0;
const MIDPOINT_EXECUTED: u32 = 1;
const FLUNG_INTO_AIR: u32 = 2;
const BOUNCED: u32 = 3;

/// `class SlowDeathBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug)]
pub struct SlowDeathBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_dieMuxData`, in a `DieModuleData` (the same pair; the behaviour base of it is unused).
    pub die: DieModuleData,
    /// `m_sinkRate`.
    pub sink_rate: f32,
    /// `m_probabilityModifier`.
    pub probability_modifier: i32,
    /// `m_modifierBonusPerOverkillPercent`.
    pub modifier_bonus_per_overkill_percent: f32,
    /// `m_sinkDelay`.
    pub sink_delay: u32,
    /// `m_sinkDelayVariance`.
    pub sink_delay_variance: u32,
    /// `m_destructionAltitude` (read, never used).
    pub destruction_altitude: f32,
    /// `m_destructionDelay`.
    pub destruction_delay: u32,
    /// `m_destructionDelayVariance`.
    pub destruction_delay_variance: u32,
    /// `m_fx[phase]`: the FX list names (`None` for `NULL`).
    pub fx: [Vec<FxListName>; SD_PHASE_COUNT],
    /// `m_ocls[phase]`: the object creation list names, every entry the line gave (found in the store when the module is made).
    pub ocls: [Vec<Vec<u8>>; SD_PHASE_COUNT],
    /// `m_weapons[phase]`: the weapon template names.
    pub weapons: [Vec<Vec<u8>>; SD_PHASE_COUNT],
    /// `m_flingForce`.
    pub fling_force: f32,
    /// `m_flingForceVariance`.
    pub fling_force_variance: f32,
    /// `m_flingPitch`.
    pub fling_pitch: f32,
    /// `m_flingPitchVariance`.
    pub fling_pitch_variance: f32,
}

impl Default for SlowDeathBehaviorModuleData {
    /// `SlowDeathBehaviorModuleData()`.
    fn default() -> Self {
        SlowDeathBehaviorModuleData {
            update: UpdateModuleData::new(),
            die: DieModuleData::new(),
            sink_rate: 0.0,
            probability_modifier: 10,
            modifier_bonus_per_overkill_percent: 0.0,
            sink_delay: 0,
            sink_delay_variance: 0,
            destruction_altitude: -10.0,
            destruction_delay: 0,
            destruction_delay_variance: 0,
            fx: Default::default(),
            ocls: Default::default(),
            weapons: Default::default(),
            fling_force: 0.0,
            fling_force_variance: 0.0,
            fling_pitch: 0.0,
            fling_pitch_variance: 0.0,
        }
    }
}

/// The phase a list line starts with: `INI::scanIndexList(ini->getNextToken(), TheSlowDeathPhaseNames)`.
fn parse_phase(ini: &mut Ini<'_>) -> IniResult<usize> {
    let token = ini.get_next_token(None)?;
    Ok(Ini::scan_index_list(&token, &PHASE_NAMES)? as usize)
}

/// Lifts entry `k` of the layer-5 die field table onto `SlowDeathBehaviorModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut SlowDeathBehaviorModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// The tokens of `dataFieldParse`, then `DieMuxData::getFieldParse()`'s, in order (entry `k` of [`slow_death_field`]).
pub const SLOW_DEATH_FIELD_TOKENS: [&str; 19] = [
    "SinkRate",
    "ProbabilityModifier",
    "ModifierBonusPerOverkillPercent",
    "SinkDelay",
    "SinkDelayVariance",
    "DestructionDelay",
    "DestructionDelayVariance",
    "DestructionAltitude",
    "FX",
    "OCL",
    "Weapon",
    "FlingForce",
    "FlingForceVariance",
    "FlingPitch",
    "FlingPitchVariance",
    "DeathTypes",
    "VeterancyLevels",
    "ExemptStatus",
    "RequiredStatus",
];

/// Entry `k` of `dataFieldParse` (then `DieMuxData::getFieldParse()`), on the `SlowDeathBehaviorModuleData` `d` (a derived data's base, batch
/// R46: see [`slow_death_field_parse!`](crate::slow_death_field_parse)).
pub fn slow_death_field(k: usize, i: &mut Ini<'_>, d: &mut SlowDeathBehaviorModuleData, u: &UserData) -> IniResult<()> {
    match k {
        0 => d.sink_rate = i.parse_velocity_real()?,
        1 => d.probability_modifier = i.parse_int()?,
        2 => d.modifier_bonus_per_overkill_percent = i.parse_percent_to_real()?,
        3 => d.sink_delay = i.parse_duration_unsigned_int()?,
        4 => d.sink_delay_variance = i.parse_duration_unsigned_int()?,
        5 => d.destruction_delay = i.parse_duration_unsigned_int()?,
        6 => d.destruction_delay_variance = i.parse_duration_unsigned_int()?,
        7 => d.destruction_altitude = i.parse_real()?,
        8 => {
            let phase = parse_phase(i)?;
            // `for (token = ini->getNextToken(); token != NULL; token = ini->getNextTokenOrNull())`: the first token is required
            let mut token = Some(i.get_next_token(None)?);
            while let Some(t) = token {
                // `findFXList(token)`: "could be null! this is OK!"
                d.fx[phase].push(if t.eq_ignore_ascii_case(b"None") { None } else { Some(t) });
                token = i.get_next_token_or_null(None);
            }
        }
        9 | 10 => {
            let phase = parse_phase(i)?;
            let mut token = Some(i.get_next_token(None)?);
            while let Some(t) = token {
                if k == 9 { d.ocls[phase].push(t) } else { d.weapons[phase].push(t) }
                token = i.get_next_token_or_null(None);
            }
        }
        11 => d.fling_force = i.parse_real()?,
        12 => d.fling_force_variance = i.parse_real()?,
        13 => d.fling_pitch = i.parse_angle_real()?,
        14 => d.fling_pitch_variance = i.parse_angle_real()?,
        _ => return lift_die_field(k - 15, i, d, u),
    }
    Ok(())
}

/// `SlowDeathBehaviorModuleData::buildFieldParse`'s table (`dataFieldParse`, then `DieMuxData::getFieldParse()`) for a data type `$ty`
/// whose `SlowDeathBehaviorModuleData` the function `$acc` gives (`std::convert::identity` for the class itself; a derived class's base
/// for a derived data, batch R46).
#[macro_export]
macro_rules! slow_death_field_parse {
    ($ty:ty, $acc:path) => {{
        use $crate::common::ini::{FieldParse, UserData};
        use $crate::logic::modules::slow_death_behavior::{slow_death_field, SLOW_DEATH_FIELD_TOKENS as T};
        [
            FieldParse::<$ty>::new(T[0], |i, d, u| slow_death_field(0, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[1], |i, d, u| slow_death_field(1, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[2], |i, d, u| slow_death_field(2, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[3], |i, d, u| slow_death_field(3, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[4], |i, d, u| slow_death_field(4, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[5], |i, d, u| slow_death_field(5, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[6], |i, d, u| slow_death_field(6, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[7], |i, d, u| slow_death_field(7, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[8], |i, d, u| slow_death_field(8, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[9], |i, d, u| slow_death_field(9, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[10], |i, d, u| slow_death_field(10, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[11], |i, d, u| slow_death_field(11, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[12], |i, d, u| slow_death_field(12, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[13], |i, d, u| slow_death_field(13, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[14], |i, d, u| slow_death_field(14, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[15], |i, d, u| slow_death_field(15, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[16], |i, d, u| slow_death_field(16, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[17], |i, d, u| slow_death_field(17, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::new(T[18], |i, d, u| slow_death_field(18, i, $acc(d), u), UserData::None),
            FieldParse::<$ty>::end(),
        ]
    }};
}

/// `dataFieldParse`, then `DieMuxData::getFieldParse()`.
static FIELD_PARSE: [FieldParse<SlowDeathBehaviorModuleData>; 20] = crate::slow_death_field_parse!(SlowDeathBehaviorModuleData, std::convert::identity);

impl SlowDeathBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields and the die mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SlowDeathBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SlowDeathBehaviorModuleData> {
        let mut data = SlowDeathBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SlowDeathBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class SlowDeathBehavior : public UpdateModule, public DieModuleInterface, public SlowDeathBehaviorInterface`.
#[derive(Clone, Debug)]
pub struct SlowDeathBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getSlowDeathBehaviorModuleData()`.
    data: Arc<SlowDeathBehaviorModuleData>,
    /// `m_sinkFrame`: "frame to be sunken into the ground on".
    sink_frame: u32,
    /// `m_midpointFrame`.
    midpoint_frame: u32,
    /// `m_destructionFrame`.
    destruction_frame: u32,
    /// `m_acceleratedTimeScale`: "used to speedup deaths when needed to improve game performance".
    accelerated_time_scale: f32,
    /// `m_flags`.
    flags: u32,
    /// Whether the data has an FX list, an OCL, a weapon (`m_maskOfLoadedEffects` & `HAS_FX`, `HAS_OCL`, `HAS_WEAPON`), found when the module is made.
    has_fx: bool,
    /// See `has_fx`.
    has_ocl: bool,
    /// See `has_fx`.
    has_weapon: bool,
}

impl PartialEq for SlowDeathBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same state (the `Real` by its bits): [`lend_clone`] and
    /// [`update_behavior::world_update`] keep the module the call changed, and take the lent copy when the call did not.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.sink_frame == other.sink_frame
            && self.midpoint_frame == other.midpoint_frame
            && self.destruction_frame == other.destruction_frame
            && self.accelerated_time_scale.to_bits() == other.accelerated_time_scale.to_bits()
            && self.flags == other.flags
            && self.has_fx == other.has_fx
            && self.has_ocl == other.has_ocl
            && self.has_weapon == other.has_weapon
    }
}

/// `UnsignedInt = Real`: undefined outside (-1, 2^32).
fn real_to_unsigned(x: f32) -> Option<u32> {
    if x > -1.0 && x < 4_294_967_296.0 {
        Some(x as u32)
    } else {
        None
    }
}

/// `Int = Real`: undefined outside (-2^31 - 1, 2^31).
fn real_to_signed(x: f32) -> Option<i32> {
    if x > -2_147_483_904.0 && x < 2_147_483_648.0 {
        Some(x as i32)
    } else {
        None
    }
}

impl SlowDeathBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data, and for a `ProbabilityModifier` under 1).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<SlowDeathBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        if data.probability_modifier < 1 {
            // DEBUG_CRASH(("ProbabilityModifer must be >= 1.\n"));
            return Err(IniError::new(IniErrorKind::InvalidData));
        }
        Ok(SlowDeathBehavior {
            update,
            data,
            sink_frame: 0,
            midpoint_frame: 0,
            destruction_frame: 0,
            accelerated_time_scale: 1.0,
            flags: 0,
            has_fx: false,
            has_ocl: false,
            has_weapon: false,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The `UpdateModule` base, to put back the scheduling half a derived class's xfer used (batch R46).
    pub(crate) fn update_module_mut(&mut self) -> &mut UpdateModule {
        &mut self.update
    }

    /// The module data (`getSlowDeathBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<SlowDeathBehaviorModuleData> {
        &self.data
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: the wake call, and the lists found (see the module docs).
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        self.find_effects(world);
        update_behavior::world_construct(self, world, me);
    }

    /// The lists of the data, found in the stores (see the module docs): the first half of [`SlowDeathBehavior::world_construct`], for a derived
    /// class's (batch R46).
    pub(crate) fn find_effects(&mut self, world: &mut World) {
        let statics = world.statics.clone();
        let d = self.data.clone();
        self.has_fx = d.fx.iter().any(|v| v.iter().any(|f| f.is_some()));
        self.has_ocl = d.ocls.iter().any(|v| v.iter().any(|n| statics.object_creation_list_store.find_object_creation_list(&mut world.names, n).is_some()));
        // (`TheWeaponStore->findWeaponTemplate(name)`: a template of that name, whatever the case)
        self.has_weapon = d.weapons.iter().any(|v| v.iter().any(|n| statics.weapon_store.templates().iter().any(|t| t.name.eq_ignore_ascii_case(n))));
    }

    /// The state `xfer` writes, as words: the three frames, the time scale and the flags (the test compares them).
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.sink_frame, self.midpoint_frame, self.destruction_frame, self.accelerated_time_scale.to_bits(), self.flags]
    }

    /// `hasNonLodEffects()`.
    fn has_non_lod_effects(&self) -> bool {
        self.has_ocl || self.has_weapon
    }

    /// `m_maskOfLoadedEffects`.
    fn mask_of_loaded_effects(&self) -> bool {
        self.has_fx || self.has_ocl || self.has_weapon
    }

    /// `isSlowDeathActivated()`.
    pub(crate) fn is_slow_death_activated(&self) -> bool {
        self.flags & (1 << SLOW_DEATH_ACTIVATED) != 0
    }

    /// The module's answer to one of `Object`'s calls: `onDie` and the `SlowDeathBehaviorInterface`.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half, then the fields.
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

    /// `isDieApplicable(damageInfo)`: the die mux's, of this object's death.
    fn is_die_applicable(&self, world: &World, me: ModuleHandle, info: &DamageInfo) -> bool {
        let Some(o) = world.objects.find(me.object) else { return false };
        self.data.die.die_mux_data.is_die_applicable(info.input.death_type, o.get_veterancy_level() as i32, &o.status)
    }

    /// Records that upstream is undefined here.
    fn undefined(world: &mut World, me: ModuleHandle, u: Undefined) {
        world.cuts.module_errors.push((me, IniError::undefined(u)));
    }

    /// `getProbabilityModifier(damageInfo)`: "calculating how far past dead we were allows us to pick more spectacular deaths when severly killed, and more
    /// sedate ones when only slightly killed". `None` is upstream's undefined behaviour (recorded).
    fn get_probability_modifier(&self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> i32 {
        // eg ( 200 hp max, had 10 left, took 50 damage, 40 overkill, (40/200) * 100 = 20 overkill %)
        let Some(overkill_damage) = real_to_signed(if crate::logic::modules::plant(1291) { info.out.actual_damage_clipped - info.out.actual_damage_dealt } else { info.out.actual_damage_dealt - info.out.actual_damage_clipped }) else {
            Self::undefined(world, me, Undefined::FloatToIntOutOfRange);
            return 1;
        };
        let Some(body) = world.get_body_module(me.object) else {
            Self::undefined(world, me, Undefined::NullBodyModule);
            return 1;
        };
        let max_health = world.module_call(me.object, body, ModuleCall::GetMaxHealth).as_real(0.0);
        let overkill_percent = overkill_damage as f32 / max_health;
        let Some(overkill_modifier) = real_to_signed(overkill_percent * self.data.modifier_bonus_per_overkill_percent) else {
            Self::undefined(world, me, Undefined::FloatToIntOutOfRange);
            return 1;
        };
        let Some(sum) = self.data.probability_modifier.checked_add(overkill_modifier) else {
            Self::undefined(world, me, Undefined::SignedIntOverflow);
            return 1;
        };
        if crate::logic::modules::plant(1292) { sum } else { crate::common::base_type::max(sum, 1) }
    }

    /// `beginSlowDeath(damageInfo)`.
    pub(crate) fn begin_slow_death(&mut self, world: &mut World, me: ModuleHandle) {
        if self.is_slow_death_activated() {
            return;
        }
        let d = self.data.clone();
        let obj = me.object;
        let is_infantry = world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_INFANTRY));
        if d.sink_rate != 0.0 && (is_infantry != crate::logic::modules::plant(1314)) && world.has_drawable(obj) {
            // this object sinks slowly after it dies so don't draw a floating shadow decal on the ground above it.
            world.emit(ObjectClientEvent::DrawableSetShadowsEnabled { object_id: obj, enabled: false });
            world.emit(ObjectClientEvent::DrawableTerrainDecalFadeTarget { object_id: obj, target: 0.0, rate: -0.2 });
        }

        // Ask game detail manager if we need to speedup all deaths to improve performance
        let time_scale = world.cuts.globals.slow_death_scale;
        self.accelerated_time_scale = 1.0; // assume normal death speed.
        if time_scale == 0.0 && (!self.has_non_lod_effects() || crate::logic::modules::plant(1293)) {
            // Deaths happen instantly so just delete the object and return
            world.body_destroy_object(obj);
            return;
        }

        // timescale is some non-zero value so we may need to speed up death
        let is_hulk = world.objects.find(obj).is_some_and(|o| o.is_kind_of(KINDOF_HULK)) && !crate::logic::modules::plant(1315);
        if is_hulk && world.cuts.globals.hulk_max_lifetime_override != -1 {
            // Scripts don't want hulks around, so start sinking immediately!
            self.sink_frame = if crate::logic::modules::plant(1294) { 0 } else { 1 };
            self.midpoint_frame = (LOGICFRAMES_PER_SECOND / 2 + 1) as u32;
            self.destruction_frame = (LOGICFRAMES_PER_SECOND + 1) as u32;
            self.accelerated_time_scale = 1.0;
        } else {
            let r = if crate::logic::modules::plant(1295) { 0 } else { world.random.game_logic_random_value(0, d.sink_delay_variance as i32) };
            let Some(sink) = real_to_unsigned(time_scale * d.sink_delay.wrapping_add(r as u32) as f32) else {
                Self::undefined(world, me, Undefined::FloatToUnsignedOutOfRange);
                return;
            };
            self.sink_frame = sink;
            let r = world.random.game_logic_random_value(0, if crate::logic::modules::plant(1296) { d.sink_delay_variance } else { d.destruction_delay_variance } as i32);
            let Some(destruction) = real_to_unsigned(time_scale * d.destruction_delay.wrapping_add(r as u32) as f32) else {
                Self::undefined(world, me, Undefined::FloatToUnsignedOutOfRange);
                return;
            };
            self.destruction_frame = destruction;
            let (Some(lo), Some(hi)) =
                (real_to_signed(if crate::logic::modules::plant(1297) { 0.5 } else { BEGIN_MIDPOINT_RATIO } * self.destruction_frame as f32), real_to_signed(END_MIDPOINT_RATIO * self.destruction_frame as f32))
            else {
                Self::undefined(world, me, Undefined::FloatToIntOutOfRange);
                return;
            };
            self.midpoint_frame = world.random.game_logic_random_value(lo, hi) as u32;
            self.accelerated_time_scale = time_scale;
        }

        let now = world.frame();
        if d.fling_force > 0.0 {
            // Just in case this is a stingersoldier or other HELD object, lets set them free so they will fly with their own physics during slow death
            if world.objects.find(obj).is_some_and(|o| o.is_disabled_by_type(DISABLED_HELD)) {
                if let Some(slave) = world.find_module(obj, b"SlavedUpdate") {
                    world.module_call(obj, slave, ModuleCall::SlavedOnSlaverDie);
                }
            }
            if let Some(physics) = world.objects.find(obj).and_then(|o| o.physics) {
                // make sure we are at least a bit above the ground
                let min_altitude: f32 = if crate::logic::modules::plant(1300) { 2.0 } else { 1.0 };
                let altitude = world.get_height_above_terrain(obj);
                if altitude < min_altitude {
                    let mut pos = world.objects.find(obj).map_or_else(Coord3D::default, |o| o.get_position());
                    pos.z += min_altitude;
                    world.set_position(obj, &pos);
                }

                let force = calc_random_force(world, d.fling_force, d.fling_force + d.fling_force_variance, d.fling_pitch, d.fling_pitch + d.fling_pitch_variance);
                let force = if crate::logic::modules::plant(1302) { Coord3D { z: -force.z, ..force } } else { force };
                world.module_call(obj, physics, ModuleCall::PhysicsSetAllowToFall(true));
                world.module_call(obj, physics, ModuleCall::PhysicsApplyForce(force));
                world.module_call(obj, physics, ModuleCall::PhysicsSetExtraBounciness(-1.0)); // we don't want this guy to bounce at all
                world.module_call(obj, physics, ModuleCall::PhysicsSetExtraFriction((if crate::logic::modules::plant(1301) { 3.0 } else { -3.0 }) * SECONDS_PER_LOGICFRAME_REAL)); // reduce his ground friction a bit
                world.module_call(obj, physics, ModuleCall::PhysicsSetAllowBouncing(true));
                let orientation = if crate::logic::modules::plant(1303) { atan2_d(force.x, force.y) } else { atan2_d(force.y, force.x) };
                world.module_call(obj, physics, ModuleCall::PhysicsSetAngles { yaw: orientation, pitch: 0.0, roll: 0.0 });
                if !world.has_drawable(obj) {
                    // `obj->getDrawable()->setModelConditionState(..)` of a null drawable
                    Self::undefined(world, me, Undefined::NullDrawable);
                    return;
                }
                world.set_model_condition_state(obj, MODELCONDITION_EXPLODED_FLAILING);
                self.flags |= 1 << FLUNG_INTO_AIR;
            }
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        } else {
            // we don't need to wake up immediately, but only when the first of these counters wants to trigger....
            let mut when_to_wake_time = self.sink_frame as i32;
            if when_to_wake_time as u32 > self.destruction_frame {
                when_to_wake_time = self.destruction_frame as i32;
            }
            if when_to_wake_time as u32 > self.midpoint_frame {
                when_to_wake_time = self.midpoint_frame as i32;
            }
            update_behavior::set_wake_frame(world, update_sleep(when_to_wake_time as u32));
        }
        self.sink_frame = self.sink_frame.wrapping_add(now);
        self.destruction_frame = self.destruction_frame.wrapping_add(now);
        self.midpoint_frame = self.midpoint_frame.wrapping_add(now);
        if !crate::logic::modules::plant(1299) {
            self.flags |= 1 << SLOW_DEATH_ACTIVATED;
        }
        self.do_phase_stuff(world, me, SDPHASE_INITIAL);
    }

    /// `doPhaseStuff(sdphase)`: one entry of each list at random.
    fn do_phase_stuff(&self, world: &mut World, me: ModuleHandle, sdphase: usize) {
        let d = self.data.clone();
        let obj = me.object;
        if !self.mask_of_loaded_effects() {
            return; // has no ocl, fx, or weapons.
        }
        let list_size = d.fx[sdphase].len();
        if list_size > 0 {
            let idx = world.random.game_logic_random_value(0, list_size as i32 - 1 - i32::from(crate::logic::modules::plant(1317) && list_size > 1));
            if let Some(Some(fx)) = d.fx[sdphase].get(idx as usize) {
                world.emit(ObjectClientEvent::FxListDoFxObj { fx: fx.clone(), object_id: obj });
            }
        }
        let list_size = d.ocls[sdphase].len();
        if list_size > 0 {
            let idx = world.random.game_logic_random_value(0, list_size as i32 - 1);
            if let Some(ocl) = d.ocls[sdphase].get(idx as usize) {
                let found = world.statics.clone().object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some();
                if found || crate::logic::modules::plant(1318) {
                    world.cut(CutCall::OclCreate { object: obj, ocl: ocl.clone() });
                }
            }
        }
        let list_size = d.weapons[sdphase].len();
        if list_size > 0 {
            let idx = world.random.game_logic_random_value(0, list_size as i32 - 1);
            if let Some(wt) = d.weapons[sdphase].get(idx as usize) {
                let weapon = world.statics.clone().weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(wt)).map(|t| t.name.clone());
                if let Some(weapon) = weapon {
                    let position = world.objects.find(obj).map_or_else(Coord3D::default, |o| o.get_position());
                    world.cut(CutCall::WeaponCreateAndFireTemp { weapon, source: obj, position });
                }
            }
        }
    }

    /// `onDie(damageInfo)`.
    pub(crate) fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        let obj = me.object;
        if !self.is_die_applicable(world, me, info) {
            return;
        }

        if let Some(ai) = world.objects.find(obj).and_then(|o| o.ai) {
            // has another AI already handled us. (hopefully another SlowDeathBehavior)
            if world.module_call(obj, ai, ModuleCall::AiIsAiInDeadState).as_bool() && !crate::logic::modules::plant(1311) {
                return;
            }
            world.module_call(obj, ai, ModuleCall::AiMarkAsDead);
        }

        // deselect this unit for all players.
        if !crate::logic::modules::plant(1312) {
            world.cut(CutCall::GameLogicDeselectObject { object: obj, player_mask: crate::common::game_common::PLAYERMASK_ALL, affect_client: true });
        }

        let mut total: i32 = 0;
        for idx in world.all_modules(obj) {
            // `getSlowDeathBehaviorInterface()` is non-null for a slow death module, which answers; any other module has no answer
            if world.module_call(obj, idx, ModuleCall::IsDieApplicable(info.clone())).as_bool() {
                let m = match world.module_call(obj, idx, ModuleCall::GetProbabilityModifier(info.clone())) {
                    ModuleReply::Int(m) => m,
                    _ => 0,
                };
                match total.checked_add(m) {
                    Some(t) => total = t,
                    None => {
                        Self::undefined(world, me, Undefined::SignedIntOverflow);
                        return;
                    }
                }
            }
        }
        // DEBUG_ASSERTCRASH(total > 0, ("Hmm, this is wrong"));

        // this returns a value from 1...total, inclusive
        let mut roll = world.random.game_logic_random_value(1, total);
        for idx in world.all_modules(obj) {
            if world.module_call(obj, idx, ModuleCall::IsDieApplicable(info.clone())).as_bool() {
                let m = match world.module_call(obj, idx, ModuleCall::GetProbabilityModifier(info.clone())) {
                    ModuleReply::Int(m) => m,
                    _ => 0,
                };
                roll = roll.wrapping_sub(m);
                if roll <= -i32::from(crate::logic::modules::plant(1313)) {
                    world.module_call(obj, idx, ModuleCall::BeginSlowDeath(info.clone()));
                    return;
                }
            }
        }
        // DEBUG_CRASH(("We should never get here"));
    }
}

/// `atan2` of two `Real`s: the C `double` function, stored in a `Real`.
fn atan2_d(y: f32, x: f32) -> f32 {
    petir::Real::atan2(f64::from(y), f64::from(x)) as f32
}

/// `calcRandomForce(minMag, maxMag, minPitch, maxPitch, force)`: a random direction (a yaw, a pitch) and magnitude, as the x axis of a scaled, rotated matrix.
fn calc_random_force(world: &mut World, min_mag: f32, max_mag: f32, min_pitch: f32, max_pitch: f32) -> Coord3D {
    let angle = world.random.game_logic_random_value_real(-PI, PI);
    let pitch = world.random.game_logic_random_value_real(min_pitch, max_pitch);
    let mag = world.random.game_logic_random_value_real(min_mag, max_mag);

    let mut mtx = Matrix3D::identity();
    mtx.scale(mag);
    mtx.rotate_z(angle);
    mtx.rotate_y(-pitch);
    let v = mtx.get_x_vector();
    Coord3D { x: v.x, y: v.y, z: v.z }
}

impl UpdateBehavior for SlowDeathBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }

    /// `update()`.
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        // DEBUG_ASSERTCRASH(isSlowDeathActivated(), ("hmm, this should not be possible"));
        let d = self.data.clone();
        let obj = me.object;
        let time_scale = world.cuts.globals.slow_death_scale;

        // Check if we have normal time scale but LODManager is requeseting acceleration
        if time_scale != 1.0 && (self.accelerated_time_scale == 1.0 || crate::logic::modules::plant(1316)) && !self.has_non_lod_effects() {
            // speed of deaths has been increased since beginning of death so adjust it to current levels.
            if time_scale == 0.0 {
                // instant death
                world.body_destroy_object(obj);
                return UPDATE_SLEEP_NONE;
            }
            let (Some(sink), Some(mid), Some(dest)) = (
                real_to_unsigned(self.sink_frame as f32 * time_scale),
                real_to_unsigned(self.midpoint_frame as f32 * time_scale),
                real_to_unsigned(self.destruction_frame as f32 * time_scale),
            ) else {
                Self::undefined(world, me, Undefined::FloatToUnsignedOutOfRange);
                return UPDATE_SLEEP_NONE;
            };
            self.sink_frame = sink;
            self.midpoint_frame = mid;
            self.destruction_frame = dest;
            self.accelerated_time_scale = time_scale;
        }

        let now = world.frame();
        if self.flags & (1 << FLUNG_INTO_AIR) != 0 && (self.flags & (1 << BOUNCED) == 0 || crate::logic::modules::plant(1304)) {
            self.sink_frame = self.sink_frame.wrapping_add(1);
            self.midpoint_frame = self.midpoint_frame.wrapping_add(1);
            self.destruction_frame = self.destruction_frame.wrapping_add(1);

            if !world.is_above_terrain(obj) {
                if crate::logic::modules::plant(1305) {
                    world.clear_and_set_model_condition_state(obj, MODELCONDITION_EXPLODED_BOUNCING, MODELCONDITION_EXPLODED_FLAILING);
                } else {
                    world.clear_and_set_model_condition_state(obj, MODELCONDITION_EXPLODED_FLAILING, MODELCONDITION_EXPLODED_BOUNCING);
                }
                self.flags |= 1 << BOUNCED;
            }

            // Here we want to make sure we die if we collide with a tree on the way down
            if let Some(phys) = world.objects.find(obj).and_then(|o| o.physics) {
                let tree_id = world.module_call(obj, phys, ModuleCall::PhysicsGetLastCollidee).as_id();
                if world.objects.find(tree_id).is_some_and(|t| t.is_kind_of(KINDOF_SHRUBBERY)) {
                    world.body_set_disabled(obj, DISABLED_HELD);
                    world.clear_model_condition_state(obj, MODELCONDITION_EXPLODED_FLAILING);
                    world.clear_model_condition_state(obj, MODELCONDITION_EXPLODED_BOUNCING);
                    world.set_model_condition_state(obj, MODELCONDITION_PARACHUTING); // looks like he is snagged in a tree
                    let z = world.objects.find(obj).map_or(0.0, |o| o.get_position().z);
                    world.set_position_z(obj, z - d.sink_rate * if crate::logic::modules::plant(1306) { 5.0 } else { 50.0 }); // make him sink faster
                    if !world.is_above_terrain(obj) {
                        world.body_destroy_object(obj);
                    }
                }
            }
        }

        if (now >= self.sink_frame || crate::logic::modules::plant(1307)) && d.sink_rate > 0.0 {
            // disable Physics (if any) so that we can control the sink...
            world.body_set_disabled(obj, DISABLED_HELD);
            let mut pos = world.objects.find(obj).map_or_else(Coord3D::default, |o| o.get_position());
            pos.z -= if crate::logic::modules::plant(1308) { d.sink_rate * self.accelerated_time_scale } else { d.sink_rate / self.accelerated_time_scale };
            world.set_position(obj, &pos);
        }

        if now >= self.midpoint_frame && self.flags & (1 << MIDPOINT_EXECUTED) == 0 {
            self.do_phase_stuff(world, me, SDPHASE_MIDPOINT);
            if !crate::logic::modules::plant(1309) {
                self.flags |= 1 << MIDPOINT_EXECUTED;
            }
        }

        if now >= self.destruction_frame {
            if !crate::logic::modules::plant(1310) {
                self.do_phase_stuff(world, me, SDPHASE_FINAL);
            }
            world.body_destroy_object(obj);
        }

        UPDATE_SLEEP_NONE
    }

    /// The constructor's second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }

    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        // sink frame
        xfer.xfer_unsigned_int(&mut self.sink_frame)?;
        // midpoint frame
        xfer.xfer_unsigned_int(&mut self.midpoint_frame)?;
        // destruction frame
        xfer.xfer_unsigned_int(&mut self.destruction_frame)?;
        // accelerated time scale
        xfer.xfer_real(&mut self.accelerated_time_scale)?;
        // flags
        if crate::logic::modules::plant(1319) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.flags)
    }

    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(info) => {
                lend_clone(world, me, self, UpdateBody::SlowDeathBehavior, Self::from_body, |w, m| m.on_die(w, me, &info));
                ModuleReply::Done
            }
            ModuleCall::BeginSlowDeath(_) => {
                lend_clone(world, me, self, UpdateBody::SlowDeathBehavior, Self::from_body, |w, m| m.begin_slow_death(w, me));
                ModuleReply::Done
            }
            ModuleCall::GetProbabilityModifier(info) => ModuleReply::Int(self.get_probability_modifier(world, me, &info)),
            ModuleCall::IsDieApplicable(info) => ModuleReply::Bool(self.is_die_applicable(world, me, &info)),
            _ => ModuleReply::Unhandled,
        }
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::SlowDeathBehavior(self)
    }

    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SlowDeathBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SlowDeathBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }

    /// `xfer`: the version, the base, the three frames, the time scale and the flags.
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

/// The factory's `NewModuleDataProc` for `SlowDeathBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SlowDeathBehavior(Arc::new(SlowDeathBehaviorModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probability_modifier_under_one_is_invalid_data_from_the_constructor() {
        let data = SlowDeathBehaviorModuleData { probability_modifier: 0, ..Default::default() };
        let e = SlowDeathBehavior::unconstructed(1, Some(Arc::new(data))).unwrap_err();
        assert_eq!(e.kind, IniErrorKind::InvalidData);
        let ok = SlowDeathBehaviorModuleData { probability_modifier: 1, ..Default::default() };
        assert!(SlowDeathBehavior::unconstructed(1, Some(Arc::new(ok))).is_ok());
        // no data at all is the same error
        assert_eq!(SlowDeathBehavior::unconstructed(1, None).unwrap_err().kind, IniErrorKind::InvalidData);
    }

    #[test]
    fn the_float_conversions_know_where_c_plus_plus_stops_defining_them() {
        assert_eq!(real_to_unsigned(0.9), Some(0));
        assert_eq!(real_to_unsigned(-0.5), Some(0));
        assert_eq!(real_to_unsigned(-1.0), None);
        assert_eq!(real_to_unsigned(4_294_967_040.0), Some(4_294_967_040));
        assert_eq!(real_to_unsigned(4_294_967_296.0), None);
        assert_eq!(real_to_unsigned(f32::NAN), None);
        assert_eq!(real_to_signed(-2.9), Some(-2));
        assert_eq!(real_to_signed(2_147_483_648.0), None);
        assert_eq!(real_to_signed(f32::INFINITY), None);
    }

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = SlowDeathBehaviorModuleData::default();
        assert_eq!((d.probability_modifier, d.destruction_altitude, d.sink_rate), (10, -10.0, 0.0));
        let m = SlowDeathBehavior::unconstructed(3, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0, 0, 1.0f32.to_bits(), 0]);
        assert!(!m.is_slow_death_activated());
    }
}
