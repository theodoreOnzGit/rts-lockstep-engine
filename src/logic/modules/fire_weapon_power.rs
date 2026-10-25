// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/FireWeaponPower.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/SpecialPower/FireWeaponPower.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FireWeaponPower`: a power that makes the object fire its own weapons (the Gattling cannon's barrage). Each of the three forms (no target,
//! a location, an object) does nothing while the object is disabled; otherwise it does the base's, reloads all the object's weapons at once
//! (`reloadAllAmmo(TRUE)`), and, with an AI, orders an attack for `MaxShotsToFire` shots (a position attack at `NULL` for no target) and
//! points both turrets at the same target (the object's own position for no target).
//!
//! `p1w13-sonnet`, batch R63, 2026-10-25.

use std::sync::Arc;

use crate::common::base_type::Coord3D;
use crate::common::game_common::MAX_TURRETS;
use crate::common::ini::{FieldParse, Ini, IniError, IniResult, MultiIniFieldParse, UserData};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::special_power_module::{SpecialPowerModule, SpecialPowerModuleData};
use crate::logic::modules::open_contain::CMD_FROM_AI;
use crate::logic::modules::special_power_body::{self, special_power_module_call, SpecialPowerBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::update_body::UpdateBody;
use crate::special_power_data_field_parse;

/// `class FireWeaponPowerModuleData : public SpecialPowerModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct FireWeaponPowerModuleData {
    /// The `SpecialPowerModuleData` base.
    pub special_power: SpecialPowerModuleData,
    /// `m_maxShotsToFire`.
    pub max_shots_to_fire: u32,
}

impl Default for FireWeaponPowerModuleData {
    /// `FireWeaponPowerModuleData()`.
    fn default() -> Self {
        FireWeaponPowerModuleData { special_power: SpecialPowerModuleData::new(), max_shots_to_fire: 1 }
    }
}

impl FireWeaponPowerModuleData {
    /// `buildFieldParse(p)`: the behaviour base, the special power fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, FireWeaponPowerModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<FireWeaponPowerModuleData> {
        let mut data = FireWeaponPowerModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, FireWeaponPowerModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

static BASE_FIELD_PARSE: [FieldParse<FireWeaponPowerModuleData>; 6] = special_power_data_field_parse!(FireWeaponPowerModuleData, special_power);

static OWN_FIELD_PARSE: [FieldParse<FireWeaponPowerModuleData>; 2] = [
    FieldParse::new("MaxShotsToFire", |ini, d, _| { d.max_shots_to_fire = ini.parse_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `class FireWeaponPower : public SpecialPowerModule`.
#[derive(Clone, Debug)]
pub struct FireWeaponPower {
    /// The `SpecialPowerModule` base.
    power: SpecialPowerModule,
    /// `getFireWeaponPowerModuleData()`.
    data: Arc<FireWeaponPowerModuleData>,
}

impl PartialEq for FireWeaponPower {
    /// The same object, the same data object (upstream compares pointers) and the same base state.
    fn eq(&self, other: &Self) -> bool {
        self.power.behavior().object() == other.power.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.power.get_available_on_frame() == other.power.get_available_on_frame()
            && self.power.paused_count() == other.power.paused_count()
            && self.power.paused_on_frame() == other.power.paused_on_frame()
            && self.power.paused_percent().to_bits() == other.power.paused_percent().to_bits()
    }
}

/// What the attack is at: no target (`NULL` position, the turrets at the object), a location, an object.
enum At {
    Nothing,
    Location(Coord3D),
    Object(ObjectId),
}

impl FireWeaponPower {
    /// `FireWeaponPower(thing, moduleData)`, the part that touches no world: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<FireWeaponPowerModuleData>>) -> IniResult<FireWeaponPower> {
        let data = data.ok_or_else(|| IniError::new(crate::common::ini::IniErrorKind::InvalidData))?;
        let power = SpecialPowerModule::new(object, Some(Arc::new(data.special_power.clone())))?;
        Ok(FireWeaponPower { power, data })
    }

    /// `SpecialPowerModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        SpecialPowerModule::get_interface_mask()
    }

    /// The data, as `getFireWeaponPowerModuleData()`.
    pub fn module_data(&self) -> &Arc<FireWeaponPowerModuleData> {
        &self.data
    }

    /// The base's four state words (the ready frame, the pause count, frame and percent).
    pub fn power_state(&self) -> &SpecialPowerModule {
        &self.power
    }

    /// The world half of the constructor, run once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        if let Err(e) = special_power_body::construct(self, world, me) {
            world.cuts.module_errors.push((me, e));
        }
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        special_power_module_call(self, world, me, call)
    }

    /// What the three forms do after the base's: reload, attack, point the turrets.
    fn fire(&self, world: &mut World, us: ObjectId, at: At) {
        if !crate::logic::modules::plant(2331) {
            world.object_reload_all_ammo(us, true);
        }
        let Some(ai) = world.objects.find(us).and_then(|o| o.ai) else { return };
        let max_shots = if crate::logic::modules::plant(2332) { 1 } else { self.data.max_shots_to_fire as i32 };
        let own = world.objects.find(us).map(|o| o.get_position()).unwrap_or_default();
        match at {
            At::Nothing => {
                world.module_call(us, ai, ModuleCall::AiAttackPosition { pos: None, max_shots, command_source: CMD_FROM_AI });
                // "Order any turrets to attack as well."
                for turret in 0..MAX_TURRETS {
                    world.module_call(us, ai, ModuleCall::AiSetTurretTargetPosition { turret, pos: own });
                }
            }
            At::Location(loc) => {
                world.module_call(us, ai, ModuleCall::AiAttackPosition { pos: Some(loc), max_shots, command_source: CMD_FROM_AI });
                let turrets = if crate::logic::modules::plant(2333) { 1 } else { MAX_TURRETS };
                for turret in 0..turrets {
                    world.module_call(us, ai, ModuleCall::AiSetTurretTargetPosition { turret, pos: loc });
                }
            }
            At::Object(target) => {
                world.module_call(us, ai, ModuleCall::AiAttackObject { victim: target, max_shots, command_source: CMD_FROM_AI });
                for turret in 0..MAX_TURRETS {
                    world.module_call(us, ai, ModuleCall::AiSetTurretTargetObject { turret, target, force_attacking: false });
                }
            }
        }
    }

    fn disabled(world: &World, us: ObjectId) -> bool {
        world.objects.find(us).is_some_and(|o| o.is_disabled()) && !crate::logic::modules::plant(2334)
    }
}

impl SpecialPowerBehavior for FireWeaponPower {
    fn power(&self) -> &SpecialPowerModule {
        &self.power
    }
    fn power_mut(&mut self) -> &mut SpecialPowerModule {
        &mut self.power
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::FireWeaponPower(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::FireWeaponPower(m) => Some(m),
            _ => None,
        }
    }
    fn do_special_power(&mut self, world: &mut World, me: ModuleHandle, command_options: u32) -> Result<(), IniError> {
        if Self::disabled(world, me.object) {
            return Ok(());
        }
        special_power_body::base_do_special_power(self, world, me, command_options)?;
        self.fire(world, me.object, At::Nothing);
        Ok(())
    }
    fn do_special_power_at_location(&mut self, world: &mut World, me: ModuleHandle, loc: Coord3D, angle: f32, command_options: u32) -> Result<(), IniError> {
        if Self::disabled(world, me.object) {
            return Ok(());
        }
        special_power_body::base_do_special_power_at_location(self, world, me, loc, angle, command_options)?;
        self.fire(world, me.object, At::Location(loc));
        Ok(())
    }
    fn do_special_power_at_object(&mut self, world: &mut World, me: ModuleHandle, target: ObjectId, command_options: u32) -> Result<(), IniError> {
        if Self::disabled(world, me.object) {
            return Ok(());
        }
        special_power_body::base_do_special_power_at_object(self, world, me, target, command_options)?;
        self.fire(world, me.object, At::Object(target));
        Ok(())
    }
}

impl Snapshot for FireWeaponPower {
    /// `crc`: `SpecialPowerModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.power.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `SpecialPowerModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.power.xfer(xfer)
    }
    /// `loadPostProcess`: `SpecialPowerModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.power.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `FireWeaponPower`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::FireWeaponPower(Arc::new(FireWeaponPowerModuleData::friend_new_module_data(ini)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_shot_by_default() {
        assert_eq!(FireWeaponPowerModuleData::default().max_shots_to_fire, 1);
        let m = FireWeaponPower::new(1, Some(Arc::new(FireWeaponPowerModuleData::default()))).unwrap();
        assert_eq!(m.power_state().paused_count(), 0);
    }
}
