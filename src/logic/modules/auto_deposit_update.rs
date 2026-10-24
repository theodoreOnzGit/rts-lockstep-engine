// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AutoDepositUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AutoDepositUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `AutoDepositUpdate`: pays its controlling player `DepositAmount` (plus the boost of an upgrade the player has) every `DepositTiming` frames, and a one-off
//! `InitialCaptureBonus` when the object changes hands. It never sleeps (`UPDATE_SLEEP_NONE`: "@todo srj use SLEEPY_UPDATE here").
//!
//! # Effects
//!
//! `Player::getMoney()->deposit(amount)` is [`CutCall::PlayerMoneyDeposit`], `getScoreKeeper()->addMoneyEarned(amount)`
//! [`CutCall::PlayerScoreAddMoneyEarned`], and the floating text over the object `TheInGameUI->addFloatingText(text, &pos, color)`
//! [`CutCall::InGameUiAddFloatingText`] (the text is `GUI:AddCash` with the amount). The text's position is the object's plus 10 in z; over a structure it is moved
//! by `GameClientRandomValue(-width, width)` and `GameClientRandomValue(-depth, depth)` with `width = majorRadius * 0.3f` and `depth = minorRadius * 0.3f`,
//! **`Real`s passed to the `Int` function**: truncated toward zero, and a value outside `int` (or NaN) is undefined
//! ([`Undefined::FloatToIntOutOfRange`]). The colour is the player's `| GameMakeColor(0, 0, 0, 230)`.
//!
//! # Upstream facts kept
//!
//! - `getUpgradedSupplyBoost` declares `static const UpgradeTemplate *upgradeTemplate = ...findUpgrade(info.type.c_str())` INSIDE its loop: the template of the
//!   first pair ever reached is the template of every pair of every `AutoDepositUpdate` for the rest of the process (`MoneyCrateCollide` has the same defect with
//!   its own `static`). Here it is `cuts.auto_deposit_upgrade_static`, one per `World`.
//! - The deposit is `moneyAmount` (the amount plus the boost, an `Int`, a negative sum becomes a huge `UnsignedInt`), the score is `m_depositAmount` alone.
//! - `awardInitialCaptureBonus` resets `m_depositOnFrame` first, whatever the player; `Player::becomingTeamMember` calls it ([`ModuleCall::AwardInitialCaptureBonus`]).
//! - A controlling player that is `NULL` (with `ActualMoney`, or a floating text) is dereferenced upstream: [`Undefined::NullControllingPlayer`].
//!
//! `p1w11-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::game_common::Relationship;
use crate::common::ini::{FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, Undefined, UserData};
use crate::common::kind_of::KINDOF_STRUCTURE;
use crate::common::module::{ModuleDataKind, ObjectId};
use crate::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime, UPDATE_SLEEP_NONE};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_pair::{parse_upgrade_pair, UpgradePair};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `CONSTRUCTION_COMPLETE`.
const CONSTRUCTION_COMPLETE: f32 = -1.0;

/// `class AutoDepositUpdateModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoDepositUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_depositFrame` (`DepositTiming`, a duration in frames).
    pub deposit_frame: u32,
    /// `m_depositAmount`.
    pub deposit_amount: i32,
    /// `m_initialCaptureBonus`.
    pub initial_capture_bonus: i32,
    /// `m_isActualMoney` (`ActualMoney`).
    pub is_actual_money: bool,
    /// `m_upgradeBoost`: the pairs of `UpgradedBoost`, in file order.
    pub upgrade_boost: Vec<UpgradePair>,
}

impl Default for AutoDepositUpdateModuleData {
    /// `AutoDepositUpdateModuleData()`.
    fn default() -> Self {
        AutoDepositUpdateModuleData { update: UpdateModuleData::new(), deposit_frame: 0, deposit_amount: 0, initial_capture_bonus: 0, is_actual_money: true, upgrade_boost: Vec::new() }
    }
}

/// `AutoDepositUpdateModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<AutoDepositUpdateModuleData>; 6] = [
    FieldParse::new("DepositTiming", |ini, d, _| { d.deposit_frame = ini.parse_duration_unsigned_int()?; Ok(()) }, UserData::None),
    FieldParse::new("DepositAmount", |ini, d, _| { d.deposit_amount = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("InitialCaptureBonus", |ini, d, _| { d.initial_capture_bonus = ini.parse_int()?; Ok(()) }, UserData::None),
    FieldParse::new("ActualMoney", |ini, d, _| { d.is_actual_money = ini.parse_bool()?; Ok(()) }, UserData::None),
    FieldParse::new("UpgradedBoost", |ini, d, _| parse_upgrade_pair(ini, &mut d.upgrade_boost), UserData::None),
    FieldParse::end(),
];

impl AutoDepositUpdateModuleData {
    /// `buildFieldParse(p)`: the behaviour base, then this class's.
    pub fn build_field_parse(p: &mut MultiIniFieldParse<'_, AutoDepositUpdateModuleData>) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<AutoDepositUpdateModuleData> {
        let mut data = AutoDepositUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, AutoDepositUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class AutoDepositUpdate : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct AutoDepositUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getAutoDepositUpdateModuleData()`.
    data: Arc<AutoDepositUpdateModuleData>,
    /// `m_depositOnFrame`.
    deposit_on_frame: u32,
    /// `m_awardInitialCaptureBonus`.
    award_initial_capture_bonus: bool,
    /// `m_initialized`.
    initialized: bool,
}

impl PartialEq for AutoDepositUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.deposit_on_frame == other.deposit_on_frame
            && self.award_initial_capture_bonus == other.award_initial_capture_bonus
            && self.initialized == other.initialized
    }
}

/// The `Int` an `Int`-parameter receives from a `Real` argument (`(Int)x`): truncated toward zero; a value `int` cannot hold is undefined.
fn real_to_int(x: f32) -> Result<i32, IniError> {
    if x.is_nan() || !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        return Err(IniError::undefined(Undefined::FloatToIntOutOfRange));
    }
    Ok(x as i32)
}

impl AutoDepositUpdate {
    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<AutoDepositUpdateModuleData>>) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(AutoDepositUpdate { update, data, deposit_on_frame: 0, award_initial_capture_bonus: false, initialized: false })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getAutoDepositUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<AutoDepositUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's.
    pub fn state_words(&self) -> Vec<u32> {
        vec![self.deposit_on_frame, u32::from(self.award_initial_capture_bonus), u32::from(self.initialized)]
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

    /// `getUpgradedSupplyBoost()`: the boost of the first pair whose upgrade the controlling player has completed (see the module docs for the `static`).
    pub fn upgraded_supply_boost(&self, world: &mut World, me: ObjectId) -> i32 {
        let Some(player) = world.get_controlling_player(me) else { return 0 };
        for info in &self.data.upgrade_boost {
            // the function-local `static`, initialised the first time control reaches it
            if world.cuts.auto_deposit_upgrade_static.is_none() || crate::logic::modules::plant(901) {
                let found = world.statics.upgrade_center.find_upgrade(&mut world.names, &info.type_name).ok().flatten().map(|t| t.upgrade_mask());
                world.cuts.auto_deposit_upgrade_static = Some(found);
            }
            if let Some(mask) = world.cuts.auto_deposit_upgrade_static.flatten() {
                if world.players.get_nth(player).is_some_and(|p| p.upgrades_completed.test_for_all(&mask)) {
                    return if crate::logic::modules::plant(902) { 0 } else { info.amount };
                }
            }
        }
        0
    }

    /// `awardInitialCaptureBonus(player)`: `player` as an index (a negative one is `NULL`).
    pub fn award_initial_capture_bonus(&mut self, world: &mut World, me: ObjectId, player: i32) -> Result<(), IniError> {
        self.deposit_on_frame = world.frame().wrapping_add(self.data.deposit_frame);
        if player < 0 || (!self.award_initial_capture_bonus && !crate::logic::modules::plant(903)) || self.data.initial_capture_bonus <= 0 {
            return Ok(());
        }
        let bonus = self.data.initial_capture_bonus;
        world.cut(CutCall::PlayerMoneyDeposit { player, amount: bonus as u32 });
        world.cut(CutCall::PlayerScoreAddMoneyEarned { player, amount: bonus });

        // Display cash income floating over the blacklotus
        if bonus > 0 {
            let mut position = world.objects.find(me).map(|o| o.get_position()).unwrap_or_default();
            position.z += 10.0; // add a little z to make it show up above the unit.
            let color = world.cuts.player_color(player) | floating_alpha();
            world.cut(CutCall::InGameUiAddFloatingText { amount: bonus, position, color });
        }
        self.award_initial_capture_bonus = false;
        Ok(())
    }

    /// The deposit and its floating text.
    fn pay(&mut self, world: &mut World, me: ObjectId) -> Result<UpdateSleepTime, IniError> {
        let now = world.frame();
        let modd = self.data.clone();
        // makes sure that buildings under construction do not get a bonus CCB
        if world.objects.find(me).is_some_and(|o| o.construction_percent != CONSTRUCTION_COMPLETE) && !crate::logic::modules::plant(904) {
            return Ok(UPDATE_SLEEP_NONE);
        }
        let _ = now;
        let money_amount = modd.deposit_amount.wrapping_add(self.upgraded_supply_boost(world, me));
        let player = world.get_controlling_player(me);
        if modd.is_actual_money {
            let player = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            world.cut(CutCall::PlayerMoneyDeposit { player, amount: money_amount as u32 });
            world.cut(CutCall::PlayerScoreAddMoneyEarned { player, amount: if crate::logic::modules::plant(905) { money_amount } else { modd.deposit_amount } });
        }

        let mut display_money = money_amount > 0;
        let (stealthed, detected) = world.objects.find(me).map_or((false, false), |o| (o.test_status(OBJECT_STATUS_STEALTHED), o.test_status(OBJECT_STATUS_DETECTED)));
        if stealthed {
            // OY LOOK!  I AM USING LOCAL PLAYER.
            if (!world.is_locally_controlled(me) && !detected) != crate::logic::modules::plant(906) {
                display_money = false;
            }
        }
        if display_money && world.objects.find(me).is_some() {
            let (mut position, is_structure, major, minor) = {
                let o = world.objects.find(me).expect("checked above");
                (o.get_position(), o.is_kind_of(KINDOF_STRUCTURE), o.geometry_info.get_major_radius(), o.geometry_info.get_minor_radius())
            };
            position.z += 10.0; // add a little z to make it show up above the unit.
            if is_structure {
                let width = major * 0.3;
                let depth = if crate::logic::modules::plant(907) { major } else { minor } * 0.3;
                let (wl, wh) = (real_to_int(-width)?, real_to_int(width)?);
                position.x += world.random.game_client_random_value(wl, wh) as f32;
                let (dl, dh) = (real_to_int(-depth)?, real_to_int(depth)?);
                position.y += world.random.game_client_random_value(dl, dh) as f32;
            }
            let player = player.ok_or_else(|| IniError::undefined(Undefined::NullControllingPlayer))?;
            let color = world.cuts.player_color(player) | floating_alpha();
            world.cut(CutCall::InGameUiAddFloatingText { amount: money_amount, position, color });
        }
        Ok(UPDATE_SLEEP_NONE)
    }
}

/// `GameMakeColor(0, 0, 0, 230)`.
fn floating_alpha() -> i32 {
    // (a << 24) | (r << 16) | (g << 8) | b, with r = g = b = 0
    (230u32 << 24) as i32
}

impl UpdateBehavior for AutoDepositUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let now = world.frame();
        if now >= self.deposit_on_frame {
            if !self.initialized {
                // Note - we have to set these in update, because during load the team is set, and we don't want to award initial bonus on load.
                self.award_initial_capture_bonus = true;
                self.initialized = true;
            }
            self.deposit_on_frame = now.wrapping_add(if crate::logic::modules::plant(910) { 0 } else { self.data.deposit_frame });
            if (world.is_neutral_controlled(me.object) && !crate::logic::modules::plant(908)) || self.data.deposit_amount < if crate::logic::modules::plant(909) { 0 } else { 1 } {
                return UPDATE_SLEEP_NONE;
            }
            return match self.pay(world, me.object) {
                Ok(t) => t,
                Err(e) => {
                    world.cuts.module_errors.push((me, e));
                    UPDATE_SLEEP_NONE
                }
            };
        }
        UPDATE_SLEEP_NONE
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        // `m_depositOnFrame = TheGameLogic->getFrame() + m_depositFrame`
        self.deposit_on_frame = world.frame().wrapping_add(self.data.deposit_frame);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        xfer.xfer_unsigned_int(&mut self.deposit_on_frame)?;
        xfer.xfer_bool(&mut self.award_initial_capture_bonus)?;
        if version > 1 {
            xfer.xfer_bool(&mut self.initialized)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::AutoDepositUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::AutoDepositUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::AwardInitialCaptureBonus { player } => {
                if let Err(e) = self.award_initial_capture_bonus(world, me.object, player) {
                    world.cuts.module_errors.push((me, e));
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for AutoDepositUpdate {
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

/// The factory's `NewModuleDataProc` for `AutoDepositUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::AutoDepositUpdate(Arc::new(AutoDepositUpdateModuleData::friend_new_module_data(ini)?)))
}

#[allow(dead_code)]
const _: Relationship = Relationship::Neutral;
