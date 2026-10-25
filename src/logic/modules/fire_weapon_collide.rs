// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireWeaponCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/FireWeaponCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireWeaponCollide`: an object that shoots whatever touches it (the toxin pools, the burning ground). It owns a weapon of
//! `CollideWeapon`; each collision with an object (not the ground), when the object has every `RequiredStatus` bit and none of the
//! `ForbiddenStatus` ones, reloads the weapon at once and fires it at the other object.
//!
//! # Upstream facts kept
//!
//! - `m_everFired` is saved but never set: `FireOnce` never stops the weapon.
//! - The weapon is made in the constructor (`allocateNewWeapon`) and saved with `xferSnapshot`; its `loadPostProcess` is not run (the module's
//!   is the collide base's).
//! - `CollideWeapon` is `INI::parseWeaponTemplate`: a name the store does not have, and `None`, is `NULL`, and the weapon's constructor
//!   reads it (`Undefined::NullWeaponTemplate`).
//!
//! The weapon's two calls go through the world ([`World::module_weapon_load_ammo_now`], [`World::module_weapon_fire_at_object`]).
//!
//! `p1w13-sonnet`, batch R62, 2026-10-25.

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::ObjectStatusMaskType;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::{CollideModule, CollideModuleData};
use crate::logic::modules::lend_clone;
use crate::logic::weapon_fire::{Weapon, WeaponSnapshot, PRIMARY_WEAPON};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;

/// `class FireWeaponCollideModuleData : public CollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FireWeaponCollideModuleData {
    pub collide: CollideModuleData,
    /// `m_collideWeaponTemplate`, by name (found in the store when the module is made; `None` and an unknown name are `NULL`).
    pub collide_weapon: Option<Vec<u8>>,
    /// `m_fireOnce`.
    pub fire_once: bool,
    /// `m_requiredStatus`, `m_forbiddenStatus`.
    pub required_status: ObjectStatusMaskType,
    pub forbidden_status: ObjectStatusMaskType,
}

static FIELD_PARSE: [FieldParse<FireWeaponCollideModuleData>; 5] = [
    FieldParse::new("CollideWeapon", |i, d, _| { d.collide_weapon = Some(i.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("FireOnce", |i, d, _| { d.fire_once = i.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("RequiredStatus", |i, d, _| d.required_status.parse_from_ini(i), UserData::None),
    FieldParse::new("ForbiddenStatus", |i, d, _| d.forbidden_status.parse_from_ini(i), UserData::None),
    FieldParse::end(),
];

impl FireWeaponCollideModuleData {
    /// `buildFieldParse(p)`: the collide base's (nothing), then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireWeaponCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireWeaponCollideModuleData> {
        let mut data = FireWeaponCollideModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireWeaponCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `FireWeaponCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireWeaponCollide(Arc::new(FireWeaponCollideModuleData::friend_new_module_data(ini)?)))
}

/// `class FireWeaponCollide : public CollideModule`.
#[derive(Clone, Debug)]
pub struct FireWeaponCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    data: Arc<FireWeaponCollideModuleData>,
    /// `m_collideWeapon` (made by the world half of the constructor).
    collide_weapon: Option<Weapon>,
    /// The weapon template's name and every template's name and store position, for the weapon's `xfer` (taken from the store when the
    /// weapon is made).
    weapon_name: Vec<u8>,
    template_names: Arc<Vec<(Vec<u8>, usize)>>,
    /// `m_everFired`.
    ever_fired: bool,
}

impl PartialEq for FireWeaponCollide {
    /// The same object, the same data object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.state_words() == other.state_words()
    }
}

impl FireWeaponCollide {
    /// The constructor's first half (it touches no world): the base, no weapon yet (`INI_INVALID_DATA` for null data).
    pub fn new(object: ObjectId, data: Option<Arc<FireWeaponCollideModuleData>>) -> IniResult<FireWeaponCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.collide.clone())))?;
        Ok(FireWeaponCollide { collide, data, collide_weapon: None, weapon_name: Vec::new(), template_names: Arc::new(Vec::new()), ever_fired: false })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<FireWeaponCollideModuleData> {
        &self.data
    }

    /// The weapon (`None` before the constructor's world half).
    pub fn collide_weapon(&self) -> Option<&Weapon> {
        self.collide_weapon.as_ref()
    }

    /// `m_everFired`.
    pub fn state_words(&self) -> Vec<u32> {
        vec![u32::from(self.ever_fired)]
    }

    /// The constructor's world half: `m_collideWeapon = TheWeaponStore->allocateNewWeapon(template, PRIMARY_WEAPON)`.
    pub fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let store = &world.statics.weapon_store;
        let found = self.data.collide_weapon.as_deref().and_then(|n| store.templates().iter().position(|t| t.name.eq_ignore_ascii_case(n)));
        let Some(template) = found else {
            // the weapon's constructor reads the template through `NULL`
            world.cuts.module_errors.push((me, IniError::undefined(Undefined::NullWeaponTemplate)));
            return;
        };
        self.weapon_name = store.templates()[template].name.to_vec();
        self.template_names = Arc::new(store.templates().iter().enumerate().map(|(i, t)| (t.name.to_vec(), i)).collect());
        self.collide_weapon = Some(Weapon::new_in_store(store, world.frame(), template, PRIMARY_WEAPON));
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnCollide { other, .. } => {
                lend_clone(world, me, self, UpdateBody::FireWeaponCollide, |b| match b {
                    UpdateBody::FireWeaponCollide(m) => Some(m),
                    _ => None,
                }, |w, m| m.on_collide(w, me, other));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// `onCollide(other, loc, normal)`: "Don't shoot the ground".
    fn on_collide(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) {
        if other == INVALID_ID || world.objects.find(other).is_none() {
            return;
        }
        if self.should_fire_weapon(world, me.object) {
            let Some(mut weapon) = self.collide_weapon.take() else { return };
            world.module_weapon_load_ammo_now(&mut weapon, me.object);
            if !crate::logic::modules::plant(2321) {
                if let Err(e) = world.module_weapon_fire_at_object(&mut weapon, me.object, other) {
                    world.cuts.module_errors.push((me, e));
                }
            }
            self.collide_weapon = Some(weapon);
        }
    }

    /// `shouldFireWeapon()`.
    fn should_fire_weapon(&self, world: &World, us: ObjectId) -> bool {
        let d = &self.data;
        let Some(o) = world.objects.find(us) else { return false };
        let status = &o.status;
        // "We need all required status or else we fail"
        if !status.test_for_all(&d.required_status) && !crate::logic::modules::plant(2322) {
            return false;
        }
        // "If we have any forbidden statii, then fail"
        if status.test_for_any(&d.forbidden_status) && !crate::logic::modules::plant(2323) {
            return false;
        }
        let ever_fired = if crate::logic::modules::plant(2324) { true } else { self.ever_fired };
        if ever_fired && d.fire_once {
            return false; // "can only fire once ever"
        }
        true
    }

    fn snapshot(&mut self) -> Option<WeaponSnapshot> {
        let weapon = self.collide_weapon.take()?;
        Some(WeaponSnapshot { weapon, current_name: self.weapon_name.clone(), templates: self.template_names.as_ref().clone(), error: None })
    }
}

impl Snapshot for FireWeaponCollide {
    /// `crc`: `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1): the version, `CollideModule::xfer`, whether the weapon is present and the weapon, `m_everFired`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.collide.xfer(xfer)?;
        let mut present = self.collide_weapon.is_some();
        xfer.xfer_bool(&mut present)?;
        if present && !crate::logic::modules::plant(2325) {
            // (a weapon a load says is present when there is none is read through `NULL`; the module never lacks one once made)
            if let Some(mut snap) = self.snapshot() {
                let r = xfer.xfer_snapshot(&mut snap);
                self.collide_weapon = Some(snap.weapon);
                r?;
            }
        }
        xfer.xfer_bool(&mut self.ever_fired)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess` (not the weapon's).
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_constructors() {
        let d = FireWeaponCollideModuleData::default();
        assert!(d.collide_weapon.is_none() && !d.fire_once);
        let m = FireWeaponCollide::new(1, Some(Arc::new(d))).unwrap();
        assert!(m.collide_weapon().is_none());
        assert_eq!(m.state_words(), vec![0]);
    }
}
