// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/FiringTracker.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/FiringTracker.cpp
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp (the tracker's calls: lines 397-401, 1487-1523, 1566, 6009)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FiringTracker` as an object's helper module: the world half of [`crate::logic::firing_tracker`] (`p1w13-sonnet`, batch R78).
//!
//! The tracker (`logic::firing_tracker`, verified against upstream on its own in `tests/differential_firing_tracker.rs`) asks everything of a
//! [`FiringHost`]; here the [`World`] is that host, for **the module that is running** (the top of `cuts.module_stack`, as for the other
//! helpers), and the module sits in its object's slot like them: made by `Object::Object` for an object whose template can have a weapon
//! (`ModuleTag_FiringTrackerHelper`), in the final update phase, never disabled, asleep until a shot.
//!
//! The object's calls are module calls: `notifyFiringTrackerShotFired` and `fireCurrentWeapon`'s `shotFired` are
//! [`ModuleCall::FiringTrackerShotFired`] (the weapon by its slot: the tracker reads the live weapon, whose next shot frame a bonus change
//! recomputes), `getNumConsecutiveShotsFiredAtTarget` is [`ModuleCall::FiringTrackerGetNumConsecutiveShotsAtVictim`] and
//! `getLastShotVictim` is [`ModuleCall::FiringTrackerGetLastShotVictim`].
//!
//! # Client cut points
//!
//! The fire sounds: the fire-and-forget sound is `AudioAddEvent` with the weapon's sound and the owner; the looping sound's start and stop
//! are [`ObjectClientEvent::FiringTrackerStartLoopingSound`] and `FiringTrackerStopLoopingSound` (the handle and `isCurrentlyPlaying` are
//! the client's); the rapid-fire voice is the template's `VoiceRapidFire` per-unit sound.
//!
//! # Deviation
//!
//! The destructor stops the looping sound; the world has no hook for a module's destruction, so it is not sent (as `BridgeBehavior`'s).

use std::sync::Arc;

use crate::common::game_type::ObjectId;
use crate::common::ini::IniResult;
use crate::common::model_state::{ModelConditionFlags, MODELCONDITION_COUNT};
use crate::common::object_status_types::OBJECT_STATUS_FAERIE_FIRE;
use crate::common::xfer::{Snapshot, Xfer, XferResult};
use crate::logic::firing_tracker::{FiringHost, FiringTracker, WeaponRef, FIRING_TRACKER_UPDATE_PHASE};
use crate::logic::module::update_module::{UpdateModule, UpdateModuleData, UpdateSleepTime};
use crate::logic::modules::{lend_clone, UpdateHost};
use crate::logic::weapon::WeaponTemplate;
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleHandle, ModuleReply};
use crate::logic::world::object_events::ObjectClientEvent;
use crate::logic::world::sleepy::UpdateHandle;
use crate::logic::world::update_body::UpdateBody;

/// `class FiringTracker : public UpdateModule`: the scheduling half and the tracker.
#[derive(Clone, Debug)]
pub struct FiringTrackerModule {
    update: UpdateModule,
    tracker: FiringTracker,
}

impl PartialEq for FiringTrackerModule {
    /// The same object and the same state.
    fn eq(&self, other: &Self) -> bool {
        self.update.behavior().object() == other.update.behavior().object() && self.tracker == other.tracker
    }
}

impl FiringTrackerModule {
    /// The constructor's first half, which touches no world: the base in the final phase, never disabled, and the tracker's state.
    pub fn unconstructed(object: ObjectId, module_data: Option<Arc<UpdateModuleData>>) -> IniResult<Self> {
        let mut update = UpdateModule::new(object, module_data, FIRING_TRACKER_UPDATE_PHASE)?;
        update.set_disabled_types_to_process(FiringTracker::get_disabled_types_to_process());
        Ok(FiringTrackerModule { update, tracker: FiringTracker::unconstructed() })
    }

    pub fn update_module(&self) -> &UpdateModule {
        &self.update
    }

    /// The tracker.
    pub fn tracker(&self) -> &FiringTracker {
        &self.tracker
    }

    /// `update()` as the scheduler runs it: this module is the running one for the host calls, and it is lent to its slot.
    pub fn world_update(&mut self, world: &mut World, me: UpdateHandle) -> UpdateSleepTime {
        let Some(h) = world.module_handle_of_update(me) else {
            return self.tracker.update(world);
        };
        lend_clone(world, h, self, UpdateBody::FiringTracker, un, |w, m| w.in_update_module(me, |w| m.tracker.update(w)))
    }

    /// The world half of the constructor: `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn world_construct(&mut self, world: &mut World, _me: ModuleHandle) {
        self.tracker = FiringTracker::new(world);
    }

    /// The object's calls: a shot fired, the consecutive shots at a victim, the last victim.
    pub fn module_call(&mut self, world: &mut World, me: ModuleHandle, call: ModuleCall) -> ModuleReply {
        match call {
            ModuleCall::FiringTrackerShotFired { slot, victim } => {
                lend_clone(world, me, self, UpdateBody::FiringTracker, un, |w, m| m.tracker.shot_fired(w, slot as WeaponRef, victim));
                ModuleReply::Done
            }
            ModuleCall::FiringTrackerGetNumConsecutiveShotsAtVictim(victim) => ModuleReply::Int(self.tracker.get_num_consecutive_shots_at_victim(victim)),
            ModuleCall::FiringTrackerGetLastShotVictim => ModuleReply::Id(self.tracker.get_last_shot_victim()),
            _ => ModuleReply::Unhandled,
        }
    }

    /// `xfer` with the scheduling half the slot holds: the version, `UpdateModule::xfer`, the fields.
    pub fn xfer_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.tracker.xfer(xfer, |x| update.xfer(x))
    }

    /// `crc` with the scheduling half the slot holds.
    pub fn crc_with(&mut self, update: &mut UpdateModule, xfer: &mut dyn Xfer) -> XferResult<()> {
        self.tracker.crc(xfer, |x| update.crc(x))
    }

    /// `loadPostProcess` with the scheduling half the slot holds.
    pub fn load_post_process_with(&mut self, update: &mut UpdateModule) {
        self.tracker.load_post_process(|| update.load_post_process());
    }
}

impl Snapshot for FiringTrackerModule {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.update.clone();
        let r = self.crc_with(&mut update, xfer);
        self.update = update;
        r
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let mut update = self.update.clone();
        let r = self.xfer_with(&mut update, xfer);
        self.update = update;
        r
    }
    fn load_post_process(&mut self) {
        let mut update = self.update.clone();
        self.load_post_process_with(&mut update);
        self.update = update;
    }
}

fn un(b: UpdateBody) -> Option<FiringTrackerModule> {
    match b {
        UpdateBody::FiringTracker(m) => Some(m),
        _ => None,
    }
}

impl World {
    /// The weapon template of the running object's weapon in `slot` (the weapon the tracker was told was fired).
    fn fired_weapon_template(&self, slot: WeaponRef) -> Option<&WeaponTemplate> {
        let o = self.objects.find(self.running_object())?;
        let w = o.weapon_set.slots().get(slot as usize)?.as_ref()?;
        self.statics.weapon_store.templates().get(w.template())
    }
}

impl FiringHost for World {
    fn frame(&self) -> u32 {
        World::frame(self)
    }
    fn victim_has_faerie_fire(&self, victim_id: ObjectId) -> bool {
        self.objects.find(victim_id).is_some_and(|o| o.test_status(OBJECT_STATUS_FAERIE_FIRE))
    }
    fn test_weapon_bonus_condition(&self, condition: i32) -> bool {
        self.objects.find(self.running_object()).is_some_and(|o| o.test_weapon_bonus_condition(condition))
    }
    fn set_weapon_bonus_condition(&mut self, condition: i32) {
        let id = self.running_object();
        World::set_weapon_bonus_condition(self, id, condition);
    }
    fn clear_weapon_bonus_condition(&mut self, condition: i32) {
        let id = self.running_object();
        World::clear_weapon_bonus_condition(self, id, condition);
    }
    fn clear_and_set_model_condition_flags(&mut self, clr: &ModelConditionFlags, set: &ModelConditionFlags) {
        let bits = |f: &ModelConditionFlags| (0..MODELCONDITION_COUNT).filter(|&i| f.test(i)).collect::<Vec<i32>>();
        let (c, s) = (bits(clr), bits(set));
        let id = self.running_object();
        World::clear_and_set_model_condition_flags(self, id, &c, &s);
    }
    fn reload_all_ammo(&mut self, now: bool) {
        let id = self.running_object();
        crate::logic::world::object_weapons::reload_all_ammo(self, id, now);
    }
    fn weapon_auto_reload_when_idle_frames(&self, weapon: WeaponRef) -> u32 {
        self.fired_weapon_template(weapon).map_or(0, |t| t.auto_reload_when_idle_frames)
    }
    fn weapon_continuous_fire_coast_frames(&self, weapon: WeaponRef) -> u32 {
        self.fired_weapon_template(weapon).map_or(0, |t| t.continuous_fire_coast_frames)
    }
    fn weapon_possible_next_shot_frame(&self, weapon: WeaponRef) -> u32 {
        let o = self.objects.find(self.running_object());
        o.and_then(|o| o.weapon_set.slots().get(weapon as usize)?.as_ref().map(|w| w.possible_next_shot_frame())).unwrap_or(0)
    }
    fn weapon_continuous_fire_one_shots_needed(&self, weapon: WeaponRef) -> i32 {
        self.fired_weapon_template(weapon).map_or(i32::MAX, |t| t.continuous_fire_one_shots_needed)
    }
    fn weapon_continuous_fire_two_shots_needed(&self, weapon: WeaponRef) -> i32 {
        self.fired_weapon_template(weapon).map_or(i32::MAX, |t| t.continuous_fire_two_shots_needed)
    }
    fn weapon_fire_sound_loop_time(&self, weapon: WeaponRef) -> u32 {
        self.fired_weapon_template(weapon).map_or(0, |t| t.fire_sound_loop_time)
    }
    fn play_fire_sound(&mut self, weapon: WeaponRef) {
        let name = self.fired_weapon_template(weapon).map(|t| t.fire_sound.clone()).unwrap_or_default();
        let id = self.running_object();
        self.audio_add_event(name, id);
    }
    fn start_looping_fire_sound(&mut self, weapon: WeaponRef, only_if_not_playing: bool) {
        let name = self.fired_weapon_template(weapon).map(|t| t.fire_sound.clone()).unwrap_or_default();
        let object_id = self.running_object();
        self.emit(ObjectClientEvent::FiringTrackerStartLoopingSound { object_id, name, only_if_not_playing });
    }
    fn stop_looping_fire_sound(&mut self) {
        let object_id = self.running_object();
        self.emit(ObjectClientEvent::FiringTrackerStopLoopingSound { object_id });
    }
    fn play_rapid_fire_voice(&mut self) {
        let id = self.running_object();
        let name = self.objects.find(id).map(|o| o.template().get_per_unit_sound(b"VoiceRapidFire").event_name.clone()).unwrap_or_default();
        self.audio_add_event(name, id);
    }
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime) {
        let frame = UpdateModule::set_wake_frame(World::frame(self), wake_delay);
        UpdateHost::awaken_update_module(self, frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_module_starts_in_the_final_phase_never_disabled() {
        let m = FiringTrackerModule::unconstructed(1, Some(Arc::new(UpdateModuleData::new()))).unwrap();
        assert_eq!(m.update_module().get_disabled_types_to_process(), FiringTracker::get_disabled_types_to_process());
        assert_eq!(m.tracker(), &FiringTracker::unconstructed());
    }
}
