// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/TechBuildingBehavior.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `TechBuildingBehavior`: the behaviour of a neutral tech building (an oil derrick, a hospital): while a playable player controls it, its drawable shows
//! `MODELCONDITION_CAPTURED` and it plays its pulse FX list at a fixed rate; when it dies it goes back to the neutral player's default team. It is an update
//! module, a die module and listens for `onCapture` (which wakes it).
//!
//! `FXList::doFXObj` is [`ObjectClientEvent::FxListDoFxObj`]; `Player::isPlayableSide` is `cuts.player_is_playable_side`; the neutral player is player 0, its
//! default team `cuts.player_default_teams`; `Object::setTeam` is [`World::set_team`], whose `onCapture` reaches this module.
//!
//! # Upstream facts kept
//!
//! - The constructor wakes the module at once (`setWakeFrame(UPDATE_SLEEP_NONE)`); `update()` sleeps for ever unless it plays the FX, and `onCapture` wakes it again.
//! - `onDie` clears `MODELCONDITION_CAPTURED` and changes the team; the team change tells every module `onCapture`, this one included, which wakes it.
//!
//! `p1w13-sonnet`, 2026-10-21.

use std::sync::Arc;

use crate::common::damage_fx::FxListName;
use crate::common::ini::{
    FieldParse, Ini, IniError, IniErrorKind, IniResult, MultiIniFieldParse, UserData,
};
use crate::common::model_state::MODELCONDITION_CAPTURED;
use crate::common::module::{ModuleDataKind, ObjectId, MODULEINTERFACE_DIE};
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::behavior_module::BehaviorModuleData;
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UpdateSleepTime,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::modules::crate_collide::CrateCollideModuleData;
use crate::logic::modules::update_behavior::{self, UpdateBehavior};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class TechBuildingBehaviorModuleData : public UpdateModuleData`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TechBuildingBehaviorModuleData {
    /// The `UpdateModuleData` base.
    pub update: UpdateModuleData,
    /// `m_pulseFX`: FXList to play when the building is owned and updated (by name; `None` is `NULL`).
    pub pulse_fx: FxListName,
    /// `m_pulseFXRate`: how frequently to play it.
    pub pulse_fx_rate: u32,
}

/// `TechBuildingBehaviorModuleData::buildFieldParse`'s own table.
static FIELD_PARSE: [FieldParse<TechBuildingBehaviorModuleData>; 3] = [
    FieldParse::new(
        "PulseFX",
        |ini, d, _| {
            d.pulse_fx = CrateCollideModuleData::parse_fx_list(ini)?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::new(
        "PulseFXRate",
        |ini, d, _| {
            d.pulse_fx_rate = ini.parse_duration_unsigned_int()?;
            Ok(())
        },
        UserData::None,
    ),
    FieldParse::end(),
];

impl TechBuildingBehaviorModuleData {
    /// `buildFieldParse(p)`: the update base, then this class's.
    pub fn build_field_parse(
        p: &mut MultiIniFieldParse<'_, TechBuildingBehaviorModuleData>,
    ) -> IniResult<()> {
        BehaviorModuleData::build_field_parse(p)?;
        p.add(&FIELD_PARSE)?;
        Ok(())
    }

    /// `friend_newModuleData(ini)`: the defaults, filled from `ini` if there is one.
    pub fn friend_new_module_data(
        ini: Option<&mut Ini<'_>>,
    ) -> IniResult<TechBuildingBehaviorModuleData> {
        let mut data = TechBuildingBehaviorModuleData::default();
        if let Some(ini) = ini {
            ini.init_from_ini_multi_proc(
                &mut data,
                TechBuildingBehaviorModuleData::build_field_parse,
            )?;
        }
        Ok(data)
    }
}

/// `class TechBuildingBehavior : public UpdateModule, public DieModuleInterface`.
#[derive(Clone, Debug)]
pub struct TechBuildingBehavior {
    /// The `UpdateModule` base (the scheduling half).
    update: UpdateModule,
    /// `getTechBuildingBehaviorModuleData()`.
    data: Arc<TechBuildingBehaviorModuleData>,
}

impl PartialEq for TechBuildingBehavior {
    /// The same object and the same data object (upstream compares pointers).
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object()
            && Arc::ptr_eq(&self.data, &other.data)
    }
}

impl TechBuildingBehavior {
    /// `getInterfaceMask()`: `UpdateModule::getInterfaceMask() | MODULEINTERFACE_DIE`.
    pub const fn get_interface_mask() -> i32 {
        UpdateModule::get_interface_mask() | MODULEINTERFACE_DIE
    }

    /// The first half of the constructor, which touches no world: the base and the data (`INI_INVALID_DATA` for null data).
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<TechBuildingBehaviorModuleData>>,
    ) -> IniResult<Self> {
        let data = module_data.ok_or_else(|| IniError::new(IniErrorKind::InvalidData))?;
        let base = Arc::new(data.update.clone());
        let update = UpdateModule::new(object, Some(base), SleepyUpdatePhase::Normal)?;
        Ok(TechBuildingBehavior { update, data })
    }

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The module data (`getTechBuildingBehaviorModuleData()`).
    pub fn module_data(&self) -> &Arc<TechBuildingBehaviorModuleData> {
        &self.data
    }

    /// `update()` as the scheduler runs it (see [`update_behavior::world_update`]).
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        update_behavior::world_update(self, world, me)
    }

    /// The constructor's second half, run by the world once the module is part of its object.
    pub fn world_construct(&mut self, world: &mut World, me: ModuleHandle) {
        update_behavior::world_construct(self, world, me);
    }

    /// The module's answer to one of `Object`'s calls: `onDie`, `onCapture`.
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
}

impl UpdateBehavior for TechBuildingBehavior {
    const XFER_VERSION: XferVersion = 1;
    fn update_base(&self) -> &UpdateModule {
        &self.update
    }
    fn update(&mut self, world: &mut World, me: ModuleHandle) -> UpdateSleepTime {
        let us = me.object;
        let d = self.data.clone();

        let player = world.get_controlling_player(us);
        let captured = if let Some(p) = player.filter(|&p| world.cuts.player_is_playable_side(p)) {
            let _ = p;
            world.set_model_condition_state(us, MODELCONDITION_CAPTURED);
            true
        } else {
            if !crate::logic::modules::plant(1191) {
                world.clear_model_condition_state(us, MODELCONDITION_CAPTURED);
            }
            false
        };

        if let (Some(fx), true, true) = (
            d.pulse_fx.as_ref(),
            d.pulse_fx_rate > 0 || crate::logic::modules::plant(1194),
            captured,
        ) {
            world.emit(ObjectClientEvent::FxListDoFxObj {
                fx: fx.clone(),
                object_id: us,
            });
            update_sleep(d.pulse_fx_rate)
        } else {
            UPDATE_SLEEP_FOREVER
        }
    }
    fn construct(&mut self, world: &mut World, _me: ModuleHandle) {
        update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
    }
    fn other_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::OnDie(_) => {
                // the team change tells every module `onCapture`, this one included: it is lent to its slot so that the call is delivered at once
                crate::logic::modules::lend_clone(
                    world,
                    me,
                    self,
                    UpdateBody::TechBuildingBehavior,
                    |b| match b {
                        UpdateBody::TechBuildingBehavior(m) => Some(m),
                        _ => None,
                    },
                    |w, _m| {
                        let us = me.object;
                        w.clear_model_condition_state(us, MODELCONDITION_CAPTURED);
                        let neutral =
                            w.cuts
                                .player_default_team(if crate::logic::modules::plant(1195) {
                                    1
                                } else {
                                    0
                                });
                        if !crate::logic::modules::plant(1192) {
                            w.set_team(us, Some(neutral));
                        }
                    },
                );
                ModuleReply::Done
            }
            ModuleCall::OnCapture { .. } => {
                if !crate::logic::modules::plant(1193) {
                    update_behavior::set_wake_frame(world, UPDATE_SLEEP_NONE);
                }
                ModuleReply::Done
            }
            _ => ModuleReply::Unhandled,
        }
    }
    fn into_body(self) -> UpdateBody {
        UpdateBody::TechBuildingBehavior(self)
    }
    fn from_body(body: UpdateBody) -> Option<Self> {
        match body {
            UpdateBody::TechBuildingBehavior(m) => Some(m),
            _ => None,
        }
    }
}

impl Snapshot for TechBuildingBehavior {
    /// `crc`: the base's.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `xfer`: the version, the base (no fields of its own).
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

/// The factory's `NewModuleDataProc` for `TechBuildingBehavior`.
pub fn new_module_data(ini: Option<&mut Ini<'_>>) -> IniResult<ModuleDataKind> {
    Ok(ModuleDataKind::TechBuildingBehavior(Arc::new(
        TechBuildingBehaviorModuleData::friend_new_module_data(ini)?,
    )))
}
