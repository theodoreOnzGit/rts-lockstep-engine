// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/GameClient/Eva.h (EvaMessage, EvaSideSounds, EvaCheckInfo, EvaCheck, Eva)
//   GeneralsMD/Code/GameEngine/Source/GameClient/Eva.cpp (TheEvaMessageNames, Eva::s_shouldPlayFuncs,
//     INI::parseEvaEvent, parseSideSoundsList, EvaSideSounds::s_evaSideSounds, EvaCheckInfo::EvaCheckInfo,
//     EvaCheckInfo::s_evaEventInfo, EvaCheck::EvaCheck, Eva::Eva, Eva::init, Eva::reset, Eva::update,
//     Eva::nameToMessage, Eva::messageToName, Eva::newEvaCheckInfo, Eva::getEvaCheckInfo, Eva::setShouldPlay,
//     Eva::setEvaEnabled, Eva::isTimeForCheck, Eva::messageShouldPlay, Eva::shouldPlayLowPower,
//     Eva::shouldPlayGenericHandler, Eva::playMessage, Eva::processPlayingMessages)
//   GeneralsMD/Code/GameEngine/Source/GameClient/InGameUI.cpp (InGameUI::addSuperweapon, findSWInfo, and the EVA half of
//     InGameUI::postDraw's superweapon timers: "Superweapon is ready for launch")
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! EVA, the voice that tells the local player what happened (GitHub #55): `TheEva`.
//!
//! # As upstream does it
//!
//! `Eva::init` reads `Data\INI\Eva.ini`: one `EvaEvent <MESSAGE>` block per message, with its `Priority`, the
//! `TimeBetweenChecksMS` before it may play again, the `ExpirationTimeMS` after which a request that never got to play is
//! dropped, and one `SideSounds` block per side (`Side`, `Sounds`). The game asks for a message with
//! `TheEva->setShouldPlay(message)`; each `Eva::update` checks every message that is not waiting (`isTimeForCheck`),
//! asks its handler (`messageShouldPlay`: low power reads the local player's energy, every other message its flag), queues
//! it (`playMessage`), then plays the waiting message of the highest priority with the local player's side's sound, one at
//! a time (`processPlayingMessages`), and clears every flag.
//!
//! # What asks for a message here
//!
//! The logic's `setShouldPlay` calls are client events ([`Eva::take_event`]): `Object::onDie`'s building and unit lost, the
//! sabotage, theft and hijack crates, the upgrade complete, the superweapons launched and detected, the general's level
//! up, the stealth detectors' messages, the beacon placed (the dispatcher's), and the script action that enables it. They
//! change nothing the logic reads. The client's own are [`Eva::set_should_play`]: insufficient funds (the control bar and
//! the placement, counted by `WorldClient::eva_insufficient_funds`) and superweapon ready ([`SuperweaponTimers`]).
//!
//! # Where this differs from upstream
//!
//! - **The update runs once per logic frame** (the client calls it after the frame's events), where upstream runs it in
//!   every client frame (`GameClient::update`); the frames it compares are the logic's in both.
//! - **The local player is a parameter** (the [`AudioHost`]'s), where upstream keeps `m_localPlayer` for the update.
//! - **`m_previousBuildingCount` and `m_previousUnitCount`** are kept and reset, as upstream, and never read (upstream
//!   reads them nowhere either).
//! - ~~**Base and ally under attack** (`Radar::tryUnderAttackEvent`) are not asked for: the radar's events are not
//!   translated (GitHub #35). The stealth detectors' messages are asked for whether or not the radar would have taken the
//!   event (`tryEvent`), as their sounds are (`client_audio`).~~ **Corrected 2026-11-06 (#35):** the radar is translated
//!   (`client::radar`): base and ally under attack are asked for by `ClientAudio::apply_radar_feedback` (Radar.cpp's
//!   `tryUnderAttackEvent`, throttled by its `tryEvent`), and the client leaves out the stealth detectors' feedback events
//!   whose `tryEvent` refused before they reach EVA.
//! - **The superweapon timers** keep only what decides the EVA: the timers are not drawn, a removed timer is not removed
//!   (the logic does not emit `removeSuperweapon`; a timer whose object is gone is skipped, as upstream skips it), and the
//!   scripts' hiding (`hideObjectSuperweaponDisplayByScript`) and `objectChangedTeam` are not followed.

use crate::client::audio_event::{AudioEventRts, AudioHandle};
use crate::client::game_audio::{AudioHost, AudioManager};
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::ini::{BlockParse, FieldParse, Ini, IniLoadType, IniResult, UserData};
use crate::common::special_power_type::*;
use crate::logic::client_event::ClientEvent;
use crate::logic::world::game_logic_dispatch::ClientCall;
use crate::logic::world::object_events::{EvaMessage as ObjectEva, EvaSuperweapon, EvaWhose, ObjectClientEvent};
use crate::logic::world::player_ops::{EvaMessage as PlayerEva, PlayerClientEvent};

pub use crate::logic::modules::stealth_update::{EVA_INVALID, EVA_MESSAGE_NAMES};

/// `EvaMessage`: an index into [`EVA_MESSAGE_NAMES`] (`EVA_FIRST` is 0), or [`EVA_INVALID`].
pub type EvaMessage = i32;
/// `EVA_COUNT`.
pub const EVA_COUNT: usize = EVA_MESSAGE_NAMES.len();
/// `EVA_LowPower`.
pub const EVA_LOW_POWER: EvaMessage = 0;
/// `EVA_InsufficientFunds`.
pub const EVA_INSUFFICIENT_FUNDS: EvaMessage = 1;
/// `Eva::init`'s file.
pub const EVA_INI_FILE: &[u8] = b"Data\\INI\\Eva.ini";

/// `Eva::nameToMessage(name)`: the message of that name, ignoring case; [`EVA_INVALID`] for none (upstream's
/// `DEBUG_CRASH`, which a release build passes).
pub fn name_to_message(name: &[u8]) -> EvaMessage {
    EVA_MESSAGE_NAMES.iter().position(|n| n.as_bytes().eq_ignore_ascii_case(name)).map_or(EVA_INVALID, |i| i as EvaMessage)
}

/// `Eva::messageToName(message)`: empty outside `EVA_FIRST..EVA_COUNT`.
pub fn message_to_name(message: EvaMessage) -> &'static str {
    usize::try_from(message).ok().and_then(|i| EVA_MESSAGE_NAMES.get(i)).copied().unwrap_or("")
}

/// `struct EvaSideSounds`: a side's sounds for a message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaSideSounds {
    /// `m_side`.
    pub side: Vec<u8>,
    /// `m_soundNames`.
    pub sound_names: Vec<Vec<u8>>,
}

/// `EvaSideSounds::s_evaSideSounds`.
pub static EVA_SIDE_SOUNDS_FIELDS: [FieldParse<EvaSideSounds>; 3] = [
    FieldParse::new("Side", |i, s, _| i.parse_ascii_string().map(|v| s.side = v), UserData::None),
    FieldParse::new("Sounds", |i, s, _| i.parse_sounds_list(&mut s.sound_names), UserData::None),
    FieldParse::end(),
];

/// `class EvaCheckInfo`: a message's description from `Eva.ini`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaCheckInfo {
    /// `m_message`.
    pub message: EvaMessage,
    /// `m_framesBetweenChecks`.
    pub frames_between_checks: u32,
    /// `m_framesToExpire`.
    pub frames_to_expire: u32,
    /// `m_priority`: higher plays first.
    pub priority: u32,
    /// `m_evaSideSounds`.
    pub side_sounds: Vec<EvaSideSounds>,
}

impl Default for EvaCheckInfo {
    /// `EvaCheckInfo::EvaCheckInfo()`: `EVA_COUNT`, priority 1 (the lowest), 900 frames between checks (30 seconds) and
    /// 150 to expire (5 seconds).
    fn default() -> Self {
        EvaCheckInfo { message: EVA_COUNT as EvaMessage, frames_between_checks: 900, frames_to_expire: 150, priority: 1, side_sounds: Vec::new() }
    }
}

/// `parseSideSoundsList`: a `SideSounds` block, appended.
fn parse_side_sounds_list(ini: &mut Ini<'_>, c: &mut EvaCheckInfo, _: &UserData) -> IniResult<()> {
    let mut new_sounds = EvaSideSounds::default();
    ini.init_from_ini(&mut new_sounds, &EVA_SIDE_SOUNDS_FIELDS)?;
    c.side_sounds.push(new_sounds);
    Ok(())
}

/// `EvaCheckInfo::s_evaEventInfo`.
pub static EVA_CHECK_INFO_FIELDS: [FieldParse<EvaCheckInfo>; 5] = [
    FieldParse::new("Priority", |i, c, _| i.parse_unsigned_int().map(|v| c.priority = v), UserData::None),
    FieldParse::new("TimeBetweenChecksMS", |i, c, _| i.parse_duration_unsigned_int().map(|v| c.frames_between_checks = v), UserData::None),
    FieldParse::new("ExpirationTimeMS", |i, c, _| i.parse_duration_unsigned_int().map(|v| c.frames_to_expire = v), UserData::None),
    FieldParse::new("SideSounds", parse_side_sounds_list, UserData::None),
    FieldParse::end(),
];

/// `INI::parseEvaEvent`: `newEvaCheckInfo(name)`, and its fields; a message already described is skipped (its block's
/// fields are left to the loader, as upstream returns without reading them).
fn parse_eva_event(ini: &mut Ini<'_>, eva: &mut Eva) -> IniResult<()> {
    let name = ini.get_next_token(None)?;
    let Some(i) = eva.new_eva_check_info(&name) else { return Ok(()) };
    ini.init_from_ini(&mut eva.all_check_infos[i], &EVA_CHECK_INFO_FIELDS)
}

/// The `EvaEvent` row of `theTypeTable`.
pub static EVA_BLOCKS: &[BlockParse<Eva>] = &[BlockParse { token: "EvaEvent", parse: parse_eva_event }];

/// `struct EvaCheck`: a message that wants to play, or has played and waits until it may be checked again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaCheck {
    /// `m_evaInfo`, as its index in [`Eva::all_check_infos`].
    pub eva_info: usize,
    /// `m_triggeredOnFrame`.
    pub triggered_on_frame: u32,
    /// `m_timeForNextCheck`.
    pub time_for_next_check: u32,
    /// `m_alreadyPlayed`.
    pub already_played: bool,
}

/// What EVA asks of the client besides the audio: the frame and the local player's power and side.
pub trait EvaHost: AudioHost {
    /// `TheGameLogic->getFrame()`.
    fn eva_frame(&self) -> u32;
    /// `player->getEnergy()->hasSufficientPower()`.
    fn eva_has_sufficient_power(&self, player: i32) -> bool;
    /// `player->getSide()`.
    fn eva_player_side(&self, player: i32) -> Vec<u8>;
    /// For [`SuperweaponTimers`]: whether the object exists and is not under construction, and, for its special power
    /// module of `template` (`getSpecialPowerModule`), `isReady()` (`None`: no such module).
    fn eva_superweapon_ready(&self, object: ObjectId, template: u32) -> Option<Option<bool>>;
    /// For [`SuperweaponTimers`]: `getSpecialPowerType()` of the template.
    fn eva_special_power_type(&self, template: u32) -> Option<SpecialPowerType>;
    /// For [`SuperweaponTimers`]: `getRequiredScience()` of the template, and whether `player` has it.
    fn eva_hidden_by_science(&self, player: i32, template: u32) -> bool;
    /// For [`SuperweaponTimers`]: the object's controlling player.
    fn eva_controlling_player(&self, object: ObjectId) -> Option<i32>;
    /// For [`SuperweaponTimers`]: `player->getRelationship(object->getTeam())`.
    fn eva_relationship(&self, player: i32, object: ObjectId) -> Relationship;
}

/// `class Eva`.
#[derive(Clone, Debug)]
pub struct Eva {
    /// `m_allCheckInfos`, in the order read.
    pub all_check_infos: Vec<EvaCheckInfo>,
    /// `m_checks`.
    pub checks: Vec<EvaCheck>,
    /// `m_evaSpeech`: the one speech EVA may be playing.
    pub eva_speech: AudioEventRts,
    /// `m_previousBuildingCount`.
    pub previous_building_count: i32,
    /// `m_previousUnitCount`.
    pub previous_unit_count: i32,
    /// `m_shouldPlay`.
    pub should_play: [bool; EVA_COUNT],
    /// `m_enabled`.
    pub enabled: bool,
}

impl Default for Eva {
    fn default() -> Self {
        Self::new()
    }
}

impl Eva {
    /// `Eva::Eva()`: nothing described, nothing waiting, enabled.
    pub fn new() -> Self {
        Eva {
            all_check_infos: Vec::new(),
            checks: Vec::new(),
            eva_speech: AudioEventRts::default(),
            previous_building_count: 0,
            previous_unit_count: 0,
            should_play: [false; EVA_COUNT],
            enabled: true,
        }
    }

    /// `Eva::init`'s `INI::load("Data\\INI\\Eva.ini", INI_LOAD_OVERWRITE)` of the file's text.
    pub fn load_text(&mut self, path: &[u8], text: &[u8]) -> IniResult<()> {
        Ini::load(path, text, IniLoadType::Overwrite, None, EVA_BLOCKS, self)
    }

    /// `Eva::reset()`: the counts, the waiting messages and the flags cleared, and enabled again.
    pub fn reset(&mut self) {
        self.previous_unit_count = 0;
        self.previous_building_count = 0;
        self.checks.clear();
        self.should_play = [false; EVA_COUNT];
        self.enabled = true;
    }

    /// `Eva::newEvaCheckInfo(name)`: a new description for the message of that name, as its index; `None` if one exists.
    pub fn new_eva_check_info(&mut self, name: &[u8]) -> Option<usize> {
        let mesg = name_to_message(name);
        if self.all_check_infos.iter().any(|c| c.message == mesg) {
            return None;
        }
        self.all_check_infos.push(EvaCheckInfo { message: mesg, ..Default::default() });
        Some(self.all_check_infos.len() - 1)
    }

    /// `Eva::getEvaCheckInfo(name)`, as its index.
    pub fn get_eva_check_info(&self, name: &[u8]) -> Option<usize> {
        let mesg = name_to_message(name);
        self.all_check_infos.iter().position(|c| c.message == mesg)
    }

    /// `Eva::setShouldPlay(message)` (a message outside the table is ignored; upstream writes past its array).
    pub fn set_should_play(&mut self, message: EvaMessage) {
        if let Some(f) = usize::try_from(message).ok().and_then(|i| self.should_play.get_mut(i)) {
            *f = true;
        }
    }

    /// `Eva::setEvaEnabled(enabled)`: the waiting flags cleared.
    pub fn set_eva_enabled(&mut self, enabled: bool) {
        self.should_play = [false; EVA_COUNT];
        self.enabled = enabled;
    }

    /// `Eva::isTimeForCheck(message, frame)`: not while the message waits in `m_checks`.
    fn is_time_for_check(&self, message: EvaMessage) -> bool {
        !self.checks.iter().any(|c| self.all_check_infos[c.eva_info].message == message)
    }

    /// `Eva::messageShouldPlay(message, frame)`: never without a local player; low power while the local player's power
    /// is short (`shouldPlayLowPower`); any other message when its flag is set, clearing it (`shouldPlayGenericHandler`).
    fn message_should_play(&mut self, message: EvaMessage, local: Option<i32>, host: &impl EvaHost) -> bool {
        let Some(local) = local else { return false };
        if message == EVA_LOW_POWER {
            return !host.eva_has_sufficient_power(local);
        }
        let f = &mut self.should_play[message as usize];
        if *f {
            *f = false;
            return true;
        }
        false
    }

    /// `Eva::playMessage(message, frame)`: a check for the message, if `Eva.ini` describes it.
    fn play_message(&mut self, message: EvaMessage, current_frame: u32) {
        let Some(info) = self.get_eva_check_info(message_to_name(message).as_bytes()) else { return };
        let frames_between_checks = self.all_check_infos[info].frames_between_checks;
        self.checks.push(EvaCheck {
            eva_info: info,
            time_for_next_check: current_frame.wrapping_add(frames_between_checks),
            triggered_on_frame: current_frame,
            already_played: false,
        });
    }

    /// `Eva::processPlayingMessages(frame)`: drops the checks that have played and may check again next frame and those
    /// that expired unplayed; then, unless the speech is still playing, plays the waiting check of the highest priority
    /// (the first of equal ones) with a random one of the local player's side's sounds (none if the side has no entry).
    fn process_playing_messages(&mut self, current_frame: u32, local: i32, host: &impl EvaHost, manager: &mut AudioManager) {
        let infos = &self.all_check_infos;
        self.checks.retain(|c| {
            // requests that will be available next frame because they have played
            if c.time_for_next_check <= current_frame.wrapping_add(1) && c.already_played {
                return false;
            }
            // requests that never got to play and have since expired
            !(c.triggered_on_frame.wrapping_add(infos[c.eva_info].frames_to_expire) <= current_frame && !c.already_played)
        });
        if self.checks.is_empty() {
            return;
        }
        // if some speech is playing, we are done
        if manager.is_currently_playing(self.eva_speech.playing_handle) {
            return;
        }
        let mut stored = None;
        let mut highest_priority = 0u32;
        for (i, c) in self.checks.iter().enumerate() {
            let p = self.all_check_infos[c.eva_info].priority;
            if p > highest_priority && !c.already_played {
                stored = Some(i);
                highest_priority = p;
            }
        }
        let Some(stored) = stored else { return };
        // we've got a winner
        let side = host.eva_player_side(local);
        let info = &self.all_check_infos[self.checks[stored].eva_info];
        // cleared: if the side is not found, nothing plays
        self.eva_speech.set_event_name(b"");
        if let Some(s) = info.side_sounds.iter().find(|s| s.side.eq_ignore_ascii_case(&side)) {
            if !s.sound_names.is_empty() {
                let pick = manager.random.game_client_random_value(0, s.sound_names.len() as i32 - 1);
                let name = s.sound_names[pick as usize].clone();
                self.eva_speech.set_event_name(&name);
            }
        }
        let frames_between_checks = info.frames_between_checks;
        let c = &mut self.checks[stored];
        c.already_played = true;
        c.time_for_next_check = current_frame.wrapping_add(frames_between_checks);
        // heard by the local player
        self.eva_speech.player_index = local;
        let h: AudioHandle = manager.add_audio_event(host, &mut self.eva_speech);
        self.eva_speech.playing_handle = h;
    }

    /// `Eva::update()`: nothing while disabled or before frame 2; then each message not waiting is checked and queued if
    /// it should play, the waiting ones are played, and every flag is cleared.
    pub fn update(&mut self, host: &impl EvaHost, manager: &mut AudioManager) {
        if !self.enabled {
            return;
        }
        let local = host.local_player_index();
        let frame = host.eva_frame();
        // not for the first few frames: the initial power is 0, etc.
        if frame < 2 {
            return;
        }
        for mesg in 0..EVA_COUNT as EvaMessage {
            if self.is_time_for_check(mesg) && self.message_should_play(mesg, local, host) {
                self.play_message(mesg, frame);
            }
        }
        // (upstream reads `ThePlayerList->getLocalPlayer()` there; with none it would not get this far with a check)
        if let Some(local) = local {
            self.process_playing_messages(frame, local, host, manager);
        }
        // the flags not probed must trigger again to be valid
        self.should_play = [false; EVA_COUNT];
    }

    /// The logic's `TheEva` calls, from its client events: `setShouldPlay` and `setEvaEnabled` (see the module docs).
    pub fn take_event(&mut self, ev: &ClientEvent) {
        let name: String = match ev {
            ClientEvent::Object(ObjectClientEvent::Eva { message }) => match message {
                ObjectEva::BuildingLost => "BUILDINGLOST",
                ObjectEva::UnitLost => "UNITLOST",
                ObjectEva::BuildingSabotaged => "BUILDINGSABOTAGED",
                ObjectEva::CashStolen => "CASHSTOLEN",
                ObjectEva::VehicleStolen => "VEHICLESTOLEN",
                ObjectEva::UpgradeComplete => "UPGRADECOMPLETE",
                ObjectEva::BuildingBeingStolen => "BUILDINGBEINGSTOLEN",
                ObjectEva::BuildingStolen => "BUILDINGSTOLEN",
            }
            .to_string(),
            ClientEvent::Object(ObjectClientEvent::EvaSuperweaponLaunched { weapon, whose }) => {
                let weapon = match weapon {
                    EvaSuperweapon::ParticleCannon => "PARTICLECANNON",
                    EvaSuperweapon::Nuke => "NUKE",
                    EvaSuperweapon::ScudStorm => "SCUDSTORM",
                    EvaSuperweapon::GpsScrambler => "GPS_SCRAMBLER",
                    EvaSuperweapon::SneakAttack => "SNEAK_ATTACK",
                };
                format!("SUPERWEAPONLAUNCHED_{}_{weapon}", whose_name(*whose))
            }
            ClientEvent::Object(ObjectClientEvent::StealthDetectorFeedback { eva, .. }) => {
                if *eva != EVA_INVALID {
                    self.set_should_play(*eva);
                }
                return;
            }
            ClientEvent::Player(PlayerClientEvent::Eva(m)) => match m {
                PlayerEva::GeneralLevelUp => "GENERALLEVELUP",
                PlayerEva::SuperweaponDetectedOwnParticleCannon => "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",
                PlayerEva::SuperweaponDetectedAllyParticleCannon => "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON",
                PlayerEva::SuperweaponDetectedEnemyParticleCannon => "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON",
                PlayerEva::SuperweaponDetectedOwnNuke => "SUPERWEAPONDETECTED_OWN_NUKE",
                PlayerEva::SuperweaponDetectedAllyNuke => "SUPERWEAPONDETECTED_ALLY_NUKE",
                PlayerEva::SuperweaponDetectedEnemyNuke => "SUPERWEAPONDETECTED_ENEMY_NUKE",
                PlayerEva::SuperweaponDetectedOwnScudStorm => "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
                PlayerEva::SuperweaponDetectedAllyScudStorm => "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
                PlayerEva::SuperweaponDetectedEnemyScudStorm => "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
            }
            .to_string(),
            ClientEvent::Dispatch(ClientCall::EvaBeaconDetected) => "BEACONDETECTED".to_string(),
            ClientEvent::EvaSetEnabled(on) => {
                self.set_eva_enabled(*on);
                return;
            }
            _ => return,
        };
        self.set_should_play(name_to_message(name.as_bytes()));
    }
}

/// `Own`, `Ally` or `Enemy`, as the message names spell them.
fn whose_name(w: EvaWhose) -> &'static str {
    match w {
        EvaWhose::Own => "OWN",
        EvaWhose::Ally => "ALLY",
        EvaWhose::Enemy => "ENEMY",
    }
}

/// `SuperweaponInfo`: what of a superweapon timer decides its EVA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuperweaponInfo {
    /// The player whose list holds it.
    pub player_index: i32,
    /// The power's name (`m_superweapons[player]`'s key).
    pub power_name: Vec<u8>,
    /// `m_id`.
    pub id: ObjectId,
    /// `m_powerTemplate`, by id.
    pub template: u32,
    /// `m_hiddenByScript`.
    pub hidden_by_script: bool,
    /// `m_hiddenByScience`.
    pub hidden_by_science: bool,
    /// `m_evaReadyPlayed`.
    pub eva_ready_played: bool,
}

/// `InGameUI::m_superweapons`: the public superweapon timers, for their "ready" EVA.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuperweaponTimers {
    /// The timers, in the order added.
    pub infos: Vec<SuperweaponInfo>,
}

impl SuperweaponTimers {
    /// `InGameUI::addSuperweapon(playerIndex, powerName, id, powerTemplate)`: not twice for the same (`findSWInfo`); hidden
    /// by science when the template requires one the player does not have.
    pub fn add_superweapon(&mut self, host: &impl EvaHost, player_index: i32, power_name: &[u8], id: ObjectId, template: u32) {
        if self.infos.iter().any(|i| i.player_index == player_index && i.power_name == power_name && i.id == id && i.template == template) {
            return;
        }
        let hidden_by_science = host.eva_hidden_by_science(player_index, template);
        self.infos.push(SuperweaponInfo {
            player_index,
            power_name: power_name.to_vec(),
            id,
            template,
            hidden_by_script: false,
            hidden_by_science,
            eva_ready_played: false,
        });
    }

    /// The logic's `addSuperweapon` calls, from its client events.
    pub fn take_event(&mut self, host: &impl EvaHost, ev: &ClientEvent) {
        if let ClientEvent::Object(ObjectClientEvent::UiAddSuperweapon { player_index, power_name, object_id, template }) = ev {
            self.add_superweapon(host, *player_index, power_name, *object_id, *template);
        }
    }

    /// The EVA half of `InGameUI::postDraw`'s superweapon timers (frame > 0): for each timer not hidden, whose object
    /// exists and is built and has the power's module, a change to ready asks for `SuperweaponReady_<whose>_<weapon>`
    /// (for the particle cannon, the neutron missile and the scud storm) once, until it is not ready again. Upstream walks
    /// the players' maps by power name; the order does not change which flags are set.
    pub fn update(&mut self, host: &impl EvaHost, eva: &mut Eva) {
        if host.eva_frame() == 0 {
            return;
        }
        let mut order: Vec<usize> = (0..self.infos.len()).collect();
        order.sort_by(|&a, &b| (self.infos[a].player_index, &self.infos[a].power_name).cmp(&(self.infos[b].player_index, &self.infos[b].power_name)));
        for i in order {
            let info = &mut self.infos[i];
            if info.hidden_by_script || info.hidden_by_science {
                continue;
            }
            // the object gone or under construction: skipped
            let Some(module) = host.eva_superweapon_ready(info.id, info.template) else { continue };
            let Some(is_ready) = module else { continue };
            if is_ready && !info.eva_ready_played {
                let kind = match host.eva_special_power_type(info.template) {
                    Some(SPECIAL_PARTICLE_UPLINK_CANNON | SUPW_SPECIAL_PARTICLE_UPLINK_CANNON | LAZR_SPECIAL_PARTICLE_UPLINK_CANNON) => Some("PARTICLECANNON"),
                    Some(SPECIAL_NEUTRON_MISSILE | NUKE_SPECIAL_NEUTRON_MISSILE | SUPW_SPECIAL_NEUTRON_MISSILE) => Some("NUKE"),
                    Some(SPECIAL_SCUD_STORM) => Some("SCUDSTORM"),
                    _ => None,
                };
                if let (Some(kind), Some(local)) = (kind, host.local_player_index()) {
                    let whose = if host.eva_controlling_player(info.id) == Some(local) {
                        EvaWhose::Own
                    } else if host.eva_relationship(local, info.id) != Relationship::Enemies {
                        // (upstream: "counting relationship NEUTRAL as ally")
                        EvaWhose::Ally
                    } else {
                        EvaWhose::Enemy
                    };
                    eva.set_should_play(name_to_message(format!("SUPERWEAPONREADY_{}_{kind}", whose_name(whose)).as_bytes()));
                }
                info.eva_ready_played = true;
            } else if !is_ready {
                // reset for next time
                info.eva_ready_played = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::audio_info::AudioTables;
    use crate::client::client_audio::{ClientAudio, WorldAudioHost};
    use crate::client::game_audio::tests::LogDevice;
    use crate::client::game_audio::ListenerView;
    use crate::common::base_type::Coord3D;
    use crate::logic::world::game_logic::World;
    use crate::logic::world::game_logic_update::CRC_RECALC;

    /// A test `Eva.ini` (this project's, not the game's).
    const EVA_INI: &[u8] = b"\
EvaEvent UNITLOST\n Priority = 2\n TimeBetweenChecksMS = 1000\n ExpirationTimeMS = 2000\n\
 SideSounds\n  Side = America\n  Sounds = EvaUSA_UnitLost\n End\n\
 SideSounds\n  Side = China\n  Sounds = EvaChina_UnitLost1 EvaChina_UnitLost2\n End\nEnd\n\
EvaEvent BUILDINGLOST\n Priority = 5\n\
 SideSounds\n  Side = America\n  Sounds = EvaUSA_BuildingLost\n End\nEnd\n\
EvaEvent LOWPOWER\n Priority = 9\n SideSounds\n  Side = America\n  Sounds = EvaUSA_LowPower\n End\nEnd\n\
EvaEvent CASHSTOLEN\n SideSounds\n  Side = America\n  Sounds = EvaUSA_CashStolen\n End\nEnd\n\
EvaEvent VEHICLESTOLEN\n SideSounds\n  Side = America\n  Sounds = EvaUSA_VehicleStolen\n End\nEnd\n";

    /// The test sounds: one file each.
    const AUDIO_INI: &[u8] = b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n SoundsExtension = wav\n SampleCount2D = 8\n SampleCount3D = 8\n MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\nEnd\n\
AudioEvent EvaUSA_UnitLost\n Sounds = usaunit\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaUSA_BuildingLost\n Sounds = usabuilding\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaUSA_VehicleStolen\n Sounds = usavehicle\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaUSA_CashStolen\n Sounds = usacash\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaUSA_LowPower\n Sounds = usapower\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaChina_UnitLost1\n Sounds = china1\n Type = ui everyone\n Volume = 100%\nEnd\n\
AudioEvent EvaChina_UnitLost2\n Sounds = china2\n Type = ui everyone\n Volume = 100%\nEnd\n";

    std::thread_local! {
        /// The test superweapons' readiness, by object (`None`: gone or under construction).
        static READY: std::cell::RefCell<std::collections::BTreeMap<ObjectId, Option<bool>>> = Default::default();
    }

    /// The audio tests' host (local player 0; 1 its enemy, 2 its ally) with superweapons: object 10 player 0's particle
    /// cannon, 11 player 1's scud storm, 12 player 2's neutron missile, 13 player 0's daisy cutter, 14 player 0's cannon
    /// whose template (5) needs a science player 0 lacks. Template = object - 9.
    impl EvaHost for crate::client::audio_event::tests::TestHost {
        fn eva_frame(&self) -> u32 {
            10
        }
        fn eva_has_sufficient_power(&self, _: i32) -> bool {
            true
        }
        fn eva_player_side(&self, _: i32) -> Vec<u8> {
            b"America".to_vec()
        }
        fn eva_superweapon_ready(&self, object: ObjectId, _: u32) -> Option<Option<bool>> {
            READY.with(|r| r.borrow().get(&object).copied().flatten()).map(Some)
        }
        fn eva_special_power_type(&self, template: u32) -> Option<SpecialPowerType> {
            Some(match template {
                1 | 5 => SPECIAL_PARTICLE_UPLINK_CANNON,
                2 => SPECIAL_SCUD_STORM,
                3 => NUKE_SPECIAL_NEUTRON_MISSILE,
                _ => SPECIAL_DAISY_CUTTER,
            })
        }
        fn eva_hidden_by_science(&self, _: i32, template: u32) -> bool {
            template == 5
        }
        fn eva_controlling_player(&self, object: ObjectId) -> Option<i32> {
            Some(match object {
                11 => 1,
                12 => 2,
                _ => 0,
            })
        }
        fn eva_relationship(&self, _: i32, object: ObjectId) -> Relationship {
            match object {
                11 => Relationship::Enemies,
                _ => Relationship::Allies,
            }
        }
    }

    /// GitHub #55: the superweapon timers' "ready" EVA (`InGameUI::postDraw`).
    ///
    /// *Predictions, written before the first run:* with every timer not ready nothing is asked; object 10 ready asks
    /// `SUPERWEAPONREADY_OWN_PARTICLECANNON` once (not on the next frame while it stays ready), and again after it was not
    /// ready for a frame; 11 ready asks `..._ENEMY_SCUDSTORM`, 12 `..._ALLY_NUKE`; the daisy cutter (13) asks nothing; the
    /// timer hidden by science (14) asks nothing though ready; 10 gone (`None`) asks nothing; adding 10 twice keeps one
    /// timer.
    #[test]
    fn a_superweapon_ready_asks_eva_once_per_readiness() {
        let host = crate::client::audio_event::tests::TestHost::default();
        let set = |pairs: &[(ObjectId, Option<bool>)]| READY.with(|r| r.borrow_mut().extend(pairs.iter().copied()));
        let mut t = SuperweaponTimers::default();
        for (o, p) in [(10, 0), (10, 0), (11, 1), (12, 2), (13, 0), (14, 0)] {
            t.add_superweapon(&host, p, b"Power", o, o as u32 - 9);
        }
        assert_eq!(t.infos.len(), 5, "one timer for 10");
        let asked = |t: &mut SuperweaponTimers| {
            let mut eva = Eva::new();
            t.update(&host, &mut eva);
            (0..EVA_COUNT).filter(|&i| eva.should_play[i]).map(|i| EVA_MESSAGE_NAMES[i]).collect::<Vec<_>>()
        };
        set(&[(10, Some(false)), (11, Some(false)), (12, Some(false)), (13, Some(false)), (14, Some(true))]);
        let none = asked(&mut t);
        set(&[(10, Some(true))]);
        let own = asked(&mut t);
        let own_again = asked(&mut t);
        set(&[(10, Some(false))]);
        let reset = asked(&mut t);
        set(&[(10, Some(true)), (11, Some(true)), (12, Some(true)), (13, Some(true))]);
        let all = asked(&mut t);
        set(&[(10, None)]);
        let gone = asked(&mut t);
        eprintln!("#55 superweapons: {none:?} {own:?} {own_again:?} {reset:?} {all:?} {gone:?}");
        assert!(none.is_empty());
        assert_eq!(own, ["SUPERWEAPONREADY_OWN_PARTICLECANNON"]);
        assert!(own_again.is_empty() && reset.is_empty());
        assert_eq!(all, ["SUPERWEAPONREADY_OWN_PARTICLECANNON", "SUPERWEAPONREADY_ALLY_NUKE", "SUPERWEAPONREADY_ENEMY_SCUDSTORM"]);
        assert!(gone.is_empty());
    }

    #[test]
    fn the_ini_reads_upstreams_fields_and_defaults() {
        let mut eva = Eva::new();
        eva.load_text(EVA_INI_FILE, EVA_INI).expect("the INI");
        assert_eq!(eva.all_check_infos.len(), 5);
        let u = &eva.all_check_infos[0];
        assert_eq!((u.message, u.priority, u.frames_between_checks, u.frames_to_expire), (39, 2, 30, 60));
        assert_eq!(u.side_sounds[1], EvaSideSounds { side: b"China".to_vec(), sound_names: vec![b"EvaChina_UnitLost1".to_vec(), b"EvaChina_UnitLost2".to_vec()] });
        let b = &eva.all_check_infos[1];
        assert_eq!((b.message, b.priority, b.frames_between_checks, b.frames_to_expire), (29, 5, 900, 150), "the defaults");
        assert_eq!(eva.new_eva_check_info(b"unitlost"), None, "one description a message");
    }

    /// What one EVA frame of [`run`] saw.
    struct Run {
        w: World,
        audio: ClientAudio,
        dev: LogDevice,
        /// The files started, with the frame.
        started: Vec<(u32, Vec<u8>)>,
        crcs: Vec<u32>,
    }

    impl Run {
        fn new() -> Self {
            let mut w = crate::logic::world::gate_scenario::gate_map_world();
            crate::logic::world::player_list::player_list_set_local_player(&mut w, Some(1)).expect("the local player");
            w.players.get_nth_mut(1).expect("Player1").side = b"America".to_vec();
            let mut tables = AudioTables::new();
            tables.load_text(b"a.ini", AUDIO_INI).expect("the audio INI");
            let mut audio = ClientAudio::new(crate::client::game_audio::AudioManager::new(tables));
            audio.eva.load_text(EVA_INI_FILE, EVA_INI).expect("the EVA INI");
            Run { w, audio, dev: LogDevice::default(), started: Vec::new(), crcs: Vec::new() }
        }

        /// One logic frame, then EVA's with `events` added to the frame's, then the audio's; the speech playing is
        /// finished first when `finish`.
        fn frame(&mut self, events: &[ClientEvent], finish: bool) {
            self.w.logic_update().expect("a frame");
            let mut ev = self.w.take_client_events();
            ev.extend_from_slice(events);
            if finish {
                let playing: Vec<_> = self.dev.started.iter().map(|s| s.0).filter(|v| !self.dev.stopped.contains(v)).collect();
                self.dev.finished.extend(playing);
            }
            let files = |p: &[u8]| p.starts_with(b"Data\\Audio\\Sounds\\");
            let host = WorldAudioHost { world: &self.w, local_player_index: None, files: &files, language: b"English" };
            self.audio.update_eva(&host, &ev, 0);
            let before = self.dev.started.len();
            self.audio.manager.update(&host, &mut self.dev, &ListenerView { ground: Coord3D::default(), angle: 0.0, camera: Coord3D { x: 0.0, y: 0.0, z: 300.0 } });
            for s in &self.dev.started[before..] {
                self.started.push((self.w.frame(), s.1.clone()));
            }
            self.crcs.push(self.w.logic_crc(CRC_RECALC, None).expect("the CRC"));
        }
    }

    fn eva(m: ObjectEva) -> ClientEvent {
        ClientEvent::Object(ObjectClientEvent::Eva { message: m })
    }

    /// GitHub #55: `Eva::update` and `processPlayingMessages` over the gate game, with a test `Eva.ini` and the logic's
    /// `setShouldPlay` events.
    ///
    /// *Predictions, written before the first run:*
    /// 1. nothing plays before frame 2; a request made at frame 1 (vehicle stolen) plays at frame 2 (`update` returns before frame 2 without
    ///    clearing the flags);
    /// 2. Unit lost and building lost asked in the same frame (5): building lost (priority 5) starts that frame
    ///    (`usabuilding.wav`), unit lost (2) waits while the speech plays;
    /// 3. the speech finished at frame 8: unit lost starts at frame 8 or 9;
    /// 4. unit lost asked again at once is not checked (it waits in `m_checks` for 30 frames after playing) and never plays;
    ///    asked again 31 frames after it played, it plays;
    /// 5. cash stolen asked while a speech plays that does not end within its 150 frames of expiry (the default) is
    ///    dropped: it does not play when the speech ends after that;
    /// 6. disabled by the script action's event, a request plays nothing;
    /// 7. Player1's power is sufficient (none made, none used), so low power never plays;
    /// 8. the side China picks one of its two sounds (`GameClientRandomValue`);
    /// 9. the logic's CRCs equal those of the same frames with no client attached.
    #[test]
    fn eva_plays_by_priority_waits_out_its_interval_and_expires() {
        let mut r = Run::new();
        let mut lone = crate::logic::world::gate_scenario::gate_map_world();
        crate::logic::world::player_list::player_list_set_local_player(&mut lone, Some(1)).expect("the local player");
        r.frame(&[eva(ObjectEva::VehicleStolen)], false); // frame 1
        let early = r.started.clone();
        r.frame(&[], true); // frame 2
        let at_two = r.started.clone();
        for _ in 3..5 {
            r.frame(&[], true);
        }
        r.started.clear();
        r.frame(&[eva(ObjectEva::UnitLost), eva(ObjectEva::BuildingLost)], true); // frame 5
        let both = r.started.clone();
        r.frame(&[], false);
        r.frame(&[], false);
        r.frame(&[], true); // frame 8: the speech finished
        r.frame(&[], false);
        let after_finish = r.started.clone();
        let unit_played = r.started.iter().find(|s| s.1.ends_with(b"usaunit.wav")).map(|s| s.0);
        // (4) asked again at once, then each frame until 31 frames after it played
        r.started.clear();
        r.frame(&[eva(ObjectEva::UnitLost)], true);
        let again_at_once = r.started.clone();
        let p = unit_played.unwrap_or(0);
        while r.w.frame() < p + 30 {
            r.frame(&[], true);
        }
        r.frame(&[eva(ObjectEva::UnitLost)], true);
        let after_interval = r.started.clone();
        // (5) unit lost (its interval over) held for 160 frames while cash stolen waits
        for _ in 0..31 {
            r.frame(&[], true);
        }
        r.started.clear();
        r.frame(&[eva(ObjectEva::UnitLost)], true);
        let held = r.started.clone();
        r.frame(&[eva(ObjectEva::CashStolen)], false);
        for _ in 0..160 {
            r.frame(&[], false);
        }
        r.started.clear();
        r.frame(&[], true);
        r.frame(&[], true);
        let after_expiry = r.started.clone();
        // (6) disabled
        r.frame(&[ClientEvent::EvaSetEnabled(false)], true);
        r.frame(&[eva(ObjectEva::UnitLost)], true);
        r.frame(&[], true);
        let disabled = r.started.clone();
        r.frame(&[ClientEvent::EvaSetEnabled(true)], true);
        // (8) China
        r.w.players.get_nth_mut(1).expect("Player1").side = b"China".to_vec();
        for _ in 0..40 {
            r.frame(&[], true);
        }
        r.started.clear();
        r.frame(&[eva(ObjectEva::UnitLost)], true);
        let china = r.started.clone();
        let lone_crcs: Vec<u32> = (0..r.crcs.len())
            .map(|_| {
                lone.logic_update().expect("a frame");
                lone.take_client_events();
                lone.logic_crc(CRC_RECALC, None).expect("the CRC")
            })
            .collect();
        let name = |v: &[(u32, Vec<u8>)]| v.iter().map(|(f, n)| format!("{f}:{}", String::from_utf8_lossy(n))).collect::<Vec<_>>();
        eprintln!(
            "#55 EVA: early {:?}; at two {:?}; both {:?}; after finish {:?}; again at once {:?}; after interval {:?}; held {:?}; after expiry {:?}; disabled {:?}; china {:?}",
            name(&early), name(&at_two), name(&both), name(&after_finish), name(&again_at_once), name(&after_interval), name(&held), name(&after_expiry), name(&disabled), name(&china)
        );
        assert!(early.is_empty(), "(1) nothing before frame 2");
        assert!(at_two.iter().any(|s| s.0 == 2 && s.1.ends_with(b"usavehicle.wav")), "(1) the frame-1 request plays at frame 2");
        assert_eq!(both.len(), 1, "(2) one speech at a time");
        assert!(both[0].1.ends_with(b"usabuilding.wav") && both[0].0 == 5, "(2) the higher priority first");
        assert!(matches!(unit_played, Some(8 | 9)), "(3) unit lost after the speech: {unit_played:?}");
        assert!(again_at_once.is_empty(), "(4) not checked while waiting");
        assert!(after_interval.iter().any(|s| s.1.ends_with(b"usaunit.wav")), "(4) checked again after its interval");
        assert!(held.len() == 1 && held[0].1.ends_with(b"usaunit.wav"), "(5) the holding speech");
        assert!(after_expiry.is_empty(), "(5) the cash stolen request expired while the speech played");
        assert!(disabled.is_empty(), "(6) disabled");
        assert!(r.started.iter().all(|s| !s.1.ends_with(b"usapower.wav")), "(7) no low power");
        assert_eq!(china.len(), 1);
        assert!(china[0].1.ends_with(b"china1.wav") || china[0].1.ends_with(b"china2.wav"), "(8) a China sound");
        assert_eq!(r.crcs, lone_crcs, "(9) EVA changes no CRC");
    }


    /// GitHub #55: each of the logic's `setShouldPlay` events asks for its message by upstream's name. Predicted: the 8
    /// object messages, the 15 superweapons launched, the general's level up and two of the superweapons detected, the
    /// beacon and the stealth detector's message number (28 events) each set exactly their flag; `EvaSetEnabled(false)`
    /// clears the flags and disables.
    #[test]
    fn the_logics_eva_events_ask_for_their_messages() {
        use crate::logic::world::object_events::StealthFeedback;
        let mut cases: Vec<(ClientEvent, String)> = Vec::new();
        for (m, n) in [
            (ObjectEva::BuildingLost, "BUILDINGLOST"),
            (ObjectEva::UnitLost, "UNITLOST"),
            (ObjectEva::BuildingSabotaged, "BUILDINGSABOTAGED"),
            (ObjectEva::CashStolen, "CASHSTOLEN"),
            (ObjectEva::VehicleStolen, "VEHICLESTOLEN"),
            (ObjectEva::UpgradeComplete, "UPGRADECOMPLETE"),
            (ObjectEva::BuildingBeingStolen, "BUILDINGBEINGSTOLEN"),
            (ObjectEva::BuildingStolen, "BUILDINGSTOLEN"),
        ] {
            cases.push((ClientEvent::Object(ObjectClientEvent::Eva { message: m }), n.to_string()));
        }
        for (w, wn) in [(EvaSuperweapon::ParticleCannon, "PARTICLECANNON"), (EvaSuperweapon::Nuke, "NUKE"), (EvaSuperweapon::ScudStorm, "SCUDSTORM"), (EvaSuperweapon::GpsScrambler, "GPS_SCRAMBLER"), (EvaSuperweapon::SneakAttack, "SNEAK_ATTACK")] {
            for (who, hn) in [(EvaWhose::Own, "OWN"), (EvaWhose::Ally, "ALLY"), (EvaWhose::Enemy, "ENEMY")] {
                cases.push((ClientEvent::Object(ObjectClientEvent::EvaSuperweaponLaunched { weapon: w, whose: who }), format!("SUPERWEAPONLAUNCHED_{hn}_{wn}")));
            }
        }
        cases.push((ClientEvent::Player(PlayerClientEvent::Eva(PlayerEva::GeneralLevelUp)), "GENERALLEVELUP".into()));
        cases.push((ClientEvent::Player(PlayerClientEvent::Eva(PlayerEva::SuperweaponDetectedAllyNuke)), "SUPERWEAPONDETECTED_ALLY_NUKE".into()));
        cases.push((ClientEvent::Player(PlayerClientEvent::Eva(PlayerEva::SuperweaponDetectedEnemyScudStorm)), "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM".into()));
        cases.push((ClientEvent::Dispatch(ClientCall::EvaBeaconDetected), "BEACONDETECTED".into()));
        let feedback = ObjectClientEvent::StealthDetectorFeedback { feedback: StealthFeedback::Discovered, try_radar_event: false, position: Default::default(), player_index: Some(0), eva: 33, };
        cases.push((ClientEvent::Object(feedback), "ENEMYBLACKLOTUSDETECTED".into()));
        for (ev, name) in &cases {
            let mut eva = Eva::new();
            eva.take_event(ev);
            let set: Vec<_> = (0..EVA_COUNT).filter(|&i| eva.should_play[i]).map(|i| EVA_MESSAGE_NAMES[i]).collect();
            assert_eq!(set, [name.as_str()], "{ev:?}");
        }
        let mut eva = Eva::new();
        eva.take_event(&cases[0].0);
        eva.take_event(&ClientEvent::EvaSetEnabled(false));
        assert!(!eva.enabled && eva.should_play.iter().all(|f| !f));
    }

    #[test]
    fn the_names_match_upstreams_table() {
        assert_eq!(EVA_COUNT, 53);
        assert_eq!(name_to_message(b"lowpower"), EVA_LOW_POWER);
        assert_eq!(name_to_message(b"INSUFFICIENTFUNDS"), EVA_INSUFFICIENT_FUNDS);
        assert_eq!(name_to_message(b"BEACONDETECTED"), 32);
        assert_eq!(name_to_message(b"SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK"), 52);
        assert_eq!(name_to_message(b"EVA_INVALID"), EVA_INVALID);
        assert_eq!(message_to_name(39), "UNITLOST");
        assert_eq!(message_to_name(53), "");
    }
}
