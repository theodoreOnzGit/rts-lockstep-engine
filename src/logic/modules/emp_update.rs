// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/EMPUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/EMPUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `EMPUpdate` and `LeafletDropBehavior`, the two modules of `EMPUpdate.cpp`.
//!
//! `EMPUpdate` is an EMP pulse: it grows towards a random scale, tinted, and when its fade starts it flashes and disables what is in its radius
//! (vehicles, faction structures, the units a spawner fights with; an aircraft in the air dies instead, unless EMP hardened), sparking on the
//! victims; when its life is over it dies. `LeafletDropBehavior` drops leaflets: its particle system, then, after a delay or when it dies, it
//! disables the enemy infantry and vehicles in its radius.
//!
//! # Where the world comes in
//!
//! - The drawable's scale and colours are the client's ([`ObjectClientEvent::DrawableSetInstanceScale`], `DrawableColorTint`,
//!   `DrawableColorFlash`, `DrawableTint`); the scan is [`World::partition_iterate_objects_in_range_at`]; the particle systems are
//!   [`World::create_particle_system_unplaced`] and the calls on them (`attachToObject`, `setPosition`, `setSystemLifetime`, `setInitialDelay`).
//! - The producer's AI is asked its victim ([`ModuleCall::AiGetCurrentVictim`]).
//!
//! # Upstream facts kept
//!
//! - `EMPUpdate::xfer` saves its version only (not even the `UpdateModule` base), and `crc` nothing: a load changes nothing but what the
//!   version check can refuse. `LeafletDropBehavior` saves its version and `m_startFrame`, no base, not `m_fxFired`.
//! - The die and fade frames are `REAL_TO_UNSIGNEDINT` of an `UnsignedInt` sum, through a `Real`.
//! - `LeafletDropBehavior::update` sleeps for ever until its start frame: once asleep it only acts when it dies (`onDie`, with no die mux).
//! - A victim filter's `m_victimKindOf` and `m_victimKindOfNot` are read and never used.
//! - The flash is `colorFlash(&end, 9999, m_tintEnvFadeFrames, TRUE)`: a decay of 9999 frames and an attack of the fade frames.
//! - `setDisabledUntil` is [`World::body_set_disabled_until`] (the real call, which the test stubs: `cuts.body.stub_object_calls`).
//! - A spark's height is `GameLogicRandomValue(3, victimHeight)` of a `Real` truncated to `Int`; the lifetime is `MAX(0, duration - 30)` of an
//!   `UnsignedInt` (modular).
//!
//! # Where upstream is undefined
//!
//! A radius of 0 or less (the scan's iterator and first victim are never set: [`Undefined::UninitialisedLocal`]); an update with no drawable
//! (`dr->setInstanceScale`, [`Undefined::NullDrawable`]); a spark height outside `Int` ([`Undefined::FloatToIntOutOfRange`]). The case is recorded
//! in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R33, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{min, real_to_int_ceil, real_to_unsigned_int, Coord3D, RGBColor, PI};
use crate::common::bit_flags::BitFlags;
use crate::common::disabled_types::DISABLED_EMP;
use crate::common::game_common::Relationship;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{
    KindOfMaskType, KINDOF_AIRCRAFT, KINDOF_EMP_HARDENED, KINDOF_INFANTRY, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon::{WEAPON_AFFECTS_ALLIES, WEAPON_AFFECTS_MASK_NAMES};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `FROM_BOUNDINGSPHERE_3D` (`DistanceCalculationType`).
const FROM_BOUNDINGSPHERE_3D: i32 = 1;
/// `TINT_STATUS_DISABLED`: "drawable tint color is deathly dark grey".
const TINT_STATUS_DISABLED: i32 = 0x0000_0001;

/// `saturateRGB(color, factor)`.
fn saturate_rgb(color: &mut RGBColor, factor: f32) {
    color.red *= factor;
    color.green *= factor;
    color.blue *= factor;
    let half_factor = factor * 0.5;
    color.red -= half_factor;
    color.green -= half_factor;
    color.blue -= half_factor;
}

/// `Int = Real`: undefined outside (-2^31 - 1, 2^31).
fn real_to_signed(x: f32) -> Option<i32> {
    if x > -2_147_483_904.0 && x < 2_147_483_648.0 {
        Some(x as i32)
    } else {
        None
    }
}

/// `ThePartitionManager->iterateObjectsInRange(pos, radius, FROM_BOUNDINGSPHERE_3D)`, which runs only for a radius above zero (upstream reads an
/// iterator it never set otherwise).
fn scan(world: &mut World, id: ObjectId, radius: f32) -> Result<(ObjectId, Vec<ObjectId>), Undefined> {
    if radius <= 0.0 {
        return Err(Undefined::UninitialisedLocal);
    }
    let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
    Ok((id, world.partition_iterate_objects_in_range_at(pos, radius, FROM_BOUNDINGSPHERE_3D)))
}

/// `class EMPUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct EmpUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_lifeFrames`.
    pub life_frames: u32,
    /// `m_startFadeFrame`.
    pub start_fade_frame: u32,
    /// `m_disabledDuration`.
    pub disabled_duration: u32,
    /// `m_startScale`: "how big I start drawing".
    pub start_scale: f32,
    /// `m_targetScaleMin`.
    pub target_scale_min: f32,
    /// `m_targetScaleMax`.
    pub target_scale_max: f32,
    /// `m_startColor`.
    pub start_color: RGBColor,
    /// `m_endColor`.
    pub end_color: RGBColor,
    /// `m_disableFXParticleSystem`, by name (`None` for `NULL`).
    pub disable_fx_particle_system: Option<Vec<u8>>,
    /// `m_sparksPerCubicFoot`.
    pub sparks_per_cubic_foot: f32,
    /// `m_effectRadius`.
    pub effect_radius: f32,
    /// `m_rejectMask` (`DoesNotAffect`).
    pub reject_mask: u32,
    /// `m_victimKindOf` (read, never used).
    pub victim_kind_of: KindOfMaskType,
    /// `m_victimKindOfNot` (read, never used).
    pub victim_kind_of_not: KindOfMaskType,
    /// `m_doesNotAffectMyOwnBuildings`.
    pub does_not_affect_my_own_buildings: bool,
}

impl Default for EmpUpdateModuleData {
    /// `EMPUpdateModuleData()`.
    fn default() -> Self {
        let mut start_color = RGBColor::default();
        start_color.set_from_int(0xffff_ffffu32 as i32);
        let mut end_color = RGBColor::default();
        end_color.set_from_int(0);
        EmpUpdateModuleData {
            update: UpdateModuleData::new(),
            life_frames: 1,
            start_fade_frame: 0,
            disabled_duration: 0,
            start_scale: 1.0,
            target_scale_min: 1.0,
            target_scale_max: 1.0,
            start_color,
            end_color,
            disable_fx_particle_system: None,
            sparks_per_cubic_foot: 0.001,
            effect_radius: 200.0,
            reject_mask: 0,
            victim_kind_of: BitFlags::NONE,
            victim_kind_of_not: BitFlags::NONE,
            does_not_affect_my_own_buildings: false,
        }
    }
}

/// `INI::parseParticleSystemTemplate`: the next token, `None` for `NULL`.
fn parse_template(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `EMPUpdateModuleData::buildFieldParse`'s table.
static EMP_FIELD_PARSE: [FieldParse<EmpUpdateModuleData>; 16] = [
    FieldParse::new(
        "Lifetime",
        |i, d, _| {
            d.life_frames = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "StartFadeTime",
        |i, d, _| {
            d.start_fade_frame = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "StartScale",
        |i, d, _| {
            d.start_scale = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DisabledDuration",
        |i, d, _| {
            d.disabled_duration = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "TargetScaleMax",
        |i, d, _| {
            d.target_scale_max = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "TargetScaleMin",
        |i, d, _| {
            d.target_scale_min = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "StartColor",
        |i, d, _| {
            d.start_color = i.parse_rgb_color()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "EndColor",
        |i, d, _| {
            d.end_color = i.parse_rgb_color()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DisableFXParticleSystem",
        |i, d, _| {
            d.disable_fx_particle_system = parse_template(i)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SparksPerCubicFoot",
        |i, d, _| {
            d.sparks_per_cubic_foot = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "EffectRadius",
        |i, d, _| {
            d.effect_radius = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("DoesNotAffect", |i, d, _| i.parse_bit_string32(&mut d.reject_mask, &WEAPON_AFFECTS_MASK_NAMES), UserData::None),
    FieldParse::new(
        "DoesNotAffectMyOwnBuildings",
        |i, d, _| {
            d.does_not_affect_my_own_buildings = i.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new("VictimRequiredKindOf", |i, d, _| d.victim_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("VictimForbiddenKindOf", |i, d, _| d.victim_kind_of_not.parse_from_ini(i), UserData::None),
    FieldParse::end(),
];

impl EmpUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, EmpUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&EMP_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<EmpUpdateModuleData> {
        let mut data = EmpUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, EmpUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class EMPUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct EmpUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getEMPUpdateModuleData()`.
    data: Arc<EmpUpdateModuleData>,
    /// `m_dieFrame`: "frame we die on".
    die_frame: u32,
    /// `m_tintEnvFadeFrames`.
    tint_env_fade_frames: u32,
    /// `m_tintEnvPlayFrame`: "which frame to trigger the tint envelope".
    tint_env_play_frame: u32,
    /// `m_targetScale`.
    target_scale: f32,
    /// `m_currentScale`: "how big I am drawing this frame".
    current_scale: f32,
}

impl PartialEq for EmpUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl EmpUpdate {
    /// `getInterfaceMask()`: the update module's.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data); the frames and the
    /// scales are the second half's.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<EmpUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(EmpUpdate { update, current_scale: data.start_scale, data, die_frame: 0, tint_env_fade_frames: 0, tint_env_play_frame: 0, target_scale: 1.0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getEMPUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<EmpUpdateModuleData> {
        &self.data
    }

    /// `getDieFrame()`.
    pub fn get_die_frame(&self) -> u32 {
        self.die_frame
    }

    /// The state, as words: the three frames and the two scales.
    pub fn state_words(&self) -> [u32; 5] {
        [self.die_frame, self.tint_env_fade_frames, self.tint_env_play_frame, self.target_scale.to_bits(), self.current_scale.to_bits()]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the frames and the random scale and orientation.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer`: the version only ("Version Info: 1: Initial version"), not even the scheduling half.
    pub fn xfer_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }

    /// `crc`: nothing.
    pub fn crc_with(&mut self, _update: &mut UpdateModule, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `loadPostProcess`: nothing.
    pub fn load_post_process_with(&mut self, _update: &mut UpdateModule) {}

    /// `doDisableAttack()`.
    fn do_disable_attack(&self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let d = self.data.clone();
        let object = me.object;
        let radius = d.effect_radius;
        let pos = world.objects.find(object).map(|o| o.get_position()).unwrap_or_default();
        let producer = world.objects.find(object).map(|o| o.producer_id).filter(|&p| world.objects.find(p).is_some());
        let mut intended_victim = None;
        let mut only_effect_airborne = false;
        let mut intended_victim_processed = false;
        if let Some(ai) = producer.and_then(|p| world.objects.find(p).and_then(|o| o.ai).map(|ai| (p, ai))) {
            intended_victim = Some(world.module_call(ai.0, ai.1, ModuleCall::AiGetCurrentVictim).as_id()).filter(|&v| world.objects.find(v).is_some());
            if intended_victim.is_some_and(|v| world.objects.find(v).is_some_and(|o| o.test_status(OBJECT_STATUS_AIRBORNE_TARGET))) {
                only_effect_airborne = true;
            }
        }
        let (_, victims) = scan(world, object, radius)?;
        let now = world.frame();
        for cur in victims {
            if cur == object {
                continue;
            }
            let Some(v) = world.objects.find(cur) else {
                continue;
            };
            let airborne = v.test_status(OBJECT_STATUS_AIRBORNE_TARGET);
            if only_effect_airborne && !airborne {
                continue;
            }
            let (structure, vehicle, spawns, aircraft, hardened) = (
                v.is_kind_of(KINDOF_STRUCTURE),
                v.is_kind_of(KINDOF_VEHICLE),
                v.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS),
                v.is_kind_of(KINDOF_AIRCRAFT),
                v.is_kind_of(KINDOF_EMP_HARDENED),
            );
            if d.does_not_affect_my_own_buildings && structure && world.get_controlling_player(cur) == world.get_controlling_player(object) {
                continue;
            }
            if !vehicle && !structure && !spawns {
                continue;
            } else if aircraft && airborne {
                // is in the sky
                if hardened && !crate::logic::modules::plant(2033) {
                    continue;
                }
                world.kill(cur, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
                if world.has_drawable(cur) {
                    // paint it black
                    world.emit(ObjectClientEvent::DrawableTint { object_id: cur, status: TINT_STATUS_DISABLED, on: true });
                }
                continue;
            } else if structure {
                if !world.is_faction_structure(cur) {
                    continue;
                }
            } else if d.reject_mask & WEAPON_AFFECTS_ALLIES != 0
                && world.get_relationship(cur, object) == Relationship::Allies
                && !crate::logic::modules::plant(2034)
            {
                continue;
            }
            world.body_set_disabled_until(cur, DISABLED_EMP, if crate::logic::modules::plant(2035) { now } else { now.wrapping_add(d.disabled_duration) });
            if intended_victim == Some(cur) {
                intended_victim_processed = true;
            }
            if world.has_drawable(cur) {
                if let Some(tmp) = &d.disable_fx_particle_system {
                    let Some(geom) = world.objects.find(cur).map(|o| o.geometry_info.clone()) else {
                        continue;
                    };
                    let victim_height = geom.get_max_height_above_position();
                    let victim_volume = geom.get_footprint_area() * min(victim_height, 10.0);
                    let emitter_count =
                        (if crate::logic::modules::plant(2036) { 10 } else { 15 }).max(real_to_int_ceil(d.sparks_per_cubic_foot * victim_volume)) as u32;
                    for _ in 0..emitter_count {
                        let sys = world.create_particle_system_unplaced(tmp.clone());
                        let mut offs = Coord3D::default();
                        geom.make_random_offset_within_footprint(&mut world.random, &mut offs);
                        let hi = real_to_signed(victim_height).ok_or(Undefined::FloatToIntOutOfRange)?;
                        offs.z = world.random.game_logic_random_value(3, hi) as f32;
                        if offs.length() > victim_height {
                            let (restore_x, restore_y) = (offs.x, offs.y);
                            offs.normalize();
                            offs.z *= victim_height;
                            offs.x = restore_x;
                            offs.y = restore_y;
                        }
                        world.body_particle_attach_to_object(sys, cur);
                        world.particle_system_set_position(sys, offs);
                        world.particle_system_set_lifetime(sys, d.disabled_duration.wrapping_sub(30));
                        let delay = world.random.game_logic_random_value(1, 100);
                        world.particle_system_set_initial_delay(sys, delay as u32);
                    }
                }
            }
        }
        if let Some(v) = intended_victim.filter(|_| !intended_victim_processed) {
            let Some((vpos, aircraft, hardened)) =
                world.objects.find(v).map(|o| (o.get_position(), o.is_kind_of(KINDOF_AIRCRAFT), o.is_kind_of(KINDOF_EMP_HARDENED)))
            else {
                return Ok(());
            };
            if aircraft && !hardened {
                let c = Coord3D { x: vpos.x - pos.x, y: vpos.y - pos.y, z: vpos.z - pos.z };
                let length_sqr = c.length_sqr();
                if length_sqr <= radius * 2.0 || (length_sqr <= 40.0 * 40.0 && !crate::logic::modules::plant(2037)) {
                    world.body_set_disabled_until(v, DISABLED_EMP, now.wrapping_add(d.disabled_duration));
                }
            }
        }
        Ok(())
    }

    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let id = me.object;
        let d = self.data.clone();
        if !world.has_drawable(id) {
            return Err(Undefined::NullDrawable);
        }
        let now = world.frame();
        self.current_scale += (self.target_scale - self.current_scale) * 0.05;
        world.emit(ObjectClientEvent::DrawableSetInstanceScale { object_id: id, scale: self.current_scale });
        if now < self.tint_env_play_frame {
            let mut start = d.start_color;
            saturate_rgb(&mut start, if crate::logic::modules::plant(2031) { 3.0 } else { 2.0 });
            world.emit(ObjectClientEvent::DrawableColorTint { object_id: id, color: start });
        }
        if now == self.tint_env_play_frame {
            let mut end = d.end_color;
            saturate_rgb(&mut end, 5.0);
            // `colorFlash(&end, 9999, m_tintEnvFadeFrames, TRUE)`: the parameters are (decay, attack, sustain)
            world.emit(ObjectClientEvent::DrawableColorFlash {
                object_id: id,
                color: end,
                decay_frames: 9999,
                attack_frames: if crate::logic::modules::plant(2032) { self.die_frame } else { self.tint_env_fade_frames },
                sustain_at_peak: true,
            });
            self.do_disable_attack(world, me)?;
        }
        if now >= self.die_frame {
            world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        }
        Ok(())
    }
}

impl UpdateBehavior for EmpUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if let Err(u) = self.update_inner(world, me) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor's world half: the frames (`REAL_TO_UNSIGNEDINT` of the `UnsignedInt` sums), the random target scale, a random orientation.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let d = self.data.clone();
        let now = world.frame();
        self.current_scale = d.start_scale;
        self.die_frame = real_to_unsigned_int(now.wrapping_add(d.life_frames) as f32);
        self.tint_env_play_frame = real_to_unsigned_int(now.wrapping_add(d.start_fade_frame) as f32);
        self.tint_env_fade_frames = self.die_frame.wrapping_sub(self.tint_env_play_frame);
        self.target_scale = world.random.game_logic_random_value_real(d.target_scale_min, d.target_scale_max);
        let angle = world.random.game_logic_random_value_real(-PI, PI);
        world.set_orientation(me.object, angle);
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::EmpUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::EmpUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for EmpUpdate {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `xfer`: the version only.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        self.xfer_with(&mut base, xfer)
    }
    fn load_post_process(&mut self) {}
}

/// The factory's `NewModuleDataProc` for `EMPUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::EmpUpdate(Arc::new(EmpUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class LeafletDropBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct LeafletDropBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_delayFrames`.
    pub delay_frames: u32,
    /// `m_disabledDuration`.
    pub disabled_duration: u32,
    /// `m_radius`.
    pub radius: f32,
    /// `m_leafletFXParticleSystem`, by name (`None` for `NULL`).
    pub leaflet_fx_particle_system: Option<Vec<u8>>,
}

impl Default for LeafletDropBehaviorModuleData {
    /// `LeafletDropBehaviorModuleData()`.
    fn default() -> Self {
        LeafletDropBehaviorModuleData { update: UpdateModuleData::new(), delay_frames: 1, disabled_duration: 0, radius: 60.0, leaflet_fx_particle_system: None }
    }
}

/// `LeafletDropBehaviorModuleData::buildFieldParse`'s table.
static LEAFLET_FIELD_PARSE: [FieldParse<LeafletDropBehaviorModuleData>; 5] = [
    FieldParse::new(
        "Delay",
        |i, d, _| {
            d.delay_frames = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "DisabledDuration",
        |i, d, _| {
            d.disabled_duration = i.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "AffectRadius",
        |i, d, _| {
            d.radius = i.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "LeafletFXParticleSystem",
        |i, d, _| {
            d.leaflet_fx_particle_system = parse_template(i)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl LeafletDropBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, LeafletDropBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&LEAFLET_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<LeafletDropBehaviorModuleData> {
        let mut data = LeafletDropBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, LeafletDropBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class LeafletDropBehavior : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct LeafletDropBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getLeafletDropBehaviorModuleData()`.
    data: Arc<LeafletDropBehaviorModuleData>,
    /// `m_startFrame`.
    start_frame: u32,
    /// `m_fxFired`: "have we done our fx yet".
    fx_fired: bool,
}

impl PartialEq for LeafletDropBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl LeafletDropBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<LeafletDropBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(LeafletDropBehavior { update, data, start_frame: 0, fx_fired: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getLeafletDropBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<LeafletDropBehaviorModuleData> {
        &self.data
    }

    /// The state, as words: the start frame and `m_fxFired`.
    pub fn state_words(&self) -> [u32; 2] {
        [self.start_frame, u32::from(self.fx_fired)]
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half: the start frame.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer`: the version and `m_startFrame`, no scheduling half.
    pub fn xfer_with(&mut self, _update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        xfer.xfer_unsigned_int(&mut self.start_frame)
    }

    /// `crc`: nothing.
    pub fn crc_with(&mut self, _update: &mut UpdateModule, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }

    /// `loadPostProcess`: nothing.
    pub fn load_post_process_with(&mut self, _update: &mut UpdateModule) {}

    /// `doDisableAttack()`: the enemy infantry and vehicles in the radius are disabled.
    fn do_disable_attack(&self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let d = self.data.clone();
        let object = me.object;
        let (_, victims) = scan(world, object, d.radius)?;
        let now = world.frame();
        for cur in victims {
            if cur == object {
                continue;
            }
            let Some(v) = world.objects.find(cur) else {
                continue;
            };
            // both commuters and pedestrians
            if !(v.is_kind_of(KINDOF_INFANTRY) || v.is_kind_of(KINDOF_VEHICLE)) {
                continue;
            }
            // only enemies
            if world.get_relationship(cur, object) == Relationship::Allies
                || (!crate::logic::modules::plant(2038) && world.get_relationship(cur, object) != Relationship::Enemies)
            {
                continue;
            }
            world.body_set_disabled_until(cur, DISABLED_EMP, now.wrapping_add(d.disabled_duration));
        }
        Ok(())
    }
}

impl UpdateBehavior for LeafletDropBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if !self.fx_fired {
            if let Some(tmp) = &self.data.leaflet_fx_particle_system {
                let sys = world.create_particle_system_unplaced(tmp.clone());
                world.body_particle_attach_to_object(sys, me.object);
            }
            // hey, at least we tried.
            self.fx_fired = !crate::logic::modules::plant(2039);
        }
        if world.frame() < self.start_frame {
            return UPDATE_SLEEP_FOREVER;
        }
        if let Err(u) = self.do_disable_attack(world, me) {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
        UPDATE_SLEEP_NONE
    }
    /// The constructor's world half: `m_startFrame = now + m_delayFrames`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.start_frame = world.frame().wrapping_add(self.data.delay_frames);
    }
    /// The die interface: `onDie` disables what is in the radius (no die mux).
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(_) => {
                if let Err(u) = self.do_disable_attack(world, me) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::LeafletDropBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::LeafletDropBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for LeafletDropBehavior {
    /// `crc`: nothing.
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> XferResult<()> {
        Ok(())
    }
    /// `xfer`: the version and `m_startFrame`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        self.xfer_with(&mut base, xfer)
    }
    fn load_post_process(&mut self) {}
}

/// The factory's `NewModuleDataProc` for `LeafletDropBehavior`.
pub fn new_leaflet_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::LeafletDropBehavior(Arc::new(LeafletDropBehaviorModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = EmpUpdateModuleData::default();
        assert_eq!((d.life_frames, d.effect_radius, d.start_color.red, d.end_color.red), (1, 200.0, 1.0, 0.0));
        let l = LeafletDropBehaviorModuleData::default();
        assert_eq!((l.delay_frames, l.radius), (1, 60.0));
        let mut c = RGBColor { red: 1.0, green: 0.5, blue: 0.0 };
        saturate_rgb(&mut c, 2.0);
        assert_eq!((c.red, c.green, c.blue), (1.0, 0.0, -1.0));
    }
}
