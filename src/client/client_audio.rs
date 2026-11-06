// SPDX-License-Identifier: GPL-3.0-only
//
// Translation-owned file: the client's side of the logic's audio calls (GitHub #13, C6). Each of the logic's client
// events that upstream makes with `TheAudio->addAudioEvent` / `removeAudioEvent` is turned back into that call here, with
// the `AudioEventRTS` setters the call site used (the events name them). The call sites translated are the logic's
// (see each event's doc comment in `logic::client_event`, `logic::world::object_events`, `logic::world::player_ops`)
// and FXList.cpp's SoundFXNugget::doFXPos/doFXObj (`client::fx_list::FxRequest::Sound`):
//   `AudioEventRTS sound(name); sound.setPosition(pos); sound.setPlayerIndex(i); TheAudio->addAudioEvent(&sound);`
//
// Original copyright notice of the files those call sites are in:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.

//! The logic's sounds, played by the client's [`AudioManager`].
//!
//! # Where this differs from upstream
//!
//! - **The listener is the local player** (`ThePlayerList->getLocalPlayer()`); upstream switches to the observed player
//!   when the local player is no longer active (`TheControlBar->getObserverLookAtPlayer()`), which the client's control
//!   bar does not track yet.
//! - **`ObjectSound::VoiceDefect`** (the object template's `VoiceDefect`), the map objects' customised ambient sounds
//!   (`DrawableAmbientSoundProperties`, upstream's `DynamicAudioEventInfo`) and the drawables' ambient sounds are not
//!   played yet; ~~nor are the script actions' sounds and music, which the logic does not emit (its `sa_audio_*` hooks are
//!   cut points).~~ **Corrected 2026-11-06 (#51):** the script actions' calls on `TheAudio` are now client events
//!   ([`ScriptAudioEvent`]) and are made here ([`ClientAudio::apply_script_event`]); `pauseAmbient` does nothing, as
//!   `MilesAudioManager::pauseAmbient` is empty.
//! - **`getAudioLengthMS`** is not answered to the logic (the script engine's speech and sound waits read 0): the file
//!   lengths would change the logic (#51's write-up).
//! - **`ObjectClientEvent::AudioRemoveEvent { handle }`** carries a handle the logic made up (upstream's comes from
//!   `addAudioEvent`, which the logic cannot call): ignored.
//! - **`StealthDetectorFeedback`** plays its sound whether or not the radar would have taken the event (`tryEvent`); the
//!   radar is not translated.

use crate::client::audio_event::{AudioEventHost, AudioEventRts, AudioHandle};
use crate::client::audio_info::MiscAudio;
use crate::client::fx_list::FxRequest;
use crate::client::game_audio::{AudioDevice, AudioHost, AudioManager};
use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CellShroudStatus, Relationship};
use crate::common::game_type::{ObjectId, LAYER_GROUND};
use crate::logic::client_event::{ClientEvent, MoneySound, ScriptAudioEvent};
use crate::logic::world::game_logic::World;
use crate::logic::world::object_events::{ObjectClientEvent, ObjectSound, StealthFeedback};
use crate::logic::world::player_ops::PlayerClientEvent;
use std::collections::BTreeMap;

/// The World as the audio reads it (all `&`).
pub struct WorldAudioHost<'a> {
    pub world: &'a World,
    pub local_player_index: Option<i32>,
    /// `TheFileSystem->doesFileExist`.
    pub files: &'a dyn Fn(&[u8]) -> bool,
    /// `GetRegistryLanguage()`.
    pub language: &'a [u8],
}

impl AudioEventHost for WorldAudioHost<'_> {
    fn does_file_exist(&self, path: &[u8]) -> bool {
        (self.files)(path)
    }
    fn registry_language(&self) -> Vec<u8> {
        self.language.to_vec()
    }
    fn object_position(&self, id: u32) -> Option<Coord3D> {
        self.world.find_object(id as ObjectId).map(|o| *o.thing.position())
    }
    /// The client's drawables are its objects' (`ClientDrawables` is keyed by object id).
    fn drawable_position(&self, id: u32) -> Option<Coord3D> {
        self.object_position(id)
    }
    fn object_player_index(&self, id: u32) -> Option<i32> {
        self.world.get_controlling_player(id as ObjectId)
    }
    fn drawable_player_index(&self, id: u32) -> Option<i32> {
        self.object_player_index(id)
    }
}

impl AudioHost for WorldAudioHost<'_> {
    fn local_player_index(&self) -> Option<i32> {
        self.local_player_index.or_else(|| self.world.players.local_index())
    }
    fn listening_player_index(&self) -> Option<i32> {
        self.local_player_index()
    }
    fn player_exists(&self, index: i32) -> bool {
        self.world.players.get_nth(index).is_some()
    }
    fn player_has_default_team(&self, index: i32) -> bool {
        self.world.players.get_nth(index).is_some_and(|p| p.default_team.is_some())
    }
    fn are_allies(&self, owner: i32, local: i32) -> Option<bool> {
        match self.world.tq().player_relationship_to_player_default_team(owner, local) {
            Relationship::Allies => Some(true),
            Relationship::Enemies => Some(false),
            Relationship::Neutral => None,
        }
    }
    fn shroud_clear(&self, player: i32, pos: &Coord3D) -> bool {
        self.world.partition.get_shroud_status_for_player(player, pos) == CellShroudStatus::Clear
    }
    fn ground_height(&self, x: f32, y: f32) -> f32 {
        self.world.peek_layer_height_and_normal(x, y, LAYER_GROUND).0
    }
}

/// The client's audio: the manager, and the handles of the firing trackers' looping sounds by object.
#[derive(Clone, Debug)]
pub struct ClientAudio {
    pub manager: AudioManager,
    /// `FiringTracker::m_audioHandle`, kept here by object (the logic keeps none).
    pub firing_loops: BTreeMap<ObjectId, AudioHandle>,
    /// The events played (name and the handle `addAudioEvent` returned), most recent last; for checks (the last 64).
    pub log: Vec<(Vec<u8>, AudioHandle)>,
}

/// `MiscAudio`'s sound for an [`ObjectSound`] (`None`: not a misc sound).
pub fn misc_sound(m: &MiscAudio, s: ObjectSound) -> Option<&AudioEventName> {
    Some(match s {
        ObjectSound::SplatterVehiclePilotsBrain => &m.splatter_vehicle_pilots_brain,
        ObjectSound::BuildingDisabled => &m.building_disabled,
        ObjectSound::VehicleDisabled => &m.vehicle_disabled,
        ObjectSound::BuildingReenabled => &m.building_reenabled,
        ObjectSound::VehicleReenabled => &m.vehicle_reenabled,
        ObjectSound::UnitPromoted => &m.unit_promoted,
        ObjectSound::DefectorTimerTick => &m.defector_timer_tick_sound,
        ObjectSound::VoiceDefect => return None,
        ObjectSound::CrateHeal => &m.crate_heal,
        ObjectSound::RepairSparks => &m.repair_sparks,
        ObjectSound::CrateShroud => &m.crate_shroud,
        ObjectSound::CrateMoney => &m.crate_money,
        ObjectSound::CrateFreeUnit => &m.crate_free_unit,
        ObjectSound::CrateSalvage => &m.crate_salvage,
        ObjectSound::DefectorTimerDing => &m.defector_timer_ding_sound,
        ObjectSound::SabotageResetTimerBuilding => &m.sabotage_reset_timer_building,
        ObjectSound::MoneyWithdraw => &m.money_withdraw_sound,
        ObjectSound::SabotageShutDownBuilding => &m.sabotage_shut_down_building,
    })
}

impl ClientAudio {
    pub fn new(manager: AudioManager) -> Self {
        ClientAudio { manager, firing_loops: BTreeMap::new(), log: Vec::new() }
    }

    fn add(&mut self, host: &dyn AudioHost, mut e: AudioEventRts) -> AudioHandle {
        let h = self.manager.add_audio_event(host, &mut e);
        self.log.push((e.event_name, h));
        if self.log.len() > 64 {
            self.log.remove(0);
        }
        h
    }

    /// An event of a name, owned by an object (`INVALID_ID`: none) or placed, with a player index.
    fn named(name: &[u8], object: ObjectId, position: Option<&Coord3D>, player: Option<i32>) -> AudioEventRts {
        let mut e = AudioEventRts::new(name);
        if object != 0 {
            e.set_object_id(object as u32);
        }
        if let Some(p) = position {
            e.set_position(p);
        }
        if let Some(p) = player {
            e.player_index = p;
        }
        e
    }

    /// One of the logic's client events, if it is a sound's (see the module docs).
    pub fn apply_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, ev: &ClientEvent) {
        let misc = self.manager.tables.misc.clone();
        match ev {
            ClientEvent::MoneySound { sound, player_index } => {
                let n = match sound {
                    MoneySound::Deposit => &misc.money_deposit_sound,
                    MoneySound::Withdraw => &misc.money_withdraw_sound,
                };
                self.add(host, Self::named(&n.event_name, 0, None, Some(*player_index)));
            }
            ClientEvent::Audio { event_name, object_id, player_index } => {
                self.add(host, Self::named(event_name, *object_id, None, Some(*player_index)));
            }
            ClientEvent::Player(PlayerClientEvent::RadarSound { online, player_index }) => {
                let n = if *online { &misc.radar_online_sound } else { &misc.radar_offline_sound };
                self.add(host, Self::named(&n.event_name, 0, None, Some(*player_index)));
            }
            ClientEvent::Object(o) => self.apply_object_event(host, device, &misc, o),
            ClientEvent::ScriptAudio(s) => self.apply_script_event(host, device, s),
            _ => {}
        }
    }

    /// GitHub #51: a script's call on `TheAudio` (ScriptActions.cpp; `ScriptEngine::loadPostProcess`'s music track), made
    /// here as upstream's action makes it.
    pub fn apply_script_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, s: &ScriptAudioEvent) {
        match s {
            ScriptAudioEvent::AddEvent(a) => {
                // `AudioEventRTS(name)`, `(name, pos)` or `(name, objectID)`, then the action's setters
                let mut e = AudioEventRts::new(&a.name);
                if let Some(p) = &a.position {
                    e.set_position(p);
                }
                if a.object != 0 {
                    e.set_object_id(a.object as u32);
                }
                // `setIsLogicalAudio(true)`: the file is picked from the client's own copy of the logic stream (#50)
                e.is_logical_audio = a.logical;
                e.player_index = a.player_index;
                e.uninterruptable = a.uninterruptable;
                e.should_fade = a.should_fade;
                self.add(host, e);
            }
            ScriptAudioEvent::RemoveEventHandle(h) => self.manager.remove_audio_event(device, *h as AudioHandle),
            // `MilesAudioManager::pauseAmbient` is empty: nothing happens upstream
            ScriptAudioEvent::PauseAmbient(_) => {}
            ScriptAudioEvent::SetVolume { volume, affect } => self.manager.set_volume(*volume, *affect as u32),
            ScriptAudioEvent::SetEventEnabled { name, enable } => self.manager.set_audio_event_enabled(name, *enable),
            ScriptAudioEvent::RemoveDisabledEvents => self.manager.remove_disabled_events(device),
            ScriptAudioEvent::RemoveEventNamed(name) => self.manager.remove_audio_event_named(device, name),
            ScriptAudioEvent::SetEventVolumeOverride { name, volume } => self.manager.set_audio_event_volume_override(name, *volume),
            ScriptAudioEvent::Pause { pause: true, affect } => self.manager.pause_audio(device, *affect as u32),
            ScriptAudioEvent::Pause { pause: false, affect } => self.manager.resume_audio(device, *affect as u32),
        }
    }

    fn apply_object_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, misc: &MiscAudio, o: &ObjectClientEvent) {
        match o {
            ObjectClientEvent::Sound { sound, object_id, position } => {
                if let Some(n) = misc_sound(misc, *sound) {
                    let obj = if position.is_some() { 0 } else { *object_id };
                    self.add(host, Self::named(&n.event_name, obj, position.as_ref(), None));
                }
            }
            ObjectClientEvent::AudioAddEvent { name, object_id, position, player_index } => {
                self.add(host, Self::named(name, *object_id, position.as_ref(), *player_index));
            }
            ObjectClientEvent::AudioAddEventVolume { name, object_id, position, player_index, volume } => {
                let mut e = Self::named(name, *object_id, position.as_ref(), *player_index);
                e.volume = *volume;
                self.add(host, e);
            }
            ObjectClientEvent::AudioAddEventAtDrawable { name, drawable } => {
                let mut e = AudioEventRts::new(name);
                e.set_drawable_id(*drawable as u32);
                self.add(host, e);
            }
            ObjectClientEvent::StealthDetectorFeedback { feedback, position: _, player_index, .. } => {
                let n = match feedback {
                    StealthFeedback::Discovered => &misc.stealth_discovered_sound,
                    StealthFeedback::Neutralized => &misc.stealth_neutralized_sound,
                };
                self.add(host, Self::named(&n.event_name, 0, None, *player_index));
            }
            ObjectClientEvent::FiringTrackerStartLoopingSound { object_id, name, only_if_not_playing } => {
                if *only_if_not_playing && self.firing_loops.get(object_id).is_some_and(|h| self.manager.is_currently_playing(*h)) {
                    return;
                }
                let h = self.add(host, Self::named(name, *object_id, None, None));
                self.firing_loops.insert(*object_id, h);
            }
            ObjectClientEvent::FiringTrackerStopLoopingSound { object_id } => {
                if let Some(h) = self.firing_loops.remove(object_id) {
                    self.manager.remove_audio_event(device, h);
                }
            }
            _ => {}
        }
    }

    /// An FX list's sound (`SoundFXNugget`): at the position, if it has one.
    pub fn fx_request(&mut self, host: &dyn AudioHost, r: &FxRequest) {
        if let FxRequest::Sound { name, position } = r {
            self.add(host, Self::named(name, 0, position.as_ref(), None));
        }
    }
}
