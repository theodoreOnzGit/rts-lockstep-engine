// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/ObjectHelper.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Helper/ObjectHelper.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `ObjectHelper` (the base of the object's helper modules): an `UpdateModule`
//! that is put to sleep forever by its constructor and woken by whoever needs
//! it, plus `sleepUntil`. The helpers that derive from it
//! ([`super::object_smc_helper`], [`super::object_repulsor_helper`],
//! [`super::object_weapon_status_helper`], [`super::status_damage_helper`],
//! [`super::temp_weapon_bonus_helper`]) call their object through
//! [`HelperHost`], the cut point to the world: every method is one upstream
//! call, made in upstream's order.

use std::sync::Arc;

use super::plant;
use crate::common::game_common::{FOREVER, NEVER};
use crate::common::ini::IniResult;
use crate::common::module::ObjectId;
use crate::common::object_status_types::ObjectStatusTypes;
use crate::common::xfer::{Snapshot, Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateModule, UpdateModuleData, UPDATE_SLEEP_FOREVER,
};

/// `TINT_STATUS_FRENZY` (`Drawable.h`).
pub const TINT_STATUS_FRENZY: i32 = 0x0000_0010;

/// The calls a helper makes on `TheGameLogic` and on its object.
pub trait HelperHost {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `TheGameLogic->friend_awakenUpdateModule(getObject(), this, frame)`
    /// (the call `setWakeFrame` ends in).
    fn awaken_update_module(&mut self, frame: u32);
    /// `getObject()->getStatusBits().test(OBJECT_STATUS_DESTROYED)`.
    fn is_destroyed(&self) -> bool;
    /// `getObject()->setStatus(MAKE_OBJECT_STATUS_MASK(status))`.
    fn set_status(&mut self, status: ObjectStatusTypes);
    /// `getObject()->clearStatus(MAKE_OBJECT_STATUS_MASK(status))`.
    fn clear_status(&mut self, status: ObjectStatusTypes);
    /// `getObject()->clearSpecialModelConditionStates()`.
    fn clear_special_model_condition_states(&mut self);
    /// `getObject()->adjustModelConditionForWeaponStatus()`.
    fn adjust_model_condition_for_weapon_status(&mut self);
    /// `getObject()->setWeaponBonusCondition(bonus)`.
    fn set_weapon_bonus_condition(&mut self, bonus: i32);
    /// `getObject()->clearWeaponBonusCondition(bonus)`.
    fn clear_weapon_bonus_condition(&mut self, bonus: i32);
    /// `getObject()->getDrawable() != NULL`.
    fn has_drawable(&self) -> bool;
    /// `getObject()->getDrawable()->setTintStatus(tint)`.
    fn set_tint_status(&mut self, tint: i32);
    /// `getObject()->getDrawable()->clearTintStatus(tint)`.
    fn clear_tint_status(&mut self, tint: i32);
}

/// `class ObjectHelper : public UpdateModule`.
#[derive(Clone, Debug)]
pub struct ObjectHelper {
    update: UpdateModule,
}

impl ObjectHelper {
    /// `ObjectHelper(thing, modData)`: the `UpdateModule`, then
    /// `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`. `phase` is the
    /// class's `getUpdatePhase()`.
    pub fn new(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
        phase: SleepyUpdatePhase,
        host: &mut impl HelperHost,
    ) -> IniResult<Self> {
        let mut me = ObjectHelper::unconstructed(object, module_data, phase)?;
        me.construct(host);
        Ok(me)
    }

    /// The first half of the constructor, which touches no world: the `UpdateModule`.
    pub fn unconstructed(
        object: ObjectId,
        module_data: Option<Arc<UpdateModuleData>>,
        phase: SleepyUpdatePhase,
    ) -> IniResult<Self> {
        Ok(ObjectHelper {
            update: UpdateModule::new(object, module_data, phase)?,
        })
    }

    /// The second half: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`. The world runs it once the module is part of its object.
    pub fn construct(&mut self, host: &mut impl HelperHost) {
        if !plant(32) {
            host.awaken_update_module(UpdateModule::set_wake_frame(
                host.frame(),
                UPDATE_SLEEP_FOREVER,
            ));
        }
    }

    /// The `UpdateModule` base.
    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    // --- p1w10-sonnet ---
    /// `ObjectHelper::xfer` on the scheduling half the slot holds (the live `UpdateModule`): the version, that half.
    pub fn xfer_with(update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        update.xfer(xfer)
    }
    // --- end p1w10-sonnet ---

    /// The `UpdateModule` base, for the scheduler's bookkeeping.
    pub fn update_module_mut(&mut self) -> &mut UpdateModule {
        &mut self.update
    }

    /// `ObjectHelper::sleepUntil(when)`: nothing for a destroyed object;
    /// `NEVER` and `FOREVER` sleep forever (`setWakeFrame(NEVER)` would
    /// otherwise awaken at once, `NEVER` being 0); else a delay from now.
    pub fn sleep_until(&self, host: &mut impl HelperHost, when: u32) {
        if host.is_destroyed() && !plant(31) {
            return;
        }
        let wake_delay = if (when == NEVER && !plant(30)) || when == FOREVER {
            UPDATE_SLEEP_FOREVER
        } else {
            update_sleep(when.wrapping_sub(host.frame()))
        };
        host.awaken_update_module(UpdateModule::set_wake_frame(host.frame(), wake_delay));
    }
}

impl Snapshot for ObjectHelper {
    /// `ObjectHelper::crc`: `UpdateModule::crc`.
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.update.crc(xfer)
    }
    /// `ObjectHelper::xfer` (version 1): the version, `UpdateModule::xfer`.
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;
        self.update.xfer(xfer)
    }
    /// `ObjectHelper::loadPostProcess`: `UpdateModule::loadPostProcess`.
    fn load_post_process(&mut self) {
        self.update.load_post_process();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::object_status_types::{OBJECT_STATUS_AFLAME, OBJECT_STATUS_WET};
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::logic::module::update_module::{update_sleep, UPDATE_SLEEP_NONE};
    use crate::logic::modules::object_repulsor_helper::ObjectRepulsorHelper;
    use crate::logic::modules::object_smc_helper::ObjectSMCHelper;
    use crate::logic::modules::object_weapon_status_helper::ObjectWeaponStatusHelper;
    use crate::logic::modules::status_damage_helper::StatusDamageHelper;
    use crate::logic::modules::temp_weapon_bonus_helper::TempWeaponBonusHelper;

    /// A host that writes a line per call.
    #[derive(Default)]
    struct Host {
        frame: u32,
        destroyed: bool,
        drawable: bool,
        wakes: Vec<u32>,
        log: Vec<String>,
    }

    impl HelperHost for Host {
        fn frame(&self) -> u32 {
            self.frame
        }
        fn awaken_update_module(&mut self, frame: u32) {
            self.wakes.push(frame);
        }
        fn is_destroyed(&self) -> bool {
            self.destroyed
        }
        fn set_status(&mut self, s: ObjectStatusTypes) {
            self.log.push(format!("set {s}"));
        }
        fn clear_status(&mut self, s: ObjectStatusTypes) {
            self.log.push(format!("clear {s}"));
        }
        fn clear_special_model_condition_states(&mut self) {
            self.log.push("smc".into());
        }
        fn adjust_model_condition_for_weapon_status(&mut self) {
            self.log.push("adjust".into());
        }
        fn set_weapon_bonus_condition(&mut self, b: i32) {
            self.log.push(format!("bonus {b}"));
        }
        fn clear_weapon_bonus_condition(&mut self, b: i32) {
            self.log.push(format!("unbonus {b}"));
        }
        fn has_drawable(&self) -> bool {
            self.drawable
        }
        fn set_tint_status(&mut self, t: i32) {
            self.log.push(format!("tint {t}"));
        }
        fn clear_tint_status(&mut self, t: i32) {
            self.log.push(format!("untint {t}"));
        }
    }

    fn data() -> Option<Arc<UpdateModuleData>> {
        Some(Arc::new(UpdateModuleData::new()))
    }

    #[test]
    fn sleep_until_sleeps_forever_on_never_and_forever_and_not_at_all_when_destroyed() {
        let mut host = Host {
            frame: 100,
            ..Default::default()
        };
        let h = ObjectHelper::new(1, data(), SleepyUpdatePhase::Normal, &mut host).unwrap();
        assert_eq!(host.wakes, vec![100 + FOREVER]);
        host.wakes.clear();
        h.sleep_until(&mut host, NEVER);
        h.sleep_until(&mut host, FOREVER);
        h.sleep_until(&mut host, 130);
        assert_eq!(host.wakes, vec![100 + FOREVER, 100 + FOREVER, 130]);
        host.wakes.clear();
        host.destroyed = true;
        h.sleep_until(&mut host, 130);
        assert!(host.wakes.is_empty());
    }

    #[test]
    fn the_simple_helpers_call_their_object_and_sleep_as_upstream() {
        let mut host = Host::default();
        let mut smc = ObjectSMCHelper::new(1, data(), &mut host).unwrap();
        assert_eq!(smc.update(&mut host), UPDATE_SLEEP_FOREVER);
        let mut rep = ObjectRepulsorHelper::new(1, data(), &mut host).unwrap();
        assert_eq!(rep.update(&mut host), UPDATE_SLEEP_FOREVER);
        let before = host.wakes.len();
        let mut ws = ObjectWeaponStatusHelper::new(1, data(), &mut host).unwrap();
        assert_eq!(
            host.wakes.len(),
            before + 2,
            "the base's wake, then its own (active)"
        );
        assert_eq!(ws.update(&mut host), UPDATE_SLEEP_NONE);
        assert_eq!(host.log, vec!["smc", "clear 9", "adjust"]);
        assert_eq!(
            ws.helper().update_module().get_update_phase(),
            SleepyUpdatePhase::Final
        );
    }

    #[test]
    fn status_damage_times_a_status_and_a_save_round_trips() {
        let mut host = Host {
            frame: 10,
            ..Default::default()
        };
        let mut h = StatusDamageHelper::new(1, data(), &mut host).unwrap();
        h.do_status_damage(&mut host, OBJECT_STATUS_AFLAME, 12.9);
        h.do_status_damage(&mut host, OBJECT_STATUS_WET, 5.0);
        assert_eq!(
            host.log,
            vec![
                format!("set {OBJECT_STATUS_AFLAME}"),
                format!("clear {OBJECT_STATUS_AFLAME}"),
                format!("set {OBJECT_STATUS_WET}")
            ]
        );
        assert_eq!((h.status_to_heal, h.frame_to_heal), (OBJECT_STATUS_WET, 15));
        assert_eq!(host.wakes.last(), Some(&(10 + update_sleep(5).0)));
        assert_eq!(
            h.get_disabled_types_to_process().count(),
            crate::common::disabled_types::disabledmask_all().count()
        );
        let mut save = XferSave::new();
        save.open(b"t").unwrap();
        save.xfer_snapshot(&mut h).unwrap();
        save.close().unwrap();
        let mut other = StatusDamageHelper::new(1, data(), &mut host).unwrap();
        let mut load = XferLoad::new(save.bytes().to_vec());
        load.open(b"t").unwrap();
        load.xfer_snapshot(&mut other).unwrap();
        load.close().unwrap();
        assert_eq!(
            (other.status_to_heal, other.frame_to_heal),
            (OBJECT_STATUS_WET, 15)
        );
        assert_eq!(other.update(&mut host), UPDATE_SLEEP_FOREVER);
        assert_eq!(host.log.last(), Some(&format!("clear {OBJECT_STATUS_WET}")));
    }

    #[test]
    fn temp_weapon_bonus_tints_with_a_drawable_and_clears_when_woken() {
        let mut host = Host {
            frame: 50,
            drawable: true,
            ..Default::default()
        };
        let mut h = TempWeaponBonusHelper::new(1, data(), &mut host).unwrap();
        h.do_temp_weapon_bonus(&mut host, 1, 30);
        h.do_temp_weapon_bonus(&mut host, 2, 10);
        assert_eq!(
            host.log,
            vec![
                "bonus 1",
                "tint 16",
                "unbonus 1",
                "untint 16",
                "bonus 2",
                "tint 16"
            ]
        );
        assert_eq!(h.frame_to_remove, 60);
        host.log.clear();
        assert_eq!(h.update(&mut host), UPDATE_SLEEP_FOREVER);
        assert_eq!(host.log, vec!["unbonus 2", "untint 16"]);
        assert!(h.update(&mut host) == UPDATE_SLEEP_FOREVER && host.log.len() == 2);
    }
}
