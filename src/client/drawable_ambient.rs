// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Source/GameClient/Drawable.cpp (the ambient sound of Drawable::Drawable, ~Drawable,
//     reactToBodyDamageStateChange, update, onLevelStart, getAmbientSoundByDamage, getBaseSoundAmbientInfo,
//     mangleCustomAudioName, setCustomSoundAmbientOff, setCustomSoundAmbientInfo, clearCustomSoundAmbient,
//     startAmbientSound (both), stopAmbientSound, enableAmbientSound, enableAmbientSoundFromScript, getNoSoundMarker)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/DynamicAudioEventInfo.cpp (overrideAudioName and the override*
//     functions, applied to the copy of the description)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/GameAudio.cpp (AudioManager::addAudioEventInfo)
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! The drawables' ambient sounds (GitHub #51, part 3): the template's `SoundAmbient` (and `SoundAmbientDamaged`,
//! `SoundAmbientReallyDamaged`, `SoundAmbientRubble` by the body's damage state), or the map object's customised sound
//! (upstream's `DynamicAudioEventInfo`), started when the drawable is made or its damage state changes, kept playing while
//! it loops for ever, and switched by the logic (`enableAmbientSound`: the jets' engines; `enableAmbientSoundFromScript`:
//! the map's properties and `doEnableObjectSound`; `stopAmbientSound`).
//!
//! # Where this differs from upstream
//!
//! - **The state is kept by the client's audio**, by object id, not in the drawable (the client's drawables,
//!   `client::drawable`, carry no audio). Each drawable's id is its object's (as everywhere in the client).
//! - **The drawable's calls arrive as the logic's client events, after the frame**, in their order: the drawable made
//!   (`ObjectCreated`), destroyed, its damage state changed (`DrawableReactToBodyDamageStateChange`), the map's properties
//!   (`DrawableAmbientSoundProperties`, applied with [`apply_ambient_sound_properties`], the logic crate's translation of
//!   `updateObjValuesFromMapProperties`' sound block), the enable and stop calls, and `setLoadingMap` / `onLevelStart`.
//! - **`Drawable::update`'s restart** runs once per logic frame ([`AmbientSounds::update`]), not each client frame.
//! - **`setTimeOfDay`** (which restarts every drawable's ambient sound) is not called: nothing in the logic changes the time
//!   of day yet. The time of day an ambient event carries is the global data's; it picks no file (`SoundsNight` and the
//!   rest are never read by `generateFilename`).
//! - **Saved games:** the drawable's xfer of its custom sound (`DynamicAudioEventInfo::xferNoName`) and of the two enable
//!   flags is not translated (the client's drawables are not saved).

use crate::client::audio_event::AudioEventRts;
use crate::client::audio_info::{AudioEventInfo, AudioPriority, ST_GLOBAL};
use crate::client::game_audio::{AudioDevice, AudioHost, AudioManager};
use crate::common::game_type::ObjectId;
use crate::common::thing_template::ThingTemplateAudioType;
use crate::logic::module::body_module::{BODY_DAMAGED, BODY_PRISTINE, BODY_REALLYDAMAGED, BODY_RUBBLE};
use crate::logic::world::object_events::AmbientSoundProperties;
use crate::logic::world::object_map_props::{apply_ambient_sound_properties, override_bits, AmbientAudioInfo, AmbientSoundClient};
use std::collections::BTreeMap;
use std::sync::Arc;

/// `m_customSoundAmbientInfo`, when set.
#[derive(Clone, Debug, PartialEq)]
pub enum CustomAmbient {
    /// `getNoSoundMarker()`: "don't allow an ambient sound to be attached" (except for rubble).
    NoSound,
    /// The map object's own description, given to the audio under its mangled name.
    Info(Arc<AudioEventInfo>),
}

/// A drawable's ambient sound state.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawableAmbient {
    /// `m_ambientSound->m_event` (`None`: `m_ambientSound` is `NULL`).
    pub sound: Option<AudioEventRts>,
    /// `m_ambientSoundEnabled` (the logic's switch).
    pub enabled: bool,
    /// `m_ambientSoundEnabledFromScript` (the map designer's switch).
    pub enabled_from_script: bool,
    /// `m_customSoundAmbientInfo` (`None`: `NULL`).
    pub custom: Option<CustomAmbient>,
}

impl Default for DrawableAmbient {
    /// The constructor's: no sound, both switches on, no customisation.
    fn default() -> Self {
        DrawableAmbient { sound: None, enabled: true, enabled_from_script: true, custom: None }
    }
}

/// Every drawable's ambient sound, and `TheGameLogic->isLoadingMap()`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbientSounds {
    pub drawables: BTreeMap<ObjectId, DrawableAmbient>,
    /// `TheGameLogic->isLoadingMap()` (the logic's `LoadingMap` events).
    pub loading_map: bool,
}

/// `getAmbientSoundByDamage(dt)`: the template's sound for the damage state.
fn sound_slot_by_damage(dt: i32) -> ThingTemplateAudioType {
    match dt {
        BODY_DAMAGED => ThingTemplateAudioType::SoundAmbientDamaged,
        BODY_REALLYDAMAGED => ThingTemplateAudioType::SoundAmbientReallyDamaged,
        BODY_RUBBLE => ThingTemplateAudioType::SoundAmbientRubble,
        _ => ThingTemplateAudioType::SoundAmbient,
    }
}

impl AmbientSounds {
    /// `Drawable::Drawable`'s last step: "If we are inside GameLogic::startNewGame(), then starting the ambient sound will be
    /// taken care of by Drawable::onLevelStart()".
    pub fn create<D: AudioDevice>(&mut self, id: ObjectId, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        self.drawables.insert(id, DrawableAmbient::default());
        if !self.loading_map {
            self.start_ambient_sound(id, false, m, device, host);
        }
    }

    /// `~Drawable`: the sound stopped and dropped.
    pub fn destroy<D: AudioDevice>(&mut self, id: ObjectId, m: &mut AudioManager, device: &mut D) {
        self.stop_ambient_sound(id, m, device);
        self.drawables.remove(&id);
    }

    /// The drawable of `id`, made (as the client's drawables are) if an event names one the client has not seen.
    fn entry(&mut self, id: ObjectId) -> &mut DrawableAmbient {
        self.drawables.entry(id).or_default()
    }

    /// `reactToBodyDamageStateChange(newState)`'s sound: "When loading map, ambient sound starting is handled by
    /// onLevelStart()". (Upstream calls the three-argument `startAmbientSound`, which does not check the enable switches.)
    pub fn react_to_body_damage_state_change<D: AudioDevice>(&mut self, id: ObjectId, state: i32, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        self.entry(id);
        if !self.loading_map {
            self.start_ambient_sound_with(id, state, host.time_of_day(), false, m, device, host);
        }
    }

    /// `Drawable::update`'s ambient sound: a sound that is not playing (the audio culls looping sounds out of range) is
    /// started again if it loops for ever (or has no description). One-shot sounds are started only once.
    pub fn update<D: AudioDevice>(&mut self, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        for id in ids {
            let Some(d) = self.drawables.get_mut(&id) else { continue };
            let Some(e) = d.sound.as_mut() else { continue };
            if !(d.enabled && d.enabled_from_script && !e.event_name.is_empty() && !m.is_currently_playing(e.playing_handle)) {
                continue;
            }
            // "We'll need this in a second anyway so cache it"
            if e.event_info.is_none() {
                m.get_info_for_audio_event(e);
            }
            if e.event_info.as_ref().is_none_or(|i| i.is_permanent_sound()) {
                self.start_ambient_sound(id, false, m, device, host);
            }
        }
    }

    /// `onLevelStart()` of every drawable: "Make sure the current ambient sound is playing if it should be playing" (looping
    /// and one-shot sounds alike).
    pub fn on_level_start<D: AudioDevice>(&mut self, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        let ids: Vec<ObjectId> = self.drawables.keys().copied().collect();
        for id in ids {
            let Some(d) = self.drawables.get(&id) else { continue };
            let wants = d.enabled
                && d.enabled_from_script
                && d.sound.as_ref().is_none_or(|e| !e.event_name.is_empty() && !m.is_currently_playing(e.playing_handle));
            if wants {
                self.start_ambient_sound(id, false, m, device, host);
            }
        }
    }

    /// `startAmbientSound(dt, tod, onlyIfPermanent)`: the customised sound (unless rubble), else the template's for the
    /// damage state (the pristine one standing in for a missing damaged one), played if it is global or critical, or if
    /// the listener is within its range.
    #[allow(clippy::too_many_arguments)]
    pub fn start_ambient_sound_with<D: AudioDevice>(
        &mut self,
        id: ObjectId,
        dt: i32,
        tod: i32,
        only_if_permanent: bool,
        m: &mut AudioManager,
        device: &mut D,
        host: &dyn AudioHost,
    ) {
        self.stop_ambient_sound(id, m, device);
        let d = self.entry(id);
        let mut try_sound = false;
        // Look for customized sound info
        if dt != BODY_RUBBLE && d.custom.is_some() {
            if let Some(CustomAmbient::Info(info)) = &d.custom {
                let info = info.clone();
                // "Make sure m_event will accept the custom info"
                let e = d.sound.get_or_insert_with(AudioEventRts::default);
                e.event_name = info.audio_name.clone();
                e.event_info = Some(info);
                try_sound = true;
            }
        } else {
            // "Didn't get customized sound"
            let name = host.object_template_audio(id as u32, sound_slot_by_damage(dt));
            if !name.is_empty() {
                d.sound = Some(AudioEventRts::new(&name));
                try_sound = true;
            } else if dt != BODY_PRISTINE && dt != BODY_RUBBLE {
                // "Most of our cases actually specify just the pristine sound and want to use it for all states (except
                // dead/rubble)."
                let pristine = host.object_template_audio(id as u32, sound_slot_by_damage(BODY_PRISTINE));
                if !pristine.is_empty() {
                    d.sound = Some(AudioEventRts::new(&pristine));
                    try_sound = true;
                }
            }
        }
        if !try_sound {
            return;
        }
        let Some(mut e) = d.sound.take() else { return };
        // (the template's event had its description looked up when the INI was read; a copy is looked up here)
        if e.event_info.is_none() {
            m.get_info_for_audio_event(&mut e);
        }
        let Some(info) = e.event_info.clone() else {
            // "Ambient sound %s missing! Skipping..." (a debug crash): the sound is deleted
            return;
        };
        if !only_if_permanent || info.is_permanent_sound() {
            let play = if info.sound_type_bits & ST_GLOBAL != 0 || info.priority == AudioPriority::Critical {
                // Play it anyways.
                true
            } else {
                // "Check if it's close enough to try playing (optimization)"
                let p = host.drawable_position(id as u32).unwrap_or_default();
                let l = m.listener_position;
                let (x, y, z) = (p.x - l.x, p.y - l.y, p.z - l.z);
                x * x + y * y + z * z < info.max_distance * info.max_distance
            };
            if play {
                e.set_drawable_id(id as u32);
                e.time_of_day = tod;
                e.playing_handle = m.add_audio_event(host, &mut e);
            }
        }
        if let Some(d) = self.drawables.get_mut(&id) {
            d.sound = Some(e);
        }
    }

    /// `startAmbientSound(onlyIfPermanent)`: "Must go through enableAmbientSound() if sound is disabled"; the damage state is
    /// the body's.
    pub fn start_ambient_sound<D: AudioDevice>(&mut self, id: ObjectId, only_if_permanent: bool, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        let d = self.entry(id);
        if !d.enabled || !d.enabled_from_script {
            return;
        }
        self.stop_ambient_sound(id, m, device);
        let dt = host.object_damage_state(id as u32);
        self.start_ambient_sound_with(id, dt, host.time_of_day(), only_if_permanent, m, device, host);
    }

    /// `stopAmbientSound()`: the playing handle removed.
    pub fn stop_ambient_sound<D: AudioDevice>(&mut self, id: ObjectId, m: &mut AudioManager, device: &mut D) {
        if let Some(e) = self.drawables.get(&id).and_then(|d| d.sound.as_ref()) {
            m.remove_audio_event(device, e.playing_handle);
        }
    }

    /// `enableAmbientSound(enable)`: "Enable and disable ambient sound from the game logic".
    pub fn enable_ambient_sound<D: AudioDevice>(&mut self, id: ObjectId, enable: bool, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        let d = self.entry(id);
        if d.enabled == enable {
            return;
        }
        d.enabled = enable;
        if enable {
            if d.enabled_from_script {
                self.start_ambient_sound(id, false, m, device, host);
            }
        } else {
            self.stop_ambient_sound(id, m, device);
        }
    }

    /// `enableAmbientSoundFromScript(enable)`: "deliberately skipping if( m_ambientSoundEnabledFromScript == enable ) check
    /// here. Allow ENABLE_OBJECT_SOUND to trigger one-shot attached sound multiple times".
    pub fn enable_ambient_sound_from_script<D: AudioDevice>(&mut self, id: ObjectId, enable: bool, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        let d = self.entry(id);
        d.enabled_from_script = enable;
        if enable {
            if d.enabled {
                self.start_ambient_sound(id, false, m, device, host);
            }
        } else {
            self.stop_ambient_sound(id, m, device);
        }
    }

    /// `clearCustomSoundAmbient(restartSound)`: "Return to using default ambient sound".
    pub fn clear_custom_sound_ambient<D: AudioDevice>(&mut self, id: ObjectId, restart: bool, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        if let Some(e) = self.entry(id).sound.as_mut() {
            // "Make sure sound doesn't keep a reference to the deleted pointer"
            e.event_info = None;
        }
        self.stop_ambient_sound(id, m, device);
        self.entry(id).custom = None;
        if restart {
            self.start_ambient_sound(id, false, m, device, host);
        }
    }

    /// `setCustomSoundAmbientOff()`: "Force the Drawable to not ever get an ambient sound attached to it (except possibly for
    /// RUBBLE)".
    pub fn set_custom_sound_ambient_off<D: AudioDevice>(&mut self, id: ObjectId, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        self.clear_custom_sound_ambient(id, false, m, device, host);
        self.entry(id).custom = Some(CustomAmbient::NoSound);
    }

    /// `setCustomSoundAmbientInfo(info)`: "Force the Drawable to use the sound described by customAmbientInfo as its ambient
    /// sound."
    pub fn set_custom_sound_ambient_info<D: AudioDevice>(&mut self, id: ObjectId, info: Arc<AudioEventInfo>, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        self.clear_custom_sound_ambient(id, false, m, device, host);
        self.entry(id).custom = Some(CustomAmbient::Info(info));
        self.start_ambient_sound(id, false, m, device, host); // Note: checks for enabled flag
    }

    /// The map object's `objectSoundAmbient*` properties (`updateObjValuesFromMapProperties`), applied to the drawable of
    /// `id` ([`apply_ambient_sound_properties`]).
    pub fn apply_map_properties<D: AudioDevice>(&mut self, id: ObjectId, p: &AmbientSoundProperties, m: &mut AudioManager, device: &mut D, host: &dyn AudioHost) {
        self.entry(id);
        let mut c = PropsClient { ambient: self, id, m, device, host };
        apply_ambient_sound_properties(p, &mut c);
    }
}

/// The fields `updateObjValuesFromMapProperties` reads of a description.
fn ambient_info(i: &AudioEventInfo) -> AmbientAudioInfo {
    AmbientAudioInfo {
        name: i.audio_name.clone(),
        control: i.control,
        loop_count: i.loop_count,
        volume: i.volume,
        min_volume: i.min_volume,
        min_distance: i.min_distance,
        max_distance: i.max_distance,
        priority: i.priority as i32,
        overridden: 0,
    }
}

/// The drawable and the audio, as the map properties' sound block talks to them.
struct PropsClient<'a, D: AudioDevice> {
    ambient: &'a mut AmbientSounds,
    id: ObjectId,
    m: &'a mut AudioManager,
    device: &'a mut D,
    host: &'a dyn AudioHost,
}

impl<D: AudioDevice> AmbientSoundClient for PropsClient<'_, D> {
    fn set_custom_sound_ambient_off(&mut self) {
        self.ambient.set_custom_sound_ambient_off(self.id, self.m, self.device, self.host);
    }
    fn find_audio_event_info(&mut self, name: &[u8]) -> Option<AmbientAudioInfo> {
        self.m.tables.find_audio_event_info(name).map(|i| ambient_info(i))
    }
    /// `getBaseSoundAmbientInfo()`: the template's `SoundAmbient`'s description (`NULL` for no sound or one not described).
    fn base_sound_ambient_info(&mut self) -> Option<AmbientAudioInfo> {
        let name = self.host.object_template_audio(self.id as u32, ThingTemplateAudioType::SoundAmbient);
        self.m.tables.find_audio_event_info(&name).map(|i| ambient_info(i))
    }
    fn enable_ambient_sound_from_script(&mut self, on: bool) {
        self.ambient.enable_ambient_sound_from_script(self.id, on, self.m, self.device, self.host);
    }
    fn ambient_sound_enabled_from_script(&mut self) -> bool {
        self.ambient.entry(self.id).enabled_from_script
    }
    /// `newInstance(DynamicAudioEventInfo)(*baseInfo)` with the overrides, `mangleCustomAudioName` (`" CUSTOM %d "` and
    /// the name: "Note space at beginning prevents collision with any names from INI file"), `TheAudio->addAudioEventInfo`
    /// (a name already there is overwritten), then `setCustomSoundAmbientInfo`.
    fn install_custom_sound_ambient(&mut self, a: AmbientAudioInfo) {
        let Some(base) = self.m.tables.find_audio_event_info(&a.name) else { return };
        let mut info = AudioEventInfo::clone(base);
        if a.overridden & override_bits::LOOP_FLAG != 0 {
            info.control = a.control;
        }
        if a.overridden & override_bits::LOOP_COUNT != 0 {
            info.loop_count = a.loop_count;
        }
        if a.overridden & override_bits::VOLUME != 0 {
            info.volume = a.volume;
        }
        if a.overridden & override_bits::MIN_VOLUME != 0 {
            info.min_volume = a.min_volume;
        }
        if a.overridden & override_bits::MIN_RANGE != 0 {
            info.min_distance = a.min_distance;
        }
        if a.overridden & override_bits::MAX_RANGE != 0 {
            info.max_distance = a.max_distance;
        }
        if a.overridden & override_bits::PRIORITY != 0 {
            info.priority = AudioPriority::from_index(a.priority);
        }
        let mut name = format!(" CUSTOM {} ", self.id).into_bytes();
        name.extend_from_slice(&a.name);
        info.audio_name = name.clone();
        let info = Arc::new(info);
        self.m.tables.infos.insert(name, info.clone());
        self.ambient.set_custom_sound_ambient_info(self.id, info, self.m, self.device, self.host);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::audio_event::AudioEventHost;
    use crate::client::audio_info::AudioTables;
    use crate::client::game_audio::tests::LogDevice;
    use crate::common::base_type::Coord3D;
    use std::cell::Cell;

    /// Objects 7 (at x 100), 8 (x 900), 9 (x 1500), 10 and 11 (x 50), each with `SoundAmbient = Hum`; 7 also has
    /// `SoundAmbientRubble = Clatter`. The body's damage state is `dt`.
    struct AmbHost {
        dt: Cell<i32>,
    }
    impl AudioEventHost for AmbHost {
        fn does_file_exist(&self, _: &[u8]) -> bool {
            true
        }
        fn registry_language(&self) -> Vec<u8> {
            b"English".to_vec()
        }
        fn object_position(&self, id: u32) -> Option<Coord3D> {
            let x = match id {
                7 => 100.0,
                8 => 900.0,
                9 => 1500.0,
                _ => 50.0,
            };
            Some(Coord3D { x, y: 0.0, z: 0.0 })
        }
        fn drawable_position(&self, id: u32) -> Option<Coord3D> {
            self.object_position(id)
        }
        fn object_player_index(&self, _: u32) -> Option<i32> {
            Some(0)
        }
        fn drawable_player_index(&self, _: u32) -> Option<i32> {
            Some(0)
        }
    }
    impl AudioHost for AmbHost {
        fn local_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn listening_player_index(&self) -> Option<i32> {
            Some(0)
        }
        fn player_exists(&self, i: i32) -> bool {
            i == 0
        }
        fn player_has_default_team(&self, _: i32) -> bool {
            true
        }
        fn are_allies(&self, _: i32, _: i32) -> Option<bool> {
            Some(true)
        }
        fn shroud_clear(&self, _: i32, _: &Coord3D) -> bool {
            true
        }
        fn ground_height(&self, _: f32, _: f32) -> f32 {
            0.0
        }
        fn object_template_audio(&self, id: u32, t: ThingTemplateAudioType) -> Vec<u8> {
            match t {
                ThingTemplateAudioType::SoundAmbient => b"Hum".to_vec(),
                ThingTemplateAudioType::SoundAmbientRubble if id == 7 => b"Clatter".to_vec(),
                _ => Vec::new(),
            }
        }
        fn object_damage_state(&self, _: u32) -> i32 {
            self.dt.get()
        }
    }

    fn manager() -> AudioManager {
        let mut t = AudioTables::new();
        t.load_text(
            b"t.ini",
            b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n SoundsExtension = wav\n SampleCount2D = 4\n SampleCount3D = 8\n\
 MinSampleVolume = 1%\n DefaultSoundVolume = 100%\n Default3DSoundVolume = 100%\nEnd\n\
AudioEvent Hum\n Sounds = hum\n Type = world everyone\n Control = loop\n MinRange = 100\n MaxRange = 500\n Volume = 100%\nEnd\n\
AudioEvent Clatter\n Sounds = clatter\n Type = world everyone\n MinRange = 100\n MaxRange = 500\n Volume = 100%\nEnd\n",
        )
        .expect("the audio INI");
        AudioManager::new(t)
    }

    /// The names of the play requests still pending, in order.
    fn pending(m: &AudioManager) -> Vec<Vec<u8>> {
        m.requests.iter().filter_map(|r| r.pending.as_ref().ok().map(|e| e.event_name.clone())).collect()
    }

    /// #51: Drawable.cpp's ambient sound, translated. *Predictions, written before the first run* (the listener at the
    /// origin, so `Hum`'s 500 range reaches x 100 and x 50, not x 900 or x 1500):
    /// - made outside a map load, 7 plays `Hum`; 8 keeps `Hum` as its sound but does not play it (out of range): one
    ///   request;
    /// - 7 really damaged: no `SoundAmbientReallyDamaged`, so the pristine `Hum` again (the old handle stopped: a stop
    ///   request); rubble: `Clatter`;
    /// - the logic's switch off stops it; off again changes nothing; on, with the body rubble, plays `Clatter` again;
    /// - made during the map load, 9, 10 and 11 play nothing; 9's properties (customised, volume 50%, max range 2000)
    ///   install `" CUSTOM 9 Hum"` (a copy of `Hum`, volume 0.5, range 2000) and play it at once (upstream's
    ///   `setCustomSoundAmbientInfo` starts it, in the load too), in range now; 10's (`objectSoundAmbient` empty) turn its
    ///   sound off and disable it from script; `onLevelStart` then plays 7's sound again (its `Clatter` request was
    ///   dropped above, so it is not playing; the body is now pristine: `Hum`) and 11's `Hum` (9's is requested, 10 is
    ///   off, 8 is out of range); (I first wrote "11's `Hum` only", forgetting that `onLevelStart` visits 7 and 8 too, and
    ///   corrected it while reading the test over, before the first run);
    /// - with the requests dropped (the audio culled them), `update` starts the looping sounds again, in id order: 7's
    ///   `Hum`, 9's custom one and 11's `Hum` (8's is out of range, asked but not played).
    #[test]
    fn ambient_sounds_follow_the_drawable() {
        let mut m = manager();
        let mut dev = LogDevice::default();
        let host = AmbHost { dt: Cell::new(BODY_PRISTINE) };
        let mut a = AmbientSounds::default();
        a.create(7, &mut m, &mut dev, &host);
        a.create(8, &mut m, &mut dev, &host);
        assert_eq!(pending(&m), [b"Hum".to_vec()]);
        assert_eq!(a.drawables[&8].sound.as_ref().map(|e| e.event_name.clone()), Some(b"Hum".to_vec()));
        let first = a.drawables[&7].sound.as_ref().expect("7's sound").playing_handle;
        assert!(m.is_currently_playing(first));

        a.react_to_body_damage_state_change(7, BODY_REALLYDAMAGED, &mut m, &mut dev, &host);
        assert!(m.requests.iter().any(|r| r.pending == Err(first)), "the first Hum is stopped");
        assert_eq!(pending(&m), [b"Hum".to_vec(), b"Hum".to_vec()]);
        a.react_to_body_damage_state_change(7, BODY_RUBBLE, &mut m, &mut dev, &host);
        assert_eq!(pending(&m).last(), Some(&b"Clatter".to_vec()));

        host.dt.set(BODY_RUBBLE);
        m.requests.clear();
        a.enable_ambient_sound(7, false, &mut m, &mut dev, &host);
        assert_eq!((m.requests.len(), pending(&m).len()), (1, 0), "a stop");
        a.enable_ambient_sound(7, false, &mut m, &mut dev, &host);
        assert_eq!(m.requests.len(), 1, "no change");
        a.enable_ambient_sound(7, true, &mut m, &mut dev, &host);
        assert_eq!(pending(&m), [b"Clatter".to_vec()]);

        host.dt.set(BODY_PRISTINE);
        m.requests.clear();
        a.loading_map = true;
        for id in [9, 10, 11] {
            a.create(id, &mut m, &mut dev, &host);
        }
        assert!(m.requests.is_empty());
        let custom = AmbientSoundProperties { customized: Some(true), volume: Some(0.5), max_range: Some(2000.0), ..Default::default() };
        a.apply_map_properties(9, &custom, &mut m, &mut dev, &host);
        let info = m.tables.find_audio_event_info(b" CUSTOM 9 Hum").expect("the custom description").clone();
        assert_eq!((info.volume, info.max_distance, info.min_distance), (0.5, 2000.0, 100.0));
        assert_eq!(pending(&m), [b" CUSTOM 9 Hum".to_vec()]);
        let off = AmbientSoundProperties { name: Some(Vec::new()), ..Default::default() };
        a.apply_map_properties(10, &off, &mut m, &mut dev, &host);
        assert_eq!(a.drawables[&10].custom, Some(CustomAmbient::NoSound));
        assert!(!a.drawables[&10].enabled_from_script);
        a.on_level_start(&mut m, &mut dev, &host);
        a.loading_map = false;
        assert_eq!(pending(&m), [b" CUSTOM 9 Hum".to_vec(), b"Hum".to_vec(), b"Hum".to_vec()]);

        m.requests.clear();
        a.update(&mut m, &mut dev, &host);
        assert_eq!(pending(&m), [b"Hum".to_vec(), b" CUSTOM 9 Hum".to_vec(), b"Hum".to_vec()]);
    }
}
