// SPDX-License-Identifier: GPL-3.0-only
//
// Translated from (EA's release, https://github.com/electronicarts/CnC_Generals_Zero_Hour,
// commit 0a05454d8574207440a5fb15241b98ad0b435590):
//   GeneralsMD/Code/GameEngine/Include/Common/AudioEventRTS.h (OwnerType, PortionToPlay, class AudioEventRTS)
//   GeneralsMD/Code/GameEngine/Source/Common/Audio/AudioEventRTS.cpp (all of it: the constructors, setEventName,
//     generateFilename, generatePlayInfo, advanceNextPlayPortion, decreaseLoopCount, hasMoreLoops, getAudioEventInfo,
//     setPosition, setObjectID, setDrawableID, isPositionalAudio, getVolume, getCurrentPosition,
//     generateFilenamePrefix, generateFilenameExtension, adjustForLocalization, getPlayerIndex)
//   GeneralsMD/Code/GameEngine/Include/Common/AudioHandleSpecialValues.h
//
// Original copyright notice of those files:
//   Command & Conquer Generals Zero Hour(tm)
//   Copyright 2025 Electronic Arts Inc.
//   (c) 2001-2003 Electronic Arts Inc.
//
// Licensed under the GNU General Public License version 3 (GPL-3.0-only for
// this translation), with the additional terms in NOTICE.md.
// Translated to Rust and modified; not the original program.

//! One request to play a sound (GitHub #13, C6): `AudioEventRTS`, with the choice of the file it plays.
//!
//! # As upstream does it
//!
//! `generateFilename` builds `<AudioRoot>\<folder>\<file>` (the folder by type: `SoundsFolder`, `StreamingFolder` or
//! `MusicFolder`), and, for a sound effect, picks one of the event's `Sounds`: with `Control = RANDOM` a random one,
//! not the one played last when there are more than two; else the next in turn. It appends `.<SoundsExtension>` (not
//! for music), then prefers the localized file `<AudioRoot>\<folder>\<language>\<file>` when the file system has it.
//! The random value is the **audio** generator's (`GameAudioRandomValue`), or the **logic** generator's for a
//! "logical" event (scripts set that, so every machine picks the same file). `generatePlayInfo` draws the pitch and
//! volume shifts and the attack and decay files.
//!
//! # Where this differs from upstream
//!
//! - **The object and drawable ids share one field** ([`AudioEventRts::owner_id`]), as upstream's union, which is
//!   initialised to 0 here (upstream leaves it uninitialised in the plain and positional constructors, where only the
//!   owner type is then read) and copied whatever the owner type (upstream's copy constructor copies it only for an
//!   object or a drawable).
//! - **The logic generator** of a logical event is the client's own [`RandomValues::logic`] state, never the World's: the
//!   client never changes the simulation (`AGENTS.md`). Upstream advances the logic's generator there.
//! - **The description** is an `Arc` (upstream: a pointer, mutable through `const`).
//! - `adjustForLocalization` keeps upstream's double backslash (`reverseFind` returns the separator, which is appended
//!   after the prefix's own); the file system skips empty path components, so the file is found as upstream finds it.

use crate::client::audio_info::{AudioEventInfo, AudioPriority, AudioSettings, AudioType, AC_ALL, AC_RANDOM, ST_WORLD};
use crate::common::base_type::Coord3D;
use crate::common::random_value::{random_int, random_real, RandomValues};
use std::sync::Arc;

/// `AudioHandle`.
pub type AudioHandle = u32;
/// `AHSV_Error`.
pub const AHSV_ERROR: AudioHandle = 0;
/// `AHSV_NoSound`.
pub const AHSV_NO_SOUND: AudioHandle = 1;
/// `AHSV_Muted`.
pub const AHSV_MUTED: AudioHandle = 2;
/// `AHSV_NotForLocal`.
pub const AHSV_NOT_FOR_LOCAL: AudioHandle = 3;
/// `AHSV_StopTheMusic`.
pub const AHSV_STOP_THE_MUSIC: AudioHandle = 4;
/// `AHSV_StopTheMusicFade`.
pub const AHSV_STOP_THE_MUSIC_FADE: AudioHandle = 5;
/// `AHSV_FirstHandle`.
pub const AHSV_FIRST_HANDLE: AudioHandle = 6;

/// `enum OwnerType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerType {
    Positional,
    Drawable,
    Object,
    Dead,
    Invalid,
}

/// `enum PortionToPlay`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PortionToPlay {
    #[default]
    Attack,
    Sound,
    Decay,
    Done,
}

/// `enum TimeOfDay` (`TIME_OF_DAY_AFTERNOON` is an event's default).
pub const TIME_OF_DAY_AFTERNOON: i32 = 2;

/// What an audio event reads of the game: the files, the language, and its owner's position and player.
pub trait AudioEventHost {
    /// `TheFileSystem->doesFileExist(path)`.
    fn does_file_exist(&self, path: &[u8]) -> bool;
    /// `GetRegistryLanguage()`.
    fn registry_language(&self) -> Vec<u8>;
    /// `TheGameLogic->findObjectByID(id)->getPosition()`.
    fn object_position(&self, id: u32) -> Option<Coord3D>;
    /// `TheGameClient->findDrawableByID(id)->getPosition()`.
    fn drawable_position(&self, id: u32) -> Option<Coord3D>;
    /// The object's `getControllingPlayer()->getPlayerIndex()`.
    fn object_player_index(&self, id: u32) -> Option<i32>;
    /// The drawable's object's controlling player's index (`None` without a drawable or an object).
    fn drawable_player_index(&self, id: u32) -> Option<i32>;
}

/// `class AudioEventRTS`.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioEventRts {
    /// `m_filenameToLoad`.
    pub filename_to_load: Vec<u8>,
    /// `m_eventInfo`.
    pub event_info: Option<Arc<AudioEventInfo>>,
    /// `m_playingHandle`.
    pub playing_handle: AudioHandle,
    /// `m_killThisHandle`.
    pub kill_this_handle: AudioHandle,
    /// `m_eventName`.
    pub event_name: Vec<u8>,
    /// `m_attackName`.
    pub attack_name: Vec<u8>,
    /// `m_decayName`.
    pub decay_name: Vec<u8>,
    /// `m_priority`.
    pub priority: AudioPriority,
    /// `m_volume` (-1: the description's).
    pub volume: f32,
    /// `m_timeOfDay`.
    pub time_of_day: i32,
    /// `m_positionOfAudio`.
    pub position_of_audio: Coord3D,
    /// The union of `m_objectID` and `m_drawableID`.
    pub owner_id: u32,
    /// `m_ownerType`.
    pub owner_type: OwnerType,
    /// `m_shouldFade`.
    pub should_fade: bool,
    /// `m_isLogicalAudio`.
    pub is_logical_audio: bool,
    /// `m_uninterruptable`.
    pub uninterruptable: bool,
    /// `m_pitchShift`.
    pub pitch_shift: f32,
    /// `m_volumeShift`.
    pub volume_shift: f32,
    /// `m_delay` (ms).
    pub delay: f32,
    /// `m_loopCount`.
    pub loop_count: i32,
    /// `m_playingAudioIndex`.
    pub playing_audio_index: i32,
    /// `m_allCount`.
    pub all_count: i32,
    /// `m_playerIndex`.
    pub player_index: i32,
    /// `m_portionToPlayNext` (uninitialised by upstream's constructors; set by `generatePlayInfo`).
    pub portion_to_play_next: PortionToPlay,
}

impl Default for AudioEventRts {
    /// `AudioEventRTS()`.
    fn default() -> Self {
        AudioEventRts {
            filename_to_load: Vec::new(),
            event_info: None,
            playing_handle: 0,
            kill_this_handle: 0,
            event_name: Vec::new(),
            attack_name: Vec::new(),
            decay_name: Vec::new(),
            priority: AudioPriority::Normal,
            volume: -1.0,
            time_of_day: TIME_OF_DAY_AFTERNOON,
            position_of_audio: Coord3D::default(),
            owner_id: 0,
            owner_type: OwnerType::Invalid,
            should_fade: false,
            is_logical_audio: false,
            uninterruptable: false,
            pitch_shift: 1.0,
            volume_shift: 0.0,
            delay: 0.0,
            loop_count: 1,
            playing_audio_index: -1,
            all_count: 0,
            player_index: -1,
            portion_to_play_next: PortionToPlay::Attack,
        }
    }
}

impl AudioEventRts {
    /// `AudioEventRTS(eventName)`.
    pub fn new(name: &[u8]) -> Self {
        AudioEventRts { event_name: name.to_vec(), ..Default::default() }
    }

    /// `AudioEventRTS(eventName, ObjectID)`: owned by the object unless the id is `INVALID_ID` (0).
    pub fn with_object(name: &[u8], id: u32) -> Self {
        let mut e = Self::new(name);
        e.owner_id = id;
        if id != 0 {
            e.owner_type = OwnerType::Object;
        }
        e
    }

    /// `AudioEventRTS(eventName, DrawableID)`.
    pub fn with_drawable(name: &[u8], id: u32) -> Self {
        let mut e = Self::new(name);
        e.owner_id = id;
        if id != 0 {
            e.owner_type = OwnerType::Drawable;
        }
        e
    }

    /// `AudioEventRTS(eventName, const Coord3D *)`.
    pub fn at_position(name: &[u8], pos: &Coord3D) -> Self {
        AudioEventRts { event_name: name.to_vec(), owner_type: OwnerType::Positional, position_of_audio: *pos, ..Default::default() }
    }

    /// `setEventName(name)`: a new name drops the description.
    pub fn set_event_name(&mut self, name: &[u8]) {
        if name != self.event_name.as_slice() && self.event_info.is_some() {
            self.event_info = None;
        }
        self.event_name = name.to_vec();
    }

    /// `getAudioEventInfo()`: the description, dropped (and `None`) if it is not this event's name's.
    pub fn get_audio_event_info(&mut self) -> Option<&Arc<AudioEventInfo>> {
        if self.event_info.as_ref().is_some_and(|i| i.audio_name != self.event_name) {
            self.event_info = None;
        }
        self.event_info.as_ref()
    }

    /// The description without upstream's name check (the manager's events always match theirs).
    pub fn info(&self) -> &AudioEventInfo {
        self.event_info.as_ref().expect("an event with its description")
    }

    /// `generateFilenamePrefix(type, localized)`.
    pub fn generate_filename_prefix(settings: &AudioSettings, ty: AudioType, localized: Option<&[u8]>) -> Vec<u8> {
        let mut r = settings.audio_root.clone();
        r.push(b'\\');
        r.extend_from_slice(match ty {
            AudioType::Music => &settings.music_folder,
            AudioType::Streaming => &settings.streaming_folder,
            AudioType::SoundEffect => &settings.sounds_folder,
        });
        r.push(b'\\');
        if let Some(lang) = localized {
            r.extend_from_slice(lang);
            r.push(b'\\');
        }
        r
    }

    /// `generateFilenameExtension(type)`: `.<SoundsExtension>`, nothing for music.
    pub fn generate_filename_extension(settings: &AudioSettings, ty: AudioType) -> Vec<u8> {
        if ty == AudioType::Music {
            return Vec::new();
        }
        let mut r = b".".to_vec();
        r.extend_from_slice(&settings.sounds_extension);
        r
    }

    /// `adjustForLocalization(str)`: the localized path, if the file system has it.
    fn adjust_for_localization(&self, settings: &AudioSettings, host: &dyn AudioEventHost, s: &mut Vec<u8>) {
        let Some(cut) = s.iter().rposition(|&c| c == b'\\') else { return };
        let lang = host.registry_language();
        let mut localized = Self::generate_filename_prefix(settings, self.info().sound_type, Some(&lang));
        localized.extend_from_slice(&s[cut..]);
        if host.does_file_exist(&localized) {
            *s = localized;
        }
    }

    /// `generateFilename()`: the file to play (see the module docs) and, for a sound effect, the delay before it.
    pub fn generate_filename(&mut self, settings: &AudioSettings, host: &dyn AudioEventHost, random: &mut RandomValues) {
        let Some(info) = self.event_info.clone() else { return };
        self.filename_to_load = Self::generate_filename_prefix(settings, info.sound_type, None);
        if info.sound_type == AudioType::Music || info.sound_type == AudioType::Streaming {
            self.filename_to_load.extend_from_slice(&info.filename);
            let mut f = std::mem::take(&mut self.filename_to_load);
            self.adjust_for_localization(settings, host, &mut f);
            self.filename_to_load = f;
            return;
        }
        let n = info.sounds.len();
        if n == 0 {
            self.filename_to_load.clear();
            return;
        }
        let which: usize;
        if info.control & AC_RANDOM != 0 {
            let mut w = if self.is_logical_audio { random_int(&mut random.logic, 0, n as i32 - 1) } else { random_int(&mut random.audio, 0, n as i32 - 1) };
            if w == self.playing_audio_index && n > 2 {
                w = (w + 1) % n as i32;
            }
            self.playing_audio_index = w;
            which = w as usize;
        } else {
            // `(++m_playingAudioIndex) % m_sounds.size()`: the Int converts to the unsigned size
            self.playing_audio_index = self.playing_audio_index.wrapping_add(1);
            which = (self.playing_audio_index as u32 as usize) % n;
        }
        self.filename_to_load.extend_from_slice(&info.sounds[which]);
        self.filename_to_load.extend_from_slice(&Self::generate_filename_extension(settings, info.sound_type));
        let mut f = std::mem::take(&mut self.filename_to_load);
        self.adjust_for_localization(settings, host, &mut f);
        self.filename_to_load = f;
        self.delay = random_real(&mut random.audio, info.delay_min as f32, info.delay_max as f32);
    }

    /// One of the attack or decay files (`generatePlayInfo`'s two blocks).
    fn pick_piece(&self, settings: &AudioSettings, host: &dyn AudioEventHost, random: &mut RandomValues, list: &[Vec<u8>]) -> Vec<u8> {
        let ty = self.info().sound_type;
        let mut name = Self::generate_filename_prefix(settings, ty, None);
        let n = list.len() as i32;
        let i = if self.is_logical_audio { random_int(&mut random.logic, 0, n - 1) } else { random_int(&mut random.audio, 0, n - 1) };
        name.extend_from_slice(&list[i as usize]);
        name.extend_from_slice(&Self::generate_filename_extension(settings, ty));
        self.adjust_for_localization(settings, host, &mut name);
        name
    }

    /// `generatePlayInfo()`: the pitch shift, the volume shift (between `1 + VolumeShift` and 1), the loop count, the
    /// attack and decay files; the event is no longer logical after it.
    pub fn generate_play_info(&mut self, settings: &AudioSettings, host: &dyn AudioEventHost, random: &mut RandomValues) {
        let info = self.event_info.clone().expect("generatePlayInfo reads the description");
        self.pitch_shift = random_real(&mut random.audio, info.pitch_shift_min, info.pitch_shift_max);
        self.volume_shift = random_real(&mut random.audio, 1.0f32 + info.volume_shift, 1.0);
        self.loop_count = info.loop_count;
        self.portion_to_play_next = PortionToPlay::Attack;
        if !info.attack_sounds.is_empty() {
            self.attack_name = self.pick_piece(settings, host, random, &info.attack_sounds);
        } else {
            self.portion_to_play_next = PortionToPlay::Sound;
        }
        if !info.decay_sounds.is_empty() {
            self.decay_name = self.pick_piece(settings, host, random, &info.decay_sounds);
        }
        self.is_logical_audio = false;
    }

    /// `decrementDelay(t)`.
    pub fn decrement_delay(&mut self, t: f32) {
        self.delay -= t;
    }

    /// `advanceNextPlayPortion()`.
    pub fn advance_next_play_portion(&mut self) {
        match self.portion_to_play_next {
            PortionToPlay::Attack => self.portion_to_play_next = PortionToPlay::Sound,
            PortionToPlay::Sound => {
                if let Some(info) = &self.event_info {
                    if info.control & AC_ALL != 0 {
                        if self.all_count as usize == info.sounds.len() {
                            self.portion_to_play_next = PortionToPlay::Decay;
                        }
                        self.all_count += 1;
                    }
                }
                self.portion_to_play_next = if !self.decay_name.is_empty() { PortionToPlay::Decay } else { PortionToPlay::Done };
            }
            PortionToPlay::Decay => self.portion_to_play_next = PortionToPlay::Done,
            PortionToPlay::Done => {}
        }
    }

    /// `decreaseLoopCount()`: 1 becomes -1 (done); more counts down; 0 (forever) stays.
    pub fn decrease_loop_count(&mut self) {
        if self.loop_count == 1 {
            self.loop_count = -1;
        } else if self.loop_count > 1 {
            self.loop_count -= 1;
        }
    }

    /// `hasMoreLoops()`.
    pub fn has_more_loops(&self) -> bool {
        self.loop_count >= 0
    }

    /// `setPosition(pos)`: only for a positional or unowned event.
    pub fn set_position(&mut self, pos: &Coord3D) {
        if !(self.owner_type == OwnerType::Positional || self.owner_type == OwnerType::Invalid) {
            return;
        }
        self.position_of_audio = *pos;
        self.owner_type = OwnerType::Positional;
    }

    /// `setObjectID(id)`: only for an object's or unowned event.
    pub fn set_object_id(&mut self, id: u32) {
        if !(self.owner_type == OwnerType::Object || self.owner_type == OwnerType::Invalid) {
            return;
        }
        self.owner_id = id;
        self.owner_type = OwnerType::Object;
    }

    /// `getObjectID()`.
    pub fn get_object_id(&self) -> u32 {
        if self.owner_type == OwnerType::Object {
            self.owner_id
        } else {
            0
        }
    }

    /// `setDrawableID(id)`.
    pub fn set_drawable_id(&mut self, id: u32) {
        if !(self.owner_type == OwnerType::Drawable || self.owner_type == OwnerType::Invalid) {
            return;
        }
        self.owner_id = id;
        self.owner_type = OwnerType::Drawable;
    }

    /// `isDead()`.
    pub fn is_dead(&self) -> bool {
        self.owner_type == OwnerType::Dead
    }

    /// `isPositionalAudio()`: a `WORLD` sound with an owner or a position.
    pub fn is_positional_audio(&self) -> bool {
        if let Some(i) = &self.event_info {
            if i.sound_type_bits & ST_WORLD == 0 {
                return false;
            }
        }
        self.owner_type != OwnerType::Invalid && (self.owner_id != 0 || self.owner_type == OwnerType::Positional)
    }

    /// `getVolume()`: the override, else the description's, else 0.5.
    pub fn get_volume(&self) -> f32 {
        if self.volume == -1.0 {
            return self.event_info.as_ref().map_or(0.5, |i| i.volume);
        }
        self.volume
    }

    /// `getCurrentPosition()`: the position, following the owner; an owner gone makes the event dead (at its last place).
    pub fn get_current_position(&mut self, host: &dyn AudioEventHost) -> Option<Coord3D> {
        match self.owner_type {
            OwnerType::Positional | OwnerType::Dead => Some(self.position_of_audio),
            OwnerType::Object | OwnerType::Drawable => {
                let p = if self.owner_type == OwnerType::Object { host.object_position(self.owner_id) } else { host.drawable_position(self.owner_id) };
                match p {
                    Some(p) => self.position_of_audio = p,
                    None => self.owner_type = OwnerType::Dead,
                }
                Some(self.position_of_audio)
            }
            OwnerType::Invalid => None,
        }
    }

    /// `getPlayerIndex()`: the owner's controlling player, else the index set.
    pub fn get_player_index(&self, host: &dyn AudioEventHost) -> i32 {
        let owner = match self.owner_type {
            OwnerType::Object => host.object_player_index(self.owner_id),
            OwnerType::Drawable => host.drawable_player_index(self.owner_id),
            _ => None,
        };
        owner.unwrap_or(self.player_index)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::client::audio_info::{AudioTables, AC_INTERRUPT};

    /// A host with a set of files, English, and objects at fixed places.
    #[derive(Default)]
    pub struct TestHost {
        pub files: Vec<Vec<u8>>,
        pub objects: Vec<(u32, Coord3D, i32)>,
    }
    impl AudioEventHost for TestHost {
        fn does_file_exist(&self, p: &[u8]) -> bool {
            self.files.iter().any(|f| f == p)
        }
        fn registry_language(&self) -> Vec<u8> {
            b"English".to_vec()
        }
        fn object_position(&self, id: u32) -> Option<Coord3D> {
            self.objects.iter().find(|o| o.0 == id).map(|o| o.1)
        }
        fn drawable_position(&self, id: u32) -> Option<Coord3D> {
            self.object_position(id)
        }
        fn object_player_index(&self, id: u32) -> Option<i32> {
            self.objects.iter().find(|o| o.0 == id).map(|o| o.2)
        }
        fn drawable_player_index(&self, id: u32) -> Option<i32> {
            self.object_player_index(id)
        }
    }

    pub fn tables() -> AudioTables {
        let mut t = AudioTables::new();
        t.load_text(
            b"t.ini",
            b"AudioSettings\n AudioRoot = Data\\Audio\n SoundsFolder = Sounds\n MusicFolder = Tracks\n StreamingFolder = Speech\n SoundsExtension = wav\nEnd\n\
AudioEvent Seq\n Sounds = a b c\n Type = world everyone\n Volume = 90%\nEnd\n\
AudioEvent Rnd\n Sounds = a b c d\n Control = random\n Delay = 10 20\n PitchShift = -20 20\n VolumeShift = -30%\n Attack = at1 at2\n Decay = de\n Type = ui everyone\nEnd\n\
AudioEvent Two\n Sounds = a b\n Control = random interrupt\nEnd\n\
DialogEvent Talk\n Filename = talk.wav\nEnd\n\
MusicTrack Song\n Filename = song.mp3\nEnd\n",
        )
        .expect("parses");
        assert_eq!(t.find_audio_event_info(b"Two").unwrap().control, AC_RANDOM | AC_INTERRUPT);
        t
    }

    fn event(t: &AudioTables, name: &[u8]) -> AudioEventRts {
        let mut e = AudioEventRts::new(name);
        e.event_info = t.find_audio_event_info(name).cloned();
        e
    }

    /// Predicted: without `RANDOM` the sounds play in turn from the first (`a`, `b`, `c`, `a`), each with the sounds
    /// folder and `.wav`; a localized file is preferred, with upstream's double backslash; speech takes the streaming
    /// folder and its `Filename` unchanged, music the tracks folder and no extension.
    #[test]
    fn file_names_in_turn_localized_and_by_type() {
        let t = tables();
        let mut host = TestHost::default();
        let mut r = RandomValues::new(1);
        let mut e = event(&t, b"Seq");
        let mut names = Vec::new();
        for _ in 0..4 {
            e.generate_filename(&t.settings, &host, &mut r);
            names.push(String::from_utf8(e.filename_to_load.clone()).unwrap());
        }
        assert_eq!(names, ["Data\\Audio\\Sounds\\a.wav", "Data\\Audio\\Sounds\\b.wav", "Data\\Audio\\Sounds\\c.wav", "Data\\Audio\\Sounds\\a.wav"]);
        host.files.push(b"Data\\Audio\\Sounds\\English\\\\b.wav".to_vec());
        e.generate_filename(&t.settings, &host, &mut r);
        assert_eq!(e.filename_to_load, b"Data\\Audio\\Sounds\\English\\\\b.wav");
        let mut s = event(&t, b"Talk");
        s.generate_filename(&t.settings, &host, &mut r);
        assert_eq!(s.filename_to_load, b"Data\\Audio\\Speech\\talk.wav");
        let mut m = event(&t, b"Song");
        m.generate_filename(&t.settings, &host, &mut r);
        assert_eq!(m.filename_to_load, b"Data\\Audio\\Tracks\\song.mp3");
    }

    /// Predicted: with `RANDOM`, each pick is the audio generator's draw in 0..=3 (checked against a copy of the
    /// generator), bumped to the next when it repeats the last (four sounds, more than two), so no two picks in a row
    /// are the same; then the delay is the audio draw in 10..20; with two sounds a repeat is allowed. The logic stream is
    /// used only for a logical event, and not again after `generatePlayInfo`.
    #[test]
    fn random_picks_avoid_repeats_and_use_the_audio_generator() {
        let t = tables();
        let host = TestHost::default();
        let mut r = RandomValues::new(7);
        let mut copy = r.clone();
        let mut e = event(&t, b"Rnd");
        let mut last = -1;
        for _ in 0..50 {
            e.generate_filename(&t.settings, &host, &mut r);
            let mut w = random_int(&mut copy.audio, 0, 3);
            if w == last {
                w = (w + 1) % 4;
            }
            assert_ne!(w, last);
            last = w;
            let delay = random_real(&mut copy.audio, 10.0, 20.0);
            assert_eq!(e.playing_audio_index, w);
            assert_eq!(e.filename_to_load.last_chunk::<5>().unwrap()[0], b"abcd"[w as usize]);
            assert_eq!(e.delay, delay);
            assert!((10.0..=20.0).contains(&e.delay));
        }
        assert_eq!(r, copy);
        // two sounds: repeats allowed
        let mut two = event(&t, b"Two");
        let mut repeats = 0;
        let mut prev = -2;
        for _ in 0..40 {
            two.generate_filename(&t.settings, &host, &mut r);
            repeats += i32::from(two.playing_audio_index == prev);
            prev = two.playing_audio_index;
        }
        assert!(repeats > 0, "with two sounds the same one may follow itself");
        // a logical event draws its pick from the logic stream
        let before = r.clone();
        e.is_logical_audio = true;
        e.generate_filename(&t.settings, &host, &mut r);
        assert_ne!(r.logic, before.logic);
        e.generate_play_info(&t.settings, &host, &mut r);
        assert!(!e.is_logical_audio);
    }

    /// Predicted: `generatePlayInfo` draws pitch in 0.8..1.2 and volume shift in 0.7..1.0 from the audio generator,
    /// picks an attack (`at1` or `at2`) and the decay `de`, and starts at the attack; the portions then go attack,
    /// sound, decay, done. Without attack it starts at the sound. Loop counts: 1 ends after one decrease, 3 after three,
    /// 0 never.
    #[test]
    fn play_info_portions_and_loops() {
        let t = tables();
        let host = TestHost::default();
        let mut r = RandomValues::new(3);
        let mut e = event(&t, b"Rnd");
        e.generate_play_info(&t.settings, &host, &mut r);
        assert!((0.8..=1.2).contains(&e.pitch_shift), "{}", e.pitch_shift);
        assert!((0.7..=1.0).contains(&e.volume_shift), "{}", e.volume_shift);
        assert!(e.attack_name == b"Data\\Audio\\Sounds\\at1.wav" || e.attack_name == b"Data\\Audio\\Sounds\\at2.wav");
        assert_eq!(e.decay_name, b"Data\\Audio\\Sounds\\de.wav");
        let mut order = vec![e.portion_to_play_next];
        for _ in 0..3 {
            e.advance_next_play_portion();
            order.push(e.portion_to_play_next);
        }
        assert_eq!(order, [PortionToPlay::Attack, PortionToPlay::Sound, PortionToPlay::Decay, PortionToPlay::Done]);
        let mut s = event(&t, b"Seq");
        s.generate_play_info(&t.settings, &host, &mut r);
        assert_eq!(s.portion_to_play_next, PortionToPlay::Sound);
        for (count, loops) in [(1, 1), (3, 3)] {
            s.loop_count = count;
            let mut n = 0;
            while s.has_more_loops() {
                s.decrease_loop_count();
                n += 1;
            }
            assert_eq!(n, loops);
        }
        s.loop_count = 0;
        s.decrease_loop_count();
        assert!(s.has_more_loops());
    }

    /// Predicted: an object's event follows the object and takes its player; when the object is gone the event is dead
    /// at the last place. A `UI` description is never positional; a `WORLD` one is with an owner, not without.
    #[test]
    fn owners_positions_and_players() {
        let t = tables();
        let mut host = TestHost { files: vec![], objects: vec![(5, Coord3D { x: 1.0, y: 2.0, z: 3.0 }, 2)] };
        let mut e = AudioEventRts::with_object(b"Seq", 5);
        e.event_info = t.find_audio_event_info(b"Seq").cloned();
        assert!(e.is_positional_audio());
        assert_eq!(e.get_player_index(&host), 2);
        assert_eq!(e.get_current_position(&host).unwrap().x, 1.0);
        host.objects.clear();
        assert_eq!(e.get_current_position(&host).unwrap().y, 2.0);
        assert!(e.is_dead());
        assert_eq!(e.get_player_index(&host), -1);
        e.set_position(&Coord3D::default());
        assert!(e.is_dead(), "a dead event takes no new position");
        let mut ui = event(&t, b"Rnd");
        ui.set_position(&Coord3D::default());
        assert!(!ui.is_positional_audio());
        let mut w = event(&t, b"Seq");
        assert!(!w.is_positional_audio());
        w.set_object_id(9);
        assert!(w.is_positional_audio());
        assert_eq!(w.get_volume(), 0.9);
        w.volume = 0.25;
        assert_eq!(w.get_volume(), 0.25);
        w.set_event_name(b"Other");
        assert!(w.event_info.is_none());
    }
}
