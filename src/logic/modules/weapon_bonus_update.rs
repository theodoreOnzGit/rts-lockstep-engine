// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/WeaponBonusUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/WeaponBonusUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `WeaponBonusUpdate`: an aura of temporary weapon bonus. Every `BonusDelay` frames (and at once when the object is made) every allied, living
//! object on the same map within `BonusRange` (2D, from the centre) that has the `RequiredAffectKindOf` kinds and none of the
//! `ForbiddenAffectKindOf` gets the `BonusConditionType` bonus for `BonusDuration` frames; so do the qualifying passengers of every such object
//! that has a contain, whether the container qualifies or not.
//!
//! The scan is [`World::partition_iterate_allies_in_range`]; the bonus [`World::crate_do_temp_weapon_bonus`]; the passengers the contain's
//! [`ModuleCall::GetContainedItems`], in order.

use std::sync::Arc;

use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_NONE,
};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::weapon_bonus::{condition, WEAPON_BONUS_NAMES};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class WeaponBonusUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponBonusUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_requiredAffectKindOf`.
    pub required_affect_kind_of: KindOfMaskType,
    /// `m_forbiddenAffectKindOf`.
    pub forbidden_affect_kind_of: KindOfMaskType,
    /// `m_bonusDuration`.
    pub bonus_duration: u32,
    /// `m_bonusDelay`.
    pub bonus_delay: u32,
    /// `m_bonusRange`.
    pub bonus_range: f32,
    /// `m_bonusConditionType` (`WEAPONBONUSCONDITION_INVALID` until read).
    pub bonus_condition_type: i32,
}

impl Default for WeaponBonusUpdateModuleData {
    /// `WeaponBonusUpdateModuleData()`: no kinds, no duration, delay or range, `WEAPONBONUSCONDITION_INVALID`.
    fn default() -> Self {
        WeaponBonusUpdateModuleData {
            update: UpdateModuleData::default(),
            required_affect_kind_of: KindOfMaskType::new(),
            forbidden_affect_kind_of: KindOfMaskType::new(),
            bonus_duration: 0,
            bonus_delay: 0,
            bonus_range: 0.0,
            bonus_condition_type: condition::INVALID,
        }
    }
}

/// `WeaponBonusUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<WeaponBonusUpdateModuleData>; 7] = [
    FieldParse::new(
        "RequiredAffectKindOf",
        |ini, d, _| d.required_affect_kind_of.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "ForbiddenAffectKindOf",
        |ini, d, _| d.forbidden_affect_kind_of.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::new(
        "BonusDuration",
        |ini, d, _| {
            d.bonus_duration = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BonusDelay",
        |ini, d, _| {
            d.bonus_delay = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BonusRange",
        |ini, d, _| {
            d.bonus_range = ini.parse_real()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "BonusConditionType",
        |ini, d, _| {
            d.bonus_condition_type = ini.parse_index_list(&WEAPON_BONUS_NAMES)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl WeaponBonusUpdateModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, WeaponBonusUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<WeaponBonusUpdateModuleData> {
        let mut data = WeaponBonusUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                WeaponBonusUpdateModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class WeaponBonusUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct WeaponBonusUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getWeaponBonusUpdateModuleData()`.
    data: Arc<WeaponBonusUpdateModuleData>,
}

impl PartialEq for WeaponBonusUpdate {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl WeaponBonusUpdate {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask()
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<WeaponBonusUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(WeaponBonusUpdate { update, data })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getWeaponBonusUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<WeaponBonusUpdateModuleData> {
        &self.data
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object: awake now.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        update_behavior::update_module_call(self, world, me, call)
    }

    /// `xfer` with the scheduling half the slot holds: the version, that half.
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

    /// `isKindOfMulti(required, forbidden)` of `obj`.
    fn qualifies(&self, world: &World, obj: ObjectId) -> bool {
        world.objects.find(obj).is_some_and(|o| {
            o.template().is_kind_of_multi(
                &self.data.required_affect_kind_of,
                &self.data.forbidden_affect_kind_of,
            )
        })
    }
}

impl UpdateBehavior for WeaponBonusUpdate {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let d = self.data.clone();
        for current in world.partition_iterate_allies_in_range(me.object, d.bonus_range) {
            if self.qualifies(world, current) && !crate::logic::modules::plant(1831) {
                world.crate_do_temp_weapon_bonus(current, d.bonus_condition_type, d.bonus_duration);
            }
            if let Some(contain) = world
                .objects
                .find(current)
                .and_then(|o| o.contain)
                .filter(|_| !crate::logic::modules::plant(1832))
            {
                // `containIteratingDoTempWeaponBonus`, over the passengers in order
                let passengers =
                    match world.module_call(current, contain, ModuleCall::GetContainedItems) {
                        ModuleReply::Ids(items) => items,
                        _ => Vec::new(),
                    };
                for passenger in passengers {
                    if self.qualifies(world, passenger) {
                        world.crate_do_temp_weapon_bonus(
                            passenger,
                            d.bonus_condition_type,
                            d.bonus_duration,
                        );
                    }
                }
            }
        }
        // "Only need an internal timer if there are external hooks for wakning us up, or a second thing we can do"
        update_sleep(d.bonus_delay + u32::from(crate::logic::modules::plant(1833)))
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    /// No fields after the base.
    fn xfer_fields(&mut self, _xfer: &mut dyn Xfer, _version: XferVersion) -> XferResult<()> {
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::WeaponBonusUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::WeaponBonusUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for WeaponBonusUpdate {
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

/// The factory's `NewModuleDataProc` for `WeaponBonusUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::WeaponBonusUpdate(Arc::new(
        WeaponBonusUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
