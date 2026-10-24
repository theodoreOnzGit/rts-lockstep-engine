// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/BodyModule.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Body/BodyModule.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Colin Day, November 2001
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `BodyModule` and `BodyModuleData`: the base of the modules that hold an
//! object's health (`ActiveBody`, `StructureBody`, ... in layer 9), with
//! `BodyDamageType` and `MaxHealthChangeType`.
//!
//! # What lives in the base
//!
//! The pure virtuals (`attemptDamage`, `attemptHealing`, `estimateDamage`,
//! `getHealth`, `getDamageState`, `setDamageState`, `setAflame`,
//! `onVeterancyLevelChanged`, `setArmorSetFlag`, `clearArmorSetFlag`,
//! `testArmorSetFlag`, `internalChangeHealth`) belong to the concrete bodies.
//! The base supplies **defaults for the rest** (a generic body has no max
//! health, no subdual, no last attacker, is indestructible, ...) and the
//! **damage scalar**: "allows outside systems to apply defensive bonuses or
//! penalties (they all stack as a multiplier!)", starting at 1 and saved.
//! The defaults are the `get_*` / `set_*` methods below, which the bodies of
//! layer 9 override by their own `match` arms.
//!
//! # `TheMaxHealthChangeTypeNames`
//!
//! Upstream's name list has **three** names for four enumerators
//! (`FULLY_HEAL` has none) and, unlike every other name list, **no `NULL`
//! terminator**. `INI::scanIndexList` over it (`MaxHealthUpgrade`'s
//! `ChangeType`) walks past the array's end for any other token, which is
//! undefined; the Rust list ([`MAX_HEALTH_CHANGE_TYPE_NAMES`]) has the three
//! names, so a caller of `scan_index_list` gets `INI_INVALID_DATA` for
//! `FULLY_HEAL` and for unknown tokens alike.
//!
//! # Cut points
//!
//! `DamageInfo`, `DamageInfoInput` and the armor flags in the pure virtuals'
//! signatures are layer 8's to hand over; `getLastDamageInfo` returns
//! `NULL` in the base ([`BodyModule::get_last_damage_info_is_present`]).
//! `setFrontCrushed` / `setBackCrushed` are `DEBUG_CRASH`es in the base
//! ("you should never call this for generic Bodys"): no-ops in a release
//! build, so no-ops here.

use std::sync::Arc;

use crate::common::ini::{IniResult, MultiIniFieldParse};
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_BODY};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::{BehaviorModule, BehaviorModuleData};

/// `enum BodyDamageType`: "damage states for structures". The comparisons
/// below depend on the enumerators being in this order.
pub type BodyDamageType = i32;
/// `BODY_PRISTINE`: "unit should appear in pristine condition".
pub const BODY_PRISTINE: BodyDamageType = 0;
/// `BODY_DAMAGED`: "unit has been damaged".
pub const BODY_DAMAGED: BodyDamageType = 1;
/// `BODY_REALLYDAMAGED`: "unit is extremely damaged / nearly destroyed".
pub const BODY_REALLYDAMAGED: BodyDamageType = 2;
/// `BODY_RUBBLE`: "unit has been reduced to rubble/corpse/exploded-hulk, etc".
pub const BODY_RUBBLE: BodyDamageType = 3;
/// `BODYDAMAGETYPE_COUNT`.
pub const BODYDAMAGETYPE_COUNT: BodyDamageType = 4;

/// `TheBodyDamageTypeNames[]` (under `DEFINE_BODYDAMAGETYPE_NAMES`), without
/// the `NULL` terminator.
pub const BODY_DAMAGE_TYPE_NAMES: [&str; 4] = ["PRISTINE", "DAMAGED", "REALLYDAMAGED", "RUBBLE"];

/// `enum MaxHealthChangeType`.
pub type MaxHealthChangeType = i32;
/// `SAME_CURRENTHEALTH`.
pub const SAME_CURRENTHEALTH: MaxHealthChangeType = 0;
/// `PRESERVE_RATIO`.
pub const PRESERVE_RATIO: MaxHealthChangeType = 1;
/// `ADD_CURRENT_HEALTH_TOO`.
pub const ADD_CURRENT_HEALTH_TOO: MaxHealthChangeType = 2;
/// `FULLY_HEAL`.
pub const FULLY_HEAL: MaxHealthChangeType = 3;

/// `TheMaxHealthChangeTypeNames[]` (under `DEFINE_MAXHEALTHCHANGETYPE_NAMES`):
/// three names, no `FULLY_HEAL` (see the module docs).
pub const MAX_HEALTH_CHANGE_TYPE_NAMES: [&str; 3] = ["SAME_CURRENTHEALTH", "PRESERVE_RATIO", "ADD_CURRENT_HEALTH_TOO"];

/// `IS_CONDITION_WORSE(a, b)`: `a > b`.
pub fn is_condition_worse(a: BodyDamageType, b: BodyDamageType) -> bool {
    a > b
}

/// `IS_CONDITION_BETTER(a, b)`: `a < b`.
pub fn is_condition_better(a: BodyDamageType, b: BodyDamageType) -> bool {
    a < b
}

/// `class BodyModuleData : public BehaviorModuleData`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BodyModuleData {
    /// The `BehaviorModuleData` base.
    pub behavior: BehaviorModuleData,
}

impl BodyModuleData {
    /// `BodyModuleData()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `BodyModuleData::buildFieldParse(p)`: the base's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, BodyModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)
    }
}

/// `class BodyModule : public BehaviorModule, public BodyModuleInterface`.
#[derive(Clone, Debug)]
pub struct BodyModule {
    /// The `BehaviorModule` base.
    behavior: BehaviorModule,
    /// `m_damageScalar`.
    damage_scalar: f32,
}

impl BodyModule {
    /// `BodyModule(thing, moduleData)`: damage scalar 1. `INI_INVALID_DATA`
    /// for null data.
    pub fn new(object: ObjectId, module_data: Option<Arc<BodyModuleData>>) -> IniResult<Self> {
        let behavior = BehaviorModule::new(object, module_data.map(ModuleDataKind::Body))?;
        Ok(BodyModule { behavior, damage_scalar: 1.0 })
    }

    /// `BodyModule::getInterfaceMask()`: `MODULEINTERFACE_BODY`.
    pub const fn get_interface_mask() -> i32 {
        MODULEINTERFACE_BODY
    }

    /// The `BehaviorModule` base.
    pub fn behavior(&self) -> &BehaviorModule {
        &self.behavior
    }

    /// `applyDamageScalar(scalar)`: "they all stack as a multiplier!".
    pub fn apply_damage_scalar(&mut self, scalar: f32) {
        self.damage_scalar *= scalar;
    }

    /// `getDamageScalar()`.
    pub fn get_damage_scalar(&self) -> f32 {
        self.damage_scalar
    }

    /// `getMaxHealth()`: 0.
    pub fn get_max_health(&self) -> f32 {
        0.0
    }

    /// `getPreviousHealth()`: 0.
    pub fn get_previous_health(&self) -> f32 {
        0.0
    }

    /// `getSubdualDamageHealRate()`: 0.
    pub fn get_subdual_damage_heal_rate(&self) -> u32 {
        0
    }

    /// `getSubdualDamageHealAmount()`: 0.
    pub fn get_subdual_damage_heal_amount(&self) -> f32 {
        0.0
    }

    /// `hasAnySubdualDamage()`: false.
    pub fn has_any_subdual_damage(&self) -> bool {
        false
    }

    /// `getCurrentSubdualDamageAmount()`: 0.
    pub fn get_current_subdual_damage_amount(&self) -> f32 {
        0.0
    }

    /// `getInitialHealth()`: 0.
    pub fn get_initial_health(&self) -> f32 {
        0.0
    }

    /// `getLastDamageInfo()` returns `NULL` in the base: whether there is one.
    pub fn get_last_damage_info_is_present(&self) -> bool {
        false
    }

    /// `getLastDamageTimestamp()`: 0.
    pub fn get_last_damage_timestamp(&self) -> u32 {
        0
    }

    /// `getLastHealingTimestamp()`: 0.
    pub fn get_last_healing_timestamp(&self) -> u32 {
        0
    }

    /// `getClearableLastAttacker()`: `INVALID_ID`.
    pub fn get_clearable_last_attacker(&self) -> ObjectId {
        crate::common::game_type::INVALID_ID
    }

    /// `clearLastAttacker()`: nothing.
    pub fn clear_last_attacker(&mut self) {}

    /// `getFrontCrushed()`: false.
    pub fn get_front_crushed(&self) -> bool {
        false
    }

    /// `getBackCrushed()`: false.
    pub fn get_back_crushed(&self) -> bool {
        false
    }

    /// `setInitialHealth(initialPercent)`: nothing.
    pub fn set_initial_health(&mut self, _initial_percent: i32) {}

    /// `setMaxHealth(maxHealth, healthChangeType)`: nothing.
    pub fn set_max_health(&mut self, _max_health: f32, _health_change_type: MaxHealthChangeType) {}

    /// `setFrontCrushed(v)`: a `DEBUG_CRASH` ("you should never call this for
    /// generic Bodys"), nothing in a release build.
    pub fn set_front_crushed(&mut self, _v: bool) {}

    /// `setBackCrushed(v)`: as `setFrontCrushed`.
    pub fn set_back_crushed(&mut self, _v: bool) {}

    /// `setIndestructible(indestructible)`: nothing.
    pub fn set_indestructible(&mut self, _indestructible: bool) {}

    /// `isIndestructible()`: true.
    pub fn is_indestructible(&self) -> bool {
        true
    }

    /// `evaluateVisualCondition()`: nothing.
    pub fn evaluate_visual_condition(&mut self) {}

    /// `updateBodyParticleSystems()`: nothing.
    pub fn update_body_particle_systems(&mut self) {}
}

impl Snapshot for BodyModule {
    /// `BodyModule::crc`: `BehaviorModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.behavior.crc(xfer)
    }
    /// `BodyModule::xfer` (version 1): the version, `BehaviorModule::xfer`,
    /// the damage scalar.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.behavior.xfer(xfer)?;
        xfer.xfer_real(&mut self.damage_scalar)
    }
    /// `BodyModule::loadPostProcess`: `BehaviorModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.behavior.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::module::test_support::{load_bytes, save_bytes};

    fn module() -> BodyModule {
        BodyModule::new(2, Some(Arc::new(BodyModuleData::new()))).unwrap()
    }

    #[test]
    fn damage_scalars_stack_as_a_product() {
        let mut m = module();
        assert_eq!(m.get_damage_scalar(), 1.0);
        m.apply_damage_scalar(0.5);
        m.apply_damage_scalar(0.5);
        m.apply_damage_scalar(3.0);
        assert_eq!(m.get_damage_scalar(), 0.75);
        m.apply_damage_scalar(0.0);
        assert_eq!(m.get_damage_scalar(), 0.0);
        assert!(BodyModule::new(2, None).is_err());
        assert_eq!(BodyModule::get_interface_mask(), 0x20);
    }

    #[test]
    fn the_defaults_are_those_of_a_generic_body() {
        let mut m = module();
        assert_eq!((m.get_max_health(), m.get_previous_health(), m.get_initial_health()), (0.0, 0.0, 0.0));
        assert_eq!((m.get_subdual_damage_heal_rate(), m.get_subdual_damage_heal_amount(), m.get_current_subdual_damage_amount()), (0, 0.0, 0.0));
        assert!(!m.has_any_subdual_damage() && !m.get_last_damage_info_is_present());
        assert_eq!((m.get_last_damage_timestamp(), m.get_last_healing_timestamp()), (0, 0));
        assert_eq!(m.get_clearable_last_attacker(), 0);
        assert!(!m.get_front_crushed() && !m.get_back_crushed());
        assert!(m.is_indestructible());
        m.set_indestructible(false);
        m.set_front_crushed(true);
        m.set_back_crushed(true);
        m.set_initial_health(50);
        m.set_max_health(10.0, FULLY_HEAL);
        m.clear_last_attacker();
        m.evaluate_visual_condition();
        m.update_body_particle_systems();
        assert!(m.is_indestructible() && !m.get_front_crushed() && m.get_max_health() == 0.0);
    }

    #[test]
    fn damage_states_compare_in_order() {
        assert!(is_condition_worse(BODY_RUBBLE, BODY_DAMAGED) && !is_condition_worse(BODY_DAMAGED, BODY_DAMAGED));
        assert!(is_condition_better(BODY_PRISTINE, BODY_REALLYDAMAGED) && !is_condition_better(BODY_RUBBLE, BODY_PRISTINE));
        assert_eq!(BODY_DAMAGE_TYPE_NAMES.len() as i32, BODYDAMAGETYPE_COUNT);
        assert_eq!(BODY_DAMAGE_TYPE_NAMES[BODY_REALLYDAMAGED as usize], "REALLYDAMAGED");
        // three names for four enumerators: FULLY_HEAL has none
        assert_eq!(MAX_HEALTH_CHANGE_TYPE_NAMES.len() as i32, FULLY_HEAL);
        assert_eq!((SAME_CURRENTHEALTH, PRESERVE_RATIO, ADD_CURRENT_HEALTH_TOO), (0, 1, 2));
    }

    #[test]
    fn xfer_saves_the_scalar() {
        let mut m = module();
        m.apply_damage_scalar(2.0);
        let bytes = save_bytes(&mut m).unwrap();
        assert_eq!(bytes, [1, 1, 1, 1, 0, 0, 0, 0x40]);
        let mut other = module();
        load_bytes(&mut other, bytes).unwrap();
        assert_eq!(other.get_damage_scalar(), 2.0);
    }
}
