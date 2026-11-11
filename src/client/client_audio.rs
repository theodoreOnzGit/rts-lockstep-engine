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
//! - ~~**`ObjectSound::VoiceDefect`** (the object template's `VoiceDefect`), the map objects' customised ambient sounds
//!   (`DrawableAmbientSoundProperties`, upstream's `DynamicAudioEventInfo`) and the drawables' ambient sounds are not
//!   played yet;~~ **Corrected 2026-11-06 (#51):** `VoiceDefect` is played from the object's template
//!   ([`AudioHost::object_template_audio`]); the unit voice responses are played ([`ClientAudio::play_unit_voice`]); the
//!   ambient sounds: see [`crate::client::drawable_ambient`]; ~~nor are the script actions' sounds and music, which the logic does not emit (its `sa_audio_*` hooks are
//!   cut points).~~ **Corrected 2026-11-06 (#51):** the script actions' calls on `TheAudio` are now client events
//!   ([`ScriptAudioEvent`]) and are made here ([`ClientAudio::apply_script_event`]); `pauseAmbient` does nothing, as
//!   `MilesAudioManager::pauseAmbient` is empty.
//! - ~~**`getAudioLengthMS`** is not answered to the logic (the script engine's speech and sound waits read 0): the file
//!   lengths would change the logic (#51's write-up).~~ **Corrected 2026-11-06 (#51):** since #52 the logic reads the
//!   lengths from the files at load (`logic::audio_length`).
//! - **`ObjectClientEvent::AudioRemoveEvent { handle }`** carries a handle the logic made up (upstream's comes from
//!   `addAudioEvent`, which the logic cannot call): ignored.
//! - ~~**`StealthDetectorFeedback`** plays its sound whether or not the radar would have taken the event (`tryEvent`); the
//!   radar is not translated.~~ **Corrected 2026-11-06 (#35):** the radar is translated (`client::radar`); the client
//!   leaves out the feedback events whose `tryEvent` refused ([`crate::client::radar::RadarFrame::refused`]) before they
//!   reach the audio and EVA, and the radar's own sounds and EVA are played by [`ClientAudio::apply_radar_feedback`]
//!   and [`ClientAudio::play_radar_event_sound`].
//! - **`TheInGameUI->isClientQuiet()`** (#117) has one reader upstream, `Player::okToPlayRadarEdgeSound` (Player.cpp:3150:
//!   not single-player defeated, not dead, **not quiet**, inside the logic's update, past frame 0), which decides whether
//!   `addRadar`, `removeRadar`, `disableRadar` and `enableRadar` play the radar's online or offline sound. The flag is
//!   the client's (the quit menu sets it when the player leaves, `GameLogic::startNewGame` clears it), so the logic
//!   evaluates the other four terms (its own stand-in for the flag, `PlayerGlobals::client_quiet`, is always false) and
//!   the client applies the fifth here: a `RadarSound` that arrives while [`ClientAudio::client_quiet`] is set is not
//!   played. The terms have no side effects, so the split changes nothing but when the flag is read: when the client
//!   takes the frame's events, after the logic frame, rather than inside it.

use crate::client::audio_event::{AudioEventHost, AudioEventRts, AudioHandle};
use crate::client::audio_info::{AudioTables, MiscAudio};
use crate::client::fx_list::FxRequest;
use crate::client::game_audio::{AudioDevice, AudioHost, AudioManager};
use crate::common::audio_event_name::AudioEventName;
use crate::common::base_type::Coord3D;
use crate::common::game_common::{CellShroudStatus, Relationship};
use crate::common::game_type::{ObjectId, LAYER_GROUND};
use crate::common::thing_template::ThingTemplateAudioType;
use crate::client::unit_voice::{UnitVoice, VoiceSource};
use crate::client::drawable_ambient::AmbientSounds;
use crate::client::eva::{Eva, EvaHost, SuperweaponTimers, EVA_INSUFFICIENT_FUNDS};
use crate::logic::world::game_logic_dispatch::ClientCall;
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
    fn object_template_audio(&self, id: u32, t: ThingTemplateAudioType) -> Vec<u8> {
        self.world.find_object(id as ObjectId).map_or_else(Vec::new, |o| o.template().get_audio(t).event_name.clone())
    }
    /// The body read in place (`World::peek_damage_state`: no module is called).
    fn object_damage_state(&self, id: u32) -> i32 {
        self.world.peek_damage_state(id as ObjectId)
    }
    fn time_of_day(&self) -> i32 {
        self.world.statics.global_data.the_global_data().map_or(crate::common::game_type::TimeOfDay::Afternoon as i32, |g| g.time_of_day)
    }
}

/// #55: EVA's questions, read from the World.
impl EvaHost for WorldAudioHost<'_> {
    fn eva_frame(&self) -> u32 {
        self.world.frame()
    }
    fn eva_has_sufficient_power(&self, player: i32) -> bool {
        let now = self.world.frame();
        self.world.players.get_nth(player).is_some_and(|p| p.energy.has_sufficient_power(now))
    }
    fn eva_player_side(&self, player: i32) -> Vec<u8> {
        self.world.players.get_nth(player).map(|p| p.side.clone()).unwrap_or_default()
    }
    fn eva_superweapon_ready(&self, object: ObjectId, template: u32) -> Option<Option<bool>> {
        use crate::common::object_status_types::OBJECT_STATUS_UNDER_CONSTRUCTION;
        use crate::logic::modules::special_power_body::{peek_ready_and_percent, peek_special_power_module};
        let o = self.world.find_object(object)?;
        if o.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION) {
            return None;
        }
        // (`isReady()` read without starting a shared timer: `peek_ready_and_percent`)
        Some(peek_special_power_module(self.world, object, template).and_then(|(me, p)| peek_ready_and_percent(p, self.world, me).ok()).map(|(ready, _)| ready))
    }
    fn eva_special_power_type(&self, template: u32) -> Option<crate::common::special_power_type::SpecialPowerType> {
        self.world.statics.special_power_store.find_special_power_template_by_id(template).map(|t| t.special_power_type())
    }
    fn eva_hidden_by_science(&self, player: i32, template: u32) -> bool {
        use crate::common::science::SCIENCE_INVALID;
        let Some(t) = self.world.statics.special_power_store.find_special_power_template_by_id(template) else { return false };
        let s = t.required_science();
        s != SCIENCE_INVALID && !self.world.players.get_nth(player).is_some_and(|p| p.sciences.contains(&s))
    }
    fn eva_controlling_player(&self, object: ObjectId) -> Option<i32> {
        self.world.get_controlling_player(object)
    }
    fn eva_relationship(&self, player: i32, object: ObjectId) -> Relationship {
        self.world.tq().player_relationship_to_team(player, self.world.get_team(object))
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
    /// #51: the file index each template event last played (`setPlayingAudioIndex` on the template's event, which the client
    /// cannot write: the template is the logic's), by template name and sound.
    pub voice_indices: BTreeMap<(Vec<u8>, VoiceSource), i32>,
    /// #51: the drawables' ambient sounds.
    pub ambient: AmbientSounds,
    /// #51: how many of the logic's INI audio events ([`AudioTables::logic_ini_events`]) are applied to the manager's tables.
    pub logic_ini_applied: usize,
    /// #55: `TheEva`, played through [`ClientAudio::manager`].
    pub eva: Eva,
    /// #55: `InGameUI`'s superweapon timers, for their "ready" EVA.
    pub superweapons: SuperweaponTimers,
    /// #55: how many of the client's `EVA_InsufficientFunds` requests (`WorldClient::eva_insufficient_funds`) EVA has taken.
    pub eva_insufficient_funds_taken: u32,
    /// #117: `TheInGameUI->isClientQuiet()` (`m_clientQuiet`, the quit menu's: `crate::client::quit_menu::QuitMenu::client_quiet`),
    /// which the owner copies here before the frame's events; it silences the radar edge sounds (see the module docs).
    pub client_quiet: bool,
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
        ClientAudio {
            manager,
            firing_loops: BTreeMap::new(),
            log: Vec::new(),
            voice_indices: BTreeMap::new(),
            ambient: AmbientSounds::default(),
            logic_ini_applied: 0,
            eva: Eva::new(),
            superweapons: SuperweaponTimers::default(),
            eva_insufficient_funds_taken: 0,
            client_quiet: false,
        }
    }

    fn add(&mut self, host: &dyn AudioHost, e: AudioEventRts) -> AudioHandle {
        self.add_index(host, e).0
    }

    /// [`Self::add`], with the file index the caller's event was given (`getPlayingAudioIndex()` after the call).
    fn add_index(&mut self, host: &dyn AudioHost, mut e: AudioEventRts) -> (AudioHandle, i32) {
        let h = self.manager.add_audio_event(host, &mut e);
        let index = e.playing_audio_index;
        self.log.push((e.event_name, h));
        if self.log.len() > 64 {
            self.log.remove(0);
        }
        (h, index)
    }

    /// #51: one of `pickAndPlayUnitVoiceResponse`'s plays ([`crate::client::unit_voice`]): `AudioEventRTS soundToPlay =
    /// *soundToPlayPtr; soundToPlay.setObjectID(id); TheAudio->addAudioEvent(&soundToPlay);` then, for a template's or a
    /// module's event, `setPlayingAudioIndex(soundToPlay.getPlayingAudioIndex())` on it, kept here.
    pub fn play_unit_voice(&mut self, host: &dyn AudioHost, v: &UnitVoice) {
        let mut e = AudioEventRts::new(&v.name);
        let key = (v.source != VoiceSource::Misc).then(|| (v.template.clone(), v.source.clone()));
        if let Some(i) = key.as_ref().and_then(|k| self.voice_indices.get(k)) {
            e.playing_audio_index = *i;
        }
        e.set_object_id(v.object as u32);
        let (_, index) = self.add_index(host, e);
        if let Some(k) = key {
            self.voice_indices.insert(k, index);
        }
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
            // #117: `Player::okToPlayRadarEdgeSound`'s `!TheInGameUI->isClientQuiet()`, the one term the logic cannot read
            ClientEvent::Player(PlayerClientEvent::RadarSound { online, player_index }) if !self.client_quiet => {
                let n = if *online { &misc.radar_online_sound } else { &misc.radar_offline_sound };
                self.add(host, Self::named(&n.event_name, 0, None, Some(*player_index)));
            }
            ClientEvent::Object(o) => self.apply_object_event(host, device, &misc, o),
            ClientEvent::ScriptAudio(s) => self.apply_script_event(host, device, s),
            // #51: the drawables' ambient sounds
            ClientEvent::ObjectCreated { object_id, .. } => self.ambient.create(*object_id, &mut self.manager, device, host),
            ClientEvent::ObjectDestroyed { object_id } => self.ambient.destroy(*object_id, &mut self.manager, device),
            ClientEvent::LoadingMap(on) => self.ambient.loading_map = *on,
            ClientEvent::DrawablesOnLevelStart => self.ambient.on_level_start(&mut self.manager, device, host),
            // #56: the dispatcher's `AudioEventRTS sound(name); sound.setPosition(&pos); sound.setPlayerIndex(i);
            // sound.setObjectID(id); TheAudio->addAudioEvent(&sound);` (the rally point's, the beacon's, the placement's)
            ClientEvent::Dispatch(ClientCall::Audio { name, pos, player, object }) => {
                self.add(host, Self::named(name.as_bytes(), object.map_or(0, |o| o as ObjectId), pos.as_ref(), *player));
            }
            _ => {}
        }
    }

    /// GitHub #51: a script's call on `TheAudio` (ScriptActions.cpp; `ScriptEngine::loadPostProcess`'s music track), made
    /// here as upstream's action makes it.
    pub fn apply_script_event<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D, s: &ScriptAudioEvent) {
        match s {
            ScriptAudioEvent::AddEvent(a, picks) => {
                // `AudioEventRTS(name)`, `(name, pos)` or `(name, objectID)`, then the action's setters
                let mut e = AudioEventRts::new(&a.name);
                if let Some(p) = &a.position {
                    e.set_position(p);
                }
                if a.object != 0 {
                    e.set_object_id(a.object as u32);
                }
                // `setIsLogicalAudio(true)`: ~~the file is picked from the client's own copy of the logic stream (#50)~~
                // **corrected 2026-11-06 (#50):** the logic made the event's `GameLogicRandomValue` draws; their values
                // replace the draws here, so this machine plays the files every machine's logic picked
                e.is_logical_audio = a.logical;
                e.logic_picks = *picks;
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
            // #51: `Object::defect`: `AudioEventRTS voiceDefect = *getTemplate()->getVoiceDefect(); voiceDefect.setObjectID(getID());
            // TheAudio->addAudioEvent(&voiceDefect);` (a copy: nothing is written back)
            ObjectClientEvent::Sound { sound: ObjectSound::VoiceDefect, object_id, .. } => {
                let name = host.object_template_audio(*object_id as u32, ThingTemplateAudioType::VoiceDefect);
                self.add(host, Self::named(&name, *object_id, None, None));
            }
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
            // #51: the drawable's ambient sound calls
            ObjectClientEvent::DrawableReactToBodyDamageStateChange { object_id, state } => {
                self.ambient.react_to_body_damage_state_change(*object_id, *state, &mut self.manager, device, host)
            }
            ObjectClientEvent::DrawableAmbientSoundProperties { object_id, properties } => {
                self.ambient.apply_map_properties(*object_id, properties, &mut self.manager, device, host)
            }
            ObjectClientEvent::DrawableStopAmbientSound { object_id } => self.ambient.stop_ambient_sound(*object_id, &mut self.manager, device),
            ObjectClientEvent::DrawableEnableAmbientSound { object_id, enable } => {
                self.ambient.enable_ambient_sound(*object_id, *enable, &mut self.manager, device, host)
            }
            ObjectClientEvent::DrawableEnableAmbientSoundFromScript { object_id, enable } => {
                self.ambient.enable_ambient_sound_from_script(*object_id, *enable, &mut self.manager, device, host)
            }
            ObjectClientEvent::FiringTrackerStopLoopingSound { object_id } => {
                if let Some(h) = self.firing_loops.remove(object_id) {
                    self.manager.remove_audio_event(device, h);
                }
            }
            _ => {}
        }
    }

    /// #51: the audio events a map's `map.ini` defined or redefined, read by the logic (`GameStatics::audio`, the logic's
    /// load of the INI through `theTypeTable`), applied to the manager's tables as the logic parsed them: the description
    /// copied (upstream's one `TheAudio` holds both), and the name added to the music track list where the logic's parse
    /// added it. Each machine's logic reads the same `map.ini`, so every machine's client gets the same descriptions.
    pub fn sync_logic_ini_audio(&mut self, logic: &AudioTables) {
        for (name, added_track) in logic.logic_ini_events.iter().skip(self.logic_ini_applied) {
            if let Some(info) = logic.find_audio_event_info(name) {
                if self.manager.tables.infos.contains_key(name) {
                    self.manager.tables.redefined.push(name.clone());
                }
                self.manager.tables.infos.insert(name.clone(), info.clone());
            }
            if *added_track {
                self.manager.tables.add_track_name(name);
            }
        }
        self.logic_ini_applied = logic.logic_ini_events.len();
    }

    /// #51: `Drawable::update`'s ambient sound restart, for every drawable; once per logic frame, after its events.
    pub fn update_ambient<D: AudioDevice>(&mut self, host: &dyn AudioHost, device: &mut D) {
        self.ambient.update(&mut self.manager, device, host);
    }

    /// #55: EVA's frame, once per logic frame after its events: the logic's `TheEva` and `addSuperweapon` calls among
    /// `events`, the client's insufficient funds requests (`insufficient_funds`, the running count of
    /// `WorldClient::eva_insufficient_funds`), the superweapon timers' ready check, then `Eva::update`. Reads the World only.
    pub fn update_eva(&mut self, host: &impl EvaHost, events: &[ClientEvent], insufficient_funds: u32) {
        for e in events {
            self.eva.take_event(e);
            self.superweapons.take_event(host, e);
        }
        if insufficient_funds != self.eva_insufficient_funds_taken {
            self.eva.set_should_play(EVA_INSUFFICIENT_FUNDS);
            self.eva_insufficient_funds_taken = insufficient_funds;
        }
        self.superweapons.update(host, &mut self.eva);
        self.eva.update(host, &mut self.manager);
    }

    /// #35: the radar's feedback (`Radar::tryUnderAttackEvent`, `tryInfiltrationEvent`): `MiscAudio`'s radar sound with
    /// `setPlayerIndex(localPlayer)`, and `TheEva->setShouldPlay(EVA_BaseUnderAttack / EVA_AllyUnderAttack)`.
    pub fn apply_radar_feedback(&mut self, host: &dyn AudioHost, feedback: &[crate::client::radar::RadarFeedback]) {
        use crate::client::radar::{RadarFeedback, RadarSound};
        let misc = self.manager.tables.misc.clone();
        for f in feedback {
            match f {
                RadarFeedback::Sound { sound, player_index } => {
                    let n = match sound {
                        RadarSound::HarvesterUnderAttack => &misc.radar_harvester_under_attack_sound,
                        RadarSound::StructureUnderAttack => &misc.radar_structure_under_attack_sound,
                        RadarSound::Infiltration => &misc.radar_infiltration_sound,
                    };
                    self.add(host, Self::named(&n.event_name, 0, None, *player_index));
                }
                RadarFeedback::Eva(name) => self.eva.set_should_play(crate::client::eva::name_to_message(name.as_bytes())),
                RadarFeedback::AttackGlow | RadarFeedback::Message(_) => {}
            }
        }
    }

    /// #35: `W3DRadar::drawEvents`'s `static AudioEventRTS eventSound("RadarEvent"); TheAudio->addAudioEvent(&eventSound);`,
    /// `count` times.
    pub fn play_radar_event_sound(&mut self, host: &dyn AudioHost, count: u32) {
        for _ in 0..count {
            self.add(host, Self::named(b"RadarEvent", 0, None, None));
        }
    }

    /// An FX list's sound (`SoundFXNugget`): at the position, if it has one.
    pub fn fx_request(&mut self, host: &dyn AudioHost, r: &FxRequest) {
        if let FxRequest::Sound { name, position } = r {
            self.add(host, Self::named(name, 0, position.as_ref(), None));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::audio_info::AudioTables;
    use crate::client::game_audio::tests::LogDevice;
    use crate::client::game_audio::AudioManager;

    /// #117, predicted from Player.cpp's `okToPlayRadarEdgeSound` before the run: the radar's online sound the logic asks
    /// for is played (`addAudioEvent` logs it) while the client is not quiet, and not at all while it is (the quit
    /// menu's `setClientQuiet(TRUE)`); another sound (the money's) plays either way.
    ///
    /// Results (2026-11-11): held.
    #[test]
    fn the_radar_edge_sounds_are_silent_while_the_client_is_quiet() {
        const INI: &[u8] = b"AudioEvent RadarOn\n Sounds = on\n Type = ui everyone\nEnd\nAudioEvent Cash\n Sounds = cash\n Type = ui everyone\nEnd\n\
MiscAudio\n RadarNotifyOnlineSound = RadarOn\n MoneyDepositSound = Cash\nEnd\n";
        let w = crate::logic::world::gate_scenario::gate_map_world();
        let mut t = AudioTables::new();
        t.load_text(b"a.ini", INI).expect("the audio INI");
        let mut audio = ClientAudio::new(AudioManager::new(t));
        let mut dev = LogDevice::default();
        let files = |_: &[u8]| true;
        let host = WorldAudioHost { world: &w, local_player_index: None, files: &files, language: b"English" };
        let radar = ClientEvent::Player(PlayerClientEvent::RadarSound { online: true, player_index: 0 });
        let money = ClientEvent::MoneySound { sound: MoneySound::Deposit, player_index: 0 };
        let names = |a: &ClientAudio| a.log.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
        audio.apply_event(&host, &mut dev, &radar);
        assert_eq!(names(&audio), [b"RadarOn".to_vec()]);
        audio.client_quiet = true;
        audio.apply_event(&host, &mut dev, &radar);
        audio.apply_event(&host, &mut dev, &money);
        assert_eq!(names(&audio), [b"RadarOn".to_vec(), b"Cash".to_vec()]);
        audio.client_quiet = false;
        audio.apply_event(&host, &mut dev, &radar);
        assert_eq!(names(&audio).len(), 3);
    }
}
