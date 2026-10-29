// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIGroup.cpp (lines 60-479 and 2038-3318: the group, its members, centre and
//     speed, and its commands but the moves)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (class AIGroup)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AIGroup`: a list of objects commanded together (`p1w13-sonnet`, layer 10, batch A24).
//!
//! The group holds its members in the order they joined (`m_memberList`, by id here), its count, the speed of its slowest
//! member, whether that is stale, and its id. Its commands pass to each member's AI (or, for some, to the member's contain, spawn
//! behaviour, special power modules, command buttons, weapon set, production or drawable). The members' world is the host's
//! ([`AiGroupHost`]); the commands to an AI are one call, [`AiGroupHost::ai_command`] with a [`GroupAiCommand`].
//!
//! `Object::enterGroup` and `leaveGroup` are the host's ([`AiGroupHost::enter_group`], [`AiGroupHost::leave_group`]): upstream's
//! leave the member's former group (which may destroy it) and, inside `remove`, call this group's `remove` again, which finds the
//! member gone. The destructor (each member leaving) is `TheAI->destroyGroup`'s business (AI.cpp).
//!
//! The moves (`computeIndividualDestination` .. `groupFollowWaypointPathAsTeamExact`, batch A25) are in
//! [`super::ai_group_moves`]; the ground path they keep is [`AiGroup::ground_path`], which `recompute` clears.
//!
//! # Upstream facts kept
//!
//! - `getCenter` divides by the count even when it is 0 (NaN for a group of none but held members; `count > 0` answers false).
//! - `getMinMaxAndCenter`'s formation test compares the first member's formation with `NO_FORMATION_ID` only (`if (id ==
//!   NO_FORMATION_ID) id = NO_FORMATION_ID`): the group is a formation when its first counted member is in one and it counts two.
//! - `recompute` finds the member nearest the centre (its distance) and keeps nothing of it.
//! - `getAttitude` answers `AI_PASSIVE` whatever the members' attitudes.
//! - `groupFollowPath` does nothing.
//! - `removeAnyObjectsNotOwnedByPlayer` would loop forever on a null member (`continue` without advancing); members are never null.
//! - The `xfer` saves the version only; the `crc` the members' ids, the count, a former leader id (always `INVALID_ID`), the
//!   speed, the dirty flag and the id.
//! - The surrender and prisoner commands are compiled out (`ALLOW_SURRENDER`).

use crate::common::base_type::{Coord2D, Coord3D};
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::kind_of::{KindOfType, KINDOF_AIRCRAFT, KINDOF_ALWAYS_SELECTABLE, KINDOF_IMMOBILE, KINDOF_STRUCTURE};
use crate::common::object_status_types::{ObjectStatusTypes, OBJECT_STATUS_CAN_STEALTH, OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use crate::common::xfer::{Xfer, XferResult, XferVersion};
use crate::logic::ai::ai_update_types::*;
use crate::logic::module::body_module::{is_condition_better, BodyDamageType};
use crate::logic::modules::plant;
use crate::logic::object_iter::{IterOrderType, SimpleObjectIterator};

/// `NO_FORMATION_ID`.
pub const NO_FORMATION_ID: u32 = 0;
/// `SCIENCE_INVALID`.
pub const SCIENCE_INVALID: i32 = -1;
/// `LOGICFRAMES_PER_SECOND` (the cheer's and the stealth delay's).
const LOGICFRAMES_PER_SECOND: i32 = 30;

/// `ATTACK_NEW_TARGET`, `ATTACK_NEW_TARGET_FORCED` (`getAbleToAttackSpecificObject`'s and `getAbleToUseWeaponAgainstTarget`'s).
pub use crate::common::game_common::{AbleToAttackType, ATTACK_NEW_TARGET, ATTACK_NEW_TARGET_FORCED};

/// A command the group gives a member's AI (`ai->aiXxx(..)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroupAiCommand {
    Idle,
    AttackObject { victim: ObjectId, max_shots: i32, forced: bool },
    AttackTeam { team: u32, max_shots: i32 },
    AttackPosition { pos: Coord3D, max_shots: i32 },
    AttackMoveToPosition { pos: Coord3D, max_shots: i32 },
    MoveToPosition { pos: Coord3D },
    Hunt,
    Repair { obj: ObjectId },
    ResumeConstruction { obj: ObjectId },
    GetHealed { depot: ObjectId },
    GetRepaired { depot: ObjectId },
    Enter { obj: ObjectId },
    Dock { obj: ObjectId },
    Exit { obj: ObjectId },
    MoveToAndEvacuate { pos: Coord3D },
    Evacuate { expose_stealth_units: bool },
    // batch A25
    FollowPathAppend { pos: Coord3D },
    TightenToPosition { pos: Coord3D },
    /// `aiFollowWaypointPath`, `..Exact`, `..AsTeam`, `..ExactAsTeam`.
    FollowWaypointPath { way: u32, exact: bool, as_team: bool },
    MoveToAndEvacuateAndExit { pos: Coord3D },
    ExecuteRailedTransport,
    GoProne { damage: u32 },
    GuardPosition { pos: Coord3D, mode: GuardMode },
    GuardObject { obj: ObjectId, mode: GuardMode },
    GuardArea { area: i32, mode: GuardMode },
    AttackArea { area: i32 },
    HackInternet,
    CombatDrop { target: ObjectId, pos: Coord3D },
}

/// The command button calls on a member (`source->doCommandButton*(..)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GroupButtonCall {
    Plain,
    AtPosition(Coord3D),
    UsingWaypoints(u32),
    AtObject(ObjectId),
}

/// What the group asks of its members' world.
pub trait AiGroupHost {
    /// `TheAI->getNextGroupID()`, `getNextFormationID()`, `destroyGroup(group)`.
    fn ai_next_group_id(&mut self) -> u32;
    fn ai_next_formation_id(&mut self) -> u32;
    fn ai_destroy_group(&mut self, group: u32);
    /// `TheGameLogic->getFrame()`; `GameLogicRandomValue(lo, hi)`.
    fn frame(&mut self) -> u32;
    fn logic_random_value(&mut self, lo: i32, hi: i32) -> i32;
    /// `TheGlobalData->m_movementPenaltyDamageState`.
    fn movement_penalty_damage_state(&mut self) -> BodyDamageType;
    // ---- the member
    /// `getAIUpdateInterface() != NULL`.
    fn has_ai(&mut self, o: ObjectId) -> bool;
    fn is_kind_of(&mut self, o: ObjectId, k: KindOfType) -> bool;
    fn status_test(&mut self, o: ObjectId, s: ObjectStatusTypes) -> bool;
    fn is_held(&mut self, o: ObjectId) -> bool;
    fn is_effectively_dead(&mut self, o: ObjectId) -> bool;
    fn is_able_to_attack(&mut self, o: ObjectId) -> bool;
    fn is_airborne_target(&mut self, o: ObjectId) -> bool;
    fn position(&mut self, o: ObjectId) -> Coord3D;
    /// `getControllingPlayer()` (a handle; `None`: none).
    fn controlling_player(&mut self, o: ObjectId) -> Option<u32>;
    fn enter_group(&mut self, o: ObjectId, group: u32);
    fn leave_group(&mut self, o: ObjectId);
    fn formation_id(&mut self, o: ObjectId) -> u32;
    fn set_formation_id(&mut self, o: ObjectId, id: u32);
    fn set_formation_offset(&mut self, o: ObjectId, offset: Coord2D);
    /// `getBodyModule()->getDamageState()`; the AI's `getCurLocomotorSpeed()`.
    fn damage_state(&mut self, o: ObjectId) -> BodyDamageType;
    fn cur_locomotor_speed(&mut self, o: ObjectId) -> f32;
    /// `getAbleToAttackSpecificObject(type, victim, src)`; `getAbleToUseWeaponAgainstTarget(type, NULL, pos, src)`.
    fn able_to_attack_specific_object(&mut self, o: ObjectId, t: AbleToAttackType, victim: ObjectId, src: CommandSourceType) -> CanAttackResultValue;
    fn able_to_use_weapon_against_position(&mut self, o: ObjectId, t: AbleToAttackType, pos: &Coord3D, src: CommandSourceType) -> CanAttackResultValue;
    // ---- the member's AI
    fn ai_command(&mut self, o: ObjectId, cmd: GroupAiCommand, src: CommandSourceType);
    fn ai_set_attitude(&mut self, o: ObjectId, a: AttitudeType);
    fn ai_is_idle(&mut self, o: ObjectId) -> bool;
    fn ai_is_busy(&mut self, o: ObjectId) -> bool;
    fn ai_can_auto_acquire(&mut self, o: ObjectId) -> bool;
    fn ai_can_auto_acquire_while_stealthed(&mut self, o: ObjectId) -> bool;
    fn ai_set_next_mood_check_time(&mut self, o: ObjectId, t: u32);
    /// `getStealth()->getStealthDelay()` (`None`: no stealth module).
    fn stealth_delay(&mut self, o: ObjectId) -> Option<u32>;
    // ---- the member's contain and spawn behaviour
    fn has_contain(&mut self, o: ObjectId) -> bool;
    /// The contained objects (`iterateContained` forward, `getContainedItemsList`).
    fn contained(&mut self, o: ObjectId) -> Vec<ObjectId>;
    fn contain_passengers_allowed_to_fire(&mut self, o: ObjectId) -> bool;
    fn contain_order_all_passengers_to_exit(&mut self, o: ObjectId, src: CommandSourceType);
    fn has_spawn(&mut self, o: ObjectId) -> bool;
    fn spawn_slaves_have_freedom(&mut self, o: ObjectId) -> bool;
    fn spawn_order_slaves_idle(&mut self, o: ObjectId, src: CommandSourceType);
    fn spawn_order_slaves_attack_target(&mut self, o: ObjectId, victim: ObjectId, max_shots: i32, src: CommandSourceType);
    fn spawn_order_slaves_attack_position(&mut self, o: ObjectId, pos: &Coord3D, max_shots: i32, src: CommandSourceType);
    // ---- the terrain (an aircraft's evacuation)
    fn highest_layer_for_destination(&mut self, pos: &Coord3D) -> i32;
    fn layer_height(&mut self, x: f32, y: f32, layer: i32) -> f32;
    // ---- special powers
    /// `TheSpecialPowerStore->findSpecialPowerTemplateByID(id)`'s required science (`None`: no such template).
    fn special_power_required_science(&mut self, sp: u32) -> Option<i32>;
    fn player_has_science(&mut self, o: ObjectId, science: i32) -> bool;
    fn has_special_power_module(&mut self, o: ObjectId, sp: u32) -> bool;
    fn can_do_special_power(&mut self, o: ObjectId, sp: u32, options: u32) -> bool;
    fn can_do_special_power_at_location(&mut self, o: ObjectId, sp: u32, pos: &Coord3D, object_in_way: ObjectId, options: u32) -> bool;
    fn can_do_special_power_at_object(&mut self, o: ObjectId, sp: u32, target: ObjectId, options: u32) -> bool;
    fn do_special_power(&mut self, o: ObjectId, sp: u32, options: u32);
    fn do_special_power_at_location(&mut self, o: ObjectId, sp: u32, pos: &Coord3D, angle: f32, options: u32);
    fn do_special_power_at_object(&mut self, o: ObjectId, sp: u32, target: ObjectId, options: u32);
    fn clear_undetected_defector(&mut self, o: ObjectId);
    /// `findSpecialPowerWithOverridableDestinationActive(type)->setSpecialPowerOverridableDestination(loc)` when there is one.
    fn override_special_power_destination(&mut self, o: ObjectId, sp_type: i32, loc: &Coord3D);
    // ---- the rest
    fn set_cheering(&mut self, o: ObjectId, frames: i32);
    fn sell(&mut self, o: ObjectId);
    /// Each behaviour module's `getOverchargeBehaviorInterface()->toggle()`.
    fn toggle_overcharge(&mut self, o: ObjectId);
    fn do_command_button(&mut self, o: ObjectId, button: u32, call: GroupButtonCall, src: CommandSourceType);
    fn set_weapon_set_flag(&mut self, o: ObjectId, wst: i32);
    fn clear_weapon_set_flag(&mut self, o: ObjectId, wst: i32);
    /// `getTemplate()->findWeaponTemplateSet(flags of wst) != NULL`.
    fn template_has_weapon_set(&mut self, o: ObjectId, wst: i32) -> bool;
    fn set_weapon_lock(&mut self, o: ObjectId, slot: i32, lock: i32) -> bool;
    fn release_weapon_lock(&mut self, o: ObjectId, lock: i32);
    /// `TheUpgradeCenter->canAffordUpgrade(player, upgrade, FALSE)`; the upgrade's type is an object's.
    fn can_afford_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool;
    fn upgrade_is_object_type(&mut self, upgrade: u32) -> bool;
    fn has_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool;
    fn affected_by_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool;
    fn can_produce_upgrade(&mut self, o: ObjectId, upgrade: u32) -> bool;
    /// The production update's `canQueueUpgrade(upgrade) == CANMAKE_QUEUE_FULL` (`None`: no production update).
    fn production_queue_full_for_upgrade(&mut self, o: ObjectId, upgrade: u32) -> Option<bool>;
    fn production_queue_upgrade(&mut self, o: ObjectId, upgrade: u32);
    /// Whether the member's command set has a button of the type (`None`: no command set).
    fn command_set_has_button_type(&mut self, o: ObjectId, command_type: i32) -> Option<bool>;
    /// `getDrawable()->setEmoticon(name, duration)` when there is a drawable.
    fn set_emoticon(&mut self, o: ObjectId, name: &[u8], duration: i32);
}

/// `AIGroup`.
#[derive(Clone, Debug, PartialEq)]
pub struct AiGroup {
    /// `m_memberList`, in order.
    pub members: Vec<ObjectId>,
    /// `m_memberListSize`.
    pub member_list_size: u32,
    pub speed: f32,
    pub dirty: bool,
    pub id: u32,
    /// `m_groundPath` (batch A25): its nodes' positions in their optimised order.
    pub ground_path: Option<Vec<Coord3D>>,
}

impl AiGroup {
    /// `AIGroup()`: no members, speed 0, the AI's next id.
    pub fn new<H: AiGroupHost>(h: &mut H) -> Self {
        AiGroup { members: Vec::new(), member_list_size: 0, speed: 0.0, dirty: false, id: h.ai_next_group_id(), ground_path: None }
    }

    /// `getID()`.
    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// `getAllIDs()`.
    pub fn get_all_ids(&self) -> Vec<ObjectId> {
        self.members.clone()
    }

    /// `getSpeed()`: recomputed when stale.
    pub fn get_speed<H: AiGroupHost>(&mut self, h: &mut H) -> f32 {
        if self.dirty {
            self.recompute(h);
        }
        self.speed
    }

    /// `isMember(obj)`.
    pub fn is_member(&self, o: ObjectId) -> bool {
        self.members.contains(&o)
    }

    /// `add(obj)`: an AI, or a structure or always-selectable object, joins at the end.
    pub fn add<H: AiGroupHost>(&mut self, h: &mut H, o: ObjectId) {
        if o == INVALID_ID {
            return;
        }
        if !h.has_ai(o) && !h.is_kind_of(o, KINDOF_STRUCTURE) && !h.is_kind_of(o, KINDOF_ALWAYS_SELECTABLE) {
            return;
        }
        self.members.push(o);
        self.member_list_size = self.member_list_size.wrapping_add(1);
        h.enter_group(o, self.id);
        self.dirty = true;
    }

    /// `remove(obj)`: true when the group, now empty, was destroyed (`TheAI->destroyGroup(this)`).
    pub fn remove<H: AiGroupHost>(&mut self, h: &mut H, o: ObjectId) -> bool {
        let Some(i) = self.members.iter().position(|&x| x == o) else { return false };
        self.members.remove(i);
        self.member_list_size = self.member_list_size.wrapping_sub(1);
        h.leave_group(o);
        self.dirty = true;
        if self.is_empty() && !plant(3530) {
            h.ai_destroy_group(self.id);
            return true;
        }
        false
    }

    /// `containsAnyObjectsNotOwnedByPlayer(player)`.
    pub fn contains_any_objects_not_owned_by_player<H: AiGroupHost>(&mut self, h: &mut H, player: Option<u32>) -> bool {
        for &o in &self.members.clone() {
            if h.controlling_player(o) != player {
                return true;
            }
        }
        false
    }

    /// `removeAnyObjectsNotOwnedByPlayer(player)`: true when the group was destroyed.
    pub fn remove_any_objects_not_owned_by_player<H: AiGroupHost>(&mut self, h: &mut H, player: Option<u32>) -> bool {
        let mut i = 0;
        while i < self.members.len() {
            let o = self.members[i];
            if h.controlling_player(o) != player {
                // (the iterator advanced first: the removal does not move it)
                if self.remove(h, o) {
                    return true;
                }
                continue;
            }
            i += 1;
        }
        false
    }

    /// `getCenter(&center)`: the centroid of the members with an AI, not held (none: of the others not held); true when any.
    pub fn get_center<H: AiGroupHost>(&mut self, h: &mut H) -> (Coord3D, bool) {
        let mut count = 0i32;
        let mut c = Coord3D::default();
        for &o in &self.members.clone() {
            if h.is_held(o) {
                continue; // "don't bother counting riders in the center calculation."
            }
            if h.has_ai(o) {
                let p = h.position(o);
                c.x += p.x;
                c.y += p.y;
                c.z += p.z;
                count += 1;
            }
        }
        if count == 0 && !self.members.is_empty() && !plant(3531) {
            for &o in &self.members.clone() {
                if h.is_held(o) {
                    continue;
                }
                let p = h.position(o);
                c.x += p.x;
                c.y += p.y;
                c.z += p.z;
                count += 1;
            }
        }
        let n = count as f32;
        c.x /= n;
        c.y /= n;
        c.z /= n;
        (c, count > 0)
    }

    /// `getMinMaxAndCenter(&min, &max, &center)`: of the members with an AI, not held; whether they are a formation.
    pub fn get_min_max_and_center<H: AiGroupHost>(&mut self, h: &mut H) -> (Coord2D, Coord2D, Coord3D, bool) {
        let mut count = 0i32;
        let mut min = Coord2D { x: 1e10, y: 1e10 };
        let mut max = Coord2D { x: -1e10, y: -1e10 };
        let mut c = Coord3D::default();
        let mut id = NO_FORMATION_ID;
        for &o in &self.members.clone() {
            if h.is_held(o) {
                continue;
            }
            if h.has_ai(o) {
                let p = h.position(o);
                c.x += p.x;
                c.y += p.y;
                c.z += p.z;
                min.x = if min.x > p.x { p.x } else { min.x };
                max.x = if max.x < p.x { p.x } else { max.x };
                min.y = if min.y > p.y { p.y } else { min.y };
                max.y = if max.y < p.y { p.y } else { max.y };
                let cur = h.formation_id(o);
                if count == 0 {
                    id = cur;
                }
                // (else `if (id == NO_FORMATION_ID) id = NO_FORMATION_ID;`)
                count += 1;
            }
        }
        let n = count as f32;
        c.x /= n;
        c.y /= n;
        c.z /= n;
        let mut is_formation = id != NO_FORMATION_ID;
        if count < 2 && !plant(3532) {
            is_formation = false;
        }
        (min, max, c, is_formation)
    }

    /// `recompute()`: the slowest mobile, not held member's speed (of those whose damage is better than the movement penalty's).
    pub fn recompute<H: AiGroupHost>(&mut self, h: &mut H) {
        let mut close_dist = 999_999_999.9f32;
        let (center, _) = self.get_center(h);
        self.ground_path = None;
        self.speed = 9_999_999_999.9f32;
        let penalty = h.movement_penalty_damage_state();
        for &o in &self.members.clone() {
            if h.is_kind_of(o, KINDOF_IMMOBILE) {
                continue;
            }
            if h.is_held(o) {
                continue;
            }
            if h.has_ai(o) {
                let max_speed = h.cur_locomotor_speed(o);
                if self.speed > max_speed && is_condition_better(h.damage_state(o), penalty) && !plant(3533) {
                    self.speed = max_speed;
                }
                let p = h.position(o);
                let (dx, dy) = (p.x - center.x, p.y - center.y);
                let dist = dx * dx + dy * dy;
                if dist < close_dist {
                    close_dist = dist;
                }
            }
        }
        self.dirty = false;
    }

    /// `getCount()`.
    pub fn get_count(&self) -> i32 {
        self.member_list_size as i32
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    // ---- the commands

    /// The members with an AI, each given the command.
    fn each_ai<H: AiGroupHost>(&self, h: &mut H, cmd: GroupAiCommand, src: CommandSourceType) {
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                h.ai_command(o, cmd, src);
            }
        }
    }

    /// `groupIdle(cmdSource)`: each AI idle (a player's stealthed-capable, auto-acquiring member not yet stealthed, unable to
    /// acquire while stealthed, waits its stealth delay and up to a second more for its mood); a member without an AI stops its
    /// contained objects; a spawner its slaves.
    pub fn group_idle<H: AiGroupHost>(&self, h: &mut H, src: CommandSourceType) {
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                h.ai_command(o, GroupAiCommand::Idle, src);
                if src == CMD_FROM_PLAYER
                    && h.status_test(o, OBJECT_STATUS_CAN_STEALTH)
                    && h.ai_can_auto_acquire(o)
                    && !h.status_test(o, OBJECT_STATUS_STEALTHED)
                    && !h.status_test(o, OBJECT_STATUS_DETECTED)
                    && !h.ai_can_auto_acquire_while_stealthed(o)
                {
                    if let Some(stealth_frames) = h.stealth_delay(o) {
                        let random_frames = h.logic_random_value(0, LOGICFRAMES_PER_SECOND) as u32;
                        let t = h.frame().wrapping_add(stealth_frames).wrapping_add(random_frames);
                        h.ai_set_next_mood_check_time(o, t);
                    }
                }
            } else if h.has_contain(o) {
                // (makeMemberStop on each contained object)
                for c in h.contained(o) {
                    if h.has_ai(c) && !plant(3534) {
                        h.ai_command(c, GroupAiCommand::Idle, src);
                    }
                }
            }
            if h.has_spawn(o) {
                h.spawn_order_slaves_idle(o, src);
            }
        }
    }

    /// `groupFollowPath(path, ignoreObject, cmdSource)`: nothing.
    pub fn group_follow_path(&self) {}

    /// `groupAttackObjectPrivate(forced, victim, maxShotsToFire, cmdSource)`: the members not held, nearest the victim first: the
    /// passengers allowed to fire that may attack it, the slaves without freedom, and the member's own AI (not the victim's).
    pub fn group_attack_object_private<H: AiGroupHost>(&self, h: &mut H, forced: bool, victim: ObjectId, max_shots: i32, src: CommandSourceType) {
        if victim == INVALID_ID {
            return;
        }
        let victim_pos = h.position(victim);
        let mut iter = SimpleObjectIterator::new();
        for &o in &self.members.clone() {
            let p = h.position(o);
            if h.is_held(o) {
                continue; // "don't bother telling the occupants to move."
            }
            let (dx, dy) = (p.x - victim_pos.x, p.y - victim_pos.y);
            let _ = iter.insert(Some(o), dx * dx + dy * dy);
        }
        iter.sort(IterOrderType::SortedNearToFar, |_| 0);
        let mut unit = iter.first();
        while let Some(u) = unit {
            if h.has_contain(u) && h.contain_passengers_allowed_to_fire(u) {
                for g in h.contained(u) {
                    let t = if forced { ATTACK_NEW_TARGET_FORCED } else { ATTACK_NEW_TARGET };
                    let result = h.able_to_attack_specific_object(g, t, victim, src);
                    if (result == ATTACKRESULT_POSSIBLE || result == ATTACKRESULT_POSSIBLE_AFTER_MOVING) && h.has_ai(g) {
                        h.ai_command(g, GroupAiCommand::AttackObject { victim, max_shots, forced }, src);
                    }
                }
            }
            if h.has_spawn(u) && !h.spawn_slaves_have_freedom(u) {
                h.spawn_order_slaves_attack_target(u, victim, max_shots, src);
            }
            if h.has_ai(u) && u != victim && !plant(3535) {
                h.ai_command(u, GroupAiCommand::AttackObject { victim, max_shots, forced }, src);
            }
            unit = iter.next();
        }
    }

    /// `groupAttackObject(victim, ..)` and `groupForceAttackObject` (AI.h's inlines).
    pub fn group_attack_object<H: AiGroupHost>(&self, h: &mut H, victim: ObjectId, max_shots: i32, src: CommandSourceType) {
        self.group_attack_object_private(h, false, victim, max_shots, src);
    }
    pub fn group_force_attack_object<H: AiGroupHost>(&self, h: &mut H, victim: ObjectId, max_shots: i32, src: CommandSourceType) {
        self.group_attack_object_private(h, true, victim, max_shots, src);
    }

    /// `groupAttackTeam(team, maxShotsToFire, cmdSource)` (`None`: no team).
    pub fn group_attack_team<H: AiGroupHost>(&self, h: &mut H, team: Option<u32>, max_shots: i32, src: CommandSourceType) {
        let Some(team) = team else { return };
        self.each_ai(h, GroupAiCommand::AttackTeam { team, max_shots }, src);
    }

    /// `groupAttackPosition(pos, maxShotsToFire, cmdSource)` (`None`: each member's own position): the passengers allowed to fire
    /// that may, the slaves without freedom, the member's AI.
    pub fn group_attack_position<H: AiGroupHost>(&self, h: &mut H, pos: Option<&Coord3D>, max_shots: i32, src: CommandSourceType) {
        // (attackPos uninitialised upstream without a position: set from the first member before use)
        let mut attack_pos = pos.copied().unwrap_or_default();
        for &o in &self.members.clone() {
            if pos.is_none() {
                attack_pos = h.position(o);
            }
            if h.has_contain(o) && h.contain_passengers_allowed_to_fire(o) {
                for g in h.contained(o) {
                    let result = h.able_to_use_weapon_against_position(g, ATTACK_NEW_TARGET, &attack_pos, src);
                    if (result == ATTACKRESULT_POSSIBLE || result == ATTACKRESULT_POSSIBLE_AFTER_MOVING) && h.has_ai(g) {
                        h.ai_command(g, GroupAiCommand::AttackPosition { pos: attack_pos, max_shots }, src);
                    }
                }
            }
            if h.has_spawn(o) && !h.spawn_slaves_have_freedom(o) {
                h.spawn_order_slaves_attack_position(o, &attack_pos, max_shots, src);
            }
            if h.has_ai(o) {
                h.ai_command(o, GroupAiCommand::AttackPosition { pos: attack_pos, max_shots }, src);
            }
        }
    }

    /// `groupAttackMoveToPosition(pos, maxShotsToFire, cmdSource)`: those able to attack attack-move, the others move.
    pub fn group_attack_move_to_position<H: AiGroupHost>(&self, h: &mut H, pos: &Coord3D, max_shots: i32, src: CommandSourceType) {
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                if h.is_able_to_attack(o) && !plant(3536) {
                    h.ai_command(o, GroupAiCommand::AttackMoveToPosition { pos: *pos, max_shots }, src);
                } else {
                    h.ai_command(o, GroupAiCommand::MoveToPosition { pos: *pos }, src);
                }
            }
        }
    }

    /// `groupHunt`, `groupRepair`, `groupResumeConstruction`, `groupGetHealed`, `groupGetRepaired`, `groupEnter`, `groupDock`,
    /// `groupExit`, `groupExecuteRailedTransport`, `groupGoProne`, `groupHackInternet`, `groupCombatDrop`: each AI given the
    /// command.
    pub fn group_command<H: AiGroupHost>(&self, h: &mut H, cmd: GroupAiCommand, src: CommandSourceType) {
        self.each_ai(h, cmd, src);
    }

    /// `groupGuardPosition`, `groupGuardObject`, `groupGuardArea`, `groupAttackArea`: nothing without the position, object or
    /// area (`None`), else each AI.
    pub fn group_guard<H: AiGroupHost>(&self, h: &mut H, cmd: Option<GroupAiCommand>, src: CommandSourceType) {
        if let Some(cmd) = cmd {
            self.each_ai(h, cmd, src);
        }
    }

    /// `groupEvacuate(cmdSource)`: an airborne aircraft lands at its spot's highest layer and evacuates there; another AI
    /// evacuates; a structure without one orders its passengers out.
    pub fn group_evacuate<H: AiGroupHost>(&self, h: &mut H, src: CommandSourceType) {
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                if h.is_kind_of(o, KINDOF_AIRCRAFT) && h.is_airborne_target(o) && !plant(3537) {
                    let mut pos = h.position(o);
                    let layer = h.highest_layer_for_destination(&pos);
                    pos.z = h.layer_height(pos.x, pos.y, layer);
                    h.ai_command(o, GroupAiCommand::MoveToAndEvacuate { pos }, src);
                } else {
                    h.ai_command(o, GroupAiCommand::Evacuate { expose_stealth_units: false }, src);
                }
            } else if h.is_kind_of(o, KINDOF_STRUCTURE) && h.has_contain(o) {
                h.contain_order_all_passengers_to_exit(o, src);
            }
        }
    }

    /// `groupCreateFormation(cmdSource)`: a new formation id (none when the group already is one, or is one member in one), each
    /// AI member given it and its offset from the centre.
    pub fn group_create_formation<H: AiGroupHost>(&mut self, h: &mut H) {
        let (_, _, center, mut is_formation) = self.get_min_max_and_center(h);
        let mut id = h.ai_next_formation_id();
        let mut count = 0;
        let mut count_id = NO_FORMATION_ID;
        for &o in &self.members.clone() {
            count += 1;
            count_id = h.formation_id(o);
        }
        if count == 1 && count_id != NO_FORMATION_ID {
            is_formation = true;
        }
        if is_formation && !plant(3538) {
            id = NO_FORMATION_ID;
        }
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                let p = h.position(o);
                let offset = Coord2D { x: p.x - center.x, y: p.y - center.y };
                h.set_formation_id(o, id);
                h.set_formation_offset(o, offset);
            }
        }
    }

    /// The special power's template and the member's science: whether the member may (`groupDoSpecialPower*`'s shared test).
    fn special_power_allowed<H: AiGroupHost>(h: &mut H, o: ObjectId, sp: u32) -> bool {
        let Some(science) = h.special_power_required_science(sp) else { return false };
        if science != SCIENCE_INVALID && !h.player_has_science(o, science) {
            return false; // "Nice try, smacktard."
        }
        h.has_special_power_module(o, sp)
    }

    /// `groupDoSpecialPower(specialPowerID, commandOptions)`.
    pub fn group_do_special_power<H: AiGroupHost>(&self, h: &mut H, sp: u32, options: u32) {
        for &o in &self.members.clone() {
            if Self::special_power_allowed(h, o, sp) && h.can_do_special_power(o, sp, options) {
                h.do_special_power(o, sp, options);
                h.clear_undetected_defector(o); // "My secret is out"
            }
        }
    }

    /// `groupDoSpecialPowerAtLocation(specialPowerID, location, angle, objectInWay, commandOptions)` (the next member taken first,
    /// "just in case the act of specialpowering changes this list").
    pub fn group_do_special_power_at_location<H: AiGroupHost>(&self, h: &mut H, sp: u32, loc: &Coord3D, angle: f32, object_in_way: ObjectId, options: u32) {
        for &o in &self.members.clone() {
            if Self::special_power_allowed(h, o, sp) && h.can_do_special_power_at_location(o, sp, loc, object_in_way, options) && !plant(3539) {
                h.do_special_power_at_location(o, sp, loc, angle, options);
                h.clear_undetected_defector(o);
            }
        }
    }

    /// `groupDoSpecialPowerAtObject(specialPowerID, target, commandOptions)`.
    pub fn group_do_special_power_at_object<H: AiGroupHost>(&self, h: &mut H, sp: u32, target: ObjectId, options: u32) {
        for &o in &self.members.clone() {
            if Self::special_power_allowed(h, o, sp) && h.can_do_special_power_at_object(o, sp, target, options) {
                h.do_special_power_at_object(o, sp, target, options);
                h.clear_undetected_defector(o);
            }
        }
    }

    /// `groupCheer(cmdSource)`: three seconds of cheering.
    pub fn group_cheer<H: AiGroupHost>(&self, h: &mut H) {
        for &o in &self.members.clone() {
            h.set_cheering(o, LOGICFRAMES_PER_SECOND * 3);
        }
    }

    /// `groupSell(cmdSource)`: each sold (`TheBuildAssistant->sellObject`).
    pub fn group_sell<H: AiGroupHost>(&self, h: &mut H) {
        for &o in &self.members.clone() {
            h.sell(o);
        }
    }

    /// `groupToggleOvercharge(cmdSource)`.
    pub fn group_toggle_overcharge<H: AiGroupHost>(&self, h: &mut H) {
        for &o in &self.members.clone() {
            h.toggle_overcharge(o);
        }
    }

    /// `groupDoCommandButton*(commandButton, .., cmdSource)`.
    pub fn group_do_command_button<H: AiGroupHost>(&self, h: &mut H, button: u32, call: GroupButtonCall, src: CommandSourceType) {
        for &o in &self.members.clone() {
            h.do_command_button(o, button, call, src);
        }
    }

    /// `setAttitude(tude)`.
    pub fn set_attitude<H: AiGroupHost>(&self, h: &mut H, tude: AttitudeType) {
        for &o in &self.members.clone() {
            if h.has_ai(o) {
                h.ai_set_attitude(o, tude);
            }
        }
    }

    /// `getAttitude()`: `AI_PASSIVE`.
    pub fn get_attitude(&self) -> AttitudeType {
        AI_PASSIVE
    }

    /// `setMineClearingDetail(set)`: `WEAPONSET_MINE_CLEARING_DETAIL` set or cleared on each.
    pub fn set_mine_clearing_detail<H: AiGroupHost>(&self, h: &mut H, set: bool, mine_clearing_detail: i32) {
        for &o in &self.members.clone() {
            if set {
                h.set_weapon_set_flag(o, mine_clearing_detail);
            } else {
                h.clear_weapon_set_flag(o, mine_clearing_detail);
            }
        }
    }

    /// `setWeaponLockForGroup(weaponSlot, lockType)`: whether any member locked.
    pub fn set_weapon_lock_for_group<H: AiGroupHost>(&self, h: &mut H, slot: i32, lock: i32) -> bool {
        let mut any = false;
        for &o in &self.members.clone() {
            if h.set_weapon_lock(o, slot, lock) {
                any = true;
            }
        }
        any
    }

    /// `releaseWeaponLockForGroup(lockType)`.
    pub fn release_weapon_lock_for_group<H: AiGroupHost>(&self, h: &mut H, lock: i32) {
        for &o in &self.members.clone() {
            h.release_weapon_lock(o, lock);
        }
    }

    /// `setWeaponSetFlag(wst)`: on each whose template has such a weapon set.
    pub fn set_weapon_set_flag<H: AiGroupHost>(&self, h: &mut H, wst: i32) {
        for &o in &self.members.clone() {
            if h.template_has_weapon_set(o, wst) && !plant(3540) {
                h.set_weapon_set_flag(o, wst);
            }
        }
    }

    /// `queueUpgrade(upgrade)` (`None`: no upgrade): queued by each member that can afford it, still needs it (an object's
    /// upgrade), can produce it, has a production update and room in its queue.
    pub fn queue_upgrade<H: AiGroupHost>(&self, h: &mut H, upgrade: Option<u32>) {
        let Some(upgrade) = upgrade else { return };
        for &o in &self.members.clone() {
            if !h.can_afford_upgrade(o, upgrade) {
                continue;
            }
            if h.upgrade_is_object_type(upgrade) && (h.has_upgrade(o, upgrade) || !h.affected_by_upgrade(o, upgrade)) {
                continue;
            }
            if !h.can_produce_upgrade(o, upgrade) {
                continue; // "They have faked their button"
            }
            match h.production_queue_full_for_upgrade(o, upgrade) {
                None => continue,
                Some(true) if !plant(3541) => continue,
                _ => {}
            }
            h.production_queue_upgrade(o, upgrade);
        }
    }

    /// `isIdle()`: every AI member idle or dead (none: true).
    pub fn is_idle<H: AiGroupHost>(&self, h: &mut H) -> bool {
        let mut idle = true;
        for &o in &self.members.clone() {
            if !h.has_ai(o) {
                continue;
            }
            idle = h.ai_is_idle(o) || h.is_effectively_dead(o);
            if !idle {
                return false;
            }
        }
        idle
    }

    /// `isBusy()`: every AI member busy and alive (none: true).
    pub fn is_busy<H: AiGroupHost>(&self, h: &mut H) -> bool {
        let mut busy = true;
        for &o in &self.members.clone() {
            if !h.has_ai(o) {
                continue;
            }
            busy = h.ai_is_busy(o) && !h.is_effectively_dead(o);
            if !busy {
                return false;
            }
        }
        busy
    }

    /// `isGroupAiDead()`: every member effectively dead.
    pub fn is_group_ai_dead<H: AiGroupHost>(&self, h: &mut H) -> bool {
        let mut dead = true;
        for &o in &self.members.clone() {
            dead = dead && h.is_effectively_dead(o);
        }
        dead
    }

    /// `getSpecialPowerSourceObject(specialPowerID)`: the first member with the special power's module.
    pub fn get_special_power_source_object<H: AiGroupHost>(&self, h: &mut H, sp: u32) -> Option<ObjectId> {
        h.special_power_required_science(sp)?;
        self.members.clone().into_iter().find(|&o| h.has_special_power_module(o, sp))
    }

    /// `getCommandButtonSourceObject(type)`: the first member whose command set has a button of the type.
    pub fn get_command_button_source_object<H: AiGroupHost>(&self, h: &mut H, command_type: i32) -> Option<ObjectId> {
        self.members.clone().into_iter().find(|&o| h.command_set_has_button_type(o, command_type) == Some(true))
    }

    /// `groupSetEmoticon(name, duration)`.
    pub fn group_set_emoticon<H: AiGroupHost>(&self, h: &mut H, name: &[u8], duration: i32) {
        for &o in &self.members.clone() {
            h.set_emoticon(o, name, duration);
        }
    }

    /// `groupOverrideSpecialPowerDestination(spType, loc, cmdSource)`.
    pub fn group_override_special_power_destination<H: AiGroupHost>(&self, h: &mut H, sp_type: i32, loc: &Coord3D) {
        for &o in &self.members.clone() {
            h.override_special_power_destination(o, sp_type, loc);
        }
    }

    /// `crc(xfer)`: the members' ids, the count, a former leader's id (`INVALID_ID`), the speed, the dirty flag and the id.
    pub fn crc(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        for &o in &self.members {
            let mut b = o.to_le_bytes();
            xfer.xfer_user(&mut b)?;
        }
        xfer.xfer_unsigned_int(&mut self.member_list_size)?;
        let mut id: ObjectId = INVALID_ID; // "Used to be leader id, unused now."
        xfer.xfer_object_id(&mut id)?;
        xfer.xfer_real(&mut self.speed)?;
        xfer.xfer_bool(&mut self.dirty)?;
        xfer.xfer_unsigned_int(&mut self.id)
    }

    /// `xfer(xfer)` (version 1): the version only.
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> XferResult<()> {
        let current: XferVersion = 1;
        let mut version = current;
        xfer.xfer_version(&mut version, current)
    }
}
