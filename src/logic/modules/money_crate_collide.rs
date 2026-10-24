// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/MoneyCrateCollide.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Collide/CrateCollide/MoneyCrateCollide.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `MoneyCrateCollide`: a crate of money. `executeCrateBehavior` deposits `MoneyProvided` plus the upgrade boost into `other`'s controlling player
//! ([`CutCall::PlayerMoneyDeposit`], which plays the cash sound, and [`CutCall::PlayerScoreAddMoneyEarned`]; no controlling player is a `NULL`
//! dereference upstream, an [`Undefined::NullControllingPlayer`] here) and plays `MiscAudio::m_crateMoney` attached to `other`. It always returns
//! `TRUE`.
//!
//! # An upstream defect kept: the `static` inside the loop
//!
//! `getUpgradedSupplyBoost` declares `static const UpgradeTemplate *upgradeTemplate = TheUpgradeCenter->findUpgrade(info.type.c_str());` INSIDE the
//! loop over the `UpgradedBoost` pairs. A function-local `static` is initialised once, the first time control reaches it: whichever pair the first
//! call reached first decides the template for every later pair, every later crate and every later call. Upstream's is process-wide; here the
//! cache lives in the `World` (`cuts.money_crate_upgrade_static`), so it is one per game, not one per program (a lockstep replay must not depend on
//! what an earlier game did). `AutoDepositUpdate::getUpgradedSupplyBoost` has the same defect, with its own `static`.

use std::sync::Arc;

use crate::common::ini::Undefined;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::crate_collide_data_field_parse;
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::collide_module::CollideModule;
use crate::logic::modules::crate_collide::{
    crate_module_call, CrateBehavior, CrateCollideModuleData,
};
use crate::logic::modules::upgrade_pair::{parse_upgrade_pair, UpgradePair};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound};
use crate::logic::world::update_body::UpdateBody;

/// `class MoneyCrateCollideModuleData : public CrateCollideModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MoneyCrateCollideModuleData {
    /// The `CrateCollideModuleData` base.
    pub crate_data: CrateCollideModuleData,
    /// `m_moneyProvided`.
    pub money_provided: u32,
    /// `m_upgradeBoost`: the pairs of `UpgradedBoost`, in file order.
    pub upgrade_boost: Vec<UpgradePair>,
}

impl MoneyCrateCollideModuleData {
    /// `MoneyCrateCollideModuleData()`.
    pub fn new() -> Self {
        MoneyCrateCollideModuleData {
            crate_data: CrateCollideModuleData::new(),
            money_provided: 0,
            upgrade_boost: Vec::new(),
        }
    }

    /// `buildFieldParse(p)`: the module data base, the crate fields, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, MoneyCrateCollideModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&BASE_FIELD_PARSE)?;
        p.add(&OWN_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<MoneyCrateCollideModuleData> {
        let mut data = MoneyCrateCollideModuleData::new();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                MoneyCrateCollideModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `CrateCollideModuleData::buildFieldParse`'s table, for this data class.
static BASE_FIELD_PARSE: [FieldParse<MoneyCrateCollideModuleData>; 12] =
    crate_collide_data_field_parse!(MoneyCrateCollideModuleData, crate_data);

/// `MoneyCrateCollideModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<MoneyCrateCollideModuleData>; 3] = [
    FieldParse::new(
        "MoneyProvided",
        |ini, d, _| {
            d.money_provided = ini.parse_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "UpgradedBoost",
        |ini, d, _| parse_upgrade_pair(ini, &mut d.upgrade_boost),
        UserData::None,
    ),
    FieldParse::end(),
];

/// `class MoneyCrateCollide : public CrateCollide`.
#[derive(Clone, Debug)]
pub struct MoneyCrateCollide {
    /// The `CollideModule` base.
    collide: CollideModule,
    /// `getMoneyCrateCollideModuleData()`.
    data: Arc<MoneyCrateCollideModuleData>,
}

impl PartialEq for MoneyCrateCollide {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.collide.behavior().object() == other.collide.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl MoneyCrateCollide {
    /// `MoneyCrateCollide(thing, moduleData)`: the base; `INI_INVALID_DATA` for no data.
    pub fn new(
        object: ObjectId,
        data: Option<Arc<MoneyCrateCollideModuleData>>,
    ) -> IniResult<MoneyCrateCollide> {
        let data = data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let collide = CollideModule::new(object, Some(Arc::new(data.crate_data.collide.clone())))?;
        Ok(MoneyCrateCollide { collide, data })
    }

    /// `CollideModule::getInterfaceMask()`.
    pub const fn get_interface_mask() -> i32 {
        CollideModule::get_interface_mask()
    }

    /// The data, as `getMoneyCrateCollideModuleData()`.
    pub fn module_data(&self) -> &Arc<MoneyCrateCollideModuleData> {
        &self.data
    }

    /// `getUpgradedSupplyBoost(other)`: the boost of the first pair whose upgrade the collecting player has completed; 0 with no player. See the module
    /// docs for the `static` that makes the template of the FIRST pair ever looked at the template of every pair.
    pub fn upgraded_supply_boost(&self, world: &mut World, other: ObjectId) -> i32 {
        let Some(player) = world.get_controlling_player(other) else {
            return 0;
        };
        for info in &self.data.upgrade_boost {
            // the function-local `static`, initialised the first time control reaches it
            if world.cuts.money_crate_upgrade_static.is_none() || crate::logic::modules::plant(773)
            {
                let found = world
                    .statics
                    .upgrade_center
                    .find_upgrade(&mut world.names, &info.type_name)
                    .ok()
                    .flatten()
                    .map(|t| t.upgrade_mask());
                world.cuts.money_crate_upgrade_static = Some(found);
            }
            if let Some(mask) = world.cuts.money_crate_upgrade_static.flatten() {
                if world
                    .players
                    .get_nth(player)
                    .is_some_and(|p| p.upgrades_completed.test_for_all(&mask))
                {
                    return info.amount;
                }
            }
        }
        0
    }

    /// The module's answer to one of `Object`'s calls.
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
        crate_module_call(self, world, me, call)
    }
}

impl CrateBehavior for MoneyCrateCollide {
    fn collide(&self) -> &CollideModule {
        &self.collide
    }
    fn crate_data(&self) -> &CrateCollideModuleData {
        &self.data.crate_data
    }
    fn execute_crate_behavior(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        other: ObjectId,
    ) -> Result<bool, IniError> {
        let _ = (&world, &me, &other);
        let mut money = self.data.money_provided;
        let boost = if crate::logic::modules::plant(771) {
            0
        } else {
            self.upgraded_supply_boost(world, other)
        };
        money = money.wrapping_add(boost as u32);
        let player = world
            .get_controlling_player(other)
            .ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
        world.cut(CutCall::PlayerMoneyDeposit {
            player,
            amount: money,
        });
        world.cut(CutCall::PlayerScoreAddMoneyEarned {
            player,
            amount: if crate::logic::modules::plant(772) {
                0
            } else {
                money as i32
            },
        });
        world.emit(ObjectClientEvent::Sound {
            sound: ObjectSound::CrateMoney,
            object_id: other,
            position: None,
        });
        Ok(true)
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::MoneyCrateCollide(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::MoneyCrateCollide(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for MoneyCrateCollide {
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

/// The factory's `NewModuleDataProc` for `MoneyCrateCollide`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::MoneyCrateCollide(Arc::new(
        MoneyCrateCollideModuleData::friend_new_module_data(ini)?,
    )))
}
