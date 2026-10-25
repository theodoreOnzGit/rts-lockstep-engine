// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireWeaponUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/FireWeaponUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireWeaponUpdate`: an object that keeps firing a weapon of its own at its own position (the radiation of a toxin field, the anthrax
//! cloud). The weapon is made and loaded in the constructor; every frame after `InitialDelay`, when the weapon is ready, the object is not
//! under construction and it has fired no real weapon in the last `ExclusiveWeaponDelay` frames, the weapon is force-fired at the object's
//! position.
//!
//! # Upstream facts kept
//!
//! - `Weapon` is `INI::parseWeaponTemplate`: `None` and a name the store does not have are `NULL`, and then the module has no weapon: it
//!   never fires, and its `xfer` (save or load) throws `XFER_INVALID_PARAMETERS` (`xferSnapshot(NULL)`; the CRC skips a `NULL` snapshot,
//!   and the module's `crc` is the base's anyway).
//! - `update` never sleeps.
//!
//! The weapon's two calls go through the world ([`World::module_weapon_load_ammo_now`],
//! [`World::module_weapon_force_fire_at_position`]).
//!
//! `p1w13-sonnet`, batch R64, 2026-10-25.

use std::sync::Arc;

use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferResult, XferStatus, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_fire::{Weapon, WeaponSnapshot, PRIMARY_WEAPON};
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class FireWeaponUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FireWeaponUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_weaponTemplate`, by name (found in the store when the module is made; `None` and an unknown name are `NULL`).
    pub weapon: Option<Vec<u8>>,
    /// `m_initialDelayFrames`.
    pub initial_delay_frames: u32,
    /// `m_exclusiveWeaponDelay`.
    pub exclusive_weapon_delay: u32,
}

static FIELD_PARSE: [FieldParse<FireWeaponUpdateModuleData>; 4] = [
    FieldParse::new("Weapon", |i, d, _| { d.weapon = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("InitialDelay", |i, d, _| { d.initial_delay_frames = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ExclusiveWeaponDelay", |i, d, _| { d.exclusive_weapon_delay = i.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FireWeaponUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireWeaponUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireWeaponUpdateModuleData> {
        let mut data = FireWeaponUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireWeaponUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `FireWeaponUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireWeaponUpdate(Arc::new(FireWeaponUpdateModuleData::friend_new_module_data(ini)?)))
}

/// `class FireWeaponUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct FireWeaponUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<FireWeaponUpdateModuleData>,
    /// `m_weapon` (made by the world half of the constructor).
    weapon: Option<Weapon>,
    /// The weapon template's name and every template's name and store position, for the weapon's `xfer` (taken from the store when the
    /// weapon is made).
    weapon_name: Vec<u8>,
    template_names: Arc<Vec<(Vec<u8>, usize)>>,
    /// `m_initialDelayFrame`.
    initial_delay_frame: u32,
}

impl PartialEq for FireWeaponUpdate {
    /// The same object, the same data object and the same state (the weapon's included).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.weapon == other.weapon
            && self.initial_delay_frame == other.initial_delay_frame
    }
}

impl FireWeaponUpdate {
    /// The constructor's first half (it touches no world): the base, no weapon yet (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, data: Option<Arc<FireWeaponUpdateModuleData>>) -> IniResult<FireWeaponUpdate> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(FireWeaponUpdate { update, data, weapon: None, weapon_name: Vec::new(), template_names: Arc::new(Vec::new()), initial_delay_frame: 0 })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<FireWeaponUpdateModuleData> {
        &self.data
    }

    /// The weapon (`None` before the constructor's world half, or for a `NULL` template).
    pub fn weapon(&self) -> Option<&Weapon> {
        self.weapon.as_ref()
    }

    /// The weapon, for a test that sets its state (`setStatus`, `setPossibleNextShotFrame`).
    pub fn weapon_mut(&mut self) -> Option<&mut Weapon> {
        self.weapon.as_mut()
    }

    /// `m_initialDelayFrame`.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.initial_delay_frame]
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

    /// `isOkayToFire()`.
    fn is_okay_to_fire(&self, world: &World, us: ObjectId) -> bool {
        let Some(weapon) = self.weapon.as_ref() else { return false };
        // "Weapon is reloading"
        if weapon.get_status(world.frame()) != WeaponStatus::ReadyToFire && !crate::logic::modules::plant(2341) {
            return false;
        }
        if world.objects.find(us).is_some_and(|o| o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)) && !crate::logic::modules::plant(2342) {
            return false; // "no hitting with a 0% building, cheater"
        }
        // "Firing a real weapon surpresses this module"
        let d = &self.data;
        let last = world.contain_last_shot_fired_frame(us);
        let until = if crate::logic::modules::plant(2343) { last } else { last.wrapping_add(d.exclusive_weapon_delay) };
        if d.exclusive_weapon_delay > 0 && world.frame() < until {
            return false;
        }
        true
    }

    fn snapshot(&mut self) -> Option<WeaponSnapshot> {
        let weapon = self.weapon.take()?;
        Some(WeaponSnapshot { weapon, current_name: self.weapon_name.clone(), templates: self.template_names.as_ref().clone(), error: None })
    }
}

impl UpdateBehavior for FireWeaponUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// The constructor's world half: the weapon (`allocateNewWeapon(template, PRIMARY_WEAPON)`, then `loadAmmoNow`), then
    /// `m_initialDelayFrame`.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let store = &world.statics.weapon_store;
        let found = self.data.weapon.as_deref().and_then(|n| store.templates().iter().position(|t| t.name.eq_ignore_ascii_case(n)));
        if let Some(template) = found {
            self.weapon_name = store.templates()[template].name.to_vec();
            self.template_names = Arc::new(store.templates().iter().enumerate().map(|(i, t)| (t.name.to_vec(), i)).collect());
            let mut weapon = Weapon::new_in_store(store, world.frame(), template, PRIMARY_WEAPON);
            if !crate::logic::modules::plant(2344) {
                world.module_weapon_load_ammo_now(&mut weapon, me.object);
            }
            self.weapon = Some(weapon);
        }
        self.initial_delay_frame = world.frame().wrapping_add(self.data.initial_delay_frames);
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if world.frame() < self.initial_delay_frame && !crate::logic::modules::plant(2345) {
            return UPDATE_SLEEP_NONE;
        }
        // "If my weapon is ready, shoot it."
        if self.is_okay_to_fire(world, me.object) {
            let pos = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
            if let Some(mut weapon) = self.weapon.take() {
                if let Err(e) = world.module_weapon_force_fire_at_position(&mut weapon, me.object, pos) {
                    world.cuts.module_errors.push((me, e));
                }
                self.weapon = Some(weapon);
            }
        }
        UPDATE_SLEEP_NONE
    }
    /// The weapon (`xferSnapshot`), then (version 2) `m_initialDelayFrame`.
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        match self.snapshot() {
            Some(mut snap) => {
                let r = xfer.xfer_snapshot(&mut snap);
                self.weapon = Some(snap.weapon);
                r?;
            }
            // `xferSnapshot(NULL)`: the CRC skips it, a save or a load throws
            None if xfer.get_xfer_mode() == XferMode::Crc => {}
            None => return Err(XferStatus::InvalidParameters),
        }
        if version >= 2 && !crate::logic::modules::plant(2346) {
            xfer.xfer_unsigned_int(&mut self.initial_delay_frame)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FireWeaponUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireWeaponUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FireWeaponUpdate {
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
    /// `loadPostProcess`: the base's (not the weapon's).
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = FireWeaponUpdateModuleData::default();
        assert!(d.weapon.is_none() && d.initial_delay_frames == 0 && d.exclusive_weapon_delay == 0);
        let m = FireWeaponUpdate::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert!(m.weapon().is_none());
        assert_eq!(m.state_words(), vec![0]);
    }
}
