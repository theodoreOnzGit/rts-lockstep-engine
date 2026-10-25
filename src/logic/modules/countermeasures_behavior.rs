// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/CountermeasuresBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/CountermeasuresBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `CountermeasuresBehavior`: an aircraft's flares (an update module behind an upgrade). A missile reported to it may be decoyed (the evasion
//! rate's chance, when it has countermeasures left or out): the missile's projectile update diverts after the decoy delay, and the first
//! volley is launched after the reaction latency. While airborne the object launches a volley every `DelayBetweenVolleys` until it runs out;
//! out of countermeasures it reloads after `ReloadTime`. A volley is `VolleySize` flares spread over the arc, made from the flare template on
//! the object's controlling player's default team, at the object's position and angle, given the object's velocity and a motive force along
//! its direction.
//!
//! # Upstream facts kept
//!
//! - `m_reloadFrame` is not set by the constructor and not saved, and the data's `m_countermeasureReactionFrames` (`ReactionLaunchLatency`)
//!   is not set by the data's constructor: both are the zero the allocator fills new memory with (`MemoryPool::allocateBlockImplementation`,
//!   `DynamicMemoryAllocator::allocateBytesImplementation`). ~~Reading either was `Undefined::UninitialisedMember`~~ **CORRECTED
//!   2026-10-25** (batch R52 as first committed excluded those cases).
//! - `calculateCountermeasureToDivertTo` tests the end iterator as a pointer (`if( it )`: the shipped `std::vector`'s iterator is one, null
//!   while the vector has never had storage), steps back from the end only past ids it does not find, and reads the same id again once it
//!   finds one (so, short of a distance of 1e15 or more, the answer is the last id still found, whatever the distances); reading before the first element is
//!   `Undefined::IteratorOutOfRange`. The storage is kept as [`CountermeasuresBehavior::allocated`]
//!   (an erase or a load keeps it).
//! - The volley launches `VolleySize` flares whatever is left: `m_availableCountermeasures` wraps below zero, as does
//!   `m_activeCountermeasures` when more flares are gone than were counted.
//! - `xfer`'s fields are read when the CURRENT version is 2 (always), not the saved one; a load into a module whose flare list is not empty
//!   is `XFER_LIST_NOT_EMPTY`.
//! - A launch with no physics on the object, or on a flare, or no controlling player, dereferences `NULL`: `Undefined::NullPhysics`,
//!   `Undefined::NullControllingPlayer`.
//!
//! `p1w13-sonnet`, batch R52, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DisabledMaskType, DISABLED_HELD};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::object_status_types::OBJECT_STATUS_AIRBORNE_TARGET;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;
use crate::ww::vector2::Vector2;

/// `class CountermeasuresBehaviorModuleData : public UpdateModuleData`.
#[derive(Debug)]
pub struct CountermeasuresBehaviorModuleData {
    /// The base.
    pub update: UpdateModuleData,
    /// `m_upgradeMuxData`.
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_flareTemplateName`, `m_flareBoneBaseName`.
    pub flare_template_name: Vec<u8>,
    pub flare_bone_base_name: Vec<u8>,
    /// `m_evasionRate`.
    pub evasion_rate: f32,
    /// `m_volleySize`, `m_volleyArcAngle`, `m_volleyVelocityFactor`, `m_framesBetweenVolleys`, `m_numberOfVolleys`, `m_reloadFrames`.
    pub volley_size: u32,
    pub volley_arc_angle: f32,
    pub volley_velocity_factor: f32,
    pub frames_between_volleys: u32,
    pub number_of_volleys: u32,
    pub reload_frames: u32,
    /// `m_missileDecoyFrames`; `m_countermeasureReactionFrames` (the constructor does not set it: the allocator's zero).
    pub missile_decoy_frames: u32,
    pub countermeasure_reaction_frames: u32,
    /// `m_mustReloadAtAirfield`.
    pub must_reload_at_airfield: bool,
}

impl Default for CountermeasuresBehaviorModuleData {
    /// `CountermeasuresBehaviorModuleData()`.
    fn default() -> Self {
        CountermeasuresBehaviorModuleData {
            update: UpdateModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            flare_template_name: Vec::new(),
            flare_bone_base_name: Vec::new(),
            evasion_rate: 0.0,
            volley_size: 0,
            volley_arc_angle: 0.0,
            volley_velocity_factor: 1.0,
            frames_between_volleys: 0,
            number_of_volleys: 0,
            reload_frames: 0,
            missile_decoy_frames: 0,
            countermeasure_reaction_frames: 0,
            must_reload_at_airfield: false,
        }
    }
}

static UPGRADE_BASE_FIELD_PARSE: [FieldParse<CountermeasuresBehaviorModuleData>; 6] = upgrade_module_data_field_parse!(CountermeasuresBehaviorModuleData, upgrade);

static OWN_FIELD_PARSE: [FieldParse<CountermeasuresBehaviorModuleData>; 13] = [
    FieldParse::new("FlareTemplateName", |i, d, _| { d.flare_template_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("FlareBoneBaseName", |i, d, _| { d.flare_bone_base_name = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("VolleySize", |i, d, _| { d.volley_size = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("VolleyArcAngle", |i, d, _| { d.volley_arc_angle = i.parse_angle_real()?; Ok(()) }, UserData::None),
    FieldParse::new("VolleyVelocityFactor", |i, d, _| { d.volley_velocity_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DelayBetweenVolleys", |i, d, _| { d.frames_between_volleys = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NumberOfVolleys", |i, d, _| { d.number_of_volleys = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ReloadTime", |i, d, _| { d.reload_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("EvasionRate", |i, d, _| { d.evasion_rate = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MustReloadAtAirfield", |i, d, _| { d.must_reload_at_airfield = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("MissileDecoyDelay", |i, d, _| { d.missile_decoy_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ReactionLaunchLatency", |i, d, _| { d.countermeasure_reaction_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl CountermeasuresBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base's table, this class's, then the upgrade mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, CountermeasuresBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<CountermeasuresBehaviorModuleData> {
        let mut data = CountermeasuresBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, CountermeasuresBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The module factory's data proc.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::CountermeasuresBehavior(Arc::new(CountermeasuresBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class CountermeasuresBehavior : public UpdateModule, public UpgradeMux, public CountermeasuresBehaviorInterface`.
#[derive(Clone, Debug)]
pub struct CountermeasuresBehavior {
    update: UpdateModule,
    data: Arc<CountermeasuresBehaviorModuleData>,
    mux: UpgradeMux,
    /// `m_counterMeasures`.
    counter_measures: Vec<ObjectId>,
    /// Whether `m_counterMeasures` has storage (see the module docs).
    allocated: bool,
    /// `m_availableCountermeasures`, `m_activeCountermeasures`, `m_divertedMissiles`, `m_incomingMissiles`, `m_reactionFrame`,
    /// `m_nextVolleyFrame`.
    available: u32,
    active: u32,
    diverted: u32,
    incoming: u32,
    reaction_frame: u32,
    next_volley_frame: u32,
    /// `m_reloadFrame` (the pool's zero until set).
    reload_frame: u32,
}

impl PartialEq for CountermeasuresBehavior {
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.counter_measures == other.counter_measures
            && self.allocated == other.allocated
            && self.state_words() == other.state_words()
    }
}

impl CountermeasuresBehavior {
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_UPGRADE
    }

    /// The constructor's first half (`INI_INVALID_DATA` for null data): the base processing `DISABLED_HELD`, the counts.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<CountermeasuresBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let mut update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        // getDisabledTypesToProcess(): MAKE_DISABLED_MASK( DISABLED_HELD )
        let mut mask = DisabledMaskType::default();
        mask.set(DISABLED_HELD, 1);
        update.set_disabled_types_to_process(mask);
        Ok(CountermeasuresBehavior {
            update,
            available: data.number_of_volleys.wrapping_mul(data.volley_size),
            data,
            mux: UpgradeMux::new(),
            counter_measures: Vec::new(),
            allocated: false,
            active: 0,
            diverted: 0,
            incoming: 0,
            reaction_frame: 0,
            next_volley_frame: 0,
            reload_frame: 0,
        })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    pub fn module_data(&self) -> &Arc<CountermeasuresBehaviorModuleData> {
        &self.data
    }

    /// The state the test compares: the counts and frames, the mux, the flares.
    pub fn state_words(&self) -> Vec<u32> {
        let mut v = vec![
            self.available,
            self.active,
            self.diverted,
            self.incoming,
            self.reaction_frame,
            self.next_volley_frame,
            self.reload_frame,
            u32::from(self.mux.is_already_upgraded()),
            self.counter_measures.len() as u32,
        ];
        v.extend(self.counter_measures.iter().map(|&id| id as u32));
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
        update_behavior::crc_with(update, xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }

    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// `reportMissileForCountermeasures(missile)`.
    fn report_missile_for_countermeasures(&mut self, world: &mut World, missile: ObjectId) -> Result<(), IniError> {
        if world.objects.find(missile).is_none() {
            return Ok(());
        }
        if !crate::logic::modules::plant(2228) {
            self.incoming = self.incoming.wrapping_add(1);
        }
        if self.available.wrapping_add(self.active) > 0 {
            let d = self.data.clone();
            if world.random.game_logic_random_value_real(0.0, 1.0) < d.evasion_rate {
                // the first module of the missile with a projectile update interface
                let pui = world.all_modules(missile).into_iter().find(|&i| world.module_call(missile, i, ModuleCall::HasProjectileUpdateInterface).as_bool());
                if let Some(pui) = pui {
                    world.module_call(missile, pui, ModuleCall::ProjectileSetFramesTillCountermeasureDiversion(d.missile_decoy_frames));
                    self.diverted = self.diverted.wrapping_add(1);
                    if self.active == 0 && self.reaction_frame == 0 && !crate::logic::modules::plant(2221) {
                        self.reaction_frame = world.frame().wrapping_add(d.countermeasure_reaction_frames);
                    }
                }
            }
        }
        Ok(())
    }

    /// `calculateCountermeasureToDivertTo(victim)` (see the module docs for the iterator).
    fn calculate_countermeasure_to_divert_to(&self, world: &mut World, me: ObjectId) -> Result<ObjectId, IniError> {
        let mut iterator_max = (self.data.volley_size as i32).max(1);
        let mut closest_dist = 1e15f32;
        let mut closest_flare = None;
        if self.allocated {
            let mut it = self.counter_measures.len() as isize - 1;
            while iterator_max != 0 {
                iterator_max -= 1;
                if it < 0 {
                    return Err(IniError::undefined(Undefined::IteratorOutOfRange));
                }
                let id = self.counter_measures[it as usize];
                if world.objects.find(id).is_some() {
                    let dist = world.partition_get_distance_squared_center_2d(id, me);
                    if dist < closest_dist {
                        closest_dist = dist;
                        closest_flare = Some(id);
                    }
                    if crate::logic::modules::plant(2227) {
                        it -= 1;
                    }
                } else {
                    it -= 1;
                }
            }
        }
        Ok(closest_flare.unwrap_or(crate::common::game_type::INVALID_ID))
    }

    /// `reloadCountermeasures()`.
    fn reload_countermeasures(&mut self) {
        self.available = self.data.number_of_volleys.wrapping_mul(self.data.volley_size);
        self.reload_frame = 0;
    }

    /// `launchVolley()`.
    fn launch_volley(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), IniError> {
        let d = self.data.clone();
        let us = me.object;
        let volley_size = d.volley_size as f32;
        for i in 0..d.volley_size {
            let current_volley = i as f32;
            let mut ratio = 0.0f32;
            if volley_size != 1.0 {
                ratio = current_volley / (volley_size - 1.0) * 2.0 - 1.0;
            }
            let angle = ratio * if crate::logic::modules::plant(2222) { 0.0 } else { d.volley_arc_angle };
            let physics = world.objects.find(us).and_then(|o| o.physics);
            let mut vel = world.objects.find(us).map_or_else(Coord3D::default, |o| o.thing.unit_direction_vector_3d());
            let mut flare_vector = Vector2::new(vel.x, vel.y);
            flare_vector.normalize();
            flare_vector.rotate(angle);
            vel.x = flare_vector.x;
            vel.y = flare_vector.y;
            vel.z = 0.0;
            let Some(physics) = physics else { return Err(IniError::undefined(Undefined::NullPhysics)) };
            let mut velocity = world.module_call(us, physics, ModuleCall::PhysicsGetVelocityMagnitude).as_real(0.0);
            if velocity < 1.0 && !crate::logic::modules::plant(2223) {
                velocity = -10.0;
            }
            let s = velocity * d.volley_velocity_factor;
            vel.x *= s;
            vel.y *= s;
            vel.z *= s;
            if let Some(thing) = world.thing_factory_find_template(&d.flare_template_name) {
                let player = world.get_controlling_player(us).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
                let team = world.players.get_nth(player).and_then(|p| p.default_team);
                let flare = match world.thing_factory_new_object(thing, team, Default::default()) {
                    Ok(f) => f,
                    Err(e) => panic!("the flare template makes an object: {e:?}"),
                };
                let (pos, angle) = world.objects.find(us).map_or((Coord3D::default(), 0.0), |o| (o.get_position(), o.thing.orientation()));
                world.set_position(flare, &pos);
                world.set_orientation(flare, angle);
                let Some(flare_physics) = world.objects.find(flare).and_then(|o| o.physics) else { return Err(IniError::undefined(Undefined::NullPhysics)) };
                let our_vel = world.module_call(us, physics, ModuleCall::PhysicsGetVelocity).as_coord();
                world.module_call(flare, flare_physics, ModuleCall::PhysicsTransferVelocityFrom(our_vel));
                world.module_call(flare, flare_physics, ModuleCall::PhysicsApplyMotiveForce(vel));
                self.active = self.active.wrapping_add(1);
                self.available = self.available.wrapping_sub(1);
                self.counter_measures.push(flare);
                self.allocated = true;
            }
        }
        Ok(())
    }

    fn run_update(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, IniError> {
        let now = world.frame();
        let d = self.data.clone();
        let us = me.object;
        if world.objects.find(us).is_some_and(|o| o.is_effectively_dead()) {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        if !self.mux.is_already_upgraded() {
            return Ok(UPDATE_SLEEP_FOREVER);
        }
        // "Remove any countermeasures that no longer exist"
        let mut i = 0;
        while i < self.counter_measures.len() {
            if world.objects.find(self.counter_measures[i]).is_none() {
                self.counter_measures.remove(i);
                if !crate::logic::modules::plant(2229) {
                    self.active = self.active.wrapping_sub(1);
                }
            } else {
                i += 1;
            }
        }
        if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_AIRBORNE_TARGET)) && self.available != 0 {
            if self.reaction_frame != 0 && self.reaction_frame == now {
                self.launch_volley(world, me)?;
                self.next_volley_frame = now.wrapping_add(d.frames_between_volleys);
                self.reaction_frame = 0;
            }
            if self.next_volley_frame == now && !crate::logic::modules::plant(2224) {
                self.launch_volley(world, me)?;
                self.next_volley_frame = now.wrapping_add(d.frames_between_volleys);
            }
        }
        if self.available == 0 && d.reload_frames != 0 {
            if self.reload_frame != 0 {
                if self.reload_frame <= now {
                    self.reload_countermeasures();
                }
            } else {
                self.reload_frame = now.wrapping_add(if crate::logic::modules::plant(2225) { 0 } else { d.reload_frames });
            }
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpgradeMuxUser for CountermeasuresBehavior {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: wake now.
    fn upgrade_implementation(&mut self, world: &mut World, _me: ModuleHandle) -> Result<(), IniError> {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::CountermeasuresBehavior(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CountermeasuresBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl UpdateBehavior for CountermeasuresBehavior {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run_update(world, me) {
            Ok(s) => s,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The constructor's second half: `setWakeFrame( getObject(), UPDATE_SLEEP_NONE )`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// `xfer`'s fields after the base: the mux, then (the current version being 2) the flares and the counts.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        self.mux.upgrade_mux_xfer(xfer)?;
        xfer.xfer_stl_object_id_vector(&mut self.counter_measures)?;
        if !self.counter_measures.is_empty() {
            self.allocated = true;
        }
        xfer.xfer_unsigned_int(&mut self.available)?;
        xfer.xfer_unsigned_int(&mut self.active)?;
        if !crate::logic::modules::plant(2226) {
            xfer.xfer_unsigned_int(&mut self.diverted)?;
        }
        xfer.xfer_unsigned_int(&mut self.incoming)?;
        xfer.xfer_unsigned_int(&mut self.reaction_frame)?;
        xfer.xfer_unsigned_int(&mut self.next_volley_frame)
    }
    fn load_post_process_fields(&mut self) {
        self.mux.upgrade_mux_load_post_process();
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::CountermeasuresBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::CountermeasuresBehavior(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        match call {
            ModuleCall::HasCountermeasuresInterface => ModuleReply::Bool(true),
            ModuleCall::CountermeasuresIsActive => ModuleReply::Bool(self.mux.is_already_upgraded()),
            ModuleCall::ReportMissileForCountermeasures { missile } => {
                if let Err(e) = self.report_missile_for_countermeasures(world, missile) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            ModuleCall::CalculateCountermeasureToDivertTo { .. } => match self.calculate_countermeasure_to_divert_to(world, me.object) {
                Ok(id) => ModuleReply::Id(id),
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    ModuleReply::Id(crate::common::game_type::INVALID_ID)
                }
            },
            ModuleCall::ReloadCountermeasures => {
                self.reload_countermeasures();
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for CountermeasuresBehavior {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut base = self.update.clone();
        let r = update_behavior::xfer_own(self, &mut base, xfer);
        self.update = base;
        r
    }
    fn load_post_process(&mut self) {
        self.update.load_post_process();
        self.mux.upgrade_mux_load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constructor_counts_the_volleys_and_the_reload_frame_is_the_pools_zero() {
        let d = CountermeasuresBehaviorModuleData { volley_size: 3, number_of_volleys: 4, ..Default::default() };
        let m = CountermeasuresBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words()[..7], [12, 0, 0, 0, 0, 0, 0]);
    }
}
