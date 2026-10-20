// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Object.cpp
//   (`attemptDamage`, `attemptHealing`, `attemptHealingFromSoleBenefactor`,
//   `getSoleHealingBenefactor`, `estimateDamage`, `kill`, `healCompletely`,
//   `onDie`, `checkAndDetonateBoobyTrap`, `scoreTheKill`,
//   `onVeterancyLevelChanged`, `canCrushOrSquish`, `topple`, `isMobile`,
//   `isAbleToAttack`, the helper forwards)
//
// Original copyright notice of that file:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Damage, healing, death and veterancy of an `Object`: the paths up to the
//! module calls (the body's `attemptDamage`, each die module's `onDie`).

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::{DISABLED_EMP, DISABLED_HACKED, DISABLED_SUBDUED, DISABLED_UNMANNED};
use crate::common::game_common::{Relationship, VeterancyLevel};
use crate::common::game_type::{ObjectId, INVALID_ID, WEAPONSLOT_COUNT};
use crate::common::kind_of::{
    KINDOF_CAN_ATTACK, KINDOF_IGNORED_IN_GUI, KINDOF_IMMOBILE, KINDOF_INFANTRY, KINDOF_MP_COUNT_FOR_VICTORY, KINDOF_PORTABLE_STRUCTURE,
    KINDOF_PROJECTILE, KINDOF_SPAWNS_ARE_THE_WEAPONS, KINDOF_STRUCTURE, KINDOF_VEHICLE,
};
use crate::common::object_status_types::{
    OBJECT_STATUS_AIRBORNE_TARGET, OBJECT_STATUS_BOOBY_TRAPPED, OBJECT_STATUS_CAN_ATTACK, OBJECT_STATUS_DETECTED, OBJECT_STATUS_DISGUISED,
    OBJECT_STATUS_NO_ATTACK, OBJECT_STATUS_RECONSTRUCTING, OBJECT_STATUS_SOLD, OBJECT_STATUS_STEALTHED, OBJECT_STATUS_UNDER_CONSTRUCTION,
};
use crate::logic::damage::{DamageInfo, DamageInfoInput, DamageType, DeathType, DAMAGE_HEALING, DAMAGE_PENALTY, HUGE_DAMAGE_AMOUNT};
use crate::logic::weapon_bonus::condition as wbc;
use crate::logic::weapon_types::{WEAPONSET_ELITE, WEAPONSET_HERO, WEAPONSET_VETERAN};
use crate::logic::world::game_logic::World;
use crate::logic::world::module_call::{ModuleCall, ModuleReply};
use crate::logic::world::object_cuts::CutCall;
use crate::logic::world::object_events::{EvaMessage, ModelConditionOp, ObjectClientEvent, ObjectSound, RadarCall};

/// `DEATH_NONE` (`enum DeathType`, `Damage.h`).
pub const DEATH_NONE: DeathType = 1;

/// `BOOBY_TRAP_SCAN_RANGE`.
pub const BOOBY_TRAP_SCAN_RANGE: f32 = 25.0;

/// `CrushSquishTestType` (`Object.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrushSquishTestType {
    /// `TEST_CRUSH_ONLY`.
    CrushOnly,
    /// `TEST_SQUISH_ONLY`.
    SquishOnly,
    /// `TEST_CRUSH_OR_SQUISH`.
    CrushOrSquish,
}

impl World {
    // ---- damage -----------------------------------------------------------------------------

    /// `attemptDamage(damageInfo)`: "something is attempting to damage this
    /// object". The body module takes the damage and writes the result into
    /// `info.out`; an object with a physics module that is not airborne and
    /// not a projectile is then pushed by the shock wave of the damage
    /// (`shockWaveAmount`, tapering off with the distance from the centre)
    /// and stunned; and the radar of the local player is told when damage
    /// was dealt to one of its objects by a player other than itself.
    ///
    /// Every call is first appended to [`Object::damage_received`](crate::logic::world::object::Object::damage_received),
    /// the record of this cut point that predates the body module.
    pub fn attempt_damage(&mut self, id: ObjectId, info: &mut DamageInfo) {
        let Some(o) = self.objects.find_mut(id) else { return };
        o.damage_received.push(info.clone());
        if let Some(body) = self.get_body_module(id) {
            if let ModuleReply::Damage(d) = self.module_call(id, body, ModuleCall::AttemptDamage(info.clone())) {
                *info = d;
            }
        }

        // Process any shockwave forces that might affect this object due to the incurred damage
        if info.input.shock_wave_amount > 0.0 && info.input.shock_wave_radius > 0.0 {
            let o = self.objects.find(id).expect("checked above");
            let physics = o.physics;
            if let Some(physics) = physics {
                if !o.test_status(OBJECT_STATUS_AIRBORNE_TARGET) && !o.is_kind_of(KINDOF_PROJECTILE) {
                    // Calculate the shockwave taperoff amount due to distance from ground zero
                    let shock_wave_scalar = info.input.shock_wave_vector.length();
                    let ratio = shock_wave_scalar / info.input.shock_wave_radius;
                    // MSVC's min(1.0f, x) is `(1.0f < x) ? 1.0f : x`
                    let distance_from_center = if 1.0f32 < ratio { 1.0f32 } else { ratio };
                    let distance_taper = distance_from_center * (1.0f32 - info.input.shock_wave_taper_off);
                    let shock_taper_mult = 1.0f32 - distance_taper;

                    // Set up the shockwave force to use apply on object
                    let mut shock_wave_force = info.input.shock_wave_vector;
                    shock_wave_force.normalize();
                    shock_wave_force.scale(info.input.shock_wave_amount * shock_taper_mult);
                    shock_wave_force.z = shock_wave_force.length(); // Apply up force equal to the lateral force for dramatic effect

                    // Apply the shock to the object
                    self.module_call(id, physics, ModuleCall::PhysicsApplyShock(shock_wave_force));
                    // Add random rotation to the object for drama
                    self.module_call(id, physics, ModuleCall::PhysicsApplyRandomRotation);
                    // Set stunned state due to the shock for the object
                    self.module_call(id, physics, ModuleCall::PhysicsSetStunned(true));
                    self.set_model_condition_state(id, crate::common::model_state::MODELCONDITION_STUNNED_FLAILING);
                }
            }
        }

        // if actual damage occurred, and this is an object owned by the local player we might do a radar event for
        // under attack. Note that we do not even try to do radar events for DAMAGE_PENALTY as that damage type is a
        // type of damage that occurs with explicit player knowledge
        if info.out.actual_damage_dealt > 0.0 && info.input.damage_type != DAMAGE_PENALTY && info.input.damage_type != DAMAGE_HEALING {
            if let Some(player) = self.get_controlling_player(id) {
                let o = self.objects.find(id).expect("checked above");
                if info.input.source_player_mask & self.cuts.player_mask(player) == 0
                    && o.on_radar
                    && self.players.local_index() == Some(player)
                {
                    self.emit(ObjectClientEvent::Radar { call: RadarCall::TryUnderAttackEvent, object_id: id, position: None });
                }
            }
        }
    }

    /// `attemptHealing(amount, source)`.
    pub fn attempt_healing(&mut self, id: ObjectId, amount: f32, source: ObjectId) {
        if let Some(body) = self.get_body_module(id) {
            let mut damage_info = DamageInfo::default();
            damage_info.input.damage_type = DAMAGE_HEALING;
            damage_info.input.death_type = DEATH_NONE;
            damage_info.input.source_id = if self.objects.find(source).is_some() { source } else { INVALID_ID };
            damage_info.input.amount = amount;
            self.module_call(id, body, ModuleCall::AttemptHealing(damage_info));
        }
    }

    /// `getSoleHealingBenefactor()`: who the only healer is, until the
    /// expiration frame.
    pub fn get_sole_healing_benefactor(&self, id: ObjectId) -> ObjectId {
        let Some(o) = self.objects.find(id) else { return INVALID_ID };
        if self.frame() > o.sole_healing_benefactor_expiration_frame {
            return INVALID_ID;
        }
        o.sole_healing_benefactor_id
    }

    /// `attemptHealingFromSoleBenefactor(amount, source, duration)`: "for the
    /// non-stacking healers like ambulance and propaganda". False without a
    /// source, or while another healer holds the benefit.
    pub fn attempt_healing_from_sole_benefactor(&mut self, id: ObjectId, amount: f32, source: ObjectId, duration: u32) -> bool {
        if self.objects.find(source).is_none() {
            return false; // sanity
        }
        let now = self.frame();
        let Some(o) = self.objects.find_mut(id) else { return false };
        // Either it is ok to accept healing from any who offer or this is my guy, calling again
        if now > o.sole_healing_benefactor_expiration_frame || o.sole_healing_benefactor_id == source {
            o.sole_healing_benefactor_id = source;
            o.sole_healing_benefactor_expiration_frame = now.wrapping_add(duration);
            if let Some(body) = self.get_body_module(id) {
                let mut damage_info = DamageInfo::default();
                damage_info.input.damage_type = DAMAGE_HEALING;
                damage_info.input.death_type = DEATH_NONE;
                damage_info.input.source_id = source;
                damage_info.input.amount = amount;
                self.module_call(id, body, ModuleCall::AttemptHealing(damage_info));
            }
            return true;
        }
        false
    }

    /// `estimateDamage(damageInfo)`: the body's estimate, 0 without a body.
    pub fn estimate_damage(&mut self, id: ObjectId, damage_info: &DamageInfoInput) -> f32 {
        match self.get_body_module(id) {
            Some(body) => self.module_call(id, body, ModuleCall::EstimateDamage(damage_info.clone())).as_real(0.0),
            None => 0.0,
        }
    }

    /// `kill(damageType, deathType)`: "do so much damage to an object that it
    /// will certainly die": unmodifiable damage equal to the maximum health,
    /// flagged to kill no matter what.
    pub fn kill(&mut self, id: ObjectId, damage_type: DamageType, death_type: DeathType) {
        let mut damage_info = DamageInfo::default();
        // Do unmodifiable damage equal to their max health to kill.
        damage_info.input.damage_type = damage_type;
        damage_info.input.death_type = death_type;
        damage_info.input.source_id = INVALID_ID;
        damage_info.input.amount = match self.get_body_module(id) {
            Some(body) => self.module_call(id, body, ModuleCall::GetMaxHealth).as_real(0.0),
            None => 0.0,
        };
        damage_info.input.kill = true; // Triggers object to die no matter what.
        self.attempt_damage(id, &mut damage_info);
    }

    /// `healCompletely()`: "restore max health to this Object".
    pub fn heal_completely(&mut self, id: ObjectId) {
        self.attempt_healing(id, HUGE_DAMAGE_AMOUNT, INVALID_ID);
    }

    /// `notifySubdualDamage(amount)`: the helper, and the tint while gaining.
    pub fn notify_subdual_damage(&mut self, id: ObjectId, amount: f32) {
        if let Some(h) = self.objects.find(id).and_then(|o| o.helpers.subdual_damage) {
            self.module_call(id, h, ModuleCall::NotifySubdualDamage { amount });
        }
        // If we are gaining subdual damage, we are slowly tinting
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableTint {
                object_id: id,
                status: crate::logic::world::object_state::TINT_STATUS_GAINING_SUBDUAL_DAMAGE,
                on: amount > 0.0,
            });
        }
    }

    /// `doStatusDamage(status, duration)`: the status-damage helper's.
    pub fn do_status_damage(&mut self, id: ObjectId, status: i32, duration: f32) {
        if let Some(h) = self.objects.find(id).and_then(|o| o.helpers.status_damage) {
            self.module_call(id, h, ModuleCall::DoStatusDamage { status, duration });
        }
    }

    /// `doTempWeaponBonus(status, duration)`: the temp-weapon-bonus helper's.
    pub fn do_temp_weapon_bonus(&mut self, id: ObjectId, status: i32, duration: u32) {
        if let Some(h) = self.objects.find(id).and_then(|o| o.helpers.temp_weapon_bonus) {
            self.module_call(id, h, ModuleCall::DoTempWeaponBonus { status, duration });
        }
    }

    // ---- death ----------------------------------------------------------------------------------

    /// `onDie(damageInfo)`: "object level events that need to happen upon
    /// game death". A booby-trapped object detonates its trap first. Every die
    /// module runs, then: the radar entry goes; the terrain decal fades; the
    /// spawner (the producer, if it spawns) is told; the object unlooks and
    /// its team is told; the local player hears of a lost building or unit
    /// (unless self-inflicted); the idle-worker list drops it; and a
    /// reconstructing building hands its attackers to the hole it is being
    /// rebuilt from.
    pub fn on_die(&mut self, id: ObjectId, damage_info: &DamageInfo) {
        self.check_and_detonate_booby_trap(id, None);

        let self_inflicted = damage_info.input.source_id == id;

        // FIRST, call our die modules.
        self.call_die_modules(id, damage_info);

        // When objects die we remove from the radar as they're really not interesting anymore
        if self.objects.find(id).is_some_and(|o| o.on_radar) {
            self.radar_remove_object(id);
        }

        // Just in case I have been sporting one of thise fancy Terrain Decals, I naturally lose it now, because I'm dead.
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableTerrainDecalFadeTarget { object_id: id, target: 0.0, rate: -0.03 });
        }

        // objects that were spawned from something, need to tell their spawner that they have died
        let producer = self.objects.find(id).map_or(INVALID_ID, |o| o.producer_id);
        if self.objects.find(producer).is_some() {
            if let Some(sbi) = self.get_spawn_behavior_interface(producer) {
                self.module_call(producer, sbi, ModuleCall::OnSpawnDeath { dead: id, info: damage_info.clone() });
            }
        }

        self.handle_partition_cell_maintenance(id);
        if let Some(team) = self.get_team(id) {
            self.cut(CutCall::TeamNotifyTeamOfObjectDeath { team });
        }

        if self.is_locally_controlled(id) && !self_inflicted {
            // wasLocallyControlled? :-)
            let o = self.objects.find(id).expect("is_locally_controlled found it");
            if o.is_kind_of(KINDOF_STRUCTURE) && o.is_kind_of(KINDOF_MP_COUNT_FOR_VICTORY) {
                self.emit(ObjectClientEvent::Eva { message: EvaMessage::BuildingLost });
            } else if o.is_kind_of(KINDOF_INFANTRY) || o.is_kind_of(KINDOF_VEHICLE) {
                let position = o.get_position();
                self.emit(ObjectClientEvent::Eva { message: EvaMessage::UnitLost });
                // Create a fake radar event so the user can use the spacebar to quickly jump to this!
                self.emit(ObjectClientEvent::Radar { call: RadarCall::TryFakeEvent, object_id: INVALID_ID, position: Some(position) });
            }
        }

        // This call won't do anything if we aren't actually in the list.
        // Kris: Added NULL check to prevent crash with combat bikes & their riders getting deleted on exit.
        if let Some(player) = self.get_controlling_player(id) {
            self.emit(ObjectClientEvent::RemoveIdleWorker { object_id: id, player_index: player });
        }

        // When a GLA hole is in the process of rebuilding, and that rebuild is lost, we need to
        // tell anyone attacking it to transfer the attack to the hole that still exists.
        if self.objects.find(id).is_some_and(|o| o.test_status(OBJECT_STATUS_RECONSTRUCTING)) {
            let hole_id = self.objects.find(id).map_or(INVALID_ID, |o| o.producer_id);
            if self.objects.find(hole_id).is_some() {
                // set the information in the hole about what to build
                if let Some(rhbi) = self.find_module(hole_id, b"RebuildHoleBehavior") {
                    // start the rebuild process
                    let template = self.objects.find(id).map_or(0, |o| o.template().get_template_id());
                    self.module_call(hole_id, rhbi, ModuleCall::StartRebuildProcess { template, spawner: id });
                }
                // Transfer any attackers from the destroyed building to the hole.
                for obj in self.objects.ids_in_order() {
                    if self.objects.find(obj).is_some_and(|o| o.ai.is_some()) {
                        self.cut(CutCall::AiTransferAttack { object: obj, old_victim: id, new_victim: hole_id });
                    }
                }
            }
        }
    }

    /// `checkAndDetonateBoobyTrap(victim)`: if this object is booby trapped,
    /// finds the sticky bomb it produced near itself (nearest first) and
    /// detonates it, unless the victim is the bomb owner's ally. True if one
    /// went off.
    pub fn check_and_detonate_booby_trap(&mut self, id: ObjectId, victim: Option<ObjectId>) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        if !o.test_status(OBJECT_STATUS_BOOBY_TRAPPED) {
            return false;
        }
        let radius = BOOBY_TRAP_SCAN_RANGE + o.geometry_info.get_bounding_circle_radius();
        let mut our_booby_trap = None;
        for other in self.cut_partition_booby_traps_in_range(id, radius) {
            // Sticky bombs call the thing they are on their producer for just such an occasion
            if self.objects.find(other).is_some_and(|o| o.producer_id == id) {
                our_booby_trap = Some(other);
                break;
            }
        }
        if let Some(trap) = our_booby_trap {
            if let Some(update) = self.find_module(trap, b"StickyBombUpdate") {
                if let Some(victim) = victim {
                    // Friends don't touch friends boobies.
                    let trap_player = self.get_controlling_player(trap);
                    let victim_team = self.get_team(victim);
                    if let Some(p) = trap_player {
                        if self.cuts.player_relationship_to_team(p, victim_team) == Relationship::Allies {
                            return false;
                        }
                    }
                }
                self.module_call(trap, update, ModuleCall::Detonate);
                return true; // Booby Trapped status will be cleared by stickybomb, as they set it
            }
        }
        false
    }

    // ---- veterancy and kills -----------------------------------------------------------------------------

    /// `getVeterancyLevel()`.
    pub fn get_veterancy_level(&self, id: ObjectId) -> VeterancyLevel {
        self.objects.find(id).map_or(VeterancyLevel::Regular, |o| o.get_veterancy_level())
    }

    /// `scoreTheKill(victim)`: "I just killed this object". Nothing for a
    /// victim of a side that is not playable, or ignored in the GUI. The
    /// victim's player loses the object on its score keeper; for an enemy
    /// kill (not of the player's own objects) the killer's player scores it,
    /// gets skill points and bounty, and the killer gains the experience the
    /// victim is worth (not for a victim still under construction).
    pub fn score_the_kill(&mut self, id: ObjectId, victim: ObjectId) {
        let Some(v) = self.objects.find(victim) else { return };
        let Some(victim_controller) = self.get_controlling_player(victim) else { return };
        // if the other player is not a playable side (i.e. they are civilian, observer, whatever)
        // we shouldn't count the kill.
        if !self.cuts.player_is_playable_side(victim_controller) {
            return;
        }
        if v.is_kind_of(KINDOF_IGNORED_IN_GUI) {
            return;
        }
        let controller = self.get_controlling_player(id);
        self.cut(CutCall::PlayerScoreAddObjectLost { player: victim_controller, object: victim });
        if self.get_relationship(id, victim) != Relationship::Enemies {
            return;
        }
        // Don't count kills that I do on my own buildings or units, cause thats just silly.
        if controller == Some(victim_controller) {
            return;
        }
        if let Some(controller) = controller {
            self.cut(CutCall::PlayerScoreAddObjectDestroyed { player: controller, object: victim });
            self.cut(CutCall::PlayerAddSkillPointsForKill { player: controller, killer: id, victim });
            self.cut(CutCall::PlayerDoBountyForKill { player: controller, killer: id, victim });
        }
        // Now handle experience, if we can gain any
        if self.cut_experience_is_accepting_points(id) {
            // srj sez: per dustin, no experience (et al) for killing things under construction.
            if !self.objects.find(victim).is_some_and(|v| v.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION)) {
                let experience_value = self.cut_experience_value(victim, id);
                self.cut_experience_add_points(id, experience_value, true);
            }
        }
    }

    /// `onVeterancyLevelChanged(oldLevel, newLevel, provideFeedback)`: "I
    /// just achieved this level right this moment". The upgrade modules
    /// re-run, the level's veterancy upgrade is given, the body is told, the
    /// weapon-set flags and weapon-bonus conditions follow the new level, and
    /// a promotion (unless hidden by stealth) shows the level-gain animation
    /// and plays the promotion sound.
    pub fn on_veterancy_level_changed(
        &mut self,
        id: ObjectId,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) {
        self.update_upgrade_modules(id);

        if let Some(up) = self.cuts.veterancy_upgrade(new_level) {
            self.give_upgrade(id, Some(&up));
        }

        if let Some(body) = self.get_body_module(id) {
            self.module_call(
                id,
                body,
                ModuleCall::OnVeterancyLevelChanged { old_level: old_level as i32, new_level: new_level as i32, provide_feedback },
            );
        }

        let Some(o) = self.objects.find(id) else { return };
        let hide_animation_for_stealth = !self.is_locally_controlled(id)
            && o.test_status(OBJECT_STATUS_STEALTHED)
            && !o.test_status(OBJECT_STATUS_DETECTED)
            && !o.test_status(OBJECT_STATUS_DISGUISED);
        // First, we plan to do the animation if the level went up
        let mut do_animation = !hide_animation_for_stealth && (new_level as i32 > old_level as i32) && !o.is_kind_of(KINDOF_IGNORED_IN_GUI);

        match new_level {
            VeterancyLevel::Regular => {
                self.clear_weapon_set_flag(id, WEAPONSET_VETERAN);
                self.clear_weapon_set_flag(id, WEAPONSET_ELITE);
                self.clear_weapon_set_flag(id, WEAPONSET_HERO);
                self.clear_weapon_bonus_condition(id, wbc::VETERAN);
                self.clear_weapon_bonus_condition(id, wbc::ELITE);
                self.clear_weapon_bonus_condition(id, wbc::HERO);
                do_animation = false; // ... but not if somehow up to Regular
            }
            VeterancyLevel::Veteran => {
                self.set_weapon_set_flag(id, WEAPONSET_VETERAN);
                self.clear_weapon_set_flag(id, WEAPONSET_ELITE);
                self.clear_weapon_set_flag(id, WEAPONSET_HERO);
                self.set_weapon_bonus_condition(id, wbc::VETERAN);
                self.clear_weapon_bonus_condition(id, wbc::ELITE);
                self.clear_weapon_bonus_condition(id, wbc::HERO);
            }
            VeterancyLevel::Elite => {
                self.clear_weapon_set_flag(id, WEAPONSET_VETERAN);
                self.set_weapon_set_flag(id, WEAPONSET_ELITE);
                self.clear_weapon_set_flag(id, WEAPONSET_HERO);
                self.clear_weapon_bonus_condition(id, wbc::VETERAN);
                self.set_weapon_bonus_condition(id, wbc::ELITE);
                self.clear_weapon_bonus_condition(id, wbc::HERO);
            }
            VeterancyLevel::Heroic => {
                self.clear_weapon_set_flag(id, WEAPONSET_VETERAN);
                self.clear_weapon_set_flag(id, WEAPONSET_ELITE);
                self.set_weapon_set_flag(id, WEAPONSET_HERO);
                self.clear_weapon_bonus_condition(id, wbc::VETERAN);
                self.clear_weapon_bonus_condition(id, wbc::ELITE);
                self.set_weapon_bonus_condition(id, wbc::HERO);
            }
        }

        if do_animation && self.cuts.globals.draw_icon_ui && provide_feedback {
            if self.cuts.globals.level_gain_animation_name_set {
                let o = self.objects.find(id).expect("checked above");
                let mut pos = o.get_position();
                pos.add(&o.health_box_offset);
                self.emit(ObjectClientEvent::LevelGainAnimation { object_id: id, position: pos });
            }
            self.emit(ObjectClientEvent::Sound { sound: ObjectSound::UnitPromoted, object_id: id, position: None });
        }
    }

    // ---- crushing ---------------------------------------------------------------------------------------------

    /// `getCrusherLevel()`.
    pub fn get_crusher_level(&self, id: ObjectId) -> u8 {
        self.objects.find(id).map_or(0, |o| o.template().get_crusher_level())
    }

    /// `getCrushableLevel()`.
    pub fn get_crushable_level(&self, id: ObjectId) -> u8 {
        self.objects.find(id).map_or(0, |o| o.template().get_crushable_level())
    }

    /// `canCrushOrSquish(otherObj, testType)`: returns true iff the object can
    /// run over the other. Not when the other is `None`, when this is unmanned,
    /// when the other is an ally ("friends don't let friends crush friends"),
    /// or when this has no crusher level. A squishable victim (it has a
    /// `SquishCollide` module) is squished; otherwise this must out-rank the
    /// victim's crushable level.
    pub fn can_crush_or_squish(&self, id: ObjectId, other: Option<ObjectId>, test_type: CrushSquishTestType) -> bool {
        let Some(other_id) = other else {
            return false; // Can't crush anything.
        };
        let Some(me) = self.objects.find(id) else { return false };
        if me.is_disabled_by_type(DISABLED_UNMANNED) {
            // Unmanned vehicles cannot crush troops. This was happening when Jarmen Kell sniped
            // the vehicle and booted the guys out while still moving.
            return false;
        }
        let crusher_level = self.get_crusher_level(id);
        // order matters: we want to know if I consider it to be an ally, not vice versa
        if self.get_relationship(id, other_id) == Relationship::Allies {
            return false; // Friends don't let friends crush friends.
        }
        if crusher_level == 0 {
            return false; // Can't crush anything!
        }
        // Test this case for generic infantry getting squished by vehicles!
        if test_type == CrushSquishTestType::SquishOnly || test_type == CrushSquishTestType::CrushOrSquish {
            // See if other is squishable
            if self.objects.find(other_id).is_some_and(|o| o.find_module(b"SquishCollide").is_some()) {
                return true; // squishable.
            }
        }
        let crushable_level = self.get_crushable_level(other_id);
        if (test_type == CrushSquishTestType::CrushOnly || test_type == CrushSquishTestType::CrushOrSquish)
            && crusher_level > crushable_level
        {
            return true;
        }
        false
    }

    /// `topple(toppleDirection, toppleSpeed, options)`: "topple an object, if possible".
    pub fn topple(&mut self, id: ObjectId, direction: Option<Coord3D>, speed: f32, options: u32) {
        if let Some(m) = self.find_module(id, b"ToppleUpdate") {
            if self.module_call(id, m, ModuleCall::IsAbleToBeToppled).as_bool() {
                // apply the topple force
                self.module_call(id, m, ModuleCall::ApplyTopplingForce { direction, speed, options });
            }
        }
    }

    // ---- small queries -------------------------------------------------------------------------------------------

    /// `isMobile()`: "returns true if object is currently able to move".
    pub fn is_mobile(&self, id: ObjectId) -> bool {
        let Some(o) = self.objects.find(id) else { return false };
        if o.is_kind_of(KINDOF_IMMOBILE) {
            return false;
        }
        !o.is_disabled()
    }

    /// `isAbleToAttack()`: "returns true if object currently has some kind of
    /// attack capability". The weapons and the turrets are cut points
    /// (`cuts.weapon_in_slot`, `ai_turret_for_slot`, `ai_turret_enabled`,
    /// `weapon_set_has_any_weapon`).
    pub fn is_able_to_attack(&mut self, id: ObjectId) -> bool {
        //******************************************************
        //********* AUTOMATICALLY FALSE CONDITIONS *************
        //******************************************************
        let Some(o) = self.objects.find(id) else { return false };

        // For things that may or may not be able to normally attack, but are under a status condition
        if o.test_status(OBJECT_STATUS_NO_ATTACK) {
            return false;
        }

        // if we're contained within a transport we cannot attack unless it specifically allows us
        let contained_by = o.contained_by;
        if contained_by != INVALID_ID && self.objects.find(contained_by).is_some() {
            if let Some(c) = self.get_contain(contained_by) {
                if !self.module_call(contained_by, c, ModuleCall::IsPassengerAllowedToFire(id)).as_bool() {
                    return false;
                }
            }
        }

        let o = self.objects.find(id).expect("checked above");
        // We can't fire if under construction
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return false;
        }
        // or being sold
        if o.test_status(OBJECT_STATUS_SOLD) {
            return false;
        }
        if o.is_disabled_by_type(DISABLED_SUBDUED) {
            return false; // A Microwave Tank is cooking me
        }

        // We can't fire if we, as a portable structure, are aptly disabled
        if o.is_kind_of(KINDOF_PORTABLE_STRUCTURE) || o.is_kind_of(KINDOF_SPAWNS_ARE_THE_WEAPONS) {
            if o.is_disabled_by_type(DISABLED_HACKED) || o.is_disabled_by_type(DISABLED_EMP) {
                return false;
            }
            if o.is_kind_of(KINDOF_INFANTRY) {
                // I must be a stinger soldier or similar
                for idx in self.all_modules(id) {
                    // expensive search, limited only to stinger soldiers
                    if let ModuleReply::Id(slaver_id) = self.module_call(id, idx, ModuleCall::GetSlaverId) {
                        if slaver_id != INVALID_ID {
                            if let Some(slaver) = self.objects.find(slaver_id) {
                                if slaver.is_disabled_by_type(DISABLED_SUBDUED) {
                                    return false; // if my stinger site is subdued, so am I
                                }
                            }
                        }
                        break; // only expect one slavedupdate, so stop searching
                    }
                }
            }
        }

        // We can't fire if all our weapons are disabled! Currently, only turreted weapons can be disabled.
        // ONLY DO THIS CHECK IF OUR UNIT DOESN'T HAVE THE KINDOF_CAN_ATTACK flag...
        let o = self.objects.find(id).expect("checked above");
        let mut any_enabled = false;
        let mut any_weapon = false;
        if o.ai.is_some() && !o.is_kind_of(KINDOF_CAN_ATTACK) {
            for i in 0..WEAPONSLOT_COUNT {
                // Find the weapon in this slot.
                // (the object's own weapon, or one a cut table gives it: the tables are how a test world gives an object weapons)
                if o.weapon_set.get_weapon_in_weapon_slot(i).is_none() && !self.cuts.weapon_in_slot(id, i) {
                    continue;
                }
                any_weapon = true;
                // We found a weapon, is it a turret?
                match self.cuts.ai_turret_for_slot(id, i) {
                    None => {
                        // Currently impossible to disable a non-turreted weapon, so we
                        // have a non turreted weapon that is enabled. Quit.
                        any_enabled = true;
                        break;
                    }
                    Some(tur) => {
                        if self.cuts.ai_turret_enabled(id, tur) {
                            // The turret is enable, meaning we have an enabled weapon. Quit.
                            any_enabled = true;
                            break;
                        }
                    }
                }
            }
            if any_weapon && !any_enabled {
                // We failed to find any active weapons.
                return false;
            }
        }

        //***************************************
        //********* TRUE CONDITIONS *************
        //***************************************
        let o = self.objects.find(id).expect("checked above");
        // for certain buildings
        if o.is_kind_of(KINDOF_CAN_ATTACK) {
            return true;
        }
        // for garrisonned buildings that can attack sometimes
        if o.test_status(OBJECT_STATUS_CAN_ATTACK) {
            return true;
        }
        // for weaponless transports. This will make me think I can, but I will check if I literally can by looking
        // at passenger weapons in CanAttack.
        if let Some(contain) = self.get_contain(id) {
            if self.module_call(id, contain, ModuleCall::IsPassengerAllowedToFire(id)).as_bool()
                && self.module_call(id, contain, ModuleCall::GetContainCount).as_int(0) > 0
            {
                return true;
            }
        }
        // if we have AI and a weapon, assume we know how to use it
        let o = self.objects.find(id).expect("checked above");
        if o.ai.is_some() && (o.weapon_set.has_any_weapon() || self.cuts.weapon_set_has_any_weapon(id)) {
            return true;
        }
        if let Some(spawn) = self.get_spawn_behavior_interface(id) {
            if self.module_call(id, spawn, ModuleCall::CanAnySlavesAttack).as_bool() {
                return true;
            }
        }
        if self.objects.find(id).is_some_and(|o| o.template().is_enter_guard()) {
            return true;
        }
        // Default is no
        false
    }

    // ---- model conditions (the client's drawable) -----------------------------------------------------------------

    /// `setModelConditionState(a)`: the drawable's, nothing without one.
    pub fn set_model_condition_state(&mut self, id: ObjectId, a: i32) {
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableModelCondition { object_id: id, op: ModelConditionOp::Set(vec![a]) });
        }
    }

    /// `clearModelConditionState(a)`.
    pub fn clear_model_condition_state(&mut self, id: ObjectId, a: i32) {
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableModelCondition { object_id: id, op: ModelConditionOp::Clear(vec![a]) });
        }
    }

    /// `clearAndSetModelConditionState(clr, set)`.
    pub fn clear_and_set_model_condition_state(&mut self, id: ObjectId, clr: i32, set: i32) {
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableModelCondition {
                object_id: id,
                op: ModelConditionOp::ClearAndSet { clear: vec![clr], set: vec![set] },
            });
        }
    }

    /// `setSpecialModelConditionState(set, frames)`: "special model states
    /// are states that are turned on for a period of time, and turned off
    /// automatically -- used for cheer, and scripted special moment
    /// animations". Setting one clears any other.
    pub fn set_special_model_condition_state(&mut self, id: ObjectId, set: i32, frames: u32) {
        self.clear_special_model_condition_states(id);
        self.set_model_condition_state(id, set);
        let frames = if frames == 0 { 1 } else { frames };
        let until = self.frame().wrapping_add(frames);
        if let Some(o) = self.objects.find_mut(id) {
            o.smc_until = until;
        }
        self.helper_sleep_until(id, crate::logic::world::object::HelperKind::Smc, until);
    }

    /// `clearSpecialModelConditionStates()`.
    pub fn clear_special_model_condition_states(&mut self, id: ObjectId) {
        if self.has_drawable(id) {
            self.emit(ObjectClientEvent::DrawableModelCondition {
                object_id: id,
                op: ModelConditionOp::Clear(vec![crate::common::model_state::MODELCONDITION_SPECIAL_CHEERING]),
            });
        }
        if let Some(o) = self.objects.find_mut(id) {
            o.smc_until = crate::common::game_common::NEVER;
        }
    }
}
