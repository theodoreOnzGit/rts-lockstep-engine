// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Update/AIUpdate/HackInternetAIUpdate.cpp
//   GeneralsMD/Code/GameEngine/Include/GameLogic/Module/HackInternetAIUpdate.h
//   GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIStates.cpp (AICommandParmsStorage)
//   GeneralsMD/Code/GameEngine/Include/GameLogic/AI.h (AICommandParmsStorage)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.
//! `HackInternetAIUpdate` (`p1w13-sonnet`, layer 10, batch A34): the hacker, who unpacks, hacks cash from the internet every few
//! frames (more often inside a building), and packs up before obeying a new command, which waits meanwhile.
//!
//! The module's state is [`HackInternet`], held in [`AiSubclass::HackInternet`]; its machine (`HackInternetStateMachine`) is the
//! AI's with three more states ([`define_hack_states`]: unpacking, hacking, packing, ids 1000 to 1002, [`AiState::HackUnpacking`],
//! [`AiState::HackInternet`], [`AiState::HackPacking`]). [`AiUpdateModule::update`] takes [`hack_internet_pre_update`] before the
//! base's update, [`commands::ai_do_command`] its `aiDoCommand` ([`hack_internet_do_command`]: a command while hacking or packing
//! waits in [`AiCommandParmsStorage`]), `AICMD_HACK_INTERNET` its `hackInternet`, `AiUpdate::is_idle` its `isIdle` (never idle
//! with a command waiting), and [`AiUpdateModule::xfer`] its `xfer` (version 1, the waiting command's storage).
//!
//! # Upstream facts kept
//!
//! - `AICommandParmsStorage::doXfer` writes the command twice and never the command's source (`xferUser(&m_cmd,
//!   sizeof(m_cmdSource))`): a loaded storage keeps its source and takes the command from the second word.
//! - Unpacking's and packing's frames are the data's time times a random variation in `[1 - f, 1 + f]`, a `float` product
//!   truncated to frames; packing inside a building takes none; the hacking delay is the fast one inside a building.
//! - A veterancy level's cash amount of 0 falls through to the level below, then to 1.
//! - The unpacking state sets its model condition again every update (a special ability's cleanup may clear it).
//! - The hacking state's `getControllingPlayer()` and the states' `getDrawable()` are dereferenced unchecked.
//!
//! # Not translated
//!
//! - A waiting command's damage and path: this port's [`AiCommandParms`] carries neither (the host's, for `privateGoProne`; no
//!   command reads the path); the storage keeps [`DamageInfo::default`] (upstream's constructor) and no path, and loads a saved path
//!   into nothing.
//! - The floating "+cash" text (the client's: its string, colour and client-random offset): a host call
//!   ([`AiStateHost::hack_cash_floating_text`]) when upstream would show it.
//!
//! [`AiUpdateModule::update`]: crate::logic::ai::ai_states::AiUpdateModule::update
//! [`AiUpdateModule::xfer`]: crate::logic::ai::ai_states::AiUpdateModule::xfer
//! [`commands::ai_do_command`]: crate::logic::ai::ai_commands::ai_do_command

use crate::common::base_type::Coord3D;
use crate::common::disabled_types::DISABLED_HACKED;
use crate::common::game_common::VeterancyLevel;
use crate::common::game_type::{ObjectId, INVALID_ID};
use crate::common::model_state::{MODELCONDITION_FIRING_A, MODELCONDITION_PACKING, MODELCONDITION_UNPACKING};
use crate::common::object_status_types::{OBJECT_STATUS_DETECTED, OBJECT_STATUS_STEALTHED};
use crate::common::state_machine::{SmResult, StateId, StateMachine, StateReturn};
use crate::common::xfer::{Xfer, XferMode, XferResult, XferVersion};
use crate::logic::ai::ai_commands::{self as commands, AiCommandParms, AiCommandType, AICMD_MOVE_TO_POSITION};
use crate::logic::ai::ai_states::{ai, aism_clear, aism_set_state, AiState, AiStateHost};
use crate::logic::ai::ai_update::AiSubclass;
use crate::logic::ai::ai_update_types::{CommandSourceType, AI_IDLE, CMD_FROM_AI};
use crate::logic::ai::ai_waypoint_states::INVALID_WAYPOINT_ID;
use crate::logic::ai::path::Path;
use crate::logic::damage::DamageInfo;
use crate::logic::modules::plant;

type Sm<H> = StateMachine<AiState<H>>;

/// `UNPACKING`: "Kneel down and set up internet satellite link".
pub const UNPACKING: StateId = 1000;
/// `HACK_INTERNET`: "Once set up, start hacking for cash."
pub const HACK_INTERNET: StateId = 1001;
/// `PACKING`: "Pack up before moving."
pub const PACKING: StateId = 1002;

/// `HackInternetAIUpdateModuleData`'s own fields (the times in frames).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HackInternetData {
    /// `UnpackTime`.
    pub unpack_time: u32,
    /// `PackTime`.
    pub pack_time: u32,
    /// `CashUpdateDelay`.
    pub cash_update_delay: u32,
    /// `CashUpdateDelayFast` (inside a building).
    pub cash_update_delay_fast: u32,
    /// `RegularCashAmount`.
    pub regular_cash_amount: u32,
    /// `VeteranCashAmount`.
    pub veteran_cash_amount: u32,
    /// `EliteCashAmount`.
    pub elite_cash_amount: u32,
    /// `HeroicCashAmount`.
    pub heroic_cash_amount: u32,
    /// `XpPerCashUpdate`.
    pub xp_per_cash_update: u32,
    /// `PackUnpackVariationFactor`.
    pub pack_unpack_variation_factor: f32,
}

/// `AICommandParmsStorage` (AI.h): a command kept by ids and names, to be given later.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiCommandParmsStorage {
    /// `m_cmd`.
    pub cmd: AiCommandType,
    /// `m_cmdSource`.
    pub cmd_source: CommandSourceType,
    /// `m_pos`.
    pub pos: Coord3D,
    /// `m_obj`.
    pub obj: ObjectId,
    /// `m_otherObj`.
    pub other_obj: ObjectId,
    /// `m_teamName`.
    pub team_name: Vec<u8>,
    /// `m_coords`.
    pub coords: Vec<Coord3D>,
    /// `m_waypoint`.
    pub waypoint: Option<u32>,
    /// `m_polygon`.
    pub polygon: Option<i32>,
    /// `m_intValue`: "misc usage".
    pub int_value: i32,
    /// `m_damage` (see the module's notes).
    pub damage: DamageInfo,
    /// `m_commandButton`.
    pub command_button: Option<u32>,
    /// `m_path` (see the module's notes).
    pub path: Option<Path>,
}

impl AiCommandParmsStorage {
    /// `store(parms)`: the objects by id (`INVALID_ID` for none), the team by name.
    pub fn store<H: AiStateHost>(&mut self, h: &mut H, parms: &AiCommandParms) {
        self.cmd = parms.cmd;
        self.cmd_source = parms.cmd_source;
        self.pos = parms.pos;
        self.obj = if h.object_exists(parms.obj) { parms.obj } else { INVALID_ID };
        self.other_obj = if h.object_exists(parms.other_obj) { parms.other_obj } else { INVALID_ID };
        self.team_name = parms.team.map(|t| h.team_name(t)).unwrap_or_default();
        self.coords = parms.coords.clone();
        self.waypoint = parms.waypoint;
        self.polygon = parms.polygon;
        self.int_value = parms.int_value;
        self.damage = DamageInfo::default();
        self.command_button = parms.command_button;
        self.path = None;
    }

    /// `reconstitute(parms)`: the objects found by id, the team by name.
    pub fn reconstitute<H: AiStateHost>(&self, h: &mut H, parms: &mut AiCommandParms) {
        parms.cmd = self.cmd;
        parms.cmd_source = self.cmd_source;
        parms.pos = self.pos;
        parms.obj = if h.object_exists(self.obj) { self.obj } else { INVALID_ID };
        parms.other_obj = if h.object_exists(self.other_obj) { self.other_obj } else { INVALID_ID };
        parms.team = h.team_by_name(&self.team_name);
        parms.coords = self.coords.clone();
        parms.waypoint = self.waypoint;
        parms.polygon = self.polygon;
        parms.int_value = self.int_value;
        parms.command_button = self.command_button;
    }

    /// `doXfer(xfer)`.
    pub fn do_xfer<H: AiStateHost>(&mut self, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
        let load = xfer.get_xfer_mode() == XferMode::Load;
        let mut b = self.cmd.to_le_bytes();
        xfer.xfer_user(&mut b)?;
        self.cmd = AiCommandType::from_le_bytes(b);
        // `xferUser(&m_cmd, sizeof(m_cmdSource))`: the command again
        let mut b = self.cmd.to_le_bytes();
        if plant(3650) {
            b = self.cmd_source.to_le_bytes();
        }
        xfer.xfer_user(&mut b)?;
        self.cmd = AiCommandType::from_le_bytes(b);
        xfer.xfer_coord3d(&mut self.pos)?;
        xfer.xfer_object_id(&mut self.obj)?;
        xfer.xfer_object_id(&mut self.other_obj)?;
        xfer.xfer_ascii_string(&mut self.team_name)?;
        let mut num_coords = self.coords.len() as i32;
        xfer.xfer_int(&mut num_coords)?;
        if load {
            for _ in 0..num_coords.max(0) {
                let mut pos = Coord3D::default();
                xfer.xfer_coord3d(&mut pos)?;
                self.coords.push(pos);
            }
        } else {
            for i in 0..num_coords as usize {
                let mut pos = self.coords[i];
                xfer.xfer_coord3d(&mut pos)?;
            }
        }
        let mut id = self.waypoint.map_or(INVALID_WAYPOINT_ID, |w| h.waypoint_id(w));
        xfer.xfer_unsigned_int(&mut id)?;
        if load && id != INVALID_WAYPOINT_ID {
            self.waypoint = h.waypoint_by_id(id);
        }
        let mut trigger_name = self.polygon.map(|p| h.polygon_trigger_name(p)).unwrap_or_default();
        xfer.xfer_ascii_string(&mut trigger_name)?;
        if load && !trigger_name.is_empty() {
            self.polygon = h.polygon_trigger_by_name(&trigger_name);
        }
        xfer.xfer_int(&mut self.int_value)?;
        xfer.xfer_snapshot(&mut self.damage)?;
        let mut cmd_name = self.command_button.map(|c| h.command_button_name(c)).unwrap_or_default();
        xfer.xfer_ascii_string(&mut cmd_name)?;
        if !cmd_name.is_empty() && self.command_button.is_none() {
            self.command_button = h.command_button_by_name(&cmd_name);
        }
        let mut has_path = self.path.is_some();
        xfer.xfer_bool(&mut has_path)?;
        if has_path && self.path.is_none() {
            self.path = Some(Path::new());
        }
        if let Some(p) = self.path.as_mut().filter(|_| has_path) {
            xfer.xfer_snapshot(p)?;
        }
        Ok(())
    }
}

/// `HackInternetAIUpdate`'s members, with its module data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HackInternet {
    /// The module data's own fields.
    pub data: HackInternetData,
    /// `m_pendingCommand`.
    pub pending_command: AiCommandParmsStorage,
    /// `m_hasPendingCommand`.
    pub has_pending_command: bool,
}

impl HackInternet {
    /// `HackInternetAIUpdate(thing, moduleData)`: no command waiting.
    pub fn new(data: HackInternetData) -> Self {
        HackInternet { data, ..Default::default() }
    }
}

/// The running AI's `HackInternetAIUpdate` members (the AI is one).
fn hk<H: AiStateHost>(h: &mut H) -> &mut HackInternet {
    match &mut ai(h).subclass {
        AiSubclass::HackInternet(k) => k,
        _ => unreachable!("a HackInternetAIUpdate"),
    }
}

/// `HackInternetStateMachine`'s own states, after the AI's: "order matters: first state is the default state" (the AI's idle state
/// stays the default).
pub fn define_hack_states<H: AiStateHost>(sm: &mut Sm<H>) -> SmResult<()> {
    sm.define_state(UNPACKING, AiState::HackUnpacking { frames_remaining: 0 }, HACK_INTERNET, HACK_INTERNET, &[])?;
    sm.define_state(HACK_INTERNET, AiState::HackInternet { frames_remaining: 0 }, PACKING, PACKING, &[])?;
    sm.define_state(PACKING, AiState::HackPacking { frames_remaining: 0 }, AI_IDLE, AI_IDLE, &[])?;
    Ok(())
}

/// `getPackTime()`: "We don't need to pack if exiting a building".
fn pack_time<H: AiStateHost>(h: &mut H) -> u32 {
    if h.contained_by() != INVALID_ID {
        return 0;
    }
    hk(h).data.pack_time
}

/// `getCashUpdateDelay()`: the fast one inside a building.
fn cash_update_delay<H: AiStateHost>(h: &mut H) -> u32 {
    let d = hk(h).data;
    if h.contained_by() != INVALID_ID && !plant(3651) {
        d.cash_update_delay_fast
    } else {
        d.cash_update_delay
    }
}

/// The unpacking's and packing's frames: the time times a random variation (the product in `float`, truncated).
fn varied_frames<H: AiStateHost>(h: &mut H, time: u32) -> u32 {
    let variation_factor = hk(h).data.pack_unpack_variation_factor;
    let variation = h.logic_random_value_real(1.0 - variation_factor, 1.0 + variation_factor);
    (time as f32 * variation) as u32
}

fn frames<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> &mut u32 {
    match sm.state_mut(id) {
        Some(AiState::HackUnpacking { frames_remaining })
        | Some(AiState::HackInternet { frames_remaining })
        | Some(AiState::HackPacking { frames_remaining }) => frames_remaining,
        _ => unreachable!("a hacking state"),
    }
}

/// `UnpackingState::onEnter()`: the unpacking condition and sound, the varied unpack time (the drawable's loop that long).
pub fn unpacking_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    h.clear_model_condition_flags(&[MODELCONDITION_PACKING, MODELCONDITION_FIRING_A, MODELCONDITION_UNPACKING]);
    h.set_model_condition_state(MODELCONDITION_UNPACKING);
    h.play_per_unit_sound(b"UnitUnpack");
    let unpack_time = hk(h).data.unpack_time;
    let f = varied_frames(h, unpack_time);
    *frames(sm, id) = f;
    h.set_drawable_animation_loop_duration(f);
    Ok(StateReturn::CONTINUE)
}

/// `UnpackingState::update()`: the condition set again ("specialability::cleanup() after my onEnter()"), the frames counted down.
pub fn unpacking_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    h.set_model_condition_state(MODELCONDITION_UNPACKING);
    count_down(sm, id)
}

fn count_down<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> SmResult<StateReturn> {
    let f = frames(sm, id);
    if *f > 0 {
        *f -= 1;
        Ok(StateReturn::CONTINUE)
    } else {
        Ok(StateReturn::SUCCESS)
    }
}

/// `PackingState::onEnter()`: the packing condition and sound, the varied pack time (none inside a building).
pub fn packing_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    h.clear_and_set_model_condition_flags(&[MODELCONDITION_FIRING_A], MODELCONDITION_PACKING);
    h.play_per_unit_sound(b"UnitPack");
    let pack_time = pack_time(h);
    let f = varied_frames(h, pack_time);
    *frames(sm, id) = f;
    h.set_drawable_animation_loop_duration(f);
    Ok(StateReturn::CONTINUE)
}

/// `PackingState::update()`.
pub fn packing_update<H: AiStateHost>(sm: &mut Sm<H>, id: StateId) -> SmResult<StateReturn> {
    count_down(sm, id)
}

/// `HackInternetState::onEnter()`: "Go into the hack internet stance."
pub fn hack_on_enter<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    h.clear_and_set_model_condition_flags(&[MODELCONDITION_UNPACKING], MODELCONDITION_FIRING_A);
    *frames(sm, id) = cash_update_delay(h);
    Ok(StateReturn::CONTINUE)
}

/// `HackInternetState::update()`: unless hacked, the delay counted down, then the cash (by veterancy), the score, the experience,
/// the floating text where it would be seen, the cash ping, and the delay again. "This is a persistent state until told otherwise."
pub fn hack_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, id: StateId) -> SmResult<StateReturn> {
    if h.is_disabled_by_type(DISABLED_HACKED) {
        // "Don't hack while hacked, hehe."
        return Ok(StateReturn::CONTINUE);
    }
    let f = frames(sm, id);
    if *f > 0 {
        *f -= 1;
        return Ok(StateReturn::CONTINUE);
    }
    // "We have waited the full amount of the delay, so hack some cash from the heavens!"
    if h.player_has_money() {
        if let Some(level) = h.veterancy_level() {
            let d = hk(h).data;
            // "If entry missing, fall through!"
            let order = [d.heroic_cash_amount, d.elite_cash_amount, d.veteran_cash_amount, d.regular_cash_amount];
            let start = match level {
                VeterancyLevel::Heroic => 0,
                VeterancyLevel::Elite => 1,
                VeterancyLevel::Veteran => 2,
                VeterancyLevel::Regular => 3,
            };
            let start = if plant(3652) { 3 } else { start };
            let amount = order[start..].iter().copied().find(|&a| a != 0).unwrap_or(1);
            h.money_deposit(amount);
            h.score_add_money_earned(amount);
            // "Grant the unit some experience for a successful hack."
            h.add_experience_points(d.xp_per_cash_update);

            let mut display_money = true;
            if h.status_test(OBJECT_STATUS_STEALTHED) && !h.is_locally_controlled(h.object_id()) && !h.status_test(OBJECT_STATUS_DETECTED)
            {
                display_money = false;
            }
            let container = h.contained_by();
            if container != INVALID_ID
                && h.object_test_status(container, OBJECT_STATUS_STEALTHED)
                && !h.is_locally_controlled(container)
                && !h.object_test_status(container, OBJECT_STATUS_DETECTED)
            {
                display_money = false;
            }
            if display_money && !plant(3653) {
                h.hack_cash_floating_text(amount);
            }
            h.play_per_unit_sound(b"UnitCashPing");
        }
    }
    // "Reset timer and start a new cycle."
    *frames(sm, id) = cash_update_delay(h);
    Ok(StateReturn::CONTINUE)
}

/// The three states' `onExit`: their condition cleared.
pub fn hack_states_on_exit<H: AiStateHost>(h: &mut H, id: StateId) {
    let condition = match id {
        UNPACKING => MODELCONDITION_UNPACKING,
        HACK_INTERNET => MODELCONDITION_FIRING_A,
        _ => MODELCONDITION_PACKING,
    };
    h.clear_model_condition_state(condition);
}

/// The three states' `xfer` (version 1): the frames.
pub fn hack_state_xfer(frames_remaining: &mut u32, xfer: &mut dyn Xfer) -> XferResult<()> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;
    xfer.xfer_unsigned_int(frames_remaining)
}

/// `hackInternet()` (the base's `privateHackInternet` finds this module): the unpacking state.
pub fn hack_internet<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    aism_set_state(sm, h, UNPACKING).map(|_| ())
}

/// `HackInternetAIUpdate::aiDoCommand(parms)`'s own part (after the base's `isAllowedToRespondToAiCommands`): a command while
/// hacking or packing waits ("nuke any existing pending cmd"), hacking turns to packing; `true` when the command is so handled.
/// Otherwise no command waits any more and the base's runs.
pub fn hack_internet_do_command<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H, parms: &AiCommandParms) -> SmResult<bool> {
    let state = sm.get_current_state_id();
    if state == HACK_INTERNET || state == PACKING {
        let mut storage = std::mem::take(&mut hk(h).pending_command);
        storage.store(h, parms);
        let k = hk(h);
        k.pending_command = storage;
        k.has_pending_command = true;
        if state == HACK_INTERNET && !plant(3654) {
            aism_clear(sm, h)?;
            ai(h).set_last_command_source(CMD_FROM_AI);
            aism_set_state(sm, h, PACKING)?;
        }
        return Ok(true);
    }
    hk(h).has_pending_command = false;
    Ok(false)
}

/// `HackInternetAIUpdate::update()`'s part before the base's (the AI lent): "have to call our parent's isIdle, because we override
/// it to never return true when we have a pending command..." — idle, the waiting command is given.
pub fn hack_internet_pre_update<H: AiStateHost>(sm: &mut Sm<H>, h: &mut H) -> SmResult<()> {
    if ai(h).base_is_idle(&*sm) && hk(h).has_pending_command {
        // "values don't matter, will be wiped by next line"
        let mut parms = AiCommandParms::new(AICMD_MOVE_TO_POSITION, CMD_FROM_AI);
        let storage = hk(h).pending_command.clone();
        storage.reconstitute(h, &mut parms);
        hk(h).has_pending_command = false;
        commands::ai_do_command(sm, h, &parms)?;
    }
    Ok(())
}

/// `HackInternetAIUpdate::xfer`'s own members, after the base's.
pub fn hack_internet_xfer_members<H: AiStateHost>(k: &mut HackInternet, h: &mut H, xfer: &mut dyn Xfer) -> XferResult<()> {
    xfer.xfer_bool(&mut k.has_pending_command)?;
    if k.has_pending_command && !plant(3655) {
        k.pending_command.do_xfer(h, xfer)?;
    }
    Ok(())
}
