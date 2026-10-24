// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/Player.h
//   GeneralsMD/Code/GameEngine/Source/Common/RTS/Player.cpp
//   (slice 2: the half of `Player` that works through objects: team membership, the local player, the shortcut-power
//   searches, hunting, garrisoning, selling, bounty, what can be built, the difficulty and battle-plan bonuses, vision
//   spying, upgrades, the hotkey squads)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Squad.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/Squad.cpp
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! Slice 2 of `Player` (`p1w10-sonnet`): the operations that act on, or search, the objects of a player, as free
//! functions over [`PlayerObjectHost`] (a [`PlayerHost`] that also answers what these ask of objects, the AI, the
//! action manager, the build assistant, the partition manager, the body and the client), and the `Squad` they use for
//! the hotkey groups. Read `player_ops.rs` first: the shape and the conventions are the same.
//!
//! # What is kept from upstream
//!
//! - **The searches go through every object, whatever they have found** (`iterateObjects` has no early exit):
//!   `findNaturalCommandCenter` keeps the *first* match, `findMostReady...` keeps the first of the highest.
//! - **`doFindSpecialPowerSourceObject` falls to its second branch for `SPECIAL_INVALID` on an object with no special
//!   power at all**, asking `hasSpecialPower(SPECIAL_INVALID)`; kept.
//! - **`Player::killPlayer` evacuates every team, marks the player dead, then kills every team**, and an AI player in a
//!   single-player game is revived (`m_isPlayerDead = FALSE`) and returns before its money is taken.
//! - **`removeUpgrade` unlinks the upgrade but never deletes it**, and its `markUIDirty` is inside the `if`.
//! - **`changeBattlePlan` rewrites the `bonus` it is given** when the last strategy centre of a plan goes (the
//!   inverse scalars, the plans turned negative): the caller sees the changed bonus.
//! - **`applyBattlePlanBonusesForObject` asks the *object* for its vision range** when it changes the *modified*
//!   object's (they differ for a projectile).
//! - **A deposit's income is recorded for the player the money was made for**, not the depositing player (see
//!   `player_ops.rs`, `init`).
//!
//! # Where upstream is undefined
//!
//! A `NULL` local player, a player that has no template where its beacon is asked, a hotkey index outside the table
//! where upstream indexes it: [`PlayerError`](super::player_ops::PlayerError).

use crate::common::base_type::{real_to_int_ceil, Coord3D};
use crate::common::game_common::{PlayerType, Relationship};
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{
    KindOfMaskType, KINDOF_COMMANDCENTER, KINDOF_DISGUISER, KINDOF_DOZER, KINDOF_FS_POWER, KINDOF_HARVESTER, KINDOF_IGNORES_SELECT_ALL, KINDOF_PROJECTILE,
    KINDOF_STRUCTURE,
};
use crate::common::name_key_generator::NameKeyType;
use crate::common::object_status_types::{OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use crate::common::special_power_type::{
    SpecialPowerType, LAZR_SPECIAL_PARTICLE_UPLINK_CANNON, NUKE_SPECIAL_NEUTRON_MISSILE, SPECIAL_INVALID, SPECIAL_NEUTRON_MISSILE, SPECIAL_PARTICLE_UPLINK_CANNON,
    SPECIAL_SCUD_STORM, SUPW_SPECIAL_NEUTRON_MISSILE, SUPW_SPECIAL_PARTICLE_UPLINK_CANNON,
};
use crate::logic::client_event::ClientEvent;
use crate::logic::weapon_bonus::condition as wbc;

use super::ids::PlayerIndex;
use super::player::*;
use super::player_ops::*;
use super::team_ops::*;

/// `CommandSourceType` (`GameCommon.h`): who gave an order. An `i32` like the enum.
pub type CommandSource = i32;
/// `CMD_FROM_SCRIPT`.
pub const CMD_FROM_SCRIPT: CommandSource = 1;

/// `BodyModuleInterface::setMaxHealth`'s `MaxHealthChangeType` that `Player` uses: `PRESERVE_RATIO`.
pub const PRESERVE_RATIO: i32 = 1;

/// `ALL_PLANS` (`BattlePlanUpdate.h`).
pub const ALL_PLANS: i32 = 1_000_000;

/// `BattlePlanStatus` (`BattlePlanUpdate.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum BattlePlanStatus {
    /// `PLANSTATUS_NONE`.
    None = 0,
    /// `PLANSTATUS_BOMBARDMENT`.
    Bombardment = 1,
    /// `PLANSTATUS_HOLDTHELINE`.
    HoldTheLine = 2,
    /// `PLANSTATUS_SEARCHANDDESTROY`.
    SearchAndDestroy = 3,
}

/// The `BuildableStatus` values `canBuild` tests (`ThingTemplate.h`).
pub const BSTATUS_YES: u8 = 0;
/// `BSTATUS_IGNORE_PREREQUISITES`.
pub const BSTATUS_IGNORE_PREREQUISITES: u8 = 1;
/// `BSTATUS_NO`.
pub const BSTATUS_NO: u8 = 2;
/// `BSTATUS_ONLY_BY_AI`.
pub const BSTATUS_ONLY_BY_AI: u8 = 3;

/// What `Player` asks of objects and the services of slice 2, beyond [`PlayerHost`].
pub trait PlayerObjectHost: PlayerHost {
    /// `TheGameLogic->findObjectByID(id)` as an id (`None` for `NULL`).
    fn find_object_by_id(&self, id: ObjectId) -> Option<ObjectId>;
    /// `obj->friend_adjustPowerForPlayer(yes)`.
    fn obj_friend_adjust_power_for_player(&mut self, o: ObjectId, yes: bool);
    /// `obj->findUpdateModule(key) != NULL` for the `AutoDepositUpdate` key.
    fn obj_has_auto_deposit_update(&self, o: ObjectId, key: NameKeyType) -> bool;
    /// `adu->awardInitialCaptureBonus(player)`.
    fn auto_deposit_award_initial_capture_bonus(&mut self, o: ObjectId, p: PlayerIndex);
    /// `obj->areModulesReady()`.
    fn obj_modules_ready(&self, o: ObjectId) -> bool;
    /// `TheInGameUI->addIdleWorker(obj)`.
    fn ui_add_idle_worker(&mut self, o: ObjectId);
    /// `TheInGameUI->removeIdleWorker(obj, playerIndex)`.
    fn ui_remove_idle_worker(&mut self, o: ObjectId, player: PlayerIndex);
    /// `tmplate->getDefaultOwningSide()`.
    fn tmpl_default_owning_side(&self, t: TemplateRef) -> Vec<u8>;
    /// `obj->getDrawable() != NULL`.
    fn obj_has_drawable(&self, o: ObjectId) -> bool;
    /// `obj->getStealth()` that `isDisguised()`: its `getDisguisedPlayerIndex()`; `None` for no stealth update or one
    /// that is not disguised.
    fn obj_stealth_disguised_player_index(&self, o: ObjectId) -> Option<PlayerIndex>;
    /// `obj->getNightIndicatorColor()`.
    fn obj_night_indicator_color(&self, o: ObjectId) -> u32;
    /// `obj->getIndicatorColor()`.
    fn obj_indicator_color(&self, o: ObjectId) -> u32;
    /// `obj->isDisabled()`.
    fn obj_is_disabled(&self, o: ObjectId) -> bool;
    /// `obj->hasAnySpecialPower()`.
    fn obj_has_any_special_power(&self, o: ObjectId) -> bool;
    /// `obj->findAnyShortcutSpecialPowerModuleInterface()`.
    fn obj_find_any_shortcut_special_power(&self, o: ObjectId) -> Option<SpecialPowerModuleRef>;
    /// `obj->hasSpecialPower(type)`.
    fn obj_has_special_power(&self, o: ObjectId, t: SpecialPowerType) -> bool;
    /// `obj->findSpecialPowerModuleInterface(type)`.
    fn obj_find_special_power(&self, o: ObjectId, t: SpecialPowerType) -> Option<SpecialPowerModuleRef>;
    /// `sp->isScriptOnly()`.
    fn sp_is_script_only(&self, m: SpecialPowerModuleRef) -> bool;
    /// `sp->getReadyFrame()`.
    fn sp_ready_frame(&self, m: SpecialPowerModuleRef) -> u32;
    /// `sp->getPercentReady()`.
    fn sp_percent_ready(&self, m: SpecialPowerModuleRef) -> u32;
    /// `sp->getSpecialPowerTemplate()->isSharedNSync()`.
    fn sp_is_shared_n_sync(&self, m: SpecialPowerModuleRef) -> bool;
    /// `obj->hasAnyWeapon()`.
    fn obj_has_any_weapon(&self, o: ObjectId) -> bool;
    /// `obj->getMostPercentReadyToFireAnyWeapon()`.
    fn obj_most_percent_ready_to_fire_any_weapon(&self, o: ObjectId) -> u32;
    /// `obj->leaveGroup()`.
    fn obj_leave_group(&mut self, o: ObjectId);
    /// `obj->isAnyKindOf(mask)`.
    fn obj_is_any_kind_of(&self, o: ObjectId, mask: &KindOfMaskType) -> bool;
    /// `ai->aiHunt(source)`.
    fn ai_hunt(&mut self, o: ObjectId, source: CommandSource);
    /// `ai->aiIdle(source)`.
    fn ai_idle(&mut self, o: ObjectId, source: CommandSource);
    /// `ai->aiEnter(building, source)`.
    fn ai_enter(&mut self, o: ObjectId, building: ObjectId, source: CommandSource);
    /// `ai->aiEvacuate(exposeStealthUnits, source)`.
    fn ai_evacuate(&mut self, o: ObjectId, expose_stealth_units: bool, source: CommandSource);
    /// `ai->aiMoveToPosition(pos, source)`.
    fn ai_move_to_position(&mut self, o: ObjectId, pos: Coord3D, source: CommandSource);
    /// `ai->getSupplyTruckAIInterface() != NULL`.
    fn ai_has_supply_truck_interface(&self, o: ObjectId) -> bool;
    /// `supplyTruckAI->setForceWantingState(true)`.
    fn ai_supply_truck_set_force_wanting_state(&mut self, o: ObjectId, v: bool);
    /// `TheAI->pathfinder()->removeObjectFromPathfindMap(obj)`.
    fn pathfinder_remove_object(&mut self, o: ObjectId);
    /// `TheAI->pathfinder()->addObjectToPathfindMap(obj)`.
    fn pathfinder_add_object(&mut self, o: ObjectId);
    /// `ThePartitionManager->getMostValuableLocation(playerIndex, ALLOW_ENEMIES, VOT_CashValue, &pos)` (the answer
    /// is not used).
    fn partition_get_most_valuable_location(&mut self, p: PlayerIndex);
    /// The structures of `ThePartitionManager->iterateObjectsInRange(&pos, 1e9, FROM_CENTER_3D, {AcceptByKindOf
    /// (STRUCTURE)}, ITER_SORTED_NEAR_TO_FAR)`, in iteration order, taken once.
    fn partition_structures_near_to_far(&mut self, pos: Coord3D) -> Vec<ObjectId>;
    /// `ThePartitionManager->iterateAllObjects()`: every object, in the partition manager's order.
    fn partition_all_objects(&self) -> Vec<ObjectId>;
    /// `building->getContain()->getPlayerWhoEntered()`: `None` without a contain module.
    fn contain_player_who_entered(&self, building: ObjectId) -> Option<u32>;
    /// `contain->recalcApparentControllingPlayer()`.
    fn contain_recalc_apparent_controlling_player(&mut self, o: ObjectId);
    /// `obj->getContain() != NULL`.
    fn obj_has_contain(&self, o: ObjectId) -> bool;
    /// `TheActionManager->canEnterObject(obj, building, source, CHECK_CAPACITY)`.
    fn action_can_enter_object(&mut self, o: ObjectId, building: ObjectId, source: CommandSource) -> bool;
    /// `TheBuildAssistant->sellObject(obj)`.
    fn build_assistant_sell_object(&mut self, o: ObjectId);
    /// `obj->isFactionStructure()`.
    fn obj_is_faction_structure(&self, o: ObjectId) -> bool;
    /// `TheThingFactory->findTemplate(name)`.
    fn thing_factory_find_template(&mut self, name: &[u8]) -> Option<TemplateRef>;
    /// `m_scoreKeeper.addMoneySpent(amount)`.
    fn score_add_money_spent(&mut self, p: PlayerIndex, amount: i32);
    /// `m_scoreKeeper.addMoneyEarned(amount)`.
    fn score_add_money_earned(&mut self, p: PlayerIndex, amount: i32);
    /// `tmplate->getBuildCost()`.
    fn tmpl_build_cost(&self, t: TemplateRef) -> i32;
    /// `tmplate->getKindOfMask()` (the `m_kindof` that `calcCostToBuild` hands on).
    fn tmpl_kind_of_mask(&self, t: TemplateRef) -> KindOfMaskType;
    /// `obj->getProducerID()`.
    fn obj_producer_id(&self, o: ObjectId) -> ObjectId;
    /// `body->getMaxHealth()`.
    fn obj_body_max_health(&self, o: ObjectId) -> f32;
    /// `body->setMaxHealth(health, PRESERVE_RATIO)`.
    fn obj_body_set_max_health(&mut self, o: ObjectId, health: f32);
    /// `body->applyDamageScalar(scalar)`.
    fn obj_body_apply_damage_scalar(&mut self, o: ObjectId, scalar: f32);
    /// `obj->setWeaponBonusCondition(c)`.
    fn obj_set_weapon_bonus_condition(&mut self, o: ObjectId, c: i32);
    /// `obj->clearWeaponBonusCondition(c)`.
    fn obj_clear_weapon_bonus_condition(&mut self, o: ObjectId, c: i32);
    /// `obj->getShroudClearingRange()`.
    fn obj_shroud_clearing_range(&self, o: ObjectId) -> f32;
    /// `obj->setVisionRange(range)`.
    fn obj_set_vision_range(&mut self, o: ObjectId, range: f32);
    /// `obj->setShroudClearingRange(range)`.
    fn obj_set_shroud_clearing_range(&mut self, o: ObjectId, range: f32);
    /// `obj->setVisionSpied(setting, byWhom)`.
    fn obj_set_vision_spied(&mut self, o: ObjectId, setting: bool, by_whom: PlayerIndex);
    /// `tmplate->getBuildable()`.
    fn tmpl_buildable(&self, t: TemplateRef) -> u8;
    /// `tmplate->getPrereqCount()`.
    fn tmpl_prereq_count(&self, t: TemplateRef) -> i32;
    /// `tmplate->getNthPrereq(i)->isSatisfied(player)`.
    fn tmpl_prereq_is_satisfied(&self, t: TemplateRef, i: i32, p: PlayerIndex) -> bool;
    /// `tmplate->getMaxSimultaneousOfType()`.
    fn tmpl_max_simultaneous_of_type(&self, t: TemplateRef) -> u32;
    /// `tmplate->getMaxSimultaneousLinkKey()`.
    fn tmpl_max_simultaneous_link_key(&self, t: TemplateRef) -> NameKeyType;
    /// `ProductionUpdate::getProductionUpdateInterfaceFromObject(obj)->countUnitTypeInQueue(type)`: `None` without the
    /// production interface.
    fn obj_production_queue_count(&self, o: ObjectId, t: TemplateRef) -> Option<u32>;
    /// `ThePlayerTemplateStore->findPlayerTemplate(key)`.
    fn player_template_find(&self, key: NameKeyType) -> Option<PlayerTemplateRef>;
}


fn pl<H: PlayerObjectHost>(h: &H, p: PlayerIndex) -> PResult<&Player> {
    h.players().get_nth(p).ok_or(PlayerError::NoPlayer)
}

fn plm<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex) -> PResult<&mut Player> {
    h.players_mut().get_nth_mut(p).ok_or(PlayerError::NoPlayer)
}

/// The team instances' members of every prototype of the player, as `for_each_member` of `player_ops.rs` walks them.
pub(super) fn for_each_member<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, mut body: impl FnMut(&mut H, ObjectId)) -> PResult<()> {
    for proto in pl(h, p)?.team_prototypes.clone() {
        let mut cur_t = h.teams().first_instance(proto);
        while let Some(t) = cur_t {
            let mut cur_o = h.teams().first_member(t);
            while let Some(o) = cur_o {
                body(h, o);
                cur_o = h.teams().next_member(o);
            }
            cur_t = h.teams().next_instance(t);
        }
    }
    Ok(())
}

fn not_under_construction_or_sold<H: PlayerObjectHost>(h: &H, o: ObjectId) -> bool {
    !h.obj_test_status(o, OBJECT_STATUS_UNDER_CONSTRUCTION) && !h.obj_test_status(o, OBJECT_STATUS_SOLD)
}

// ---- team membership and the local player ---------------------------------------------------------------------------

/// `Player::becomingTeamMember(obj, yes)`: the object has just become (or ceased to be) a member of one of the
/// player's teams.
pub fn player_becoming_team_member<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, obj: Option<ObjectId>, yes: bool) -> PResult<()> {
    let Some(obj) = obj else { return Ok(()) };
    if !h.obj_test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION) || plant(101) {
        h.obj_friend_adjust_power_for_player(obj, yes);
    }
    if (p != 0 || plant(102)) && yes {
        let key_auto_deposit_update = h.name_to_key(b"AutoDepositUpdate");
        if h.obj_has_auto_deposit_update(obj, key_auto_deposit_update) {
            h.auto_deposit_award_initial_capture_bonus(obj, p);
        }
    }
    if player_get_num_battle_plans_active(h, p)? > 0 && (h.obj_modules_ready(obj) || plant(103)) {
        if yes != plant(104) {
            player_apply_battle_plan_bonuses_for_object(h, p, obj)?;
        } else {
            player_remove_battle_plan_bonuses_for_object(h, p, obj)?;
        }
    }
    if h.obj_is_kind_of(obj, KINDOF_DOZER) && h.obj_has_ai(obj) && (h.ai_is_idle(obj) || plant(105)) {
        if yes {
            h.ui_add_idle_worker(obj);
        } else {
            h.ui_remove_idle_worker(obj, if plant(106) { 0 } else { p });
        }
    }
    Ok(())
}

/// `Player::becomingLocalPlayer(yes)`: the player becomes (or ceases to be) the local player.
pub fn player_becoming_local_player<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, yes: bool) -> PResult<()> {
    if yes {
        // TheGameClient->setTeamColor(REAL_TO_INT(rgb.red*255), ...) with rgb.setFromInt(m_color)
        let color = pl(h, p)?.color;
        let mut rgb = crate::common::base_type::RGBColor::default();
        rgb.set_from_int(color as i32);
        h.client_event(ClientEvent::Player(PlayerClientEvent::SetTeamColor {
            red: crate::common::base_type::real_to_int(rgb.red * if plant(107) { 256.0 } else { 255.0 }),
            green: crate::common::base_type::real_to_int(rgb.green * 255.0),
            blue: crate::common::base_type::real_to_int(rgb.blue * 255.0),
        }));
        // ThePartitionManager
        for object in h.partition_all_objects() {
            if h.obj_has_contain(object) {
                h.contain_recalc_apparent_controlling_player(object);
                h.client_event(ClientEvent::Player(PlayerClientEvent::RadarRemoveObject { object }));
                h.client_event(ClientEvent::Player(PlayerClientEvent::RadarAddObject { object }));
            }
            if h.obj_is_kind_of(object, KINDOF_DISGUISER) && (h.obj_has_drawable(object) || plant(108)) {
                if let Some(disguised) = h.obj_stealth_disguised_player_index(object) {
                    let relationship = player_get_relationship(h, p, h.obj_team(object))?;
                    let night = h.globals().time_of_day_is_night;
                    let color = if (relationship != Relationship::Allies || plant(109)) && player_is_player_active(h, p)? {
                        let d = pl(h, disguised)?;
                        if night {
                            d.night_color
                        } else {
                            d.color
                        }
                    } else if night != plant(110) {
                        h.obj_night_indicator_color(object)
                    } else {
                        h.obj_indicator_color(object)
                    };
                    h.client_event(ClientEvent::Player(PlayerClientEvent::IndicatorColor { object, color }));
                    h.client_event(ClientEvent::Player(PlayerClientEvent::RadarRemoveObject { object }));
                    h.client_event(ClientEvent::Player(PlayerClientEvent::RadarAddObject { object }));
                }
            }
        }
        // TheControlBar
        h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty));
    }
    Ok(())
}

// ---- the shortcut-power searches ---------------------------------------------------------------------------------

/// `Player::findNaturalCommandCenter()`: the first command centre of the player's own side that is neither being
/// built nor sold.
pub fn player_find_natural_command_center<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex) -> PResult<Option<ObjectId>> {
    let side = pl(h, p)?.side.clone();
    let mut found: Option<ObjectId> = None;
    for_each_member(h, p, |h, obj| {
        if found.is_none()
            && h.obj_is_kind_of(obj, KINDOF_COMMANDCENTER)
            && (plant(111) || h.obj_template(obj).is_some_and(|t| h.tmpl_default_owning_side(t) == side))
            && !h.obj_test_status(obj, OBJECT_STATUS_UNDER_CONSTRUCTION)
            && (!h.obj_test_status(obj, OBJECT_STATUS_SOLD) || plant(112))
        {
            found = Some(obj);
        }
    })?;
    Ok(found)
}

/// `doFindSpecialPowerSourceObject` over the player's objects with the given type (`SPECIAL_INVALID` = any), starting
/// from `lowestReadyFrame = 0xffffffff`.
fn find_special_power_source<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, sp_type: SpecialPowerType) -> PResult<Option<ObjectId>> {
    let mut found: Option<ObjectId> = None;
    let mut lowest_ready_frame: u32 = 0xffff_ffff;
    for_each_member(h, p, |h, obj| {
        if lowest_ready_frame == 0 && !plant(113) {
            return;
        }
        if not_under_construction_or_sold(h, obj) && !h.obj_is_effectively_dead(obj) {
            if sp_type == SPECIAL_INVALID && h.obj_has_any_special_power(obj) {
                if let Some(m) = if plant(114) { h.obj_find_special_power(obj, sp_type) } else { h.obj_find_any_shortcut_special_power(obj) } {
                    if !h.sp_is_script_only(m) {
                        found = Some(obj);
                        lowest_ready_frame = 0;
                    }
                }
            } else if h.obj_has_special_power(obj, sp_type) {
                if let Some(m) = h.obj_find_special_power(obj, sp_type) {
                    if !h.sp_is_script_only(m) {
                        let mut ready_frame = h.sp_ready_frame(m);
                        if h.obj_is_disabled(obj) && !plant(115) {
                            ready_frame = u32::MAX - 10;
                        }
                        if if plant(116) { ready_frame <= h.frame() } else { ready_frame < h.frame() } {
                            found = Some(obj);
                            lowest_ready_frame = 0;
                        } else if if plant(117) { ready_frame <= lowest_ready_frame } else { ready_frame < lowest_ready_frame } {
                            found = Some(obj);
                            lowest_ready_frame = ready_frame;
                        }
                    }
                }
            }
        }
    })?;
    Ok(found)
}

/// `Player::findMostReadyShortcutSpecialPowerOfType(spType)`.
pub fn player_find_most_ready_shortcut_special_power_of_type<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, sp_type: SpecialPowerType) -> PResult<Option<ObjectId>> {
    find_special_power_source(h, p, sp_type)
}

/// `Player::hasAnyShortcutSpecialPower()`.
pub fn player_has_any_shortcut_special_power<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex) -> PResult<bool> {
    Ok(find_special_power_source(h, p, SPECIAL_INVALID)?.is_some())
}

/// `Player::countReadyShortcutSpecialPowersOfType(spType)`: `doCountSpecialPowersReady` (a shared-and-synced power counts
/// once).
pub fn player_count_ready_shortcut_special_powers_of_type<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, sp_type: SpecialPowerType) -> PResult<i32> {
    let mut num_ready: u32 = 0;
    for_each_member(h, p, |h, obj| {
        if not_under_construction_or_sold(h, obj) && !h.obj_is_effectively_dead(obj) && h.obj_has_special_power(obj, sp_type) {
            if let Some(m) = h.obj_find_special_power(obj, sp_type) {
                if !h.sp_is_script_only(m) {
                    if h.sp_is_shared_n_sync(m) && num_ready == if plant(118) { 2 } else { 1 } {
                        return;
                    }
                    let mut ready_frame = h.sp_ready_frame(m);
                    if h.obj_is_disabled(obj) && !plant(176) {
                        ready_frame = u32::MAX - 10;
                    }
                    if (if plant(177) { ready_frame <= h.frame() } else { ready_frame < h.frame() }) || plant(119) {
                        num_ready += 1;
                    }
                }
            }
        }
    })?;
    Ok(num_ready as i32)
}

/// `Player::findMostReadyShortcutWeaponForThing(thing, mostReadyPercentage)`: the object and the highest percentage.
pub fn player_find_most_ready_shortcut_weapon_for_thing<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, thing: TemplateRef) -> PResult<(Option<ObjectId>, u32)> {
    let mut found: Option<ObjectId> = None;
    let mut highest: u32 = 0;
    for_each_member(h, p, |h, obj| {
        if highest >= if plant(120) { 101 } else { 100 } {
            return;
        }
        let equivalent = h.obj_template(obj).is_some_and(|t| h.tmpl_is_equivalent_to(thing, Some(t)));
        if equivalent && not_under_construction_or_sold(h, obj) && !h.obj_is_effectively_dead(obj) && h.obj_has_any_weapon(obj) {
            let percentage = h.obj_most_percent_ready_to_fire_any_weapon(obj);
            if if plant(121) { percentage >= highest } else { percentage > highest } {
                found = Some(obj);
                highest = percentage;
            }
        }
    })?;
    Ok((found, highest))
}

/// `Player::findMostReadyShortcutSpecialPowerForThing(thing, mostReadyPercentage)`.
pub fn player_find_most_ready_shortcut_special_power_for_thing<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, thing: TemplateRef) -> PResult<(Option<ObjectId>, u32)> {
    let mut found: Option<ObjectId> = None;
    let mut highest: u32 = 0;
    for_each_member(h, p, |h, obj| {
        if highest >= 100 {
            return;
        }
        let equivalent = h.obj_template(obj).is_some_and(|t| h.tmpl_is_equivalent_to(thing, Some(t)));
        if equivalent && not_under_construction_or_sold(h, obj) && !h.obj_is_effectively_dead(obj) {
            for m in h.obj_special_power_modules(obj) {
                let percentage = h.sp_percent_ready(m);
                if percentage > highest || (plant(122) && found.is_none()) {
                    found = Some(obj);
                    highest = percentage;
                }
            }
        }
    })?;
    Ok((found, highest))
}

/// `Player::findAnyExistingObjectWithThingTemplate(thing)`.
pub fn player_find_any_existing_object_with_thing_template<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, thing: TemplateRef) -> PResult<Option<ObjectId>> {
    let mut found: Option<ObjectId> = None;
    for_each_member(h, p, |h, obj| {
        if found.is_some() {
            return;
        }
        let equivalent = h.obj_template(obj).is_some_and(|t| h.tmpl_is_equivalent_to(thing, Some(t)));
        if equivalent && (not_under_construction_or_sold(h, obj) || plant(123)) && !h.obj_is_effectively_dead(obj) {
            found = Some(obj);
        }
    })?;
    Ok(found)
}

// ---- structures, units and orders --------------------------------------------------------------------------------

/// `Player::onStructureConstructionComplete(builder, structure, isRebuild)`.
pub fn player_on_structure_construction_complete<H: PlayerObjectHost>(
    h: &mut H,
    p: PlayerIndex,
    builder: Option<ObjectId>,
    structure: ObjectId,
    is_rebuild: bool,
) -> PResult<()> {
    h.script_notify_of_object_creation_or_destruction();
    h.pathfinder_remove_object(structure);
    h.pathfinder_add_object(structure);
    if !is_rebuild {
        if !plant(124) {
            h.score_add_object_built(p, Some(structure));
        }
        let cost = match h.obj_template(structure) {
            Some(t) => player_calc_cost_to_build(h, p, t)?,
            None => 0,
        };
        h.score_add_money_spent(p, cost);
    }
    h.obj_friend_adjust_power_for_player(structure, true);
    if pl(h, p)?.ai.is_some() && !plant(125) {
        h.ai_on_structure_produced(p, builder, Some(structure));
    }
    h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarDirty));
    let local = h.players().local_index();
    let controlling = h.obj_controlling_player(structure);
    let eva_for = |h: &mut H, own: EvaMessage, ally: EvaMessage, enemy: EvaMessage| -> PResult<()> {
        let m = if (local == controlling && local.is_some() || (local.is_none() && controlling.is_none())) && !plant(126) {
            own
        } else {
            let lp = local.ok_or(PlayerError::NoPlayer)?;
            if (player_get_relationship(h, lp, h.obj_team(structure))? != Relationship::Enemies) != plant(127) {
                ally
            } else {
                enemy
            }
        };
        h.client_event(ClientEvent::Player(PlayerClientEvent::Eva(m)));
        Ok(())
    };
    if h.obj_has_special_power(structure, SPECIAL_PARTICLE_UPLINK_CANNON)
        || h.obj_has_special_power(structure, SUPW_SPECIAL_PARTICLE_UPLINK_CANNON)
        || (h.obj_has_special_power(structure, LAZR_SPECIAL_PARTICLE_UPLINK_CANNON) && !plant(128))
    {
        eva_for(h, EvaMessage::SuperweaponDetectedOwnParticleCannon, EvaMessage::SuperweaponDetectedAllyParticleCannon, EvaMessage::SuperweaponDetectedEnemyParticleCannon)?;
    }
    if h.obj_has_special_power(structure, SPECIAL_NEUTRON_MISSILE)
        || h.obj_has_special_power(structure, NUKE_SPECIAL_NEUTRON_MISSILE)
        || (h.obj_has_special_power(structure, SUPW_SPECIAL_NEUTRON_MISSILE) && !plant(129))
    {
        eva_for(h, EvaMessage::SuperweaponDetectedOwnNuke, EvaMessage::SuperweaponDetectedAllyNuke, EvaMessage::SuperweaponDetectedEnemyNuke)?;
    }
    if h.obj_has_special_power(structure, SPECIAL_SCUD_STORM) {
        eva_for(h, EvaMessage::SuperweaponDetectedOwnScudStorm, EvaMessage::SuperweaponDetectedAllyScudStorm, EvaMessage::SuperweaponDetectedEnemyScudStorm)?;
    }
    Ok(())
}

/// `Player::setUnitsShouldHunt(unitsShouldHunt, source)`.
pub fn player_set_units_should_hunt<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, units_should_hunt: bool, source: CommandSource) -> PResult<()> {
    plm(h, p)?.units_should_hunt = units_should_hunt;
    h.partition_get_most_valuable_location(p);
    let mut disqualifying = KindOfMaskType::new();
    disqualifying.set(KINDOF_DOZER, 1);
    if !plant(130) {
        disqualifying.set(KINDOF_HARVESTER, 1);
    }
    disqualifying.set(KINDOF_IGNORES_SELECT_ALL, 1);
    for_each_member(h, p, |h, obj| {
        if h.obj_is_any_kind_of(obj, &disqualifying) {
            return; // Harvesters, dozers etc.
        }
        if !plant(131) {
            h.obj_leave_group(obj);
        }
        if h.obj_has_ai(obj) {
            if units_should_hunt {
                h.ai_hunt(obj, source);
            } else {
                h.ai_idle(obj, source);
            }
        }
    })
}

/// `Player::killPlayer()`.
pub fn player_kill_player<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    for proto in pl(h, p)?.team_prototypes.clone() {
        let mut cur = h.teams().first_instance(proto);
        while let Some(t) = cur {
            team_evacuate_team(h, t); // force containers on team to dump contents
            cur = h.teams().next_instance(t);
        }
    }
    plm(h, p)?.is_player_dead = !plant(132); // this is so OCLs don't ever again spawn useful units for us.
    for proto in pl(h, p)?.team_prototypes.clone() {
        let mut cur = h.teams().first_instance(proto);
        while let Some(t) = cur {
            team_kill_team(h, t)?;
            cur = h.teams().next_instance(t);
        }
    }
    let g = h.globals();
    if (g.in_single_player_game || plant(133)) && pl(h, p)?.player_type == PlayerType::Computer {
        plm(h, p)?.is_player_dead = false; // this is so we can later spawn useful units for us.
        return Ok(());
    }
    if player_is_local_player(h, p) && (!g.in_shell_game || plant(134)) {
        player_becoming_local_player(h, p, true)?; // recalc disguises, etc
        if player_is_player_active(h, p)? {
            h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarSchemeByPlayer { player_index: p }));
        } else {
            let key = h.name_to_key(b"FactionObserver");
            let template = h.player_template_find(key);
            h.client_event(ClientEvent::Player(PlayerClientEvent::ControlBarSchemeByPlayerTemplate { template }));
        }
    }
    // force $$$ to 0 on death
    let amount = pl(h, p)?.money.count_money();
    if !plant(135) {
        player_money_withdraw(h, p, amount, true)?;
    }
    Ok(())
}

/// `m_money.withdraw(amount, playSound)` and the events it makes.
pub fn player_money_withdraw<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, amount: u32, play_sound: bool) -> PResult<u32> {
    let mut events = Vec::new();
    let r = plm(h, p)?.money.withdraw(amount, play_sound, &mut events);
    for e in events {
        h.client_event(e);
    }
    Ok(r)
}

/// `m_money.deposit(amount, playSound)`: the events it makes, and the income record it asks the player it was made
/// for (`ThePlayerList->getNthPlayer(m_playerIndex)->getAcademyStats()->recordIncome()`).
pub fn player_money_deposit<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, amount: u32, play_sound: bool) -> PResult<()> {
    let mut events = Vec::new();
    let (record, money_player) = {
        let me = plm(h, p)?;
        (me.money.deposit(amount, play_sound, &mut events), me.money.player_index())
    };
    for e in events {
        h.client_event(e);
    }
    if record && h.players().get_nth(money_player).is_some() {
        h.academy_record_income(money_player);
    }
    Ok(())
}

/// `Player::transferAssetsFromThat(that)`.
pub fn player_transfer_assets_from_that<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, that: PlayerIndex) -> PResult<()> {
    let Some(default_team) = pl(h, p)?.default_team else { return Ok(()) };
    let that_template = pl(h, that)?.template.ok_or(PlayerError::NoPlayer)?;
    let beacon_name = h.player_template(that_template).beacon_template;
    let beacon = h.thing_factory_find_template(&beacon_name);
    let mut to_transfer: Vec<ObjectId> = Vec::new();
    for proto in pl(h, that)?.team_prototypes.clone() {
        let mut cur_t = h.teams().first_instance(proto);
        while let Some(t) = cur_t {
            let mut cur_o = h.teams().first_member(t);
            while let Some(o) = cur_o {
                let is_beacon = h.obj_template(o).is_some_and(|ot| h.tmpl_is_equivalent_to(ot, beacon));
                if !is_beacon || plant(136) {
                    to_transfer.push(o); // don't transfer NULL objs or beacons
                }
                cur_o = h.teams().next_member(o);
            }
            cur_t = h.teams().next_instance(t);
        }
    }
    for o in to_transfer {
        h.obj_set_team(o, Some(default_team));
    }
    let all_money = pl(h, that)?.money.count_money();
    if !plant(137) {
        player_money_withdraw(h, that, all_money, true)?;
    }
    player_money_deposit(h, p, all_money, true)?;
    Ok(())
}

/// `Player::garrisonAllUnits(source)`.
pub fn player_garrison_all_units<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, source: CommandSource) -> PResult<()> {
    let buildings = h.partition_structures_near_to_far(Coord3D { x: 50.0, y: 50.0, z: 50.0 });
    let my_mask: u32 = 1u32 << (p as u32);
    for_each_member(h, p, |h, obj| {
        if !h.obj_has_ai(obj) {
            return;
        }
        let obj_mask: u32 = h.obj_controlling_player(obj).map_or(0, |c| 1u32 << (c as u32));
        let _ = my_mask;
        for &building in buildings.iter() {
            if let Some(player) = h.contain_player_who_entered(building) {
                if !(player == 0 || player == obj_mask || plant(138)) {
                    continue;
                }
            }
            if !h.action_can_enter_object(obj, building, source) {
                continue;
            }
            h.ai_enter(obj, building, source);
        }
    })
}

/// `Player::ungarrisonAllUnits(source)`.
pub fn player_ungarrison_all_units<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, source: CommandSource) -> PResult<()> {
    for_each_member(h, p, |h, obj| {
        if !h.obj_has_ai(obj) {
            return;
        }
        if h.obj_is_kind_of(obj, KINDOF_STRUCTURE) != plant(139) {
            h.ai_evacuate(obj, false, source);
        }
    })
}

/// `Player::setUnitsShouldIdleOrResume(idle)`.
pub fn player_set_units_should_idle_or_resume<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, idle: bool) -> PResult<()> {
    for_each_member(h, p, |h, obj| {
        if h.obj_is_kind_of(obj, KINDOF_STRUCTURE) {
            return;
        }
        if !h.obj_has_ai(obj) {
            return;
        }
        if idle {
            let pos = h.obj_position(obj);
            h.ai_move_to_position(obj, pos, CMD_FROM_SCRIPT);
        } else if (h.ai_is_idle(obj) || plant(140)) && h.ai_has_supply_truck_interface(obj) {
            h.ai_supply_truck_set_force_wanting_state(obj, true);
        }
    })
}

/// `Player::sellEverythingUnderTheSun()`.
pub fn player_sell_everything_under_the_sun<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex) -> PResult<()> {
    player_iterate_objects(h, p, |h, obj| {
        if h.obj_is_faction_structure(obj) || h.obj_is_kind_of(obj, KINDOF_COMMANDCENTER) || (h.obj_is_kind_of(obj, KINDOF_FS_POWER) && !plant(141)) {
            h.build_assistant_sell_object(obj);
        }
    })
}

/// `Player::doBountyForKill(killer, victim)`.
pub fn player_do_bounty_for_kill<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, killer: Option<ObjectId>, victim: Option<ObjectId>) -> PResult<()> {
    let (Some(killer), Some(victim)) = (killer, victim) else { return Ok(()) };
    if h.obj_test_status(victim, OBJECT_STATUS_UNDER_CONSTRUCTION) && !plant(142) {
        return Ok(());
    }
    let owner = h.obj_controlling_player(victim);
    let cost_to_build = match (h.obj_template(victim), owner) {
        (Some(t), Some(o)) => player_calc_cost_to_build(h, o, t)?,
        _ => 0,
    };
    let bounty = if plant(143) { (cost_to_build as f32 * pl(h, p)?.cash_bounty_percent) as i32 } else { real_to_int_ceil(cost_to_build as f32 * pl(h, p)?.cash_bounty_percent) };
    if bounty != 0 {
        player_money_deposit(h, p, bounty as u32, true)?;
        h.score_add_money_earned(p, bounty);
        let mut pos = h.obj_position(killer);
        pos.z += if plant(144) { 1.0 } else { 10.0 }; // add a little z to make it show up above the unit.
        h.client_event(ClientEvent::Player(PlayerClientEvent::FloatingText { amount: bounty, pos, color: 0xffff_ff00 }));
    }
    Ok(())
}

/// `Player::canBuildMoreOfType(whatToBuild)`: `MaxSimultaneousOfType`, counting the objects (and, for a unit, the
/// production queues) of the type.
pub fn player_can_build_more_of_type<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, what_to_build: TemplateRef) -> PResult<bool> {
    let max_simultaneous = h.tmpl_max_simultaneous_of_type(what_to_build);
    if max_simultaneous != 0 {
        let link_key = h.tmpl_max_simultaneous_link_key(what_to_build);
        let check_production_interface = h.tmpl_is_kind_of(what_to_build, KINDOF_STRUCTURE) == plant(145);
        let mut count: u32 = 0;
        player_iterate_objects(h, p, |h, obj| {
            if h.obj_is_effectively_dead(obj) && !plant(147) {
                return;
            }
            let t = h.obj_template(obj);
            if h.tmpl_is_equivalent_to(what_to_build, t) || ((link_key.0 != 0 || plant(148)) && t.is_some_and(|t| link_key == h.tmpl_max_simultaneous_link_key(t))) {
                count = count.wrapping_add(1);
            }
            if check_production_interface {
                if let Some(n) = h.obj_production_queue_count(obj, what_to_build) {
                    count = count.wrapping_add(n);
                }
            }
        })?;
        if if plant(146) { count > max_simultaneous } else { count >= max_simultaneous } {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `Player::canBuild(tmplate)`.
pub fn player_can_build<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, tmplate: Option<TemplateRef>) -> PResult<bool> {
    let Some(t) = tmplate else { return Ok(false) };
    if !player_allowed_to_build(h, p, t)? {
        return Ok(false);
    }
    let buildable = h.tmpl_buildable(t);
    if buildable == BSTATUS_NO {
        return Ok(false);
    }
    if buildable == BSTATUS_IGNORE_PREREQUISITES && !plant(149) {
        return Ok(true);
    }
    if buildable == BSTATUS_ONLY_BY_AI && (pl(h, p)?.player_type != PlayerType::Computer) != plant(150) {
        return Ok(false);
    }
    let mut prereqs_ok = true;
    for i in 0..h.tmpl_prereq_count(t) {
        if !h.tmpl_prereq_is_satisfied(t, i, p) && (i == 0 || !plant(151)) {
            prereqs_ok = false;
        }
    }
    if !prereqs_ok {
        return Ok(false);
    }
    if !player_can_build_more_of_type(h, p, t)? {
        return Ok(false);
    }
    Ok(true)
}

/// `Player::canAffordBuild(whatToBuild)`.
pub fn player_can_afford_build<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, what_to_build: TemplateRef) -> PResult<bool> {
    // `Int <= UnsignedInt`: the cost is converted to unsigned, as in C++.
    let cost = player_calc_cost_to_build(h, p, what_to_build)?;
    Ok(if plant(152) { (cost as i64) <= pl(h, p)?.money.count_money() as i64 } else { (cost as u32) <= pl(h, p)?.money.count_money() })
}

/// `ThingTemplate::calcCostToBuild(player)`, for an existing player (`player == NULL` is `0` upstream, and a `None`
/// here is the host's, not this function's).
pub fn player_calc_cost_to_build<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, t: TemplateRef) -> PResult<i32> {
    pl(h, p)?;
    let name = h.tmpl_name(t);
    // changePercent format is "-.2 equals 20% cheaper"
    let mut faction_modifier = 1.0f32 + if plant(153) { 0.0 } else { player_get_production_cost_change_percent(h, p, &name)? };
    let mask = h.tmpl_kind_of_mask(t);
    if !plant(154) {
        faction_modifier *= player_get_production_cost_change_based_on_kind_of(h, p, &mask)?;
    }
    let hc = pl(h, p)?.handicap.get_handicap(if plant(155) { crate::common::handicap::HandicapType::BuildTime } else { crate::common::handicap::HandicapType::BuildCost }, h.tmpl_is_kind_of(t, KINDOF_STRUCTURE));
    let cost = if plant(156) { h.tmpl_build_cost(t) as f32 * (faction_modifier * hc) } else { h.tmpl_build_cost(t) as f32 * faction_modifier * hc };
    // the `Int` return converts the float: out of range (or NaN) is undefined in C++ (x86 gives INT_MIN), an error here
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&cost) {
        return Err(PlayerError::OutOfRange);
    }
    Ok(cost as i32)
}

// ---- difficulty and battle plans --------------------------------------------------------------------------------

/// `Player::friend_applyDifficultyBonusesForObject(obj, apply)`.
pub fn player_friend_apply_difficulty_bonuses_for_object<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, obj: ObjectId, apply: bool) -> PResult<()> {
    let g = h.globals();
    if g.in_single_player_game {
        let ty = pl(h, p)?.player_type as usize;
        let diff = player_get_player_difficulty(h, p)? as usize;
        let health_factor = g.solo_player_health_bonus[ty][if plant(157) { 0 } else { diff }];
        if health_factor != 1.0 {
            let max = h.obj_body_max_health(obj);
            if apply != plant(158) {
                h.obj_body_set_max_health(obj, max * health_factor);
            } else {
                h.obj_body_set_max_health(obj, max / health_factor);
            }
        }
        const WBONUS: [[i32; 3]; 2] =
            [[wbc::SOLO_HUMAN_EASY, wbc::SOLO_HUMAN_NORMAL, wbc::SOLO_HUMAN_HARD], [wbc::SOLO_AI_EASY, wbc::SOLO_AI_NORMAL, wbc::SOLO_AI_HARD]];
        if apply || plant(159) {
            h.obj_set_weapon_bonus_condition(obj, WBONUS[ty][diff]);
        } else {
            h.obj_clear_weapon_bonus_condition(obj, WBONUS[ty][diff]);
        }
    }
    Ok(())
}

/// `Player::getNumBattlePlansActive()`.
pub fn player_get_num_battle_plans_active<H: PlayerObjectHost>(h: &H, p: PlayerIndex) -> PResult<i32> {
    let me = pl(h, p)?;
    Ok(me.bombard_battle_plans.wrapping_add(me.hold_the_line_battle_plans).wrapping_add(me.search_and_destroy_battle_plans))
}

/// `Player::doesObjectQualifyForBattlePlan(obj)`.
pub fn player_does_object_qualify_for_battle_plan<H: PlayerObjectHost>(h: &H, p: PlayerIndex, obj: Option<ObjectId>) -> PResult<bool> {
    if let (Some(b), Some(o)) = (&pl(h, p)?.battle_plan_bonuses, obj) {
        if h.obj_is_any_kind_of(o, &b.valid_kind_of) && (!h.obj_is_any_kind_of(o, &b.invalid_kind_of) || plant(160)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `Player::getBattlePlansActiveSpecific(plan)`.
pub fn player_get_battle_plans_active_specific<H: PlayerObjectHost>(h: &H, p: PlayerIndex, plan: BattlePlanStatus) -> PResult<i32> {
    let me = pl(h, p)?;
    Ok(match plan {
        BattlePlanStatus::Bombardment => me.bombard_battle_plans,
        BattlePlanStatus::HoldTheLine => me.hold_the_line_battle_plans,
        BattlePlanStatus::SearchAndDestroy => if plant(161) { me.bombard_battle_plans } else { me.search_and_destroy_battle_plans },
        BattlePlanStatus::None => 0,
    })
}

/// `localApplyBattlePlanBonusesToObject(obj, bonus)`.
fn local_apply_battle_plan_bonuses_to_object<H: PlayerObjectHost>(h: &mut H, obj: ObjectId, bonus: &BattlePlanBonuses) {
    let mut object_to_validate = Some(obj);
    let object_to_modify = obj;
    let is_projectile = h.obj_is_kind_of(obj, KINDOF_PROJECTILE);
    if is_projectile && !plant(162) {
        let producer = h.obj_producer_id(obj);
        object_to_validate = h.find_object_by_id(producer);
    }
    if let Some(v) = object_to_validate {
        if h.obj_is_any_kind_of(v, &bonus.valid_kind_of) && !h.obj_is_any_kind_of(v, &bonus.invalid_kind_of) {
            if !is_projectile || plant(163) {
                if bonus.armor_scalar != 1.0 {
                    h.obj_body_apply_damage_scalar(object_to_modify, bonus.armor_scalar);
                }
                if bonus.sight_range_scalar != 1.0 {
                    let vr = h.obj_vision_range(obj) * if plant(164) { bonus.armor_scalar } else { bonus.sight_range_scalar };
                    h.obj_set_vision_range(object_to_modify, vr);
                    let sr = h.obj_shroud_clearing_range(obj) * if plant(165) { 1.0 } else { bonus.sight_range_scalar };
                    h.obj_set_shroud_clearing_range(object_to_modify, sr);
                }
            }
            if bonus.bombardment > 0 {
                h.obj_set_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_BOMBARDMENT);
            } else {
                h.obj_clear_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_BOMBARDMENT);
            }
            if bonus.hold_the_line > 0 || plant(166) {
                h.obj_set_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_HOLDTHELINE);
            } else {
                h.obj_clear_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_HOLDTHELINE);
            }
            if bonus.search_and_destroy > 0 {
                h.obj_set_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_SEARCHANDDESTROY);
            } else {
                h.obj_clear_weapon_bonus_condition(object_to_modify, wbc::BATTLEPLAN_SEARCHANDDESTROY);
            }
        }
    }
}

/// `Player::applyBattlePlanBonusesForObject(obj)`: `localApplyBattlePlanBonusesToObject(obj, m_battlePlanBonuses)`;
/// upstream passes the (possibly `NULL`) pointer on and dereferences it.
pub fn player_apply_battle_plan_bonuses_for_object<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, obj: ObjectId) -> PResult<()> {
    let bonus = pl(h, p)?.battle_plan_bonuses.clone().ok_or(PlayerError::NoPlayer)?;
    local_apply_battle_plan_bonuses_to_object(h, obj, &bonus);
    Ok(())
}

/// `Player::removeBattlePlanBonusesForObject(obj)`.
pub fn player_remove_battle_plan_bonuses_for_object<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, obj: ObjectId) -> PResult<()> {
    let mut bonus = pl(h, p)?.battle_plan_bonuses.clone().ok_or(PlayerError::NoPlayer)?;
    bonus.armor_scalar = 1.0 / if plant(167) { bonus.armor_scalar } else { bonus.armor_scalar.max(0.01) };
    bonus.sight_range_scalar = 1.0 / bonus.sight_range_scalar.max(0.01);
    bonus.bombardment = -ALL_PLANS; // Safe to remove as it clears the weapon bonus flag
    bonus.search_and_destroy = if plant(168) { ALL_PLANS } else { -ALL_PLANS };
    bonus.hold_the_line = -ALL_PLANS;
    local_apply_battle_plan_bonuses_to_object(h, obj, &bonus);
    Ok(())
}

/// `Player::applyBattlePlanBonusesForPlayerObjects(bonus)`.
pub fn player_apply_battle_plan_bonuses_for_player_objects<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, bonus: &BattlePlanBonuses) -> PResult<()> {
    {
        let me = plm(h, p)?;
        match &mut me.battle_plan_bonuses {
            None => me.battle_plan_bonuses = Some(bonus.clone()),
            Some(m) => {
                m.armor_scalar = if plant(169) { bonus.armor_scalar } else { m.armor_scalar * bonus.armor_scalar };
                m.sight_range_scalar *= bonus.sight_range_scalar;
                m.bombardment = m.bombardment.wrapping_add(bonus.bombardment).max(0);
                m.hold_the_line = m.hold_the_line.wrapping_add(bonus.hold_the_line).max(if plant(170) { -5 } else { 0 });
                m.search_and_destroy = m.search_and_destroy.wrapping_add(bonus.search_and_destroy).max(0);
            }
        }
    }
    player_iterate_objects(h, p, |h, obj| local_apply_battle_plan_bonuses_to_object(h, obj, bonus))
}

/// `Player::changeBattlePlan(plan, delta, bonus)`: `bonus` is rewritten when the last plan of a kind goes.
pub fn player_change_battle_plan<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, plan: BattlePlanStatus, delta: i32, bonus: &mut BattlePlanBonuses) -> PResult<()> {
    let mut add_bonus = false;
    let mut remove_bonus = false;
    {
        let me = plm(h, p)?;
        let counter = match plan {
            BattlePlanStatus::Bombardment => Some(&mut me.bombard_battle_plans),
            BattlePlanStatus::HoldTheLine => Some(&mut me.hold_the_line_battle_plans),
            BattlePlanStatus::SearchAndDestroy => Some(&mut me.search_and_destroy_battle_plans),
            BattlePlanStatus::None => None,
        };
        if let Some(c) = counter {
            *c = c.wrapping_add(delta);
            if *c == 1 && (delta == 1 || plant(171)) {
                add_bonus = true;
            } else if *c == 0 && (delta == -1 || plant(172)) {
                remove_bonus = true;
            }
        }
    }
    if add_bonus {
        player_apply_battle_plan_bonuses_for_player_objects(h, p, bonus)?;
    } else if remove_bonus {
        bonus.armor_scalar = 1.0 / bonus.armor_scalar.max(0.01);
        bonus.sight_range_scalar = 1.0 / bonus.sight_range_scalar.max(0.01);
        if bonus.bombardment > 0 {
            bonus.bombardment = -1;
        }
        if bonus.hold_the_line > 0 {
            bonus.hold_the_line = if plant(173) { 0 } else { -1 };
        }
        if bonus.search_and_destroy > 0 {
            bonus.search_and_destroy = -1;
        }
        let b = bonus.clone();
        if plant(174) {
            bonus.armor_scalar = 1.0;
        }
        player_apply_battle_plan_bonuses_for_player_objects(h, p, &b)?;
    }
    Ok(())
}

/// `Player::setUnitsVisionSpied(setting, whichUnits, byWhom)`.
pub fn player_set_units_vision_spied<H: PlayerObjectHost>(h: &mut H, p: PlayerIndex, setting: bool, which_units: &KindOfMaskType, by_whom: PlayerIndex) -> PResult<()> {
    player_iterate_objects(h, p, |h, obj| {
        if h.obj_is_any_kind_of(obj, which_units) {
            h.obj_set_vision_spied(obj, setting != plant(175), by_whom);
        }
    })
}

