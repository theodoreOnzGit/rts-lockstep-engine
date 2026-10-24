// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/SpyVisionUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/SpyVisionUpdate.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `SpyVisionUpdate`: lets the object's owner see every enemy unit of the kinds `SpyOnKindof` names, for a while
//! ([`SpyVisionUpdate::activate_spy_vision`], sent by `SpyVisionSpecialPower`), or all the time at an interval (`SelfPowered`), or from the
//! moment an upgrade arrives (`NeedsUpgrade`). It is an update module and an `UpgradeMux` (`MODULEINTERFACE_UPGRADE`), and it is told when the
//! object is deleted, captured or disabled.
//!
//! The switch is `Player::setUnitsVisionSpied(setting, kinds, byWhom)` for every player the owner considers an enemy of that player's default
//! team ([`CutCall::PlayerSetUnitsVisionSpied`]).
//!
//! # Upstream facts kept
//!
//! - `xfer` writes the base, the deactivate frame, the active flag and (version 2) the reset flag and the disabled-until frame, and NOT the
//!   `UpgradeMux`'s `m_upgradeExecuted` (a module that derives from `UpgradeModule` saves it; this one does not).
//! - `setDisabledUntilFrame` passes `m_disabledUntilFrame - now` (an `UnsignedInt` difference) to `setWakeFrame` as the sleep time; the sum
//!   `now + (frame - now)` wraps back to the frame.
//! - `onDisabledEdge(TRUE)` is `setDisabledUntilFrame(UINT_MAX)`.

use std::sync::Arc;

use crate::common::game_common::Relationship;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::kind_of::KindOfMaskType;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_UPGRADE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::module::upgrade_module::{UpgradeModuleData, UpgradeMux};
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::modules::upgrade_mux_user::{upgrade_mux_call, UpgradeMuxUser};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;
use crate::upgrade_module_data_field_parse;

/// `class SpyVisionUpdateModuleData : public UpdateModuleData`.
#[derive(Debug)]
pub struct SpyVisionUpdateModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_upgradeMuxData`, in an `UpgradeModuleData` (shared, with the mutable members).
    pub upgrade: Arc<UpgradeModuleData>,
    /// `m_needsUpgrade`.
    pub needs_upgrade: bool,
    /// `m_selfPowered`.
    pub self_powered: bool,
    /// `m_selfPoweredDuration` (frames).
    pub self_powered_duration: u32,
    /// `m_selfPoweredInterval` (frames).
    pub self_powered_interval: u32,
    /// `m_spyOnKindof`: every kind unless the INI says otherwise.
    pub spy_on_kindof: KindOfMaskType,
}

impl Default for SpyVisionUpdateModuleData {
    /// `SpyVisionUpdateModuleData()`: `m_spyOnKindof = KINDOFMASK_NONE; m_spyOnKindof.flip()`.
    fn default() -> Self {
        let mut spy_on_kindof = KindOfMaskType::new();
        spy_on_kindof.flip();
        SpyVisionUpdateModuleData {
            update: UpdateModuleData::new(),
            upgrade: Arc::new(UpgradeModuleData::new()),
            needs_upgrade: false,
            self_powered: false,
            self_powered_duration: 0,
            self_powered_interval: 0,
            spy_on_kindof,
        }
    }
}

/// `UpgradeMuxData::getFieldParse()`'s table, for this data class.
static UPGRADE_BASE_FIELD_PARSE: [FieldParse<SpyVisionUpdateModuleData>; 6] =
    upgrade_module_data_field_parse!(SpyVisionUpdateModuleData, upgrade);

/// `SpyVisionUpdateModuleData::buildFieldParse`'s own table.
static OWN_FIELD_PARSE: [FieldParse<SpyVisionUpdateModuleData>; 6] = [
    FieldParse::new(
        "NeedsUpgrade",
        |ini, d, _| {
            d.needs_upgrade = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SelfPowered",
        |ini, d, _| {
            d.self_powered = ini.parse_bool()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SelfPoweredDuration",
        |ini, d, _| {
            d.self_powered_duration = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SelfPoweredInterval",
        |ini, d, _| {
            d.self_powered_interval = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "SpyOnKindof",
        |ini, d, _| d.spy_on_kindof.parse_from_ini(ini),
        UserData::None,
    ),
    FieldParse::end(),
];

impl SpyVisionUpdateModuleData {
    /// `buildFieldParse(p)`: the behaviour base, this class's own fields, then the mux's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, SpyVisionUpdateModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&OWN_FIELD_PARSE)?;
        p.add(&UPGRADE_BASE_FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<SpyVisionUpdateModuleData> {
        let mut data = SpyVisionUpdateModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(&mut data, SpyVisionUpdateModuleData::build_field_parse)?;
        }
        Ok(data)
    }
}

/// `class SpyVisionUpdate : public UpdateModule, public UpgradeMux`.
#[derive(Clone, Debug)]
pub struct SpyVisionUpdate {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getSpyVisionUpdateModuleData()`.
    data: Arc<SpyVisionUpdateModuleData>,
    /// The `UpgradeMux` base.
    mux: UpgradeMux,
    /// `m_deactivateFrame`.
    deactivate_frame: u32,
    /// `m_currentlyActive`.
    currently_active: bool,
    /// `m_resetTimersNextUpdate`.
    reset_timers_next_update: bool,
    /// `m_disabledUntilFrame`.
    disabled_until_frame: u32,
}

impl PartialEq for SpyVisionUpdate {
    /// The same object, the same data object (upstream compares pointers) and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
            && self.mux == other.mux
            && self.deactivate_frame == other.deactivate_frame
            && self.currently_active == other.currently_active
            && self.reset_timers_next_update == other.reset_timers_next_update
            && self.disabled_until_frame == other.disabled_until_frame
    }
}

impl SpyVisionUpdate {
    /// `SpyVisionUpdate::getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_UPGRADE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_UPGRADE
    }

    /// The first half of the constructor, which touches no world: the base, the data (`INI_INVALID_DATA` for null data) and the zeroed state.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<SpyVisionUpdateModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(SpyVisionUpdate {
            update,
            data,
            mux: UpgradeMux::new(),
            deactivate_frame: 0,
            currently_active: false,
            reset_timers_next_update: false,
            disabled_until_frame: 0,
        })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getSpyVisionUpdateModuleData()`).
    pub fn module_data(&self) -> &Arc<SpyVisionUpdateModuleData> {
        &self.data
    }

    /// The module's own state as words, for the tests that compare it with upstream's: the deactivate frame, whether it is active, the reset flag,
    /// the disabled-until frame, and whether the upgrade ran.
    pub fn state_words(&self) -> Vec<u32> {
        vec![
            self.deactivate_frame,
            u32::from(self.currently_active),
            u32::from(self.reset_timers_next_update),
            self.disabled_until_frame,
            u32::from(self.mux.is_already_upgraded()),
        ]
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
    pub fn module_call(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
        call: ModuleCall,
    ) -> ModuleReply {
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

    /// `activateSpyVision(duration)`: on now, off `duration` frames from now (never for 0).
    pub fn activate_spy_vision(&mut self, world: &mut World, me: ModuleHandle, duration: u32) {
        let now = world.frame();
        self.deactivate_frame = if duration == 0 && !crate::logic::modules::plant(861) {
            u32::MAX
        } else {
            now.wrapping_add(duration)
        };
        let player = world.get_controlling_player(me.object);
        self.do_activation_work(world, player, true);
        update_behavior::set_wake_frame(
            world,
            if duration == 0 {
                UPDATE_SLEEP_FOREVER
            } else {
                update_sleep(duration)
            },
        );
    }

    /// `setDisabledUntilFrame(frame)`.
    pub fn set_disabled_until_frame(&mut self, world: &mut World, me: ModuleHandle, frame: u32) {
        let now = world.frame();
        if frame > now || (frame == now && crate::logic::modules::plant(863)) {
            // Turn off the spy vision now since we are disabled
            if self.currently_active {
                let player = world.get_controlling_player(me.object);
                self.do_activation_work(world, player, false);
            }
            // When should we wake up?
            self.disabled_until_frame = frame;
            // Mark it so we can turn it on again on next update.
            self.reset_timers_next_update = true;
            // Now sleep until the disabled has expired.
            update_behavior::set_wake_frame(
                world,
                update_sleep(self.disabled_until_frame.wrapping_sub(now)),
            );
        } else {
            // Else it is a wakeup message.  Update does the turning on.
            self.disabled_until_frame = now;
            self.reset_timers_next_update = true;
            update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
        }
    }

    /// `onCapture(oldOwner, newOwner)`, the owners as player indices (a negative one is `NULL`).
    pub fn on_capture(&mut self, world: &mut World, old_owner: i32, new_owner: i32) {
        if self.currently_active {
            // If on, flick the switch real fast to get everything to update for the new owner
            if !crate::logic::modules::plant(864) {
                self.do_activation_work(world, (old_owner >= 0).then_some(old_owner), false);
            }
            self.do_activation_work(world, (new_owner >= 0).then_some(new_owner), true);
        }
    }

    /// `onDisabledEdge(nowDisabled)`.
    pub fn on_disabled_edge(&mut self, world: &mut World, me: ModuleHandle, now_disabled: bool) {
        self.set_disabled_until_frame(world, me, if now_disabled { u32::MAX } else { 0 });
    }

    /// `onDelete()`: "if I was left on at the time of death, then turn me off".
    pub fn on_delete(&mut self, world: &mut World, me: ModuleHandle) {
        if self.currently_active && !crate::logic::modules::plant(867) {
            let player = world.get_controlling_player(me.object);
            self.do_activation_work(world, player, false);
        }
    }

    /// `doActivationWork(playerToSetFor, setting)`: the switch for every player `playerToSetFor` considers an enemy of that player's default team.
    fn do_activation_work(
        &mut self,
        world: &mut World,
        player_to_set_for: Option<i32>,
        setting: bool,
    ) {
        let Some(by_whom) = player_to_set_for else {
            return;
        };
        for i in 0..world.players.player_count() {
            let team = world.players.get_nth(i).and_then(|p| p.default_team);
            let related = world.cuts.player_relationship_to_team(by_whom, team);
            if related == Relationship::Enemies
                || (related == Relationship::Neutral && crate::logic::modules::plant(865))
            {
                let which_units = self.data.spy_on_kindof;
                world.cut(CutCall::PlayerSetUnitsVisionSpied {
                    player: i,
                    setting,
                    which_units,
                    by_whom,
                });
            }
        }
        self.currently_active = setting;
    }
}

impl UpgradeMuxUser for SpyVisionUpdate {
    fn mux(&self) -> &UpgradeMux {
        &self.mux
    }
    fn mux_mut(&mut self) -> &mut UpgradeMux {
        &mut self.mux
    }
    fn mux_data(&self) -> &Arc<UpgradeModuleData> {
        &self.data.upgrade
    }
    /// `upgradeImplementation()`.
    fn upgrade_implementation(
        &mut self,
        world: &mut World,
        me: ModuleHandle,
    ) -> Result<(), IniError> {
        let data = self.data.clone();
        if (data.needs_upgrade || crate::logic::modules::plant(868))
            && !self.mux.is_already_upgraded()
        {
            // If zero, will turn on permanently.  And it does the wake up setting
            self.activate_spy_vision(world, me, data.self_powered_duration);
        }
        Ok(())
    }
    fn mux_into_body(self) -> UpdateBody {
        UpdateBody::SpyVisionUpdate(self)
    }
    fn mux_from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpyVisionUpdate(m) => Some(m),
            _ => None,
        }
    }
}

impl UpdateBehavior for SpyVisionUpdate {
    const XFER_VERSION: XferVersion = 2;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let data = self.data.clone();
        let now = world.frame();

        // Once disable has complete and we are waking up from it, then reset the interval so it has to wait before going on again. If it
        // doesn't have an interval, we turn it on immediately, but only for self powered objects.
        if self.reset_timers_next_update {
            self.reset_timers_next_update = false;
            if data.self_powered {
                if data.self_powered_interval == 0 && !crate::logic::modules::plant(866) {
                    // It's an always on self-powered special. So turn it back on now.
                    let player = world.get_controlling_player(me.object);
                    self.do_activation_work(world, player, true);
                    return UPDATE_SLEEP_FOREVER;
                }
                // Reset the interval timer via sleeping (so we have to wait a longer time before going on)
                return update_sleep(data.self_powered_interval);
            }
        }

        if self.currently_active
            && (self.deactivate_frame < world.frame()
                || (self.deactivate_frame == world.frame() && !crate::logic::modules::plant(862)))
        {
            // Turn off SpyVision.
            let player = world.get_controlling_player(me.object);
            self.do_activation_work(world, player, false);
            self.deactivate_frame = 0;
        } else if !self.currently_active && data.self_powered {
            let player = world.get_controlling_player(me.object);
            self.do_activation_work(world, player, true);
            self.deactivate_frame = if data.self_powered_duration == 0 {
                u32::MAX
            } else {
                now.wrapping_add(if crate::logic::modules::plant(869) {
                    data.self_powered_interval
                } else {
                    data.self_powered_duration
                })
            };
        }

        if data.self_powered {
            if self.currently_active {
                return update_sleep(data.self_powered_duration);
            }
            return update_sleep(data.self_powered_interval);
        }
        UPDATE_SLEEP_FOREVER
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        // the constructor's `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_FOREVER);
    }
    fn xfer_fields(&mut self, xfer: &mut dyn Xfer, version: XferVersion) -> XferResult<()> {
        // (the mux's `m_upgradeExecuted` is not saved: see the module docs)
        xfer.xfer_unsigned_int(&mut self.deactivate_frame)?;
        xfer.xfer_bool(&mut self.currently_active)?;
        if version >= 2 && !crate::logic::modules::plant(870) {
            xfer.xfer_bool(&mut self.reset_timers_next_update)?;
            xfer.xfer_unsigned_int(&mut self.disabled_until_frame)?;
        }
        Ok(())
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::SpyVisionUpdate(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::SpyVisionUpdate(m) => Some(m),
            _ => None,
        }
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        if let Some(r) = upgrade_mux_call(self, world, me, call.clone()) {
            return r;
        }
        match call {
            ModuleCall::ActivateSpyVision { duration } => {
                self.activate_spy_vision(world, me, duration);
                ModuleReply::Done
            }
            ModuleCall::SetSpyVisionDisabledUntilFrame { frame } => {
                self.set_disabled_until_frame(world, me, frame);
                ModuleReply::Done
            }
            ModuleCall::OnDelete => {
                self.on_delete(world, me);
                ModuleReply::Done
            }
            ModuleCall::OnCapture {
                old_owner,
                new_owner,
            } => {
                self.on_capture(world, old_owner, new_owner);
                ModuleReply::Done
            }
            ModuleCall::OnDisabledEdge { becoming_disabled } => {
                self.on_disabled_edge(world, me, becoming_disabled);
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
}

impl Snapshot for SpyVisionUpdate {
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

/// The factory's `NewModuleDataProc` for `SpyVisionUpdate`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::SpyVisionUpdate(Arc::new(
        SpyVisionUpdateModuleData::friend_new_module_data(ini)?,
    )))
}
