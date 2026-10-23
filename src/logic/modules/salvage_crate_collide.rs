// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SalvageCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/SalvageCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SalvageCrateCollide`: the salvage a salvager (`KINDOF_SALVAGER`) picks up. In order, the first that applies: an armor upgrade for an
//! `ARMOR_SALVAGER` not yet at the second (always taken: "if you can get it, you get it"), a weapon upgrade for a `WEAPON_SALVAGER` not yet at the
//! second (with `WeaponChance`), a level for a trainable unit below heroic (with `LevelChance`), else money between `MinMoney` and `MaxMoney`
//! (`MoneyChance` is read and never used: "just assume the testMoneyChance"). Every pickup is recorded in the collector's player's academy stats.
//!
//! Upstream facts kept: a chance of exactly 1 draws no random number; the money is a random value only when the two bounds differ; the floating
//! text is over the crate, in the collector's player's colour; the collector's controlling player is read unchecked for money above 0 and for the
//! academy stats ([`Undefined::NullControllingPlayer`]).

use std::sync::Arc;

use crate::common::game_common::VeterancyLevel;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::{KINDOF_ARMOR_SALVAGER, KINDOF_SALVAGER, KINDOF_WEAPON_SALVAGER};
use crate::common::model_state::{MODELCONDITION_ARMORSET_CRATEUPGRADE_ONE, MODELCONDITION_ARMORSET_CRATEUPGRADE_TWO};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::armor_set::{ARMORSET_CRATE_UPGRADE_ONE, ARMORSET_CRATE_UPGRADE_TWO};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{crate_is_valid_to_execute, crate_module_call, CrateBehavior, CrateCollideModuleData};
use crate::logic::weapon_types::{WEAPONSET_CRATEUPGRADE_ONE, WEAPONSET_CRATEUPGRADE_TWO};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::update_body::UpdateBody;

/// `class SalvageCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SalvageCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_weaponChance`: "Chance to get a weapon upgrade, if possible".
    pub weapon_chance: f32,
    /// `m_levelChance`: "Chance to get a level, if weaponChance fails".
    pub level_chance: f32,
    /// `m_moneyChance`: "Chance to get money, if weaponChance fails" (never read).
    pub money_chance: f32,
    /// `m_minimumMoney`.
    pub minimum_money: i32,
    /// `m_maximumMoney`.
    pub maximum_money: i32,
}

impl SalvageCrateCollideModuleData {
    /// `SalvageCrateCollideModuleData()`.
    pub fn new() -> Self {
        SalvageCrateCollideModuleData {
            crate_data: CrateCollideModuleData::new(),
            weapon_chance: 1.0,
            level_chance: 0.25,
            money_chance: 0.75,
            minimum_money: 25,
            maximum_money: 75,
        }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, SalvageCrateCollideModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<SalvageCrateCollideModuleData> {
        let mut data = SalvageCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SalvageCrateCollideModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<SalvageCrateCollideModuleData>; 12] = crate_collide_data_field_parse!(SalvageCrateCollideModuleData, crate_data);

/// `SalvageCrateCollideModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<SalvageCrateCollideModuleData>; 6] = [
    FieldParse::new("WeaponChance", |ini, d, _| { d.weapon_chance = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("LevelChance", |ini, d, _| { d.level_chance = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MoneyChance", |ini, d, _| { d.money_chance = ini.parse_percent_to_real()?; Ok(()) }, UserData::None),
    FieldParse::new("MinMoney", |ini, d, _| { d.minimum_money = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("MaxMoney", |ini, d, _| { d.maximum_money = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::end(),
];

/// `GameMakeColor(0, 0, 0, 230)`: `(a << 24) | (r << 16) | (g << 8) | b`.
const FLOATING_ALPHA: i32 = (230u32 << 24) as i32;

/// `class SalvageCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct SalvageCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getSalvageCrateCollideModuleData()`.
    data: Arc<SalvageCrateCollideModuleData>,
}

impl PartialEq for SalvageCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object() && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl SalvageCrateCollide {
    /// `SalvageCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(object: ObjectId, data: Option<Arc<SalvageCrateCollideModuleData>>) -> IniResult<SalvageCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(SalvageCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getSalvageCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<SalvageCrateCollideModuleData> {
        &self.data
    }

    /// `eligibleForWeaponSet(other)`: "A kindof marks eligibility, and you must not be fully upgraded".
    fn eligible_for_weapon_set(world: &World, other: ObjectId) -> bool {
        let Some(o) = world.objects.find(other) else { return false };
        o.is_kind_of(KINDOF_WEAPON_SALVAGER) && (!o.test_weapon_set_flag(WEAPONSET_CRATEUPGRADE_TWO) || crate::logic::modules::plant(1682))
    }

    /// `eligibleForArmorSet(other)`.
    fn eligible_for_armor_set(world: &mut World, other: ObjectId) -> bool {
        if !world.objects.find(other).is_some_and(|o| o.is_kind_of(KINDOF_ARMOR_SALVAGER)) {
            return false;
        }
        !world.test_armor_set_flag(other, ARMORSET_CRATE_UPGRADE_TWO)
    }

    /// `eligibleForLevel(other)`: "Sorry, you are max level", "Sorry, you can't gain levels".
    fn eligible_for_level(world: &World, other: ObjectId) -> bool {
        let Some(o) = world.objects.find(other) else { return false };
        if o.experience.level == VeterancyLevel::Heroic && !crate::logic::modules::plant(1683) {
            return false;
        }
        o.template().is_trainable()
    }

    /// `testWeaponChance()` / `testLevelChance()`: "don't waste a random number for a 100%".
    fn test_chance(world: &mut World, chance: f32) -> bool {
        if chance == 1.0 {
            return true;
        }
        let random_number = world.random.game_logic_random_value_real(0.0, 1.0);
        random_number < chance
    }

    /// `doWeaponSet(other)`: the first upgrade, or the second in its place.
    fn do_weapon_set(world: &mut World, other: ObjectId) {
        if world.objects.find(other).is_some_and(|o| o.test_weapon_set_flag(WEAPONSET_CRATEUPGRADE_ONE)) && !crate::logic::modules::plant(1684) {
            world.clear_weapon_set_flag(other, WEAPONSET_CRATEUPGRADE_ONE);
            world.set_weapon_set_flag(other, WEAPONSET_CRATEUPGRADE_TWO);
        } else {
            world.set_weapon_set_flag(other, WEAPONSET_CRATEUPGRADE_ONE);
        }
    }

    /// `doArmorSet(other)`: the first upgrade, or the second in its place, with the model condition.
    fn do_armor_set(world: &mut World, other: ObjectId) {
        if world.test_armor_set_flag(other, ARMORSET_CRATE_UPGRADE_ONE) {
            world.clear_armor_set_flag(other, ARMORSET_CRATE_UPGRADE_ONE);
            world.set_armor_set_flag(other, ARMORSET_CRATE_UPGRADE_TWO);
            if !crate::logic::modules::plant(1685) {
                world.clear_and_set_model_condition_state(other, MODELCONDITION_ARMORSET_CRATEUPGRADE_ONE, MODELCONDITION_ARMORSET_CRATEUPGRADE_TWO);
            }
        } else {
            world.set_armor_set_flag(other, ARMORSET_CRATE_UPGRADE_ONE);
            world.set_model_condition_state(other, MODELCONDITION_ARMORSET_CRATEUPGRADE_ONE);
        }
    }

    /// `doMoney(other)`: a deposit, the score and the floating text over the crate.
    fn do_money(&self, world: &mut World, me: ObjectId, other: ObjectId) -> Result<(), IniError> {
        let md = &self.data;
        // "Random value doesn't like to get a constant range"
        let money = if md.minimum_money != md.maximum_money && !crate::logic::modules::plant(1686) {
            world.random.game_logic_random_value(md.minimum_money, md.maximum_money)
        } else {
            md.minimum_money
        };
        if money > 0 {
            let player = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            world.cut(CutCall::PlayerMoneyDeposit { player, amount: money as u32 });
            if !crate::logic::modules::plant(1687) {
                world.cut(CutCall::PlayerScoreAddMoneyEarned { player, amount: money });
            }
            // "Display cash income floating over the crate. Position is me, everything else is them."
            let mut pos = world.objects.find(me).map(|o| *o.thing.position()).unwrap_or_default();
            pos.z += if crate::logic::modules::plant(1688) { 0.0 } else { 10.0 };
            let color = world.cuts.player_color(player) | FLOATING_ALPHA;
            world.cut(CutCall::InGameUiAddFloatingTextNamed { text: b"GUI:AddCash".to_vec(), amount: money, position: pos, color });
        }
        Ok(())
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for SalvageCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }

    /// `isValidToExecute(other)`: "Only salvage units can pick up a Salvage crate".
    fn is_valid_to_execute(&self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        if !crate_is_valid_to_execute(&self.data.crate_data, world, me, other)? {
            return Ok(false);
        }
        Ok(world.objects.find(other).is_some_and(|o| o.template().is_kind_of(KINDOF_SALVAGER)) || crate::logic::modules::plant(1681))
    }

    /// `executeCrateBehavior(other)`.
    fn execute_crate_behavior(&mut self, world: &mut World, me: ModuleHandle, other: ObjectId) -> Result<bool, IniError> {
        let salvage_sound = ObjectClientEvent::Sound { sound: ObjectSound::CrateSalvage, object_id: other, position: None };
        if Self::eligible_for_armor_set(world, other) {
            // "No percent chance on this one, if you can get it, you get it."
            Self::do_armor_set(world, other);
            world.emit(salvage_sound);
        } else if Self::eligible_for_weapon_set(world, other) && Self::test_chance(world, self.data.weapon_chance) {
            Self::do_weapon_set(world, other);
            world.emit(salvage_sound);
        } else if Self::eligible_for_level(world, other) && Self::test_chance(world, if crate::logic::modules::plant(1689) { self.data.weapon_chance } else { self.data.level_chance }) {
            world.cut_experience_gain_for_level(other, 1, true);
        } else {
            // just assume the testMoneyChance
            self.do_money(world, me.object, other)?;
            if !crate::logic::modules::plant(1690) {
                world.emit(ObjectClientEvent::Sound { sound: ObjectSound::CrateMoney, object_id: other, position: None });
            }
        }
        let player = world.get_controlling_player(other).ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        if !crate::logic::modules::plant(1691) {
            world.cut(CutCall::PlayerAcademyRecordSalvageCollected { player });
        }
        Ok(true)
    }

    fn into_body(self) -> UpdateBody {
        UpdateBody::SalvageCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SalvageCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for SalvageCrateCollide {
    /// `crc`: `CrateCollide::crc`, which is `CollideModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.collide.crc(xfer)
    }
    /// `xfer` (version 1, "Initial version"): the version, `CrateCollide::xfer` (its own version, then `CollideModule::xfer`).
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        let mut crate_version = current;
        xfer.xfer_version(&mut crate_version, current)?;
        self.collide.xfer(xfer)
    }
    /// `loadPostProcess`: `CollideModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.collide.load_post_process();
    }
}

/// The factory's `NewModuleDataProc` for `SalvageCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SalvageCrateCollide(Arc::new(SalvageCrateCollideModuleData::friend_new_module_data(ini)?)))
}
