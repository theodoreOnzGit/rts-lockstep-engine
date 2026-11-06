// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/MessageStream/CommandXlat.cpp (PickAndPlayInfo::PickAndPlayInfo,
//     pickAndPlayUnitVoiceResponse)
//   GeneralsMD/Code/GameEngine/Include/GameClient/CommandXlat.h (class PickAndPlayInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The units' voice responses (GitHub #51, part 2): `pickAndPlayUnitVoiceResponse`, which the command translators call when
//! they order the selection (move, attack, enter, dock, repair, special powers, ...) and when a group is selected.
//!
//! **Client side only.** Upstream's function reads the selected objects (their templates' sounds, their AI's moving state,
//! weapons, special power modules, the local player's upgrades) and calls `TheAudio->addAudioEvent`, which draws from the
//! audio's own random. Here the reads are the host's ([`UnitVoiceHost`], every one a read without side effects on the World),
//! and the call is a [`UnitVoice`] handed to the host ([`UnitVoiceHost::play_unit_voice`]), which the client's audio plays
//! (`client_audio::ClientAudio::play_unit_voice`) with its own random. Nothing goes to the logic.
//!
//! **What upstream keeps.** After playing, upstream writes the file index played back into the template's event
//! (`setPlayingAudioIndex` on the `const` pointer, "so that we can randomly pick a different sound the next time"). The
//! template is the logic's, so the client keeps those indices itself, by template and sound ([`UnitVoice::source`]).

use crate::client::command_xlat::CommandXlatHost;
use crate::client::in_game_ui::{DrawableId, InGameUiSelection};
use crate::client::selection_groups::SelectionGroupsHost;
use crate::common::game_common::Relationship;
use crate::common::game_type::ObjectId;
use crate::common::kind_of::{KINDOF_DOZER, KINDOF_HARVESTER, KINDOF_HEAL_PAD, KINDOF_IGNORED_IN_GUI, KINDOF_INFANTRY, KINDOF_STRUCTURE};
use crate::common::message_stream::MessageType;
use crate::common::message_types::*;
use crate::common::special_power_type::{SpecialPowerType, SPECIAL_INVALID};
use crate::common::thing_template::ThingTemplateAudioType;
use crate::logic::damage::{DamageType, DAMAGE_DISARM, DAMAGE_FLAME, DAMAGE_KILLPILOT, DAMAGE_MELEE, DAMAGE_POISON, DAMAGE_SURRENDER};

/// `PRIMARY_WEAPON`.
const PRIMARY_WEAPON: i32 = 0;
/// `SECONDARY_WEAPON`.
const SECONDARY_WEAPON: i32 = 1;
/// `TERTIARY_WEAPON`.
const TERTIARY_WEAPON: i32 = 2;

/// `class PickAndPlayInfo`, as its constructor makes it: not air, no target, no weapon slot, `SPECIAL_INVALID`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickAndPlayInfo {
    /// `m_air`: "are we attacking an airborned target?"
    pub air: bool,
    /// `m_drawTarget`: "do we have an override draw target?"
    pub draw_target: Option<DrawableId>,
    /// `m_weaponSlot`: "are we forcing a specific weapon slot? NULL if unspecified."
    pub weapon_slot: Option<i32>,
    /// `m_specialPowerType`.
    pub special_power_type: SpecialPowerType,
}

impl Default for PickAndPlayInfo {
    fn default() -> Self {
        PickAndPlayInfo { air: false, draw_target: None, weapon_slot: None, special_power_type: SPECIAL_INVALID }
    }
}

/// Whose event a voice is a copy of: upstream's `soundToPlayPtr`, the event `setPlayingAudioIndex` writes back into.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoiceSource {
    /// `templ->getAudio(t)` (`getVoiceSelect()`, `getVoiceMove()`, ...).
    Template(ThingTemplateAudioType),
    /// `templ->getPerUnitSound(name)`.
    PerUnit(Vec<u8>),
    /// `spmInterface->getInitiateSound()`: the special power module's data's `InitiateSound`.
    InitiateSound(SpecialPowerType),
    /// `TheAudio->getMiscAudio()->m_terroristInCar*Voice`, a copy: nothing is written back.
    Misc,
}

/// One `TheAudio->addAudioEvent(&soundToPlay)` of the function: the event's name, its object (`setObjectID`) and whose
/// event it copies (with the object's template's name, which the client's kept index is filed under).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitVoice {
    /// `getEventName()` (empty: `s_audioEventNoSound`, which `addAudioEvent` does not play).
    pub name: Vec<u8>,
    /// `setObjectID(objectWithSound->getID())`.
    pub object: ObjectId,
    /// `objectWithSound->getTemplate()->getName()`.
    pub template: Vec<u8>,
    pub source: VoiceSource,
}

/// The car bomb's extra voice (`MiscAudio`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerroristVoice {
    /// `m_terroristInCarAttackVoice`.
    Attack,
    /// `m_terroristInCarMoveVoice`.
    Move,
    /// `m_terroristInCarSelectVoice`.
    Select,
}

/// What `obj->getCurrentWeapon()` / `getWeaponInWeaponSlot(slot)` tells the function.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceWeapon {
    /// `getDamageType()`.
    pub damage_type: DamageType,
    /// `getWeaponSlot()`.
    pub slot: i32,
    /// `getName()`.
    pub name: Vec<u8>,
}

/// What the voice responses read, and where they go. Each answer must leave the World as it was (`AGENTS.md`: the client
/// never changes the simulation). The defaults answer for a host with no objects' sounds: nothing is played.
pub trait UnitVoiceHost: SelectionGroupsHost {
    /// `obj->getTemplate()->getName()`.
    fn voice_template_name(&self, _obj: ObjectId) -> Vec<u8> {
        Vec::new()
    }
    /// `obj->getTemplate()->getAudio(t)`'s event name (empty: `s_audioEventNoSound`).
    fn template_audio(&self, _obj: ObjectId, _t: ThingTemplateAudioType) -> Vec<u8> {
        Vec::new()
    }
    /// `obj->getTemplate()->getPerUnitSound(name)`'s event name (empty: `s_audioEventNoSound`).
    fn template_per_unit_sound(&self, _obj: ObjectId, _name: &[u8]) -> Vec<u8> {
        Vec::new()
    }
    /// `obj->getAI()` and, with one, `ai->isMoving() || ai->isWaitingForPath()`; `None` without an AI.
    fn ai_effectively_moving(&mut self, _obj: ObjectId) -> Option<bool> {
        None
    }
    /// `obj->canCrushOrSquish(target)` (`TEST_CRUSH_OR_SQUISH`).
    fn can_crush_or_squish(&self, _obj: ObjectId, _target: ObjectId) -> bool {
        false
    }
    /// `TheAudio->isValidAudioEvent(event)`: a name with a description.
    fn is_valid_audio_event(&self, _name: &[u8]) -> bool {
        false
    }
    /// `player = obj->getControllingPlayer()`, `player && player->hasUpgradeComplete(TheUpgradeCenter->findUpgrade(
    /// "Upgrade_GLAWorkerShoes"))`.
    fn has_worker_shoes(&self, _obj: ObjectId) -> bool {
        false
    }
    /// `obj->getCurrentWeapon()`, or with a slot `obj->getWeaponInWeaponSlot(slot)`; `None` for `NULL`.
    fn voice_weapon(&self, _obj: ObjectId, _slot: Option<i32>) -> Option<VoiceWeapon> {
        None
    }
    /// `obj->findSpecialPowerModuleInterface(type)` and, with one, its `getInitiateSound()`'s name.
    fn special_power_initiate_sound(&mut self, _obj: ObjectId, _sp: SpecialPowerType) -> Option<Vec<u8>> {
        None
    }
    /// `obj->isUsingAirborneLocomotor()`.
    fn is_using_airborne_locomotor(&self, _obj: ObjectId) -> bool {
        false
    }
    /// `TheAudio->getMiscAudio()`'s car bomb voice.
    fn terrorist_voice(&self, _which: TerroristVoice) -> Vec<u8> {
        Vec::new()
    }
    /// `TheAudio->addAudioEvent(&soundToPlay)`.
    fn play_unit_voice(&mut self, _v: UnitVoice) {}
}

/// `pickAndPlayUnitVoiceResponse(list, msgType, info)`: the voice of the first unit of the list with one for the message
/// (the move's default voice "upgraded" to a crush, salvage or worker-shoes voice by any unit that has one), played on that
/// unit; a car bomb adds the terrorist's voice.
pub fn pick_and_play_unit_voice_response<H: CommandXlatHost>(
    list: &[DrawableId],
    msg: MessageType,
    info: Option<&PickAndPlayInfo>,
    ui: &InGameUiSelection,
    h: &mut H,
) {
    // (the "sound to play" is kept as its name and source: the event itself is the template's)
    let mut sound: Option<(Vec<u8>, VoiceSource)> = None;
    let mut object_with_sound: Option<ObjectId> = None;
    let target = info.and_then(|i| i.draw_target).and_then(|d| h.drawable_object(d));
    let per_unit = |h: &H, obj: ObjectId, name: &[u8]| Some((h.template_per_unit_sound(obj, name), VoiceSource::PerUnit(name.to_vec())));
    let templ = |h: &H, obj: ObjectId, t: ThingTemplateAudioType| Some((h.template_audio(obj, t), VoiceSource::Template(t)));

    for &d in list {
        // (upstream dereferences `getObject()`; a drawable without one has nothing to say here)
        let Some(obj) = h.drawable_object(d) else { continue };
        if h.is_kind_of(obj, KINDOF_IGNORED_IN_GUI) {
            continue;
        }
        let mut skip = false;
        match msg {
            MSG_DOCK => {
                sound = per_unit(h, obj, b"VoiceSupply");
                object_with_sound = Some(obj);
                skip = true;
            }
            t if (MSG_SELECT_TEAM0.0..=MSG_SELECT_TEAM9.0).contains(&t.0) || t == MSG_CREATE_SELECTED_GROUP => {
                sound = templ(h, obj, ThingTemplateAudioType::VoiceSelect);
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_EVACUATE => {
                sound = per_unit(h, obj, b"VoiceUnload");
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_DO_REPAIR => {
                sound = per_unit(h, obj, b"VoiceRepair");
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_COMBATDROP_AT_LOCATION | MSG_COMBATDROP_AT_OBJECT => {
                sound = per_unit(h, obj, b"VoiceCombatDrop");
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_ENTER => {
                sound = match target {
                    Some(t) if h.is_kind_of(t, KINDOF_HEAL_PAD) => per_unit(h, obj, b"VoiceGetHealed"),
                    Some(t) if h.is_kind_of(t, KINDOF_STRUCTURE) => {
                        if h.object_relationship(obj, t) == Relationship::Enemies {
                            // Saboteurs
                            per_unit(h, obj, b"VoiceEnterHostile")
                        } else {
                            per_unit(h, obj, b"VoiceGarrison")
                        }
                    }
                    // order matters: we want to know if I consider it to be an ally, not vice versa
                    Some(t) if h.object_relationship(obj, t) != Relationship::Allies => per_unit(h, obj, b"VoiceEnterHostile"),
                    _ => per_unit(h, obj, b"VoiceEnter"),
                };
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_DO_MOVETO | MSG_DO_ATTACKMOVETO | MSG_GET_REPAIRED | MSG_GET_HEALED | MSG_DO_SALVAGE => {
                if let Some(effectively_moving) = h.ai_effectively_moving(obj) {
                    // "Don't want to play the sound unless he's not moving!"
                    if ui.is_in_waypoint_mode() && effectively_moving {
                        continue;
                    }
                    // "Default to voice move (it'll be selected if we can't find a crush case as we iterate through the
                    // rest of the drawables)"
                    if sound.is_none() {
                        sound = templ(h, obj, ThingTemplateAudioType::VoiceMove);
                        object_with_sound = Some(obj);
                    }
                    if ui.is_in_force_move_to_mode() {
                        if let Some(t) = target {
                            if h.can_crush_or_squish(obj, t) {
                                // "Change it to voice crush because we are intentionally trying to crush this!"
                                sound = per_unit(h, obj, b"VoiceCrush");
                                object_with_sound = Some(obj);
                                skip = true;
                            }
                        }
                    }
                    if msg == MSG_DO_SALVAGE {
                        let temp = h.template_per_unit_sound(obj, b"VoiceSalvage");
                        if h.is_valid_audio_event(&temp) {
                            sound = Some((temp, VoiceSource::PerUnit(b"VoiceSalvage".to_vec())));
                            object_with_sound = Some(obj);
                            skip = true;
                        }
                    }
                    // "Special case for GLA worker to use a different set of move voices when he has received the worker
                    // shoes upgrade"
                    if h.has_worker_shoes(obj) && h.is_kind_of(obj, KINDOF_INFANTRY) && h.is_kind_of(obj, KINDOF_DOZER) && h.is_kind_of(obj, KINDOF_HARVESTER) {
                        sound = per_unit(h, obj, b"VoiceMoveUpgraded");
                        object_with_sound = Some(obj);
                        skip = true;
                    }
                }
            }
            MSG_RESUME_CONSTRUCTION | MSG_DOZER_CONSTRUCT | MSG_DOZER_CONSTRUCT_LINE => {
                sound = per_unit(h, obj, b"VoiceBuildResponse");
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_DO_FORCE_ATTACK_GROUND | MSG_SWITCH_WEAPONS => {
                let mut fall_through = msg == MSG_SWITCH_WEAPONS;
                if msg == MSG_DO_FORCE_ATTACK_GROUND {
                    sound = per_unit(h, obj, b"VoiceBombard");
                    object_with_sound = Some(obj);
                    skip = true;
                    if !h.is_valid_audio_event(&sound.as_ref().map_or_else(Vec::new, |s| s.0.clone())) {
                        // "clear out the sound to play, and drop into the attack object logic" (the switch weapons case,
                        // as written)
                        sound = None;
                        fall_through = true;
                    }
                }
                if fall_through {
                    if let Some(slot) = info.and_then(|i| i.weapon_slot) {
                        match slot {
                            PRIMARY_WEAPON => sound = per_unit(h, obj, b"VoicePrimaryWeaponMode"),
                            SECONDARY_WEAPON => sound = per_unit(h, obj, b"VoiceSecondaryWeaponMode"),
                            TERTIARY_WEAPON => sound = per_unit(h, obj, b"VoiceTertiaryWeaponMode"),
                            _ => {}
                        }
                        object_with_sound = Some(obj);
                        skip = true;
                    }
                }
            }
            MSG_DO_FORCE_ATTACK_OBJECT | MSG_DO_ATTACK_OBJECT | MSG_DO_WEAPON_AT_OBJECT => {
                if sound.is_none() {
                    // Low priority sounds -- only do this if uninitialized.
                    sound = if info.is_some_and(|i| i.air) {
                        templ(h, obj, ThingTemplateAudioType::VoiceAttackAir)
                    } else {
                        templ(h, obj, ThingTemplateAudioType::VoiceAttack)
                    };
                    object_with_sound = Some(obj);
                }
                // Look for a specialty weapon fired via command button!
                let specialty_weapon = msg == MSG_DO_WEAPON_AT_OBJECT;
                let weapon = h.voice_weapon(obj, info.and_then(|i| i.weapon_slot));
                if let Some(w) = weapon {
                    match w.damage_type {
                        // (kept without ALLOW_SURRENDER: "flashbangs still use 'em")
                        DAMAGE_SURRENDER => {
                            if target.is_some_and(|t| h.is_kind_of(t, KINDOF_STRUCTURE)) {
                                // We are attempting to take over a building with rangers and flashbangs!
                                sound = per_unit(h, obj, b"VoiceClearBuilding");
                            } else {
                                // Special for subdue attacks.
                                sound = per_unit(h, obj, b"VoiceSubdue");
                            }
                            object_with_sound = Some(obj);
                            skip = true;
                        }
                        DAMAGE_DISARM => {
                            // Special for mine clearing attacks.
                            sound = per_unit(h, obj, b"VoiceDisarm");
                            object_with_sound = Some(obj);
                            skip = true;
                        }
                        DAMAGE_KILLPILOT if specialty_weapon => {
                            // Special for sniping vehicle pilots.
                            sound = per_unit(h, obj, b"VoiceSnipePilot");
                            object_with_sound = Some(obj);
                            skip = true;
                        }
                        DAMAGE_MELEE if specialty_weapon => {
                            // Special for stabbing
                            sound = per_unit(h, obj, b"VoiceMelee");
                            object_with_sound = Some(obj);
                            skip = true;
                        }
                        _ => {}
                    }
                }
            }
            MSG_DO_WEAPON_AT_LOCATION => {
                if sound.is_none() {
                    // Low priority sounds -- only do this if uninitialized.
                    sound = if info.is_some_and(|i| i.air) {
                        templ(h, obj, ThingTemplateAudioType::VoiceAttackAir)
                    } else {
                        templ(h, obj, ThingTemplateAudioType::VoiceAttack)
                    };
                    object_with_sound = Some(obj);
                }
                // Check for possibility of a higher priority sound!
                if let Some(w) = h.voice_weapon(obj, info.and_then(|i| i.weapon_slot)) {
                    match w.damage_type {
                        DAMAGE_SURRENDER => {}
                        DAMAGE_DISARM => {
                            // Special for mine clearing attacks.
                            sound = per_unit(h, obj, b"VoiceDisarm");
                            object_with_sound = Some(obj);
                        }
                        // "specific to the guicommand based ground attacks, the toxin sprinkler and the firestorm wall
                        // thing hence the additional check for it being a non-primary weapon"
                        DAMAGE_FLAME => {
                            if w.slot != PRIMARY_WEAPON {
                                sound = per_unit(h, obj, b"VoiceFlameLocation");
                                object_with_sound = Some(obj);
                            }
                        }
                        DAMAGE_POISON => {
                            if w.slot != PRIMARY_WEAPON {
                                sound = per_unit(h, obj, b"VoicePoisonLocation");
                                object_with_sound = Some(obj);
                            }
                        }
                        _ => {
                            if w.name == b"ComancheRocketPodWeapon" {
                                // Special case for comanche rocket pods.
                                sound = per_unit(h, obj, b"VoiceFireRocketPods");
                                object_with_sound = Some(obj);
                                skip = true;
                            }
                        }
                    }
                }
            }
            MSG_DO_GUARD_POSITION | MSG_DO_GUARD_OBJECT => {
                sound = templ(h, obj, ThingTemplateAudioType::VoiceGuard);
                object_with_sound = Some(obj);
                skip = true;
            }
            MSG_DO_SPECIAL_POWER | MSG_DO_SPECIAL_POWER_AT_LOCATION | MSG_DO_SPECIAL_POWER_AT_OBJECT => {
                if let Some(sp) = info.map(|i| i.special_power_type).filter(|&t| t != SPECIAL_INVALID) {
                    if let Some(name) = h.special_power_initiate_sound(obj, sp) {
                        object_with_sound = Some(obj);
                        sound = Some((name, VoiceSource::InitiateSound(sp)));
                        skip = true;
                    }
                }
            }
            MSG_INTERNET_HACK => {
                object_with_sound = Some(obj);
                sound = per_unit(h, obj, b"VoiceHackInternet");
                skip = true;
            }
            // "Requested to add voice of message type %d, but don't know how" (a debug log)
            _ => {}
        }
        if skip {
            // "The unit sound doesn't have any sort of priority or special case, so simply play the first one that comes
            // along."
            break;
        }
    }

    let Some((name, source)) = sound else { return };
    // "to prevent voice stepping."
    let Some(obj) = object_with_sound else { return };
    let template = h.voice_template_name(obj);
    h.play_unit_voice(UnitVoice { name, object: obj, template: template.clone(), source });
    if h.is_car_bomb(obj) {
        // Additional sounds for terrorists in cars.
        let which = match msg {
            MSG_DO_FORCE_ATTACK_GROUND | MSG_DO_FORCE_ATTACK_OBJECT | MSG_DO_ATTACK_OBJECT => Some(TerroristVoice::Attack),
            MSG_DO_MOVETO | MSG_DO_ATTACKMOVETO | MSG_GET_REPAIRED | MSG_GET_HEALED | MSG_DO_SALVAGE => Some(TerroristVoice::Move),
            t if (MSG_SELECT_TEAM0.0..=MSG_SELECT_TEAM9.0).contains(&t.0) || t == MSG_CREATE_SELECTED_GROUP => Some(TerroristVoice::Select),
            _ => None,
        };
        if let Some(w) = which {
            let name = h.terrorist_voice(w);
            h.play_unit_voice(UnitVoice { name, object: obj, template, source: VoiceSource::Misc });
        }
    }
}

/// `translateGameMessage`'s `MSG_CREATE_SELECTED_GROUP` / `MSG_SELECT_TEAMn` case: "weed out unit responses for things we
/// don't own", then the selection's voice.
pub fn selected_group_voice<H: CommandXlatHost>(msg: MessageType, ui: &InGameUiSelection, h: &mut H) {
    if !(msg == MSG_CREATE_SELECTED_GROUP || (MSG_SELECT_TEAM0.0..=MSG_SELECT_TEAM9.0).contains(&msg.0)) {
        return;
    }
    let list: Vec<DrawableId> =
        ui.all_selected_drawables().iter().copied().filter(|&d| h.drawable_object(d).is_some_and(|o| h.is_locally_controlled(o))).collect();
    if !list.is_empty() {
        pick_and_play_unit_voice_response(&list, MSG_CREATE_SELECTED_GROUP, None, ui, h);
    }
}
