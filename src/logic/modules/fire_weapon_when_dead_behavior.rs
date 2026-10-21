// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireWeaponWhenDeadBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/FireWeaponWhenDeadBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireWeaponWhenDeadBehavior`: fires a temporary weapon at the object's own position when the object dies, if its upgrade is active (it starts
//! inactive unless `StartsActive = Yes`), the die mux applies, the object is not under construction ("canceling construction sets it off, and killing a
//! one hitpoint one percent building will too") and neither the object nor its player has a conflicting upgrade. It is a behaviour module with
//! `DieModuleInterface` and an `UpgradeMux` (`MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DIE`), not a `DieModule`: its `xfer` is the behaviour base's and the
//! mux's, with no die module version.
//!
//! `DeathWeapon` is `INI::parseWeaponTemplate`: the name is kept and found in `world.statics.weapon_store` when the weapon fires (`None` and a name the
//! store does not have are `NULL` upstream, and nothing fires). The lookup compares the names without a case and does not assign a name key: upstream
//! assigns the key when it reads the data, which the module's data proc (no store, no generator) cannot do.
//!
//! `TheWeaponStore->createAndFireTempWeapon(weapon, object, position)` is [`CutCall::WeaponCreateAndFireTemp`].
//!
//! `p1w11-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::lend_clone;
use crate::logic::modules::upgrade_mux_user::{give_self_upgrade, upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class FireWeaponWhenDeadBehaviorModuleData : public BehaviorModuleData`. The behaviour base and the die mux are a [`DieModuleData`] here (the same
/// pair); the module's base is a plain behaviour module.
#[derive(Debug)]
pub struct FireWeaponWhenDeadBehaviorModuleData {
    /// The `BehaviorModuleData` base and `m_dieMuxData`.
    pub die: DieModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_initiallyActive`.
    pub initially_active: bool,
    /// `m_deathWeapon`, as the name that was written (`None` for a `NULL` before any lookup).
    pub death_weapon: Option<Vec<u8>>,
}

impl Default for FireWeaponWhenDeadBehaviorModuleData {
    /// `FireWeaponWhenDeadBehaviorModuleData()`: inactive, no weapon.
    fn default() -> Self {
        FireWeaponWhenDeadBehaviorModuleData { die: DieModuleData::new(), upgrade: Arc::new(UpgradeModuleData::new()), initially_active: false, death_weapon: None }
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `FireWeaponWhenDeadBehaviorModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut FireWeaponWhenDeadBehaviorModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `dataFieldParse`, then `DieMuxData::getFieldParse()` (the order the fields are added in).
static OWN_FIELD_PARSE: [FieldParse<FireWeaponWhenDeadBehaviorModuleData>; 8] = [
    FieldParse::new("StartsActive", |ini, d, _| { d.initially_active = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DeathWeapon", |ini, d, _| { d.death_weapon = Some(ini.get_next_token(None)?); Ok(()) }, UserData::None),
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::end(),
    FieldParse::end(),
];

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<FireWeaponWhenDeadBehaviorModuleData>; 6] = upgrade_module_data_field_parse!(FireWeaponWhenDeadBehaviorModuleData, upgrade);

impl FireWeaponWhenDeadBehaviorModuleData {
    /// `buildFieldParse(p)`: the behaviour base, this class's fields (and the die mux's), then the upgrade mux's. (Upstream adds the upgrade mux's table
    /// before the die mux's; the tables name different fields, so the order cannot be seen.)
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireWeaponWhenDeadBehaviorModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireWeaponWhenDeadBehaviorModuleData> {
        let mut data = FireWeaponWhenDeadBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireWeaponWhenDeadBehaviorModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FireWeaponWhenDeadBehavior : public BehaviorModule, public UpgradeMux, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct FireWeaponWhenDeadBehavior {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// `getFireWeaponWhenDeadBehaviorModuleData()`.
    data: Arc<FireWeaponWhenDeadBehaviorModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
}

impl PartialEq for FireWeaponWhenDeadBehavior {
    /// The same object, the same data object (upstream compares pointers) and the same executed flag.
    fn eq(&self, other: &Self) -> bool {
        self.behavior.object() == other.behavior.object() && Arc::ptr_eq(&self.data, &other.data) && self.mux == other.mux
    }
}

impl FireWeaponWhenDeadBehavior {
    /// `FireWeaponWhenDeadBehavior::getInterfaceMask()`: `BehaviorModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        DieModule::get_interface_mask() | MODULEINTERFACE_UPGRADE
    }

    /// The constructor, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<FireWeaponWhenDeadBehaviorModuleData>>) -> IniResult<Self> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let behavior = BehaviorModule::new(object, Some(ModuleDataKind::Die(Arc::new(data.die.clone()))))?;
        Ok(FireWeaponWhenDeadBehavior { behavior, data, mux: UpgradeMux::new() })
    }

    /// The data, as `getFireWeaponWhenDeadBehaviorModuleData()`.
    pub fn module_data(&self) -> &Arc<FireWeaponWhenDeadBehaviorModuleData> {
        &self.data
    }

    /// The world half of the constructor: `if (m_initiallyActive) giveSelfUpgrade()`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if self.data.initially_active {
            if let Err(e) = give_self_upgrade(self, world, me) {
                world.cuts.module_errors.push((me, e));
            }
        }
    }

    /// The module's answer to one of `Object`'s calls: `onDie`, the upgrade calls, `onDelete` and `onCapture` (empty here).
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        match call {
            ModuleCall::OnDie(info) => {
                lend_clone(world, me, self, UpdateBody::FireWeaponWhenDeadBehavior, Self::mux_from_body, |w, m| m.on_die(w, me, &info));
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }

    /// `onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.mux.is_already_upgraded() && !crate::logic::modules::plant(896) {
            return;
        }
        // right type?
        let Some(obj) = world.objects.find(me.object) else { return };
        if !self.data.die.die_mux_data.is_die_applicable(info.input.death_type, obj.get_veterancy_level() as i32, &obj.status) && !crate::logic::modules::plant(897) {
            return;
        }
        // This will never apply until built.
        if obj.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) && !crate::logic::modules::plant(893) {
            return;
        }
        let statics = world.statics.clone();
        let masks = self.data.upgrade.mux_data_mut().get_upgrade_activation_masks(&statics.upgrade_center, &mut world.names);
        let conflicting = match masks {
            Ok((_activation, conflicting)) => conflicting,
            Err(e) => {
                world.cuts.module_errors.push((me, e));
                return;
            }
        };
        let Some(obj) = world.objects.find(me.object) else { return };
        if obj.object_upgrades_completed.test_for_any(&conflicting) && !crate::logic::modules::plant(894) {
            return;
        }
        if let Some(player) = world.get_controlling_player(me.object) {
            if world.cuts.player_completed_upgrade_mask(player).test_for_any(&conflicting) && !crate::logic::modules::plant(895) {
                return;
            }
        }
        // fire the default weapon
        let Some(name) = self.data.death_weapon.as_deref() else { return };
        let weapon = statics.weapon_store.templates().iter().find(|t| t.name.eq_ignore_ascii_case(name)).map(|t| t.name.clone());
        let (Some(weapon), Some(obj)) = (weapon, world.objects.find(me.object)) else { return };
        let position = if crate::logic::modules::plant(891) { crate::common::base_type::Coord3D::default() } else { obj.get_position() };
        let source = if crate::logic::modules::plant(892) { INVALID_ID } else { me.object };
        world.cut(CutCall::WeaponCreateAndFireTemp { weapon, source, position });
    }
}

impl UpgradeMuxUser for FireWeaponWhenDeadBehavior {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`: "nothing!".
    fn upgrade_implementation(&mut self, _world: &mut World, _me: ModuleHandle) -> Result<(), IniError> {
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::FireWeaponWhenDeadBehavior(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireWeaponWhenDeadBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FireWeaponWhenDeadBehavior {
    /// `crc`: `BehaviorModule::crc`, then `upgradeMuxCRC`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)?;
        self.mux.upgrade_mux_crc(xfer)
    }
    /// `xfer` (version 1): the version, `BehaviorModule::xfer`, `upgradeMuxXfer` (its own version, the executed flag).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        if crate::logic::modules::plant(898) {
            return Ok(());
        }
        self.mux.upgrade_mux_xfer(xfer)
    }
    /// `loadPostProcess`: `BehaviorModule::loadPostProcess`, then `upgradeMuxLoadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
        self.mux.upgrade_mux_load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `FireWeaponWhenDeadBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireWeaponWhenDeadBehavior(Arc::new(FireWeaponWhenDeadBehaviorModuleData::friend_new_module_data(ini)?)))
}
