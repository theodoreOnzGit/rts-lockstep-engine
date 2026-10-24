// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FXListDie.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Die/FXListDie.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FXListDie`: plays an effect list when the object dies, if its upgrade is active (it starts active unless `StartsActive = No`; "should default to
//! FALSE but only ONE case sets it false out of 847!") and neither the object nor its player has a conflicting upgrade. It is a die module and an
//! `UpgradeMux` (`MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DIE`), so an upgrade can switch it on.
//!
//! The effect list is a name (`DeathFX`, `None` is `NULL`). `OrientToObject` (the default) plays `FXList::doFXObj(fx, object, damageDealer)` with the
//! damage dealer found by `TheGameLogic->findObjectByID(damageInfo->in.m_sourceID)` (or none), else `FXList::doFXPos(fx, position)`
//! ([`ObjectClientEvent::FxListDoFxObjSecondary`], [`ObjectClientEvent::FxListDoFxPos`]).
//!
//! The constructor's `giveSelfUpgrade()` (when `StartsActive`) is the mux's own: the upgrade's effect list, its removals, nothing for
//! `upgradeImplementation` ("nothing!"), the upgrade marked executed. An upgrade name that is not one (`INI_INVALID_DATA` thrown out of the
//! constructor upstream) is recorded in `world.cuts.module_errors`.
//!
//! `p1w11-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::damage_fx::FxListName;
use crate::common::game_type::INVALID_ID;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::damage::DamageInfo;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::die_module::{DieModule, DieModuleData, DIE_MODULE_DATA_FIELD_PARSE};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::crate_collide::CrateCollideModuleData;
use crate::logic::modules::die_body::{die_module_call, DieBehavior};
use crate::logic::modules::upgrade_mux_user::{give_self_upgrade, upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class FXListDieModuleData : public DieModuleData`.
#[derive(Debug)]
pub struct FXListDieModuleData {
    /// The `DieModuleData` base (the die mux and the behaviour base).
    pub die: DieModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_defaultDeathFX`, as a name.
    pub default_death_fx: FxListName,
    /// `m_orientToObject`.
    pub orient_to_object: bool,
    /// `m_initiallyActive`.
    pub initially_active: bool,
}

impl Default for FXListDieModuleData {
    /// `FXListDieModuleData()`: no list, oriented, initially active.
    fn default() -> Self {
        FXListDieModuleData { die: DieModuleData::new(), upgrade: Arc::new(UpgradeModuleData::new()), default_death_fx: None, orient_to_object: true, initially_active: true }
    }
}

/// Lifts entry `k` of the layer-5 die field table onto `FXListDieModuleData::die`.
fn lift_die_field(k: usize, i: &mut Ini<'_>, d: &mut FXListDieModuleData, u: &UserData) -> IniResult<()> {
    match DIE_MODULE_DATA_FIELD_PARSE[k].parse {
        Some(f) => f(i, &mut d.die, u),
        None => Ok(()),
    }
}

/// `DieModuleData::buildFieldParse`'s four fields, then this class's own.
static OWN_FIELD_PARSE: [FieldParse<FXListDieModuleData>; 8] = [
    FieldParse::new("DeathTypes", |i, d, u| lift_die_field(0, i, d, u), UserData::None),
    FieldParse::new("VeterancyLevels", |i, d, u| lift_die_field(1, i, d, u), UserData::None),
    FieldParse::new("ExemptStatus", |i, d, u| lift_die_field(2, i, d, u), UserData::None),
    FieldParse::new("RequiredStatus", |i, d, u| lift_die_field(3, i, d, u), UserData::None),
    FieldParse::new("StartsActive", |ini, d, _| { d.initially_active = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("DeathFX", |ini, d, _| { d.default_death_fx = CrateCollideModuleData::parse_fx_list(ini)?; Ok(()) }, UserData::None),
    FieldParse::new("OrientToObject", |ini, d, _| { d.orient_to_object = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<FXListDieModuleData>; 6] = upgrade_module_data_field_parse!(FXListDieModuleData, upgrade);

impl FXListDieModuleData {
    /// `buildFieldParse(p)`: the behaviour base, the die mux and this class's fields, then the upgrade mux's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FXListDieModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FXListDieModuleData> {
        let mut data = FXListDieModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FXListDieModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class FXListDie : public DieModule, public UpgradeMux`.
#[derive(Clone, Debug)]
pub struct FXListDie {
    die: DieModule,
    /// `getFXListDieModuleData()`.
    data: Arc<FXListDieModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
}

impl PartialEq for FXListDie {
    /// The same object, the same data object (upstream compares pointers) and the same executed flag.
    fn eq(&self, other: &Self) -> bool {
        self.die.behavior().object() == other.die.behavior().object() && Arc::ptr_eq(&self.data, &other.data) && self.mux == other.mux
    }
}

impl FXListDie {
    /// `FXListDie::getInterfaceMask()`: `BehaviorModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        DieModule::get_interface_mask() | MODULEINTERFACE_UPGRADE
    }

    /// `FXListDie(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<FXListDieModuleData>>) -> IniResult<FXListDie> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let die = DieModule::new(object, Some(Arc::new(data.die.clone())))?;
        Ok(FXListDie { die, data, mux: UpgradeMux::new() })
    }

    /// The data, as `getFXListDieModuleData()`.
    pub fn module_data(&self) -> &Arc<FXListDieModuleData> {
        &self.data
    }

    /// Whether the upgrade ran (`isUpgradeActive()`), for the tests.
    pub fn is_upgrade_active(&self) -> bool {
        self.mux.is_already_upgraded()
    }

    /// The world half of the constructor: `if (m_initiallyActive) giveSelfUpgrade()`.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if self.data.initially_active && !crate::logic::modules::plant(887) {
            if let Err(e) = give_self_upgrade(self, world, me) {
                world.cuts.module_errors.push((me, e));
            }
        }
    }

    /// The module's answer to one of `Object`'s calls: `onDie` and the upgrade calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        die_module_call(self, world, me, call)
    }
}

impl DieBehavior for FXListDie {
    fn die(&self) -> &DieModule {
        &self.die
    }
    /// `FXListDie::onDie(damageInfo)`.
    fn on_die(&mut self, world: &mut World, me: ModuleHandle, info: &DamageInfo) {
        if !self.mux.is_already_upgraded() && !crate::logic::modules::plant(881) {
            return;
        }
        if !self.is_die_applicable(world, me, info) && !crate::logic::modules::plant(886) {
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
        if obj.object_upgrades_completed.test_for_any(&conflicting) && !crate::logic::modules::plant(882) {
            return;
        }
        if let Some(player) = world.get_controlling_player(me.object) {
            if world.cuts.player_completed_upgrade_mask(player).test_for_any(&conflicting) && !crate::logic::modules::plant(883) {
                return;
            }
        }
        let Some(fx) = self.data.default_death_fx.clone() else { return };
        if self.data.orient_to_object != crate::logic::modules::plant(884) {
            let secondary = if crate::logic::modules::plant(885) { INVALID_ID } else { world.find_object(info.input.source_id).map_or(INVALID_ID, |o| o.id) };
            world.emit(ObjectClientEvent::FxListDoFxObjSecondary { fx, object_id: me.object, secondary });
        } else if let Some(o) = world.objects.find(me.object) {
            let position = o.get_position();
            world.emit(ObjectClientEvent::FxListDoFxPos { fx, position });
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FXListDie(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FXListDie(m) => Some(m),
            _ => None,
        }
    }
}

impl UpgradeMuxUser for FXListDie {
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
        UpdateBody::FXListDie(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FXListDie(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for FXListDie {
    /// `crc`: `DieModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.die.crc(xfer)
    }
    /// `xfer` (version 1): the version, `DieModule::xfer`. (The mux's executed flag is not saved.)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.die.xfer(xfer)
    }
    /// `loadPostProcess`: `DieModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.die.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `FXListDie`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FXListDie(Arc::new(FXListDieModuleData::friend_new_module_data(ini)?)))
}
