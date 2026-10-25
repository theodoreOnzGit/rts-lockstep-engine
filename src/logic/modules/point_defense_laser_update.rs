// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/PointDefenseLaserUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/PointDefenseLaserUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `PointDefenseLaserUpdate`: a laser that shoots down incoming projectiles (the Avenger's and the Paladin's). Every `ScanRate` frames it
//! scans `ScanRange` around the object for the closest enemy of `PrimaryTargetTypes` (else `SecondaryTargetTypes`), preferring one already
//! within the weapon's attack range, and between scans tracks only that target; when the target is in range and the laser has cooled down,
//! it fires a temporary weapon of `WeaponTemplate` at it (made, loaded, fired and thrown away) and waits the template's delay between
//! shots. A target that leaves the range, or dies, forces a new scan within 0 to 3 frames.
//!
//! # Upstream facts kept
//!
//! - The data is a plain `ModuleData`; `UpdateModuleData` adds no field.
//! - The out-of-range candidate's predicted position (velocity times `PredictTargetVelocityFactor`) is computed and never used: the
//!   distance is measured again from the same positions. The prediction has no effect and is not translated; the physics module is only
//!   read.
//! - A target in range that is dead is not shot, but still forces the rescan.
//! - `WeaponTemplate` is `INI::parseWeaponTemplate`: `None` and an unknown name are `NULL`, and the scan and the tracking read the
//!   template through it ([`Undefined::NullWeaponTemplate`]); `onObjectCreated`'s checks are debug crashes only.
//!
//! `p1w13-sonnet`, batch R66, 2026-10-25.

use std::sync::Arc;

use crate::common::game_common::Relationship;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KindOfMaskType, KINDOF_IMMOBILE};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon::WEAPON_ANTI_GROUND;
use crate::logic::weapon_bonus::WeaponBonus;
use crate::logic::weapon_fire::Weapon;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `TERTIARY_WEAPON`: the slot of the temporary weapon.
const TERTIARY_WEAPON: i32 = 2;
/// `FROM_CENTER_2D`.
const FROM_CENTER_2D: i32 = 0;

/// `class PointDefenseLaserUpdateModuleData : public ModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PointDefenseLaserUpdateModuleData {
    /// The `ModuleData` base (an `UpdateModuleData` adds no field).
    pub update: UpdateModuleData,
    /// `m_weaponTemplate`, by name (looked up in the store when used; `None` and an unknown name are `NULL`).
    pub weapon_template: Option<Vec<u8>>,
    /// `m_primaryTargetKindOf`, `m_secondaryTargetKindOf`.
    pub primary_target_kind_of: KindOfMaskType,
    pub secondary_target_kind_of: KindOfMaskType,
    /// `m_scanFrames`.
    pub scan_frames: u32,
    /// `m_scanRange`.
    pub scan_range: f32,
    /// `m_velocityFactor`.
    pub velocity_factor: f32,
}

static FIELD_PARSE: [FieldParse<PointDefenseLaserUpdateModuleData>; 7] = [
    FieldParse::new("WeaponTemplate", |i, d, _| { d.weapon_template = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("PrimaryTargetTypes", |i, d, _| d.primary_target_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("SecondaryTargetTypes", |i, d, _| d.secondary_target_kind_of.parse_from_ini(i), UserData::None),
    FieldParse::new("ScanRate", |i, d, _| { d.scan_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ScanRange", |i, d, _| { d.scan_range = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::new("PredictTargetVelocityFactor", |i, d, _| { d.velocity_factor = i.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl PointDefenseLaserUpdateModuleData {
    /// `buildFieldParse(p)`: `ModuleData`'s (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, PointDefenseLaserUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<PointDefenseLaserUpdateModuleData> {
        let mut data = PointDefenseLaserUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, PointDefenseLaserUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `PointDefenseLaserUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::PointDefenseLaserUpdate(Arc::new(PointDefenseLaserUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class PointDefenseLaserUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct PointDefenseLaserUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<PointDefenseLaserUpdateModuleData>,
    /// `m_bestTargetID`.
    best_target_id: ObjectId,
    /// `m_inRange`.
    in_range: bool,
    /// `m_nextScanFrames`.
    next_scan_frames: i32,
    /// `m_nextShotAvailableInFrames`.
    next_shot_available_in_frames: i32,
}

impl PartialEq for PointDefenseLaserUpdate {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl PointDefenseLaserUpdate {
    /// The constructor's first half (it touches no world): the base and the zeroed state (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, data: Option<Arc<PointDefenseLaserUpdateModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(PointDefenseLaserUpdate { update, data, best_target_id: INVALID_ID, in_range: false, next_scan_frames: 0, next_shot_available_in_frames: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<PointDefenseLaserUpdateModuleData> {
        &self.data
    }

    /// The state: the best target, whether it is in range, the frames to the next scan and to the next shot.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.best_target_id as u32, u32::from(self.in_range), self.next_scan_frames as u32, self.next_shot_available_in_frames as u32]
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

    /// The weapon template's place in the store (`None` for `NULL`).
    fn template(&self, world: &World) -> Option<usize> {
        let store = &world.statics.weapon_store;
        self.data.weapon_template.as_deref().and_then(|n| store.templates().iter().position(|t| t.name.eq_ignore_ascii_case(n)))
    }

    /// `m_weaponTemplate->getAttackRange(bonus)` with a cleared bonus, read through the template.
    fn fire_range(&self, world: &World) -> Result<f32, Undefined> {
        let t = self.template(world).ok_or(Undefined::NullWeaponTemplate)?;
        Ok(world.statics.weapon_store.templates()[t].get_attack_range(&WeaponBonus::new()))
    }

    /// `sqrt(ThePartitionManager->getDistanceSquared(me, other, FROM_CENTER_2D))`.
    fn distance(world: &mut World, me: ObjectId, other: ObjectId) -> f32 {
        world.partition_get_distance_squared_center_2d(me, other).sqrt()
    }

    /// "Set target to NULL ... re-evaluate by forcing a new scan": a rescan in 0 to 3 frames, at once if that is 0.
    fn force_rescan(&mut self, world: &mut World, h: ModuleHandle) -> Result<(), Undefined> {
        let me = h.object;
        self.next_scan_frames = world.random.game_logic_random_value(0, 3);
        self.best_target_id = INVALID_ID;
        if self.next_scan_frames == 0 {
            self.scan_closest_target(world, me)?;
            self.next_scan_frames = self.data.scan_frames as i32;
            return Ok(());
        }
        Ok(())
    }

    /// `fireWhenReady()`.
    fn fire_when_ready(&mut self, world: &mut World, h: ModuleHandle) -> Result<(), Undefined> {
        let me = h.object;
        // "Track our target"
        let mut target = world.objects.find(self.best_target_id).map(|_| self.best_target_id);
        if let Some(t) = target {
            let fire_range = self.fire_range(world)?;
            let dist = Self::distance(world, me, t);
            if dist < fire_range {
                // "We are currently in range!"
                self.in_range = true;
            } else if self.in_range {
                // "We were in range last frame, but the target has moved out of firing range, so re-evaluate by forcing a new scan."
                self.next_scan_frames = world.random.game_logic_random_value(0, 3);
                self.best_target_id = INVALID_ID;
                if self.next_scan_frames == 0 {
                    self.scan_closest_target(world, me)?;
                    self.next_scan_frames = self.data.scan_frames as i32;
                    target = None; // "so we don't shoot at it (might be out of range)"
                }
            } else {
                self.in_range = false; // "Not in range"
            }
        }
        if self.next_shot_available_in_frames > 0 {
            // "We can't fire this frame."
            self.next_shot_available_in_frames -= 1;
            return Ok(());
        }
        let Some(wt) = self.template(world) else { return Ok(()) };
        // "Fire control!"
        let Some(t) = target else { return Ok(()) };
        if !self.in_range && !crate::logic::modules::plant(2361) {
            return Ok(());
        }
        let dead = |w: &World| w.objects.find(t).is_some_and(|o| o.is_effectively_dead());
        if !dead(world) {
            let statics = world.statics.clone();
            let mut weapon = Weapon::new_in_store(&statics.weapon_store, world.frame(), wt, TERTIARY_WEAPON);
            world.module_weapon_load_ammo_now(&mut weapon, me);
            if let Err(e) = world.module_weapon_fire_at_object(&mut weapon, me, t) {
                world.cuts.module_errors.push((h, e));
            }
            // "And now that we have shot, set our internal reload timer."
            let delay = statics.weapon_store.templates()[wt].get_delay_between_shots(&WeaponBonus::new(), &mut world.random);
            self.next_shot_available_in_frames = if crate::logic::modules::plant(2362) { 0 } else { delay };
        }
        if dead(world) && !crate::logic::modules::plant(2363) {
            self.force_rescan(world, h)?;
        }
        Ok(())
    }

    /// `scanClosestTarget()`: whether a target was found (`m_bestTargetID`, `m_inRange` set either way).
    fn scan_closest_target(&mut self, world: &mut World, me: ObjectId) -> Result<bool, Undefined> {
        let d = self.data.clone();
        let fire_range = self.fire_range(world)?;
        let anti_mask = self.template(world).map_or(0, |t| world.statics.weapon_store.templates()[t].anti_mask);
        let mut out_of_range: [Option<(ObjectId, f32)>; 2] = [None, None];
        let mut in_range: [Option<(ObjectId, f32)>; 2] = [None, None];
        let pos = world.objects.find(me).map(|o| o.get_position()).unwrap_or_default();
        for other in world.partition_iterate_objects_in_range_at(pos, d.scan_range, FROM_CENTER_2D) {
            let Some(o) = world.objects.find(other) else { continue };
            let index = if o.is_any_kind_of(&d.primary_target_kind_of) {
                0 // "Primary target type"
            } else if o.is_any_kind_of(&d.secondary_target_kind_of) && !crate::logic::modules::plant(2364) {
                1 // "Secondary target type (use only if we can't find any primary targets)"
            } else {
                continue; // "Not a valid target."
            };
            // "Borrow" the check for being an AA only laser to stop from shooting planes on airports.
            if !o.test_status(OBJECT_STATUS_AIRBORNE_TARGET) && anti_mask & WEAPON_ANTI_GROUND == 0 {
                continue;
            }
            let stealthed = o.test_status(OBJECT_STATUS_STEALTHED) && !o.test_status(OBJECT_STATUS_DETECTED) && !o.test_status(OBJECT_STATUS_DISGUISED);
            let immobile = o.is_kind_of(KINDOF_IMMOBILE);
            // "order matters: we want to know if I consider it to be an enemy, not vice versa"
            if world.get_relationship(me, other) != Relationship::Enemies && !crate::logic::modules::plant(2365) {
                continue; // "Don't kill our friends!"
            }
            if stealthed {
                continue; // "We can't see it."
            }
            let mut dist = Self::distance(world, me, other);
            if dist <= fire_range {
                // "Inside fire range (which one is closest?)"
                if in_range[index].is_none_or(|(_, c)| dist < c) {
                    in_range[index] = Some((other, dist));
                }
            } else if in_range[index].is_none() {
                // "Determine where the target will be based on current velocity": the predicted position is never used, and the distance is
                // measured again from the same positions (with a physics module)
                if d.velocity_factor != 0.0 && !immobile && world.objects.find(other).and_then(|o| o.physics).is_some() {
                    dist = Self::distance(world, me, other);
                }
                // "Now calculate the best outside range target."
                if out_of_range[index].is_none_or(|(_, c)| dist < c) {
                    out_of_range[index] = Some((other, dist));
                }
            }
        }
        let best = [(in_range[0], true), (in_range[1], true), (out_of_range[0], false), (out_of_range[1], false)].into_iter().find_map(|(c, r)| c.map(|(id, _)| (id, r)));
        match best {
            Some((id, r)) => {
                self.best_target_id = id;
                self.in_range = r;
                Ok(true)
            }
            None => {
                // "Utter failure -- nothing on the scope."
                self.best_target_id = INVALID_ID;
                self.in_range = false;
                Ok(false)
            }
        }
    }

    /// The body of `update()`.
    fn run(&mut self, world: &mut World, h: ModuleHandle) -> Result<UpdateSleepTime, Undefined> {
        let me = h.object;
        if world.objects.find(me).is_some_and(|o| o.is_effectively_dead()) {
            return Ok(UPDATE_SLEEP_FOREVER); // "No more laser fo you."
        }
        // "Optimized firing at acquired target"
        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            self.fire_when_ready(world, h)?; // "Only happens if something is tracked."
            return Ok(UPDATE_SLEEP_NONE);
        }
        self.next_scan_frames = if crate::logic::modules::plant(2366) { 0 } else { self.data.scan_frames as i32 };
        // "Periodic scanning (expensive)"
        if self.scan_closest_target(world, me)? {
            // "1 frame can make a big difference so fire ASAP!"
            self.fire_when_ready(world, h)?;
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

impl UpdateBehavior for PointDefenseLaserUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// The constructor's world half: "No starting sleep, but we want to sleep later." (`onObjectCreated`'s checks are debug crashes.)
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        match self.run(world, me) {
            Ok(t) => t,
            Err(u) => {
                world.cuts.module_errors.push((me, IniError::undefined(u)));
                UPDATE_SLEEP_NONE
            }
        }
    }
    /// The best target, whether it is in range, the frames to the next scan and to the next shot.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        xfer.xfer_object_id(&mut self.best_target_id)?;
        xfer.xfer_bool(&mut self.in_range)?;
        xfer.xfer_int(&mut self.next_scan_frames)?;
        if crate::logic::modules::plant(2367) {
            return Ok(());
        }
        xfer.xfer_int(&mut self.next_shot_available_in_frames)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::PointDefenseLaserUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::PointDefenseLaserUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for PointDefenseLaserUpdate {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = PointDefenseLaserUpdateModuleData::default();
        assert!(d.weapon_template.is_none() && d.scan_frames == 0 && d.scan_range == 0.0 && d.velocity_factor == 0.0);
        let m = PointDefenseLaserUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert_eq!(m.state_words(), vec![0, 0, 0, 0]);
    }
}
