// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate.cpp (AIUpdateInterface::crc, xfer, loadPostProcess)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! The AI's snapshot: `AIUpdateInterface::crc`, `xfer` and `loadPostProcess` (`p1w13-sonnet`, layer 10, batch A21).
//!
//! The `UpdateModule` base's part, the turrets' AIs, the locomotor set's own snapshot and the names of the guarded area, the attack
//! priority info and the completed waypoint are the host's ([`AiStateHost`]); the state machine is [`aism_xfer`]'s, the path
//! [`Path`]'s.
//!
//! # Upstream facts kept
//!
//! - The guard target types are transferred as `xferUser(&m_guardTargetType[0], sizeof(m_guardTargetType))` and then
//!   `xferUser(&m_guardTargetType[1], sizeof(m_guardTargetType))`: the whole two-element array twice, the second time from its
//!   second element, so that its last four bytes are `m_locationToGuard.x`'s (the next member; saved as it is, loaded into it, then
//!   overwritten by the location's own transfer).
//! - `m_isSafePath` is transferred twice.
//! - On load the path is made anew only when one was saved: without one, the AI keeps the path it had.
//! - The state machine is saved through `xferSnapshot`, so a CRC visits its `crc`, which adds nothing; the path's and the turrets'
//!   likewise.
//! - Versions before 4 (a locomotor set by name, the last frame moved, the repulsor countdown, the demoralised and surrendered
//!   frames) are read as upstream reads them; this build saves version 4.

use crate::common::base_type::{Coord3D, ICoord2D};
use crate::common::state_machine::{SmResult, StateMachineError};
use crate::common::xfer::{Snapshot, Xfer, XferMode, XferStatus, XferVersion};
use crate::logic::ai::ai_states::{aism_xfer, AiStateHost, AiUpdateModule};
use crate::logic::ai::ai_update::LocoGoalType;
use crate::logic::ai::ai_update_types::*;
use crate::logic::ai::ai_waypoint_states::INVALID_WAYPOINT_ID;
use crate::logic::ai::path::{Path, PATHFIND_CELL_SIZE_F};
use crate::logic::locomotor::LocomotorSetError;
use crate::logic::modules::plant;

/// `MAX_WAYPOINTS`.
use crate::logic::ai::ai_update::MAX_WAYPOINTS;

/// A raw four-byte `xferUser` of an `Int`-sized value.
fn xfer_user_i32(xfer: &mut dyn Xfer, v: &mut i32) -> Result<(), XferStatus> {
    let mut b = v.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    *v = i32::from_le_bytes(b);
    Ok(())
}

impl From<LocomotorSetError> for StateMachineError {
    /// The locomotor set's snapshot throws its `XferStatus`.
    fn from(e: LocomotorSetError) -> Self {
        StateMachineError::Xfer(match e {
            LocomotorSetError::Xfer(s) => s,
            LocomotorSetError::NotEmpty => XferStatus::ListNotEmpty,
            LocomotorSetError::UnknownString(_) => XferStatus::UnknownString,
            LocomotorSetError::UnknownTemplate => XferStatus::ErrorUnknown,
        })
    }
}

impl<H: AiStateHost> AiUpdateModule<H> {
    /// `AIUpdateInterface::crc(x)`: the base's, then [`xfer`](Self::xfer) (in CRC mode: the state machine, the path and the turrets
    /// visit their `crc`).
    pub fn crc(
        &mut self,
        h: &mut H,
        xfer: &mut dyn Xfer,
        waypoint_name: impl Fn(u32) -> Vec<u8>,
        waypoint_by_name: impl Fn(&[u8]) -> Option<u32>,
    ) -> SmResult<()> {
        h.update_module_crc(xfer)?;
        self.xfer(h, xfer, waypoint_name, waypoint_by_name)
    }

    /// `xfer(xfer)` of the module's class: a subclass's (batch A30: `WanderAIUpdate`, `TransportAIUpdate`, version 1; batch A31:
    /// `DeployStyleAIUpdate`, version 4, its state and frame after the base's) then `AIUpdateInterface`'s. (`crc` calls it, as
    /// upstream's virtual `xfer(x)`.)
    pub fn xfer(
        &mut self,
        h: &mut H,
        xfer: &mut dyn Xfer,
        waypoint_name: impl Fn(u32) -> Vec<u8>,
        waypoint_by_name: impl Fn(&[u8]) -> Option<u32>,
    ) -> SmResult<()> {
        if let crate::logic::ai::ai_update::AiSubclass::DeployStyle(_) = self.ai().subclass {
            let current_version: XferVersion = 4;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            // "extend base class"
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            if version < 4 {
                // ("Good riddance!!!": an AICommandParmsStorage, not translated; this port writes version 4)
                return Err(XferStatus::InvalidVersion.into());
            }
            if let crate::logic::ai::ai_update::AiSubclass::DeployStyle(d) = &mut self.ai_mut().subclass {
                crate::logic::ai::deploy_style_ai_update::deploy_style_xfer_members(d, xfer)?;
            }
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::Dozer(_) = self.ai().subclass {
            // batch A38: `DozerAIUpdate::xfer` (version 1): the base's, then its members around its machine's snapshot (in CRC mode the
            // machine's `crc`, nothing)
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            let mut d = match &self.ai().subclass {
                crate::logic::ai::ai_update::AiSubclass::Dozer(d) => *d,
                _ => unreachable!(),
            };
            let machine = self.dozer_machine.as_mut().expect("the dozer's machine");
            let r = crate::logic::ai::dozer_ai_update::dozer_xfer_members(&mut d.m, xfer, |x| {
                if x.get_xfer_mode() == XferMode::Crc || plant(3721) {
                    return Ok(());
                }
                crate::logic::ai::dozer_ai_update::dozer_machine_xfer(machine, x)
            });
            self.ai_mut().subclass = crate::logic::ai::ai_update::AiSubclass::Dozer(d);
            r?;
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::DeliverPayload(_) = self.ai().subclass {
            // batch A37: `DeliverPayloadAIUpdate::xfer` (version 5): the base's, its members, its machine (made on a load if saved;
            // its snapshot whenever there is one; in CRC mode `StateMachine::crc`'s nothing), the decal, the flags
            let current_version: XferVersion = 5;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            let mut d = match &self.ai().subclass {
                crate::logic::ai::ai_update::AiSubclass::DeliverPayload(d) => d.clone(),
                _ => unreachable!(),
            };
            let r = crate::logic::ai::deliver_payload_ai_update::deliver_xfer_members(&mut d, h, xfer, version);
            self.ai_mut().subclass = crate::logic::ai::ai_update::AiSubclass::DeliverPayload(d.clone());
            r?;
            let mut has_state_machine = self.deliver_machine.is_some();
            xfer.xfer_bool(&mut has_state_machine)?;
            if has_state_machine && self.deliver_machine.is_none() {
                self.deliver_machine = Some(crate::logic::ai::deliver_payload_ai_update::make_deliver_machine()?);
            }
            if version >= 2 && xfer.get_xfer_mode() != XferMode::Crc && !plant(3696) {
                if let Some(m) = self.deliver_machine.as_mut() {
                    crate::logic::ai::deliver_payload_ai_update::deliver_machine_xfer(m, xfer)?;
                }
            }
            let r = crate::logic::ai::deliver_payload_ai_update::deliver_xfer_tail(&mut d, h, xfer, version);
            self.ai_mut().subclass = crate::logic::ai::ai_update::AiSubclass::DeliverPayload(d);
            r?;
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::Missile(_) = self.ai().subclass {
            // batch A36: `MissileAIUpdate::xfer` (version 6): the base's, then its members
            let current_version: XferVersion = 6;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            let mut m = match &self.ai().subclass {
                crate::logic::ai::ai_update::AiSubclass::Missile(m) => m.clone(),
                _ => unreachable!(),
            };
            let r = crate::logic::ai::missile_ai_update::missile_xfer_members(&mut m, h, xfer, version);
            self.ai_mut().subclass = crate::logic::ai::ai_update::AiSubclass::Missile(m);
            r?;
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::SupplyTruck(_) = self.ai().subclass {
            // batch A35: `SupplyTruckAIUpdate::xfer` (version 1): the base's, its machine, the preferred dock, the boxes, the
            // forced-wanting flag
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            // (`xferSnapshot`: in CRC mode the machine's `crc`, `StateMachine::crc`'s nothing)
            if xfer.get_xfer_mode() != XferMode::Crc && !plant(3667) {
                crate::logic::ai::supply_truck_ai_update::supply_machine_xfer(self.supply_machine.as_mut().expect("the supply truck's machine"), xfer)?;
            }
            if let crate::logic::ai::ai_update::AiSubclass::SupplyTruck(s) = &mut self.ai_mut().subclass {
                xfer.xfer_object_id(&mut s.preferred_dock)?;
                xfer.xfer_int(&mut s.number_boxes)?;
                if !plant(3666) {
                    xfer.xfer_bool(&mut s.force_pending)?;
                }
            }
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::HackInternet(_) = self.ai().subclass {
            // batch A34: `HackInternetAIUpdate::xfer` (version 1): the base's, then the waiting command
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            let mut k = match &mut self.ai_mut().subclass {
                crate::logic::ai::ai_update::AiSubclass::HackInternet(k) => std::mem::take(k),
                _ => unreachable!(),
            };
            let r = crate::logic::ai::hack_internet_ai_update::hack_internet_xfer_members(&mut k, h, xfer);
            self.ai_mut().subclass = crate::logic::ai::ai_update::AiSubclass::HackInternet(k);
            r?;
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::RailedTransport(_) = self.ai().subclass {
            // batch A33: `RailedTransportAIUpdate::xfer` (version 1): the base's, then its members
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            if let crate::logic::ai::ai_update::AiSubclass::RailedTransport(r) = &mut self.ai_mut().subclass {
                crate::logic::ai::railed_transport_ai_update::railed_transport_xfer_members(r, xfer)?;
            }
            return Ok(());
        }
        if let crate::logic::ai::ai_update::AiSubclass::AssaultTransport(_) = self.ai().subclass {
            // batch A32: `AssaultTransportAIUpdate::xfer` (version 1): the base's, then its members
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
            self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)?;
            if let crate::logic::ai::ai_update::AiSubclass::AssaultTransport(a) = &mut self.ai_mut().subclass {
                crate::logic::ai::assault_transport_ai_update::assault_transport_xfer_members(a, xfer)?;
            }
            return Ok(());
        }
        if self.ai().subclass != crate::logic::ai::ai_update::AiSubclass::Plain && !plant(3613) {
            let current_version: XferVersion = 1;
            let mut version = current_version;
            xfer.xfer_version(&mut version, current_version)?;
        }
        self.base_xfer(h, xfer, waypoint_name, waypoint_by_name)
    }

    /// `AIUpdateInterface::xfer(xfer)` (version 4).
    pub fn base_xfer(
        &mut self,
        h: &mut H,
        xfer: &mut dyn Xfer,
        waypoint_name: impl Fn(u32) -> Vec<u8>,
        waypoint_by_name: impl Fn(&[u8]) -> Option<u32>,
    ) -> SmResult<()> {
        let current_version: XferVersion = 4;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        // "extend base class"
        h.update_module_xfer(xfer)?;
        let load = xfer.get_xfer_mode() == XferMode::Load;
        {
            let ai = self.ai_mut();
            xfer.xfer_unsigned_int(&mut ai.prior_waypoint_id)?;
            xfer.xfer_unsigned_int(&mut ai.current_waypoint_id)?;
        }
        // xferSnapshot(m_stateMachine): AIStateMachine::crc adds nothing
        if xfer.get_xfer_mode() != XferMode::Crc {
            let (sm, ai) = (self.machine.as_mut().expect("the AI's machine"), self.ai.as_mut().expect("the AI (not lent)"));
            aism_xfer(sm, &mut ai.machine, xfer, waypoint_name, waypoint_by_name)?;
        }
        let ai = self.ai.as_mut().expect("the AI (not lent)");
        xfer.xfer_bool(&mut ai.is_ai_dead)?;
        xfer.xfer_bool(&mut ai.is_recruitable)?;
        xfer.xfer_unsigned_int(&mut ai.next_enemy_scan_time)?;
        xfer.xfer_object_id(&mut ai.current_victim_id)?;
        xfer.xfer_real(&mut ai.desired_speed)?;
        xfer_user_i32(xfer, &mut ai.last_command_source)?;
        // (the whole array from [0], then from [1]: its last word is m_locationToGuard.x's)
        {
            let mut b = [0u8; 8];
            b[..4].copy_from_slice(&ai.guard_target_type[0].to_le_bytes());
            b[4..].copy_from_slice(&ai.guard_target_type[1].to_le_bytes());
            xfer.xfer_user(&mut b)?;
            ai.guard_target_type[0] = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            ai.guard_target_type[1] = i32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            let mut b = [0u8; 8];
            b[..4].copy_from_slice(&ai.guard_target_type[1].to_le_bytes());
            b[4..].copy_from_slice(&ai.location_to_guard.x.to_le_bytes());
            xfer.xfer_user(&mut b)?;
            ai.guard_target_type[1] = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            if !plant(3500) {
                ai.location_to_guard.x = f32::from_le_bytes([b[4], b[5], b[6], b[7]]);
            }
        }
        xfer.xfer_coord3d(&mut ai.location_to_guard)?;
        xfer.xfer_object_id(&mut ai.object_to_guard)?;
        let mut trigger_name = ai.area_to_guard.map(|a| h.polygon_trigger_name(a)).unwrap_or_default();
        xfer.xfer_ascii_string(&mut trigger_name)?;
        if load && !trigger_name.is_empty() {
            ai.area_to_guard = h.polygon_trigger_by_name(&trigger_name);
        }
        let mut attack_name = ai.attack_info.map(|i| h.attack_info_name(i)).unwrap_or_default();
        xfer.xfer_ascii_string(&mut attack_name)?;
        if load && !attack_name.is_empty() {
            ai.attack_info = h.attack_info_by_name(&attack_name);
        }
        xfer.xfer_int(&mut ai.waypoint_count)?;
        if ai.waypoint_count < 0 || ai.waypoint_count > MAX_WAYPOINTS as i32 {
            // "Invalid waypoint count"
            return Err(StateMachineError::InvalidData);
        }
        for i in 0..ai.waypoint_count as usize {
            xfer.xfer_coord3d(&mut ai.waypoint_queue[i])?;
        }
        xfer.xfer_int(&mut ai.waypoint_index)?;
        xfer.xfer_bool(&mut ai.executing_waypoint_queue)?;
        let mut id = ai.completed_waypoint.unwrap_or(INVALID_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut id)?;
        if load {
            ai.completed_waypoint = h.waypoint_by_id(id);
        }
        xfer.xfer_bool(&mut ai.waiting_for_path)?;
        let mut got_path = ai.path.is_some();
        xfer.xfer_bool(&mut got_path)?;
        if load && got_path && !plant(3501) {
            ai.path = Some(Path::new());
        }
        if got_path {
            if let Some(p) = ai.path.as_mut() {
                xfer.xfer_snapshot(p)?;
            }
        }
        xfer.xfer_object_id(&mut ai.requested_victim_id)?;
        xfer.xfer_coord3d(&mut ai.requested_destination)?;
        xfer.xfer_coord3d(&mut ai.requested_destination2)?;
        // ("Not needed - we will recompute paths on load": m_pathTimestamp)
        xfer.xfer_object_id(&mut ai.ignore_obstacle_id)?;
        xfer.xfer_real(&mut ai.path_extra_distance)?;
        xfer.xfer_icoord2d(&mut ai.pathfind_goal_cell)?;
        xfer.xfer_icoord2d(&mut ai.pathfind_cur_cell)?;
        // ("Not needed": the blocked frames, the max blocked speed, the blocked flags, m_isInUpdate, m_fixLocoInPostProcess)
        xfer.xfer_unsigned_int(&mut ai.ignore_collisions_until)?;
        xfer.xfer_unsigned_int(&mut ai.queue_for_path_frame)?;
        xfer.xfer_coord3d(&mut ai.final_position)?;
        xfer.xfer_bool(&mut ai.do_final_position)?;
        xfer.xfer_bool(&mut ai.is_attack_path)?;
        xfer.xfer_bool(&mut ai.is_final_goal)?;
        xfer.xfer_bool(&mut ai.is_approach_path)?;
        xfer.xfer_bool(&mut ai.is_safe_path)?;
        xfer.xfer_bool(&mut ai.movement_complete)?;
        xfer.xfer_bool(&mut ai.is_safe_path)?;
        xfer.xfer_bool(&mut ai.upgraded_locomotors)?;
        xfer.xfer_bool(&mut ai.can_path_through_units)?;
        xfer.xfer_bool(&mut ai.randomly_offset_mood_check)?;
        xfer.xfer_object_id(&mut ai.repulsor1)?;
        xfer.xfer_object_id(&mut ai.repulsor2)?;
        if version < 3 {
            let mut last_frame_moved = 0i32;
            xfer.xfer_int(&mut last_frame_moved)?;
        }
        xfer.xfer_object_id(&mut ai.move_out_of_way1)?;
        xfer.xfer_object_id(&mut ai.move_out_of_way2)?;
        if load && version < 4 {
            // "Read in from .ini"
            let mut set_name = Vec::new();
            if ai.cur_locomotor_set > LOCOMOTORSET_INVALID && ai.cur_locomotor_set < LOCOMOTORSET_COUNT {
                set_name = LOCOMOTOR_SET_NAMES[ai.cur_locomotor_set as usize].as_bytes().to_vec();
            }
            xfer.xfer_ascii_string(&mut set_name)?;
            if !set_name.is_empty() {
                // (INI::scanIndexList: an unknown name throws)
                let i = LOCOMOTOR_SET_NAMES.iter().position(|n| n.as_bytes() == set_name.as_slice()).ok_or(StateMachineError::InvalidData)?;
                ai.cur_locomotor_set = i as LocomotorSetType;
            }
            ai.fix_loco_in_post_process = true;
        } else {
            if load {
                // "our ctor choose a NORMAL set for us. it's simpler to simply clear out whatever we have here"
                ai.locomotor_set.clear();
                ai.cur_locomotor = None;
            }
            h.locomotor_set_xfer(&mut ai.locomotor_set, &mut ai.cur_locomotor, xfer)?;
            xfer_user_i32(xfer, &mut ai.cur_locomotor_set)?;
        }
        let mut goal_type = ai.locomotor_goal_type as i32;
        xfer_user_i32(xfer, &mut goal_type)?;
        ai.locomotor_goal_type = match goal_type {
            0 => LocoGoalType::None,
            1 => LocoGoalType::PositionOnPath,
            2 => LocoGoalType::PositionExplicit,
            3 => LocoGoalType::Angle,
            // (an enum holding another value: undefined upstream)
            _ => return Err(StateMachineError::InvalidData),
        };
        xfer.xfer_coord3d(&mut ai.locomotor_goal_data)?;
        for i in 0..ai.has_turret_ai.len() {
            if ai.has_turret_ai[i] {
                h.turret_ai_snapshot(i, xfer)?;
            }
        }
        xfer_user_i32(xfer, &mut ai.turret_sync_flag)?;
        xfer_user_i32(xfer, &mut ai.attitude)?;
        xfer.xfer_unsigned_int(&mut ai.next_mood_check_time)?;
        if version == 1 {
            // "surrender + demoralize" (neither compiled in)
            let (mut demoralized, mut surrendered, mut index) = (0u32, 0u32, 0i32);
            xfer.xfer_unsigned_int(&mut demoralized)?;
            xfer.xfer_unsigned_int(&mut surrendered)?;
            xfer.xfer_int(&mut index)?;
        } else if version == 2 {
            // "demoralize only"
            let mut tmp0 = 0u32;
            xfer.xfer_unsigned_int(&mut tmp0)?;
        }
        xfer.xfer_object_id(&mut ai.crate_created)?;
        if version < 3 {
            let mut repulsor_countdown = 0i32;
            xfer.xfer_int(&mut repulsor_countdown)?;
        }
        Ok(())
    }

    /// `AIUpdateInterface::loadPostProcess()`: a locomotor set read by name chosen again; not moving: the goal and current cells
    /// forgotten and the pathfinder told the object's position; moving: the goal cell's centre (once) given to the pathfinder.
    pub fn load_post_process(&mut self, h: &mut H) {
        // (UpdateModule::loadPostProcess: the base's)
        h.update_module_load_post_process();
        let ai = self.ai_mut();
        if ai.fix_loco_in_post_process && ai.cur_locomotor_set != LOCOMOTORSET_INVALID {
            ai.fix_loco_in_post_process = false;
            let lst = ai.cur_locomotor_set;
            // "Set the current to invalid, because chooseLocomotorSet aborts if it is already set to the desired value."
            ai.cur_locomotor_set = LOCOMOTORSET_INVALID;
            let mut ai = self.ai.take().expect("the AI (not lent)");
            ai.choose_locomotor_set(h, lst);
            self.ai = Some(ai);
        }
        let moving = {
            let (ai, sm) = (self.ai.as_ref().expect("the AI"), self.machine.as_ref().expect("the AI's machine"));
            ai.is_moving(sm)
        };
        let layer = h.layer();
        let pos = h.position();
        let ai = self.ai_mut();
        if !moving {
            ai.pathfind_goal_cell = ICoord2D { x: -1, y: -1 };
            h_update_goal(h, &pos, layer);
            let ai = self.ai_mut();
            ai.pathfind_cur_cell = ICoord2D { x: -1, y: -1 };
            if !plant(3502) {
                h.pf_update_pos(&pos);
            }
        } else if ai.pathfind_goal_cell.x >= 0 && ai.pathfind_goal_cell.y >= 0 {
            let goal = Coord3D {
                x: ai.pathfind_goal_cell.x as f32 * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F * 0.5,
                y: ai.pathfind_goal_cell.y as f32 * PATHFIND_CELL_SIZE_F + PATHFIND_CELL_SIZE_F * 0.5,
                z: 0.0,
            };
            ai.pathfind_goal_cell = ICoord2D { x: -1, y: -1 };
            h_update_goal(h, &goal, layer);
        }
    }
}

/// `TheAI->pathfinder()->updateGoal(getObject(), pos, getObject()->getLayer())`.
fn h_update_goal<H: AiStateHost>(h: &mut H, pos: &Coord3D, layer: crate::common::game_type::PathfindLayer) {
    h.pf_update_goal(pos, layer);
}

#[allow(unused_imports)]
use Snapshot as _;
