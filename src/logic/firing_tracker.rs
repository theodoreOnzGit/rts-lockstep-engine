// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameLogic/FiringTracker.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/FiringTracker.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//   Author: Graham Smallwood, February 2002
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! `FiringTracker`: the update module that counts consecutive shots at the
//! same victim, steps an object's fire rate up through the continuous-fire
//! states (mean, then fast), cools it down when it stops, reloads it after an
//! idle spell and keeps its looping fire sound running.
//!
//! # The shape: the state here, the Object behind a [`FiringHost`]
//!
//! The tracker owns the five fields upstream keeps (the consecutive shots,
//! the victim, the cooldown frame, the reload frame and the looping sound's
//! stop frame). Everything else upstream reaches (`getObject()`,
//! `TheGameLogic`, the `Weapon` just fired, `TheAudio`, the module base's
//! `setWakeFrame`) is a method of the [`FiringHost`] the caller passes, and
//! **each is called at exactly the point upstream calls it, in upstream's
//! order**. The `Object` is not translated yet (and `world::object` is
//! another wave's file), so the host is the cut point; the full game's host
//! is the object plus the world.
//!
//! The tracker is the behaviour half of an update module (the world design's
//! `UpdateBody`): the scheduling half, `UpdateModule`, stays in the object's
//! module slot. Hence [`FiringTracker::new`] takes the host only to make the
//! constructor's `setWakeFrame(FOREVER)` call, [`FiringTracker::shot_fired`]
//! and [`FiringTracker::update`] end in a `set_wake_frame` / return of the
//! sleep, and `xfer` takes the module base's part as an argument (below).
//!
//! # The decision: the owner's weapon-bonus word, and the order of effects
//!
//! The previous draft handed the tracker a copy of the owner's
//! `m_weaponBonusCondition` word and a snapshot of the weapon's getters, and
//! returned a list of effects for the owner to replay. **That is not what
//! upstream does, and the difference changes the answer.** Reading
//! `Object::setWeaponBonusCondition` / `clearWeaponBonusCondition`
//! (`Object.cpp:4674-4697`): when the word actually changes they call
//! `m_weaponSet.weaponSetOnWeaponBonusChange(this)` at once, which calls
//! `Weapon::onWeaponBonusChange` on every weapon (`Weapon.cpp:1959`), which
//! for a weapon `RELOADING_CLIP` or `BETWEEN_FIRING_SHOTS` (the state a weapon
//! is in the moment it has fired, `Weapon::privateFireWeapon`,
//! `Weapon.cpp:2640-2690`, which is the only caller of `shotFired`, via
//! `Object::fireCurrentWeapon`) **recomputes `m_whenWeCanFireAgain`** from the
//! new bonus. `shotFired` then reads `weaponFired->getPossibleNextShotFrame()`
//! (the cooldown frame is that plus the coast time) *after* its faerie-fire
//! step, so a target marked with faerie fire changes the cooldown frame the
//! tracker stores. A snapshot taken before the call would be stale.
//!
//! So the rules this file keeps:
//!
//! 1. **The bonus word is the host's.** The tracker tests, sets and clears one
//!    condition at a time through the host (`testWeaponBonusCondition`,
//!    `setWeaponBonusCondition`, `clearWeaponBonusCondition`); a set or clear
//!    that changes the word is the host's cue to run
//!    `weaponSetOnWeaponBonusChange`, and the tracker never holds a copy.
//! 2. **Weapon getters are read when upstream reads them**, each a host call
//!    on the [`WeaponRef`] fired: the auto-reload frames, then the coast
//!    frames, then (only when the coast is non-zero) the possible next shot
//!    frame, then the two shots-needed counts, then the loop time, all after
//!    the faerie-fire step and before the mean/fast/cool-down step.
//! 3. **Effects happen as they are called**, not afterwards: the bonus
//!    changes of `speedUp` / `coolDown` interleave with the voice sound and
//!    end with the one `clearAndSetModelConditionFlags` call (made even when
//!    both masks are empty, as upstream does).
//!
//! The differential test (`tests/differential_firing_tracker.rs`) models
//! `weaponSetOnWeaponBonusChange` in both the C++ stand-in and the Rust host
//! (a reloading weapon's next-shot frame is recomputed from the word), logs
//! every call in order, and a planted variant that reads the next-shot frame
//! before the faerie-fire step is caught.
//!
//! # Client cut points
//!
//! `TheAudio->isCurrentlyPlaying(m_audioHandle)` (`FiringTracker.cpp:166`) only
//! decides whether a looping sound is restarted, and `m_audioHandle` is the
//! audio system's handle: both are client state that no logic ever reads. The
//! tracker does not hold the handle. It tells the host
//! [`FiringHost::start_looping_fire_sound`] with `only_if_not_playing` set to
//! upstream's `m_frameToStopLoopingSound != 0` (when it is zero the sound is
//! added unconditionally), so the client adds the sound when
//! `!only_if_not_playing || !isCurrentlyPlaying(handle)` and keeps the new
//! handle; [`FiringHost::stop_looping_fire_sound`] is
//! `removeAudioEvent(handle)` and the client then forgets the handle. The full
//! game's host turns these into [`crate::logic::client_event::ClientEvent`]s.
//!
//! # Semantics kept from upstream
//!
//! - **Shot bookkeeping.** A shot at the same victim adds one. A shot at a new
//!   victim inside the coast window (`now < m_frameToStartCooldown`) adds one
//!   too and takes the new victim; otherwise the count restarts at 1.
//! - **Rate changes.** With mean fire: below `one_needed` cools down, above
//!   `two_needed` speeds up. With fast fire: below `two_needed` cools down.
//!   With neither: above `one_needed` speeds up.
//! - **`coolDown`** drops fast and mean at once ("straight to zero") and
//!   always clears the count and the victim; from neither state it also
//!   clears the cooldown frame, from fast or mean it leaves it (the update
//!   re-arms it a second on).
//! - **`update`** reloads when the reload frame has come (`>=`), stops the
//!   looping sound when its frame has come (`>=`), and cools down when the
//!   cooldown frame is *passed* (`>`), then re-arms it one second on and
//!   sleeps one second. Otherwise it sleeps until the earliest pending frame.
//! - **`calcTimeToSleep`**: forever with no pending frame; otherwise the least
//!   distance to a pending frame, `UPDATE_SLEEP_NONE` for one already due.
//! - **Wrapping.** Frames wrap as unsigned arithmetic does upstream, and the
//!   shot count as the `-fwrapv` build does (the checked release profile
//!   would otherwise panic on the same input).
//! - **Not saved.** `xfer` writes the count, the victim and the cooldown frame
//!   only; the reload and looping-sound frames are not in the save (as
//!   upstream), so after a load they keep the values they had.
//!
//! # Transfer and the module base
//!
//! Upstream's `xfer` is: the version, `UpdateModule::xfer` (the base's part:
//! its own versions and `m_nextCallFrameAndPhase`), then the three fields.
//! The base belongs to the slot, so [`FiringTracker::xfer`] takes a closure
//! that writes it, called at exactly that point; `crc` is the base's alone,
//! and so is `load_post_process`. There is no `Snapshot` impl: its methods take
//! `&mut dyn Xfer`, which this crate does not use, and the base closure
//! carries the part the trait object would.
//!
//! # Not translated
//!
//! `MemoryPoolObject` glue, `MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA`
//! (the empty `FiringTrackerModuleData` has no fields; the module factory's
//! registration arrives with layer 9) and the unused `m_audioHandle`.

use crate::common::disabled_types::{disabledmask_all, DisabledMaskType};
use crate::common::game_common::LOGICFRAMES_PER_SECOND;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::model_state::{
    ModelConditionFlags, MODELCONDITION_CONTINUOUS_FIRE_FAST, MODELCONDITION_CONTINUOUS_FIRE_MEAN,
    MODELCONDITION_CONTINUOUS_FIRE_SLOW,
};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::module::update_module::{
    update_sleep, SleepyUpdatePhase, UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::logic::weapon_bonus::condition::{CONTINUOUS_FIRE_FAST, CONTINUOUS_FIRE_MEAN, TARGET_FAERIE_FIRE};

/// A handle to the `Weapon` just fired (`const Weapon* weaponFired`): opaque
/// to the tracker, resolved by the host (a weapon slot in the full game).
pub type WeaponRef = u32;

/// `FiringTracker::getUpdatePhase()`: after every normal update module
/// ("the firingtracker needs to have its update run after all 'normal' user
/// update modules ... Please don't redefine this for other modules without
/// very careful deliberation. (srj)").
pub const FIRING_TRACKER_UPDATE_PHASE: SleepyUpdatePhase = SleepyUpdatePhase::Final;

/// What `FiringTracker` reaches outside itself: its `Object`, `TheGameLogic`,
/// the weapon fired, `TheAudio` and the module base. See the module docs for
/// the order contract: the tracker calls these exactly where upstream does.
pub trait FiringHost {
    /// `TheGameLogic->getFrame()`.
    fn frame(&self) -> u32;
    /// `victim && victim->testStatus(OBJECT_STATUS_FAERIE_FIRE)` for
    /// `victim = TheGameLogic->findObjectByID(victim_id)`: false for a ground
    /// shot (`INVALID_ID`) or an id no object has. A destroyed object is
    /// still found until the end of the frame, as upstream.
    fn victim_has_faerie_fire(&self, victim_id: ObjectId) -> bool;
    /// `Object::testWeaponBonusCondition(condition)` on the owner.
    fn test_weapon_bonus_condition(&self, condition: i32) -> bool;
    /// `Object::setWeaponBonusCondition(condition)`: including
    /// `weaponSetOnWeaponBonusChange` when the word changes.
    fn set_weapon_bonus_condition(&mut self, condition: i32);
    /// `Object::clearWeaponBonusCondition(condition)`: likewise.
    fn clear_weapon_bonus_condition(&mut self, condition: i32);
    /// `Object::clearAndSetModelConditionFlags(clr, set)`.
    fn clear_and_set_model_condition_flags(&mut self, clr: &ModelConditionFlags, set: &ModelConditionFlags);
    /// `Object::reloadAllAmmo(now)`.
    fn reload_all_ammo(&mut self, now: bool);
    /// `weaponFired->getAutoReloadWhenIdleFrames()`.
    fn weapon_auto_reload_when_idle_frames(&self, weapon: WeaponRef) -> u32;
    /// `weaponFired->getContinuousFireCoastFrames()`.
    fn weapon_continuous_fire_coast_frames(&self, weapon: WeaponRef) -> u32;
    /// `weaponFired->getPossibleNextShotFrame()` (`m_whenWeCanFireAgain`,
    /// which a bonus change can recompute: see the module docs).
    fn weapon_possible_next_shot_frame(&self, weapon: WeaponRef) -> u32;
    /// `weaponFired->getContinuousFireOneShotsNeeded()`.
    fn weapon_continuous_fire_one_shots_needed(&self, weapon: WeaponRef) -> i32;
    /// `weaponFired->getContinuousFireTwoShotsNeeded()`.
    fn weapon_continuous_fire_two_shots_needed(&self, weapon: WeaponRef) -> i32;
    /// `weaponFired->getFireSoundLoopTime()`: 0 for a fire-and-forget sound.
    fn weapon_fire_sound_loop_time(&self, weapon: WeaponRef) -> u32;
    /// Client cut point: the fire-and-forget case of `shotFired`, a copy of
    /// `weaponFired->getFireSound()` with the owner's object id,
    /// `TheAudio->addAudioEvent(&sound)`.
    fn play_fire_sound(&mut self, weapon: WeaponRef);
    /// Client cut point: the looping case, the same event added and its
    /// handle kept. With `only_if_not_playing` the client adds it only when
    /// `!TheAudio->isCurrentlyPlaying(handle)`; without, always.
    fn start_looping_fire_sound(&mut self, weapon: WeaponRef, only_if_not_playing: bool);
    /// Client cut point: `TheAudio->removeAudioEvent(m_audioHandle)` and the
    /// handle becomes `AHSV_NoSound`.
    fn stop_looping_fire_sound(&mut self);
    /// Client cut point: `speedUp`'s voice, `getTemplate()->getPerUnitSound(
    /// "VoiceRapidFire")` copied, given the owner's object id and added.
    fn play_rapid_fire_voice(&mut self);
    /// `UpdateModule::setWakeFrame(getObject(), wakeDelay)` for this module.
    fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime);
}

/// `class FiringTracker`: the state of one object's continuous fire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiringTracker {
    /// `m_consecutiveShots`: how many times the owner has shot at the victim.
    consecutive_shots: i32,
    /// `m_victimID`: the victim being shot at.
    victim_id: ObjectId,
    /// `m_frameToStartCooldown`: the frame to cool down at, pushed back by
    /// each shot (0 = none).
    frame_to_start_cooldown: u32,
    /// `m_frameToForceReload`: the frame to reload at, pushed back by each
    /// shot (0 = none).
    frame_to_force_reload: u32,
    /// `m_frameToStopLoopingSound`: the frame to stop the looping sound at
    /// (0 = not looping).
    frame_to_stop_looping_sound: u32,
}

impl FiringTracker {
    /// `FiringTracker(thing, modData)`: no shots, no victim, nothing pending,
    /// and `setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)`.
    pub fn new<H: FiringHost>(host: &mut H) -> Self {
        host.set_wake_frame(UPDATE_SLEEP_FOREVER);
        FiringTracker {
            consecutive_shots: 0,
            victim_id: INVALID_ID,
            frame_to_start_cooldown: 0,
            frame_to_force_reload: 0,
            frame_to_stop_looping_sound: 0,
        }
    }

    /// `~FiringTracker()`: `TheAudio->removeAudioEvent(m_audioHandle)`,
    /// unconditionally ("no need to protect this").
    pub fn destroy<H: FiringHost>(&mut self, host: &mut H) {
        host.stop_looping_fire_sound();
    }

    /// `getNumConsecutiveShotsAtVictim(victim)`: the count when `victim` (the
    /// id of the object asked about, `None` for NULL) is the current victim,
    /// 0 otherwise.
    pub fn get_num_consecutive_shots_at_victim(&self, victim: Option<ObjectId>) -> i32 {
        match victim {
            None => 0,
            Some(id) if id != self.victim_id => 0,
            Some(_) => self.consecutive_shots,
        }
    }

    /// `getLastShotVictim()`.
    pub fn get_last_shot_victim(&self) -> ObjectId {
        self.victim_id
    }

    /// `getDisabledTypesToProcess()`: `DISABLEDMASK_ALL` ("this is never
    /// disabled, since we want disabled things to continue to slowly 'spin
    /// down'... (srj)").
    pub fn get_disabled_types_to_process() -> DisabledMaskType {
        disabledmask_all()
    }

    /// `shotFired(weaponFired, victimID)`: the owner just fired `weapon` at
    /// `victim_id` (`INVALID_ID` for a ground shot). Ends with
    /// `setWakeFrame(me, calcTimeToSleep())`, made on the host.
    pub fn shot_fired<H: FiringHost>(&mut self, host: &mut H, weapon: WeaponRef, victim_id: ObjectId) {
        let now = host.frame();

        if host.victim_has_faerie_fire(victim_id) {
            if !host.test_weapon_bonus_condition(TARGET_FAERIE_FIRE) {
                // We shoot faster at guys marked thusly
                host.set_weapon_bonus_condition(TARGET_FAERIE_FIRE);
            }
        } else {
            // A ground shot or the lack of the status on the target will clear this
            if host.test_weapon_bonus_condition(TARGET_FAERIE_FIRE) {
                host.clear_weapon_bonus_condition(TARGET_FAERIE_FIRE);
            }
        }

        if victim_id == self.victim_id {
            // Shooting at the same guy
            self.consecutive_shots = self.consecutive_shots.wrapping_add(1);
        } else if now < self.frame_to_start_cooldown {
            // Switching targets within the coast time is valid, and we will not spin down
            self.consecutive_shots = self.consecutive_shots.wrapping_add(1);
            self.victim_id = victim_id;
        } else {
            // Start the count over for the new guy
            self.consecutive_shots = 1;
            self.victim_id = victim_id;
        }

        // Push back the time that we should force a reload with each shot
        let auto_reload_delay = host.weapon_auto_reload_when_idle_frames(weapon);
        if auto_reload_delay > 0 {
            self.frame_to_force_reload = now.wrapping_add(auto_reload_delay);
        }

        let coast = host.weapon_continuous_fire_coast_frames(weapon);
        if coast != 0 {
            self.frame_to_start_cooldown = host.weapon_possible_next_shot_frame(weapon).wrapping_add(coast);
        } else {
            self.frame_to_start_cooldown = 0;
        }

        let shots_needed_one = host.weapon_continuous_fire_one_shots_needed(weapon);
        let shots_needed_two = host.weapon_continuous_fire_two_shots_needed(weapon);

        if host.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN) {
            // Can either go up or down from here.
            if self.consecutive_shots < shots_needed_one {
                self.cool_down(host);
            } else if self.consecutive_shots > shots_needed_two {
                self.speed_up(host);
            }
        } else if host.test_weapon_bonus_condition(CONTINUOUS_FIRE_FAST) {
            // Only place I can go here from here is all the way down
            if self.consecutive_shots < shots_needed_two {
                self.cool_down(host);
            }
        } else if self.consecutive_shots > shots_needed_one {
            // Check to go up
            self.speed_up(host);
        }

        let fire_sound_loop_time = host.weapon_fire_sound_loop_time(weapon);
        if fire_sound_loop_time != 0 {
            // If the sound has stopped playing, then we need to re-add it.
            host.start_looping_fire_sound(weapon, self.frame_to_stop_looping_sound != 0);
            self.frame_to_stop_looping_sound = now.wrapping_add(fire_sound_loop_time);
        } else {
            host.play_fire_sound(weapon);
            self.frame_to_stop_looping_sound = 0;
        }

        let sleep = self.calc_time_to_sleep(host);
        host.set_wake_frame(sleep);
    }

    /// `update()`: reload after an idle spell, stop the looping sound when it
    /// is due, cool down when the cooldown frame has passed, and the sleep
    /// until the next pending frame.
    pub fn update<H: FiringHost>(&mut self, host: &mut H) -> UpdateSleepTime {
        let now = host.frame();

        // I have been idle long enough that I should reload, so I do not hang around with a near empty clip forever.
        if self.frame_to_force_reload != 0 && now >= self.frame_to_force_reload {
            host.reload_all_ammo(true);
            self.frame_to_force_reload = 0;
        }

        // If it has been too long since I fired.  I have to start over.
        // (don't call if we don't need to cool down... it's expensive!)
        if self.frame_to_stop_looping_sound != 0 && now >= self.frame_to_stop_looping_sound {
            host.stop_looping_fire_sound();
            self.frame_to_stop_looping_sound = 0;
        }

        if self.frame_to_start_cooldown != 0 && now > self.frame_to_start_cooldown {
            self.frame_to_start_cooldown = now.wrapping_add(LOGICFRAMES_PER_SECOND as u32);
            self.cool_down(host); // if this is the coolest call to cooldown, it will set m_frameToStartCooldown to zero
            return update_sleep(LOGICFRAMES_PER_SECOND as u32);
        }

        self.calc_time_to_sleep(host)
    }

    /// `calcTimeToSleep()`: the longest the module can sleep as unneeded.
    fn calc_time_to_sleep<H: FiringHost>(&self, host: &H) -> UpdateSleepTime {
        // If all the timers are off, then we aren't needed at all
        if self.frame_to_stop_looping_sound == 0
            && self.frame_to_start_cooldown == 0
            && self.frame_to_force_reload == 0
        {
            return UPDATE_SLEEP_FOREVER;
        }

        // Otherwise, we need to wake up to service the shortest timer
        let now = host.frame();
        let mut sleep_time = UPDATE_SLEEP_FOREVER.0;
        for frame in [self.frame_to_stop_looping_sound, self.frame_to_start_cooldown, self.frame_to_force_reload] {
            if frame != 0 {
                if frame <= now {
                    sleep_time = UPDATE_SLEEP_NONE.0;
                } else if frame - now < sleep_time {
                    sleep_time = frame - now;
                }
            }
        }

        update_sleep(sleep_time)
    }

    /// `speedUp()`: from none to mean, from mean to fast (with the voice);
    /// fast stays fast. Always ends in one `clearAndSetModelConditionFlags`.
    fn speed_up<H: FiringHost>(&mut self, host: &mut H) {
        let mut clr = ModelConditionFlags::default();
        let mut set = ModelConditionFlags::default();

        if host.test_weapon_bonus_condition(CONTINUOUS_FIRE_FAST) {
            // nothing: the clears of upstream's commented-out lines are not made
        } else if host.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN) {
            host.play_rapid_fire_voice();

            // These flags are exclusive, not cumulative
            host.set_weapon_bonus_condition(CONTINUOUS_FIRE_FAST);
            set.set(MODELCONDITION_CONTINUOUS_FIRE_FAST, 1);

            // These flags are exclusive, not cumulative
            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_MEAN, 1);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_SLOW, 1);
        } else {
            host.set_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN);
            set.set(MODELCONDITION_CONTINUOUS_FIRE_MEAN, 1);

            // These flags are exclusive, not cumulative
            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_FAST);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_FAST, 1);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_SLOW, 1);
        }

        host.clear_and_set_model_condition_flags(&clr, &set);
    }

    /// `coolDown()`: drops the fire state, the count and the victim; from a
    /// fast or mean state the cooldown frame stays (the update restarts it),
    /// otherwise it is cleared.
    fn cool_down<H: FiringHost>(&mut self, host: &mut H) {
        let mut clr = ModelConditionFlags::default();
        let mut set = ModelConditionFlags::default();

        if host.test_weapon_bonus_condition(CONTINUOUS_FIRE_FAST)
            || host.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN)
        {
            // Straight to zero from wherever it is
            set.set(MODELCONDITION_CONTINUOUS_FIRE_SLOW, 1);

            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_FAST);
            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_FAST, 1);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_MEAN, 1);
        } else {
            // we stop, now
            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_FAST);
            host.clear_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN);
            // Just chillin, nothing to change
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_FAST, 1);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_MEAN, 1);
            clr.set(MODELCONDITION_CONTINUOUS_FIRE_SLOW, 1);

            self.frame_to_start_cooldown = 0;
        }

        host.clear_and_set_model_condition_flags(&clr, &set);

        // Start everything over
        self.consecutive_shots = 0;
        self.victim_id = INVALID_ID;
    }

    /// `crc(xfer)`: `UpdateModule::crc` only, which is `update_module` (the
    /// slot's); this module adds nothing.
    pub fn crc<X: Xfer + ?Sized>(
        &mut self,
        xfer: &mut X,
        update_module: impl FnOnce(&mut X) -> XferResult<()>,
    ) -> XferResult<()> {
        update_module(xfer)
    }

    /// `xfer(xfer)`, version 1: the version, `UpdateModule::xfer`
    /// (`update_module`, called at that point), then the consecutive shots, the
    /// victim and the cooldown frame.
    pub fn xfer<X: Xfer + ?Sized>(
        &mut self,
        xfer: &mut X,
        update_module: impl FnOnce(&mut X) -> XferResult<()>,
    ) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)?;

        // object helper base class
        update_module(xfer)?;

        // consecutive shots
        xfer.xfer_int(&mut self.consecutive_shots)?;

        // victim id
        xfer.xfer_object_id(&mut self.victim_id)?;

        // frame to start cooldown
        xfer.xfer_unsigned_int(&mut self.frame_to_start_cooldown)
    }

    /// `loadPostProcess()`: `UpdateModule::loadPostProcess` only
    /// (`update_module`).
    pub fn load_post_process(&mut self, update_module: impl FnOnce()) {
        update_module();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::xfer_load::XferLoad;
    use crate::common::xfer_save::XferSave;
    use crate::logic::weapon_bonus::condition::VETERAN;

    /// One recorded host call, in call order.
    #[derive(Clone, Debug, PartialEq)]
    enum Call {
        Set(i32),
        Clear(i32),
        Model { clr: Vec<i32>, set: Vec<i32> },
        Reload(bool),
        Fire,
        Loop(bool),
        StopLoop,
        Voice,
        Wake(u32),
    }

    /// A host with one weapon whose next-shot frame is recomputed when the
    /// bonus word changes (the stand-in for `weaponSetOnWeaponBonusChange`).
    #[derive(Default)]
    struct Host {
        frame: u32,
        word: u32,
        faerie: bool,
        next_shot: u32,
        recompute_to: Option<u32>,
        auto_reload: u32,
        coast: u32,
        one: i32,
        two: i32,
        loop_time: u32,
        calls: Vec<Call>,
    }

    impl Host {
        fn bits(f: &ModelConditionFlags) -> Vec<i32> {
            (0..crate::common::model_state::MODELCONDITION_COUNT).filter(|&i| f.test(i)).collect()
        }
        fn changed(&mut self, old: u32) {
            if old != self.word {
                if let Some(v) = self.recompute_to {
                    self.next_shot = v;
                }
            }
        }
    }

    impl FiringHost for Host {
        fn frame(&self) -> u32 {
            self.frame
        }
        fn victim_has_faerie_fire(&self, victim_id: ObjectId) -> bool {
            victim_id != INVALID_ID && self.faerie
        }
        fn test_weapon_bonus_condition(&self, c: i32) -> bool {
            self.word & (1 << c) != 0
        }
        fn set_weapon_bonus_condition(&mut self, c: i32) {
            let old = self.word;
            self.word |= 1 << c;
            self.calls.push(Call::Set(c));
            self.changed(old);
        }
        fn clear_weapon_bonus_condition(&mut self, c: i32) {
            let old = self.word;
            self.word &= !(1 << c);
            self.calls.push(Call::Clear(c));
            self.changed(old);
        }
        fn clear_and_set_model_condition_flags(&mut self, clr: &ModelConditionFlags, set: &ModelConditionFlags) {
            self.calls.push(Call::Model { clr: Host::bits(clr), set: Host::bits(set) });
        }
        fn reload_all_ammo(&mut self, now: bool) {
            self.calls.push(Call::Reload(now));
        }
        fn weapon_auto_reload_when_idle_frames(&self, _: WeaponRef) -> u32 {
            self.auto_reload
        }
        fn weapon_continuous_fire_coast_frames(&self, _: WeaponRef) -> u32 {
            self.coast
        }
        fn weapon_possible_next_shot_frame(&self, _: WeaponRef) -> u32 {
            self.next_shot
        }
        fn weapon_continuous_fire_one_shots_needed(&self, _: WeaponRef) -> i32 {
            self.one
        }
        fn weapon_continuous_fire_two_shots_needed(&self, _: WeaponRef) -> i32 {
            self.two
        }
        fn weapon_fire_sound_loop_time(&self, _: WeaponRef) -> u32 {
            self.loop_time
        }
        fn play_fire_sound(&mut self, _: WeaponRef) {
            self.calls.push(Call::Fire);
        }
        fn start_looping_fire_sound(&mut self, _: WeaponRef, only_if_not_playing: bool) {
            self.calls.push(Call::Loop(only_if_not_playing));
        }
        fn stop_looping_fire_sound(&mut self) {
            self.calls.push(Call::StopLoop);
        }
        fn play_rapid_fire_voice(&mut self) {
            self.calls.push(Call::Voice);
        }
        fn set_wake_frame(&mut self, wake_delay: UpdateSleepTime) {
            self.calls.push(Call::Wake(wake_delay.0));
        }
    }

    fn host() -> Host {
        Host { one: 2, two: 4, coast: 10, next_shot: 100, ..Host::default() }
    }

    #[test]
    fn a_new_tracker_sleeps_forever_and_knows_no_victim() {
        let mut h = host();
        let t = FiringTracker::new(&mut h);
        assert_eq!(h.calls, vec![Call::Wake(0x3fff_ffff)]);
        assert_eq!(t.get_last_shot_victim(), INVALID_ID);
        assert_eq!(t.get_num_consecutive_shots_at_victim(None), 0);
        assert_eq!(t.get_num_consecutive_shots_at_victim(Some(0)), 0, "INVALID_ID is the victim of a new tracker, with no shots");
        assert_eq!(FiringTracker::get_disabled_types_to_process(), disabledmask_all());
        assert_eq!(FIRING_TRACKER_UPDATE_PHASE, SleepyUpdatePhase::Final);
    }

    #[test]
    fn shots_at_the_same_victim_climb_through_mean_to_fast_with_the_voice_once() {
        let mut h = host();
        let mut t = FiringTracker::new(&mut h);
        h.calls.clear();
        for _ in 0..6 {
            h.frame += 1;
            t.shot_fired(&mut h, 0, 7);
        }
        assert_eq!(t.get_num_consecutive_shots_at_victim(Some(7)), 6);
        assert_eq!(t.get_num_consecutive_shots_at_victim(Some(8)), 0);
        // shot 3 (> one) speeds up to mean; shot 5 (> two) speeds up to fast with the voice
        let sets: Vec<&Call> = h.calls.iter().filter(|c| matches!(c, Call::Set(_) | Call::Voice)).collect();
        assert_eq!(sets, vec![&Call::Set(CONTINUOUS_FIRE_MEAN), &Call::Voice, &Call::Set(CONTINUOUS_FIRE_FAST)]);
        assert!(h.test_weapon_bonus_condition(CONTINUOUS_FIRE_FAST));
        assert!(!h.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN));
    }

    #[test]
    fn the_next_shot_frame_is_read_after_the_faerie_fire_step() {
        // a faerie-fire victim sets the bonus, which recomputes the weapon's next-shot frame to 150;
        // the cooldown frame must be 150 + coast, not the 100 the weapon had before the call
        let mut h = Host { faerie: true, recompute_to: Some(150), ..host() };
        h.frame = 90;
        let mut t = FiringTracker::new(&mut h);
        t.shot_fired(&mut h, 0, 3);
        assert_eq!(h.next_shot, 150);
        assert_eq!(t.frame_to_start_cooldown, 160);
        // and the sleep is until that frame (the reload and the sound are off)
        assert_eq!(h.calls.last(), Some(&Call::Wake(160 - 90)));
        // a shot with no faerie fire clears the bonus again
        h.faerie = false;
        t.shot_fired(&mut h, 0, 3);
        assert!(h.calls.contains(&Call::Clear(TARGET_FAERIE_FIRE)));
    }

    #[test]
    fn switching_targets_inside_the_coast_window_keeps_the_count() {
        let mut h = host();
        h.frame = 50;
        let mut t = FiringTracker::new(&mut h);
        t.shot_fired(&mut h, 0, 1); // cooldown = 110
        t.shot_fired(&mut h, 0, 2); // 50 < 110: the count goes on
        assert_eq!((t.get_last_shot_victim(), t.consecutive_shots), (2, 2));
        h.frame = 200;
        t.shot_fired(&mut h, 0, 3); // 200 >= 110: starts over
        assert_eq!((t.get_last_shot_victim(), t.consecutive_shots), (3, 1));
    }

    #[test]
    fn the_looping_sound_restarts_only_when_a_loop_is_running_and_stops_when_due() {
        let mut h = Host { loop_time: 30, ..host() };
        h.frame = 10;
        let mut t = FiringTracker::new(&mut h);
        h.calls.clear();
        t.shot_fired(&mut h, 0, 1);
        h.frame = 20;
        t.shot_fired(&mut h, 0, 1);
        let loops: Vec<&Call> = h.calls.iter().filter(|c| matches!(c, Call::Loop(_) | Call::Fire)).collect();
        assert_eq!(loops, vec![&Call::Loop(false), &Call::Loop(true)]);
        assert_eq!(t.frame_to_stop_looping_sound, 50);
        h.frame = 50;
        h.calls.clear();
        t.update(&mut h);
        assert!(h.calls.contains(&Call::StopLoop));
        assert_eq!(t.frame_to_stop_looping_sound, 0);
        // a weapon with no loop time plays once and clears the stop frame
        h.loop_time = 0;
        t.shot_fired(&mut h, 0, 1);
        assert!(h.calls.contains(&Call::Fire));
        assert_eq!(t.frame_to_stop_looping_sound, 0);
        t.destroy(&mut h);
        assert_eq!(h.calls.last(), Some(&Call::StopLoop));
    }

    #[test]
    fn update_reloads_cools_down_and_computes_the_sleep() {
        let mut h = Host { auto_reload: 20, ..host() };
        h.frame = 5;
        let mut t = FiringTracker::new(&mut h);
        for _ in 0..3 {
            t.shot_fired(&mut h, 0, 1); // reaches mean fire
        }
        assert!(h.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN));
        // reload due at 25, cooldown at 110
        h.frame = 24;
        assert_eq!(t.update(&mut h), update_sleep(1));
        h.frame = 25;
        h.calls.clear();
        assert_eq!(t.update(&mut h), update_sleep(110 - 25));
        assert_eq!(h.calls, vec![Call::Reload(true)]);
        // past the cooldown frame: cool down, re-arm a second on, sleep a second
        h.frame = 111;
        h.calls.clear();
        assert_eq!(t.update(&mut h), update_sleep(30));
        assert_eq!(t.frame_to_start_cooldown, 141);
        assert!(!h.test_weapon_bonus_condition(CONTINUOUS_FIRE_MEAN));
        assert_eq!(t.consecutive_shots, 0);
        // from neither state the next cool down clears the cooldown frame
        h.frame = 142;
        assert_eq!(t.update(&mut h), update_sleep(30));
        assert_eq!(t.frame_to_start_cooldown, 0);
        // nothing pending: forever
        assert_eq!(t.update(&mut h), UPDATE_SLEEP_FOREVER);
    }

    #[test]
    fn cooling_down_leaves_an_unrelated_bonus_alone() {
        let mut h = host();
        h.word = 1 << VETERAN;
        let mut t = FiringTracker::new(&mut h);
        for _ in 0..5 {
            t.shot_fired(&mut h, 0, 1);
        }
        assert!(h.test_weapon_bonus_condition(VETERAN));
        h.frame = 1000;
        t.update(&mut h);
        assert!(h.test_weapon_bonus_condition(VETERAN), "cooling down clears fast and mean only");
    }

    #[test]
    fn xfer_writes_the_version_the_base_then_three_fields_and_loads_them_back() {
        let mut h = host();
        let mut t = FiringTracker::new(&mut h);
        t.shot_fired(&mut h, 0, 9);
        let mut save = XferSave::new();
        save.open(b"firing").unwrap();
        t.xfer(&mut save, |x| x.xfer_unsigned_int(&mut 0xaabb_ccdd)).unwrap();
        save.close().unwrap();
        let bytes = save.take_bytes();
        assert_eq!(bytes.len(), 1 + 4 + 4 + 4 + 4);
        assert_eq!(&bytes[1..5], &0xaabb_ccddu32.to_le_bytes());
        assert_eq!(&bytes[5..9], &1i32.to_le_bytes());
        assert_eq!(&bytes[9..13], &9i32.to_le_bytes());
        assert_eq!(&bytes[13..17], &t.frame_to_start_cooldown.to_le_bytes());

        let mut loaded = FiringTracker::new(&mut host());
        let mut load = XferLoad::new(bytes);
        load.open(b"firing").unwrap();
        let mut base = 0u32;
        loaded.xfer(&mut load, |x| x.xfer_unsigned_int(&mut base)).unwrap();
        load.close().unwrap();
        assert_eq!(base, 0xaabb_ccdd);
        assert_eq!(loaded.consecutive_shots, 1);
        assert_eq!(loaded.victim_id, 9);
        assert_eq!(loaded.frame_to_start_cooldown, t.frame_to_start_cooldown);
        assert_eq!(loaded.frame_to_force_reload, 0, "not saved");

        // crc and load_post_process are the base's alone
        let mut seen = 0;
        let mut crc = XferSave::new();
        crc.open(b"crc").unwrap();
        t.crc(&mut crc, |_| {
            seen += 1;
            Ok(())
        })
        .unwrap();
        t.load_post_process(|| seen += 10);
        assert_eq!(seen, 11);
    }
}
