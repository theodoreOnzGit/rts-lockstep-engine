// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/AssaultTransportAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/AssaultTransportAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `AssaultTransportAIUpdate` (`p1w13-sonnet`, layer 10, batch A32): a transport (the Troop Crawler's kind) whose passengers are
//! its assault members: told to attack, it lets the healthy ones out to fight the target its weapon designated (`beginAssault`),
//! takes the wounded back in to heal, and calls them all back when the attack is over.
//!
//! The module's state is [`AssaultTransport`], held in [`AiSubclass::AssaultTransport`]. [`AiUpdateModule::update`] takes
//! [`assault_transport_update`] for it, [`commands::ai_do_command`] its `aiDoCommand` ([`assault_transport_do_command`]: a
//! command not from the AI resets the assault first), and [`AiUpdateModule::xfer`] its `xfer` (version 1). `isIdle` and
//! `loadPostProcess` are the base's. The members' AIs are other objects' (`AiStateHost::member_ai_*`).
//!
//! # Upstream facts kept
//!
//! - Removing a member moves the last one into its slot and goes on with the next slot: the moved member is not looked at again
//!   until the next update.
//! - `beginAssault` (the weapon's) only sets the designated target; the `m_state` and `m_framesRemaining` members are never changed
//!   after `reset` (the transport's own manoeuvres are commented out upstream), but are saved.
//! - The fighters' centroid is gathered and never used (it fed the commented-out code); it is not computed here.
//! - `xfer` saves neither `m_newMember` nor `m_newOccupantsAreNewMembers`; a load keeps the slots past the saved count as they were.
//! - Only the member ids and healing flags up to the saved count are loaded; a count above `MAX_TRANSPORT_SLOTS` would overrun
//!   upstream's arrays (refused here with `XferStatus::ErrorUnknown`).
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update
//! [`AiUpdateModule::xfer`]: crate::logic::ai::ai_states::AiUpdateModule::xfer
//! [`commands::ai_do_command`]: crate::logic::ai::ai_commands::ai_do_command

use crate::common::base_type::Coord3D;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::object_status_types::OBJECT_STATUS_IS_ATTACKING;
use crate::common::state_machine::{SmResult, StateMachine};
use crate::common::xfer::{Xfer, XferResult, XferStatus};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms, AICMD_ATTACKMOVE_TO_POSITION, AICMD_ATTACK_OBJECT, AICMD_IDLE};
use crate::logic::ai::ai_states::{ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, AI_ATTACK_MOVE_TO, AI_ENTER, CMD_FROM_AI, CMD_FROM_PLAYER};
use crate::logic::module::update_module::{UpdateSleepTime, UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE};
use crate::logic::modules::plant;
use crate::logic::weapon::NO_MAX_SHOTS_LIMIT;

type Sm<H> = StateMachine<AiState<H>>;

/// `MAX_TRANSPORT_SLOTS`.
pub const MAX_TRANSPORT_SLOTS: usize = 10;

/// `AssaultStateTypes`' `IDLE` ("Not doing anything.").
pub const ASSAULT_IDLE: i32 = 0;

/// A command to another object's AI (an assault member's).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MemberCommand {
    /// `aiExit(transport, src)`.
    Exit { transport: ObjectId, src: CommandSourceType },
    /// `aiEnter(transport, src)`.
    Enter { transport: ObjectId, src: CommandSourceType },
    /// `aiAttackObject(victim, maxShots, src)`.
    AttackObject { victim: ObjectId, max_shots: i32, src: CommandSourceType },
    /// `aiAttackMoveToPosition(pos, maxShots, src)`.
    AttackMoveToPosition { pos: Coord3D, max_shots: i32, src: CommandSourceType },
    /// Batch A37 (a delivered item's): `aiMoveToPosition(pos, src)`.
    MoveToPosition { pos: Coord3D, src: CommandSourceType },
}

/// `AssaultTransportAIUpdateModuleData`'s own fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssaultTransportData {
    /// `MembersGetHealedAtLifeRatio` (default 0).
    pub members_get_healed_at_life_ratio: f32,
    /// `ClearRangeRequiredToContinueAttackMove` (default 50; never read upstream).
    pub clear_range_required_to_continue_attack_move: f32,
}

impl Default for AssaultTransportData {
    fn default() -> Self {
        AssaultTransportData { members_get_healed_at_life_ratio: 0.0, clear_range_required_to_continue_attack_move: 50.0 }
    }
}

/// `AssaultTransportAIUpdate`'s members, with its module data.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AssaultTransport {
    /// The module data's own fields.
    pub data: AssaultTransportData,
    /// `m_memberIDs`.
    pub member_ids: [ObjectId; MAX_TRANSPORT_SLOTS],
    /// `m_memberHealing`.
    pub member_healing: [bool; MAX_TRANSPORT_SLOTS],
    /// `m_newMember`.
    pub new_member: [bool; MAX_TRANSPORT_SLOTS],
    /// `m_attackMoveGoalPos`.
    pub attack_move_goal_pos: Coord3D,
    /// `m_designatedTarget`.
    pub designated_target: ObjectId,
    /// `m_state` (an `AssaultStateTypes`; as loaded, any value).
    pub state: i32,
    /// `m_framesRemaining`.
    pub frames_remaining: u32,
    /// `m_currentMembers`.
    pub current_members: i32,
    /// `m_isAttackMove`.
    pub is_attack_move: bool,
    /// `m_isAttackObject`.
    pub is_attack_object: bool,
    /// `m_newOccupantsAreNewMembers`.
    pub new_occupants_are_new_members: bool,
}

impl AssaultTransport {
    /// `AssaultTransportAIUpdate(thing, moduleData)`: "First time, max it out, to ensure clearing arrays in reset."
    pub fn new(data: AssaultTransportData) -> Self {
        let mut a = AssaultTransport { data, current_members: MAX_TRANSPORT_SLOTS as i32, ..Default::default() };
        a.reset();
        a
    }

    /// `reset()`: the member slots in use cleared, then everything else.
    pub fn reset(&mut self) {
        for i in 0..self.current_members.max(0) as usize {
            self.member_ids[i] = INVALID_ID;
            self.member_healing[i] = false;
            self.new_member[i] = false;
        }
        self.current_members = 0;
        self.attack_move_goal_pos = Coord3D::default();
        self.designated_target = INVALID_ID;
        self.state = ASSAULT_IDLE;
        self.frames_remaining = 0;
        self.is_attack_move = false;
        self.is_attack_object = false;
        self.new_occupants_are_new_members = false;
    }

    /// `isAttackPointless()`: "If all members are new members (thus can't attack), and the transport itself is still attacking,
    /// stop!"
    fn is_attack_pointless(&self, attacking: bool) -> bool {
        attacking && !self.new_member[..self.current_members as usize].iter().any(|n| !n)
    }
}

/// The running AI's `AssaultTransportAIUpdate` members (the AI is one).
fn at<H: AiStateHost>(h: &mut H) -> &mut AssaultTransport {
    match &mut ai(h).subclass {
        AiSubclass::AssaultTransport(a) => a,
        _ => unreachable!("an AssaultTransportAIUpdate"),
    }
}

/// `isMemberWounded(member)`: "Member requires medical attention?" (its body's health ratio below the data's).
fn is_member_wounded<H: AiStateHost>(h: &mut H, member: ObjectId) -> bool {
    let ratio_limit = at(h).data.members_get_healed_at_life_ratio;
    match h.object_health(member) {
        Some((health, max_health)) => health / max_health < ratio_limit,
        None => false,
    }
}

/// `isMemberHealthy(member)`: "Member has full health?"
fn is_member_healthy<H: AiStateHost>(h: &mut H, member: ObjectId) -> bool {
    matches!(h.object_health(member), Some((health, max_health)) if health == max_health)
}

/// `AssaultTransportAIUpdate::aiDoCommand(parms)`'s own part (the AI lent), before the base's: "Inspect the command and reset
/// everything when necessary."
pub fn assault_transport_do_command<H: AiStateHost>(h: &mut H, parms: &AiCommandParms) {
    if parms.cmd_source == CMD_FROM_AI {
        return;
    }
    // "Now the only time we care about anything is if we were ordered to attack something or attack move."
    match parms.cmd {
        AICMD_ATTACKMOVE_TO_POSITION => {
            // "Reset because we have been ordered to do something."
            let a = at(h);
            a.reset();
            a.attack_move_goal_pos = parms.pos;
            a.is_attack_move = true;
        }
        AICMD_ATTACK_OBJECT => {
            let a = at(h);
            a.reset();
            a.is_attack_object = !plant(3630);
        }
        AICMD_IDLE => {
            at(h).designated_target = INVALID_ID;
            // "Order all outside members back inside!"
            retrieve_members(h);
            at(h).reset();
        }
        // "Reset because we have been ordered to do something we're not handling."
        _ => at(h).reset(),
    }
}

/// `beginAssault(designatedTarget)`: "The transport has determined it is in range to begin the assault (via weapon system)."
pub fn begin_assault(a: &mut AssaultTransport, designated_target: Option<ObjectId>) {
    if let Some(t) = designated_target {
        a.designated_target = t;
    }
}

/// `retrieveMembers()`: "Order all outside members back inside!"
fn retrieve_members<H: AiStateHost>(h: &mut H) {
    let me = h.object_id();
    for i in 0..at(h).current_members as usize {
        let member = at(h).member_ids[i];
        if h.object_exists(member) && h.object_has_ai(member) && !h.object_is_contained(member) && h.member_ai_state_type(member) != AI_ENTER {
            h.member_ai_command(member, MemberCommand::Enter { transport: me, src: CMD_FROM_AI });
        }
    }
}

/// `giveFinalOrders()`: "All members have been ejected outside already -- transfer the original order to the troops".
fn give_final_orders<H: AiStateHost>(h: &mut H) {
    for i in 0..at(h).current_members as usize {
        let a = *at(h);
        let member = a.member_ids[i];
        if h.object_exists(member) && h.object_has_ai(member) {
            let target = a.designated_target;
            if a.is_attack_object && h.object_exists(target) {
                h.member_ai_command(member, MemberCommand::AttackObject { victim: target, max_shots: NO_MAX_SHOTS_LIMIT, src: CMD_FROM_PLAYER });
            } else if a.is_attack_move {
                let pos = a.attack_move_goal_pos;
                h.member_ai_command(member, MemberCommand::AttackMoveToPosition { pos, max_shots: NO_MAX_SHOTS_LIMIT, src: CMD_FROM_PLAYER });
            }
            h.member_ai_set_allowed_to_chase(member, false);
        }
    }
}

/// `AssaultTransportAIUpdate::update()`'s own part (the AI lent): the members updated (the gone, the dead and those another
/// command took dropped; the passengers not yet members added), then the assault (the healthy members let out to attack the
/// designated target, the wounded called back in) or, without a target, the attack-move continued or the members called back.
/// `Some(sleep)` returns at once (dead: the final orders, `UPDATE_SLEEP_FOREVER`; a pointless attack: idle); `None` goes on to
/// `AIUpdateInterface::update()`.
pub fn assault_transport_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<Option<UpdateSleepTime>> {
    if h.is_effectively_dead() {
        give_final_orders(h);
        return Ok(Some(UPDATE_SLEEP_FOREVER));
    }

    // "First removing dead members or members that have been ordered to do something outside of this AI."
    let mut i = 0usize;
    while (i as i32) < at(h).current_members {
        let member = at(h).member_ids[i];
        let exists = h.object_exists(member);
        let has_ai = exists && h.object_has_ai(member);
        let toast = !exists || h.object_is_effectively_dead(member) || h.member_ai_last_command_source(member) != CMD_FROM_AI;
        if toast && !plant(3631) {
            // "Member is toast -- so remove him from our list!"
            let a = at(h);
            let last = (a.current_members - 1) as usize;
            if last > i {
                // "Move the last slot to this slot to keep array contiguous."
                a.member_ids[i] = a.member_ids[last];
                a.member_healing[i] = a.member_healing[last];
                a.new_member[i] = a.new_member[last];
            } else {
                // "Just clean out last slot."
                a.member_ids[i] = INVALID_ID;
                a.member_healing[i] = false;
                a.new_member[i] = false;
            }
            if has_ai {
                // "We need to turn this off though, because this ex-member is no longer under transport control."
                h.member_ai_set_allowed_to_chase(member, false);
            }
            at(h).current_members -= 1;
        }
        i += 1;
    }

    // "Now add any potentially new members to the group."
    if h.has_contain() {
        let me = h.object_id();
        for passenger in h.contain_items(me) {
            // "Make sure it isn't in our list already."
            let a = *at(h);
            if a.member_ids[..a.current_members as usize].contains(&passenger) {
                continue;
            }
            // "It's possible to add members manually -- but if we already have 10 members, then wait!"
            if (a.current_members as usize) < MAX_TRANSPORT_SLOTS {
                let k = a.current_members as usize;
                at(h).member_ids[k] = passenger;
                if h.object_has_ai(passenger) {
                    // "Members of our assault transport must be allowed to chase down designated enemies."
                    h.member_ai_set_allowed_to_chase(passenger, true);
                }
                // "Check if the passenger is wounded below threshhold (if so make sure we heal him before ordering him to fight!)"
                if is_member_wounded(h, passenger) {
                    at(h).member_healing[k] = true;
                }
                if at(h).new_occupants_are_new_members && !plant(3632) {
                    // "New members won't eject out until a new attack order is issued."
                    at(h).new_member[k] = true;
                }
                at(h).current_members += 1;
            }
        }
        at(h).new_occupants_are_new_members = true;
    }

    let attacking = h.status_test(OBJECT_STATUS_IS_ATTACKING);
    if at(h).is_attack_pointless(attacking) {
        commands::ai_idle(sm, h, CMD_FROM_AI)?;
        return Ok(Some(UPDATE_SLEEP_NONE));
    }

    // "If we're already in the process, reacquire the designated target again... see if it's still alive."
    let mut target = at(h).designated_target;
    if !h.object_exists(target) || h.object_is_effectively_dead(target) {
        target = INVALID_ID;
    }
    if target != INVALID_ID {
        let me = h.object_id();
        // "Look for members not currently attacking this target."
        for i in 0..at(h).current_members as usize {
            let member = at(h).member_ids[i];
            if !(h.object_exists(member) && h.object_has_ai(member)) {
                continue;
            }
            let contained = h.object_is_contained(member);
            let wounded = is_member_wounded(h, member);
            if contained && is_member_healthy(h, member) && !at(h).new_member[i] {
                // "This contained member is healthy so order him to exit to start fighting! New members are exempt!"
                h.member_ai_command(member, MemberCommand::Exit { transport: me, src: CMD_FROM_AI });
            }
            if !contained {
                if wounded {
                    if h.member_ai_state_type(member) != AI_ENTER && !plant(3633) {
                        // "Order wounded members back to get healed."
                        h.member_ai_command(member, MemberCommand::Enter { transport: me, src: CMD_FROM_AI });
                    }
                } else if !h.member_ai_is_moving(member) && h.member_ai_goal_object(member) != target {
                    // "Okay, this dude is outside and waiting... order him to attack the designated target"
                    h.member_ai_command(member, MemberCommand::AttackObject { victim: target, max_shots: NO_MAX_SHOTS_LIMIT, src: CMD_FROM_AI });
                }
            }
        }
    } else {
        let a = *at(h);
        if a.is_attack_move && sm.get_current_state_id() != AI_ATTACK_MOVE_TO {
            // "Continue to move towards the attackmove area."
            commands::ai_attack_move_to_position(sm, h, &a.attack_move_goal_pos, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI)?;
        } else if a.is_attack_object {
            retrieve_members(h);
        }
    }
    Ok(None)
}

/// `AssaultTransportAIUpdate::xfer`'s own members, after the base's.
pub fn assault_transport_xfer_members(a: &mut AssaultTransport, xfer: &mut dyn Xfer) -> XferResult<()> {
    xfer.xfer_int(&mut a.current_members)?;
    if a.current_members < 0 || a.current_members as usize > MAX_TRANSPORT_SLOTS {
        // (upstream's arrays overrun)
        return Err(XferStatus::ErrorUnknown);
    }
    for i in 0..a.current_members as usize {
        xfer.xfer_object_id(&mut a.member_ids[i])?;
        xfer.xfer_bool(&mut a.member_healing[i])?;
    }
    xfer.xfer_coord3d(&mut a.attack_move_goal_pos)?;
    xfer.xfer_object_id(&mut a.designated_target)?;
    xfer.xfer_int(&mut a.state)?;
    xfer.xfer_unsigned_int(&mut a.frames_remaining)?;
    xfer.xfer_bool(&mut a.is_attack_move)?;
    if !plant(3634) {
        xfer.xfer_bool(&mut a.is_attack_object)?;
    }
    Ok(())
}
