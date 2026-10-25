// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireWeaponWhenDamagedBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDamagedBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireWeaponWhenDamagedBehavior`: an object that fires weapons of its own when it is hit, and maybe all the time, by its body's damage
//! state. It owns up to eight weapons: a reaction weapon and a continuous one for each of pristine, damaged, really damaged and rubble,
//! each made and reloaded by the constructor. Once its upgrade is active (`StartsActive`, or an upgrade), damage of one of `DamageTypes`
//! and at least `DamageAmount` (dealt, after armour) force-fires the reaction weapon of the current state at the object's position, if it
//! is ready; and the update, awake only when there is a continuous weapon, does the same with the continuous one every frame. It is an
//! update module, an `UpgradeMux` and a `DamageModuleInterface`.
//!
//! # Upstream facts kept
//!
//! - A weapon name the store does not have, and `None`, is `NULL`: no weapon for that slot.
//! - `onDamage` and `update` read the body module through the object (`Undefined::NullBodyModule` for an object without one).
//!
//! The weapons' calls go through the world ([`World::module_weapon_reload_ammo`], [`World::module_weapon_force_fire_at_position`]).
//!
//! `p1w13-sonnet`, batch R65, 2026-10-25.

use std::sync::Arc;

use crate::common::global_data::{BODY_DAMAGED, BODY_REALLYDAMAGED, BODY_RUBBLE};
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DAMAGE, MODULEINTERFACE_UPGRADE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::{DamageInfo, DamageTypeFlags, DAMAGE_TYPE_FLAGS_ALL};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{give_self_upgrade, upgrade_mux_call, UpgradeMuxUser};
use crate::logic::weapon_fire::{Weapon, WeaponSnapshot, PRIMARY_WEAPON};
use crate::logic::weapon_types::WeaponStatus;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// The eight weapon slots, in the order of the data, the constructor and `xfer`: the four reaction weapons (pristine, damaged, really
/// damaged, rubble), then the four continuous ones.
pub const WEAPON_FIELDS: [&str; 8] = [
    "ReactionWeaponPristine",
    "ReactionWeaponDamaged",
    "ReactionWeaponReallyDamaged",
    "ReactionWeaponRubble",
    "ContinuousWeaponPristine",
    "ContinuousWeaponDamaged",
    "ContinuousWeaponReallyDamaged",
    "ContinuousWeaponRubble",
];
/// The first continuous slot of [`WEAPON_FIELDS`].
const CONTINUOUS: usize = 4;

/// `class FireWeaponWhenDamagedBehaviorModuleData : public UpdateModuleData`.
#[derive(Debug)]
pub struct FireWeaponWhenDamagedBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_initiallyActive`.
    pub initially_active: bool,
    /// `m_damageTypes` (all by default).
    pub damage_types: DamageTypeFlags,
    /// `m_damageAmount`.
    pub damage_amount: f32,
    /// The eight weapon templates (see [`WEAPON_FIELDS`]), by name (found in the store when the module is made; `None` and an unknown name
    /// are `NULL`).
    pub weapons: [Option<Vec<u8>>; 8],
}

impl Default for FireWeaponWhenDamagedBehaviorModuleData {
    /// `FireWeaponWhenDamagedBehaviorModuleData()`.
    fn default() -> Self {
        FireWeaponWhenDamagedBehaviorModuleData {
            update: UpdateModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            initially_active: false,
            damage_types: DAMAGE_TYPE_FLAGS_ALL,
            damage_amount: 0.0,
            weapons: Default::default(),
        }
    }
}

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<FireWeaponWhenDamagedBehaviorModuleData>; 6] =
    upgrade_module_data_field_parse!(FireWeaponWhenDamagedBehaviorModuleData, upgrade);

macro_rules! weapon_field {
    ($k:expr) => {
        FieldParse::new(WEAPON_FIELDS[$k], |ini, d, _| { d.weapons[$k] = Some(ini.get_next_token(None)?); Ok(()) }, UserData::None)
    };
}

/// `buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<FireWeaponWhenDamagedBehaviorModuleData>; 12] = [
    FieldParse::new("StartsActive", |ini, d, _| { d.initially_active = ini.parse_bool()?; Ok(()) }, UserData::None),
    weapon_field!(0),
    weapon_field!(1),
    weapon_field!(2),
    weapon_field!(3),
    weapon_field!(4),
    weapon_field!(5),
    weapon_field!(6),
    weapon_field!(7),
    FieldParse::new("DamageTypes", |ini, d, _| { d.damage_types = ini.parse_damage_type_flags()?; Ok(()) }, UserData::None),
    FieldParse::new("DamageAmount", |ini, d, _| { d.damage_amount = ini.parse_real()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

impl FireWeaponWhenDamagedBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, this class's own fields, then the mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireWeaponWhenDamagedBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireWeaponWhenDamagedBehaviorModuleData> {
        let mut data = FireWeaponWhenDamagedBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireWeaponWhenDamagedBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// The factory's `NewModuleDataProc` for `FireWeaponWhenDamagedBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireWeaponWhenDamagedBehavior(Arc::new(FireWeaponWhenDamagedBehaviorModuleData::friend_new_module_data(ini)?)))
}

/// `class FireWeaponWhenDamagedBehavior : public UpdateModule, public UpgradeMux, public DamageModuleInterface`.
#[derive(Clone, Debug)]
pub struct FireWeaponWhenDamagedBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    data: Arc<FireWeaponWhenDamagedBehaviorModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
    /// The eight weapons (see [`WEAPON_FIELDS`]; made by the world half of the constructor), each with its template's name.
    weapons: [Option<(Weapon, Vec<u8>)>; 8],
    /// Every template's name and store position, for the weapons' `xfer` (taken from the store when the weapons are made).
    template_names: Arc<Vec<(Vec<u8>, usize)>>,
}

impl PartialEq for FireWeaponWhenDamagedBehavior {
    /// The same object, the same data object and the same state (the weapons' included).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.weapons == other.weapons
    }
}

impl FireWeaponWhenDamagedBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DAMAGE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DAMAGE
    }

    /// The constructor's first half (it touches no world): the base, no weapons yet (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, data: Option<Arc<FireWeaponWhenDamagedBehaviorModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let update = UpdateModule::new(object, Some(Arc::new(data.update.clone())), SleepyUpdatePhase::Normal)?;
        Ok(FireWeaponWhenDamagedBehavior { update, data, mux: UpgradeMux::new(), weapons: Default::default(), template_names: Arc::new(Vec::new()) })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The data.
    pub fn module_data(&self) -> &Arc<FireWeaponWhenDamagedBehaviorModuleData> {
        &self.data
    }

    /// Weapon `k` of [`WEAPON_FIELDS`] (`None` before the constructor's world half, or for a `NULL` template).
    pub fn weapon(&self, k: usize) -> Option<&Weapon> {
        self.weapons[k].as_ref().map(|(w, _)| w)
    }

    /// Weapon `k`, for a test that sets its state (`setStatus`, `setPossibleNextShotFrame`).
    pub fn weapon_mut(&mut self, k: usize) -> Option<&mut Weapon> {
        self.weapons[k].as_mut().map(|(w, _)| w)
    }

    /// Whether the upgrade ran (`isUpgradeActive()`), and which weapons were made (a bit each).
    pub fn state_words(&self) -> Vec<u32> {
        let made = (0..8).filter(|&k| self.weapons[k].is_some()).fold(0u32, |m, k| m | 1 << k);
        vec![u32::from(self.mux.is_already_upgraded()), made]
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

    /// `crc` with the scheduling half the slot holds: `UpdateModule::crc`, then `upgradeMuxCRC`.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        update_behavior::crc_with(update, xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        update_behavior::load_post_process_with(self, update);
    }

    /// The slot of the body's damage state among four (pristine, damaged, really damaged, rubble; any other state is "not damaged yet").
    fn state_slot(world: &mut World, us: ObjectId) -> Result<usize, Undefined> {
        let body = world.get_body_module(us).ok_or(Undefined::NullBodyModule)?;
        let bdt = world.module_call(us, body, ModuleCall::GetDamageState).as_int(0);
        Ok(match bdt {
            b if b == BODY_RUBBLE => 3,
            b if b == BODY_REALLYDAMAGED => 2,
            b if b == BODY_DAMAGED && !crate::logic::modules::plant(2351) => 1,
            _ => 0,
        })
    }

    /// Fires weapon `k` at the object's position if it is ready (`getStatus() == READY_TO_FIRE`).
    fn fire_if_ready(&mut self, world: &mut World, me: ModuleHandle, k: usize) {
        let Some((mut weapon, name)) = self.weapons[k].take() else { return };
        if weapon.get_status(world.frame()) == WeaponStatus::ReadyToFire || crate::logic::modules::plant(2352) {
            let pos = world.objects.find(me.object).map(|o| o.get_position()).unwrap_or_default();
            if let Err(e) = world.module_weapon_force_fire_at_position(&mut weapon, me.object, pos) {
                world.cuts.module_errors.push((me, e));
            }
        }
        self.weapons[k] = Some((weapon, name));
    }

    /// `onDamage(damageInfo)`: "Damage has been dealt, this is an opportunity to reach to that damage".
    fn on_damage(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) -> Result<(), Undefined> {
        if !self.mux.is_already_upgraded() {
            return Ok(());
        }
        let d = self.data.clone();
        // "right type?"
        if !d.damage_types.test(info.input.damage_type) && !crate::logic::modules::plant(2353) {
            return Ok(());
        }
        // "right amount? (use actual [post-armor] damage dealt)"
        let dealt = if crate::logic::modules::plant(2354) { info.input.amount } else { info.out.actual_damage_dealt };
        if dealt < d.damage_amount {
            return Ok(());
        }
        let slot = Self::state_slot(world, me.object)?;
        self.fire_if_ready(world, me, slot);
        Ok(())
    }
}

impl UpgradeMuxUser for FireWeaponWhenDamagedBehavior {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: `setWakeFrame(getObject(), UPDATE_SLEEP_NONE)`.
    fn upgrade_implementation(&mut self, world: &mut World, _me: ModuleHandle) -> Result<(), IniError> {
        if !crate::logic::modules::plant(2355) {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        }
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::FireWeaponWhenDamagedBehavior(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireWeaponWhenDamagedBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl UpdateBehavior for FireWeaponWhenDamagedBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    /// The constructor's world half: each weapon made (`allocateNewWeapon(template, PRIMARY_WEAPON)`) and reloaded (`reloadAmmo`), then the
    /// upgrade given if `StartsActive`, then the wake frame: every frame with the upgrade and a continuous weapon, else never.
    fn construct(&mut self, world: &mut World, me: ModuleHandle) {
        let d = self.data.clone();
        let statics = world.statics.clone();
        let store = &statics.weapon_store;
        let mut found = [None; 8];
        for (k, f) in found.iter_mut().enumerate() {
            *f = d.weapons[k].as_deref().and_then(|n| store.templates().iter().position(|t| t.name.eq_ignore_ascii_case(n)));
        }
        if found.iter().any(Option::is_some) {
            self.template_names = Arc::new(store.templates().iter().enumerate().map(|(i, t)| (t.name.to_vec(), i)).collect());
        }
        for (k, f) in found.iter().enumerate() {
            let Some(template) = *f else { continue };
            let mut weapon = Weapon::new_in_store(store, world.frame(), template, PRIMARY_WEAPON);
            if !crate::logic::modules::plant(2356) {
                world.module_weapon_reload_ammo(&mut weapon, me.object);
            }
            self.weapons[k] = Some((weapon, store.templates()[template].name.to_vec()));
        }
        if d.initially_active {
            if let Err(e) = give_self_upgrade(self, world, me) {
                world.cuts.module_errors.push((me, e));
                return;
            }
        }
        let continuous = found[CONTINUOUS..].iter().any(Option::is_some);
        if self.mux.is_already_upgraded() && continuous {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        } else {
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
        }
    }
    /// "if object fires weapon constantly, figure out which one and do it".
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        if !self.mux.is_already_upgraded() {
            // ("hmm, this should not be possible")
            return UPDATE_SLEEP_FOREVER;
        }
        match Self::state_slot(world, me.object) {
            Ok(slot) => self.fire_if_ready(world, me, CONTINUOUS + slot),
            Err(u) => world.cuts.module_errors.push((me, IniError::undefined(u))),
        }
        UPDATE_SLEEP_NONE
    }
    /// The mux, then each weapon: whether it is present, and if so the weapon (`xferSnapshot`).
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        self.mux.upgrade_mux_xfer(xfer)?;
        for k in 0..8 {
            let mut present = self.weapons[k].is_some();
            xfer.xfer_bool(&mut present)?;
            if present && !(k == 7 && crate::logic::modules::plant(2357)) {
                // (a weapon a load says is present when there is none is read through `NULL`; a slot's weapon is made or not once, by the
                // constructor, and every save of it says so)
                if let Some((weapon, name)) = self.weapons[k].take() {
                    let mut snap = WeaponSnapshot { weapon, current_name: name.clone(), templates: self.template_names.as_ref().clone(), error: None };
                    let r = xfer.xfer_snapshot(&mut snap);
                    self.weapons[k] = Some((snap.weapon, name));
                    r?;
                }
            }
        }
        Ok(())
    }
    fn load_post_process_fields(&mut self) {
        self.mux.upgrade_mux_load_post_process();
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FireWeaponWhenDamagedBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireWeaponWhenDamagedBehavior(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        match call {
            ModuleCall::OnDamage(info) => {
                if let Err(u) = self.on_damage(world, me, &info) {
                    world.cuts.module_errors.push((me, IniError::undefined(u)));
                }
                ModuleReply::Done
            }
            // `onHealing` and `onBodyDamageStateChange` are empty
            ModuleCall::OnHealing(_) | ModuleCall::OnBodyDamageStateChange { .. } => ModuleReply::Done,
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for FireWeaponWhenDamagedBehavior {
    /// `crc`: `UpdateModule::crc`, then `upgradeMuxCRC`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }
    /// `xfer`: the version, the base, the mux, then the weapons.
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
    fn the_defaults_are_the_constructors() {
        let d = FireWeaponWhenDamagedBehaviorModuleData::default();
        assert!(!d.initially_active && d.damage_amount == 0.0 && d.damage_types == DAMAGE_TYPE_FLAGS_ALL);
        assert!(d.weapons.iter().all(Option::is_none));
        let m = FireWeaponWhenDamagedBehavior::unconstructed(1, Some(Arc::new(d))).unwrap();
        assert!(m.weapon(0).is_none());
        assert_eq!(m.state_words(), vec![0, 0]);
    }
}
