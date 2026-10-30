// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/DeployStyleAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/DeployStyleAIUpdate.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `DeployStyleAIUpdate` (`p1w13-sonnet`, layer 10, batch A31): an AI that must unpack before it attacks and pack up before it
//! moves (the Nuke Cannon, the Tomahawk launcher's kind of unit).
//!
//! The module's state is [`DeployStyle`], held in [`AiSubclass::DeployStyle`]; [`AiUpdateModule::update`] takes
//! [`deploy_style_update`] for it, [`commands::ai_do_command`] its `aiDoCommand` (one more `isAllowedToRespondToAiCommands`
//! first), and [`AiUpdateModule::xfer`] its `xfer` (version 4: the base's, then the state and the frame). `isIdle` and
//! `loadPostProcess` are the base's.
//!
//! # Upstream facts kept
//!
//! - The frame arithmetic is `UnsignedInt`'s: a deploy reversed after its frame has passed wraps; the animation frame is that
//!   unsigned value given to `setAnimationFrame(Int)`.
//! - A reversed undeploy's new wait is computed from the unpack time, not the pack time (both reversals use `getUnpackTime()`).
//! - `ResetTurretBeforePacking` is parsed and never read.
//! - A deploy's manual animation frame counts up over the **pack** time (`update`), a reversed deploy's over the unpack time.
//! - Loading a version before 4 reads an `AICommandParmsStorage` (twice before version 2); that path is not translated (refused
//!   with `XferStatus::InvalidVersion`): this port writes version 4 only.
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update
//! [`AiUpdateModule::xfer`]: crate::logic::ai::ai_states::AiUpdateModule::xfer
//! [`commands::ai_do_command`]: crate::logic::ai::ai_commands::ai_do_command

use crate::common::game_common::TURRET_INVALID;
use crate::common::game_type::INVALID_ID;
use crate::common::model_state::{MODELCONDITION_DEPLOYED, MODELCONDITION_MOVING, MODELCONDITION_PACKING, MODELCONDITION_UNPACKING};
use crate::common::object_status_types::OBJECT_STATUS_DEPLOYED;
use crate::common::state_machine::{SmResult, StateContext, StateMachine};
use crate::common::xfer::{Xfer, XferResult};
use crate::logic::ai::ai_attack_states::{get_which_turret_for_cur_weapon, is_turret_in_natural_position, recenter_turret, set_turret_enabled};
use crate::logic::ai::ai_states::{ai, aism_set_temporary_state, with_ai, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::AI_BUSY;
use crate::logic::module::update_module::UPDATE_SLEEP_NONE;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `DeployStateTypes`: "Mobile, can't attack".
pub const READY_TO_MOVE: u32 = 0;
/// "Not mobile, can't attack, currently unpacking to attack".
pub const DEPLOY: u32 = 1;
/// "Not mobile, can attack".
pub const READY_TO_ATTACK: u32 = 2;
/// "Not mobile, can't attack, currently packing to move".
pub const UNDEPLOY: u32 = 3;
/// "While deployed, we must wait for the turret to go back to natural position prior to undeploying."
pub const ALIGNING_TURRETS: u32 = 4;

/// `DeployStyleAIUpdateModuleData`'s own fields (the times in frames, as `parseDurationUnsignedInt` leaves them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeployStyleData {
    /// `UnpackTime`.
    pub unpack_time: u32,
    /// `PackTime`.
    pub pack_time: u32,
    /// `ResetTurretBeforePacking` (never read upstream).
    pub reset_turret_before_packing: bool,
    /// `TurretsFunctionOnlyWhenDeployed`.
    pub turrets_function_only_when_deployed: bool,
    /// `TurretsMustCenterBeforePacking`.
    pub turrets_must_center_before_packing: bool,
    /// `ManualDeployAnimations`.
    pub manual_deploy_animations: bool,
}

/// `DeployStyleAIUpdate`'s members, with its module data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeployStyle {
    /// The module data's own fields.
    pub data: DeployStyleData,
    /// `m_state` (a `DeployStateTypes`; as loaded, any value).
    pub state: u32,
    /// `m_frameToWaitForDeploy`.
    pub frame_to_wait_for_deploy: u32,
}

impl DeployStyle {
    /// `DeployStyleAIUpdate(thing, moduleData)`: ready to move, no wait.
    pub fn new(data: DeployStyleData) -> Self {
        DeployStyle { data, state: READY_TO_MOVE, frame_to_wait_for_deploy: 0 }
    }
}

/// The running AI's `DeployStyleAIUpdate` members (the AI is one).
fn ds<H: AiStateHost>(h: &mut H) -> &mut DeployStyle {
    match &mut ai(h).subclass {
        AiSubclass::DeployStyle(d) => d,
        _ => unreachable!("a DeployStyleAIUpdate"),
    }
}

/// `DeployStyleAIUpdate::update()`'s own part (the AI lent): deploy when in range of the victim or guarding idle, undeploy (the
/// turrets centred first when the data says so) when trying to move, hold the AI busy while packing, unpacking or aligning. The
/// caller then runs `AIUpdateInterface::update()`; "we can't sleep the deploy AI".
pub fn deploy_style_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    // "have to call our parent's isIdle, because we override it to never return true when we have a pending command..."
    let weapon = h.current_weapon_slot().is_some();
    let now = StateContext::frame(h);

    // "Are we attempting to move? If so we can't do it unless we are undeployed."
    let is_trying_to_move = ai(h).is_waiting_for_path() || ai(h).path.is_some();

    // "Are we trying to attack something. If so, we need to be in range before we can do so."
    let is_trying_to_attack = sm.is_in_attack_state();
    let mut is_in_range = false;

    // "Are we in guard mode? If so, are we idle... idle guarders deploy for fastest response against attackers."
    let is_in_guard_idle_state = sm.is_in_guard_idle_state();

    if is_trying_to_attack && weapon {
        let victim = with_ai(h, |a, h| a.get_current_victim(&*h));
        if victim != INVALID_ID {
            is_in_range = h.weapon_is_within_attack_range_of_object(victim);
        } else if let Some(pos) = with_ai(h, |a, h| a.get_current_victim_pos(h, &*sm)) {
            is_in_range = h.weapon_is_within_attack_range_of_position(&pos);
        }
    }

    let (state, wait) = (ds(h).state, ds(h).frame_to_wait_for_deploy);
    if wait != 0 && now >= wait && !plant(3620) {
        match state {
            DEPLOY => set_my_state(h, READY_TO_ATTACK, false),
            UNDEPLOY => set_my_state(h, READY_TO_MOVE, false),
            _ => {}
        }
    }

    if is_in_range || (is_in_guard_idle_state && !plant(3621)) {
        match ds(h).state {
            // "We're need to deploy before we can attack."
            READY_TO_MOVE => set_my_state(h, DEPLOY, false),
            // "Let the AI handle attacking." / "We can't start attacking until we are fully deployed."
            READY_TO_ATTACK | DEPLOY => {}
            UNDEPLOY => {
                if ds(h).frame_to_wait_for_deploy != 0 {
                    // "Reverse the undeploy at it's current frame!"
                    set_my_state(h, DEPLOY, true);
                }
            }
            // "If turrets are aligning, we are able to attack right now."
            ALIGNING_TURRETS => set_my_state(h, READY_TO_ATTACK, false),
            _ => {}
        }
    } else if is_trying_to_move {
        match ds(h).state {
            // "We're ready... ai will handle moving."
            READY_TO_MOVE => {}
            READY_TO_ATTACK => {
                let tur = get_which_turret_for_cur_weapon(h);
                if tur != TURRET_INVALID && ds(h).data.turrets_must_center_before_packing && !plant(3622) {
                    // "Turrets need to center before we can undeploy."
                    set_my_state(h, ALIGNING_TURRETS, false);
                } else {
                    // "Start undeploying."
                    set_my_state(h, UNDEPLOY, false);
                }
            }
            DEPLOY => {
                if ds(h).frame_to_wait_for_deploy != 0 {
                    // "Reverse the deploy at it's current frame!"
                    set_my_state(h, UNDEPLOY, true);
                }
            }
            UNDEPLOY => {}
            ALIGNING_TURRETS => {
                let tur = get_which_turret_for_cur_weapon(h);
                // "Turrets are centers, so now we can undeploy."
                if tur != TURRET_INVALID && is_turret_in_natural_position(h, tur) {
                    set_my_state(h, UNDEPLOY, false);
                }
            }
            _ => {}
        }
    }

    let d = *ds(h);
    match d.state {
        READY_TO_MOVE => {
            if is_trying_to_move {
                h.set_model_condition_state(MODELCONDITION_MOVING);
            }
        }
        READY_TO_ATTACK => {}
        DEPLOY | UNDEPLOY | ALIGNING_TURRETS => {
            if d.state != ALIGNING_TURRETS && d.data.manual_deploy_animations {
                let frames_left = d.frame_to_wait_for_deploy.wrapping_sub(now);
                // (a deploy counts up from 0 over the pack time, an undeploy down)
                let frame = if d.state == DEPLOY && !plant(3623) { d.data.pack_time.wrapping_sub(frames_left) } else { frames_left };
                h.set_drawable_animation_frame(frame as i32);
            }
            aism_set_temporary_state(sm, h, AI_BUSY, UPDATE_SLEEP_NONE.0 as i32)?;
            crate::logic::ai::ai_states::with_ai(h, |a, h| a.set_locomotor_goal_none(h));
        }
        _ => {}
    }
    Ok(())
}

/// `setMyState(stateID, reverseDeploy)`: the state, its model conditions, its wait (a reversal continues from the frame reached),
/// its sound, its turrets.
pub fn set_my_state<H: AiStateHost>(h: &mut H, state_id: u32, reverse_deploy: bool) {
    ds(h).state = state_id;
    let now = StateContext::frame(h);
    let data = ds(h).data;

    match state_id {
        DEPLOY => {
            // "Tell our object to deploy (so it can continue the same attack later)."
            h.clear_and_set_model_condition_flags(&[MODELCONDITION_PACKING], MODELCONDITION_UNPACKING);
            if reverse_deploy {
                // "This is a zero to max animation."
                let total_frames = data.unpack_time;
                let frames_left = ds(h).frame_to_wait_for_deploy.wrapping_sub(now);
                ds(h).frame_to_wait_for_deploy = now.wrapping_add(total_frames).wrapping_sub(frames_left);
                if data.manual_deploy_animations {
                    h.set_drawable_animation_frame(total_frames.wrapping_sub(frames_left) as i32);
                }
            } else {
                ds(h).frame_to_wait_for_deploy = data.unpack_time.wrapping_add(now);
            }
            // "Play deploy sound"
            h.play_per_unit_sound(b"Deploy");
        }
        UNDEPLOY => {
            // "This status will tell the approach AI state to succeed automatically. This prevents twitching on deploy. Make sure
            // we clear it now so when it goes into that state, it'll actually move."
            h.set_status_bits(&[OBJECT_STATUS_DEPLOYED], false);
            // "Tell our object to pack up (so it can continue the same move later)."
            h.clear_and_set_model_condition_flags(&[MODELCONDITION_UNPACKING, MODELCONDITION_DEPLOYED], MODELCONDITION_PACKING);
            if reverse_deploy {
                // "This is a max to zero animation." (the unpack time, as upstream)
                let total_frames = if plant(3624) { data.pack_time } else { data.unpack_time };
                let frames_left = ds(h).frame_to_wait_for_deploy.wrapping_sub(now);
                ds(h).frame_to_wait_for_deploy = now.wrapping_add(total_frames).wrapping_sub(frames_left);
                if data.manual_deploy_animations {
                    h.set_drawable_animation_frame(frames_left as i32);
                }
            } else {
                ds(h).frame_to_wait_for_deploy = data.pack_time.wrapping_add(now);
            }
            if data.turrets_function_only_when_deployed {
                let tur = get_which_turret_for_cur_weapon(h);
                if tur != TURRET_INVALID {
                    set_turret_enabled(h, tur, false);
                }
            }
            // "Play undeploy sound"
            h.play_per_unit_sound(b"Undeploy");
        }
        READY_TO_MOVE => {
            ds(h).frame_to_wait_for_deploy = 0;
            h.clear_model_condition_flags(&[MODELCONDITION_PACKING]);
        }
        READY_TO_ATTACK => {
            // "This status will tell the approach AI state to succeed automatically. This prevents twitching on deploy."
            if !plant(3625) {
                h.set_status_bits(&[OBJECT_STATUS_DEPLOYED], true);
            }
            ds(h).frame_to_wait_for_deploy = 0;
            h.clear_and_set_model_condition_flags(&[MODELCONDITION_UNPACKING], MODELCONDITION_DEPLOYED);
            if data.turrets_function_only_when_deployed {
                let tur = get_which_turret_for_cur_weapon(h);
                if tur != TURRET_INVALID {
                    set_turret_enabled(h, tur, true);
                }
            }
        }
        ALIGNING_TURRETS => {
            ds(h).frame_to_wait_for_deploy = 0;
            let tur = get_which_turret_for_cur_weapon(h);
            if tur != TURRET_INVALID {
                recenter_turret(h, tur);
            }
        }
        _ => {}
    }
}

/// `DeployStyleAIUpdate::xfer`'s own members, after the base's: the state (`xferUser`, 4 bytes) and the frame.
pub fn deploy_style_xfer_members(d: &mut DeployStyle, xfer: &mut dyn Xfer) -> XferResult<()> {
    let mut b = d.state.to_le_bytes();
    xfer.xfer_user(&mut b)?;
    d.state = u32::from_le_bytes(b);
    if !plant(3626) {
        xfer.xfer_unsigned_int(&mut d.frame_to_wait_for_deploy)?;
    }
    Ok(())
}
