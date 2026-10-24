// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (the status, script-status and private-status bits, the disabled states,
//   the weapon-set, weapon-bonus and armor-set flags, selectability,
//   geometry, the contain bookkeeping, vision and shroud with the partition
//   maintenance they trigger, and the small queries)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `Object`'s state operations (see `object.rs` for the data). Every function
//! here is `Object::name(..)` as `World::name(id, ..)`; a missing object
//! makes it a no-op (upstream dereferences `this`).

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{
    DisabledMaskType, DisabledType, DISABLED_ANY, DISABLED_COUNT, DISABLED_EMP, DISABLED_HACKED, DISABLED_HELD, DISABLED_SCRIPT_DISABLED,
    DISABLED_SCRIPT_UNDERPOWERED, DISABLED_SUBDUED, DISABLED_UNDERPOWERED, DISABLED_UNMANNED,
};
use crate::common::game_common::Relationship;
use crate::common::game_common::{FOREVER, LOGICFRAMES_PER_SECOND, NEVER, PLAYERMASK_ALL};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::geometry::GeometryInfo;
use crate::common::kind_of::{
    KINDOF_ALWAYS_SELECTABLE, KINDOF_DRONE, KINDOF_IGNORED_IN_GUI, KINDOF_MINE, KINDOF_MOB_NEXUS, KINDOF_REVEAL_TO_ALL,
    KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::object_status_types::{
    ObjectStatusMaskType, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED, OBJECT_STATUS_MASKED, OBJECT_STATUS_REPULSOR,
    OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION, OBJECT_STATUS_UNSELECTABLE,
};
use crate::logic::object_script_status_bits::{OBJECT_STATUS_SCRIPT_DISABLED, OBJECT_STATUS_SCRIPT_UNPOWERED};
use crate::logic::weapon_set::WeaponTemplateSet;
use crate::logic::weapon_set_flags::WeaponSetFlags;
use crate::logic::weapon_types::WEAPONSET_CARBOMB;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::ModuleCall;
use crate::logic::world::object::{HelperKind, PRIVATE_CAPTURED, PRIVATE_EFFECTIVELY_DEAD, PRIVATE_OFF_MAP, PRIVATE_UNDETECTED_DEFECTOR};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{ModelConditionOp, ObjectClientEvent, ObjectSound};

/// `TINT_STATUS_DISABLED` (`Drawable.h`).
pub const TINT_STATUS_DISABLED: i32 = 0x01;
/// `TINT_STATUS_GAINING_SUBDUAL_DAMAGE`.
pub const TINT_STATUS_GAINING_SUBDUAL_DAMAGE: i32 = 0x08;

/// `TheWeaponSetTypeToModelConditionTypeMap` (`WeaponSet.h`), indexed by `WeaponSetType`.
pub const WEAPON_SET_TYPE_TO_MODEL_CONDITION: [i32; 17] = {
    use crate::common::model_state::*;
    [
        MODELCONDITION_WEAPONSET_VETERAN,
        MODELCONDITION_WEAPONSET_ELITE,
        MODELCONDITION_WEAPONSET_HERO,
        MODELCONDITION_WEAPONSET_PLAYER_UPGRADE,
        MODELCONDITION_WEAPONSET_CRATEUPGRADE_ONE,
        MODELCONDITION_WEAPONSET_CRATEUPGRADE_TWO,
        MODELCONDITION_INVALID,
        MODELCONDITION_INVALID,
        MODELCONDITION_INVALID,
        MODELCONDITION_RIDER1,
        MODELCONDITION_RIDER2,
        MODELCONDITION_RIDER3,
        MODELCONDITION_RIDER4,
        MODELCONDITION_RIDER5,
        MODELCONDITION_RIDER6,
        MODELCONDITION_RIDER7,
        MODELCONDITION_RIDER8,
    ]
};

impl World {
    // ---- status ---------------------------------------------------------------------

    /// `getStatusBits()`.
    pub fn get_status_bits(&self, id: ObjectId) -> ObjectStatusMaskType {
        self.objects.find(id).map_or(ObjectStatusMaskType::new(), |o| o.status)
    }

    /// `setStatus(objectStatus, set)`: sets or clears the bits, and on a
    /// change: arms the repulsor helper (two seconds) when the repulsor bit
    /// comes on; relooks when a stealth bit changes on an object that reveals
    /// itself to all; and when the under-construction bit changes, detonates
    /// the enemy mines and destroys the friendly mines the object is on
    /// (`iteratePotentialCollisions`, a cut point) and dirties the object's
    /// partition data.
    pub fn set_status(&mut self, id: ObjectId, object_status: ObjectStatusMaskType, set: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let old_status = o.status;
        if set {
            o.status.set_mask(&object_status);
        } else {
            o.status.clear_mask(&object_status);
        }
        let new_status = o.status;
        if new_status == old_status {
            return;
        }
        if set && object_status.test(OBJECT_STATUS_REPULSOR) && self.objects.find(id).is_some_and(|o| o.helpers.repulsor.is_some()) {
            // Damaged repulsable civilians scare (repulse) other civs, but only for a short amount of time...
            let when = self.frame().wrapping_add(2 * LOGICFRAMES_PER_SECOND as u32);
            self.helper_sleep_until(id, HelperKind::Repulsor, when);
        }
        if object_status.test(OBJECT_STATUS_STEALTHED)
            || object_status.test(OBJECT_STATUS_DETECTED)
            || object_status.test(OBJECT_STATUS_DISGUISED)
        {
            // When any of the three key status bits for stealth go on or off, then handle partition updates for vision.
            if self.objects.find(id).is_some_and(|o| o.template().get_shroud_reveal_to_all_range() > 0.0) {
                self.handle_partition_cell_maintenance(id);
            }
        }
        // when an object's construction status changes, it needs to have its partition data updated,
        // in order to maintain the shroud correctly.
        if new_status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) != old_status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            // CHECK FOR MINES, AND DETONATE THEM NOW
            for them in self.cut_partition_potential_collisions(id) {
                if !self.objects.find(them).is_some_and(|t| t.is_kind_of(KINDOF_MINE)) {
                    continue;
                }
                // DETONATE ANY ENEMY MINES, OR DELETE FRIENDLY ONES
                if self.get_relationship(id, them) == Relationship::Enemies {
                    self.kill(them, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
                // detonate mine
                } else {
                    self.destroy_object(them);
                }
            }
            if self.objects.find(id).is_some_and(|o| o.in_partition) {
                self.cut(CutCall::PartitionMakeDirty { object: id });
            }
        }
    }

    /// `clearStatus(mask)`.
    pub fn clear_status(&mut self, id: ObjectId, object_status: ObjectStatusMaskType) {
        self.set_status(id, object_status, false);
    }

    /// `setScriptStatus(bit, set)`: the status bits a script sets. Disabled
    /// and unpowered also set or clear the matching `DISABLED_SCRIPT_*` and
    /// dirty the partition data (they change how far the object sees).
    pub fn set_script_status(&mut self, id: ObjectId, bit: u8, set: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let old_script_status = o.script_status;
        if set {
            o.script_status |= bit;
        } else {
            o.script_status &= !bit;
        }
        let new = o.script_status;
        if new == old_script_status {
            return;
        }
        for (flag, disabled_type) in
            [(OBJECT_STATUS_SCRIPT_DISABLED, DISABLED_SCRIPT_DISABLED), (OBJECT_STATUS_SCRIPT_UNPOWERED, DISABLED_SCRIPT_UNDERPOWERED)]
        {
            if (new & flag) != (old_script_status & flag) {
                if self.objects.find(id).is_some_and(|o| o.in_partition) {
                    // if an object becomes disabled or unpowered, then you have to update its partition data because it will
                    // change how far it can see.
                    self.cut(CutCall::PartitionMakeDirty { object: id });
                }
                if new & flag != 0 {
                    self.set_disabled(id, disabled_type);
                } else {
                    self.clear_disabled(id, disabled_type);
                }
            }
        }
    }

    /// `clearScriptStatus(bit)`.
    pub fn clear_script_status(&mut self, id: ObjectId, bit: u8) {
        self.set_script_status(id, bit, false);
    }

    /// `setEffectivelyDead(dead)`; a dead object leaves the radar.
    pub fn set_effectively_dead(&mut self, id: ObjectId, dead: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if dead {
            o.private_status |= PRIVATE_EFFECTIVELY_DEAD;
        } else {
            o.private_status &= !PRIVATE_EFFECTIVELY_DEAD;
        }
        if dead && self.objects.find(id).is_some_and(|o| o.on_radar) {
            self.radar_remove_object(id);
        }
    }

    /// `setCaptured(isCaptured)`: clearing it "should never happen".
    pub fn set_captured(&mut self, id: ObjectId, is_captured: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if is_captured {
            o.private_status |= PRIVATE_CAPTURED;
        } else {
            o.private_status &= !PRIVATE_CAPTURED;
        }
    }

    /// `friend_setUndetectedDefector(status)`.
    pub fn friend_set_undetected_defector(&mut self, id: ObjectId, status: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if status {
            o.private_status |= PRIVATE_UNDETECTED_DEFECTOR;
        } else {
            o.private_status &= !PRIVATE_UNDETECTED_DEFECTOR;
        }
    }

    /// Sets or clears `OFF_MAP` (the end of `reactToTransformChange` and
    /// `friend_notifyOfNewMapBoundary`).
    pub(crate) fn set_off_map(&mut self, id: ObjectId, off_map: bool) {
        if let Some(o) = self.objects.find_mut(id) {
            if off_map {
                o.private_status |= PRIVATE_OFF_MAP;
            } else {
                o.private_status &= !PRIVATE_OFF_MAP;
            }
        }
    }

    // ---- disabled -------------------------------------------------------------------------

    /// `setDisabled(type)`: disabled forever.
    pub fn set_disabled(&mut self, id: ObjectId, t: DisabledType) {
        self.set_disabled_until(id, t, FOREVER);
    }

    /// `setDisabledUntil(type, frame)`.
    ///
    /// Plays the "building/vehicle disabled" sounds when the first
    /// power-type disabledness arrives (and the sniped-pilot splatter),
    /// pauses the special-power countdowns on the edge (never for `HELD`),
    /// tints the drawable, passes the disabledness to a rider and to the
    /// slaves of an object whose spawns are its weapons, and finally handles
    /// a sniped car bomb (it blows up) or a sniped vehicle (it loses its
    /// veterancy and its healing bonus). `onDisabledEdge(true)` runs last if
    /// the object was not disabled before.
    pub fn set_disabled_until(&mut self, id: ObjectId, t: DisabledType, frame: u32) {
        let Some(o) = self.objects.find(id) else { return };
        let edge_case = !o.is_disabled();
        if !(0..DISABLED_COUNT).contains(&t) {
            return; // "Invalid disabled type value specified -- doesn't not exist!"
        }
        let position = o.get_position();
        let is_drone = o.is_kind_of(KINDOF_DRONE);
        // Handle audio events!
        if t == DISABLED_UNMANNED && !is_drone {
            // We've been sniped! Play a splatter sound for the pilot losing his face.
            self.emit(ObjectClientEvent::Sound {
                sound: ObjectSound::SplatterVehiclePilotsBrain,
                object_id: INVALID_ID,
                position: Some(position),
            });
        } else if t == DISABLED_UNDERPOWERED || t == DISABLED_EMP || t == DISABLED_SUBDUED || t == DISABLED_HACKED {
            // We've lost power -- make sure we aren't already out of power as the sounds shouldn't happen
            // if you were already disabled.
            let o = self.objects.find(id).expect("checked above");
            if !o.is_disabled_by_type(DISABLED_UNDERPOWERED)
                && !o.is_disabled_by_type(DISABLED_EMP)
                && !o.is_disabled_by_type(DISABLED_SUBDUED)
                && !o.is_disabled_by_type(DISABLED_HACKED)
            {
                let sound = if o.is_kind_of(KINDOF_STRUCTURE) {
                    Some(ObjectSound::BuildingDisabled)
                } else if o.is_kind_of(KINDOF_VEHICLE) {
                    Some(ObjectSound::VehicleDisabled)
                } else {
                    None
                };
                if let Some(sound) = sound {
                    self.emit(ObjectClientEvent::Sound { sound, object_id: INVALID_ID, position: Some(position) });
                }
            }
        }

        let o = self.objects.find(id).expect("checked above");
        if o.disabled_till_frame[t as usize] != frame {
            // an edge-test for disabledness, for type. This INCREMENTS m_pauseCount
            // srj sez: HELD nevers disables special powers.
            if t != DISABLED_HELD && !o.is_disabled_by_type(t) {
                self.pause_all_special_powers(id, true);
            }
            let now = self.frame();
            let o = self.objects.find_mut(id).expect("checked above");
            o.disabled_till_frame[t as usize] = frame;
            o.disabled.set(t, i32::from(frame > now));

            if o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID && o.is_disabled() {
                // Held does not tint anybody. If we are multiply disabled, the other setting will hit the tint,
                // and in clear, only-held and not-disabled are both causes to untint.
                // Also shouldn't be tinting when disabled by scripting, or by being unmanned.
                if t != DISABLED_HELD && t != DISABLED_SCRIPT_DISABLED && t != DISABLED_UNMANNED {
                    self.emit(ObjectClientEvent::DrawableTint { object_id: id, status: TINT_STATUS_DISABLED, on: true });
                }
            }

            if let Some(contain) = self.get_contain(id) {
                let rider = self.module_call(id, contain, ModuleCall::GetRider).as_id();
                if rider != INVALID_ID {
                    self.set_disabled_until(rider, t, frame);
                }
            }

            if self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS)) {
                if let Some(sbi) = self.get_spawn_behavior_interface(id) {
                    // Kris: Patch 1.01 - November 12, 2003: we want to disable the slaves, not order them to go idle!
                    self.module_call(id, sbi, ModuleCall::OrderSlavesDisabledUntil { disabled_type: t, frame });
                }
            }
        }

        if t == DISABLED_UNMANNED && !is_drone {
            // strange but true: If I am a carbomb, my driver actually has a dead-man's trigger for my dynamite...
            // If he gets sniped, I blow up! Wheeee!
            let mut flags = WeaponSetFlags::new();
            flags.set(WEAPONSET_CARBOMB, 1);
            let is_carbomb = {
                let o = self.objects.find(id).expect("checked above");
                let sets = o.template().get_weapon_template_sets();
                crate::common::sparse_match_finder::SparseMatchFinder::find_best_info_slow::<_, WeaponTemplateSet>(sets, &flags)
                    .is_some_and(|i| sets[i].test_weapon_set_flag(WEAPONSET_CARBOMB))
            };
            if is_carbomb {
                let sniper_id = match self.get_body_module(id) {
                    Some(b) => self.module_call(id, b, ModuleCall::GetLastDamageSource).as_id(),
                    None => INVALID_ID,
                };
                if self.objects.find(sniper_id).is_some() {
                    self.score_the_kill(sniper_id, id);
                }
                self.kill(id, crate::logic::damage::DAMAGE_UNRESISTABLE, crate::logic::damage::DEATH_NORMAL);
            } else {
                // This vehicle's pilot has been sniped, so we want to clear the veterancy rating (if any)
                self.cut_experience_set_experience_and_level(id, 0, false);
                // Not only that, but it also loses any healing bonuses it may have earned in its prior life
                if let Some(m) = self.find_module(id, b"AutoHealBehavior") {
                    self.module_call(id, m, ModuleCall::UndoUpgrade);
                }
            }
        }

        // This will only be called if we were NOT disabled before coming into this function.
        if edge_case {
            self.on_disabled_edge(id, true);
        }
    }

    /// `getDisabledUntil(type)`: for `DISABLED_ANY` the latest frame of any
    /// type that is on; for one type its frame if it is on; else 0.
    pub fn get_disabled_until(&self, id: ObjectId, t: DisabledType) -> u32 {
        let Some(o) = self.objects.find(id) else { return 0 };
        if t == DISABLED_ANY {
            let mut highest_frame = 0;
            // Iterate through each disabled type and return the one with the highest frame.
            for i in 0..DISABLED_COUNT {
                if o.disabled.test(i) && o.disabled_till_frame[i as usize] > highest_frame {
                    highest_frame = o.disabled_till_frame[i as usize];
                }
            }
            highest_frame
        } else if (0..DISABLED_COUNT).contains(&t) && o.disabled.test(t) {
            // Specific query.
            o.disabled_till_frame[t as usize]
        } else {
            // Not disabled.
            0
        }
    }

    /// `clearDisabled(type)`: false if the object was not disabled by `type`.
    /// Plays the "re-enabled" sound when the last power-type disabledness
    /// goes, resumes the special-power countdowns (not for `HELD`), clears a
    /// rider disabled forever, clears the slaves of a spawner, tints, runs
    /// `checkDisabledStatus` ("in case we just edged") and, if nothing
    /// disables the object any more, `onDisabledEdge(false)`.
    pub fn clear_disabled(&mut self, id: ObjectId, t: DisabledType) -> bool {
        if !(0..DISABLED_COUNT).contains(&t) {
            return false; // "Invalid disabled type value specified -- doesn't not exist!"
        }
        let Some(o) = self.objects.find(id) else { return false };
        if !o.is_disabled_by_type(t) {
            return false;
        }
        if t == DISABLED_UNDERPOWERED || t == DISABLED_EMP || t == DISABLED_SUBDUED || t == DISABLED_HACKED {
            // We've regained power-- make sure we aren't still disabled by another type.
            if (!o.is_disabled_by_type(DISABLED_UNDERPOWERED) || t == DISABLED_UNDERPOWERED)
                && (!o.is_disabled_by_type(DISABLED_EMP) || t == DISABLED_EMP)
                && (!o.is_disabled_by_type(DISABLED_SUBDUED) || t == DISABLED_SUBDUED)
                && (!o.is_disabled_by_type(DISABLED_HACKED) || t == DISABLED_HACKED)
            {
                let position = o.get_position();
                let sound = if o.is_kind_of(KINDOF_STRUCTURE) {
                    Some(ObjectSound::BuildingReenabled)
                } else if o.is_kind_of(KINDOF_VEHICLE) {
                    Some(ObjectSound::VehicleReenabled)
                } else {
                    None
                };
                if let Some(sound) = sound {
                    self.emit(ObjectClientEvent::Sound { sound, object_id: INVALID_ID, position: Some(position) });
                }
            }
        }

        // an edge-test for disabledness, for type. This DECREMENTS m_pauseCount
        // srj sez: HELD nevers disables special powers.
        // (the object is disabled by `t`: checked above)
        if t != DISABLED_HELD {
            self.pause_all_special_powers(id, false);
        }

        if let Some(contain) = self.get_contain(id) {
            // We explicitly pass stuff in up in the set, so we need to turn it off if it is a forever type
            let rider = self.module_call(id, contain, ModuleCall::GetRider).as_id();
            let forever = self.objects.find(id).is_some_and(|o| o.disabled_till_frame[t as usize] == FOREVER);
            if rider != INVALID_ID && forever {
                self.clear_disabled(rider, t);
            }
        }

        if self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS)) {
            if let Some(sbi) = self.get_spawn_behavior_interface(id) {
                // Kris: Patch 1.02 - December 17, 2003: make sure slaves can recover from being disabled by subdual
                self.module_call(id, sbi, ModuleCall::OrderSlavesToClearDisabled { disabled_type: t });
            }
        }

        let o = self.objects.find_mut(id).expect("checked above");
        o.disabled_till_frame[t as usize] = NEVER;
        o.disabled.set(t, 0);

        let mut exceptions = DisabledMaskType::new();
        exceptions.set(DISABLED_HELD, 1);
        exceptions.set(DISABLED_SCRIPT_DISABLED, 1);
        exceptions.set(DISABLED_UNMANNED, 1);
        let mut my_flags_minus_exceptions = o.disabled;
        my_flags_minus_exceptions.clear_and_set(&exceptions, &DisabledMaskType::new());
        let drawable = o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID;

        // to clarify, if I am NOT disabled by anything other than DISABLED_HELD, DISABLED_SCRIPT_DISABLED or
        // DISABLED_UNMANNED, the tint goes.
        if my_flags_minus_exceptions.count() == 0 && drawable {
            self.emit(ObjectClientEvent::DrawableTint { object_id: id, status: TINT_STATUS_DISABLED, on: false });
        }

        self.check_disabled_status(id); // in case we just edged

        // if we're no longer disabled by anything, then call the edge function.
        if self.objects.find(id).is_some_and(|o| !o.is_disabled()) {
            self.on_disabled_edge(id, false);
        }
        true
    }

    /// `checkDisabledStatus()`: clears the disabled types whose time is up.
    pub fn check_disabled_status(&mut self, id: ObjectId) {
        let now = self.frame();
        for i in 0..DISABLED_COUNT {
            let Some(o) = self.objects.find(id) else { return };
            if o.is_disabled_by_type(i) && now >= o.disabled_till_frame[i as usize] {
                self.clear_disabled(id, i); // This will also DECREMENT m_pauseCount in all specialpowers
                if let Some(o) = self.objects.find_mut(id) {
                    o.disabled.set(i, 0);
                }
            }
        }
    }

    /// `onDisabledEdge(becomingDisabled)`: every module is told; a dozer
    /// cancels its task; the radar upgrade adds or removes the player's radar
    /// count; and a power producer adjusts the player's power (with the power
    /// plant upgrade's and the overcharge's bonus). The AI, radar and energy
    /// calls are cut points; the modules' state is asked through the modules.
    pub fn on_disabled_edge(&mut self, id: ObjectId, becoming_disabled: bool) {
        // rip through the behavior modules and call the onDisabledEdge for any modules that care
        self.call_all_modules(id, ModuleCall::OnDisabledEdge { becoming_disabled });

        if becoming_disabled && self.objects.find(id).is_some_and(|o| o.ai.is_some()) && self.cuts.dozer_ais.contains(&id) {
            // Have to say goodbye to the thing we might be building or repairing so someone else can do it.
            self.cut(CutCall::DozerCancelTask { object: id });
        }

        let controller = self.get_controlling_player(id);
        // can be called during game teardown, thus controller can be null
        if let Some(controller) = controller {
            //@todo jkmcd - Colin suggested we rewrite this to use the interface stuff.
            if let Some(m) = self.find_module(id, b"RadarUpgrade") {
                if self.module_call(id, m, ModuleCall::IsAlreadyUpgraded).as_bool() {
                    // Need to decrement the count here, because we own a radar upgrade
                    let disable_proof = self.module_call(id, m, ModuleCall::GetRadarDisableProof).as_bool();
                    if becoming_disabled {
                        self.cut(CutCall::PlayerRemoveRadar { player: controller, disable_proof });
                    } else {
                        self.cut(CutCall::PlayerAddRadar { player: controller, disable_proof });
                    }
                }
            }
        }

        // We will need to adjust power ... somehow ...
        let (mut power_to_adjust, energy_bonus) = match self.objects.find(id) {
            Some(o) => (o.template().get_energy_production(), o.template().get_energy_bonus()),
            None => return,
        };
        if power_to_adjust > 0 {
            // We can't affect something that consumes, or else we go low power which removes the consumption
            // which makes us not low power so we add the consumption so we go low power...
            // This check also guards the IsDisabled in friend_adjustPower above
            if let Some(m) = self.find_module(id, b"PowerPlantUpgrade") {
                if self.module_call(id, m, ModuleCall::IsAlreadyUpgraded).as_bool() {
                    power_to_adjust += energy_bonus;
                }
            }
            if let Some(m) = self.find_module(id, b"OverchargeBehavior") {
                if self.module_call(id, m, ModuleCall::IsOverchargeActive).as_bool() {
                    power_to_adjust += energy_bonus;
                }
            }
            // Now, adjust the power for the player.
            if let Some(controller) = controller {
                self.cut(CutCall::EnergyAdjustPower { player: controller, amount: power_to_adjust, add: !becoming_disabled });
            }
        }
    }

    // ---- weapon set, weapon bonus and armor set flags ----------------------------------------

    /// `setWeaponSetFlag(wst)`: the flag, the weapon set refresh (a cut
    /// point) and the drawable's model condition.
    pub fn set_weapon_set_flag(&mut self, id: ObjectId, wst: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.cur_weapon_set_flags.set(wst, 1);
        self.cut(CutCall::WeaponSetUpdate { object: id });
        self.drawable_model_condition_single(id, wst, true);
    }

    /// `clearWeaponSetFlag(wst)`.
    pub fn clear_weapon_set_flag(&mut self, id: ObjectId, wst: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.cur_weapon_set_flags.set(wst, 0);
        self.cut(CutCall::WeaponSetUpdate { object: id });
        self.drawable_model_condition_single(id, wst, false);
    }

    /// The drawable half of `set/clearWeaponSetFlag`:
    /// `m_drawable->set/clearModelConditionState(TheWeaponSetTypeToModelConditionTypeMap[wst])`.
    fn drawable_model_condition_single(&mut self, id: ObjectId, wst: i32, set: bool) {
        let Some(o) = self.objects.find(id) else { return };
        if o.drawable == crate::common::game_type::INVALID_DRAWABLE_ID {
            return;
        }
        let mc =
            WEAPON_SET_TYPE_TO_MODEL_CONDITION.get(wst as usize).copied().unwrap_or(crate::common::model_state::MODELCONDITION_INVALID);
        let list = vec![mc];
        let op = if set { ModelConditionOp::Set(list) } else { ModelConditionOp::Clear(list) };
        self.emit(ObjectClientEvent::DrawableModelCondition { object_id: id, op });
    }

    /// `setWeaponBonusCondition(wst)`: on a change the weapons are told (a cut point).
    pub fn set_weapon_bonus_condition(&mut self, id: ObjectId, wst: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let old = o.weapon_bonus_condition;
        o.weapon_bonus_condition |= 1u32.wrapping_shl(wst as u32);
        if old != o.weapon_bonus_condition {
            // Our weapon bonus just changed, so we need to immediately update our weapons
            self.cut(CutCall::WeaponSetOnWeaponBonusChange { object: id });
        }
    }

    /// `clearWeaponBonusCondition(wst)`.
    pub fn clear_weapon_bonus_condition(&mut self, id: ObjectId, wst: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let old = o.weapon_bonus_condition;
        o.weapon_bonus_condition &= !1u32.wrapping_shl(wst as u32);
        if old != o.weapon_bonus_condition {
            self.cut(CutCall::WeaponSetOnWeaponBonusChange { object: id });
        }
    }

    /// `setArmorSetFlag(ast)`: the body module's.
    pub fn set_armor_set_flag(&mut self, id: ObjectId, ast: i32) {
        if let Some(b) = self.get_body_module(id) {
            self.module_call(id, b, ModuleCall::SetArmorSetFlag(ast));
        }
    }

    /// `clearArmorSetFlag(ast)`.
    pub fn clear_armor_set_flag(&mut self, id: ObjectId, ast: i32) {
        if let Some(b) = self.get_body_module(id) {
            self.module_call(id, b, ModuleCall::ClearArmorSetFlag(ast));
        }
    }

    /// `testArmorSetFlag(ast)`.
    pub fn test_armor_set_flag(&mut self, id: ObjectId, ast: i32) -> bool {
        match self.get_body_module(id) {
            Some(b) => self.module_call(id, b, ModuleCall::TestArmorSetFlag(ast)).as_bool(),
            None => false,
        }
    }

    /// `maskObject(mask)`: mask or unmask. "When masking objects they become
    /// unselected ... we do this in any situation for any player cause you
    /// aren't allowed to select masked objects".
    pub fn mask_object(&mut self, id: ObjectId, mask: bool) {
        self.set_status(id, mask_of(OBJECT_STATUS_MASKED), mask);
        if mask {
            let player_mask = self.controlling_player_mask(id);
            self.cut(CutCall::GameLogicDeselectObject { object: id, player_mask: !player_mask, affect_client: true });
        }
    }

    /// `isUsingAirborneLocomotor()`: the AI's current locomotor flies.
    pub fn is_using_airborne_locomotor(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.ai.is_some()) && self.cuts.ai_airborne.contains(&id)
    }

    /// `getShroudedStatus(playerIndex)`: always clear for `KINDOF_ALWAYS_VISIBLE`;
    /// the partition data's otherwise; clear for an object not in the
    /// partition manager (a soldier garrisoned in a building).
    pub fn get_shrouded_status(&self, id: ObjectId, player_index: i32) -> crate::common::game_common::ObjectShroudStatus {
        use crate::common::game_common::ObjectShroudStatus;
        let Some(o) = self.objects.find(id) else { return ObjectShroudStatus::Clear };
        if o.template().is_kind_of(crate::common::kind_of::KINDOF_ALWAYS_VISIBLE) {
            return ObjectShroudStatus::Clear;
        }
        if o.in_partition {
            return self.cuts.shrouded_status.get(&(id, player_index)).copied().unwrap_or(ObjectShroudStatus::Clear);
        }
        // This can happen for objects removed from the partition system (e.g., for soldiers that are
        // garrisoned inside a building).
        ObjectShroudStatus::Clear
    }

    /// `reactToTurretChange(turret, oldRotation, oldPitch)`: a changed rotation
    /// tells the container.
    pub fn react_to_turret_change(&mut self, id: ObjectId, turret: i32, old_rotation: f32, _old_pitch: f32) {
        let mut current_rotation = 0.0f32;
        if self.objects.find(id).is_some_and(|o| o.ai.is_some()) {
            current_rotation = self.cuts.ai_turret_rotation(id, turret);
        }
        let rotation_change = current_rotation != old_rotation;
        if rotation_change {
            if let Some(c) = self.get_contain(id) {
                self.module_call(id, c, ModuleCall::ContainReactToTransformChange);
            }
        }
    }

    // ---- selectability -----------------------------------------------------------------------

    /// `setSelectable(selectable)`.
    pub fn set_selectable(&mut self, id: ObjectId, selectable: bool) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.is_selectable = selectable;
        if o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID {
            self.emit(ObjectClientEvent::DrawableSetSelectable { object_id: id, selectable });
        }
    }

    /// `isSelectable()`: `KINDOF_ALWAYS_SELECTABLE`, or selectable and not
    /// `UNSELECTABLE` and not effectively dead.
    pub fn is_selectable(&self, id: ObjectId) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        if o.template().is_kind_of(KINDOF_ALWAYS_SELECTABLE) {
            return true;
        }
        o.is_selectable && !o.test_status(OBJECT_STATUS_UNSELECTABLE) && !o.is_effectively_dead()
    }

    /// `isMassSelectable()`.
    pub fn is_mass_selectable(&self, id: ObjectId) -> bool {
        self.is_selectable(id) && !self.objects.find(id).is_some_and(|o| o.is_kind_of(KINDOF_STRUCTURE))
    }

    // ---- geometry ----------------------------------------------------------------------------

    /// `setGeometryInfo(geom)`: an object in the partition manager is
    /// unregistered and registered again so that its size is updated; the
    /// drawable is told.
    pub fn set_geometry_info(&mut self, id: ObjectId, geom: &GeometryInfo) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.geometry_info = geom.clone();
        if o.in_partition {
            // if our geometry changes, we unregister and re-register with the partitionmgr so that our size gets updated.
            self.partition_unregister_object(id);
            self.partition_register_object(id);
        }
        if self.objects.find(id).is_some_and(|o| o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID) {
            self.emit(ObjectClientEvent::DrawableReactToGeometryChange { object_id: id });
        }
    }

    /// `setGeometryInfoZ(newZ)`: "a Z change only does not need to un/register with the PartitionManager".
    pub fn set_geometry_info_z(&mut self, id: ObjectId, new_z: f32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.geometry_info.set_max_height_above_position(new_z);
        if o.drawable != crate::common::game_type::INVALID_DRAWABLE_ID {
            self.emit(ObjectClientEvent::DrawableReactToGeometryChange { object_id: id });
        }
    }

    /// `getHealthBoxPosition(pos)`.
    pub fn get_health_box_position(&self, id: ObjectId) -> Option<Coord3D> {
        let o = self.objects.find(id)?;
        let mut pos = o.get_position();
        pos.z += o.geometry_info.get_max_height_above_position() + 10.0;
        pos.x += o.health_box_offset.x;
        pos.y += o.health_box_offset.y;
        pos.z += o.health_box_offset.z;
        // this needs to get moved to the mobspawnerupdate
        if o.is_kind_of(KINDOF_MOB_NEXUS) {
            // quicker idiot test
            pos.z += 20.0; // dear God, I confess my kluge, and repent.
        }
        Some(pos)
    }

    /// `getHealthBoxDimensions(height, width)`: `None` for an object ignored
    /// in the GUI (upstream returns false with 0, 0), else `(height, width)`.
    pub fn get_health_box_dimensions(&self, id: ObjectId) -> Option<(f32, f32)> {
        let o = self.objects.find(id)?;
        if o.is_kind_of(KINDOF_IGNORED_IN_GUI) {
            return None;
        }
        // just add the major and minor axes
        let sum = o.geometry_info.get_major_radius() + o.geometry_info.get_minor_radius();
        // MAX(20.0f, MIN(150.0f, ..)) of the macros, whose NaN behaviour `clamp` does not have
        #[allow(clippy::manual_clamp)]
        let size = f32::max(20.0, f32::min(150.0, sum));
        let width = f32::max(20.0, size * 2.0);
        Some((3.0, width))
    }

    // ---- contain bookkeeping -----------------------------------------------------------------

    /// `onContainedBy(containedBy)`: this object is now contained.
    /// Unselectable; masked if the container hides its passengers; relooks.
    pub fn on_contained_by(&mut self, id: ObjectId, contained_by: ObjectId) {
        self.set_status(id, mask_of(OBJECT_STATUS_UNSELECTABLE), true);
        let enclosing = match self.get_contain(contained_by) {
            Some(c) if self.objects.find(contained_by).is_some() => {
                self.module_call(contained_by, c, ModuleCall::IsEnclosingContainerFor(id)).as_bool()
            }
            _ => false,
        };
        if enclosing {
            self.set_status(id, mask_of(OBJECT_STATUS_MASKED), true);
        } else {
            self.set_status(id, mask_of(OBJECT_STATUS_MASKED), false);
        }
        let now = self.frame();
        if let Some(o) = self.objects.find_mut(id) {
            o.contained_by = contained_by;
            o.contained_by_frame = now;
        }
        self.handle_partition_cell_maintenance(id); // which should unlook me now that I am contained
    }

    /// `onRemovedFrom(removedFrom)`: no longer contained.
    pub fn on_removed_from(&mut self, id: ObjectId) {
        let mut both = ObjectStatusMaskType::new();
        both.set(OBJECT_STATUS_MASKED, 1);
        both.set(OBJECT_STATUS_UNSELECTABLE, 1);
        self.set_status(id, both, false);
        if let Some(o) = self.objects.find_mut(id) {
            o.contained_by = INVALID_ID;
            o.contained_by_frame = 0;
        }
        self.handle_partition_cell_maintenance(id); // get a clean look, now that I am outdoors, again
    }

    /// `isContained()`.
    pub fn is_contained(&self, id: ObjectId) -> bool {
        self.objects.find(id).is_some_and(|o| o.contained_by != INVALID_ID)
    }

    /// `getTransportSlotCount()`: the template's raw count, or for a special
    /// zero-slot container the sum of what it holds.
    pub fn get_transport_slot_count(&mut self, id: ObjectId) -> i32 {
        let mut ring = false;
        self.transport_slot_count_at(id, 0, &mut ring)
    }

    /// `getTransportSlotCount()` `depth` containers down. A ring of special
    /// zero-slot containers (parachutes holding each other) recurses until the
    /// stack is gone upstream: past `MAX_RECURSION` the count is undefined
    /// ([`Undefined::EndlessLoop`](crate::common::ini::Undefined::EndlessLoop),
    /// recorded against the container) and 0, and the rest of the walk is not
    /// taken: a ring that holds an object twice branches at every level
    /// (p1w13-sonnet, batch R8).
    fn transport_slot_count_at(&mut self, id: ObjectId, depth: u32, ring: &mut bool) -> i32 {
        if *ring {
            return 0;
        }
        let Some(o) = self.objects.find(id) else { return 0 };
        let mut count = o.template().get_raw_transport_slot_count();
        if let Some(contain) = self.get_contain(id) {
            if self.module_call(id, contain, ModuleCall::IsSpecialZeroSlotContainer).as_bool() {
                count = 0;
                if depth >= crate::logic::modules::open_contain::MAX_RECURSION {
                    let me = crate::logic::world::module_call::ModuleHandle { object: id, index: contain };
                    self.cuts.module_errors.push((me, crate::common::ini::IniError::undefined(crate::common::ini::Undefined::EndlessLoop)));
                    *ring = true;
                    return 0;
                }
                if let crate::logic::world::module_call::ModuleReply::Ids(items) =
                    self.module_call(id, contain, ModuleCall::GetContainedItems)
                {
                    for item in items {
                        count += self.transport_slot_count_at(item, depth + 1, ring);
                    }
                }
            }
        }
        count
    }

    /// `isHero()`: a hero inside, or this is one.
    pub fn is_hero(&mut self, id: ObjectId) -> bool {
        if let Some(contain) = self.get_contain(id) {
            if self.module_call(id, contain, ModuleCall::IsHeroInside).as_bool() {
                return true;
            }
        }
        self.objects.find(id).is_some_and(|o| o.is_kind_of(crate::common::kind_of::KINDOF_HERO))
    }

    // ---- vision and shroud ---------------------------------------------------------------------

    /// `getVisionRange()`.
    pub fn get_vision_range(&self, id: ObjectId) -> f32 {
        self.objects.find(id).map_or(0.0, |o| o.vision_range)
    }

    /// `setVisionRange(r)`.
    pub fn set_vision_range(&mut self, id: ObjectId, r: f32) {
        if let Some(o) = self.objects.find_mut(id) {
            o.vision_range = r;
        }
    }

    /// `getShroudClearingRange()`: structures under construction "can only see
    /// themselves" (the bounding circle).
    pub fn get_shroud_clearing_range(&self, id: ObjectId) -> f32 {
        let Some(o) = self.objects.find(id) else { return 0.0 };
        if o.status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return o.geometry_info.get_bounding_circle_radius();
        }
        o.shroud_clearing_range
    }

    /// `setShroudClearingRange(r)`: a change relooks, except at the origin
    /// ("complete and total monkey hack fix": a new object has not been moved
    /// yet and would clear the shroud around the wrong place).
    pub fn set_shroud_clearing_range(&mut self, id: ObjectId, r: f32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if r != o.shroud_clearing_range {
            o.shroud_clearing_range = r;
            let pos = o.get_position();
            if pos.x != 0.0 || pos.y != 0.0 || pos.z != 0.0 {
                self.handle_partition_cell_maintenance(id);
            }
        }
    }

    /// `getShroudRange()`.
    pub fn get_shroud_range(&self, id: ObjectId) -> f32 {
        self.objects.find(id).map_or(0.0, |o| o.shroud_range)
    }

    /// `setShroudRange(r)`.
    pub fn set_shroud_range(&mut self, id: ObjectId, r: f32) {
        if let Some(o) = self.objects.find_mut(id) {
            o.shroud_range = r;
        }
    }

    /// `setVisionSpied(setting, byWhom)`: a reference count per spying
    /// player; the object relooks on the edge (0 to 1 and 1 to 0).
    pub fn set_vision_spied(&mut self, id: ObjectId, setting: bool, by_whom: i32) {
        let Some(o) = self.objects.find_mut(id) else { return };
        let Some(slot) = o.vision_spied_by.get_mut(by_whom as usize) else { return };
        let mut need_refresh = false; // If this setting is an edge trigger on the reference count, I need to refresh
        if setting {
            *slot += 1;
            if *slot == 1 {
                need_refresh = true;
            }
        } else {
            *slot -= 1;
            if *slot == 0 {
                need_refresh = true;
            }
        }
        if need_refresh {
            let mut working_mask: u16 = 0;
            for i in 0..crate::common::game_common::MAX_PLAYER_COUNT {
                if o.vision_spied_by[i as usize] > 0 {
                    working_mask |= 1 << i;
                } else {
                    working_mask &= !(1 << i);
                }
            }
            o.vision_spied_mask = working_mask;
            self.handle_partition_cell_maintenance(id);
        }
    }

    // ---- the partition maintenance of Object ---------------------------------------------------

    /// `onPartitionCellChange()`: "we have moved a 'significant' amount".
    pub fn on_partition_cell_change(&mut self, id: ObjectId) {
        self.handle_partition_cell_maintenance(id);
    }

    /// `handlePartitionCellMaintenance()`: undo and redo all shroud actions.
    pub fn handle_partition_cell_maintenance(&mut self, id: ObjectId) {
        self.handle_shroud(id);
        self.handle_value_map(id);
        self.handle_threat_map(id);
    }

    fn handle_shroud(&mut self, id: ObjectId) {
        self.unlook(id); // Undo last looking
        self.unshroud(id); // and shrouding
        self.shroud(id); // redo shrouding
        self.look(id); // Redo looking
    }

    fn handle_value_map(&mut self, id: ObjectId) {
        self.remove_value(id);
        self.add_value(id);
    }

    fn handle_threat_map(&mut self, id: ObjectId) {
        self.remove_threat(id);
        self.add_threat(id);
    }

    /// Whether the object may add value or threat to the maps: it has a
    /// player, and is neither under construction, effectively dead, nor
    /// blind.
    fn may_affect_maps(&self, id: ObjectId) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        if self.get_controlling_player(id).is_none() {
            return false;
        }
        !(o.status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.is_effectively_dead() || self.get_shroud_clearing_range(id) <= 0.0)
    }

    fn add_value(&mut self, id: ObjectId) {
        if !self.objects.find(id).is_some_and(|o| o.partition_last_value.is_invalid()) {
            return; // "An Object is adding value, but hasn't removed his previous value."
        }
        if !self.may_affect_maps(id) {
            return;
        }
        let player = self.get_controlling_player(id).expect("may_affect_maps checked it");
        let mask = self.cuts.player_mask(player);
        let range = self.get_vision_range(id); // we are valuable all the way to where we can target.
        let o = self.objects.find_mut(id).expect("checked above");
        o.partition_last_value.where_ = o.thing.position().to_owned();
        o.partition_last_value.data = o.template().friend_get_build_cost() as u32;
        o.partition_last_value.for_whom = mask;
        o.partition_last_value.how_far = range;
        let s = o.partition_last_value;
        self.cut(CutCall::PartitionDoValueAffect { x: s.where_.x, y: s.where_.y, how_far: s.how_far, data: s.data, for_whom: s.for_whom });
    }

    fn remove_value(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.partition_last_value.is_invalid() {
            // removing before adding is valid, cause we always remove before adding.
            return;
        }
        let s = o.partition_last_value;
        o.partition_last_value.reset();
        self.cut(CutCall::PartitionUndoValueAffect {
            x: s.where_.x,
            y: s.where_.y,
            how_far: s.how_far,
            data: s.data,
            for_whom: s.for_whom,
        });
    }

    fn add_threat(&mut self, id: ObjectId) {
        if !self.objects.find(id).is_some_and(|o| o.partition_last_threat.is_invalid()) {
            return; // "An Object is adding threat, but hasn't removed his previous threat."
        }
        if !self.may_affect_maps(id) {
            return;
        }
        let player = self.get_controlling_player(id).expect("may_affect_maps checked it");
        let mask = self.cuts.player_mask(player);
        let range = self.get_vision_range(id); // we are threatening all the way to where we can target.
        let o = self.objects.find_mut(id).expect("checked above");
        o.partition_last_threat.where_ = o.thing.position().to_owned();
        o.partition_last_threat.data = o.template().get_threat_value();
        o.partition_last_threat.for_whom = mask;
        o.partition_last_threat.how_far = range;
        let s = o.partition_last_threat;
        self.cut(CutCall::PartitionDoThreatAffect { x: s.where_.x, y: s.where_.y, how_far: s.how_far, data: s.data, for_whom: s.for_whom });
    }

    fn remove_threat(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.partition_last_threat.is_invalid() {
            return;
        }
        let s = o.partition_last_threat;
        o.partition_last_threat.reset();
        self.cut(CutCall::PartitionUndoThreatAffect {
            x: s.where_.x,
            y: s.where_.y,
            how_far: s.how_far,
            data: s.data,
            for_whom: s.for_whom,
        });
    }

    /// `look()`: reveal the shroud around the object for its allies (and the
    /// players spying through it), and, if its template says so, to everyone
    /// else at another range. Not while dead, destroyed, or inside a
    /// container that does not let its passengers see out.
    fn look(&mut self, id: ObjectId) {
        if !self.objects.find(id).is_some_and(|o| o.partition_last_look.is_invalid()) {
            return; // "An Object is looking, but hasn't unlooked the last one."
        }
        let Some(controller) = self.get_controlling_player(id) else { return };
        let Some(o) = self.objects.find(id) else { return };
        // Some things get Destroyed directly without hitting Death.
        if o.is_destroyed() || o.is_effectively_dead() {
            return;
        }
        let contained_by = o.contained_by;
        if contained_by != INVALID_ID {
            if let Some(c) = self.get_contain(contained_by) {
                if !self.module_call(contained_by, c, ModuleCall::IsGarrisonable).as_bool() {
                    return; // dont look, 'cause you are in a tunnel, now
                }
            }
        }
        let shroud_clearing_range = self.get_shroud_clearing_range(id);
        if shroud_clearing_range > 0.0 {
            let o = self.objects.find(id).expect("checked above");
            let mut looking_mask: u16;
            if o.is_kind_of(KINDOF_REVEAL_TO_ALL) {
                looking_mask = PLAYERMASK_ALL;
            } else {
                looking_mask = 0;
                let count = self.players.player_count();
                for current_index in (0..count).rev() {
                    // Build mask of of allies who can see me.
                    if self.cuts.player_relationship_to_player_default_team(controller, current_index) == Relationship::Allies {
                        looking_mask |= self.cuts.player_mask(current_index);
                    }
                }
                // Other players can also be looking through our eyes.
                looking_mask |= self.objects.find(id).expect("checked above").vision_spied_mask;
            }
            let pos = self.objects.find(id).expect("checked above").get_position();
            self.cut(CutCall::PartitionDoShroudReveal { x: pos.x, y: pos.y, range: shroud_clearing_range, mask: looking_mask });
            let o = self.objects.find_mut(id).expect("checked above");
            o.partition_last_look.where_ = pos;
            o.partition_last_look.for_whom = looking_mask;
            o.partition_last_look.how_far = shroud_clearing_range;
        }
        // Now reveal to everyone if we're special. Note this works differently than KINDOF_REVEAL_TO_ALL because
        // the kindof uses the same range as allies, spies, and owners would see.
        let o = self.objects.find(id).expect("checked above");
        let reveal_to_all_range = o.template().get_shroud_reveal_to_all_range();
        // And don't reveal while under construction. When finished, a refresh occurs, so don't worry.
        if reveal_to_all_range > 0.0 && !o.status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            // Only reveal to all when the unit is visible. If it's stealthed, we won't reveal it anymore.
            let stealthed_and_not_detected =
                o.status.test(OBJECT_STATUS_STEALTHED) && !o.status.test(OBJECT_STATUS_DETECTED) && !o.status.test(OBJECT_STATUS_DISGUISED);
            if !stealthed_and_not_detected {
                let pos = o.get_position();
                let players_mask = self.cut_players_with_relationship(controller, true, true);
                self.cut(CutCall::PartitionDoShroudReveal { x: pos.x, y: pos.y, range: reveal_to_all_range, mask: players_mask });
                let o = self.objects.find_mut(id).expect("checked above");
                o.partition_reveal_all_last_look.where_ = pos;
                o.partition_reveal_all_last_look.for_whom = players_mask;
                o.partition_reveal_all_last_look.how_far = reveal_to_all_range;
            }
        }
    }

    fn unlook(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.partition_last_look.is_invalid() {
            // Your very first action will be an unlook, so of course you haven't looked yet. This is not an error
            return;
        }
        let s = o.partition_last_look;
        o.partition_last_look.reset();
        self.cut(CutCall::PartitionQueueUndoShroudReveal { x: s.where_.x, y: s.where_.y, range: s.how_far, mask: s.for_whom });
        let o = self.objects.find_mut(id).expect("checked above");
        if !o.partition_reveal_all_last_look.is_invalid() {
            let r = o.partition_reveal_all_last_look;
            o.partition_reveal_all_last_look.reset();
            self.cut(CutCall::PartitionQueueUndoShroudReveal { x: r.where_.x, y: r.where_.y, range: r.how_far, mask: r.for_whom });
        }
    }

    fn shroud(&mut self, id: ObjectId) {
        if !self.objects.find(id).is_some_and(|o| o.partition_last_shroud.is_invalid()) {
            return; // "An Object is shrouding, but hasn't unshrouded the last one."
        }
        let Some(controller) = self.get_controlling_player(id) else { return };
        let o = self.objects.find(id).expect("checked above");
        // things under construction don't shroud, nor do dead or blind things
        #[allow(clippy::neg_cmp_op_on_partial_ord)] // `!(range > 0.0f)` of upstream, whose NaN answer is `true`
        if o.status.test(OBJECT_STATUS_UNDER_CONSTRUCTION) || o.is_effectively_dead() || !(self.get_shroud_range(id) > 0.0) {
            return;
        }
        let mut shrouding_mask: u16 = 0;
        let count = self.players.player_count();
        for current_index in (0..count).rev() {
            // Build mask of NON-allies.
            if self.cuts.player_relationship_to_player_default_team(controller, current_index) != Relationship::Allies {
                shrouding_mask |= self.cuts.player_mask(current_index);
            }
        }
        let pos = self.objects.find(id).expect("checked above").get_position();
        let range = self.get_shroud_range(id);
        self.cut(CutCall::PartitionDoShroudCover { x: pos.x, y: pos.y, range, mask: shrouding_mask });
        let o = self.objects.find_mut(id).expect("checked above");
        o.partition_last_shroud.where_ = pos;
        o.partition_last_shroud.for_whom = shrouding_mask;
        o.partition_last_shroud.how_far = range;
    }

    fn unshroud(&mut self, id: ObjectId) {
        let Some(o) = self.objects.find_mut(id) else { return };
        if o.partition_last_shroud.is_invalid() {
            return;
        }
        let s = o.partition_last_shroud;
        o.partition_last_shroud.reset();
        self.cut(CutCall::PartitionUndoShroudCover { x: s.where_.x, y: s.where_.y, range: s.how_far, mask: s.for_whom });
    }
}

/// `MAKE_OBJECT_STATUS_MASK(bit)`.
pub fn mask_of(bit: i32) -> ObjectStatusMaskType {
    let mut m = ObjectStatusMaskType::new();
    m.set(bit, 1);
    m
}
