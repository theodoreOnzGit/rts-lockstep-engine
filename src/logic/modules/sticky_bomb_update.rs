// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/StickyBombUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/StickyBombUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `StickyBombUpdate`: a bomb stuck to a target. Made by a shooter, it finds its target as the shooter's AI goal; it rides on the target
//! (at ground height on an immobile one), pings once a second (on the second boundaries of its countdown, if it has a lifetime), dies with
//! its target, and when detonated hurts everything near the target by the target's size, then dies (its death modules fire the weapon).
//!
//! The calls other modules make are [`ModuleCall::StickyBombInit`], `StickyBombIsTimedBomb`, `StickyBombGetDetonationFrame`,
//! `StickyBombGetTargetObject`, `StickyBombSetTargetObject` and `Detonate`.
//!
//! # Upstream facts kept
//!
//! - The target is also set as the bomb's producer. The countdown is `LifetimeUpdate`'s die frame, found by name; without one the bomb is
//!   remotely triggered (die frame 0, a ping in a second). The ping arithmetic is unsigned (a die frame already past wraps).
//! - The bomb is placed only when there is a target: at a given position, or the bomber's position on an immobile target (both at ground
//!   height), or `OffsetZ` above the target. A `BOOBY_TRAP` bomb marks its target `BOOBY_TRAPPED` (and clears it on detonating).
//! - The created sound is a copy of the template's `StickyBombCreated` at the bomb's position; the ping is `UnitBombPing` on the bomb.
//! - `GeometryBasedDamageWeapon` is the weapon store's template of that name, as `INI::parseWeaponTemplate` resolves it (`None`, or an
//!   unknown name, is `NULL`); this port keeps the name and resolves it when the bomb detonates (the store does not change after loading).
//!   Its radii grow by the target's bounding circle; the scan is `FROM_BOUNDINGSPHERE_3D` with the larger (MSVC's `max`); each victim takes
//!   the primary damage inside the primary radius (by the iterator's numeric) and the secondary otherwise, from the bomb, with its player's
//!   mask; the FX is at the target with the secondary radius. `AttachToTargetBone` is parsed and never read.
//! - `update` never sleeps. The xfer is the version (1), the `UpdateModule` base, the target, the die frame and the next ping frame.
//!
//! # Where upstream is undefined
//!
//! A geometry-based detonation of a bomb with no controlling player (`getControllingPlayer()->getPlayerMask()`:
//! [`Undefined::NullControllingPlayer`]), and a detonation of a bomb without a body (`Object::kill` reads it unchecked:
//! [`Undefined::NullBodyModule`]). The case is recorded in `world.cuts.module_errors` and the call returns.
//!
//! `p1w13-sonnet`, batch R39, 2026-10-24.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_BOOBY_TRAP, KINDOF_IMMOBILE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_BOOBY_TRAPPED;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DAMAGE_UNRESISTABLE, DEATH_NORMAL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::WeaponBonus;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::object_state::mask_of;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `LOGICFRAMES_PER_SECOND`.
const LOGICFRAMES_PER_SECOND: u32 = 30;
/// `FROM_BOUNDINGSPHERE_3D` (`DistanceCalculationType`).
const FROM_BOUNDINGSPHERE_3D: i32 = 3;

/// `class StickyBombUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct StickyBombUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_attachToBone` (never read).
    pub attach_to_bone: Vec<u8>,
    /// `m_offsetZ`.
    pub offset_z: f32,
    /// `m_geometryBasedDamageWeaponTemplate`, by the name `INI::parseWeaponTemplate` read (`None` for `None`).
    pub geometry_based_damage_weapon: Option<Vec<u8>>,
    /// `m_geometryBasedDamageFX` (`None` for `NULL`).
    pub geometry_based_damage_fx: Option<Vec<u8>>,
}

impl Default for StickyBombUpdateModuleData {
    /// `StickyBombUpdateModuleData()`.
    fn default() -> Self {
        StickyBombUpdateModuleData {
            update: UpdateModuleData::new(),
            attach_to_bone: Vec::new(),
            offset_z: 10.0,
            geometry_based_damage_weapon: None,
            geometry_based_damage_fx: None,
        }
    }
}

/// `INI::parseFXList` or `INI::parseWeaponTemplate`: the next token, `None` for `None`.
fn parse_name(ini: &mut Ini<'_>) -> IniResult<Option<Vec<u8>>> {
    let name = ini.get_next_token(None)?;
    Ok(if name.eq_ignore_ascii_case(b"None") { None } else { Some(name) })
}

/// `StickyBombUpdateModuleData::buildFieldParse`'s table.
static FIELD_PARSE: [FieldParse<StickyBombUpdateModuleData>; 5] = [
    FieldParse::new("AttachToTargetBone", |i, d, _| { d.attach_to_bone = i.parse_ascii_string()?; Ok(()) }, UserData::None),
    FieldParse::new("OffsetZ", |i, d, _| { d.offset_z = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("GeometryBasedDamageWeapon", |i, d, _| { d.geometry_based_damage_weapon = parse_name(i)?; Ok(()) }, UserData::None),
    FieldParse::new("GeometryBasedDamageFX", |i, d, _| { d.geometry_based_damage_fx = parse_name(i)?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl StickyBombUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's fields.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, StickyBombUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<StickyBombUpdateModuleData> {
        let mut data = StickyBombUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, StickyBombUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class StickyBombUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct StickyBombUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getStickyBombUpdateModuleData()`.
    data: Arc<StickyBombUpdateModuleData>,
    /// `m_targetID`.
    target_id: ObjectId,
    /// `m_dieFrame`.
    die_frame: u32,
    /// `m_nextPingFrame`.
    next_ping_frame: u32,
}

impl PartialEq for StickyBombUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl StickyBombUpdate {
    /// `getInterfaceMask()`: `UpdateModule`'s.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<StickyBombUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(StickyBombUpdate { update, data, target_id: INVALID_ID, die_frame: 0, next_ping_frame: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getStickyBombUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<StickyBombUpdateModuleData> {
        &self.data
    }

    /// The state the test compares: the target, the die frame, the next ping frame.
    pub fn state_words(&self) -> [u32; 3] {
        [self.target_id as u32, self.die_frame, self.next_ping_frame]
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

    /// `isTimedBomb()`.
    pub fn is_timed_bomb(&self) -> bool {
        self.die_frame > 0
    }

    /// `onObjectCreated()`: the shooter (the bomb's producer) has its target as its AI's goal.
    fn on_object_created(&mut self, world: &mut World, me: ModuleHandle) {
        let shooter = world.objects.find(me.object).map_or(INVALID_ID, |o| o.producer_id);
        let Some(ai) = world.objects.find(shooter).and_then(|o| o.ai) else { return };
        let target = world.module_call(shooter, ai, ModuleCall::AiGetGoalObject).as_id();
        if world.objects.find(target).is_some() {
            self.init_sticky_bomb(world, me, target, INVALID_ID, None);
        }
    }

    /// `initStickyBomb(target, bomber, specificPos)`.
    fn init_sticky_bomb(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId, bomber: ObjectId, specific_pos: Option<Coord3D>) {
        let id = me.object;
        let target = world.objects.find(target).map(|_| target);
        // Store the target.
        self.target_id = target.unwrap_or(INVALID_ID);
        // "Set the target as the producer ID -- so we can access it from other areas."
        if let Some(o) = world.objects.find_mut(id) {
            o.producer_id = target.unwrap_or(INVALID_ID);
        }
        let now = world.frame();
        // "Also determine our lifetime.... for countdown purposes."
        let lifetime = world.find_module(id, b"LifetimeUpdate");
        let die_frame = lifetime.and_then(|l| match world.module_call(id, l, ModuleCall::LifetimeGetDieFrame) {
            ModuleReply::Int(f) => Some(f as u32),
            _ => None,
        });
        if let Some(die_frame) = die_frame {
            // we are a timer bomb
            self.die_frame = die_frame;
            // Calculate the number of seconds (rounded down)
            let pings = self.die_frame.wrapping_sub(now) / LOGICFRAMES_PER_SECOND;
            // Now determine the next frame we will make a "ping" sound.
            let step = if crate::logic::modules::plant(2092) { LOGICFRAMES_PER_SECOND - 1 } else { LOGICFRAMES_PER_SECOND };
            self.next_ping_frame = self.die_frame.wrapping_sub(pings.wrapping_mul(step));
        } else {
            // we are a remotely triggered bomb.
            self.die_frame = 0;
            // "Because we don't die -- make our ping in a second."
            self.next_ping_frame = now.wrapping_add(LOGICFRAMES_PER_SECOND);
        }
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        let Some(target) = target else { return };
        let Some(t) = world.objects.find(target) else { return };
        let mut pos = t.get_position();
        let immobile = t.is_kind_of(KINDOF_IMMOBILE);
        let bomber = world.objects.find(bomber).map(|b| b.get_position());
        if let Some(p) = specific_pos {
            pos = p;
            pos.z = world.terrain_get_ground_height(pos.x, pos.y);
        } else if let (true, Some(b)) = (immobile, bomber) {
            // "if bomber has placed bomb on a structure let the bomb just stay where it was first put, so a mine clearing unit can get to it"
            pos = b;
            pos.z = world.terrain_get_ground_height(pos.x, pos.y);
        } else {
            // ride on the roof of the truck/tank
            pos.z += if crate::logic::modules::plant(2093) { 0.0 } else { self.data.offset_z };
        }
        world.set_position(id, &pos);
        if world.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_BOOBY_TRAP)) {
            // "This kind of sticky bomb needs to set a status, so the poor victim can trigger us from assorted places"
            world.set_status(target, mask_of(OBJECT_STATUS_BOOBY_TRAPPED), true);
        }
        let Some(o) = world.objects.find(id) else { return };
        let name = o.template().get_per_unit_sound(b"StickyBombCreated").event_name.clone();
        let position = o.get_position();
        let object_id = if crate::logic::modules::plant(2091) { id } else { INVALID_ID };
        world.emit(ObjectClientEvent::AudioAddEvent { name, object_id, position: Some(position), player_index: None });
    }

    /// `update()`: "continually reset position of stickybomb to match the position of the target", and the ping.
    fn update_inner(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let id = me.object;
        if let Some(t) = world.objects.find(self.target_id) {
            if t.is_effectively_dead() {
                // If the target is dead, then
                world.body_destroy_object(id);
                return UPDATE_SLEEP_NONE;
            }
            let target_pos = t.get_position();
            let new_pos = if t.is_kind_of(KINDOF_IMMOBILE) && !crate::logic::modules::plant(2094) {
                let pos = world.objects.find(id).map(|o| o.get_position()).unwrap_or_default();
                // keep it at ground height for mine clearing units to reach
                Coord3D { x: pos.x, y: pos.y, z: world.terrain_get_ground_height(pos.x, pos.y) }
            } else {
                // make the bomb follow the target around
                Coord3D { x: target_pos.x, y: target_pos.y, z: target_pos.z + self.data.offset_z }
            };
            world.set_position(id, &new_pos);
        }
        let now = world.frame();
        if now >= self.next_ping_frame && !(crate::logic::modules::plant(2095) && now == self.next_ping_frame) {
            self.next_ping_frame = self.next_ping_frame.wrapping_add(LOGICFRAMES_PER_SECOND);
            // Play the "ping" sound.
            if let Some(o) = world.objects.find(id) {
                let name = o.template().get_per_unit_sound(b"UnitBombPing").event_name.clone();
                world.emit(ObjectClientEvent::AudioAddEvent { name, object_id: id, position: None, player_index: None });
            }
        }
        UPDATE_SLEEP_NONE
    }

    /// `detonate()`.
    fn detonate(&mut self, world: &mut World, me: ModuleHandle) -> Result<(), Undefined> {
        let id = me.object;
        let trapped = world.objects.find(self.target_id).map(|_| self.target_id);
        let statics = world.statics.clone();
        let weapon = match &self.data.geometry_based_damage_weapon {
            Some(name) => statics.weapon_store.find_weapon_template(&mut world.names, name).ok().flatten(),
            None => None,
        };
        if let (Some(wt), Some(trapped)) = (weapon, trapped) {
            // "We need to hurt people based on the size of the thing we are on. The radius in our weapon is the radius beyond our borders"
            let null_bonus = WeaponBonus::default();
            let t = world.objects.find(trapped).expect("found above");
            let bounding_circle = t.geometry_info.get_bounding_circle_radius();
            let trapped_pos = t.get_position();
            let primary_damage = wt.get_primary_damage(&null_bonus);
            let secondary_damage = wt.get_secondary_damage(&null_bonus);
            let mut primary_damage_range = wt.get_primary_damage_radius(&null_bonus);
            let mut secondary_damage_range = wt.get_secondary_damage_radius(&null_bonus);
            if !crate::logic::modules::plant(2096) {
                primary_damage_range += bounding_circle;
            }
            secondary_damage_range += bounding_circle;
            let primary_damage_range_sqr = primary_damage_range * primary_damage_range;
            // MSVC's max(a, b) is `((a) > (b) ? (a) : (b))`
            let radius = if primary_damage_range > secondary_damage_range { primary_damage_range } else { secondary_damage_range };
            let victims = world.partition_iterate_objects_in_range_numeric_at(trapped_pos, radius, FROM_BOUNDINGSPHERE_3D);
            let mut info = DamageInfo::default();
            info.input.damage_type = wt.damage_type;
            info.input.death_type = wt.death_type;
            info.input.source_id = id;
            let player = world.get_controlling_player(id).ok_or(Undefined::NullControllingPlayer)?;
            info.input.source_player_mask = world.cuts.player_mask(player);
            info.input.damage_status_type = wt.damage_status_type;
            for (victim, dist_sqr) in victims {
                let inside = crate::logic::modules::plant(2097) || dist_sqr <= primary_damage_range_sqr;
                info.input.amount = if inside { primary_damage } else { secondary_damage };
                world.attempt_damage(victim, &mut info);
            }
            if let Some(fx) = &self.data.geometry_based_damage_fx {
                // And we make FX based on that size too.
                world.emit(ObjectClientEvent::FxListDoFxPosRadius { fx: fx.clone(), position: trapped_pos, radius: secondary_damage_range });
            }
        }
        if let Some(trapped) = trapped {
            if world.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_BOOBY_TRAP)) && !crate::logic::modules::plant(2098) {
                world.set_status(trapped, mask_of(OBJECT_STATUS_BOOBY_TRAPPED), false);
            }
        }
        // "Most things just fire weapons in their death modules"; Object::kill reads the body unchecked
        world.get_body_module(id).ok_or(Undefined::NullBodyModule)?;
        world.kill(id, DAMAGE_UNRESISTABLE, DEATH_NORMAL);
        Ok(())
    }
}

impl UpdateBehavior for StickyBombUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        self.update_inner(world, me)
    }
    /// The constructor's world half: asleep until `initStickyBomb`.
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    /// `xfer`'s fields after the base: the target, the die frame, the next ping frame.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.target_id)?;
        xfer.xfer_unsigned_int(&mut self.die_frame)?;
        if crate::logic::modules::plant(2099) {
            return Ok(());
        }
        xfer.xfer_unsigned_int(&mut self.next_ping_frame)
    }
    /// The object's calls: `onObjectCreated` and the bomb's interface.
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnObjectCreated => {
                self.on_object_created(world, me);
                ModuleReply::Done
            }
            ModuleCall::StickyBombInit { target, bomber, specific_pos } => {
                self.init_sticky_bomb(world, me, target, bomber, specific_pos);
                ModuleReply::Done
            }
            ModuleCall::StickyBombIsTimedBomb => ModuleReply::Bool(self.is_timed_bomb()),
            ModuleCall::StickyBombGetDetonationFrame => ModuleReply::Int(self.die_frame as i32),
            ModuleCall::StickyBombGetTargetObject => ModuleReply::Id(world.objects.find(self.target_id).map_or(INVALID_ID, |_| self.target_id)),
            ModuleCall::StickyBombSetTargetObject(obj) => {
                self.target_id = if world.objects.find(obj).is_some() { obj } else { INVALID_ID };
                ModuleReply::Done
            }
            ModuleCall::Detonate => {
                if let Err(u) = self.detonate(world, me) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::StickyBombUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::StickyBombUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for StickyBombUpdate {
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

/// The factory's `NewModuleDataProc` for `StickyBombUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::StickyBombUpdate(Arc::new(StickyBombUpdateModuleData::friend_new_module_data(ini)?)))
}
