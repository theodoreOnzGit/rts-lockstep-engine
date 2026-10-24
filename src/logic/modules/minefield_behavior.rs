// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MinefieldBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/MinefieldBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MinefieldBehavior`: a mine, or a field of "virtual" mines in one object whose health stands for how many are left. Something it
//! should go off for collides with it and it detonates once (the weapon at the point of contact clipped to its footprint, the creation list),
//! its health falling to the share of the mines left; damage and healing detonate or restore mines to match the health. A regenerating
//! field is never destroyed by detonating; it drains away once its creator dies. Mine clearers are immune while they keep touching it; the
//! same detonator must move a little before it sets off another. Scattered mines "scoot" from where they were thrown to where they land.
//!
//! The `LandMineInterface` is [`ModuleCall::LandMineSetScootParms`] and [`ModuleCall::LandMineDisarm`]; the collide interface is
//! [`ModuleCall::OnCollide`].
//!
//! # Upstream facts kept
//!
//! - The constructor wakes the module and gives the object `OBJECT_STATUS_NO_ATTACK_FROM_AI`.
//! - `DetonationWeapon` and `CreationList` are resolved by name as `INI::parseWeaponTemplate` and `INI::parseObjectCreationList` do (an
//!   unknown name is `NULL`); this port keeps the names and resolves them where they are used (the stores do not change after loading).
//!   The weapon is `TheWeaponStore->createAndFireTempWeapon` ([`CutCall::WeaponCreateAndFireTemp`]); the list is created with the mine as both
//!   objects ([`CutCall::OclCreateWithSecondary`]).
//! - The expected mine count is `numVirtualMines * health / maxHealth`, floored for healing and ceiled otherwise, compared with the
//!   unsigned counts as unsigned (C++'s usual arithmetic conversions). A detonation never takes the health under 0.1, and its own damage is
//!   ignored while it is dealt. A draining field's own unresistable damage drops mines without detonating them.
//! - `calcSleepTime`: every frame while draining, scooting or tracking an immune; otherwise forever, or until the next creator check (MSVC's
//!   `min`, unsigned) when it regenerates and stops when its creator dies; never 0.
//! - Scooting: the highest layer under a point at z 99999 (`setLayer`), its height (+1 above a bridge), the ground reached on the last frame
//!   or when below it. `setScootParms` falls for `ceil(sqrt(2 dz / |gravity|))` frames at least.
//! - The xfer saves the mines left, the next creator check, the scoot frames, velocity and acceleration, the three flags and the three
//!   immunities after a count byte (a count other than 3 on a load is `SC_INVALID_DATA`, [`XferStatus::ListNotEmpty`] here); the detonators
//!   are not saved (a load clears them).
//!
//! # Where upstream is undefined
//!
//! Reading the body of a mine without one (`getBodyModule()` unchecked: a detonation that keeps the object, damage or healing, a disarm of
//! a regenerating field, a drain): [`Undefined::NullBodyModule`]. The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R40, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::{real_to_int_ceil, real_to_int_floor, Coord3D};
use crate::common::game_common::{Relationship, RELATIONSHIP_NAMES};
use crate::common::game_type::{INVALID_ID, LAYER_GROUND};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_DOZER, KINDOF_INFANTRY};
use crate::common::model_state::MODELCONDITION_RUBBLE;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_COLLIDE, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_DIE};
use crate::common::object_status_types::{OBJECT_STATUS_MASKED, OBJECT_STATUS_NO_ATTACK_FROM_AI};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_HEALING, DAMAGE_UNRESISTABLE, DEATH_NONE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: u32 = 30;
/// `MIN_HEALTH`: "detonation never puts our health below this, since we probably auto-regen".
const MIN_HEALTH: f32 = 0.1;
/// `MAX_IMMUNITY`: "mines are small, so we can get by with a small fixed number here".
const MAX_IMMUNITY: usize = 3;

/// `class MinefieldBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct MinefieldBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_detonationWeapon`, by the name `INI::parseWeaponTemplate` read (`None` for `None`).
    pub detonation_weapon: Option<Vec<u8>>,
    /// `m_detonatedBy`: bit `r` for each relationship `r` that sets it off.
    pub detonated_by: u32,
    /// `m_stopsRegenAfterCreatorDies`.
    pub stops_regen_after_creator_dies: bool,
    /// `m_regenerates`.
    pub regenerates: bool,
    /// `m_workersDetonate`.
    pub workers_detonate: bool,
    /// `m_creatorDeathCheckRate`.
    pub creator_death_check_rate: u32,
    /// `m_scootFromStartingPointTime`.
    pub scoot_from_starting_point_time: u32,
    /// `m_numVirtualMines`.
    pub num_virtual_mines: u32,
    /// `m_repeatDetonateMoveThresh`.
    pub repeat_detonate_move_thresh: f32,
    /// `m_healthPercentToDrainPerSecond`.
    pub health_percent_to_drain_per_second: f32,
    /// `m_ocl`, by the name `INI::parseObjectCreationList` read (`None` for `None`).
    pub ocl: Option<Vec<u8>>,
}

impl Default for MinefieldBehaviorModuleData {
    /// `MinefieldBehaviorModuleData()`.
    fn default() -> Self {
        MinefieldBehaviorModuleData {
            update: UpdateModuleData::new(),
            detonation_weapon: None,
            detonated_by: (1 << Relationship::Enemies as u32) | (1 << Relationship::Neutral as u32),
            stops_regen_after_creator_dies: true,
            regenerates: false,
            workers_detonate: false,
            creator_death_check_rate: LOGICFRAMES_PER_SECOND,
            scoot_from_starting_point_time: 0,
            num_virtual_mines: 1,
            repeat_detonate_move_thresh: 1.0,
            health_percent_to_drain_per_second: 0.0,
            ocl: None,
        }
    }
}

/// `INI::parseWeaponTemplate` or `INI::parseObjectCreationList`: the next token, `None` for `None`.
fn parse_name(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `MinefieldBehaviorModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<MinefieldBehaviorModuleData>; 12] = [
    FieldParse::new("DetonationWeapon", |i, d, _| { d.detonation_weapon = parse_name(i)?; Ok(()) }, UserData::None),
    FieldParse::new("DetonatedBy", |i, d, _| i.parse_bit_string32(&mut d.detonated_by, &RELATIONSHIP_NAMES), UserData::None),
    FieldParse::new("StopsRegenAfterCreatorDies", |i, d, _| { d.stops_regen_after_creator_dies = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("Regenerates", |i, d, _| { d.regenerates = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("WorkersDetonate", |i, d, _| { d.workers_detonate = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("CreatorDeathCheckRate", |i, d, _| { d.creator_death_check_rate = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScootFromStartingPointTime", |i, d, _| { d.scoot_from_starting_point_time = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("NumVirtualMines", |i, d, _| { d.num_virtual_mines = i.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("RepeatDetonateMoveThresh", |i, d, _| { d.repeat_detonate_move_thresh = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("DegenPercentPerSecondAfterCreatorDies", |i, d, _| { d.health_percent_to_drain_per_second = i.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("CreationList", |i, d, _| { d.ocl = parse_name(i)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl MinefieldBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, MinefieldBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<MinefieldBehaviorModuleData> {
        let mut data = MinefieldBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, MinefieldBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `struct ImmuneInfo`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ImmuneInfo {
    id: ObjectId,
    collide_time: u32,
}

/// `class MinefieldBehavior : public UpdateModule, public CollideModuleInterface, public DamageModuleInterface, public DieModuleInterface,
/// public LandMineInterface`.
#[derive(Clone, Debug)]
pub struct MinefieldBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getMinefieldBehaviorModuleData()`.
    data: Arc<MinefieldBehaviorModuleData>,
    /// `m_nextDeathCheckFrame`.
    next_death_check_frame: u32,
    /// `m_scootFramesLeft`.
    scoot_frames_left: u32,
    /// `m_scootVel`.
    scoot_vel: Coord3D,
    /// `m_scootAccel`.
    scoot_accel: Coord3D,
    /// `m_virtualMinesRemaining`.
    virtual_mines_remaining: u32,
    /// `m_immunes`.
    immunes: [ImmuneInfo; MAX_IMMUNITY],
    /// `m_detonators`: who set off a mine, and where they were.
    detonators: Vec<(ObjectId, Coord3D)>,
    /// `m_ignoreDamage`.
    ignore_damage: bool,
    /// `m_regenerates`.
    regenerates: bool,
    /// `m_draining`.
    draining: bool,
}

impl PartialEq for MinefieldBehavior {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

fn dist_sqr(a: &Coord3D, b: &Coord3D) -> f32 {
    let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
    dx * dx + dy * dy + dz * dz
}

impl MinefieldBehavior {
    /// `getInterfaceMask()`: `UpdateModule`'s, collide, damage and die.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_COLLIDE | MODULEINTERFACE_DAMAGE | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<MinefieldBehaviorModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(MinefieldBehavior {
            update,
            next_death_check_frame: 0,
            scoot_frames_left: 0,
            scoot_vel: Coord3D::default(),
            scoot_accel: Coord3D::default(),
            virtual_mines_remaining: data.num_virtual_mines,
            immunes: [ImmuneInfo::default(); MAX_IMMUNITY],
            detonators: Vec::new(),
            ignore_damage: false,
            regenerates: data.regenerates,
            draining: false,
            data,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getMinefieldBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<MinefieldBehaviorModuleData> {
        &self.data
    }

    /// The state the test compares: the next creator check, the scoot frames, velocity and acceleration, the mines left, the immunities, the
    /// three flags, and the detonators.
    pub fn state_words(&self) -> Vec<u32> {
        let c = |p: &Coord3D| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
        let mut w = vec![self.next_death_check_frame, self.scoot_frames_left];
        w.extend(c(&self.scoot_vel));
        w.extend(c(&self.scoot_accel));
        w.push(self.virtual_mines_remaining);
        for i in &self.immunes {
            w.extend([i.id as u32, i.collide_time]);
        }
        w.extend([u32::from(self.ignore_damage), u32::from(self.regenerates), u32::from(self.draining), self.detonators.len() as u32]);
        for (id, p) in &self.detonators {
            w.push(*id as u32);
            w.extend(c(p));
        }
        w
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

    /// `getBodyModule()`'s health and maximum; [`Undefined::NullBodyModule`] without a body.
    fn health(world: &mut World, id: ObjectId) -> Result<(f32, f32), Undefined> {
        let body = world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        let health = world.module_call(id, body, ModuleCall::GetHealth).as_real(0.0);
        let max = world.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0);
        Ok((health, max))
    }

    /// `obj->attemptDamage(...)` with an unresistable `DamageInfo` from the mine itself.
    fn damage_self(world: &mut World, id: ObjectId, amount: f32, death_type: i32) {
        let mut info = DamageInfo::default();
        info.input.damage_type = DAMAGE_UNRESISTABLE;
        info.input.death_type = death_type;
        info.input.source_id = id;
        info.input.amount = amount;
        world.attempt_damage(id, &mut info);
    }

    /// `calcSleepTime()`.
    fn calc_sleep_time(&self, world: &World) -> UpdateSleepTime {
        // if we're draining we have to update every frame
        if self.draining {
            return UPDATE_SLEEP_NONE;
        }
        // if we're scooting we need to update every frame
        if self.scoot_frames_left > 0 {
            return UPDATE_SLEEP_NONE;
        }
        // if there is anybody in our immulity monitoring we need to update every frame
        if self.immunes.iter().any(|i| i.id != INVALID_ID) && !crate::logic::modules::plant(2101) {
            return UPDATE_SLEEP_NONE;
        }
        let mut sleep_time = FOREVER;
        let now = world.frame();
        // "sleep until the next death check frame we already have figured out if we care about it (that is, when our creator dies)"
        if self.regenerates && self.data.stops_regen_after_creator_dies {
            // MSVC's min(a, b) is `((a) < (b) ? (a) : (b))`
            let until = self.next_death_check_frame.wrapping_sub(now);
            sleep_time = if sleep_time < until { sleep_time } else { until };
        }
        // if we don't want to sleep forever, prevent 0 frame sleeps
        if sleep_time == 0 {
            sleep_time = 1;
        }
        UpdateSleepTime(sleep_time)
    }

    /// `update()`.
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let id = me.object;
        let now = world.frame();
        if self.scoot_frames_left > 0 {
            let mut pt = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
            self.scoot_vel.x += self.scoot_accel.x;
            self.scoot_vel.y += self.scoot_accel.y;
            self.scoot_vel.z += self.scoot_accel.z;
            pt.x += self.scoot_vel.x;
            pt.y += self.scoot_vel.y;
            pt.z += self.scoot_vel.z;
            // "srj sez: scooting mines always go on the highest layer."
            let tmp = Coord3D { x: pt.x, y: pt.y, z: 99999.0 };
            let new_layer = world.terrain_get_highest_layer_for_destination(&tmp);
            world.crate_set_layer(id, new_layer);
            let mut ground = world.terrain_get_layer_height(pt.x, pt.y, new_layer);
            if new_layer != LAYER_GROUND {
                // "ensure we are slightly above the bridge, to account for fudge & sloppy art"
                ground += 1.0;
            }
            if pt.z < ground || self.scoot_frames_left <= 1 {
                pt.z = ground;
            }
            world.set_position(id, &pt);
            self.scoot_frames_left -= 1;
        }
        // check for expired immunities.
        for i in 0..MAX_IMMUNITY {
            if self.immunes[i].id == INVALID_ID {
                continue;
            }
            let expiry = if crate::logic::modules::plant(2102) { 1 } else { 2 };
            if world.objects.find(self.immunes[i].id).is_none() || now > self.immunes[i].collide_time.wrapping_add(expiry) {
                self.immunes[i] = ImmuneInfo::default(); // he's dead, jim.
            }
        }
        if now >= self.next_death_check_frame {
            // "check to see if the building that made me is gone, and whether therefore, I should go"
            if self.regenerates && self.data.stops_regen_after_creator_dies {
                self.next_death_check_frame = now.wrapping_add(self.data.creator_death_check_rate);
                let producer_id = world.objects.find(id).map_or(INVALID_ID, |o| o.producer_id);
                if producer_id != INVALID_ID {
                    let gone = world.objects.find(producer_id).is_none_or(|p| p.is_effectively_dead());
                    if gone {
                        self.regenerates = false;
                        self.draining = true;
                        if let Some(ahb) = world.find_module(id, b"AutoHealBehavior") {
                            world.module_call(id, ahb, ModuleCall::StopHealing);
                        }
                    }
                }
            }
        }
        if self.draining {
            let (_, max) = Self::health(world, id)?;
            let amount = (max * self.data.health_percent_to_drain_per_second) / LOGICFRAMES_PER_SECOND as f32;
            Self::damage_self(world, id, amount, DEATH_NORMAL);
        }
        Ok(self.calc_sleep_time(world))
    }

    /// `detonateOnce(position)`.
    fn detonate_once(&mut self, world: &mut World, me: ModuleHandle, position: &Coord3D) -> Result<(), Undefined> {
        let id = me.object;
        let statics = world.statics.clone();
        if let Some(name) = &self.data.detonation_weapon {
            if let Some(wt) = statics.weapon_store.find_weapon_template(&mut world.names, name).ok().flatten() {
                world.cut(CutCall::WeaponCreateAndFireTemp { weapon: wt.name.clone(), source: id, position: *position });
            }
        }
        if self.virtual_mines_remaining > 0 {
            self.virtual_mines_remaining -= 1;
        }
        if !self.regenerates && self.virtual_mines_remaining == 0 {
            world.body_destroy_object(id);
        } else {
            let percent = self.virtual_mines_remaining as f32 / self.data.num_virtual_mines as f32;
            let (health, max) = Self::health(world, id)?;
            let mut desired = percent * max;
            if desired < MIN_HEALTH && !crate::logic::modules::plant(2103) {
                desired = MIN_HEALTH;
            }
            let amount = health - desired;
            if amount > 0.0 {
                self.ignore_damage = true;
                // "can't use this, AutoHeal won't work unless we go thru normal damage stuff"
                Self::damage_self(world, id, amount, DEATH_NONE);
                self.ignore_damage = false;
            }
        }
        if self.virtual_mines_remaining == 0 {
            world.set_model_condition_state(id, MODELCONDITION_RUBBLE);
            world.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
        } else {
            world.clear_model_condition_state(id, MODELCONDITION_RUBBLE);
            world.set_status(id, mask_of(OBJECT_STATUS_MASKED), false);
        }
        if let Some(ocl) = &self.data.ocl {
            if statics.object_creation_list_store.find_object_creation_list(&mut world.names, ocl).is_some() {
                world.ocl_create_returning(id, id, ocl);
            }
        }
        Ok(())
    }

    /// `onCollide(other, loc, normal)`.
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<(), Undefined> {
        let id = me.object;
        let Some(o) = world.objects.find(other) else { return Ok(()) };
        if o.is_effectively_dead() {
            return Ok(());
        }
        if self.virtual_mines_remaining == 0 {
            return Ok(());
        }
        let now = world.frame();
        // "is this guy in our immune list? NOTE NOTE NOTE, must always do this check FIRST so that 'collideTime' is updated..."
        for i in 0..MAX_IMMUNITY {
            if self.immunes[i].id == other {
                self.immunes[i].collide_time = now;
                return Ok(());
            }
        }
        if !self.data.workers_detonate {
            // infantry+dozer=worker.
            if o.is_kind_of(KINDOF_INFANTRY) && o.is_kind_of(KINDOF_DOZER) {
                return Ok(());
            }
        }
        let other_pos = o.get_position();
        let other_ai = o.ai;
        let r = world.get_relationship(id, other);
        let required_mask = 1u32 << r as u32;
        if self.data.detonated_by & required_mask == 0 && !crate::logic::modules::plant(2104) {
            return Ok(());
        }
        // are we active?
        if self.scoot_frames_left > 0 {
            return Ok(());
        }
        // "things that are in the process of clearing mines are immune to mine detonation"
        if let Some(ai) = other_ai {
            let clearing = world.module_call(other, ai, ModuleCall::AiIsClearingMines).as_bool();
            let has_goal = clearing && {
                let goal = world.module_call(other, ai, ModuleCall::AiGetGoalObject).as_id();
                world.objects.find(goal).is_some()
            };
            if has_goal {
                for i in 0..MAX_IMMUNITY {
                    if self.immunes[i].id == INVALID_ID || self.immunes[i].id == other {
                        self.immunes[i] = ImmuneInfo { id: other, collide_time: now };
                        // wake up
                        let sleep = self.calc_sleep_time(world);
                        update_behavior::set_wake_frame(world, sleep);
                        break;
                    }
                }
                return Ok(());
            }
        }
        // "if we detonated another one nearby, we have to move a little bit to detonate another one."
        let thresh = self.data.repeat_detonate_move_thresh;
        match self.detonators.iter_mut().find(|(d, _)| *d == other) {
            Some((_, where_)) => {
                if dist_sqr(&other_pos, where_) <= thresh * thresh && !crate::logic::modules::plant(2105) {
                    // too close. punt for now.
                    return Ok(());
                }
                // far enough. update the loc, then break out and blow up.
                *where_ = other_pos;
            }
            None => self.detonators.push((other, other_pos)),
        }
        let mut det_pt = other_pos;
        let Some(me_obj) = world.objects.find(id) else { return Ok(()) };
        if !crate::logic::modules::plant(2106) {
            me_obj.geometry_info.clip_point_to_footprint(&me_obj.get_position(), &mut det_pt);
        }
        self.detonate_once(world, me, &det_pt)
    }

    /// `onDamage(damageInfo)` (and `onHealing`): "detonate as many times as neccessary for our virtual mine count to match our health".
    fn on_damage(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        if self.ignore_damage {
            return Ok(());
        }
        let id = me.object;
        let n = self.data.num_virtual_mines;
        loop {
            let (health, max) = Self::health(world, id)?;
            let expected_f = n as f32 * health / max;
            let healing = info.input.damage_type == DAMAGE_HEALING && !crate::logic::modules::plant(2109);
            let expected = if healing { real_to_int_floor(expected_f) } else { real_to_int_ceil(expected_f) };
            // compared unsigned (the counts are UnsignedInt)
            let mut expected = expected as u32;
            if expected > n {
                expected = n;
            }
            if self.virtual_mines_remaining < expected {
                self.virtual_mines_remaining = expected;
            } else if self.virtual_mines_remaining > expected {
                if self.draining && info.input.source_id == id && info.input.damage_type == DAMAGE_UNRESISTABLE && !crate::logic::modules::plant(2107) {
                    // "don't detonate.... just ditch a mine"
                    self.virtual_mines_remaining -= 1;
                } else {
                    let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
                    self.detonate_once(world, me, &pos)?;
                }
            } else {
                break;
            }
        }
        if self.virtual_mines_remaining == 0 {
            // "if someone did weapon damage they may have nuked our health to zero, which would be bad if we regen. prevent this. (srj)"
            let (health, _) = Self::health(world, id)?;
            if self.regenerates && health < MIN_HEALTH {
                if let Some(body) = world.get_body_module(id) {
                    world.module_call(id, body, ModuleCall::InternalChangeHealth(MIN_HEALTH - health));
                }
            }
            world.set_model_condition_state(id, MODELCONDITION_RUBBLE);
            world.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
        } else {
            world.clear_model_condition_state(id, MODELCONDITION_RUBBLE);
            world.set_status(id, mask_of(OBJECT_STATUS_MASKED), false);
        }
        Ok(())
    }

    /// `disarm()`.
    fn disarm(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let id = me.object;
        if !self.regenerates {
            world.body_destroy_object(id);
            return Ok(());
        }
        let (health, _) = Self::health(world, id)?;
        let amount = health - MIN_HEALTH;
        self.ignore_damage = true;
        Self::damage_self(world, id, amount, DEATH_NONE);
        self.ignore_damage = false;
        self.virtual_mines_remaining = 0;
        world.set_model_condition_state(id, MODELCONDITION_RUBBLE);
        world.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
        Ok(())
    }

    /// `setScootParms(start, end)`.
    fn set_scoot_parms(&mut self, world: &mut World, me: ModuleHandle, start: Coord3D, end: Coord3D) {
        let id = me.object;
        let mut scoot_time = self.data.scoot_from_starting_point_time;
        let mut end_on_ground = end;
        end_on_ground.z = world.terrain_get_ground_height(end_on_ground.x, end_on_ground.y);
        if start.z > end_on_ground.z {
            // "figure out how long it will take to fall, and replace scoot time with that"
            let gravity = world.cuts.globals.gravity;
            let falling_time = real_to_int_ceil((2.0 * (start.z - end_on_ground.z) / gravity.abs()).sqrt()) as u32;
            // "we can scoot after we land, but don't want to stop scooting before we land"
            if scoot_time < falling_time {
                scoot_time = falling_time;
            }
        }
        if scoot_time == 0 {
            world.set_position(id, &end_on_ground);
            self.scoot_frames_left = 0;
            return;
        }
        // x = x0 + vt + 0.5at^2, thus 2(dx - vt)/t^2 = a
        let dx = end_on_ground.x - start.x;
        let dy = end_on_ground.y - start.y;
        let dz = end_on_ground.z - start.z;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist <= 0.1 && dz.abs() <= 0.1 {
            world.set_position(id, &end_on_ground);
            self.scoot_frames_left = 0;
            return;
        }
        let t = scoot_time as f32;
        let speed = dist / t;
        let accel_mag = (2.0 * (dist - speed * t) / (t * t)).abs();
        let dx_norm = if dist <= 0.1 { 0.0 } else { dx / dist };
        let dy_norm = if dist <= 0.1 { 0.0 } else { dy / dist };
        self.scoot_vel.x = dx_norm * speed;
        self.scoot_vel.y = dy_norm * speed;
        self.scoot_accel.x = -dx_norm * accel_mag;
        self.scoot_accel.y = -dy_norm * accel_mag;
        self.scoot_accel.z = world.cuts.globals.gravity;
        world.set_position(id, &start);
        self.scoot_frames_left = scoot_time;
        // "we need to wake ourselves up because we could be lying here sleeping forever"
        let sleep = self.calc_sleep_time(world);
        update_behavior::set_wake_frame(world, sleep);
    }

    fn undefined(world: &mut World, me: ModuleHandle, r: Result<(), Undefined>) {
        if let Err(u) = r {
            world.cuts.module_errors.push((me, IniError::undefined(u)));
        }
    }
}

impl UpdateBehavior for MinefieldBehavior {
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
    /// The constructor's world half: awake, and "mines aren't auto-acquirable".
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        world.set_status(me.object, mask_of(OBJECT_STATUS_NO_ATTACK_FROM_AI), true);
    }
    /// `xfer`'s fields after the base.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.virtual_mines_remaining)?;
        xfer.xfer_unsigned_int(&mut self.next_death_check_frame)?;
        xfer.xfer_unsigned_int(&mut self.scoot_frames_left)?;
        xfer.xfer_coord3d(&mut self.scoot_vel)?;
        xfer.xfer_coord3d(&mut self.scoot_accel)?;
        xfer.xfer_bool(&mut self.ignore_damage)?;
        xfer.xfer_bool(&mut self.regenerates)?;
        if !crate::logic::modules::plant(2108) {
            xfer.xfer_bool(&mut self.draining)?;
        }
        let mut max_immunity = MAX_IMMUNITY as u8;
        xfer.xfer_unsigned_byte(&mut max_immunity)?;
        if max_immunity as usize != MAX_IMMUNITY {
            // "MAX_IMMUNITY has changed size, you must version this code": throw SC_INVALID_DATA
            return Err(XferStatus::ListNotEmpty);
        }
        for i in self.immunes.iter_mut() {
            xfer.xfer_object_id(&mut i.id)?;
            xfer.xfer_unsigned_int(&mut i.collide_time)?;
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            self.detonators.clear();
        }
        Ok(())
    }
    /// The object's calls: the collide, damage and die interfaces and the `LandMineInterface`.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        let r = match call {
            ModuleCall::OnCollide { other, .. } => self.on_collide(world, me, other),
            ModuleCall::OnDamage(info) | ModuleCall::OnHealing(info) => self.on_damage(world, me, &info),
            ModuleCall::OnBodyDamageStateChange { .. } => Ok(()),
            ModuleCall::OnDie(_) => {
                world.body_destroy_object(me.object);
                Ok(())
            }
            ModuleCall::LandMineSetScootParms { start, end } => {
                self.set_scoot_parms(world, me, start, end);
                Ok(())
            }
            ModuleCall::LandMineDisarm => self.disarm(world, me),
            _ => return ModuleReply::Unhandled,
        };
        Self::undefined(world, me, r);
        ModuleReply::Done
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MinefieldBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MinefieldBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for MinefieldBehavior {
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

/// The factory's `NewModuleDataProc` for `MinefieldBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MinefieldBehavior(Arc::new(MinefieldBehaviorModuleData::friend_new_module_data(ini)?)))
}
